//! Configuration persistante (serveur, préférences) et session (cookies du serveur).
//!
//! - `~/.config/looty/config.toml` : adresse du serveur et préférences ;
//! - `~/.local/share/looty/session.json` (0600) : cookies de session du serveur.
//!   La session appartient au processus Rust, pas à la vue web : la barre système et
//!   les notifications fonctionnent même fenêtre fermée.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Adresse du serveur Looty (ex. https://looty.example.com), sans barre finale.
    pub server: Option<String>,
    /// Fermer la fenêtre la cache dans la barre système au lieu de quitter.
    #[serde(default = "yes")]
    pub close_to_tray: bool,
    /// Notifications système.
    #[serde(default = "yes")]
    pub notifications: bool,
    /// Port local (stable : l'origine de la vue web, donc ses données, en dépend).
    #[serde(default)]
    pub port: Option<u16>,
}

fn yes() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Config { server: None, close_to_tray: true, notifications: true, port: None }
    }
}

fn config_path() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("looty").join("config.toml")
}

pub fn data_dir() -> PathBuf {
    let d = dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("looty");
    let _ = std::fs::create_dir_all(&d);
    d
}

fn session_path() -> PathBuf {
    data_dir().join("session.json")
}

impl Config {
    pub fn load() -> Config {
        std::fs::read_to_string(config_path()).ok().and_then(|s| toml::from_str(&s).ok()).unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = config_path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, toml::to_string_pretty(self).unwrap_or_default())
    }
}

pub type Jar = Arc<reqwest_cookie_store::CookieStoreMutex>;

// ---------------------------------------------------------------------------
// Chiffrement de la session avec une clé du trousseau système
// ---------------------------------------------------------------------------
//
// La clé (32 octets aléatoires) est rangée dans le trousseau (Secret Service : GNOME
// Keyring, KWallet…) ; la session est alors écrite chiffrée (AES-256-GCM) dans
// `session.enc`. Sans trousseau disponible, repli sur `session.json` (0600).

static SESSION_KEY: std::sync::OnceLock<Option<[u8; 32]>> = std::sync::OnceLock::new();

/// À appeler une fois au démarrage (hors du runtime asynchrone) : lit ou crée la clé.
pub fn init_session_key() -> bool {
    SESSION_KEY
        .get_or_init(|| {
            // Trousseau verrouillé ou absent : on ne bloque pas le démarrage.
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _ = tx.send(keyring_key());
            });
            match rx.recv_timeout(std::time::Duration::from_secs(5)) {
                Ok(k) => k,
                Err(_) => {
                    tracing::warn!("system keyring did not answer: session stored unencrypted (0600)");
                    None
                }
            }
        })
        .is_some()
}

fn keyring_key() -> Option<[u8; 32]> {
    let entry = keyring::Entry::new("looty-desktop", "session-key").ok()?;
    let decode = |s: &str| -> Option<[u8; 32]> { hex::decode(s.trim()).ok().and_then(|b| b.try_into().ok()) };
    match entry.get_password() {
        Ok(s) => decode(&s),
        Err(keyring::Error::NoEntry) => {
            let mut k = [0u8; 32];
            rand::Rng::fill(&mut rand::thread_rng(), &mut k);
            match entry.set_password(&hex::encode(k)) {
                Ok(()) => Some(k),
                Err(e) => {
                    tracing::warn!("cannot store the session key in the keyring: {e}");
                    None
                }
            }
        }
        Err(e) => {
            tracing::info!("no system keyring ({e}): session stored unencrypted (0600)");
            None
        }
    }
}

fn key() -> Option<&'static [u8; 32]> {
    SESSION_KEY.get().and_then(|k| k.as_ref())
}

fn enc_path() -> PathBuf {
    data_dir().join("session.enc")
}

fn encrypt(key: &[u8; 32], plain: &[u8]) -> Option<Vec<u8>> {
    use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
    let cipher = aes_gcm::Aes256Gcm::new(key.into());
    let nonce = aes_gcm::Aes256Gcm::generate_nonce(&mut OsRng);
    let mut out = nonce.to_vec();
    out.extend(cipher.encrypt(&nonce, plain).ok()?);
    Some(out)
}

fn decrypt(key: &[u8; 32], data: &[u8]) -> Option<Vec<u8>> {
    use aes_gcm::aead::{Aead, KeyInit};
    if data.len() < 12 {
        return None;
    }
    let cipher = aes_gcm::Aes256Gcm::new(key.into());
    cipher.decrypt(aes_gcm::Nonce::from_slice(&data[..12]), &data[12..]).ok()
}

pub fn load_jar() -> Jar {
    let parse = |bytes: &[u8]| cookie_store::serde::json::load(std::io::BufReader::new(bytes)).ok();
    let store = key()
        .and_then(|k| std::fs::read(enc_path()).ok().and_then(|d| decrypt(k, &d)))
        .and_then(|b| parse(&b))
        // Session en clair (première version, ou pas de trousseau) : migrée au prochain enregistrement.
        .or_else(|| std::fs::read(session_path()).ok().and_then(|b| parse(&b)))
        .unwrap_or_default();
    Arc::new(reqwest_cookie_store::CookieStoreMutex::new(store))
}

/// Enregistre la session (chiffrée si le trousseau est disponible).
pub fn save_jar(jar: &Jar) {
    let res = (|| -> std::io::Result<()> {
        let mut json = Vec::new();
        {
            let store = jar.lock().map_err(|_| std::io::Error::other("cookie store poisoned"))?;
            cookie_store::serde::json::save_incl_expired_and_nonpersistent(&store, &mut json).map_err(|e| std::io::Error::other(e.to_string()))?;
        }
        match key() {
            Some(k) => {
                let data = encrypt(k, &json).ok_or_else(|| std::io::Error::other("encryption failed"))?;
                write_private(&enc_path(), &data)?;
                let _ = std::fs::remove_file(session_path());
                Ok(())
            }
            None => write_private(&session_path(), &json),
        }
    })();
    if let Err(e) = res {
        tracing::warn!("cannot save session: {e}");
    }
}

fn write_private(path: &std::path::Path, data: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(&tmp)?;
    f.write_all(data)?;
    f.flush()?;
    std::fs::rename(&tmp, path)
}

/// Session encore en clair alors qu'une clé du trousseau est disponible : chiffrée tout de suite.
pub fn migrate_plain_session(jar: &Jar) {
    if key().is_some() && session_path().exists() {
        save_jar(jar);
        tracing::info!("existing session moved to encrypted storage");
    }
}

pub fn clear_jar(jar: &Jar) {
    if let Ok(mut s) = jar.lock() {
        s.clear();
    }
    let _ = std::fs::remove_file(session_path());
    let _ = std::fs::remove_file(enc_path());
}

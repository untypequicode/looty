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
}

fn yes() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Config { server: None, close_to_tray: true }
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

pub fn load_jar() -> Jar {
    let store = std::fs::File::open(session_path())
        .ok()
        .and_then(|f| cookie_store::serde::json::load(std::io::BufReader::new(f)).ok())
        .unwrap_or_default();
    Arc::new(reqwest_cookie_store::CookieStoreMutex::new(store))
}

/// Écrit les cookies persistants (fichier lisible par l'utilisateur seul).
pub fn save_jar(jar: &Jar) {
    let path = session_path();
    let tmp = path.with_extension("json.tmp");
    let res = (|| -> std::io::Result<()> {
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&tmp)?;
        let store = jar.lock().map_err(|_| std::io::Error::other("cookie store poisoned"))?;
        cookie_store::serde::json::save_incl_expired_and_nonpersistent(&store, &mut f).map_err(|e| std::io::Error::other(e.to_string()))?;
        f.flush()?;
        std::fs::rename(&tmp, &path)
    })();
    if let Err(e) = res {
        tracing::warn!("cannot save session: {e}");
    }
}

pub fn clear_jar(jar: &Jar) {
    if let Ok(mut s) = jar.lock() {
        s.clear();
    }
    let _ = std::fs::remove_file(session_path());
}

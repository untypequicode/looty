//! Chiffrement des secrets stockés en base (mots de passe CalDAV) en AES-256-GCM.
//!
//! Clé : variable `LOOTY_SECRET_KEY` (dérivée par SHA-256), sinon une clé aléatoire
//! générée une fois dans `uploads/.secret.key` (volume persistant, jamais servi).

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

static KEY: OnceLock<[u8; 32]> = OnceLock::new();

fn key() -> &'static [u8; 32] {
    KEY.get_or_init(|| {
        if let Ok(k) = std::env::var("LOOTY_SECRET_KEY") {
            if !k.trim().is_empty() {
                return Sha256::digest(k.trim().as_bytes()).into();
            }
        }
        let path = std::path::Path::new(crate::upload::UPLOAD_DIR).join(".secret.key");
        if let Ok(bytes) = std::fs::read(&path) {
            if bytes.len() == 32 {
                let mut k = [0u8; 32];
                k.copy_from_slice(&bytes);
                return k;
            }
        }
        // Générateur cryptographique du système (rand::rng).
        let k: [u8; 32] = rand::random();
        let _ = std::fs::create_dir_all(crate::upload::UPLOAD_DIR);
        if std::fs::write(&path, k).is_ok() {
            tracing::warn!("LOOTY_SECRET_KEY not set: generated an encryption key in {}. Keep this file (or set LOOTY_SECRET_KEY) or stored calendar passwords become unreadable.", path.display());
        }
        k
    })
}

/// Chiffre une chaîne : renvoie `base64(nonce || ciphertext)`.
pub fn encrypt(plain: &str) -> String {
    let cipher = Aes256Gcm::new_from_slice(key()).expect("32-byte key");
    let nonce: [u8; 12] = rand::random();
    let nonce = Nonce::from(nonce);
    let ct = cipher.encrypt(&nonce, plain.as_bytes()).expect("encryption failure");
    let mut out = nonce.to_vec();
    out.extend_from_slice(&ct);
    B64.encode(out)
}

pub fn decrypt(enc: &str) -> Option<String> {
    let raw = B64.decode(enc).ok()?;
    if raw.len() < 13 {
        return None;
    }
    let (nonce, ct) = raw.split_at(12);
    let cipher = Aes256Gcm::new_from_slice(key()).expect("32-byte key");
    let nonce: [u8; 12] = nonce.try_into().ok()?;
    let pt = cipher.decrypt(&Nonce::from(nonce), ct).ok()?;
    String::from_utf8(pt).ok()
}

#[cfg(test)]
mod tests {
    #[test]
    fn roundtrip() {
        std::env::set_var("LOOTY_SECRET_KEY", "test-key");
        let e = super::encrypt("s3cret ✓");
        assert_ne!(e, "s3cret ✓");
        assert_eq!(super::decrypt(&e).as_deref(), Some("s3cret ✓"));
        assert!(super::decrypt("garbage").is_none());
    }
}

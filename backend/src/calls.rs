//! Appels vocaux (ADR 0019) : le serveur ne voit jamais le média, il n'autorise que la signalisation.
//! - `/api/calls/{page|project|workspace}/{id}/access` : appelé par le serveur de collaboration (comme
//!   `/api/pages/{id}/access`) à la connexion à une salle et à chaque revalidation. Mêmes droits que
//!   le contenu : lire la page, le projet ou l'espace suffit pour entendre et parler.
//! - `/api/calls/config` : serveurs ICE et limite de participants pour le navigateur.

use axum::{extract::{Path, State}, routing::get, Json, Router};
use sqlx::PgPool;
use base64::Engine;
use serde_json::{json, Value};
use sha1::{Digest, Sha1};
use uuid::Uuid;

use crate::access::{self, AuthUser, PageAccess};
use crate::error::{ApiError, ApiResult};
use crate::projects::Need;
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new().route("/config", get(config)).route("/{kind}/{id}/access", get(room_access))
}

#[derive(Debug, PartialEq, Eq)]
enum Kind {
    Page,
    Project,
    Workspace,
}

fn parse_kind(s: &str) -> Option<Kind> {
    match s {
        "page" => Some(Kind::Page),
        "project" => Some(Kind::Project),
        "workspace" => Some(Kind::Workspace),
        _ => None,
    }
}

/// Participants maximum d'une salle (maillage : chacun envoie son audio aux autres).
pub fn max_participants() -> u32 {
    parse_max(std::env::var("LOOTY_CALL_MAX_PARTICIPANTS").ok().as_deref())
}

fn parse_max(v: Option<&str>) -> u32 {
    v.and_then(|s| s.trim().parse::<u32>().ok()).filter(|n| valid_max(*n as i32)).unwrap_or(6)
}

/// Plafond dur du réglage : au-delà de 12 le maillage (N-1 flux par personne) n'a plus de sens.
pub const HARD_MAX: i32 = 12;
pub fn valid_max(n: i32) -> bool {
    (2..=HARD_MAX).contains(&n)
}

/// Réglages d'instance appliqués côté serveur : appels actifs, participants maximum, caméra, partage d'écran.
pub struct Limits {
    pub enabled: bool,
    pub max_participants: u32,
    pub video: bool,
    pub screen_share: bool,
}

impl Limits {
    fn json(&self) -> Value {
        json!({ "enabled": self.enabled, "max_participants": self.max_participants, "video": self.video, "screen_share": self.screen_share })
    }
}

async fn limits(pool: &PgPool) -> ApiResult<Limits> {
    let s = crate::admin::call_settings(pool).await?;
    Ok(Limits {
        enabled: s.calls_enabled,
        max_participants: s.calls_max_participants.filter(|n| valid_max(*n)).map(|n| n as u32).unwrap_or_else(max_participants),
        video: s.calls_video,
        screen_share: s.calls_screen_share,
    })
}

/// HMAC-SHA1 (RFC 2104), le condensat des identifiants éphémères de coturn.
fn hmac_sha1(key: &[u8], msg: &[u8]) -> [u8; 20] {
    let mut k = [0u8; 64];
    if key.len() > 64 {
        k[..20].copy_from_slice(&Sha1::digest(key));
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut inner = Sha1::new();
    inner.update(k.map(|b| b ^ 0x36));
    inner.update(msg);
    let mut outer = Sha1::new();
    outer.update(k.map(|b| b ^ 0x5c));
    outer.update(inner.finalize());
    outer.finalize().into()
}

/// Durée de validité des identifiants TURN (`LOOTY_TURN_TTL`, secondes : 5 minutes à 7 jours, défaut 24 h).
fn turn_ttl(v: Option<&str>) -> u64 {
    v.and_then(|s| s.trim().parse::<u64>().ok()).filter(|n| (300..=604_800).contains(n)).unwrap_or(86_400)
}

/// Adresses `turn:` / `turns:` de `LOOTY_TURN_URLS` (séparées par des virgules).
fn turn_urls(v: Option<&str>) -> Vec<String> {
    v.map(|s| s.split(',').map(|u| u.trim().to_string()).filter(|u| u.starts_with("turn:") || u.starts_with("turns:")).collect()).unwrap_or_default()
}

/// Serveur TURN à identifiants éphémères (mode « REST API » de coturn, `use-auth-secret`) : nom = « expiration:personne »,
/// mot de passe = base64(HMAC-SHA1(secret, nom)). Rien n'est renvoyé sans adresses ni secret. Le secret ne quitte jamais le serveur.
fn turn_server(urls: Vec<String>, secret: &str, user: Uuid, now: u64, ttl: u64) -> Option<Value> {
    if urls.is_empty() || secret.is_empty() {
        return None;
    }
    let username = format!("{}:{}", now + ttl, user);
    let credential = base64::engine::general_purpose::STANDARD.encode(hmac_sha1(secret.as_bytes(), username.as_bytes()));
    Some(json!({ "urls": urls, "username": username, "credential": credential }))
}

/// `LOOTY_STUN_URLS` : adresses STUN séparées par des virgules (défaut : STUN public de Google).
/// Crochet TURN (#264) : les serveurs TURN s'ajouteront à cette liste, avec leurs identifiants.
fn ice_servers(stun: Option<&str>) -> Value {
    let urls: Vec<String> = stun
        .map(|s| s.split(',').map(|u| u.trim().to_string()).filter(|u| u.starts_with("stun:") || u.starts_with("stuns:")).collect())
        .filter(|v: &Vec<String>| !v.is_empty())
        .unwrap_or_else(|| vec!["stun:stun.l.google.com:19302".to_string()]);
    json!([{ "urls": urls }])
}

async fn config(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<Value>> {
    let lim = limits(&state.pool).await?;
    let mut servers = ice_servers(std::env::var("LOOTY_STUN_URLS").ok().as_deref());
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let secret = std::env::var("LOOTY_TURN_SECRET").unwrap_or_default();
    let ttl = turn_ttl(std::env::var("LOOTY_TURN_TTL").ok().as_deref());
    if let Some(turn) = turn_server(turn_urls(std::env::var("LOOTY_TURN_URLS").ok().as_deref()), secret.trim(), user.id, now, ttl) {
        servers.as_array_mut().expect("liste").push(turn);
    }
    Ok(Json(json!({ "ice_servers": servers, "max_participants": lim.max_participants, "enabled": lim.enabled, "video": lim.video, "screen_share": lim.screen_share })))
}

async fn room_access(State(state): State<AppState>, user: AuthUser, Path((kind, id)): Path<(String, Uuid)>) -> ApiResult<Json<Value>> {
    let kind = parse_kind(&kind).ok_or(ApiError::NotFound)?;
    let pool = &state.pool;
    // Appels désactivés par l'administrateur : salle refusée (la signalisation est coupée à la révalidation).
    let lim = limits(pool).await?;
    if !lim.enabled {
        return Err(ApiError::Forbidden);
    }
    match kind {
        Kind::Page => {
            let (_, _, a) = access::page_access(pool, &user, id).await?;
            if a == PageAccess::None {
                return Err(ApiError::NotFound);
            }
        }
        Kind::Project => {
            crate::projects::project_for(&state, &user, id, Need::Read).await?;
        }
        Kind::Workspace => {
            access::require_member_or_guest(pool, &user, id).await?;
        }
    }
    let name: String = sqlx::query_scalar("SELECT name FROM users WHERE id = $1").bind(user.id).fetch_one(pool).await?;
    Ok(Json(json!({ "user_id": user.id, "name": name, "can_write": false, "max_participants": lim.max_participants, "calls": lim.json() })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds() {
        assert_eq!(parse_kind("page"), Some(Kind::Page));
        assert_eq!(parse_kind("project"), Some(Kind::Project));
        assert_eq!(parse_kind("workspace"), Some(Kind::Workspace));
        assert_eq!(parse_kind("user"), None);
    }

    #[test]
    fn participant_limit() {
        assert_eq!(parse_max(None), 6);
        assert_eq!(parse_max(Some("4")), 4);
        assert_eq!(parse_max(Some("1")), 6);
        assert_eq!(parse_max(Some("500")), 6);
        assert_eq!(parse_max(Some("12")), 12);
        assert_eq!(parse_max(Some("13")), 6);
        assert!(valid_max(2) && valid_max(12) && !valid_max(1) && !valid_max(13));
        assert_eq!(parse_max(Some("abc")), 6);
    }

    #[test]
    fn hmac_sha1_matches_rfc_2202() {
        assert_eq!(hex::encode(hmac_sha1(b"Jefe", b"what do ya want for nothing?")), "effcdf6ae5eb2fa2d27416d5f184df9c259a7c79");
        assert_eq!(hex::encode(hmac_sha1(&[0xaa; 80], b"Test Using Larger Than Block-Size Key - Hash Key First")), "aa4ae5e15272d00e95705637ce8a3b55ed402112");
    }

    #[test]
    fn turn_credentials_are_ephemeral_and_signed() {
        let user = Uuid::nil();
        let s = turn_server(vec!["turn:t.example:3478".into()], "s3cret", user, 1_000, 3_600).unwrap();
        assert_eq!(s["username"], format!("4600:{user}"));
        let expected = base64::engine::general_purpose::STANDARD.encode(hmac_sha1(b"s3cret", s["username"].as_str().unwrap().as_bytes()));
        assert_eq!(s["credential"], expected);
        assert!(!s.to_string().contains("s3cret"));
        // Sans secret ou sans adresse : pas de TURN du tout.
        assert!(turn_server(vec![], "s3cret", user, 0, 1).is_none());
        assert!(turn_server(vec!["turn:t.example".into()], "", user, 0, 1).is_none());
    }

    #[test]
    fn turn_settings() {
        assert_eq!(turn_ttl(None), 86_400);
        assert_eq!(turn_ttl(Some("3600")), 3_600);
        assert_eq!(turn_ttl(Some("5")), 86_400);
        assert_eq!(turn_urls(Some("turn:a:3478?transport=udp, turns:b:5349 ,http://x,stun:c")), vec!["turn:a:3478?transport=udp", "turns:b:5349"]);
        assert!(turn_urls(None).is_empty());
    }

    #[test]
    fn ice_servers_default_and_override() {
        assert_eq!(ice_servers(None)[0]["urls"][0], "stun:stun.l.google.com:19302");
        assert_eq!(ice_servers(Some("stun:a.example:3478, stun:b.example"))[0]["urls"][1], "stun:b.example");
        // Une valeur qui n'est pas une adresse STUN est ignorée (pas de javascript:, http:…).
        assert_eq!(ice_servers(Some("http://x"))[0]["urls"][0], "stun:stun.l.google.com:19302");
    }
}

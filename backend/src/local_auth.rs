//! Comptes locaux (adresse e-mail et mot de passe), facultatifs : `LOOTY_LOCAL_ACCOUNTS=true` (ADR 0015).
//!
//! Désactivés par défaut : Looty privilégie OpenID Connect. Quand ils sont actifs :
//! - inscription fermée (sur invitation d'un administrateur) sauf `LOOTY_LOCAL_SIGNUP=open` ;
//! - le premier compte d'une instance vide devient administrateur, avec le code affiché dans les journaux ;
//! - mots de passe en argon2id, longueur minimale, liste hors ligne de mots de passe courants ;
//! - limitation des tentatives par compte et par adresse IP ;
//! - adresse vérifiée par e-mail, réinitialisation par e-mail (ou lien donné par un administrateur) ;
//! - double authentification TOTP, obligatoire pour les administrateurs (issue 173).

use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    routing::{delete, get, post, put},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha1::{Digest as _, Sha1};
use sqlx::PgPool;
use tower_sessions::Session;
use uuid::Uuid;

use crate::access::{hash_token, AuthUser, SESSION_USER_KEY};
use crate::admin::require_super_admin;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

const PENDING_2FA: &str = "pending_2fa";
const PENDING_ENROL: &str = "pending_enrol";
const TOTP_SETUP: &str = "totp_setup_secret";
/// Délai pour saisir le code après le mot de passe, ou pour terminer l'inscription du second facteur.
const PENDING_SECS: i64 = 600;
const MIN_PASSWORD: usize = 12;
const MAX_PASSWORD: usize = 256;
const ACCOUNT_FAILURES: i64 = 5;
const IP_FAILURES: i64 = 30;
const WINDOW_MINUTES: i32 = 15;
const RECOVERY_CODES: usize = 10;
/// Compte technique des auteurs supprimés (ADR 0013) : jamais un « vrai » compte.
const DELETED_USER: Uuid = Uuid::nil();

pub fn env_enabled() -> bool {
    !crate::demo_mode::enabled() && std::env::var("LOOTY_LOCAL_ACCOUNTS").is_ok_and(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
}

pub fn env_signup_open() -> bool {
    std::env::var("LOOTY_LOCAL_SIGNUP").is_ok_and(|v| v.trim().eq_ignore_ascii_case("open"))
}

/// Code à saisir pour créer le premier compte : aléatoire à chaque démarrage, lisible dans les journaux du serveur.
pub fn new_setup_code() -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let raw: [u8; 8] = rand::random();
    let c: String = raw.iter().map(|b| ALPHABET[*b as usize % ALPHABET.len()] as char).collect();
    format!("{}-{}", &c[..4], &c[4..])
}

/// Aucun vrai compte sur l'instance (les personnages de la démo et le compte technique ne comptent pas).
pub async fn no_real_users(pool: &PgPool) -> ApiResult<bool> {
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE email NOT LIKE $1 AND id <> $2")
        .bind(crate::demo::EMAIL_PATTERN)
        .bind(DELETED_USER)
        .fetch_one(pool)
        .await?;
    Ok(n == 0)
}

/// À lancer au démarrage : indique où lire le code du premier compte.
pub async fn announce_setup_code(state: &AppState) {
    if state.local_accounts && no_real_users(&state.pool).await.unwrap_or(false) {
        tracing::warn!("Comptes locaux : instance vide. Code d'installation (setup code) pour créer le premier compte administrateur : {}", state.setup_code);
    }
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/config", get(config))
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/totp", post(login_totp))
        .route("/totp/setup", post(totp_setup))
        .route("/totp/confirm", post(totp_confirm))
        .route("/totp/disable", post(totp_disable))
        .route("/recovery-codes", post(new_recovery_codes))
        .route("/security", get(security))
        .route("/password", post(change_password))
        .route("/forgot", post(forgot))
        .route("/reset", post(reset))
        .route("/verify", post(verify))
        .route("/invite/{token}", get(invite_info))
        .route("/accept-invite", post(accept_invite))
}

pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(admin_overview))
        .route("/settings", put(admin_settings))
        .route("/invite", post(admin_invite))
        .route("/invites/{token_hash}", delete(admin_cancel_invite))
        .route("/users/{id}/verify", post(admin_verify))
        .route("/users/{id}/reset-link", post(admin_reset_link))
        .route("/users/{id}/totp-reset", post(admin_totp_reset))
}

// ---------------------------------------------------------------------------
// Mots de passe
// ---------------------------------------------------------------------------

fn common_words() -> &'static std::collections::HashSet<&'static str> {
    static WORDS: std::sync::OnceLock<std::collections::HashSet<&'static str>> = std::sync::OnceLock::new();
    WORDS.get_or_init(|| include_str!("common_passwords.txt").lines().map(str::trim).filter(|l| !l.is_empty()).collect())
}

/// Règles de mot de passe : longueur, liste de mots de passe courants (hors ligne), pas l'adresse elle-même.
pub fn check_password(password: &str, email: &str) -> Result<(), &'static str> {
    let len = password.chars().count();
    if len < MIN_PASSWORD {
        return Err("password_too_short");
    }
    if password.len() > MAX_PASSWORD {
        return Err("password_too_long");
    }
    let lower = password.to_lowercase();
    let mut distinct: Vec<char> = lower.chars().collect();
    distinct.sort_unstable();
    distinct.dedup();
    // Le mot courant, avec ou sans chiffres et signes à la fin (« Password2024! »), ou répété.
    let base = lower.trim_end_matches(|c: char| c.is_ascii_digit() || "!?.#$*@_-".contains(c));
    let words = common_words();
    let repeated = |w: &str| !w.is_empty() && base.len() % w.len() == 0 && base == w.repeat(base.len() / w.len());
    let known = words.contains(lower.as_str())
        || words.contains(base)
        || words.iter().any(|w| w.len() >= 6 && repeated(w))
        || lower.chars().all(|c| c.is_ascii_digit())
        || distinct.len() <= 3;
    if known {
        return Err("password_common");
    }
    let local = email.split('@').next().unwrap_or("").to_lowercase();
    if lower == email.to_lowercase() || (local.len() >= 4 && base == local) {
        return Err("password_contains_email");
    }
    Ok(())
}

async fn hash_password(password: String) -> ApiResult<String> {
    tokio::task::spawn_blocking(move || {
        let salt: [u8; 16] = rand::random();
        Argon2::default().hash_password_with_salt(password.as_bytes(), &salt).map(|h| h.to_string()).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .map_err(ApiError::Internal)
}

/// Compare en temps à peu près constant : sans compte, un hachage factice occupe le même temps.
async fn verify_password(password: String, hash: Option<String>) -> bool {
    static DUMMY: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let real = hash.is_some();
    let ok = tokio::task::spawn_blocking(move || {
        let hash = hash.unwrap_or_else(|| DUMMY.get_or_init(|| Argon2::default().hash_password_with_salt(b"looty-dummy", &[7u8; 16]).map(|h| h.to_string()).unwrap_or_default()).clone());
        Argon2::default().verify_password(password.as_bytes(), hash.as_str()).is_ok()
    })
    .await
    .unwrap_or(false);
    ok && real
}

// ---------------------------------------------------------------------------
// TOTP (RFC 6238) : HMAC-SHA1, 6 chiffres, pas de 30 s, tolérance d'un pas
// ---------------------------------------------------------------------------

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

fn hotp(secret: &[u8], counter: u64, digits: u32) -> u32 {
    let h = hmac_sha1(secret, &counter.to_be_bytes());
    let o = (h[19] & 0xf) as usize;
    let n = u32::from_be_bytes([h[o] & 0x7f, h[o + 1], h[o + 2], h[o + 3]]);
    n % 10u32.pow(digits)
}

const B32: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

fn base32_encode(data: &[u8]) -> String {
    let mut out = String::new();
    let (mut buf, mut bits) = (0u32, 0);
    for b in data {
        buf = (buf << 8) | *b as u32;
        bits += 8;
        while bits >= 5 {
            out.push(B32[((buf >> (bits - 5)) & 31) as usize] as char);
            bits -= 5;
        }
    }
    if bits > 0 {
        out.push(B32[((buf << (5 - bits)) & 31) as usize] as char);
    }
    out
}

fn base32_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let (mut buf, mut bits) = (0u32, 0);
    for c in s.bytes().filter(|c| *c != b' ' && *c != b'=') {
        let v = B32.iter().position(|x| *x == c.to_ascii_uppercase())? as u32;
        buf = (buf << 5) | v;
        bits += 5;
        if bits >= 8 {
            out.push((buf >> (bits - 8)) as u8);
            bits -= 8;
        }
    }
    Some(out)
}

/// Pas de 30 s qui correspond au code (le pas courant ou un voisin), s'il y en a un.
fn totp_match(secret: &[u8], code: &str, now: i64) -> Option<i64> {
    let code = code.trim().replace(' ', "");
    if code.len() != 6 || !code.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let step = now / 30;
    (-1..=1).map(|d| step + d).find(|s| *s >= 0 && format!("{:06}", hotp(secret, *s as u64, 6)) == code)
}

fn new_totp_secret() -> String {
    let raw: [u8; 20] = rand::random();
    base32_encode(&raw)
}

fn normalize_recovery(code: &str) -> String {
    code.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_uppercase()
}

fn is_recovery_shape(code: &str) -> bool {
    normalize_recovery(code).len() == 10
}

fn new_recovery_code() -> String {
    let raw: [u8; 10] = rand::random();
    let c: String = raw.iter().map(|b| B32[*b as usize % 32] as char).collect();
    format!("{}-{}", &c[..5], &c[5..])
}

// ---------------------------------------------------------------------------
// Tentatives et jetons
// ---------------------------------------------------------------------------

fn client_ip(headers: &HeaderMap) -> String {
    // Dernière valeur : celle que le proxy de confiance a ajoutée (les précédentes viennent du client).
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.rsplit(',').next())
        .map(|v| v.trim().chars().take(64).collect::<String>())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "direct".into())
}

async fn failures(pool: &PgPool, scope: &str, minutes: i32) -> ApiResult<i64> {
    Ok(sqlx::query_scalar("SELECT count(*) FROM login_attempts WHERE scope = $1 AND at > NOW() - make_interval(mins => $2)")
        .bind(scope)
        .bind(minutes)
        .fetch_one(pool)
        .await?)
}

async fn record(pool: &PgPool, scopes: &[&str]) {
    let _ = sqlx::query("DELETE FROM login_attempts WHERE at < NOW() - interval '1 day'").execute(pool).await;
    for s in scopes {
        let _ = sqlx::query("INSERT INTO login_attempts (scope) VALUES ($1)").bind(s).execute(pool).await;
    }
}

/// Trop d'échecs récents pour ce compte ou cette adresse : refus temporaire (même réponse que le compte existe ou non).
async fn guard_attempts(pool: &PgPool, email: &str, ip: &str) -> ApiResult<()> {
    if failures(pool, &format!("acct:{email}"), WINDOW_MINUTES).await? >= ACCOUNT_FAILURES || failures(pool, &format!("ip:{ip}"), WINDOW_MINUTES).await? >= IP_FAILURES {
        return Err(ApiError::Conflict("too_many_attempts".into()));
    }
    Ok(())
}

fn new_token() -> String {
    let raw: [u8; 32] = rand::random();
    hex::encode(raw)
}

async fn make_token(pool: &PgPool, user: Option<Uuid>, kind: &str, email: &str, name: &str, make_admin: bool, hours: i32, by: Option<Uuid>) -> ApiResult<String> {
    let raw = new_token();
    sqlx::query(
        "INSERT INTO local_tokens (token_hash, user_id, kind, email, name, make_admin, created_by, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, NOW() + make_interval(hours => $8))",
    )
    .bind(hash_token(&raw))
    .bind(user)
    .bind(kind)
    .bind(email)
    .bind(name)
    .bind(make_admin)
    .bind(by)
    .bind(hours)
    .execute(pool)
    .await?;
    Ok(raw)
}

#[derive(sqlx::FromRow)]
struct TokenRow {
    user_id: Option<Uuid>,
    email: String,
    name: String,
    make_admin: bool,
}

/// Un jeton valable, à usage unique : il est supprimé en le lisant.
async fn take_token(pool: &PgPool, raw: &str, kind: &str) -> ApiResult<TokenRow> {
    sqlx::query_as("DELETE FROM local_tokens WHERE token_hash = $1 AND kind = $2 AND expires_at > NOW() RETURNING user_id, email, name, make_admin")
        .bind(hash_token(raw.trim()))
        .bind(kind)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::bad_request("invalid_link"))
}

async fn mail_link(state: &AppState, to: &str, name: &str, fr: bool, subject: (&str, &str), line: (&str, &str), url: &str) -> bool {
    let Some(c) = crate::mail::config(&state.pool).await else { return false };
    let (subject, line) = if fr { (subject.1, line.1) } else { (subject.0, line.0) };
    let (text, html) = crate::delivery::render_email(&state.base_url, fr, subject, line, None, url, None);
    crate::mail::send(&c, &crate::mail::Email { to: to.to_string(), to_name: name.to_string(), subject: subject.to_string(), text, html }).await.is_ok()
}

// ---------------------------------------------------------------------------
// Sessions
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct Pending {
    user: Uuid,
    at: i64,
}

async fn set_pending(session: &Session, key: &str, user: Uuid) -> ApiResult<()> {
    session.insert(key, Pending { user, at: Utc::now().timestamp() }).await.map_err(|e| ApiError::Internal(e.to_string()))
}

async fn get_pending(session: &Session, key: &str) -> Option<Uuid> {
    let p: Pending = session.get(key).await.ok().flatten()?;
    (Utc::now().timestamp() - p.at <= PENDING_SECS).then_some(p.user)
}

async fn finish_login(session: &Session, pool: &PgPool, user: Uuid) -> ApiResult<()> {
    let e = |e: tower_sessions::session::Error| ApiError::Internal(e.to_string());
    session.cycle_id().await.map_err(e)?;
    let _ = session.remove::<Pending>(PENDING_2FA).await;
    let _ = session.remove::<Pending>(PENDING_ENROL).await;
    let _ = session.remove::<String>(TOTP_SETUP).await;
    session.insert(SESSION_USER_KEY, user).await.map_err(e)?;
    let _ = sqlx::query("UPDATE users SET updated_at = NOW() WHERE id = $1").bind(user).execute(pool).await;
    Ok(())
}

/// Ferme les autres sessions d'une personne (changement ou réinitialisation du mot de passe).
/// Les sessions contiennent l'identifiant en clair : on le cherche dans les données.
async fn drop_other_sessions(pool: &PgPool, user: Uuid, keep: Option<String>) {
    let _ = sqlx::query("DELETE FROM tower_sessions.session WHERE position(convert_to($1, 'UTF8') in data) > 0 AND ($2::text IS NULL OR id <> $2)")
        .bind(user.to_string())
        .bind(keep)
        .execute(pool)
        .await;
}

/// Double authentification requise pour cette personne (administrateur, ou imposée à l'instance).
async fn totp_required(pool: &PgPool, is_admin: bool) -> ApiResult<bool> {
    if is_admin {
        return Ok(true);
    }
    Ok(sqlx::query_scalar("SELECT require_totp FROM global_settings WHERE id = 1").fetch_optional(pool).await?.unwrap_or(false))
}

fn ensure_enabled(state: &AppState) -> ApiResult<()> {
    if state.local_accounts { Ok(()) } else { Err(ApiError::NotFound) }
}

// ---------------------------------------------------------------------------
// Configuration publique, inscription, connexion
// ---------------------------------------------------------------------------

async fn config(State(state): State<AppState>) -> ApiResult<Json<serde_json::Value>> {
    if !state.local_accounts {
        return Ok(Json(serde_json::json!({ "enabled": false })));
    }
    let first = no_real_users(&state.pool).await?;
    Ok(Json(serde_json::json!({
        "enabled": true,
        "signup": if state.local_signup_open { "open" } else { "closed" },
        "first_account": first,
    })))
}

#[derive(Deserialize)]
struct RegisterReq {
    email: String,
    name: String,
    password: String,
    locale: Option<String>,
    /// Code des journaux du serveur, exigé pour le tout premier compte.
    setup_code: Option<String>,
}

fn clean_email(raw: &str) -> ApiResult<String> {
    let e = raw.trim().to_lowercase();
    if e.len() > 255 || e.parse::<lettre::Address>().is_err() {
        return Err(ApiError::bad_request("invalid_email"));
    }
    Ok(e)
}

fn clean_name(raw: &str, email: &str) -> ApiResult<String> {
    let n = raw.trim();
    let n = if n.is_empty() { email.split('@').next().unwrap_or("") } else { n };
    if n.is_empty() || n.chars().count() > 120 {
        return Err(ApiError::bad_request("invalid_name"));
    }
    Ok(n.to_string())
}

fn same_code(a: &str, b: &str) -> bool {
    let (a, b) = (normalize_recovery(a), normalize_recovery(b));
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Locale et fuseau par défaut de l'instance pour un compte qui n'a rien choisi.
async fn instance_defaults(pool: &PgPool) -> (Option<String>, Option<String>) {
    sqlx::query_as("SELECT default_locale, default_timezone FROM global_settings WHERE id = 1").fetch_optional(pool).await.ok().flatten().unwrap_or((None, None))
}

pub(crate) async fn insert_user(pool: &PgPool, email: &str, name: &str, hash: &str, admin: bool, verified: bool, locale: Option<&str>) -> ApiResult<Uuid> {
    let (dl, tz) = instance_defaults(pool).await;
    let locale = locale.filter(|l| matches!(*l, "fr" | "en")).map(str::to_string).or(dl);
    Ok(sqlx::query_scalar(
        "INSERT INTO users (email, name, is_super_admin, password_hash, email_verified_at, locale, timezone)
         VALUES ($1, $2, $3, $4, CASE WHEN $5 THEN NOW() END, $6, $7) RETURNING id",
    )
    .bind(email)
    .bind(name)
    .bind(admin)
    .bind(hash)
    .bind(verified)
    .bind(locale)
    .bind(tz)
    .fetch_one(pool)
    .await?)
}

async fn register(State(state): State<AppState>, headers: HeaderMap, session: Session, Json(p): Json<RegisterReq>) -> ApiResult<Json<serde_json::Value>> {
    ensure_enabled(&state)?;
    let ip = client_ip(&headers);
    let email = clean_email(&p.email)?;
    let name = clean_name(&p.name, &email)?;
    check_password(&p.password, &email).map_err(ApiError::bad_request)?;
    // Limite les inscriptions (et le calcul des hachages) par adresse IP.
    if failures(&state.pool, &format!("signup:{ip}"), 60).await? >= 20 {
        return Err(ApiError::Conflict("too_many_attempts".into()));
    }
    record(&state.pool, &[&format!("signup:{ip}")]).await;

    // Le premier compte : seul le code des journaux l'autorise, et il est créé sous verrou.
    let mut tx = state.pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(7316001)").execute(&mut *tx).await?;
    let first = no_real_users(&state.pool).await?;
    if first {
        if guard_attempts(&state.pool, "setup", &ip).await.is_err() {
            return Err(ApiError::Conflict("too_many_attempts".into()));
        }
        if !p.setup_code.as_deref().is_some_and(|c| same_code(c, &state.setup_code)) {
            record(&state.pool, &["acct:setup", &format!("ip:{ip}")]).await;
            return Err(ApiError::bad_request("invalid_setup_code"));
        }
    } else if !state.local_signup_open {
        return Err(ApiError::bad_request("signups_closed"));
    }
    let hash = hash_password(p.password).await?;
    if !first {
        let taken: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE lower(email) = $1)").bind(&email).fetch_one(&mut *tx).await?;
        if taken {
            // Même réponse qu'une inscription réussie : on ne révèle pas qui a un compte.
            return Ok(Json(serde_json::json!({ "status": "verify_email" })));
        }
    }
    let (dl, tz) = instance_defaults(&state.pool).await;
    let locale = p.locale.as_deref().filter(|l| matches!(*l, "fr" | "en")).map(str::to_string).or(dl);
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO users (email, name, is_super_admin, password_hash, email_verified_at, locale, timezone)
         VALUES ($1, $2, $3, $4, CASE WHEN $3 THEN NOW() END, $5, $6) RETURNING id",
    )
    .bind(&email)
    .bind(&name)
    .bind(first)
    .bind(&hash)
    .bind(locale.clone())
    .bind(tz)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;

    if first {
        // Un administrateur doit configurer la double authentification avant d'entrer.
        set_pending(&session, PENDING_ENROL, id).await?;
        return Ok(Json(serde_json::json!({ "status": "totp_enrol" })));
    }
    let raw = make_token(&state.pool, Some(id), "verify", &email, &name, false, 48, None).await?;
    let url = format!("{}/login?verify={raw}", state.base_url);
    let fr = locale.as_deref() == Some("fr");
    let sent = mail_link(&state, &email, &name, fr, ("Confirm your email address", "Confirmez votre adresse e-mail"), ("Confirm your email address to finish creating your account.", "Confirmez votre adresse e-mail pour terminer la création de votre compte."), &url).await;
    Ok(Json(serde_json::json!({ "status": if sent { "verify_email" } else { "pending_admin" } })))
}

#[derive(sqlx::FromRow)]
struct LoginRow {
    id: Uuid,
    email: String,
    password_hash: Option<String>,
    is_suspended: bool,
    is_super_admin: bool,
    email_verified_at: Option<DateTime<Utc>>,
    totp_enabled_at: Option<DateTime<Utc>>,
}

#[derive(Deserialize)]
struct LoginReq {
    email: String,
    password: String,
}

async fn login(State(state): State<AppState>, headers: HeaderMap, session: Session, Json(p): Json<LoginReq>) -> ApiResult<Json<serde_json::Value>> {
    ensure_enabled(&state)?;
    let ip = client_ip(&headers);
    let email = p.email.trim().to_lowercase();
    guard_attempts(&state.pool, &email, &ip).await?;
    let row: Option<LoginRow> = sqlx::query_as("SELECT id, email, password_hash, is_suspended, is_super_admin, email_verified_at, totp_enabled_at FROM users WHERE lower(email) = $1 AND id <> $2")
        .bind(&email)
        .bind(DELETED_USER)
        .fetch_optional(&state.pool)
        .await?;
    let ok = verify_password(p.password, row.as_ref().and_then(|r| r.password_hash.clone())).await;
    let Some(user) = row.filter(|_| ok) else {
        record(&state.pool, &[&format!("acct:{email}"), &format!("ip:{ip}")]).await;
        return Err(ApiError::Unauthorized);
    };
    // Le mot de passe est juste : on peut dire pourquoi l'entrée est refusée.
    if user.is_suspended {
        return Err(ApiError::bad_request("suspended"));
    }
    if user.email_verified_at.is_none() {
        return Err(ApiError::bad_request("email_not_verified"));
    }
    if user.totp_enabled_at.is_some() {
        set_pending(&session, PENDING_2FA, user.id).await?;
        return Ok(Json(serde_json::json!({ "status": "totp" })));
    }
    if totp_required(&state.pool, user.is_super_admin).await? {
        set_pending(&session, PENDING_ENROL, user.id).await?;
        return Ok(Json(serde_json::json!({ "status": "totp_enrol" })));
    }
    let _ = sqlx::query("DELETE FROM login_attempts WHERE scope = $1").bind(format!("acct:{}", user.email.to_lowercase())).execute(&state.pool).await;
    finish_login(&session, &state.pool, user.id).await?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

#[derive(Deserialize)]
struct CodeReq {
    code: String,
}

/// Vérifie un code TOTP (pas déjà utilisé) ou un code de secours (à usage unique).
async fn check_second_factor(pool: &PgPool, user: Uuid, code: &str) -> ApiResult<bool> {
    let secret_enc: Option<String> = sqlx::query_scalar("SELECT totp_secret_enc FROM users WHERE id = $1").bind(user).fetch_one(pool).await?;
    let Some(secret) = secret_enc.as_deref().and_then(crate::crypto::decrypt).and_then(|s| base32_decode(&s)) else {
        return Ok(false);
    };
    if let Some(step) = totp_match(&secret, code, Utc::now().timestamp()) {
        // Un code ne sert qu'une fois : le pas doit dépasser le dernier accepté.
        let done = sqlx::query("UPDATE users SET totp_last_step = $2 WHERE id = $1 AND (totp_last_step IS NULL OR totp_last_step < $2)").bind(user).bind(step).execute(pool).await?;
        return Ok(done.rows_affected() == 1);
    }
    if is_recovery_shape(code) {
        let used = sqlx::query("UPDATE user_recovery_codes SET used_at = NOW() WHERE user_id = $1 AND code_hash = $2 AND used_at IS NULL")
            .bind(user)
            .bind(hash_token(&normalize_recovery(code)))
            .execute(pool)
            .await?;
        return Ok(used.rows_affected() == 1);
    }
    Ok(false)
}

async fn login_totp(State(state): State<AppState>, headers: HeaderMap, session: Session, Json(p): Json<CodeReq>) -> ApiResult<Json<serde_json::Value>> {
    ensure_enabled(&state)?;
    let user = get_pending(&session, PENDING_2FA).await.ok_or(ApiError::Unauthorized)?;
    let (email, suspended): (String, bool) = sqlx::query_as("SELECT lower(email), is_suspended FROM users WHERE id = $1").bind(user).fetch_one(&state.pool).await?;
    let ip = client_ip(&headers);
    guard_attempts(&state.pool, &email, &ip).await?;
    if suspended || !check_second_factor(&state.pool, user, &p.code).await? {
        record(&state.pool, &[&format!("acct:{email}"), &format!("ip:{ip}")]).await;
        return Err(ApiError::bad_request("invalid_code"));
    }
    let _ = sqlx::query("DELETE FROM login_attempts WHERE scope = $1").bind(format!("acct:{email}")).execute(&state.pool).await;
    finish_login(&session, &state.pool, user).await?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

// ---------------------------------------------------------------------------
// Double authentification : inscription, retrait, codes de secours
// ---------------------------------------------------------------------------

/// La personne qui configure son second facteur : connectée, ou en attente d'inscription du second facteur.
async fn totp_subject(session: &Session, user: Option<AuthUser>) -> ApiResult<(Uuid, bool)> {
    if let Some(u) = user {
        return Ok((u.id, false));
    }
    get_pending(session, PENDING_ENROL).await.map(|id| (id, true)).ok_or(ApiError::Unauthorized)
}

async fn local_user(pool: &PgPool, id: Uuid) -> ApiResult<(String, String, bool, bool)> {
    let r: (String, Option<String>, bool, bool) = sqlx::query_as("SELECT email, password_hash, is_super_admin, totp_enabled_at IS NOT NULL FROM users WHERE id = $1").bind(id).fetch_one(pool).await?;
    if r.1.is_none() {
        return Err(ApiError::bad_request("not_local_account"));
    }
    Ok((r.0, r.1.unwrap_or_default(), r.2, r.3))
}

async fn totp_setup(State(state): State<AppState>, session: Session, user: Option<AuthUser>) -> ApiResult<Json<serde_json::Value>> {
    ensure_enabled(&state)?;
    let (id, _) = totp_subject(&session, user).await?;
    let (email, _, _, enabled) = local_user(&state.pool, id).await?;
    if enabled {
        return Err(ApiError::Conflict("totp_already_enabled".into()));
    }
    let secret = new_totp_secret();
    session.insert(TOTP_SETUP, &secret).await.map_err(|e| ApiError::Internal(e.to_string()))?;
    let app: String = sqlx::query_scalar("SELECT app_name FROM global_settings WHERE id = 1").fetch_optional(&state.pool).await?.unwrap_or_else(|| "Looty".into());
    let enc = |s: &str| s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect::<String>();
    let url = format!("otpauth://totp/{}:{}?secret={secret}&issuer={}&algorithm=SHA1&digits=6&period=30", enc(&app), enc(&email), enc(&app));
    Ok(Json(serde_json::json!({ "secret": secret, "otpauth_url": url })))
}

async fn store_recovery_codes(pool: &PgPool, user: Uuid) -> ApiResult<Vec<String>> {
    sqlx::query("DELETE FROM user_recovery_codes WHERE user_id = $1").bind(user).execute(pool).await?;
    let codes: Vec<String> = (0..RECOVERY_CODES).map(|_| new_recovery_code()).collect();
    for c in &codes {
        sqlx::query("INSERT INTO user_recovery_codes (user_id, code_hash) VALUES ($1, $2)").bind(user).bind(hash_token(&normalize_recovery(c))).execute(pool).await?;
    }
    Ok(codes)
}

async fn totp_confirm(State(state): State<AppState>, headers: HeaderMap, session: Session, user: Option<AuthUser>, Json(p): Json<CodeReq>) -> ApiResult<Json<serde_json::Value>> {
    ensure_enabled(&state)?;
    let (id, pending) = totp_subject(&session, user).await?;
    let (email, _, _, enabled) = local_user(&state.pool, id).await?;
    if enabled {
        return Err(ApiError::Conflict("totp_already_enabled".into()));
    }
    let ip = client_ip(&headers);
    let email = email.to_lowercase();
    guard_attempts(&state.pool, &email, &ip).await?;
    let secret: String = session.get(TOTP_SETUP).await.ok().flatten().ok_or_else(|| ApiError::bad_request("totp_not_started"))?;
    let step = base32_decode(&secret).and_then(|s| totp_match(&s, &p.code, Utc::now().timestamp()));
    let Some(step) = step else {
        record(&state.pool, &[&format!("acct:{email}"), &format!("ip:{ip}")]).await;
        return Err(ApiError::bad_request("invalid_code"));
    };
    sqlx::query("UPDATE users SET totp_secret_enc = $2, totp_enabled_at = NOW(), totp_last_step = $3 WHERE id = $1")
        .bind(id)
        .bind(crate::crypto::encrypt(&secret))
        .bind(step)
        .execute(&state.pool)
        .await?;
    let codes = store_recovery_codes(&state.pool, id).await?;
    let _ = session.remove::<String>(TOTP_SETUP).await;
    if pending {
        finish_login(&session, &state.pool, id).await?;
    }
    Ok(Json(serde_json::json!({ "recovery_codes": codes, "logged_in": pending })))
}

#[derive(Deserialize)]
struct PasswordCodeReq {
    password: String,
    code: String,
}

/// Retrait du second facteur ou nouveaux codes de secours : mot de passe et code, même connecté.
async fn reauth(state: &AppState, headers: &HeaderMap, id: Uuid, password: String, code: &str) -> ApiResult<bool> {
    let (email, hash, is_admin, enabled) = local_user(&state.pool, id).await?;
    let email = email.to_lowercase();
    let ip = client_ip(headers);
    guard_attempts(&state.pool, &email, &ip).await?;
    if !verify_password(password, Some(hash)).await || (enabled && !check_second_factor(&state.pool, id, code).await?) {
        record(&state.pool, &[&format!("acct:{email}"), &format!("ip:{ip}")]).await;
        return Err(ApiError::bad_request("invalid_code"));
    }
    Ok(is_admin)
}

async fn totp_disable(State(state): State<AppState>, headers: HeaderMap, user: AuthUser, Json(p): Json<PasswordCodeReq>) -> ApiResult<Json<serde_json::Value>> {
    ensure_enabled(&state)?;
    let is_admin = reauth(&state, &headers, user.id, p.password, &p.code).await?;
    if totp_required(&state.pool, is_admin).await? {
        return Err(ApiError::Conflict("totp_required".into()));
    }
    sqlx::query("UPDATE users SET totp_secret_enc = NULL, totp_enabled_at = NULL, totp_last_step = NULL WHERE id = $1").bind(user.id).execute(&state.pool).await?;
    sqlx::query("DELETE FROM user_recovery_codes WHERE user_id = $1").bind(user.id).execute(&state.pool).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn new_recovery_codes(State(state): State<AppState>, headers: HeaderMap, user: AuthUser, Json(p): Json<PasswordCodeReq>) -> ApiResult<Json<serde_json::Value>> {
    ensure_enabled(&state)?;
    let (_, _, _, enabled) = local_user(&state.pool, user.id).await?;
    if !enabled {
        return Err(ApiError::bad_request("totp_not_enabled"));
    }
    reauth(&state, &headers, user.id, p.password, &p.code).await?;
    Ok(Json(serde_json::json!({ "recovery_codes": store_recovery_codes(&state.pool, user.id).await? })))
}

async fn security(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<serde_json::Value>> {
    if !state.local_accounts {
        return Ok(Json(serde_json::json!({ "enabled": false, "local": false })));
    }
    let (has_password, totp, is_admin, left): (bool, bool, bool, i64) = sqlx::query_as(
        "SELECT password_hash IS NOT NULL, totp_enabled_at IS NOT NULL, is_super_admin,
                (SELECT count(*) FROM user_recovery_codes c WHERE c.user_id = users.id AND c.used_at IS NULL)
         FROM users WHERE id = $1",
    )
    .bind(user.id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(serde_json::json!({
        "enabled": true, "local": has_password, "totp": totp, "recovery_left": left,
        "totp_required": has_password && totp_required(&state.pool, is_admin).await?,
    })))
}

#[derive(Deserialize)]
struct ChangePasswordReq {
    current: String,
    new: String,
}

async fn change_password(State(state): State<AppState>, headers: HeaderMap, session: Session, user: AuthUser, Json(p): Json<ChangePasswordReq>) -> ApiResult<Json<serde_json::Value>> {
    ensure_enabled(&state)?;
    let (email, hash, _, _) = local_user(&state.pool, user.id).await?;
    let (e, ip) = (email.to_lowercase(), client_ip(&headers));
    guard_attempts(&state.pool, &e, &ip).await?;
    if !verify_password(p.current, Some(hash)).await {
        record(&state.pool, &[&format!("acct:{e}"), &format!("ip:{ip}")]).await;
        return Err(ApiError::bad_request("invalid_credentials"));
    }
    check_password(&p.new, &email).map_err(ApiError::bad_request)?;
    let new_hash = hash_password(p.new).await?;
    sqlx::query("UPDATE users SET password_hash = $2 WHERE id = $1").bind(user.id).bind(new_hash).execute(&state.pool).await?;
    drop_other_sessions(&state.pool, user.id, session.id().map(|i| i.to_string())).await;
    Ok(Json(serde_json::json!({ "ok": true })))
}

// ---------------------------------------------------------------------------
// Adresse vérifiée, mot de passe oublié, invitation
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct EmailReq {
    email: String,
}

async fn forgot(State(state): State<AppState>, headers: HeaderMap, Json(p): Json<EmailReq>) -> ApiResult<Json<serde_json::Value>> {
    ensure_enabled(&state)?;
    let email = p.email.trim().to_lowercase();
    let ip = client_ip(&headers);
    // Même réponse dans tous les cas ; quelques demandes par heure, par adresse et par IP.
    let (a, b) = (format!("forgot:{email}"), format!("forgot-ip:{ip}"));
    if failures(&state.pool, &a, 60).await? >= 3 || failures(&state.pool, &b, 60).await? >= 20 {
        return Ok(Json(serde_json::json!({ "ok": true })));
    }
    record(&state.pool, &[&a, &b]).await;
    let row: Option<(Uuid, String, Option<String>)> = sqlx::query_as("SELECT id, name, locale FROM users WHERE lower(email) = $1 AND password_hash IS NOT NULL AND NOT is_suspended AND id <> $2")
        .bind(&email)
        .bind(DELETED_USER)
        .fetch_optional(&state.pool)
        .await?;
    if let Some((id, name, locale)) = row {
        if crate::mail::config(&state.pool).await.is_some() {
            let raw = make_token(&state.pool, Some(id), "reset", &email, &name, false, 1, None).await?;
            let url = format!("{}/login?reset={raw}", state.base_url);
            let st = state.clone();
            tokio::spawn(async move {
                let fr = locale.as_deref() == Some("fr");
                mail_link(&st, &email, &name, fr, ("Reset your password", "Réinitialiser votre mot de passe"), ("Use this link within one hour to choose a new password. If you did not ask for it, ignore this message.", "Utilisez ce lien dans l'heure pour choisir un nouveau mot de passe. Si vous n'avez rien demandé, ignorez ce message."), &url).await;
            });
        }
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Deserialize)]
struct ResetReq {
    token: String,
    password: String,
}

async fn reset(State(state): State<AppState>, headers: HeaderMap, Json(p): Json<ResetReq>) -> ApiResult<Json<serde_json::Value>> {
    ensure_enabled(&state)?;
    let ip = client_ip(&headers);
    if failures(&state.pool, &format!("reset-ip:{ip}"), WINDOW_MINUTES).await? >= IP_FAILURES {
        return Err(ApiError::Conflict("too_many_attempts".into()));
    }
    // Vérifie le mot de passe avant de consommer le lien : une faute de frappe ne le brûle pas.
    let peek: Option<(Uuid, String)> = sqlx::query_as("SELECT t.user_id, t.email FROM local_tokens t WHERE t.token_hash = $1 AND t.kind = 'reset' AND t.expires_at > NOW() AND t.user_id IS NOT NULL")
        .bind(hash_token(p.token.trim()))
        .fetch_optional(&state.pool)
        .await?;
    let Some((_, email)) = peek else {
        record(&state.pool, &[&format!("reset-ip:{ip}")]).await;
        return Err(ApiError::bad_request("invalid_link"));
    };
    check_password(&p.password, &email).map_err(ApiError::bad_request)?;
    let t = take_token(&state.pool, &p.token, "reset").await?;
    let Some(id) = t.user_id else { return Err(ApiError::bad_request("invalid_link")) };
    let hash = hash_password(p.password).await?;
    sqlx::query("UPDATE users SET password_hash = $2, email_verified_at = COALESCE(email_verified_at, NOW()) WHERE id = $1 AND password_hash IS NOT NULL")
        .bind(id)
        .bind(hash)
        .execute(&state.pool)
        .await?;
    sqlx::query("DELETE FROM local_tokens WHERE user_id = $1 AND kind = 'reset'").bind(id).execute(&state.pool).await?;
    sqlx::query("DELETE FROM login_attempts WHERE scope = $1").bind(format!("acct:{}", t.email.to_lowercase())).execute(&state.pool).await?;
    drop_other_sessions(&state.pool, id, None).await;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Deserialize)]
struct TokenReq {
    token: String,
}

async fn verify(State(state): State<AppState>, Json(p): Json<TokenReq>) -> ApiResult<Json<serde_json::Value>> {
    ensure_enabled(&state)?;
    let t = take_token(&state.pool, &p.token, "verify").await?;
    sqlx::query("UPDATE users SET email_verified_at = COALESCE(email_verified_at, NOW()) WHERE id = $1").bind(t.user_id).execute(&state.pool).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn invite_info(State(state): State<AppState>, Path(token): Path<String>) -> ApiResult<Json<serde_json::Value>> {
    ensure_enabled(&state)?;
    let t: TokenRow = sqlx::query_as("SELECT user_id, email, name, make_admin FROM local_tokens WHERE token_hash = $1 AND kind = 'invite' AND expires_at > NOW()")
        .bind(hash_token(token.trim()))
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::bad_request("invalid_link"))?;
    Ok(Json(serde_json::json!({ "email": t.email, "name": t.name })))
}

#[derive(Deserialize)]
struct AcceptReq {
    token: String,
    name: Option<String>,
    password: String,
    locale: Option<String>,
}

async fn accept_invite(State(state): State<AppState>, session: Session, Json(p): Json<AcceptReq>) -> ApiResult<Json<serde_json::Value>> {
    ensure_enabled(&state)?;
    let peek: Option<(String,)> = sqlx::query_as("SELECT email FROM local_tokens WHERE token_hash = $1 AND kind = 'invite' AND expires_at > NOW()").bind(hash_token(p.token.trim())).fetch_optional(&state.pool).await?;
    let Some((email,)) = peek else { return Err(ApiError::bad_request("invalid_link")) };
    check_password(&p.password, &email).map_err(ApiError::bad_request)?;
    let t = take_token(&state.pool, &p.token, "invite").await?;
    let name = clean_name(p.name.as_deref().unwrap_or(&t.name), &t.email)?;
    let taken: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE lower(email) = $1)").bind(&t.email).fetch_one(&state.pool).await?;
    if taken {
        return Err(ApiError::Conflict("email_taken".into()));
    }
    let hash = hash_password(p.password).await?;
    // L'invitation a été remise à cette adresse par un administrateur : l'adresse compte comme vérifiée.
    let id = insert_user(&state.pool, &t.email, &name, &hash, t.make_admin, true, p.locale.as_deref()).await?;
    if totp_required(&state.pool, t.make_admin).await? {
        set_pending(&session, PENDING_ENROL, id).await?;
        return Ok(Json(serde_json::json!({ "status": "totp_enrol" })));
    }
    finish_login(&session, &state.pool, id).await?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

// ---------------------------------------------------------------------------
// Administration
// ---------------------------------------------------------------------------

#[derive(Serialize, sqlx::FromRow)]
struct AccountDto {
    id: Uuid,
    email: String,
    name: String,
    is_super_admin: bool,
    is_suspended: bool,
    verified: bool,
    totp: bool,
    created_at: DateTime<Utc>,
}

#[derive(Serialize, sqlx::FromRow)]
struct InviteDto {
    id: String,
    email: String,
    name: String,
    make_admin: bool,
    expires_at: DateTime<Utc>,
}

async fn admin_overview(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<serde_json::Value>> {
    require_super_admin(&user)?;
    let accounts: Vec<AccountDto> = sqlx::query_as(
        "SELECT id, email, name, is_super_admin, is_suspended, email_verified_at IS NOT NULL AS verified, totp_enabled_at IS NOT NULL AS totp, created_at
         FROM users WHERE password_hash IS NOT NULL AND id <> $1 ORDER BY created_at",
    )
    .bind(DELETED_USER)
    .fetch_all(&state.pool)
    .await?;
    let invites: Vec<InviteDto> = sqlx::query_as("SELECT token_hash AS id, email, name, make_admin, expires_at FROM local_tokens WHERE kind = 'invite' AND expires_at > NOW() ORDER BY created_at")
        .fetch_all(&state.pool)
        .await?;
    let require_totp: bool = sqlx::query_scalar("SELECT require_totp FROM global_settings WHERE id = 1").fetch_optional(&state.pool).await?.unwrap_or(false);
    Ok(Json(serde_json::json!({
        "enabled": state.local_accounts,
        "signup": if state.local_signup_open { "open" } else { "closed" },
        "require_totp": require_totp,
        "smtp": crate::mail::config(&state.pool).await.is_some(),
        "accounts": accounts,
        "invites": invites,
    })))
}

#[derive(Deserialize)]
struct AdminSettingsReq {
    require_totp: bool,
}

async fn admin_settings(State(state): State<AppState>, user: AuthUser, Json(p): Json<AdminSettingsReq>) -> ApiResult<Json<serde_json::Value>> {
    require_super_admin(&user)?;
    ensure_enabled(&state)?;
    sqlx::query("INSERT INTO global_settings (id, require_totp) VALUES (1, $1) ON CONFLICT (id) DO UPDATE SET require_totp = $1").bind(p.require_totp).execute(&state.pool).await?;
    Ok(Json(serde_json::json!({ "require_totp": p.require_totp })))
}

#[derive(Deserialize)]
struct InviteReq {
    email: String,
    name: Option<String>,
    #[serde(default)]
    admin: bool,
}

async fn admin_invite(State(state): State<AppState>, user: AuthUser, Json(p): Json<InviteReq>) -> ApiResult<Json<serde_json::Value>> {
    require_super_admin(&user)?;
    ensure_enabled(&state)?;
    let email = clean_email(&p.email)?;
    let name = clean_name(p.name.as_deref().unwrap_or(""), &email)?;
    let taken: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE lower(email) = $1)").bind(&email).fetch_one(&state.pool).await?;
    if taken {
        return Err(ApiError::Conflict("email_taken".into()));
    }
    sqlx::query("DELETE FROM local_tokens WHERE kind = 'invite' AND email = $1").bind(&email).execute(&state.pool).await?;
    let raw = make_token(&state.pool, None, "invite", &email, &name, p.admin, 24 * 7, Some(user.id)).await?;
    let link = format!("{}/login?invite={raw}", state.base_url);
    let fr: bool = sqlx::query_scalar("SELECT COALESCE(locale = 'fr', FALSE) FROM users WHERE id = $1").bind(user.id).fetch_one(&state.pool).await?;
    let emailed = mail_link(&state, &email, &name, fr, ("You are invited to Looty", "Vous êtes invité sur Looty"), ("An administrator created an account for you. Open the link to choose your password (valid for 7 days).", "Un administrateur vous a créé un compte. Ouvrez le lien pour choisir votre mot de passe (valable 7 jours)."), &link).await;
    Ok(Json(serde_json::json!({ "link": link, "emailed": emailed })))
}

async fn admin_cancel_invite(State(state): State<AppState>, user: AuthUser, Path(id): Path<String>) -> ApiResult<Json<serde_json::Value>> {
    require_super_admin(&user)?;
    sqlx::query("DELETE FROM local_tokens WHERE token_hash = $1 AND kind = 'invite'").bind(id).execute(&state.pool).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn admin_verify(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<Json<serde_json::Value>> {
    require_super_admin(&user)?;
    sqlx::query("UPDATE users SET email_verified_at = COALESCE(email_verified_at, NOW()) WHERE id = $1 AND password_hash IS NOT NULL").bind(id).execute(&state.pool).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// Lien de réinitialisation remis par l'administrateur (quand l'instance n'envoie pas d'e-mails).
async fn admin_reset_link(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<Json<serde_json::Value>> {
    require_super_admin(&user)?;
    ensure_enabled(&state)?;
    let (email, name): (String, String) = sqlx::query_as("SELECT email, name FROM users WHERE id = $1 AND password_hash IS NOT NULL").bind(id).fetch_optional(&state.pool).await?.ok_or(ApiError::NotFound)?;
    let raw = make_token(&state.pool, Some(id), "reset", &email, &name, false, 24, Some(user.id)).await?;
    Ok(Json(serde_json::json!({ "link": format!("{}/login?reset={raw}", state.base_url) })))
}

/// Retire le second facteur d'un compte (téléphone perdu, codes de secours perdus).
async fn admin_totp_reset(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<Json<serde_json::Value>> {
    require_super_admin(&user)?;
    sqlx::query("UPDATE users SET totp_secret_enc = NULL, totp_enabled_at = NULL, totp_last_step = NULL WHERE id = $1").bind(id).execute(&state.pool).await?;
    sqlx::query("DELETE FROM user_recovery_codes WHERE user_id = $1").bind(id).execute(&state.pool).await?;
    drop_other_sessions(&state.pool, id, None).await;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn totp_matches_the_rfc_6238_vector() {
        // RFC 6238 annexe B, SHA-1, secret « 12345678901234567890 » : à 59 s, 94287082 (8 chiffres) donc 287082.
        let secret = b"12345678901234567890";
        assert_eq!(hotp(secret, 59 / 30, 8), 94287082);
        assert_eq!(hotp(secret, 59 / 30, 6), 287082);
        assert_eq!(hotp(secret, 1111111109 / 30, 8), 7081804);
        assert_eq!(totp_match(secret, "287082", 59), Some(1));
        // Tolérance d'un pas, pas plus.
        assert_eq!(totp_match(secret, "287082", 59 + 30), Some(1));
        assert_eq!(totp_match(secret, "287082", 59 + 90), None);
        assert_eq!(totp_match(secret, "28708", 59), None);
        assert_eq!(totp_match(secret, "abcdef", 59), None);
    }

    #[test]
    fn base32_round_trips_and_matches_the_rfc_vector() {
        assert_eq!(base32_encode(b"foobar"), "MZXW6YTBOI");
        assert_eq!(base32_decode("MZXW6YTBOI").unwrap(), b"foobar");
        assert_eq!(base32_decode("mzxw 6ytb oi").unwrap(), b"foobar");
        let s = new_totp_secret();
        assert_eq!(base32_decode(&s).unwrap().len(), 20);
        assert!(base32_decode("not base32!").is_none());
    }

    #[test]
    fn recovery_codes_ignore_case_and_dashes() {
        let c = new_recovery_code();
        assert!(is_recovery_shape(&c));
        assert!(same_code(&c, &c.to_lowercase().replace('-', " ")));
        assert!(!is_recovery_shape("123456"));
    }

    #[test]
    fn passwords_must_be_long_and_uncommon() {
        assert_eq!(check_password("short1!", "a@b.org"), Err("password_too_short"));
        assert_eq!(check_password("123456789012", "a@b.org"), Err("password_common"));
        assert_eq!(check_password("Password2024!!", "a@b.org"), Err("password_common"));
        assert_eq!(check_password(&"azerty".repeat(3), "a@b.org"), Err("password_common"));
        assert_eq!(check_password(&"aaaabbbb".repeat(2), "a@b.org"), Err("password_common"));
        assert_eq!(check_password("maya.dupont@example.org", "maya.dupont@example.org"), Err("password_contains_email"));
        assert_eq!(check_password(&"x1".repeat(200), "a@b.org"), Err("password_too_long"));
        assert!(check_password("violet-lantern-27-orbit", "a@b.org").is_ok());
    }

    #[tokio::test]
    async fn argon2id_hashes_verify_and_never_repeat() {
        let a = hash_password("violet-lantern-27-orbit".into()).await.unwrap();
        let b = hash_password("violet-lantern-27-orbit".into()).await.unwrap();
        assert!(a.starts_with("$argon2id$"));
        assert_ne!(a, b);
        assert!(verify_password("violet-lantern-27-orbit".into(), Some(a.clone())).await);
        assert!(!verify_password("violet-lantern-27-orbiT".into(), Some(a)).await);
        assert!(!verify_password("anything".into(), None).await);
    }

    #[test]
    fn the_client_address_is_the_one_the_proxy_appended() {
        let mut h = HeaderMap::new();
        assert_eq!(client_ip(&h), "direct");
        h.insert("x-forwarded-for", "6.6.6.6, 203.0.113.9".parse().unwrap());
        assert_eq!(client_ip(&h), "203.0.113.9");
    }

    // -----------------------------------------------------------------------
    // Parcours complets sur le vrai routeur (cookie de session conservé)
    // -----------------------------------------------------------------------

    use axum::{body::Body, http::Request};
    use serde_json::{json, Value};
    use tower::ServiceExt;

    struct Client {
        app: axum::Router,
        pool: PgPool,
        state: AppState,
        cookie: String,
    }

    impl Client {
        async fn new(pool: &PgPool, local: bool, open: bool) -> Client {
            crate::session_store::PgSessionStore::new(pool.clone()).migrate().await.unwrap();
            let mut state = AppState::new(pool.clone(), "http://localhost".into());
            state.local_accounts = local;
            state.local_signup_open = open;
            Client { app: crate::app(state.clone()), pool: pool.clone(), state, cookie: String::new() }
        }
        /// Un autre navigateur : sans cookie.
        fn fresh(&self) -> Client {
            Client { app: self.app.clone(), pool: self.pool.clone(), state: self.state.clone(), cookie: String::new() }
        }
        async fn call(&mut self, method: &str, path: &str, body: Option<Value>) -> (u16, Value) {
            let mut req = Request::builder().method(method).uri(path).header("content-type", "application/json");
            if !self.cookie.is_empty() {
                req = req.header("cookie", self.cookie.clone());
            }
            let res = self.app.clone().oneshot(req.body(body.map(|b| Body::from(b.to_string())).unwrap_or_else(Body::empty)).unwrap()).await.unwrap();
            if let Some(c) = res.headers().get("set-cookie").and_then(|v| v.to_str().ok()) {
                self.cookie = c.split(';').next().unwrap_or("").to_string();
            }
            let status = res.status().as_u16();
            let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
            (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
        }
        async fn me(&mut self) -> Value {
            self.call("GET", "/api/auth/me", None).await.1
        }
        async fn user(&self, email: &str, password: &str, admin: bool) -> Uuid {
            let hash = hash_password(password.to_string()).await.unwrap();
            insert_user(&self.pool, email, "Someone", &hash, admin, true, None).await.unwrap()
        }
    }

    const PW: &str = "violet-lantern-27-orbit";

    fn code_for(secret: &str) -> String {
        format!("{:06}", hotp(&base32_decode(secret).unwrap(), (Utc::now().timestamp() / 30) as u64, 6))
    }

    fn code(v: &Value) -> &str {
        v["error"].as_str().unwrap_or("")
    }

    /// Enrôle un second facteur sur la session en cours et rend le secret.
    async fn enrol(c: &mut Client) -> (String, Vec<String>) {
        let (s, setup) = c.call("POST", "/api/auth/local/totp/setup", None).await;
        assert_eq!(s, 200, "{setup}");
        let secret = setup["secret"].as_str().unwrap().to_string();
        let (s, done) = c.call("POST", "/api/auth/local/totp/confirm", Some(json!({ "code": code_for(&secret) }))).await;
        assert_eq!(s, 200, "{done}");
        (secret, done["recovery_codes"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect())
    }

    #[sqlx::test]
    async fn local_accounts_are_off_by_default(pool: PgPool) {
        let mut c = Client::new(&pool, false, false).await;
        assert_eq!(c.call("GET", "/api/auth/local/config", None).await.1["enabled"], false);
        let (s, _) = c.call("POST", "/api/auth/local/login", Some(json!({ "email": "a@example.org", "password": PW }))).await;
        assert_eq!(s, 404);
        let (s, _) = c.call("POST", "/api/auth/local/register", Some(json!({ "email": "a@example.org", "name": "A", "password": PW }))).await;
        assert_eq!(s, 404);
    }

    #[sqlx::test]
    async fn the_first_account_needs_the_log_code_and_becomes_an_administrator_with_two_factor(pool: PgPool) {
        let mut c = Client::new(&pool, true, false).await;
        assert_eq!(c.call("GET", "/api/auth/local/config", None).await.1["first_account"], true);
        let body = |code: Option<&str>| json!({ "email": "Root@Example.org", "name": "Root", "password": PW, "setup_code": code });
        let (s, e) = c.call("POST", "/api/auth/local/register", Some(body(None))).await;
        assert_eq!((s, code(&e)), (400, "invalid_setup_code"));
        let (s, e) = c.call("POST", "/api/auth/local/register", Some(body(Some("AAAA-AAAA")))).await;
        assert_eq!((s, code(&e)), (400, "invalid_setup_code"));
        let good = c.state.setup_code.to_lowercase();
        let (s, r) = c.call("POST", "/api/auth/local/register", Some(body(Some(&good)))).await;
        assert_eq!((s, r["status"].as_str()), (200, Some("totp_enrol")), "{r}");
        // Un administrateur n'entre pas tant que le second facteur n'est pas configuré.
        assert_eq!(c.me().await["authenticated"], false);
        let (secret, recovery) = enrol(&mut c).await;
        assert_eq!(recovery.len(), 10);
        let me = c.me().await;
        assert_eq!((me["authenticated"].clone(), me["user"]["is_super_admin"].clone(), me["user"]["email"].clone()), (json!(true), json!(true), json!("root@example.org")));
        // L'instance n'est plus vide : plus de premier compte, inscription fermée.
        let mut other = c.fresh();
        assert_eq!(other.call("GET", "/api/auth/local/config", None).await.1["first_account"], false);
        let (s, e) = other.call("POST", "/api/auth/local/register", Some(json!({ "email": "b@example.org", "name": "B", "password": PW }))).await;
        assert_eq!((s, code(&e)), (400, "signups_closed"));
        // Connexion suivante : mot de passe puis code ; un code ne sert qu'une fois.
        let mut login = c.fresh();
        let (_, r) = login.call("POST", "/api/auth/local/login", Some(json!({ "email": "root@example.org", "password": PW }))).await;
        assert_eq!(r["status"], "totp");
        assert_eq!(login.me().await["authenticated"], false);
        let (s, _) = login.call("POST", "/api/auth/local/totp", Some(json!({ "code": "000000" }))).await;
        assert_eq!(s, 400);
        // Le pas courant a déjà servi à l'inscription : le même code est refusé, le suivant passe.
        let replay = code_for(&secret);
        let (s, e) = login.call("POST", "/api/auth/local/totp", Some(json!({ "code": replay }))).await;
        assert_eq!((s, code(&e)), (400, "invalid_code"));
        let next = format!("{:06}", hotp(&base32_decode(&secret).unwrap(), (Utc::now().timestamp() / 30) as u64 + 1, 6));
        let (s, r) = login.call("POST", "/api/auth/local/totp", Some(json!({ "code": next }))).await;
        assert_eq!((s, r["status"].as_str()), (200, Some("ok")), "{r}");
        assert_eq!(login.me().await["authenticated"], true);
        // Un code de secours ouvre une session une seule fois.
        let mut rec = c.fresh();
        rec.call("POST", "/api/auth/local/login", Some(json!({ "email": "root@example.org", "password": PW }))).await;
        let (s, _) = rec.call("POST", "/api/auth/local/totp", Some(json!({ "code": recovery[0].to_lowercase() }))).await;
        assert_eq!(s, 200);
        let mut again = c.fresh();
        again.call("POST", "/api/auth/local/login", Some(json!({ "email": "root@example.org", "password": PW }))).await;
        let (s, _) = again.call("POST", "/api/auth/local/totp", Some(json!({ "code": recovery[0] }))).await;
        assert_eq!(s, 400);
    }

    #[sqlx::test]
    async fn an_administrator_without_two_factor_is_held_at_enrolment(pool: PgPool) {
        let mut c = Client::new(&pool, true, false).await;
        c.user("admin@example.org", PW, true).await;
        let (_, r) = c.call("POST", "/api/auth/local/login", Some(json!({ "email": "admin@example.org", "password": PW }))).await;
        assert_eq!(r["status"], "totp_enrol");
        assert_eq!(c.me().await["authenticated"], false);
        // Les routes protégées restent fermées, l'inscription du second facteur est la seule porte.
        assert_eq!(c.call("GET", "/api/workspaces", None).await.0, 401);
        enrol(&mut c).await;
        assert_eq!(c.me().await["authenticated"], true);
        // Retirer le second facteur d'un administrateur est refusé.
        let (s, e) = c.call("POST", "/api/auth/local/totp/disable", Some(json!({ "password": PW, "code": "123456" }))).await;
        assert_eq!(s, 400, "{e}");
    }

    #[sqlx::test]
    async fn two_factor_is_optional_for_others_unless_the_instance_requires_it(pool: PgPool) {
        let mut c = Client::new(&pool, true, false).await;
        c.user("ana@example.org", PW, false).await;
        let (_, r) = c.call("POST", "/api/auth/local/login", Some(json!({ "email": "ana@example.org", "password": PW }))).await;
        assert_eq!(r["status"], "ok");
        assert_eq!(c.me().await["authenticated"], true);
        sqlx::query("UPDATE global_settings SET require_totp = TRUE WHERE id = 1").execute(&pool).await.unwrap();
        let mut d = c.fresh();
        let (_, r) = d.call("POST", "/api/auth/local/login", Some(json!({ "email": "ana@example.org", "password": PW }))).await;
        assert_eq!(r["status"], "totp_enrol");
        assert_eq!(d.me().await["authenticated"], false);
    }

    #[sqlx::test]
    async fn failed_attempts_lock_the_account_for_a_while_without_revealing_it(pool: PgPool) {
        let mut c = Client::new(&pool, true, false).await;
        c.user("ana@example.org", PW, false).await;
        for email in ["ana@example.org", "nobody@example.org"] {
            for _ in 0..5 {
                let (s, e) = c.call("POST", "/api/auth/local/login", Some(json!({ "email": email, "password": "wrong-password-1" }))).await;
                assert_eq!((s, code(&e)), (401, "unauthorized"));
            }
            // Même réponse pour un compte qui existe et pour un autre : verrouillé, même avec le bon mot de passe.
            let (s, e) = c.call("POST", "/api/auth/local/login", Some(json!({ "email": email, "password": PW }))).await;
            assert_eq!((s, code(&e)), (409, "too_many_attempts"));
        }
        // Les échecs expirent : on efface la fenêtre et la connexion revient.
        sqlx::query("UPDATE login_attempts SET at = NOW() - interval '2 hours'").execute(&pool).await.unwrap();
        let (s, r) = c.call("POST", "/api/auth/local/login", Some(json!({ "email": "ana@example.org", "password": PW }))).await;
        assert_eq!((s, r["status"].as_str()), (200, Some("ok")));
    }

    #[sqlx::test]
    async fn too_many_attempts_from_one_address_lock_it_whatever_the_account(pool: PgPool) {
        let c = Client::new(&pool, true, false).await;
        for i in 0..IP_FAILURES {
            sqlx::query("INSERT INTO login_attempts (scope) VALUES ('ip:203.0.113.7')").execute(&pool).await.unwrap();
            let _ = i;
        }
        let req = Request::builder().method("POST").uri("/api/auth/local/login").header("content-type", "application/json").header("x-forwarded-for", "203.0.113.7")
            .body(Body::from(json!({ "email": "fresh@example.org", "password": "x" }).to_string())).unwrap();
        let res = c.app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status().as_u16(), 409);
    }

    #[sqlx::test]
    async fn open_signup_needs_a_verified_address_and_an_existing_email_is_not_revealed(pool: PgPool) {
        let mut c = Client::new(&pool, true, true).await;
        let admin = c.user("admin@example.org", PW, true).await;
        let new = json!({ "email": "new@example.org", "name": "New", "password": PW });
        let (s, r) = c.call("POST", "/api/auth/local/register", Some(new.clone())).await;
        // Sans SMTP, un administrateur doit confirmer l'adresse.
        assert_eq!((s, r["status"].as_str()), (200, Some("pending_admin")), "{r}");
        let (s, e) = c.call("POST", "/api/auth/local/login", Some(json!({ "email": "new@example.org", "password": PW }))).await;
        assert_eq!((s, code(&e)), (400, "email_not_verified"));
        // Même réponse pour une adresse déjà prise.
        let (s, r) = c.call("POST", "/api/auth/local/register", Some(json!({ "email": "admin@example.org", "name": "X", "password": PW }))).await;
        assert_eq!((s, r["status"].as_str()), (200, Some("verify_email")));
        // Lien de vérification : à usage unique.
        let id: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE email = 'new@example.org'").fetch_one(&pool).await.unwrap();
        let raw = make_token(&pool, Some(id), "verify", "new@example.org", "New", false, 1, None).await.unwrap();
        assert_eq!(c.call("POST", "/api/auth/local/verify", Some(json!({ "token": raw }))).await.0, 200);
        assert_eq!(c.call("POST", "/api/auth/local/verify", Some(json!({ "token": raw }))).await.0, 400);
        let (_, r) = c.call("POST", "/api/auth/local/login", Some(json!({ "email": "new@example.org", "password": PW }))).await;
        assert_eq!(r["status"], "ok");
        let _ = admin;
    }

    #[sqlx::test]
    async fn a_weak_password_is_refused_at_signup(pool: PgPool) {
        let mut c = Client::new(&pool, true, true).await;
        c.user("admin@example.org", PW, true).await;
        for (pw, expected) in [("short", "password_too_short"), ("password1234", "password_common"), ("new@example.org", "password_contains_email")] {
            let (s, e) = c.call("POST", "/api/auth/local/register", Some(json!({ "email": "new@example.org", "name": "N", "password": pw }))).await;
            assert_eq!((s, code(&e)), (400, expected));
        }
    }

    #[sqlx::test]
    async fn a_password_reset_link_works_once_and_closes_old_sessions(pool: PgPool) {
        let mut c = Client::new(&pool, true, false).await;
        let ana = c.user("ana@example.org", PW, false).await;
        c.call("POST", "/api/auth/local/login", Some(json!({ "email": "ana@example.org", "password": PW }))).await;
        assert_eq!(c.me().await["authenticated"], true);
        // Sans SMTP, la demande répond pareil et n'écrit aucun lien ; l'administrateur en donne un.
        let mut anon = c.fresh();
        let (s, r) = anon.call("POST", "/api/auth/local/forgot", Some(json!({ "email": "ana@example.org" }))).await;
        assert_eq!((s, r["ok"].clone()), (200, json!(true)));
        let (s, _) = anon.call("POST", "/api/auth/local/forgot", Some(json!({ "email": "ghost@example.org" }))).await;
        assert_eq!(s, 200);
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM local_tokens").fetch_one(&pool).await.unwrap();
        assert_eq!(n, 0);
        let raw = make_token(&pool, Some(ana), "reset", "ana@example.org", "Ana", false, 1, None).await.unwrap();
        let (s, e) = anon.call("POST", "/api/auth/local/reset", Some(json!({ "token": raw, "password": "short" }))).await;
        assert_eq!((s, code(&e)), (400, "password_too_short"));
        let new_pw = "another-violet-lantern-9";
        let (s, _) = anon.call("POST", "/api/auth/local/reset", Some(json!({ "token": raw, "password": new_pw }))).await;
        assert_eq!(s, 200);
        let (s, _) = anon.call("POST", "/api/auth/local/reset", Some(json!({ "token": raw, "password": new_pw }))).await;
        assert_eq!(s, 400);
        assert_eq!(c.me().await["authenticated"], false, "the old session is closed");
        let (s, _) = anon.call("POST", "/api/auth/local/login", Some(json!({ "email": "ana@example.org", "password": PW }))).await;
        assert_eq!(s, 401);
        let (_, r) = anon.call("POST", "/api/auth/local/login", Some(json!({ "email": "ana@example.org", "password": new_pw }))).await;
        assert_eq!(r["status"], "ok");
    }

    #[sqlx::test]
    async fn an_administrator_invites_and_the_invited_person_picks_a_password(pool: PgPool) {
        let mut admin = Client::new(&pool, true, false).await;
        admin.user("admin@example.org", PW, true).await;
        admin.call("POST", "/api/auth/local/login", Some(json!({ "email": "admin@example.org", "password": PW }))).await;
        enrol(&mut admin).await;
        let (s, inv) = admin.call("POST", "/api/admin/local/invite", Some(json!({ "email": "Ben@Example.org", "name": "Ben" }))).await;
        assert_eq!(s, 200, "{inv}");
        assert_eq!(inv["emailed"], false);
        let token = inv["link"].as_str().unwrap().rsplit('=').next().unwrap().to_string();
        let mut ben = admin.fresh();
        assert_eq!(ben.call("GET", &format!("/api/auth/local/invite/{token}"), None).await.1["email"], "ben@example.org");
        let (s, r) = ben.call("POST", "/api/auth/local/accept-invite", Some(json!({ "token": token, "password": PW }))).await;
        assert_eq!((s, r["status"].as_str()), (200, Some("ok")), "{r}");
        let me = ben.me().await;
        assert_eq!((me["authenticated"].clone(), me["user"]["is_super_admin"].clone()), (json!(true), json!(false)));
        assert_eq!(ben.fresh().call("POST", "/api/auth/local/accept-invite", Some(json!({ "token": token, "password": PW }))).await.0, 400);
        // Un compte qui n'est pas administrateur ne gère rien.
        assert_eq!(ben.call("GET", "/api/admin/local", None).await.0, 403);
        // L'administrateur voit le compte et peut retirer son second facteur.
        let (_, overview) = admin.call("GET", "/api/admin/local", None).await;
        assert_eq!(overview["accounts"].as_array().unwrap().len(), 2);
        let id = overview["accounts"].as_array().unwrap().iter().find(|a| a["email"] == "ben@example.org").unwrap()["id"].as_str().unwrap().to_string();
        assert_eq!(admin.call("POST", &format!("/api/admin/local/users/{id}/totp-reset"), None).await.0, 200);
    }

    #[sqlx::test]
    async fn a_logged_in_person_can_add_and_remove_two_factor_and_change_the_password(pool: PgPool) {
        let mut c = Client::new(&pool, true, false).await;
        c.user("ana@example.org", PW, false).await;
        c.call("POST", "/api/auth/local/login", Some(json!({ "email": "ana@example.org", "password": PW }))).await;
        assert_eq!(c.call("GET", "/api/auth/local/security", None).await.1["totp"], false);
        let (secret, recovery) = enrol(&mut c).await;
        let sec = c.call("GET", "/api/auth/local/security", None).await.1;
        assert_eq!((sec["totp"].clone(), sec["recovery_left"].clone(), sec["totp_required"].clone()), (json!(true), json!(10), json!(false)));
        // Nouveaux codes de secours : l'ancienne série ne vaut plus.
        let next = format!("{:06}", hotp(&base32_decode(&secret).unwrap(), (Utc::now().timestamp() / 30) as u64 + 1, 6));
        let (s, r) = c.call("POST", "/api/auth/local/recovery-codes", Some(json!({ "password": PW, "code": next }))).await;
        assert_eq!(s, 200, "{r}");
        assert_eq!(r["recovery_codes"].as_array().unwrap().len(), 10);
        let (s, _) = c.call("POST", "/api/auth/local/recovery-codes", Some(json!({ "password": "wrong-password-1", "code": recovery[1] }))).await;
        assert_eq!(s, 400);
        // Retrait avec le mot de passe et un code de secours.
        let fresh_code = r["recovery_codes"][0].as_str().unwrap().to_string();
        let (s, _) = c.call("POST", "/api/auth/local/totp/disable", Some(json!({ "password": PW, "code": fresh_code }))).await;
        assert_eq!(s, 200);
        assert_eq!(c.call("GET", "/api/auth/local/security", None).await.1["totp"], false);
        // Changement de mot de passe.
        let (s, e) = c.call("POST", "/api/auth/local/password", Some(json!({ "current": "nope-nope-nope-1", "new": "another-violet-lantern-9" }))).await;
        assert_eq!((s, code(&e)), (400, "invalid_credentials"));
        let (s, _) = c.call("POST", "/api/auth/local/password", Some(json!({ "current": PW, "new": "another-violet-lantern-9" }))).await;
        assert_eq!(s, 200);
        assert_eq!(c.me().await["authenticated"], true, "this session stays");
    }

    #[sqlx::test]
    async fn tokens_never_reach_the_account_security_routes(pool: PgPool) {
        assert!(crate::token_guard::never_for_tokens("/api/auth/local/totp/disable"));
        assert!(crate::token_guard::never_for_tokens("/api/admin/local/users/x/reset-link"));
        assert!(crate::token_guard::never_for_tokens("/api/setup/complete"));
        let _ = pool;
    }
}

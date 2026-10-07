//! Comptes de messagerie (IMAP, + SMTP facultatif pour l'envoi) et cache de la boîte de
//! réception : consulter sa boîte (par ex. celle de l'université) et être notifié des
//! nouveaux messages. Dossiers, envoi, déplacement et corbeille : voir `mailclient`.
//!
//! - Comptes par utilisateur, mot de passe chiffré, serveur IMAP en TLS (993) ou
//!   STARTTLS (143), adresses privées refusées (sauf CALDAV_ALLOW_PRIVATE).
//! - Toutes les 2 minutes : nouveaux messages de la boîte de réception (en-têtes et
//!   aperçu en cache), état lu / non lu, notification « mail ».
//! - Marquer lu / non lu, tout marquer comme lu, charger les messages plus anciens.
//! - Le contenu d'un message est lu à la demande et nettoyé (HTML sans scripts).

use async_imap::Session;
use async_native_tls::TlsStream;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use futures::TryStreamExt;
use mail_parser::{MessageParser, MimeHeaders};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::time::Duration;
use tokio::net::TcpStream;
use uuid::Uuid;

use crate::access::{require_member, AuthUser};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub(crate) type Imap = Session<TlsStream<TcpStream>>;

#[derive(sqlx::FromRow, Clone)]
pub(crate) struct Account {
    pub id: Uuid,
    pub user_id: Uuid,
    pub workspace_id: Uuid,
    pub imap_host: String,
    pub imap_port: i32,
    pub security: String,
    pub username: String,
    pub password_enc: String,
    pub notify: bool,
    pub uid_validity: Option<i64>,
    pub last_uid: i64,
    pub email: String,
    pub display_name: Option<String>,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<i32>,
    pub smtp_security: Option<String>,
    pub smtp_username: Option<String>,
    pub smtp_password_enc: Option<String>,
}

const ACCOUNT_COLS: &str = "id, user_id, workspace_id, imap_host, imap_port, security, username, password_enc, notify, uid_validity, last_uid,
    email, display_name, smtp_host, smtp_port, smtp_security, smtp_username, smtp_password_enc";

pub(crate) fn imap_err(e: impl std::fmt::Display) -> ApiError {
    ApiError::BadRequest(format!("mail_imap_error: {e}"))
}

/// Connexion et authentification (TLS direct ou STARTTLS).
async fn connect(host: &str, port: u16, security: &str, user: &str, pass: &str) -> ApiResult<Imap> {
    let addrs: Vec<IpAddr> = tokio::net::lookup_host((host, port)).await.map_err(|_| ApiError::bad_request("mail_unreachable"))?.map(|a| a.ip()).collect();
    if addrs.is_empty() {
        return Err(ApiError::bad_request("mail_unreachable"));
    }
    if !crate::caldav::allow_private() && addrs.iter().any(|ip| crate::caldav::is_private(*ip)) {
        return Err(ApiError::bad_request("mail_private_address"));
    }
    let work = async {
        let tcp = TcpStream::connect((host, port)).await.map_err(|_| ApiError::bad_request("mail_unreachable"))?;
        let tls = async_native_tls::TlsConnector::new();
        let client = if security == "starttls" {
            let mut plain = async_imap::Client::new(tcp);
            plain.read_response().await.ok_or_else(|| ApiError::bad_request("mail_unreachable"))?.map_err(imap_err)?;
            plain.run_command_and_check_ok("STARTTLS", None).await.map_err(|_| ApiError::bad_request("mail_starttls_failed"))?;
            let tcp = plain.into_inner();
            async_imap::Client::new(tls.connect(host, tcp).await.map_err(|_| ApiError::bad_request("mail_tls_failed"))?)
        } else {
            let stream = tls.connect(host, tcp).await.map_err(|_| ApiError::bad_request("mail_tls_failed"))?;
            let mut c = async_imap::Client::new(stream);
            c.read_response().await.ok_or_else(|| ApiError::bad_request("mail_unreachable"))?.map_err(imap_err)?;
            c
        };
        client.login(user, pass).await.map_err(|_| ApiError::bad_request("mail_auth_failed"))
    };
    tokio::time::timeout(Duration::from_secs(20), work).await.map_err(|_| ApiError::bad_request("mail_timeout"))?
}

pub(crate) async fn connect_account(a: &Account) -> ApiResult<Imap> {
    let pass = crate::crypto::decrypt(&a.password_enc).ok_or_else(|| ApiError::bad_request("mail_secret_unreadable"))?;
    connect(&a.imap_host, a.imap_port as u16, &a.security, &a.username, &pass).await
}

pub(crate) struct Parsed {
    pub from_name: String,
    pub from_addr: String,
    pub subject: String,
    pub snippet: String,
    pub sent_at: Option<DateTime<Utc>>,
    pub has_attachments: bool,
}

pub(crate) fn parse_summary(raw: &[u8]) -> Parsed {
    let msg = MessageParser::default().parse(raw);
    let (from_name, from_addr) = msg
        .as_ref()
        .and_then(|m| m.from())
        .and_then(|f| f.first())
        .map(|a| (a.name().unwrap_or("").to_string(), a.address().unwrap_or("").to_string()))
        .unwrap_or_default();
    let snippet = msg
        .as_ref()
        .and_then(|m| m.body_text(0))
        .map(|t| t.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(180).collect())
        .unwrap_or_default();
    Parsed {
        from_name,
        from_addr,
        subject: msg.as_ref().and_then(|m| m.subject()).unwrap_or("").chars().take(300).collect(),
        snippet,
        sent_at: msg.as_ref().and_then(|m| m.date()).and_then(|d| DateTime::from_timestamp(d.to_timestamp(), 0)),
        has_attachments: msg.as_ref().is_some_and(|m| m.attachment_count() > 0),
    }
}

/// Synchronise la boîte de réception ; renvoie les nouveaux messages (pour notifier).
pub(crate) async fn sync_account(state: &AppState, a: &Account) -> ApiResult<Vec<Parsed>> {
    let pool = &state.pool;
    let mut s = connect_account(a).await?;
    let mailbox = s.select("INBOX").await.map_err(imap_err)?;
    let validity = mailbox.uid_validity.map(|v| v as i64);
    // Boîte recréée côté serveur (UIDVALIDITY changé) : on repart de zéro.
    let reset = a.uid_validity.is_some() && a.uid_validity != validity;
    let first = a.last_uid == 0 || reset;
    if reset {
        sqlx::query("DELETE FROM mail_messages WHERE account_id = $1").bind(a.id).execute(pool).await?;
    }
    let mut new_msgs = Vec::new();
    let mut max_uid = if reset { 0 } else { a.last_uid };
    if mailbox.exists > 0 {
        // Première fois : les 50 derniers messages ; ensuite, ceux après le dernier vu.
        let fetches: Vec<_> = if first {
            let start = mailbox.exists.saturating_sub(49).max(1);
            s.fetch(format!("{start}:*"), "(UID FLAGS BODY.PEEK[]<0.32768>)").await.map_err(imap_err)?.try_collect().await.map_err(imap_err)?
        } else {
            s.uid_fetch(format!("{}:*", a.last_uid + 1), "(UID FLAGS BODY.PEEK[]<0.32768>)").await.map_err(imap_err)?.try_collect().await.map_err(imap_err)?
        };
        let floor = if first { 0 } else { max_uid };
        for (uid, p, seen) in store_fetches(pool, a.id, &fetches).await? {
            if uid <= floor {
                continue;
            }
            max_uid = max_uid.max(uid);
            if !first && !seen {
                new_msgs.push(p);
            }
        }
        // Messages non lus plus anciens que la fenêtre en cache : tous récupérés (sans
        // notification), pour que « Non lus » soit complet.
        let unseen = s.uid_search("UNSEEN").await.map_err(imap_err)?;
        let cached: Vec<i64> = sqlx::query_scalar("SELECT uid FROM mail_messages WHERE account_id = $1").bind(a.id).fetch_all(pool).await?;
        let mut missing: Vec<u32> = unseen.into_iter().filter(|u| !cached.contains(&(*u as i64))).collect();
        missing.sort_unstable();
        missing.reverse();
        missing.truncate(300);
        if !missing.is_empty() {
            let set = missing.iter().map(|u| u.to_string()).collect::<Vec<_>>().join(",");
            let older: Vec<_> = s.uid_fetch(set, "(UID FLAGS BODY.PEEK[]<0.32768>)").await.map_err(imap_err)?.try_collect().await.map_err(imap_err)?;
            store_fetches(pool, a.id, &older).await?;
        }
    }
    // État lu / non lu et messages supprimés, sur la fenêtre en cache.
    let min_uid: Option<i64> = sqlx::query_scalar("SELECT MIN(uid) FROM mail_messages WHERE account_id = $1").bind(a.id).fetch_one(pool).await?;
    if let Some(min_uid) = min_uid {
        let flags: Vec<_> = s.uid_fetch(format!("{min_uid}:*"), "(UID FLAGS)").await.map_err(imap_err)?.try_collect().await.map_err(imap_err)?;
        let mut present = Vec::new();
        for f in &flags {
            let Some(uid) = f.uid.map(|u| u as i64) else { continue };
            present.push(uid);
            let seen = f.flags().any(|fl| matches!(fl, async_imap::types::Flag::Seen));
            sqlx::query("UPDATE mail_messages SET seen = $3 WHERE account_id = $1 AND uid = $2 AND seen <> $3").bind(a.id).bind(uid).bind(seen).execute(pool).await?;
        }
        sqlx::query("DELETE FROM mail_messages WHERE account_id = $1 AND NOT (uid = ANY($2))").bind(a.id).bind(&present).execute(pool).await?;
    }
    sqlx::query("DELETE FROM mail_messages WHERE account_id = $1 AND uid NOT IN (SELECT uid FROM mail_messages WHERE account_id = $1 ORDER BY uid DESC LIMIT 3000)").bind(a.id).execute(pool).await?;
    sqlx::query("UPDATE mail_accounts SET uid_validity = $2, last_uid = $3, last_check_at = NOW(), last_error = NULL WHERE id = $1").bind(a.id).bind(validity).bind(max_uid).execute(pool).await?;
    let _ = s.logout().await;
    Ok(new_msgs)
}

/// Met en cache des messages récupérés (en-têtes, aperçu, état lu) ; renvoie leur résumé.
async fn store_fetches(pool: &sqlx::PgPool, account: Uuid, fetches: &[async_imap::types::Fetch]) -> ApiResult<Vec<(i64, Parsed, bool)>> {
    let mut out = Vec::new();
    for f in fetches {
        let Some(uid) = f.uid.map(|u| u as i64) else { continue };
        let seen = f.flags().any(|fl| matches!(fl, async_imap::types::Flag::Seen));
        let p = parse_summary(f.body().unwrap_or_default());
        sqlx::query(
            "INSERT INTO mail_messages (account_id, uid, from_name, from_addr, subject, snippet, sent_at, seen, has_attachments)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) ON CONFLICT (account_id, uid) DO NOTHING",
        )
        .bind(account)
        .bind(uid)
        .bind(&p.from_name)
        .bind(&p.from_addr)
        .bind(&p.subject)
        .bind(&p.snippet)
        .bind(p.sent_at)
        .bind(seen)
        .bind(p.has_attachments)
        .execute(pool)
        .await?;
        out.push((uid, p, seen));
    }
    Ok(out)
}

async fn sync_and_notify(state: &AppState, a: &Account) -> ApiResult<usize> {
    match sync_account(state, a).await {
        Ok(new_msgs) => {
            if a.notify {
                // Au-delà de 3 nouveaux messages : une seule notification récapitulative.
                if new_msgs.len() > 3 {
                    crate::notifications::notify(&state.pool, &state.hub, a.workspace_id, a.user_id, None, "mail", &format!("mail:{}", a.id),
                        serde_json::json!({ "count": new_msgs.len() })).await;
                } else {
                    for m in &new_msgs {
                        let from = if m.from_name.is_empty() { m.from_addr.clone() } else { m.from_name.clone() };
                        crate::notifications::notify(&state.pool, &state.hub, a.workspace_id, a.user_id, None, "mail", &format!("mail:{}", a.id),
                            serde_json::json!({ "title": m.subject, "from": from })).await;
                    }
                }
            }
            Ok(new_msgs.len())
        }
        Err(e) => {
            let msg = match &e {
                ApiError::BadRequest(m) => m.clone(),
                other => format!("{other:?}"),
            };
            sqlx::query("UPDATE mail_accounts SET last_check_at = NOW(), last_error = $2 WHERE id = $1").bind(a.id).bind(msg.chars().take(300).collect::<String>()).execute(&state.pool).await?;
            Err(e)
        }
    }
}

pub fn start_poll_loop(state: AppState) {
    tokio::spawn(async move {
        loop {
            let accounts: Vec<Account> = sqlx::query_as(&format!(
                "SELECT {ACCOUNT_COLS} FROM mail_accounts WHERE last_check_at IS NULL OR last_check_at < NOW() - interval '110 seconds' ORDER BY last_check_at NULLS FIRST LIMIT 50"
            ))
            .fetch_all(&state.pool)
            .await
            .unwrap_or_default();
            for a in accounts {
                if let Err(e) = tokio::time::timeout(Duration::from_secs(90), sync_and_notify(&state, &a)).await.unwrap_or(Err(ApiError::bad_request("mail_timeout"))) {
                    tracing::debug!("mail account {}: {e:?}", a.id);
                }
            }
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    });
}

// ---------------------------------------------------------------------------
// API
// ---------------------------------------------------------------------------

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/accounts", get(list_accounts).post(create_account))
        .route("/accounts/:id", axum::routing::put(update_account).delete(delete_account))
        .route("/accounts/:id/refresh", post(refresh))
        .route("/accounts/:id/messages", get(list_messages))
        .route("/accounts/:id/messages/:uid", get(read_message))
        .route("/accounts/:id/messages/:uid/seen", axum::routing::put(set_seen))
        .route("/accounts/:id/read-all", post(read_all))
}

#[derive(Serialize, sqlx::FromRow)]
struct AccountDto {
    id: Uuid,
    workspace_id: Uuid,
    label: String,
    email: String,
    imap_host: String,
    imap_port: i32,
    security: String,
    username: String,
    webmail_url: Option<String>,
    notify: bool,
    last_check_at: Option<DateTime<Utc>>,
    last_error: Option<String>,
    unread: i64,
    display_name: Option<String>,
    signature: Option<String>,
    smtp_host: Option<String>,
    smtp_port: Option<i32>,
    smtp_security: Option<String>,
    smtp_username: Option<String>,
    /// Envoi configuré (SMTP).
    can_send: bool,
}

const DTO: &str = "SELECT a.id, a.workspace_id, a.label, a.email, a.imap_host, a.imap_port, a.security, a.username, a.webmail_url, a.notify,
        a.last_check_at, a.last_error, a.display_name, a.signature, a.smtp_host, a.smtp_port, a.smtp_security, a.smtp_username,
        (a.smtp_host IS NOT NULL) AS can_send,
        (SELECT count(*) FROM mail_messages m WHERE m.account_id = a.id AND NOT m.seen) AS unread
     FROM mail_accounts a";

async fn list_accounts(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<Vec<AccountDto>>> {
    Ok(Json(sqlx::query_as(&format!("{DTO} WHERE a.user_id = $1 ORDER BY a.created_at")).bind(user.id).fetch_all(&state.pool).await?))
}

#[derive(Deserialize)]
struct CreateReq {
    workspace_id: Uuid,
    label: String,
    email: Option<String>,
    imap_host: String,
    imap_port: Option<u16>,
    security: Option<String>,
    username: String,
    password: String,
    webmail_url: Option<String>,
    notify: Option<bool>,
    display_name: Option<String>,
    signature: Option<String>,
    smtp: Option<crate::mailclient::SmtpReq>,
}

fn clean_webmail(u: Option<&str>) -> ApiResult<Option<String>> {
    match u.map(str::trim).filter(|u| !u.is_empty()) {
        Some(u) if (u.starts_with("https://") || u.starts_with("http://")) && u.len() < 1000 => Ok(Some(u.to_string())),
        Some(_) => Err(ApiError::bad_request("mail_invalid_webmail")),
        None => Ok(None),
    }
}

async fn create_account(State(state): State<AppState>, user: AuthUser, Json(p): Json<CreateReq>) -> ApiResult<Json<AccountDto>> {
    require_member(&state.pool, &user, p.workspace_id).await?;
    let host = p.imap_host.trim().to_lowercase();
    if host.is_empty() || host.len() > 255 || host.contains(['/', ' ', ':']) {
        return Err(ApiError::bad_request("mail_invalid_host"));
    }
    let security = p.security.as_deref().unwrap_or("tls");
    if !matches!(security, "tls" | "starttls") {
        return Err(ApiError::bad_request("invalid_value"));
    }
    let port = p.imap_port.unwrap_or(if security == "tls" { 993 } else { 143 });
    let label = p.label.trim();
    if label.is_empty() || label.chars().count() > 120 || p.username.trim().is_empty() {
        return Err(ApiError::bad_request("invalid_value"));
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM mail_accounts WHERE user_id = $1").bind(user.id).fetch_one(&state.pool).await?;
    if count >= 5 {
        return Err(ApiError::bad_request("mail_too_many"));
    }
    // Vérifie la connexion avant d'enregistrer (identifiants, TLS, boîte de réception).
    let mut s = connect(&host, port, security, p.username.trim(), &p.password).await?;
    s.select("INBOX").await.map_err(imap_err)?;
    let _ = s.logout().await;
    let smtp = match &p.smtp {
        Some(r) => Some(r.validate(p.username.trim(), &p.password, None).await?),
        None => None,
    };
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO mail_accounts (user_id, workspace_id, label, email, imap_host, imap_port, security, username, password_enc, webmail_url, notify,
            display_name, signature, smtp_host, smtp_port, smtp_security, smtp_username, smtp_password_enc)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18) RETURNING id",
    )
    .bind(user.id)
    .bind(p.workspace_id)
    .bind(label)
    .bind(p.email.as_deref().unwrap_or(p.username.trim()).trim())
    .bind(&host)
    .bind(port as i32)
    .bind(security)
    .bind(p.username.trim())
    .bind(crate::crypto::encrypt(&p.password))
    .bind(clean_webmail(p.webmail_url.as_deref())?)
    .bind(p.notify.unwrap_or(true))
    .bind(clean_text(p.display_name.as_deref(), 120))
    .bind(clean_text(p.signature.as_deref(), 4000))
    .bind(smtp.as_ref().map(|s| s.host.clone()))
    .bind(smtp.as_ref().map(|s| s.port as i32))
    .bind(smtp.as_ref().map(|s| s.security.clone()))
    .bind(smtp.as_ref().and_then(|s| s.username.clone()))
    .bind(smtp.as_ref().and_then(|s| s.password_enc.clone()))
    .fetch_one(&state.pool)
    .await?;
    let a: Account = sqlx::query_as(&format!("SELECT {ACCOUNT_COLS} FROM mail_accounts WHERE id = $1")).bind(id).fetch_one(&state.pool).await?;
    let _ = sync_and_notify(&state, &a).await;
    Ok(Json(sqlx::query_as(&format!("{DTO} WHERE a.id = $1")).bind(id).fetch_one(&state.pool).await?))
}

pub(crate) async fn own_account(state: &AppState, user: &AuthUser, id: Uuid) -> ApiResult<Account> {
    sqlx::query_as(&format!("SELECT {ACCOUNT_COLS} FROM mail_accounts WHERE id = $1 AND user_id = $2"))
        .bind(id)
        .bind(user.id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(ApiError::NotFound)
}

#[derive(Deserialize)]
struct UpdateReq {
    label: Option<String>,
    webmail_url: Option<String>,
    notify: Option<bool>,
    password: Option<String>,
    display_name: Option<String>,
    signature: Option<String>,
    smtp: Option<crate::mailclient::SmtpReq>,
    /// Supprime la configuration d'envoi.
    smtp_remove: Option<bool>,
}

/// Texte libre facultatif : vide → NULL.
fn clean_text(v: Option<&str>, max: usize) -> Option<String> {
    v.map(str::trim).filter(|v| !v.is_empty()).map(|v| v.chars().take(max).collect())
}

async fn update_account(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(p): Json<UpdateReq>) -> ApiResult<StatusCode> {
    let a = own_account(&state, &user, id).await?;
    crate::mailclient::forget(id);
    if let Some(pw) = p.password.as_deref().filter(|x| !x.is_empty()) {
        let mut s = connect(&a.imap_host, a.imap_port as u16, &a.security, &a.username, pw).await?;
        let _ = s.logout().await;
    }
    let label = p.label.as_deref().map(str::trim).filter(|l| !l.is_empty() && l.chars().count() <= 120);
    if p.display_name.is_some() || p.signature.is_some() {
        sqlx::query(
            "UPDATE mail_accounts SET display_name = CASE WHEN $2 THEN $3 ELSE display_name END,
                signature = CASE WHEN $4 THEN $5 ELSE signature END WHERE id = $1",
        )
        .bind(id)
        .bind(p.display_name.is_some())
        .bind(clean_text(p.display_name.as_deref(), 120))
        .bind(p.signature.is_some())
        .bind(clean_text(p.signature.as_deref(), 4000))
        .execute(&state.pool)
        .await?;
    }
    if p.smtp_remove == Some(true) {
        sqlx::query("UPDATE mail_accounts SET smtp_host = NULL, smtp_port = NULL, smtp_security = NULL, smtp_username = NULL, smtp_password_enc = NULL WHERE id = $1")
            .bind(id)
            .execute(&state.pool)
            .await?;
    } else if let Some(r) = &p.smtp {
        let imap_pass = match p.password.as_deref().filter(|x| !x.is_empty()) {
            Some(pw) => pw.to_string(),
            None => crate::crypto::decrypt(&a.password_enc).unwrap_or_default(),
        };
        // Mot de passe SMTP laissé vide : on garde l'actuel (sinon celui de l'IMAP).
        let keep = a.smtp_password_enc.as_deref().and_then(crate::crypto::decrypt);
        let v = r.validate(&a.username, &imap_pass, keep.as_deref()).await?;
        sqlx::query("UPDATE mail_accounts SET smtp_host = $2, smtp_port = $3, smtp_security = $4, smtp_username = $5, smtp_password_enc = $6 WHERE id = $1")
            .bind(id)
            .bind(&v.host)
            .bind(v.port as i32)
            .bind(&v.security)
            .bind(&v.username)
            .bind(&v.password_enc)
            .execute(&state.pool)
            .await?;
    }
    sqlx::query(
        "UPDATE mail_accounts SET label = COALESCE($2, label), notify = COALESCE($3, notify),
            webmail_url = CASE WHEN $4 THEN $5 ELSE webmail_url END,
            password_enc = COALESCE($6, password_enc), last_error = CASE WHEN $6 IS NOT NULL THEN NULL ELSE last_error END
         WHERE id = $1",
    )
    .bind(id)
    .bind(label)
    .bind(p.notify)
    .bind(p.webmail_url.is_some())
    .bind(clean_webmail(p.webmail_url.as_deref())?)
    .bind(p.password.as_deref().filter(|x| !x.is_empty()).map(crate::crypto::encrypt))
    .execute(&state.pool)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_account(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<StatusCode> {
    own_account(&state, &user, id).await?;
    crate::mailclient::forget(id);
    sqlx::query("DELETE FROM mail_accounts WHERE id = $1").bind(id).execute(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn refresh(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<Json<serde_json::Value>> {
    let a = own_account(&state, &user, id).await?;
    let n = sync_and_notify(&state, &a).await?;
    Ok(Json(serde_json::json!({ "new": n })))
}

#[derive(Deserialize)]
struct ListQuery {
    before: Option<i64>,
    /// Non lus seulement (tous en cache, cf. synchronisation).
    unread: Option<bool>,
}

#[derive(Serialize, sqlx::FromRow)]
struct MessageDto {
    uid: i64,
    from_name: String,
    from_addr: String,
    subject: String,
    snippet: String,
    sent_at: Option<DateTime<Utc>>,
    seen: bool,
    has_attachments: bool,
}

const PAGE: usize = 50;

async fn cached_page(pool: &sqlx::PgPool, id: Uuid, before: Option<i64>, unread: bool) -> ApiResult<Vec<MessageDto>> {
    Ok(sqlx::query_as(
        "SELECT uid, from_name, from_addr, subject, snippet, sent_at, seen, has_attachments FROM mail_messages
         WHERE account_id = $1 AND ($2::bigint IS NULL OR uid < $2) AND NOT ($3 AND seen) ORDER BY uid DESC LIMIT 51",
    )
    .bind(id)
    .bind(before)
    .bind(unread)
    .fetch_all(pool)
    .await?)
}

/// Une page de la boîte de réception. Au-delà du cache, les messages plus anciens sont
/// récupérés sur le serveur IMAP à la demande (« Charger plus »).
async fn list_messages(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Query(q): Query<ListQuery>) -> ApiResult<Json<serde_json::Value>> {
    let a = own_account(&state, &user, id).await?;
    let unread = q.unread.unwrap_or(false);
    let mut rows = cached_page(&state.pool, id, q.before, unread).await?;
    let mut server_has_more = false;
    if !unread && rows.len() <= PAGE && a.last_uid > 0 {
        let oldest: Option<i64> = sqlx::query_scalar("SELECT MIN(uid) FROM mail_messages WHERE account_id = $1").bind(id).fetch_one(&state.pool).await?;
        if let Some(oldest) = oldest {
            let mut s = connect_account(&a).await?;
            s.select("INBOX").await.map_err(imap_err)?;
            let mut older: Vec<u32> = s.uid_search("ALL").await.map_err(imap_err)?.into_iter().filter(|u| (*u as i64) < oldest).collect();
            older.sort_unstable();
            let want = PAGE + 1 - rows.len();
            server_has_more = older.len() > want;
            let take: Vec<u32> = older.iter().rev().take(want).copied().collect();
            if !take.is_empty() {
                let set = take.iter().map(|u| u.to_string()).collect::<Vec<_>>().join(",");
                let fetched: Vec<_> = s.uid_fetch(set, "(UID FLAGS BODY.PEEK[]<0.32768>)").await.map_err(imap_err)?.try_collect().await.map_err(imap_err)?;
                store_fetches(&state.pool, id, &fetched).await?;
                rows = cached_page(&state.pool, id, q.before, false).await?;
            }
            let _ = s.logout().await;
        }
    }
    let has_more = rows.len() > PAGE || server_has_more;
    rows.truncate(PAGE);
    Ok(Json(serde_json::json!({ "messages": rows, "has_more": has_more })))
}

#[derive(Deserialize)]
struct SeenReq {
    seen: bool,
}

/// Marque un message comme lu ou non lu (sur le serveur et dans le cache).
async fn set_seen(State(state): State<AppState>, user: AuthUser, Path((id, uid)): Path<(Uuid, i64)>, Json(p): Json<SeenReq>) -> ApiResult<StatusCode> {
    let a = own_account(&state, &user, id).await?;
    let mut s = connect_account(&a).await?;
    s.select("INBOX").await.map_err(imap_err)?;
    let op = if p.seen { "+FLAGS.SILENT (\\Seen)" } else { "-FLAGS.SILENT (\\Seen)" };
    let _: Vec<_> = s.uid_store(uid.to_string(), op).await.map_err(imap_err)?.try_collect().await.map_err(imap_err)?;
    let _ = s.logout().await;
    sqlx::query("UPDATE mail_messages SET seen = $3 WHERE account_id = $1 AND uid = $2").bind(id).bind(uid).bind(p.seen).execute(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn read_all(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<StatusCode> {
    let a = own_account(&state, &user, id).await?;
    let mut s = connect_account(&a).await?;
    let mailbox = s.select("INBOX").await.map_err(imap_err)?;
    if mailbox.exists > 0 {
        let _: Vec<_> = s.uid_store("1:*", "+FLAGS.SILENT (\\Seen)").await.map_err(imap_err)?.try_collect().await.map_err(imap_err)?;
    }
    let _ = s.logout().await;
    sqlx::query("UPDATE mail_messages SET seen = TRUE WHERE account_id = $1 AND NOT seen").bind(id).execute(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// HTML d'un message, nettoyé : ni scripts, ni formulaires, ni cadres ; liens ouverts à
/// part. Affiché dans un cadre isolé (sandbox) qui bloque aussi les images distantes.
pub(crate) fn clean_html(html: &str) -> String {
    ammonia::Builder::default()
        .add_generic_attributes(["style", "align", "valign", "bgcolor", "width", "height", "border", "cellpadding", "cellspacing", "color"])
        .add_tags(["font", "center"])
        .link_rel(Some("noopener noreferrer nofollow"))
        .set_tag_attribute_value("a", "target", "_blank")
        .url_schemes(["http", "https", "mailto", "cid", "data"].into_iter().collect())
        .clean(html)
        .to_string()
}

async fn read_message(State(state): State<AppState>, user: AuthUser, Path((id, uid)): Path<(Uuid, i64)>) -> ApiResult<Json<serde_json::Value>> {
    let a = own_account(&state, &user, id).await?;
    let mut s = connect_account(&a).await?;
    s.select("INBOX").await.map_err(imap_err)?;
    // BODY[] (sans PEEK) : le message est marqué comme lu, comme dans un client classique.
    let fetches: Vec<_> = s.uid_fetch(uid.to_string(), "(UID BODY[])").await.map_err(imap_err)?.try_collect().await.map_err(imap_err)?;
    let _ = s.logout().await;
    let raw = fetches.iter().find(|f| f.uid.map(|u| u as i64) == Some(uid)).and_then(|f| f.body()).ok_or(ApiError::NotFound)?;
    let msg = MessageParser::default().parse(raw).ok_or_else(|| ApiError::bad_request("mail_unreadable"))?;
    let addrs = |list: Option<&mail_parser::Address>| -> Vec<serde_json::Value> {
        list.map(|l| l.iter().map(|a| serde_json::json!({ "name": a.name(), "address": a.address() })).collect()).unwrap_or_default()
    };
    // Vrai HTML seulement : pour un message texte, mail-parser fabrique un HTML (sans liens).
    let html = msg.html_part(0).filter(|p| p.is_text_html()).and_then(|_| msg.body_html(0)).map(|h| clean_html(&h));
    let text = if html.is_none() { msg.body_text(0).map(|t| t.to_string()) } else { None };
    let attachments: Vec<serde_json::Value> = msg
        .attachments()
        .map(|p| serde_json::json!({ "name": p.attachment_name().unwrap_or("attachment"), "size": p.len(), "type": p.content_type().map(|c| format!("{}/{}", c.ctype(), c.subtype().unwrap_or(""))) }))
        .collect();
    sqlx::query("UPDATE mail_messages SET seen = TRUE WHERE account_id = $1 AND uid = $2").bind(id).bind(uid).execute(&state.pool).await?;
    Ok(Json(serde_json::json!({
        "uid": uid,
        "subject": msg.subject(),
        "from": addrs(msg.from()),
        "to": addrs(msg.to()),
        "cc": addrs(msg.cc()),
        "date": msg.date().and_then(|d| DateTime::from_timestamp(d.to_timestamp(), 0)),
        "html": html,
        "text": text,
        "attachments": attachments,
    })))
}

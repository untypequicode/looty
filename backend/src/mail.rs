//! Envoi d'e-mails (SMTP) : configuration de l'instance et transport.
//!
//! La configuration se fait dans l'administration (mot de passe chiffré en base) ;
//! à défaut, les variables `SMTP_HOST`, `SMTP_PORT`, `SMTP_SECURITY`
//! (`starttls` | `tls` | `none`), `SMTP_USERNAME`, `SMTP_PASSWORD`, `SMTP_FROM`
//! et `SMTP_FROM_NAME` sont utilisées.

use axum::{extract::State, routing::{get, post}, Json, Router};
use lettre::{
    message::{header::ContentType, Mailbox, MultiPart, SinglePart},
    transport::smtp::{authentication::Credentials, client::{Tls, TlsParameters}},
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::time::Duration;

use crate::access::AuthUser;
use crate::admin::require_super_admin;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

#[derive(Clone, Debug)]
pub struct MailConfig {
    pub host: String,
    pub port: u16,
    pub security: String,
    pub username: String,
    pub password: String,
    pub from_address: String,
    pub from_name: String,
    pub delay_minutes: i32,
}

#[derive(sqlx::FromRow)]
struct MailRow {
    enabled: bool,
    host: String,
    port: i32,
    security: String,
    username: String,
    password_enc: Option<String>,
    from_address: String,
    from_name: String,
    delay_minutes: i32,
}

fn env_config() -> Option<MailConfig> {
    let host = std::env::var("SMTP_HOST").ok().filter(|h| !h.trim().is_empty())?;
    let from = std::env::var("SMTP_FROM").ok().filter(|h| !h.trim().is_empty())?;
    // Les variables vides (docker compose) valent « non définies ».
    let var = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
    let security = var("SMTP_SECURITY").unwrap_or_else(|| "starttls".into());
    let default_port = match security.as_str() { "tls" => 465, "none" => 25, _ => 587 };
    Some(MailConfig {
        host,
        port: var("SMTP_PORT").and_then(|p| p.parse().ok()).unwrap_or(default_port),
        security,
        username: var("SMTP_USERNAME").unwrap_or_default(),
        password: var("SMTP_PASSWORD").unwrap_or_default(),
        from_address: from,
        from_name: var("SMTP_FROM_NAME").unwrap_or_else(|| "Looty".into()),
        delay_minutes: 2,
    })
}

/// Configuration active : celle de l'administration si elle est activée, sinon l'environnement.
pub async fn config(pool: &PgPool) -> Option<MailConfig> {
    let row: Option<MailRow> = sqlx::query_as("SELECT enabled, host, port, security, username, password_enc, from_address, from_name, delay_minutes FROM mail_settings WHERE id = 1")
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();
    match row {
        Some(r) if r.enabled && !r.host.is_empty() && !r.from_address.is_empty() => Some(MailConfig {
            host: r.host,
            port: r.port as u16,
            security: r.security,
            username: r.username,
            password: r.password_enc.as_deref().and_then(crate::crypto::decrypt).unwrap_or_default(),
            from_address: r.from_address,
            from_name: r.from_name,
            delay_minutes: r.delay_minutes,
        }),
        Some(r) if r.enabled => None,
        _ => env_config(),
    }
}

fn transport(c: &MailConfig) -> ApiResult<AsyncSmtpTransport<Tokio1Executor>> {
    let tls = || TlsParameters::new(c.host.clone()).map_err(|e| ApiError::BadRequest(format!("smtp_tls: {e}")));
    let mut b = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&c.host)
        .port(c.port)
        .timeout(Some(Duration::from_secs(20)));
    b = match c.security.as_str() {
        "tls" => b.tls(Tls::Wrapper(tls()?)),
        "none" => b.tls(Tls::None),
        _ => b.tls(Tls::Required(tls()?)),
    };
    if !c.username.is_empty() {
        b = b.credentials(Credentials::new(c.username.clone(), c.password.clone()));
    }
    Ok(b.build())
}

pub struct Email {
    pub to: String,
    pub to_name: String,
    pub subject: String,
    pub text: String,
    pub html: String,
}

pub async fn send(c: &MailConfig, e: &Email) -> ApiResult<()> {
    let name = c.from_name.replace(['<', '>', '"', ','], "");
    let from: Mailbox = format!("{} <{}>", if name.trim().is_empty() { "Looty" } else { name.trim() }, c.from_address)
        .parse()
        .map_err(|_| ApiError::bad_request("smtp_invalid_from"))?;
    let to: Mailbox = format!("{} <{}>", e.to_name.replace(['<', '>', '"', ','], ""), e.to)
        .parse()
        .or_else(|_| e.to.parse())
        .map_err(|_| ApiError::bad_request("smtp_invalid_recipient"))?;
    let msg = Message::builder()
        .from(from)
        .to(to)
        .subject(&e.subject)
        .multipart(
            MultiPart::alternative()
                .singlepart(SinglePart::builder().header(ContentType::TEXT_PLAIN).body(e.text.clone()))
                .singlepart(SinglePart::builder().header(ContentType::TEXT_HTML).body(e.html.clone())),
        )
        .map_err(|err| ApiError::BadRequest(format!("smtp_message: {err}")))?;
    transport(c)?.send(msg).await.map_err(|err| ApiError::BadRequest(format!("smtp_send_failed: {err}")))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Administration
// ---------------------------------------------------------------------------

pub fn admin_routes() -> Router<AppState> {
    Router::new().route("/", get(get_settings).put(put_settings)).route("/test", post(test))
}

#[derive(Serialize)]
struct SettingsDto {
    enabled: bool,
    host: String,
    port: i32,
    security: String,
    username: String,
    has_password: bool,
    from_address: String,
    from_name: String,
    delay_minutes: i32,
    /// Configuration fournie par l'environnement (utilisée si rien n'est activé ici).
    env_configured: bool,
}

async fn get_settings(State(pool): State<PgPool>, user: AuthUser) -> ApiResult<Json<SettingsDto>> {
    require_super_admin(&user)?;
    let r: Option<MailRow> = sqlx::query_as("SELECT enabled, host, port, security, username, password_enc, from_address, from_name, delay_minutes FROM mail_settings WHERE id = 1")
        .fetch_optional(&pool)
        .await?;
    let env_configured = env_config().is_some();
    Ok(Json(match r {
        Some(r) => SettingsDto {
            enabled: r.enabled, host: r.host, port: r.port, security: r.security, username: r.username,
            has_password: r.password_enc.is_some(), from_address: r.from_address, from_name: r.from_name,
            delay_minutes: r.delay_minutes, env_configured,
        },
        None => SettingsDto {
            enabled: false, host: String::new(), port: 587, security: "starttls".into(), username: String::new(),
            has_password: false, from_address: String::new(), from_name: "Looty".into(), delay_minutes: 2, env_configured,
        },
    }))
}

#[derive(Deserialize)]
struct SettingsReq {
    enabled: bool,
    host: String,
    port: i32,
    security: String,
    username: String,
    /// Absent = inchangé ; chaîne vide = supprimé.
    password: Option<String>,
    from_address: String,
    from_name: String,
    delay_minutes: i32,
}

fn valid_email(s: &str) -> bool {
    s.parse::<lettre::Address>().is_ok()
}

async fn put_settings(State(pool): State<PgPool>, user: AuthUser, Json(p): Json<SettingsReq>) -> ApiResult<Json<SettingsDto>> {
    require_super_admin(&user)?;
    let host = p.host.trim();
    let from = p.from_address.trim();
    if !matches!(p.security.as_str(), "starttls" | "tls" | "none") || !(1..=65535).contains(&p.port) || !(0..=1440).contains(&p.delay_minutes) {
        return Err(ApiError::bad_request("invalid_value"));
    }
    if host.len() > 255 || host.contains(char::is_whitespace) || p.username.len() > 255 || p.from_name.chars().count() > 120 {
        return Err(ApiError::bad_request("invalid_value"));
    }
    if p.enabled && (host.is_empty() || !valid_email(from)) {
        return Err(ApiError::bad_request("smtp_incomplete"));
    }
    if !from.is_empty() && !valid_email(from) {
        return Err(ApiError::bad_request("smtp_invalid_from"));
    }
    let password = p.password.as_ref().map(|pw| if pw.is_empty() { None } else { Some(crate::crypto::encrypt(pw)) });
    sqlx::query(
        "INSERT INTO mail_settings (id, enabled, host, port, security, username, password_enc, from_address, from_name, delay_minutes, updated_at)
         VALUES (1, $1, $2, $3, $4, $5, $7, $8, $9, $10, NOW())
         ON CONFLICT (id) DO UPDATE SET enabled = $1, host = $2, port = $3, security = $4, username = $5,
            password_enc = CASE WHEN $6 THEN $7 ELSE mail_settings.password_enc END,
            from_address = $8, from_name = $9, delay_minutes = $10, updated_at = NOW()",
    )
    .bind(p.enabled)
    .bind(host)
    .bind(p.port)
    .bind(&p.security)
    .bind(p.username.trim())
    .bind(password.is_some())
    .bind(password.flatten())
    .bind(from)
    .bind(p.from_name.trim())
    .bind(p.delay_minutes)
    .execute(&pool)
    .await?;
    get_settings(State(pool), user).await
}

/// Envoie un e-mail de test à l'administrateur avec la configuration active.
async fn test(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<serde_json::Value>> {
    require_super_admin(&user)?;
    let c = config(&state.pool).await.ok_or_else(|| ApiError::bad_request("smtp_not_configured"))?;
    let (email, name, locale): (String, String, Option<String>) = sqlx::query_as("SELECT email, name, locale FROM users WHERE id = $1").bind(user.id).fetch_one(&state.pool).await?;
    let fr = locale.as_deref() == Some("fr");
    let (subject, line) = if fr {
        ("E-mail de test Looty", "La configuration SMTP fonctionne : Looty peut envoyer des e-mails.")
    } else {
        ("Looty test email", "SMTP is configured correctly: Looty can send emails.")
    };
    let mail = crate::delivery::render_email(&state.base_url, fr, subject, line, None, &state.base_url, None);
    send(&c, &Email { to: email.clone(), to_name: name, subject: subject.into(), text: mail.0, html: mail.1 }).await?;
    Ok(Json(serde_json::json!({ "sent_to": email })))
}

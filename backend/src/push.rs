//! Notifications push (Web Push, RFC 8030 / 8291 / 8292).
//!
//! La paire de clés VAPID est générée au premier besoin et stockée chiffrée ;
//! chaque navigateur abonné enregistre son point de terminaison. Les points de
//! terminaison sont fournis par le client : ils doivent être en https et résoudre
//! vers des adresses publiques (garde-fou SSRF).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64URL, Engine};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::time::Duration;
use tokio::sync::OnceCell;
use uuid::Uuid;
use web_push_native::{jwt_simple::algorithms::ES256KeyPair, p256::PublicKey, Auth, WebPushBuilder};

use crate::access::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

static KEYS: OnceCell<Option<(Vec<u8>, String)>> = OnceCell::const_new();

/// (clé privée brute, clé publique base64url non compressée)
async fn keys(pool: &PgPool) -> Option<&'static (Vec<u8>, String)> {
    KEYS.get_or_init(|| async {
        let stored: Option<String> = sqlx::query_scalar("SELECT private_key_enc FROM push_keys WHERE id = 1").fetch_optional(pool).await.ok().flatten();
        let raw = match stored.and_then(|s| crate::crypto::decrypt(&s)).and_then(|s| B64URL.decode(s).ok()) {
            Some(r) => r,
            None => {
                let kp = ES256KeyPair::generate();
                let raw = kp.to_bytes();
                let enc = crate::crypto::encrypt(&B64URL.encode(&raw));
                // Une autre instance a pu en créer une entre-temps : on relit après insertion.
                sqlx::query("INSERT INTO push_keys (id, private_key_enc) VALUES (1, $1) ON CONFLICT (id) DO NOTHING").bind(enc).execute(pool).await.ok()?;
                let s: String = sqlx::query_scalar("SELECT private_key_enc FROM push_keys WHERE id = 1").fetch_one(pool).await.ok()?;
                crate::crypto::decrypt(&s).and_then(|s| B64URL.decode(s).ok()).unwrap_or(raw)
            }
        };
        let kp = ES256KeyPair::from_bytes(&raw).ok()?;
        let pk = PublicKey::from_sec1_bytes(&kp.public_key().to_bytes()).ok()?;
        use web_push_native::p256::elliptic_curve::sec1::ToEncodedPoint;
        Some((raw, B64URL.encode(pk.to_encoded_point(false).as_bytes())))
    })
    .await
    .as_ref()
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/key", get(public_key))
        .route("/subscriptions", get(list).post(subscribe))
        .route("/subscriptions/{id}", delete(unsubscribe))
        .route("/test", post(test))
}

async fn public_key(State(pool): State<PgPool>, _user: AuthUser) -> ApiResult<Json<serde_json::Value>> {
    let (_, public) = keys(&pool).await.ok_or_else(|| ApiError::Internal("vapid".into()))?;
    Ok(Json(serde_json::json!({ "key": public })))
}

#[derive(Serialize, sqlx::FromRow)]
struct SubscriptionDto {
    id: Uuid,
    device: Option<String>,
    endpoint: String,
    endpoint_host: String,
    created_at: chrono::DateTime<chrono::Utc>,
    last_used_at: Option<chrono::DateTime<chrono::Utc>>,
}

async fn list(State(pool): State<PgPool>, user: AuthUser) -> ApiResult<Json<Vec<SubscriptionDto>>> {
    Ok(Json(
        sqlx::query_as(
            "SELECT id, device, endpoint, COALESCE(substring(endpoint from '^https?://([^/]+)'), '') AS endpoint_host, created_at, last_used_at
             FROM push_subscriptions WHERE user_id = $1 ORDER BY created_at DESC",
        )
        .bind(user.id)
        .fetch_all(&pool)
        .await?,
    ))
}

#[derive(Deserialize)]
struct SubKeys {
    p256dh: String,
    auth: String,
}
#[derive(Deserialize)]
struct SubscribeReq {
    endpoint: String,
    keys: SubKeys,
    device: Option<String>,
}

/// Le point de terminaison doit être en https et sur une adresse publique.
async fn check_endpoint(raw: &str) -> ApiResult<()> {
    let url = reqwest::Url::parse(raw).map_err(|_| ApiError::bad_request("push_invalid_endpoint"))?;
    // Tests uniquement : autorise un service push local (http / réseau privé).
    let insecure = std::env::var("PUSH_ALLOW_PRIVATE").as_deref() == Ok("true");
    if insecure && matches!(url.scheme(), "http" | "https") && raw.len() <= 2000 {
        return Ok(());
    }
    if url.scheme() != "https" || raw.len() > 2000 || !url.username().is_empty() {
        return Err(ApiError::bad_request("push_invalid_endpoint"));
    }
    let host = url.host_str().ok_or_else(|| ApiError::bad_request("push_invalid_endpoint"))?;
    let addrs = tokio::net::lookup_host((host, url.port_or_known_default().unwrap_or(443))).await.map_err(|_| ApiError::bad_request("push_invalid_endpoint"))?;
    let ips: Vec<_> = addrs.map(|a| a.ip()).collect();
    if ips.is_empty() || ips.iter().any(|ip| crate::caldav::is_private(*ip)) {
        return Err(ApiError::bad_request("push_invalid_endpoint"));
    }
    Ok(())
}

async fn subscribe(State(pool): State<PgPool>, user: AuthUser, Json(p): Json<SubscribeReq>) -> ApiResult<Json<serde_json::Value>> {
    check_endpoint(&p.endpoint).await?;
    let p256 = B64URL.decode(p.keys.p256dh.trim_end_matches('=')).ok().filter(|k| PublicKey::from_sec1_bytes(k).is_ok());
    let auth = B64URL.decode(p.keys.auth.trim_end_matches('=')).ok().filter(|a| a.len() == 16);
    if p256.is_none() || auth.is_none() {
        return Err(ApiError::bad_request("push_invalid_keys"));
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM push_subscriptions WHERE user_id = $1").bind(user.id).fetch_one(&pool).await?;
    if count >= 20 {
        return Err(ApiError::bad_request("push_too_many"));
    }
    let device: Option<String> = p.device.map(|d| d.chars().filter(|c| !c.is_control()).take(120).collect());
    // Un point de terminaison appartient à un seul utilisateur (réabonnement après changement de compte).
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO push_subscriptions (user_id, endpoint, p256dh, auth, device) VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (endpoint) DO UPDATE SET user_id = $1, p256dh = $3, auth = $4, device = $5
         RETURNING id",
    )
    .bind(user.id)
    .bind(&p.endpoint)
    .bind(&p.keys.p256dh)
    .bind(&p.keys.auth)
    .bind(device)
    .fetch_one(&pool)
    .await?;
    Ok(Json(serde_json::json!({ "id": id })))
}

async fn unsubscribe(State(pool): State<PgPool>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<StatusCode> {
    sqlx::query("DELETE FROM push_subscriptions WHERE id = $1 AND user_id = $2").bind(id).bind(user.id).execute(&pool).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn test(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<serde_json::Value>> {
    let locale: Option<String> = sqlx::query_scalar("SELECT locale FROM users WHERE id = $1").bind(user.id).fetch_one(&state.pool).await?;
    let body = if locale.as_deref() == Some("fr") { "Les notifications push fonctionnent sur cet appareil." } else { "Push notifications work on this device." };
    let payload = serde_json::json!({ "title": "Looty", "body": body, "url": "/", "tag": "looty-test" });
    let sent = send_to_user(&state, user.id, &payload).await?;
    Ok(Json(serde_json::json!({ "sent": sent })))
}

/// Envoie une notification à tous les appareils d'un utilisateur ; renvoie le nombre de réussites.
/// Les abonnements expirés (404 / 410) sont supprimés.
pub async fn send_to_user(state: &AppState, user_id: Uuid, payload: &serde_json::Value) -> ApiResult<u32> {
    let (raw, _) = keys(&state.pool).await.ok_or_else(|| ApiError::Internal("vapid".into()))?;
    let kp = ES256KeyPair::from_bytes(raw).map_err(|e| ApiError::Internal(e.to_string()))?;
    let contact = crate::mail::config(&state.pool).await.map(|c| format!("mailto:{}", c.from_address)).unwrap_or_else(|| state.base_url.clone());
    let subs: Vec<(Uuid, String, String, String)> = sqlx::query_as("SELECT id, endpoint, p256dh, auth FROM push_subscriptions WHERE user_id = $1")
        .bind(user_id)
        .fetch_all(&state.pool)
        .await?;
    let body = serde_json::to_vec(payload).unwrap_or_default();
    let mut ok = 0;
    for (id, endpoint, p256dh, auth) in subs {
        let (Ok(uri), Some(pk), Some(auth)) = (
            endpoint.parse(),
            B64URL.decode(p256dh.trim_end_matches('=')).ok().and_then(|k| PublicKey::from_sec1_bytes(&k).ok()),
            B64URL.decode(auth.trim_end_matches('=')).ok().filter(|a| a.len() == 16),
        ) else {
            continue;
        };
        let mut ua_auth = Auth::default();
        ua_auth.copy_from_slice(&auth);
        let req = match WebPushBuilder::new(uri, pk, ua_auth)
            .with_valid_duration(Duration::from_secs(12 * 3600))
            .with_vapid(&kp, &contact)
            .build(body.clone())
        {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("push build failed: {e}");
                continue;
            }
        };
        if check_endpoint(&endpoint).await.is_err() {
            continue;
        }
        let mut r = http_client().post(&endpoint).body(req.body().clone());
        for (k, v) in req.headers() {
            r = r.header(k.as_str(), v.as_bytes());
        }
        match r.header("Urgency", "normal").send().await {
            Ok(res) if res.status().is_success() => {
                ok += 1;
                sqlx::query("UPDATE push_subscriptions SET last_used_at = NOW() WHERE id = $1").bind(id).execute(&state.pool).await.ok();
            }
            Ok(res) if res.status() == reqwest::StatusCode::NOT_FOUND || res.status() == reqwest::StatusCode::GONE => {
                sqlx::query("DELETE FROM push_subscriptions WHERE id = $1").bind(id).execute(&state.pool).await.ok();
            }
            Ok(res) => tracing::warn!("push to {} failed: {}", reqwest::Url::parse(&endpoint).ok().and_then(|u| u.host_str().map(str::to_string)).unwrap_or_default(), res.status()),
            Err(e) => tracing::warn!("push send error: {e}"),
        }
    }
    Ok(ok)
}

fn http_client() -> &'static reqwest::Client {
    static C: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    C.get_or_init(|| {
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(15))
            .build()
            .expect("http client")
    })
}

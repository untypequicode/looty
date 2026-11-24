//! Webhooks d'espace (GLOSSARY : Webhook) : l'adresse choisie reçoit un POST signé quand une tâche, un
//! document, un événement ou une annonce de l'espace change.
//!
//! - Gérés par les administrateurs de l'espace ; jamais sur un espace personnel, et jamais pour un
//!   élément personnel (ADR 0002).
//! - Chaque livraison est une ligne de `webhook_deliveries` : elle sert de file (reprise après un
//!   redémarrage) et de journal. Échec = nouvelle tentative avec un délai croissant, six au plus ;
//!   dix livraisons abandonnées d'affilée désactivent le webhook.
//! - Signature : `X-Looty-Signature: sha256=<HMAC-SHA256(secret, "<horodatage>.<corps>")>` avec
//!   l'horodatage de `X-Looty-Timestamp`, pour que le destinataire refuse les rejeux.
//! - Le serveur n'appelle pas d'adresse privée sauf `WEBHOOKS_ALLOW_PRIVATE=true` (ADR 0014).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post, put},
    Json, Router,
};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::sync::Notify;
use uuid::Uuid;

use crate::access::{require_ws_admin, AuthUser};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

/// Événements qu'un webhook peut recevoir.
pub const EVENTS: &[&str] = &["task.created", "task.updated", "task.completed", "page.updated", "event.created", "announcement.published"];
const MAX_HOOKS: i64 = 20;
const MAX_PENDING: i64 = 1000;
/// Délais avant la nouvelle tentative, après la 1re, 2e… livraison ratée (donc 6 tentatives au plus).
const BACKOFF_MINUTES: [i64; 5] = [1, 5, 30, 120, 720];
const DISABLE_AFTER: i32 = 10;

fn allow_private() -> bool {
    #[cfg(test)]
    if TEST_ALLOW_PRIVATE.with(|f| f.get()) {
        return true;
    }
    std::env::var("WEBHOOKS_ALLOW_PRIVATE").as_deref() == Ok("true")
}

// Les tests de livraison parlent à un serveur local (par fil : chaque test a le sien).
#[cfg(test)]
thread_local! {
    static TEST_ALLOW_PRIVATE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn wake() -> &'static Arc<Notify> {
    static W: OnceLock<Arc<Notify>> = OnceLock::new();
    W.get_or_init(|| Arc::new(Notify::new()))
}

// ---------------------------------------------------------------------------
// Signature
// ---------------------------------------------------------------------------

pub(crate) fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
    let mut k = [0u8; 64];
    if key.len() > 64 {
        k[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut inner = Sha256::new();
    inner.update(k.map(|b| b ^ 0x36));
    inner.update(msg);
    let mut outer = Sha256::new();
    outer.update(k.map(|b| b ^ 0x5c));
    outer.update(inner.finalize());
    outer.finalize().into()
}

pub(crate) fn signature(secret: &str, timestamp: i64, body: &str) -> String {
    format!("sha256={}", hex::encode(hmac_sha256(secret.as_bytes(), format!("{timestamp}.{body}").as_bytes())))
}

// ---------------------------------------------------------------------------
// Adresse
// ---------------------------------------------------------------------------

/// Adresse valide (https, ou http si le réseau privé est autorisé) et, sauf autorisation, publique.
/// Renvoie l'adresse et les IP vérifiées, à utiliser telles quelles pour l'envoi (pas de nouvelle résolution).
pub(crate) async fn check_url(raw: &str) -> ApiResult<(reqwest::Url, Vec<SocketAddr>)> {
    let url = reqwest::Url::parse(raw.trim()).map_err(|_| ApiError::bad_request("webhook_invalid_url"))?;
    let private_ok = allow_private();
    match url.scheme() {
        "https" => {}
        "http" if private_ok => {}
        _ => return Err(ApiError::bad_request("webhook_https_required")),
    }
    if !url.username().is_empty() || url.password().is_some() || url.host_str().is_none() {
        return Err(ApiError::bad_request("webhook_invalid_url"));
    }
    let host = url.host_str().unwrap_or_default().trim_start_matches('[').trim_end_matches(']').to_string();
    let port = url.port_or_known_default().unwrap_or(443);
    let addrs: Vec<SocketAddr> = match host.parse::<IpAddr>() {
        Ok(ip) => vec![SocketAddr::new(ip, port)],
        Err(_) => tokio::net::lookup_host((host.as_str(), port)).await.map_err(|_| ApiError::bad_request("webhook_unreachable"))?.collect(),
    };
    if addrs.is_empty() {
        return Err(ApiError::bad_request("webhook_unreachable"));
    }
    if !private_ok && addrs.iter().any(|a| crate::caldav::is_private(a.ip())) {
        return Err(ApiError::bad_request("webhook_private_address"));
    }
    Ok((url, addrs))
}

// ---------------------------------------------------------------------------
// Gestion (administrateurs de l'espace)
// ---------------------------------------------------------------------------

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/{id}", put(update).delete(remove))
        .route("/{id}/secret", post(rotate_secret))
        .route("/{id}/test", post(test))
        .route("/{id}/deliveries", get(deliveries))
}

#[derive(Serialize, sqlx::FromRow)]
struct HookDto {
    id: Uuid,
    url: String,
    events: Vec<String>,
    active: bool,
    failures: i32,
    created_at: DateTime<Utc>,
}

const HOOK_COLUMNS: &str = "id, url, events, active, failures, created_at";

async fn require_hooks_admin(state: &AppState, user: &AuthUser, ws: Uuid) -> ApiResult<()> {
    require_ws_admin(&state.pool, user, ws).await?;
    let personal: bool = sqlx::query_scalar("SELECT personal_of IS NOT NULL FROM workspaces WHERE id = $1").bind(ws).fetch_one(&state.pool).await?;
    if personal {
        return Err(ApiError::Forbidden);
    }
    Ok(())
}

fn new_secret() -> String {
    format!("whsec_{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

fn clean_events(events: &[String]) -> ApiResult<Vec<String>> {
    let mut out: Vec<String> = events.to_vec();
    out.sort();
    out.dedup();
    if out.is_empty() || out.iter().any(|e| !EVENTS.contains(&e.as_str())) {
        return Err(ApiError::bad_request("webhook_invalid_events"));
    }
    Ok(out)
}

async fn list(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>) -> ApiResult<Json<serde_json::Value>> {
    require_hooks_admin(&state, &user, ws).await?;
    let hooks: Vec<HookDto> = sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT {HOOK_COLUMNS} FROM workspace_webhooks WHERE workspace_id = $1 ORDER BY created_at")))
        .bind(ws)
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(serde_json::json!({ "webhooks": hooks, "events": EVENTS })))
}

#[derive(Deserialize)]
struct HookReq {
    url: Option<String>,
    events: Option<Vec<String>>,
    active: Option<bool>,
}

async fn create(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>, Json(p): Json<HookReq>) -> ApiResult<Json<serde_json::Value>> {
    require_hooks_admin(&state, &user, ws).await?;
    let (url, _) = check_url(p.url.as_deref().unwrap_or("")).await?;
    let events = clean_events(p.events.as_deref().unwrap_or(&[]))?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM workspace_webhooks WHERE workspace_id = $1").bind(ws).fetch_one(&state.pool).await?;
    if count >= MAX_HOOKS {
        return Err(ApiError::bad_request("webhook_too_many"));
    }
    let secret = new_secret();
    let id: Uuid = sqlx::query_scalar("INSERT INTO workspace_webhooks (workspace_id, url, secret_enc, events, created_by) VALUES ($1, $2, $3, $4, $5) RETURNING id")
        .bind(ws)
        .bind(url.as_str())
        .bind(crate::crypto::encrypt(&secret))
        .bind(&events)
        .bind(user.id)
        .fetch_one(&state.pool)
        .await?;
    // Le secret n'est montré qu'ici (ou après une rotation) : seule sa version chiffrée est conservée.
    Ok(Json(serde_json::json!({ "id": id, "secret": secret })))
}

async fn update(State(state): State<AppState>, user: AuthUser, Path((ws, id)): Path<(Uuid, Uuid)>, Json(p): Json<HookReq>) -> ApiResult<Json<HookDto>> {
    require_hooks_admin(&state, &user, ws).await?;
    let url = match p.url.as_deref() {
        Some(u) => Some(check_url(u).await?.0.to_string()),
        None => None,
    };
    let events = p.events.as_deref().map(clean_events).transpose()?;
    let hook: HookDto = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "UPDATE workspace_webhooks SET url = COALESCE($3, url), events = COALESCE($4, events), active = COALESCE($5, active),
                failures = CASE WHEN $5 IS TRUE THEN 0 ELSE failures END
         WHERE id = $1 AND workspace_id = $2 RETURNING {HOOK_COLUMNS}"
    )))
    .bind(id)
    .bind(ws)
    .bind(url)
    .bind(events)
    .bind(p.active)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(ApiError::NotFound)?;
    Ok(Json(hook))
}

async fn remove(State(state): State<AppState>, user: AuthUser, Path((ws, id)): Path<(Uuid, Uuid)>) -> ApiResult<StatusCode> {
    require_hooks_admin(&state, &user, ws).await?;
    let n = sqlx::query("DELETE FROM workspace_webhooks WHERE id = $1 AND workspace_id = $2").bind(id).bind(ws).execute(&state.pool).await?.rows_affected();
    if n == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn rotate_secret(State(state): State<AppState>, user: AuthUser, Path((ws, id)): Path<(Uuid, Uuid)>) -> ApiResult<Json<serde_json::Value>> {
    require_hooks_admin(&state, &user, ws).await?;
    let secret = new_secret();
    let n = sqlx::query("UPDATE workspace_webhooks SET secret_enc = $3 WHERE id = $1 AND workspace_id = $2")
        .bind(id)
        .bind(ws)
        .bind(crate::crypto::encrypt(&secret))
        .execute(&state.pool)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(Json(serde_json::json!({ "secret": secret })))
}

async fn test(State(state): State<AppState>, user: AuthUser, Path((ws, id)): Path<(Uuid, Uuid)>) -> ApiResult<StatusCode> {
    require_hooks_admin(&state, &user, ws).await?;
    let found: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM workspace_webhooks WHERE id = $1 AND workspace_id = $2)").bind(id).bind(ws).fetch_one(&state.pool).await?;
    if !found {
        return Err(ApiError::NotFound);
    }
    enqueue(&state.pool, id, ws, "ping", serde_json::json!({ "message": "Looty webhook test" })).await?;
    Ok(StatusCode::ACCEPTED)
}

#[derive(Serialize, sqlx::FromRow)]
struct DeliveryDto {
    id: Uuid,
    event: String,
    status: String,
    attempts: i32,
    response_status: Option<i32>,
    error: Option<String>,
    created_at: DateTime<Utc>,
    next_attempt_at: DateTime<Utc>,
    delivered_at: Option<DateTime<Utc>>,
}

async fn deliveries(State(state): State<AppState>, user: AuthUser, Path((ws, id)): Path<(Uuid, Uuid)>) -> ApiResult<Json<Vec<DeliveryDto>>> {
    require_hooks_admin(&state, &user, ws).await?;
    Ok(Json(
        sqlx::query_as(
            "SELECT d.id, d.event, d.status, d.attempts, d.response_status, d.error, d.created_at, d.next_attempt_at, d.delivered_at
             FROM webhook_deliveries d JOIN workspace_webhooks h ON h.id = d.webhook_id
             WHERE h.id = $1 AND h.workspace_id = $2 ORDER BY d.created_at DESC LIMIT 100",
        )
        .bind(id)
        .bind(ws)
        .fetch_all(&state.pool)
        .await?,
    ))
}

// ---------------------------------------------------------------------------
// Émission
// ---------------------------------------------------------------------------

async fn enqueue(pool: &PgPool, hook: Uuid, ws: Uuid, event: &str, data: serde_json::Value) -> ApiResult<()> {
    let pending: i64 = sqlx::query_scalar("SELECT count(*) FROM webhook_deliveries WHERE webhook_id = $1 AND status = 'pending'").bind(hook).fetch_one(pool).await?;
    if pending >= MAX_PENDING {
        return Ok(());
    }
    let id = Uuid::new_v4();
    let payload = serde_json::json!({ "id": id, "event": event, "workspace_id": ws, "created_at": Utc::now(), "data": data });
    sqlx::query("INSERT INTO webhook_deliveries (id, webhook_id, event, payload) VALUES ($1, $2, $3, $4)").bind(id).bind(hook).bind(event).bind(payload).execute(pool).await?;
    wake().notify_one();
    Ok(())
}

/// Met en file l'événement pour chaque webhook actif de l'espace qui l'a choisi (sans bloquer l'appelant).
pub fn dispatch(state: &AppState, ws: Uuid, event: &'static str, data: serde_json::Value) {
    if state.demo {
        return;
    }
    let pool = state.pool.clone();
    tokio::spawn(async move {
        let hooks: Vec<Uuid> = match sqlx::query_scalar("SELECT id FROM workspace_webhooks WHERE workspace_id = $1 AND active AND $2 = ANY(events)").bind(ws).bind(event).fetch_all(&pool).await {
            Ok(h) => h,
            Err(e) => {
                tracing::warn!("webhooks: {e}");
                return;
            }
        };
        for h in hooks {
            if let Err(e) = enqueue(&pool, h, ws, event, data.clone()).await {
                tracing::warn!("webhooks: enqueue failed: {e:?}");
            }
        }
    });
}

fn pick(v: &serde_json::Value, keys: &[&str]) -> serde_json::Value {
    let mut out = serde_json::Map::new();
    for k in keys {
        if let Some(x) = v.get(*k) {
            out.insert((*k).to_string(), x.clone());
        }
    }
    serde_json::Value::Object(out)
}

/// Dernier état « terminée » vu pour chaque tâche : `task.completed` n'est envoyé qu'au passage à terminé
/// (la première mise à jour vue après un redémarrage d'une tâche déjà terminée peut le renvoyer une fois).
fn completed_seen() -> &'static DashMap<Uuid, bool> {
    static S: OnceLock<DashMap<Uuid, bool>> = OnceLock::new();
    S.get_or_init(DashMap::new)
}

/// Appelé par `AppState::emit` pour chaque événement temps réel : retient ceux qui ont un webhook.
/// Les éléments personnels n'en ont jamais (ADR 0002).
pub fn from_realtime(state: &AppState, ws: Uuid, event: &serde_json::Value) {
    let kind = event["type"].as_str().unwrap_or_default();
    let (key, obj) = match kind {
        "TASK_CREATED" | "TASK_UPDATED" | "TASK_MOVED" => ("task", &event["task"]),
        "PAGE_UPDATED" => ("page", &event["page"]),
        _ => return,
    };
    if !obj["private_to"].is_null() {
        return;
    }
    if key == "page" {
        dispatch(state, ws, "page.updated", pick(obj, &["id", "title", "parent_id", "updated_by", "updated_at"]));
        return;
    }
    let data = pick(obj, &["id", "column_id", "parent_id", "title", "is_completed", "due_date", "start_date", "priority", "assignees", "updated_at"]);
    let done = obj["is_completed"].as_bool().unwrap_or(false);
    let id = obj["id"].as_str().and_then(|s| Uuid::parse_str(s).ok());
    let was_done = id.map(|i| completed_seen().insert(i, done).unwrap_or(false)).unwrap_or(false);
    if completed_seen().len() > 50_000 {
        completed_seen().clear();
    }
    if kind == "TASK_CREATED" {
        dispatch(state, ws, "task.created", data);
    } else {
        dispatch(state, ws, "task.updated", data.clone());
        if done && !was_done {
            dispatch(state, ws, "task.completed", data);
        }
    }
}

// ---------------------------------------------------------------------------
// Livraison
// ---------------------------------------------------------------------------

pub fn start_worker(state: AppState) {
    tokio::spawn(async move {
        let mut last_purge = std::time::Instant::now() - Duration::from_secs(7200);
        loop {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(20)) => {}
                _ = wake().notified() => {}
            }
            if last_purge.elapsed() >= Duration::from_secs(3600) {
                last_purge = std::time::Instant::now();
                let _ = sqlx::query("DELETE FROM webhook_deliveries WHERE created_at < NOW() - INTERVAL '30 days' AND status <> 'pending'").execute(&state.pool).await;
            }
            crate::bg::tick("webhooks");
            if let Some(Err(e)) = crate::bg::guard_result("webhooks", process(&state)).await {
                tracing::warn!("webhooks: {e:?}");
            }
        }
    });
}

#[derive(sqlx::FromRow)]
struct Due {
    id: Uuid,
    webhook_id: Uuid,
    event: String,
    payload: serde_json::Value,
    attempts: i32,
    url: String,
    secret_enc: String,
}

async fn process(state: &AppState) -> ApiResult<()> {
    loop {
        // Réservation : la tentative est comptée et reportée d'avance, pour qu'un plantage ne la rejoue pas en boucle.
        let batch: Vec<Due> = sqlx::query_as(
            "WITH due AS (
                SELECT d.id FROM webhook_deliveries d JOIN workspace_webhooks h ON h.id = d.webhook_id
                WHERE d.status = 'pending' AND d.next_attempt_at <= NOW() AND h.active
                ORDER BY d.next_attempt_at LIMIT 20 FOR UPDATE OF d SKIP LOCKED
             ), claimed AS (
                UPDATE webhook_deliveries d SET attempts = d.attempts + 1, next_attempt_at = NOW() + INTERVAL '10 minutes'
                FROM due WHERE d.id = due.id
                RETURNING d.id, d.webhook_id, d.event, d.payload, d.attempts
             )
             SELECT c.id, c.webhook_id, c.event, c.payload, c.attempts, h.url, h.secret_enc FROM claimed c JOIN workspace_webhooks h ON h.id = c.webhook_id",
        )
        .fetch_all(&state.pool)
        .await?;
        if batch.is_empty() {
            // Les webhooks désactivés n'ont plus rien à envoyer : leurs livraisons en attente sont abandonnées.
            sqlx::query("UPDATE webhook_deliveries d SET status = 'failed', error = 'webhook_disabled' FROM workspace_webhooks h WHERE h.id = d.webhook_id AND NOT h.active AND d.status = 'pending'")
                .execute(&state.pool)
                .await?;
            return Ok(());
        }
        let mut sends = Vec::new();
        for d in batch {
            let pool = state.pool.clone();
            sends.push(tokio::spawn(async move { deliver(&pool, d).await }));
        }
        for s in sends {
            let _ = s.await;
        }
    }
}

async fn deliver(pool: &PgPool, d: Due) {
    let outcome = send(&d).await;
    match outcome {
        Ok(status) => {
            let _ = sqlx::query("UPDATE webhook_deliveries SET status = 'delivered', response_status = $2, error = NULL, delivered_at = NOW() WHERE id = $1").bind(d.id).bind(status as i32).execute(pool).await;
            let _ = sqlx::query("UPDATE workspace_webhooks SET failures = 0 WHERE id = $1").bind(d.webhook_id).execute(pool).await;
            crate::bg::record("webhooks", Ok(()));
        }
        Err((status, err)) => {
            let last = d.attempts as usize > BACKOFF_MINUTES.len();
            let delay = BACKOFF_MINUTES.get(d.attempts as usize - 1).copied().unwrap_or(0);
            let _ = sqlx::query(
                "UPDATE webhook_deliveries SET status = $2, response_status = $3, error = $4, next_attempt_at = NOW() + make_interval(mins => $5::int) WHERE id = $1",
            )
            .bind(d.id)
            .bind(if last { "failed" } else { "pending" })
            .bind(status.map(|s| s as i32))
            .bind(&err)
            .bind(delay as i32)
            .execute(pool)
            .await;
            if last {
                let _ = sqlx::query("UPDATE workspace_webhooks SET failures = failures + 1, active = (failures + 1 < $2) WHERE id = $1").bind(d.webhook_id).bind(DISABLE_AFTER).execute(pool).await;
            }
            crate::bg::record("webhooks", Err(err));
        }
    }
}

/// Un POST signé. `Ok(statut)` pour une réponse 2xx ; sinon le statut éventuel et la raison.
async fn send(d: &Due) -> Result<u16, (Option<u16>, String)> {
    let fail = |e: ApiError| (None, format!("{e:?}").chars().take(200).collect::<String>());
    // L'adresse est revérifiée à chaque envoi, et la connexion se fait sur les IP vérifiées (pas de nouvelle résolution).
    let (url, addrs) = check_url(&d.url).await.map_err(fail)?;
    let secret = crate::crypto::decrypt(&d.secret_enc).ok_or((None, "secret_unreadable".to_string()))?;
    let body = d.payload.to_string();
    let ts = Utc::now().timestamp();
    let mut builder = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(5))
        .user_agent("Looty-Webhook/1.0");
    if let Some(host) = url.host_str().filter(|h| h.parse::<IpAddr>().is_err()) {
        builder = builder.resolve_to_addrs(host, &addrs);
    }
    let client = builder.build().map_err(|e| (None, e.to_string()))?;
    let res = client
        .post(url)
        .header("Content-Type", "application/json")
        .header("X-Looty-Event", &d.event)
        .header("X-Looty-Delivery", d.id.to_string())
        .header("X-Looty-Timestamp", ts.to_string())
        .header("X-Looty-Signature", signature(&secret, ts, &body))
        .body(body)
        .send()
        .await
        .map_err(|e| (None, e.to_string().chars().take(200).collect::<String>()))?;
    let status = res.status();
    if status.is_success() {
        Ok(status.as_u16())
    } else {
        Err((Some(status.as_u16()), format!("http_{}", status.as_u16())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_matches_the_rfc_4231_vectors() {
        // Cas 2 : clé courte.
        assert_eq!(hex::encode(hmac_sha256(b"Jefe", b"what do ya want for nothing?")), "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843");
        // Cas 6 : clé plus longue que le bloc.
        assert_eq!(
            hex::encode(hmac_sha256(&[0xaa; 131], b"Test Using Larger Than Block-Size Key - Hash Key First")),
            "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54"
        );
    }

    #[test]
    fn signature_covers_the_timestamp_and_the_body() {
        let a = signature("s", 1, "{}");
        assert!(a.starts_with("sha256="));
        assert_ne!(a, signature("s", 2, "{}"));
        assert_ne!(a, signature("s", 1, "{ }"));
        assert_ne!(a, signature("t", 1, "{}"));
    }

    #[test]
    fn events_are_checked() {
        assert!(clean_events(&["task.created".into(), "task.created".into()]).is_ok_and(|e| e.len() == 1));
        assert!(clean_events(&[]).is_err());
        assert!(clean_events(&["task.deleted".into()]).is_err());
    }

    #[tokio::test]
    async fn private_and_non_https_addresses_are_refused() {
        for u in ["http://example.org/hook", "https://127.0.0.1/hook", "https://[::1]/hook", "https://10.0.0.5/x", "https://user:pw@1.1.1.1/x", "ftp://1.1.1.1/x", "nope"] {
            assert!(check_url(u).await.is_err(), "{u}");
        }
        assert!(check_url("https://1.1.1.1/hook").await.is_ok());
    }

    // -- Livraison de bout en bout, contre un petit serveur HTTP local --

    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// Serveur qui répond `status` à `n` requêtes et renvoie ce qu'il a reçu (en-têtes et corps).
    async fn server(status: u16, n: usize) -> (String, tokio::task::JoinHandle<Vec<String>>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/hook", listener.local_addr().unwrap());
        let handle = tokio::spawn(async move {
            let mut seen = Vec::new();
            for _ in 0..n {
                let (mut sock, _) = listener.accept().await.unwrap();
                let mut buf = vec![0u8; 16384];
                let mut got = 0;
                loop {
                    let r = sock.read(&mut buf[got..]).await.unwrap();
                    got += r;
                    let text = String::from_utf8_lossy(&buf[..got]).to_string();
                    if let Some(i) = text.find("\r\n\r\n") {
                        let len = text.lines().find_map(|l| l.to_lowercase().strip_prefix("content-length: ").and_then(|v| v.trim().parse::<usize>().ok())).unwrap_or(0);
                        if got >= i + 4 + len {
                            break;
                        }
                    }
                    if r == 0 {
                        break;
                    }
                }
                seen.push(String::from_utf8_lossy(&buf[..got]).to_string());
                let _ = sock.write_all(format!("HTTP/1.1 {status} X\r\ncontent-length: 0\r\nconnection: close\r\n\r\n").as_bytes()).await;
            }
            seen
        });
        (url, handle)
    }

    async fn setup(pool: &PgPool, url: &str, events: &[&str]) -> (AppState, Uuid, Uuid) {
        TEST_ALLOW_PRIVATE.with(|f| f.set(true));
        let user: Uuid = sqlx::query_scalar("INSERT INTO users (email, name) VALUES ('a@example.org', 'a') RETURNING id").fetch_one(pool).await.unwrap();
        let ws: Uuid = sqlx::query_scalar("INSERT INTO workspaces (name) VALUES ('W') RETURNING id").fetch_one(pool).await.unwrap();
        let hook: Uuid = sqlx::query_scalar("INSERT INTO workspace_webhooks (workspace_id, url, secret_enc, events, created_by) VALUES ($1, $2, $3, $4, $5) RETURNING id")
            .bind(ws)
            .bind(url)
            .bind(crate::crypto::encrypt("whsec_test"))
            .bind(events.iter().map(|e| e.to_string()).collect::<Vec<_>>())
            .bind(user)
            .fetch_one(pool)
            .await
            .unwrap();
        (AppState::new(pool.clone(), "http://localhost".into()), ws, hook)
    }

    async fn wait_for_deliveries(pool: &PgPool, n: i64) {
        for _ in 0..100 {
            let c: i64 = sqlx::query_scalar("SELECT count(*) FROM webhook_deliveries").fetch_one(pool).await.unwrap();
            if c >= n {
                return;
            }
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
        panic!("no delivery was queued");
    }

    #[sqlx::test]
    async fn a_chosen_event_is_delivered_signed_and_others_are_not(pool: PgPool) {
        let (url, srv) = server(200, 1).await;
        let (state, ws, _) = setup(&pool, &url, &["task.created"]).await;
        // Un événement non choisi, puis un élément personnel : rien n'est mis en file.
        from_realtime(&state, ws, &serde_json::json!({ "type": "PAGE_UPDATED", "page": { "id": Uuid::new_v4(), "title": "p" } }));
        from_realtime(&state, ws, &serde_json::json!({ "type": "TASK_CREATED", "task": { "id": Uuid::new_v4(), "title": "secret", "private_to": Uuid::new_v4() } }));
        tokio::time::sleep(Duration::from_millis(200)).await;
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM webhook_deliveries").fetch_one(&pool).await.unwrap();
        assert_eq!(n, 0);
        let task_id = Uuid::new_v4();
        from_realtime(&state, ws, &serde_json::json!({ "type": "TASK_CREATED", "task": { "id": task_id, "title": "Write docs", "description": "long text", "is_completed": false } }));
        wait_for_deliveries(&pool, 1).await;
        process(&state).await.unwrap();
        let req = srv.await.unwrap().remove(0);
        let (head, body) = req.split_once("\r\n\r\n").unwrap();
        let header = |name: &str| head.lines().find_map(|l| l.to_lowercase().strip_prefix(&format!("{name}: ")).map(|_| l.split_once(": ").unwrap().1.trim().to_string())).unwrap();
        assert_eq!(header("x-looty-event"), "task.created");
        let ts: i64 = header("x-looty-timestamp").parse().unwrap();
        assert_eq!(header("x-looty-signature"), signature("whsec_test", ts, body), "the receiver can verify the body");
        let payload: serde_json::Value = serde_json::from_str(body).unwrap();
        assert_eq!((payload["event"].as_str(), payload["data"]["title"].as_str()), (Some("task.created"), Some("Write docs")));
        assert!(payload["data"].get("description").is_none(), "no free text beyond what the event needs");
        let (status, attempts): (String, i32) = sqlx::query_as("SELECT status, attempts FROM webhook_deliveries").fetch_one(&pool).await.unwrap();
        assert_eq!((status.as_str(), attempts), ("delivered", 1));
    }

    #[sqlx::test]
    async fn a_failing_receiver_is_retried_then_given_up_and_the_webhook_switched_off(pool: PgPool) {
        let (url, srv) = server(500, 6).await;
        let (state, ws, hook) = setup(&pool, &url, &["event.created"]).await;
        sqlx::query("UPDATE workspace_webhooks SET failures = $2 WHERE id = $1").bind(hook).bind(DISABLE_AFTER - 1).execute(&pool).await.unwrap();
        dispatch(&state, ws, "event.created", serde_json::json!({ "id": Uuid::new_v4(), "summary": "Party" }));
        wait_for_deliveries(&pool, 1).await;
        for attempt in 1..=6 {
            process(&state).await.unwrap();
            let (status, attempts, err): (String, i32, Option<String>) = sqlx::query_as("SELECT status, attempts, error FROM webhook_deliveries").fetch_one(&pool).await.unwrap();
            assert_eq!(attempts, attempt);
            assert_eq!(err.as_deref(), Some("http_500"));
            if attempt < 6 {
                assert_eq!(status, "pending", "attempt {attempt} is retried later");
                // Pas avant l'échéance : un passage immédiat ne renvoie rien.
                process(&state).await.unwrap();
                let again: i32 = sqlx::query_scalar("SELECT attempts FROM webhook_deliveries").fetch_one(&pool).await.unwrap();
                assert_eq!(again, attempt);
                sqlx::query("UPDATE webhook_deliveries SET next_attempt_at = NOW() - INTERVAL '1 second'").execute(&pool).await.unwrap();
            } else {
                assert_eq!(status, "failed");
            }
        }
        assert_eq!(srv.await.unwrap().len(), 6);
        let active: bool = sqlx::query_scalar("SELECT active FROM workspace_webhooks WHERE id = $1").bind(hook).fetch_one(&pool).await.unwrap();
        assert!(!active, "ten abandoned deliveries in a row switch the webhook off");
    }

    #[sqlx::test]
    async fn a_task_is_reported_completed_once_and_the_test_ping_reaches_the_receiver(pool: PgPool) {
        let (url, srv) = server(200, 1).await;
        let (state, ws, hook) = setup(&pool, &url, &["task.completed", "task.updated"]).await;
        let id = Uuid::new_v4();
        let update = |done: bool| serde_json::json!({ "type": "TASK_UPDATED", "task": { "id": id, "title": "T", "is_completed": done } });
        from_realtime(&state, ws, &update(false));
        from_realtime(&state, ws, &update(true));
        from_realtime(&state, ws, &update(true));
        tokio::time::sleep(Duration::from_millis(300)).await;
        let completed: i64 = sqlx::query_scalar("SELECT count(*) FROM webhook_deliveries WHERE event = 'task.completed'").fetch_one(&pool).await.unwrap();
        assert_eq!(completed, 1);
        let updated: i64 = sqlx::query_scalar("SELECT count(*) FROM webhook_deliveries WHERE event = 'task.updated'").fetch_one(&pool).await.unwrap();
        assert_eq!(updated, 3);
        // Le test d'un administrateur passe même si « ping » n'est pas dans les événements choisis.
        enqueue(&pool, hook, ws, "ping", serde_json::json!({})).await.unwrap();
        sqlx::query("DELETE FROM webhook_deliveries WHERE event <> 'ping'").execute(&pool).await.unwrap();
        process(&state).await.unwrap();
        assert_eq!(srv.await.unwrap().len(), 1);
        let delivered: i64 = sqlx::query_scalar("SELECT count(*) FROM webhook_deliveries WHERE status = 'delivered' AND event = 'ping'").fetch_one(&pool).await.unwrap();
        assert_eq!(delivered, 1);
    }
}

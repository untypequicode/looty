//! Tunnel PC : jetons, point d'entrée des agents `looty-bridge`, réglages d'instance et audit (ADR 0020).
//!
//! Une personne crée un jeton de tunnel (lié à elle et à un espace, éventuellement à un projet),
//! lance l'agent sur son PC avec ce jeton, et l'agent ouvre une WebSocket sortante vers
//! `/api/tunnels/agent`. Le projet peut alors cibler « le tunnel de X » au lieu d'une URL de serveur
//! Jupyter (`jupyter.rs`). Le protocole de multiplexage est dans `tunnel_relay.rs` et
//! `docs/tunnel-protocol.md`.
//!
//! - Jeton : `lty_tun_` + 32 octets aléatoires, montré une seule fois, stocké haché (SHA-256),
//!   durée limitée (30 jours par défaut, 365 au plus), révocable. La révocation coupe la connexion.
//! - Réglages (Administration > Instance > Tunnels, `global_settings`) : activation (`LOOTY_TUNNELS` en valeur
//!   initiale), jetons par personne, durée de vie maximale, flux et débit par tunnel, types de service permis.
//!   Appliqués à la création, à la connexion, puis toutes les 15 secondes à chaque connexion ouverte
//!   (`revalidate`) ; la désactivation, elle, coupe tout de suite (`settings_changed`).
//! - Créer un jeton demande de confirmer (`ack_remote_code`) : les membres du projet peuvent exécuter du
//!   code sur le PC. `owner_only` réserve le tunnel à son propriétaire, appliqué par le relais.
//! - Audit : `tunnel_audit` (qui, quoi, quand), lisible par les administrateurs, conservation bornée.
//! - Seul le propriétaire du jeton l'attache à un projet qu'il peut modifier ; les autres membres
//!   l'utilisent selon les droits du projet (`jupyter.rs`), sauf tunnel `owner_only`.

use axum::{
    extract::{
        ws::{CloseFrame, Message as AxMsg, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get},
    Json, Router,
};
use base64::Engine;
use chrono::{DateTime, Utc};
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::access::{hash_token, AuthUser};
use crate::admin::GlobalSettings;
use crate::error::{ApiError, ApiResult};
use crate::projects::{project_for, Need};
use crate::state::AppState;
use crate::tunnel_relay::{self as relay, Ctl, Live, Out, Session};

pub const TOKEN_PREFIX: &str = "lty_tun_";
const DEFAULT_MAX_PER_USER: i32 = 20;
const MAX_DAYS: i32 = 365;
const DEFAULT_AUDIT_DAYS: i32 = 90;
/// Plancher du plafond de débit : en dessous, une trame de 64 Kio ferait attendre trop longtemps le relais.
const MIN_KBPS: i32 = 16;
const MAX_KBPS: i32 = 1_000_000;

/// Valeur initiale de l'interrupteur : `LOOTY_TUNNELS=on`, tant qu'aucun réglage n'est enregistré.
pub fn env_enabled() -> bool {
    std::env::var("LOOTY_TUNNELS").is_ok_and(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "on" | "1" | "true" | "yes"))
}

// ---------------------------------------------------------------------------
// Réglages d'instance
// ---------------------------------------------------------------------------

/// Réglages des tunnels, valeurs résolues.
#[derive(Debug, Clone, PartialEq)]
pub struct Policy {
    pub enabled: bool,
    pub max_per_user: i64,
    pub max_token_days: i64,
    pub max_streams: usize,
    /// Kio/s par tunnel, 0 = illimité.
    pub kbps: u64,
    pub kinds: Vec<String>,
    pub audit_days: i64,
}

impl Policy {
    pub fn of(s: &GlobalSettings) -> Policy {
        Policy {
            enabled: s.tunnels_enabled.unwrap_or_else(env_enabled),
            max_per_user: s.tunnels_max_per_user.unwrap_or(DEFAULT_MAX_PER_USER).into(),
            max_token_days: s.tunnels_max_token_days.unwrap_or(MAX_DAYS).into(),
            max_streams: s.tunnels_max_streams.map_or(relay::MAX_STREAMS, |n| n.clamp(1, 64) as usize),
            kbps: s.tunnels_max_kbps.unwrap_or(0).max(0) as u64,
            kinds: s.tunnels_kinds.clone().unwrap_or_else(|| relay::KINDS.iter().map(|k| k.to_string()).collect()),
            audit_days: s.tunnels_audit_days.unwrap_or(DEFAULT_AUDIT_DAYS).into(),
        }
    }

    pub fn live(&self) -> Live {
        Live { max_streams: self.max_streams, bytes_per_sec: self.kbps * 1024, kinds: self.kinds.clone() }
    }

    fn json(&self) -> Value {
        json!({ "max_per_user": self.max_per_user, "max_token_days": self.max_token_days, "max_streams": self.max_streams, "max_kbps": self.kbps, "kinds": self.kinds })
    }
}

/// Remplace les valeurs absentes par leurs valeurs par défaut (ce que l'administrateur voit et modifie).
pub fn resolve(s: &mut GlobalSettings) {
    let p = Policy::of(s);
    s.tunnels_enabled = Some(p.enabled);
    s.tunnels_max_per_user = Some(p.max_per_user as i32);
    s.tunnels_max_token_days = Some(p.max_token_days as i32);
    s.tunnels_max_streams = Some(p.max_streams as i32);
    s.tunnels_max_kbps = Some(p.kbps as i32);
    s.tunnels_kinds = Some(p.kinds);
    s.tunnels_audit_days = Some(p.audit_days as i32);
}

pub fn validate_settings(s: &GlobalSettings) -> ApiResult<()> {
    let bad = |c: &str| Err(ApiError::bad_request(c));
    if s.tunnels_max_per_user.is_some_and(|n| !(1..=100).contains(&n)) {
        return bad("invalid_tunnel_max");
    }
    if s.tunnels_max_token_days.is_some_and(|n| !(1..=MAX_DAYS).contains(&n)) {
        return bad("invalid_tunnel_days");
    }
    if s.tunnels_max_streams.is_some_and(|n| !(1..=64).contains(&n)) {
        return bad("invalid_tunnel_streams");
    }
    if s.tunnels_max_kbps.is_some_and(|n| n != 0 && !(MIN_KBPS..=MAX_KBPS).contains(&n)) {
        return bad("invalid_tunnel_rate");
    }
    if s.tunnels_kinds.as_ref().is_some_and(|k| k.len() > relay::KINDS.len() || k.iter().any(|k| !relay::KINDS.contains(&k.as_str()))) {
        return bad("invalid_tunnel_kinds");
    }
    if s.tunnels_audit_days.is_some_and(|n| !(1..=MAX_DAYS).contains(&n)) {
        return bad("invalid_tunnel_audit_days");
    }
    Ok(())
}

pub async fn policy(pool: &PgPool) -> ApiResult<Policy> {
    Ok(Policy::of(&crate::admin::tunnel_settings(pool).await?))
}

/// Pourquoi une connexion ouverte ne respecte plus les réglages : code de fermeture et raison.
fn violation(p: &Policy, s: &Session) -> Option<(u16, &'static str)> {
    if !p.enabled {
        return Some((4003, "tunnels_disabled"));
    }
    if s.services().iter().any(|d| !p.kinds.contains(&d.kind)) {
        return Some((4403, "tunnel_kind_not_allowed"));
    }
    None
}

/// Après un changement de réglages : les connexions ouvertes s'y plient tout de suite (désactivation
/// comprise), et le changement est journalisé.
pub async fn settings_changed(pool: &PgPool, actor: Uuid, before: &Policy, after: &Policy) {
    settings_changed_for(pool, actor, before, after, relay::sessions()).await
}

async fn settings_changed_for(pool: &PgPool, actor: Uuid, before: &Policy, after: &Policy, sessions: Vec<Arc<Session>>) {
    for s in sessions {
        match violation(after, &s) {
            Some((code, why)) => s.close(code, why),
            None => s.apply(&after.live()),
        }
    }
    if before != after {
        audit(pool, "settings_changed", None, Some(actor), json!({ "before": before.json_all(), "after": after.json_all() })).await;
    }
}

impl Policy {
    fn json_all(&self) -> Value {
        let mut v = self.json();
        v["enabled"] = json!(self.enabled);
        v["audit_days"] = json!(self.audit_days);
        v
    }
}

// ---------------------------------------------------------------------------
// Audit
// ---------------------------------------------------------------------------

/// Écrit un événement d'audit. Jamais de jeton ni de secret dans `details`. Une erreur d'écriture
/// n'empêche pas l'action (elle est journalisée).
pub async fn audit(pool: &PgPool, event: &str, t: Option<&TokenRow>, actor: Option<Uuid>, details: Value) {
    let r = sqlx::query(
        "INSERT INTO tunnel_audit (event, tunnel_id, tunnel_name, owner_id, actor_id, workspace_id, project_id, details) VALUES ($1, $2, $3, $4, $5, $6, $7, $8::jsonb)",
    )
    .bind(event)
    .bind(t.map(|t| t.id))
    .bind(t.map(|t| t.name.clone()))
    .bind(t.map(|t| t.user_id))
    .bind(actor)
    .bind(t.map(|t| t.workspace_id))
    .bind(t.and_then(|t| t.project_id))
    .bind(details.to_string())
    .execute(pool)
    .await;
    if let Err(e) = r {
        tracing::warn!(error = %e, event, "tunnel audit write failed");
        return;
    }
    // Conservation bornée : on purge à chaque événement rare (pas à chaque connexion du relais).
    if matches!(event, "created" | "connected" | "settings_changed") {
        if let Ok(p) = policy(pool).await {
            let _ = sqlx::query("DELETE FROM tunnel_audit WHERE at < NOW() - make_interval(days => $1::int)").bind(p.audit_days as i32).execute(pool).await;
        }
    }
}

#[derive(Deserialize)]
struct AuditQuery {
    limit: Option<i64>,
    before: Option<i64>,
    tunnel: Option<Uuid>,
}

/// Journal d'audit, réservé aux administrateurs : les plus récents d'abord, pagination par `before`.
async fn audit_list(State(state): State<AppState>, user: AuthUser, Query(q): Query<AuditQuery>) -> ApiResult<Json<Value>> {
    crate::admin::require_super_admin(&user)?;
    let limit = q.limit.unwrap_or(100).clamp(1, 500);
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT row_to_json(r)::text FROM (
            SELECT a.id, a.at, a.event, a.tunnel_id, a.tunnel_name, a.owner_id, o.name AS owner_name, o.email AS owner_email,
                   a.actor_id, x.name AS actor_name, a.workspace_id, a.project_id, a.details
              FROM tunnel_audit a LEFT JOIN users o ON o.id = a.owner_id LEFT JOIN users x ON x.id = a.actor_id
             WHERE ($1::bigint IS NULL OR a.id < $1) AND ($2::uuid IS NULL OR a.tunnel_id = $2)
             ORDER BY a.id DESC LIMIT $3) r",
    )
    .bind(q.before)
    .bind(q.tunnel)
    .bind(limit)
    .fetch_all(&state.pool)
    .await?;
    let events: Vec<Value> = rows.iter().filter_map(|r| serde_json::from_str(r).ok()).collect();
    Ok(Json(json!({ "retention_days": policy(&state.pool).await?.audit_days, "events": events })))
}

// ---------------------------------------------------------------------------
// Jetons
// ---------------------------------------------------------------------------

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/agent", get(agent))
        .route("/audit", get(audit_list))
        .route("/{id}", delete(revoke).patch(update))
        .route("/{id}/status", get(status))
}

async fn available(state: &AppState) -> ApiResult<Policy> {
    if state.demo {
        return Err(ApiError::DemoUnavailable);
    }
    let p = policy(&state.pool).await?;
    if !p.enabled {
        return Err(ApiError::bad_request("tunnels_disabled"));
    }
    Ok(p)
}

#[derive(sqlx::FromRow)]
pub struct TokenRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub workspace_id: Uuid,
    pub project_id: Option<Uuid>,
    pub name: String,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub last_connected_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub owner_only: bool,
}

const COLS: &str = "id, user_id, workspace_id, project_id, name, expires_at, revoked_at, last_connected_at, created_at, owner_only";

impl TokenRow {
    pub fn usable(&self) -> bool {
        self.revoked_at.is_none() && self.expires_at > Utc::now()
    }
}

pub async fn token_row(state: &AppState, id: Uuid) -> ApiResult<Option<TokenRow>> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT {COLS} FROM tunnel_tokens WHERE id = $1"))).bind(id).fetch_optional(&state.pool).await?)
}

/// Une personne peut-elle se servir de ce tunnel ? Faux pour un tunnel réservé à son propriétaire.
pub fn may_use(t: &TokenRow, caller: Uuid) -> bool {
    !t.owner_only || t.user_id == caller
}

/// État en direct d'un jeton, sans rien de secret.
pub fn live_json(id: Uuid) -> Value {
    match relay::connected(id) {
        None => json!({ "connected": false }),
        Some(s) => json!({
            "connected": true,
            "latency_ms": s.latency_ms(),
            "since_secs": s.since.elapsed().as_secs(),
            "agent": s.agent,
            "services": s.services(),
            "streams": s.open_streams(),
            "bytes_in": s.bytes_in.load(std::sync::atomic::Ordering::Relaxed),
            "bytes_out": s.bytes_out.load(std::sync::atomic::Ordering::Relaxed),
        }),
    }
}

/// Jeton pour l'API : jamais le jeton lui-même, ni son empreinte (seule la création renvoie `token`).
fn token_json(t: &TokenRow) -> Value {
    json!({
        "id": t.id,
        "name": t.name,
        "workspace_id": t.workspace_id,
        "project_id": t.project_id,
        "expires_at": t.expires_at,
        "revoked": t.revoked_at.is_some(),
        "last_connected_at": t.last_connected_at,
        "created_at": t.created_at,
        "owner_only": t.owner_only,
        "status": live_json(t.id),
    })
}

async fn list(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<Value>> {
    let p = policy(&state.pool).await?;
    if state.demo || !p.enabled {
        return Ok(Json(json!({ "enabled": false, "tunnels": [] })));
    }
    let rows: Vec<TokenRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT {COLS} FROM tunnel_tokens WHERE user_id = $1 AND revoked_at IS NULL ORDER BY created_at DESC")))
        .bind(user.id)
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(json!({ "enabled": true, "policy": p.json(), "tunnels": rows.iter().map(token_json).collect::<Vec<_>>() })))
}

#[derive(Deserialize)]
struct CreateBody {
    name: String,
    workspace_id: Uuid,
    project_id: Option<Uuid>,
    expires_in_days: Option<i64>,
    /// Confirmation explicite : les membres du projet peuvent exécuter du code sur le PC.
    #[serde(default)]
    ack_remote_code: bool,
    /// Réservé à son propriétaire : les membres du projet ne peuvent pas s'en servir.
    #[serde(default)]
    owner_only: bool,
}

async fn create(State(state): State<AppState>, user: AuthUser, Json(b): Json<CreateBody>) -> ApiResult<Json<Value>> {
    let policy = available(&state).await?;
    if !b.ack_remote_code {
        return Err(ApiError::bad_request("tunnel_ack_required"));
    }
    let name = b.name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(ApiError::bad_request("invalid_value"));
    }
    let days = b.expires_in_days.unwrap_or_else(|| 30.min(policy.max_token_days));
    if days < 1 {
        return Err(ApiError::bad_request("invalid_value"));
    }
    if days > policy.max_token_days {
        return Err(ApiError::bad_request("tunnel_lifetime_too_long"));
    }
    crate::access::require_member(&state.pool, &user, b.workspace_id).await?;
    if let Some(p) = b.project_id {
        let (project, _) = project_for(&state, &user, p, Need::Write).await?;
        if project.workspace_id != b.workspace_id {
            return Err(ApiError::bad_request("tunnel_wrong_workspace"));
        }
    }
    let active: i64 = sqlx::query_scalar("SELECT count(*) FROM tunnel_tokens WHERE user_id = $1 AND revoked_at IS NULL AND expires_at > NOW()").bind(user.id).fetch_one(&state.pool).await?;
    if active >= policy.max_per_user {
        return Err(ApiError::bad_request("tunnel_too_many_tokens"));
    }
    let raw: [u8; 32] = rand::random();
    let token = format!("{TOKEN_PREFIX}{}", base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw));
    let row: TokenRow = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "INSERT INTO tunnel_tokens (user_id, workspace_id, project_id, name, token_hash, expires_at, owner_only)
         VALUES ($1, $2, $3, $4, $5, NOW() + make_interval(days => $6::int), $7) RETURNING {COLS}"
    )))
    .bind(user.id)
    .bind(b.workspace_id)
    .bind(b.project_id)
    .bind(name)
    .bind(hash_token(&token))
    .bind(days as i32)
    .bind(b.owner_only)
    .fetch_one(&state.pool)
    .await?;
    tracing::info!(tunnel = %row.id, user = %user.id, "tunnel token created");
    audit(&state.pool, "created", Some(&row), Some(user.id), json!({ "days": days, "owner_only": row.owner_only, "ack_remote_code": true })).await;
    let mut v = token_json(&row);
    // Seule fois où le jeton est visible.
    v["token"] = json!(token);
    Ok(Json(v))
}

async fn revoke(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<StatusCode> {
    // Le propriétaire révoque son jeton ; un administrateur peut révoquer n'importe lequel.
    let row: Option<TokenRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "UPDATE tunnel_tokens SET revoked_at = NOW() WHERE id = $1 AND (user_id = $2 OR $3) AND revoked_at IS NULL RETURNING {COLS}"
    )))
    .bind(id)
    .bind(user.id)
    .bind(user.is_super_admin)
    .fetch_optional(&state.pool)
    .await?;
    let row = row.ok_or(ApiError::NotFound)?;
    relay::disconnect(id, 4001, "revoked");
    tracing::info!(tunnel = %id, user = %user.id, "tunnel token revoked");
    audit(&state.pool, "revoked", Some(&row), Some(user.id), json!({ "by_admin": row.user_id != user.id })).await;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct UpdateBody {
    owner_only: bool,
}

/// Change l'option « réservé à son propriétaire » d'un tunnel, y compris pendant qu'il est connecté.
async fn update(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(b): Json<UpdateBody>) -> ApiResult<Json<Value>> {
    let row: TokenRow = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "UPDATE tunnel_tokens SET owner_only = $3 WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL RETURNING {COLS}"
    )))
    .bind(id)
    .bind(user.id)
    .bind(b.owner_only)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(ApiError::NotFound)?;
    if let Some(s) = relay::connected(id) {
        s.set_owner_only(row.owner_only);
    }
    audit(&state.pool, "owner_only_changed", Some(&row), Some(user.id), json!({ "owner_only": row.owner_only })).await;
    Ok(Json(token_json(&row)))
}

async fn status(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    let t = token_row(&state, id).await?.filter(|t| t.user_id == user.id).ok_or(ApiError::NotFound)?;
    Ok(Json(token_json(&t)))
}

// ---------------------------------------------------------------------------
// Point d'entrée des agents
// ---------------------------------------------------------------------------

fn refuse(status: StatusCode, code: &str) -> Response {
    (status, Json(json!({ "error": code }))).into_response()
}

#[derive(sqlx::FromRow)]
struct AuthRow {
    #[sqlx(flatten)]
    t: TokenRow,
    is_suspended: bool,
}

/// Jeton valide pour une connexion : non révoqué, non expiré, compte non suspendu, durée de vie
/// dans la limite actuelle de l'instance.
pub async fn authenticate(state: &AppState, raw: &str) -> Result<TokenRow, &'static str> {
    if !raw.starts_with(TOKEN_PREFIX) || raw.len() > 128 {
        return Err("tunnel_token_invalid");
    }
    let row: Option<AuthRow> = sqlx::query_as(
        "SELECT t.id, t.user_id, t.workspace_id, t.project_id, t.name, t.expires_at, t.revoked_at, t.last_connected_at, t.created_at, t.owner_only, u.is_suspended
           FROM tunnel_tokens t JOIN users u ON u.id = t.user_id WHERE t.token_hash = $1",
    )
    .bind(hash_token(raw))
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| "internal_error")?;
    let Some(AuthRow { t, is_suspended }) = row else { return Err("tunnel_token_invalid") };
    if t.revoked_at.is_some() {
        return Err("tunnel_token_revoked");
    }
    if t.expires_at <= Utc::now() {
        return Err("tunnel_token_expired");
    }
    if is_suspended {
        return Err("tunnel_token_invalid");
    }
    let max = policy(&state.pool).await.map_err(|_| "internal_error")?.max_token_days;
    if (t.expires_at - t.created_at).num_days() > max {
        return Err("tunnel_token_lifetime");
    }
    Ok(t)
}

async fn agent(ws: WebSocketUpgrade, State(state): State<AppState>, headers: HeaderMap) -> Response {
    if state.demo {
        return refuse(StatusCode::NOT_FOUND, "not_found");
    }
    let Ok(policy) = policy(&state.pool).await else { return refuse(StatusCode::INTERNAL_SERVER_ERROR, "internal_error") };
    if !policy.enabled {
        return refuse(StatusCode::FORBIDDEN, "tunnels_disabled");
    }
    let raw = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer ")).map(str::trim).unwrap_or("");
    let token = match authenticate(&state, raw).await {
        Ok(t) => t,
        Err("internal_error") => return refuse(StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
        Err(code) => {
            tracing::warn!(reason = code, "tunnel agent refused");
            return refuse(StatusCode::UNAUTHORIZED, code);
        }
    };
    let _ = sqlx::query("UPDATE tunnel_tokens SET last_connected_at = NOW() WHERE id = $1").bind(token.id).execute(&state.pool).await;
    ws.protocols([relay::SUBPROTOCOL]).max_message_size(relay::MAX_FRAME + 64).max_frame_size(relay::MAX_FRAME + 64).on_upgrade(move |socket| serve(socket, token, state, policy))
}

async fn close_with(socket: &mut WebSocket, code: u16, reason: &str) {
    let _ = socket.send(AxMsg::Close(Some(CloseFrame { code, reason: reason.to_string().into() }))).await;
}

async fn serve(mut socket: WebSocket, token: TokenRow, state: AppState, policy: Policy) {
    // Poignée de main : le premier message est `hello`, dans les 10 secondes.
    let first = tokio::time::timeout(relay::HELLO_TIMEOUT, async {
        loop {
            match socket.recv().await {
                Some(Ok(AxMsg::Text(t))) => return Some(t.as_str().to_string()),
                Some(Ok(AxMsg::Ping(_) | AxMsg::Pong(_))) => continue,
                _ => return None,
            }
        }
    })
    .await;
    let Ok(Some(first)) = first else {
        close_with(&mut socket, 4400, "hello_expected").await;
        return;
    };
    let (agent, services) = match serde_json::from_str::<Ctl>(&first) {
        Ok(Ctl::Hello { v, agent, services }) => {
            if v != relay::PROTOCOL {
                let bye = serde_json::to_string(&Ctl::Bye { code: "unsupported_version".into(), message: Some(format!("supported: {}", relay::PROTOCOL)) }).unwrap_or_default();
                let _ = socket.send(AxMsg::Text(bye.into())).await;
                close_with(&mut socket, 4426, "unsupported_version").await;
                return;
            }
            if !relay::validate_services(&services) {
                close_with(&mut socket, 4400, "bad_services").await;
                return;
            }
            // Types de service permis par l'administrateur : refus clair dès la poignée de main.
            if services.iter().any(|s| !policy.kinds.contains(&s.kind)) {
                let bye = serde_json::to_string(&Ctl::Bye { code: "tunnel_kind_not_allowed".into(), message: Some(format!("allowed: {}", policy.kinds.join(","))) }).unwrap_or_default();
                let _ = socket.send(AxMsg::Text(bye.into())).await;
                close_with(&mut socket, 4403, "tunnel_kind_not_allowed").await;
                audit(&state.pool, "refused", Some(&token), None, json!({ "reason": "tunnel_kind_not_allowed" })).await;
                return;
            }
            (agent.chars().filter(|c| !c.is_control()).take(64).collect::<String>(), services)
        }
        _ => {
            close_with(&mut socket, 4400, "hello_expected").await;
            return;
        }
    };

    let (mut sink, mut stream) = socket.split();
    let (out_tx, mut out_rx) = mpsc::channel::<Out>(256);
    let session = Session::new(token.id, token.user_id, agent, token.expires_at, services, out_tx);
    session.apply(&policy.live());
    session.set_owner_only(token.owner_only);
    let writer = tokio::spawn(async move {
        while let Some(o) = out_rx.recv().await {
            let m = match o {
                Out::Text(s) => AxMsg::Text(s.into()),
                Out::Binary(b) => AxMsg::Binary(b.into()),
                Out::Close(code, reason) => {
                    let _ = sink.send(AxMsg::Close(Some(CloseFrame { code, reason: reason.into() }))).await;
                    break;
                }
            };
            if sink.send(m).await.is_err() {
                break;
            }
        }
    });
    relay::register(session.clone());
    tracing::info!(tunnel = %token.id, user = %token.user_id, agent = %session.agent, services = session.services().len(), "tunnel connected");
    audit(&state.pool, "connected", Some(&token), Some(token.user_id), json!({ "agent": session.agent, "services": session.services() })).await;
    let _ = session.welcome().await;
    run(&state, &session, &mut stream).await;
    session.shutdown();
    relay::unregister(&session);
    // Laisse partir le `bye` et la fermeture avant de couper l'écriture.
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), writer).await;
    let (bytes_in, bytes_out) = (session.bytes_in.load(std::sync::atomic::Ordering::Relaxed), session.bytes_out.load(std::sync::atomic::Ordering::Relaxed));
    tracing::info!(tunnel = %token.id, user = %token.user_id, secs = session.since.elapsed().as_secs(), bytes_in, bytes_out, "tunnel disconnected");
    audit(&state.pool, "disconnected", Some(&token), Some(token.user_id), json!({ "secs": session.since.elapsed().as_secs(), "bytes_in": bytes_in, "bytes_out": bytes_out, "reason": session.reason() })).await;
}

/// Revalide une connexion ouverte contre la base et les réglages d'instance : jeton révoqué ou expiré,
/// compte suspendu, tunnels désactivés ou type de service retiré ferment la connexion ; sinon les
/// limites actuelles (flux, débit, types, propriétaire seul) sont appliquées. Appelée toutes les 15 secondes.
pub async fn revalidate(state: &AppState, s: &Arc<Session>) {
    let Ok(p) = policy(&state.pool).await else { return };
    let row: Result<Option<(Option<DateTime<Utc>>, DateTime<Utc>, DateTime<Utc>, bool, bool)>, _> = sqlx::query_as(
        "SELECT t.revoked_at, t.expires_at, t.created_at, t.owner_only, u.is_suspended FROM tunnel_tokens t JOIN users u ON u.id = t.user_id WHERE t.id = $1",
    )
    .bind(s.id)
    .fetch_optional(&state.pool)
    .await;
    let Ok(row) = row else { return };
    let Some((revoked, expires, created, owner_only, suspended)) = row else { return s.close(4001, "revoked") };
    if revoked.is_some() {
        return s.close(4001, "revoked");
    }
    if expires <= Utc::now() {
        return s.close(4002, "expired");
    }
    if suspended {
        return s.close(4004, "suspended");
    }
    if (expires - created).num_days() > p.max_token_days {
        return s.close(4005, "tunnel_token_lifetime");
    }
    if let Some((code, why)) = violation(&p, s) {
        return s.close(code, why);
    }
    s.apply(&p.live());
    s.set_owner_only(owner_only);
}

async fn run(state: &AppState, session: &Arc<Session>, stream: &mut (impl futures::Stream<Item = Result<AxMsg, axum::Error>> + Unpin)) {
    let mut tick = tokio::time::interval(relay::PING_EVERY);
    tick.tick().await;
    loop {
        tokio::select! {
            msg = stream.next() => {
                let verdict = match msg {
                    Some(Ok(AxMsg::Text(t))) => session.on_text(t.as_str()).await,
                    Some(Ok(AxMsg::Binary(b))) => {
                        let verdict = session.on_binary(&b);
                        // Plafond de débit : on cesse de lire le temps qu'il faut, la contre-pression fait le reste.
                        session.pace(b.len()).await;
                        verdict
                    }
                    Some(Ok(AxMsg::Ping(_) | AxMsg::Pong(_))) => Ok(()),
                    _ => break,
                };
                if let Err(reason) = verdict {
                    tracing::warn!(tunnel = %session.id, reason, "tunnel protocol violation");
                    session.close(1002, reason);
                    break;
                }
            }
            _ = tick.tick() => {
                if session.is_closed() {
                    break;
                }
                revalidate(state, session).await;
                if session.is_closed() {
                    break;
                }
                if session.idle_for() > relay::DEAD_AFTER {
                    tracing::warn!(tunnel = %session.id, "tunnel agent silent, closing");
                    session.close(4008, "timeout");
                    break;
                }
                if session.ping().await.is_err() {
                    break;
                }
            }
            _ = session.closed_notify.notified() => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tunnel_relay::ServiceDecl;
    use tokio_tungstenite::tungstenite::{self, client::IntoClientRequest, Message as T};

    async fn seed(pool: &sqlx::PgPool) -> (Uuid, Uuid) {
        let user: Uuid = sqlx::query_scalar("INSERT INTO users (email, name) VALUES ('a@example.org', 'a') RETURNING id").fetch_one(pool).await.unwrap();
        let ws: Uuid = sqlx::query_scalar("INSERT INTO workspaces (name) VALUES ('Team') RETURNING id").fetch_one(pool).await.unwrap();
        (user, ws)
    }

    async fn add_token(pool: &sqlx::PgPool, user: Uuid, ws: Uuid, raw: &str, days: i32) -> Uuid {
        sqlx::query_scalar(
            "INSERT INTO tunnel_tokens (user_id, workspace_id, name, token_hash, expires_at) VALUES ($1, $2, 't', $3, NOW() + make_interval(days => $4)) RETURNING id",
        )
        .bind(user)
        .bind(ws)
        .bind(hash_token(raw))
        .bind(days)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    #[sqlx::test]
    async fn tokens_authenticate_until_revoked_or_expired(pool: sqlx::PgPool) {
        let state = AppState::new(pool.clone(), "http://localhost".into());
        let (user, ws) = seed(&pool).await;
        let ok = format!("{TOKEN_PREFIX}good");
        let id = add_token(&pool, user, ws, &ok, 1).await;
        assert_eq!(authenticate(&state, &ok).await.unwrap().id, id);
        // Le jeton est stocké haché.
        let stored: String = sqlx::query_scalar("SELECT token_hash FROM tunnel_tokens WHERE id = $1").bind(id).fetch_one(&pool).await.unwrap();
        assert_ne!(stored, ok);
        assert_eq!(authenticate(&state, "lty_tun_other").await.err(), Some("tunnel_token_invalid"));
        assert_eq!(authenticate(&state, "looty_abc").await.err(), Some("tunnel_token_invalid"));
        sqlx::query("UPDATE tunnel_tokens SET revoked_at = NOW() WHERE id = $1").bind(id).execute(&pool).await.unwrap();
        assert_eq!(authenticate(&state, &ok).await.err(), Some("tunnel_token_revoked"));
        let old = format!("{TOKEN_PREFIX}old");
        add_token(&pool, user, ws, &old, -1).await;
        assert_eq!(authenticate(&state, &old).await.err(), Some("tunnel_token_expired"));
        let sus = format!("{TOKEN_PREFIX}sus");
        add_token(&pool, user, ws, &sus, 1).await;
        sqlx::query("UPDATE users SET is_suspended = TRUE WHERE id = $1").bind(user).execute(&pool).await.unwrap();
        assert_eq!(authenticate(&state, &sus).await.err(), Some("tunnel_token_invalid"));
    }

    /// Serveur réel sur un port local : le faux agent est un client WebSocket.
    async fn server(pool: sqlx::PgPool) -> String {
        let state = AppState::new(pool, "http://localhost".into());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, crate::app(state)).await.unwrap() });
        format!("ws://{addr}/api/tunnels/agent")
    }

    fn request(url: &str, token: Option<&str>) -> tungstenite::handshake::client::Request {
        let mut r = url.into_client_request().unwrap();
        if let Some(t) = token {
            r.headers_mut().insert("authorization", format!("Bearer {t}").parse().unwrap());
        }
        r.headers_mut().insert("sec-websocket-protocol", relay::SUBPROTOCOL.parse().unwrap());
        r
    }

    async fn text(ws: &mut (impl futures::Stream<Item = Result<T, tungstenite::Error>> + Unpin)) -> Option<Ctl> {
        loop {
            match tokio::time::timeout(std::time::Duration::from_secs(5), ws.next()).await.ok()?? {
                Ok(T::Text(t)) => return serde_json::from_str(t.as_str()).ok(),
                Ok(T::Close(_)) | Err(_) => return None,
                _ => {}
            }
        }
    }

    #[sqlx::test]
    async fn the_agent_endpoint_relays_http_and_websocket_and_cleans_up(pool: sqlx::PgPool) {
        std::env::set_var("LOOTY_TUNNELS", "on");
        let (user, ws) = seed(&pool).await;
        let raw = format!("{TOKEN_PREFIX}relay");
        let id = add_token(&pool, user, ws, &raw, 1).await;
        let url = server(pool.clone()).await;

        // Sans jeton, jeton inconnu : refus avant l'upgrade.
        for t in [None, Some("lty_tun_nope")] {
            let e = tokio_tungstenite::connect_async(request(&url, t)).await.err().unwrap();
            assert!(matches!(e, tungstenite::Error::Http(r) if r.status() == 401));
        }
        let (mut agent, res) = tokio_tungstenite::connect_async(request(&url, Some(&raw))).await.unwrap();
        assert_eq!(res.headers().get("sec-websocket-protocol").unwrap(), relay::SUBPROTOCOL);
        let hello = Ctl::Hello { v: 1, agent: "fake/0".into(), services: vec![ServiceDecl { name: "pc".into(), kind: "jupyter".into() }] };
        agent.send(T::text(serde_json::to_string(&hello).unwrap())).await.unwrap();
        let Some(Ctl::Welcome { v, limits, .. }) = text(&mut agent).await else { panic!("welcome expected") };
        assert_eq!((v, limits.max_streams), (1, relay::MAX_STREAMS));
        assert!(relay::connected(id).is_some());
        assert_eq!(live_json(id)["connected"], true);

        // Relais HTTP : l'instance appelle, le faux agent répond.
        let session = relay::connected(id).unwrap();
        let s2 = session.clone();
        let call = tokio::spawn(async move { relay::http(&s2, "pc", "jupyter", "GET", "/api/kernelspecs", vec![("authorization".into(), "token t".into())], None).await });
        let Some(Ctl::Open { id: sid, .. }) = text(&mut agent).await else { panic!("open expected") };
        let head = serde_json::to_string(&Ctl::Head { id: sid, status: 200, headers: vec![] }).unwrap();
        agent.send(T::text(head)).await.unwrap();
        let mut frame = sid.to_be_bytes().to_vec();
        frame.push(0);
        frame.extend_from_slice(b"{\"ok\":true}");
        agent.send(T::binary(frame)).await.unwrap();
        agent.send(T::text(serde_json::to_string(&Ctl::End { id: sid }).unwrap())).await.unwrap();
        let reply = call.await.unwrap().unwrap();
        assert_eq!((reply.status, reply.body.as_slice()), (200, b"{\"ok\":true}".as_slice()));

        // Relais WebSocket : message de l'instance vers l'agent, écho en retour.
        let s3 = session.clone();
        let open = tokio::spawn(async move { relay::ws_open(&s3, "pc", "jupyter", "/api/kernels/k/channels", vec![]).await });
        let wid = loop {
            match text(&mut agent).await {
                Some(Ctl::Open { id, .. }) => break id,
                Some(_) => continue,
                None => panic!("open expected"),
            }
        };
        agent.send(T::text(serde_json::to_string(&Ctl::Head { id: wid, status: 101, headers: vec![] }).unwrap())).await.unwrap();
        let mut stream = open.await.unwrap().unwrap();
        stream.tx.send_message(true, b"ping").await.unwrap();
        loop {
            match agent.next().await.unwrap().unwrap() {
                T::Binary(b) => {
                    assert_eq!(&b[5..], b"ping");
                    break;
                }
                _ => {}
            }
        }
        let mut echo = wid.to_be_bytes().to_vec();
        echo.extend_from_slice(&[relay::FLAG_TEXT]);
        echo.extend_from_slice(b"pong");
        agent.send(T::binary(echo)).await.unwrap();
        assert_eq!(stream.rx.recv_message().await.unwrap().unwrap().data, b"pong");

        // Déconnexion de l'agent : flux échoués, registre nettoyé.
        drop(agent);
        assert!(matches!(stream.rx.recv_message().await, Some(Err(relay::TunnelError::Closed))));
        for _ in 0..50 {
            if relay::connected(id).is_none() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        assert!(relay::connected(id).is_none());
        assert_eq!(session.open_streams(), 0);
    }

    #[sqlx::test]
    async fn the_handshake_rejects_bad_versions_and_revocation_cuts_the_link(pool: sqlx::PgPool) {
        std::env::set_var("LOOTY_TUNNELS", "on");
        let (user, ws) = seed(&pool).await;
        let raw = format!("{TOKEN_PREFIX}hs");
        let id = add_token(&pool, user, ws, &raw, 1).await;
        let url = server(pool.clone()).await;

        let (mut agent, _) = tokio_tungstenite::connect_async(request(&url, Some(&raw))).await.unwrap();
        agent.send(T::text(serde_json::to_string(&Ctl::Hello { v: 9, agent: "x".into(), services: vec![] }).unwrap())).await.unwrap();
        assert!(matches!(text(&mut agent).await, Some(Ctl::Bye { code, .. }) if code == "unsupported_version"));

        let (mut agent, _) = tokio_tungstenite::connect_async(request(&url, Some(&raw))).await.unwrap();
        agent.send(T::text(serde_json::to_string(&Ctl::Hello { v: 1, agent: "x".into(), services: vec![] }).unwrap())).await.unwrap();
        assert!(matches!(text(&mut agent).await, Some(Ctl::Welcome { .. })));
        // Révocation : bye, fermeture, puis le jeton ne passe plus.
        sqlx::query("UPDATE tunnel_tokens SET revoked_at = NOW() WHERE id = $1").bind(id).execute(&pool).await.unwrap();
        relay::disconnect(id, 4001, "revoked");
        assert!(matches!(text(&mut agent).await, Some(Ctl::Bye { code, .. }) if code == "revoked"));
        let e = tokio_tungstenite::connect_async(request(&url, Some(&raw))).await.err().unwrap();
        assert!(matches!(e, tungstenite::Error::Http(r) if r.status() == 401));
    }

    async fn set(pool: &sqlx::PgPool, assignments: &str) {
        sqlx::query("INSERT INTO global_settings (id, app_name) VALUES (1, 'Looty') ON CONFLICT (id) DO NOTHING").execute(pool).await.unwrap();
        sqlx::query(sqlx::AssertSqlSafe(format!("UPDATE global_settings SET {assignments} WHERE id = 1"))).execute(pool).await.unwrap();
    }

    async fn member(pool: &sqlx::PgPool, ws: Uuid, email: &str) -> AuthUser {
        let id: Uuid = sqlx::query_scalar("INSERT INTO users (email, name) VALUES ($1, 'm') RETURNING id").bind(email).fetch_one(pool).await.unwrap();
        sqlx::query("INSERT INTO workspace_members (workspace_id, user_id, role) VALUES ($1, $2, 'owner')").bind(ws).bind(id.to_string()).execute(pool).await.unwrap();
        AuthUser { id, is_super_admin: false }
    }

    fn body(ws: Uuid, extra: Value) -> CreateBody {
        let mut b = json!({ "name": "pc", "workspace_id": ws, "ack_remote_code": true });
        b.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        serde_json::from_value(b).unwrap()
    }

    fn code(e: ApiError) -> String {
        match e {
            ApiError::BadRequest(c) => c,
            other => format!("{other:?}"),
        }
    }

    #[sqlx::test]
    async fn creation_needs_confirmation_and_respects_the_admin_limits_and_never_leaks_the_token(pool: sqlx::PgPool) {
        let state = AppState::new(pool.clone(), "http://localhost".into());
        let (_, ws) = seed(&pool).await;
        let ana = member(&pool, ws, "ana@example.org").await;
        set(&pool, "tunnels_enabled = FALSE").await;
        let e = create(State(state.clone()), ana, Json(body(ws, json!({})))).await.err().unwrap();
        assert_eq!(code(e), "tunnels_disabled");
        set(&pool, "tunnels_enabled = TRUE, tunnels_max_per_user = 2, tunnels_max_token_days = 10").await;
        let e = create(State(state.clone()), ana, Json(body(ws, json!({ "ack_remote_code": false })))).await.err().unwrap();
        assert_eq!(code(e), "tunnel_ack_required");
        let e = create(State(state.clone()), ana, Json(body(ws, json!({ "expires_in_days": 11 })))).await.err().unwrap();
        assert_eq!(code(e), "tunnel_lifetime_too_long");
        let Json(v) = create(State(state.clone()), ana, Json(body(ws, json!({ "owner_only": true })))).await.unwrap();
        let token = v["token"].as_str().unwrap().to_string();
        assert!(token.starts_with(TOKEN_PREFIX));
        assert_eq!(v["owner_only"], true);
        let exp: DateTime<Utc> = serde_json::from_value(v["expires_at"].clone()).unwrap();
        assert!((exp - Utc::now()).num_days() <= 10);
        // Jamais renvoyé ensuite : ni liste, ni statut, ni audit.
        let Json(l) = list(State(state.clone()), ana).await.unwrap();
        assert!(!l.to_string().contains(&token) && l["tunnels"][0].get("token").is_none());
        let id: Uuid = serde_json::from_value(v["id"].clone()).unwrap();
        let Json(s) = status(State(state.clone()), ana, Path(id)).await.unwrap();
        assert!(!s.to_string().contains(&token));
        let audit_text: String = sqlx::query_scalar("SELECT COALESCE(string_agg(details::text || event, ' '), '') FROM tunnel_audit").fetch_one(&pool).await.unwrap();
        assert!(!audit_text.contains(&token) && audit_text.contains("created"));
        create(State(state.clone()), ana, Json(body(ws, json!({})))).await.unwrap();
        let e = create(State(state.clone()), ana, Json(body(ws, json!({})))).await.err().unwrap();
        assert_eq!(code(e), "tunnel_too_many_tokens");
        // Maximum de durée abaissé : un jeton plus long ne se connecte plus.
        set(&pool, "tunnels_max_token_days = 3").await;
        assert_eq!(authenticate(&state, &token).await.err(), Some("tunnel_token_lifetime"));
    }

    #[test]
    fn settings_are_validated() {
        let base = json!({ "app_name": "x", "favicon_url": null, "allow_public_signups": true, "umami_website_id": null, "umami_script_url": null });
        let with = |k: &str, v: Value| {
            let mut b = base.clone();
            b[k] = v;
            serde_json::from_value::<GlobalSettings>(b).unwrap()
        };
        assert!(validate_settings(&with("tunnels_enabled", json!(true))).is_ok());
        for (k, v) in [
            ("tunnels_max_per_user", json!(0)),
            ("tunnels_max_token_days", json!(400)),
            ("tunnels_max_streams", json!(65)),
            ("tunnels_max_kbps", json!(5)),
            ("tunnels_audit_days", json!(0)),
            ("tunnels_kinds", json!(["jupyter", "rm"])),
        ] {
            assert!(validate_settings(&with(k, v)).is_err(), "{k}");
        }
        assert!(validate_settings(&with("tunnels_kinds", json!(["latex"]))).is_ok());
        assert!(validate_settings(&with("tunnels_max_kbps", json!(0))).is_ok());
    }

    type Agent = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

    /// Connexion réelle d'un faux agent qui annonce `services`.
    async fn connect(pool: &sqlx::PgPool, user: Uuid, ws: Uuid, name: &str, services: Vec<ServiceDecl>) -> (Agent, Uuid) {
        let raw = format!("{TOKEN_PREFIX}{name}");
        let id = add_token(pool, user, ws, &raw, 1).await;
        let url = server(pool.clone()).await;
        let (mut agent, _) = tokio_tungstenite::connect_async(request(&url, Some(&raw))).await.unwrap();
        agent.send(T::text(serde_json::to_string(&Ctl::Hello { v: 1, agent: "fake".into(), services }).unwrap())).await.unwrap();
        (agent, id)
    }

    #[sqlx::test]
    async fn admin_settings_apply_on_handshake_and_mid_session_and_are_audited(pool: sqlx::PgPool) {
        let (user, ws) = seed(&pool).await;
        let pc = |kind: &str| vec![ServiceDecl { name: "pc".into(), kind: kind.into() }];
        set(&pool, "tunnels_enabled = TRUE, tunnels_kinds = ARRAY['jupyter'], tunnels_max_streams = 2").await;
        // Type de service interdit : refus clair à la poignée de main.
        let (mut a, id) = connect(&pool, user, ws, "kind", pc("latex")).await;
        assert!(matches!(text(&mut a).await, Some(Ctl::Bye { code, .. }) if code == "tunnel_kind_not_allowed"));
        assert!(relay::connected(id).is_none());
        // Type permis : connecté, flux limités à 2 (annoncé et appliqué).
        let (mut a, id) = connect(&pool, user, ws, "ok", pc("jupyter")).await;
        let Some(Ctl::Welcome { limits, .. }) = text(&mut a).await else { panic!("welcome") };
        assert_eq!(limits.max_streams, 2);
        let session = relay::connected(id).unwrap();
        let mut held = Vec::new();
        for _ in 0..2 {
            held.push(session.open("pc", "jupyter", relay::Proto::Http, "GET", "/a", vec![]).await.unwrap());
        }
        assert_eq!(session.open("pc", "jupyter", relay::Proto::Http, "GET", "/a", vec![]).await.err(), Some(relay::TunnelError::TooManyStreams));
        // Changement en direct, appliqué à la revalidation.
        let state = AppState::new(pool.clone(), "http://localhost".into());
        set(&pool, "tunnels_max_streams = 8").await;
        revalidate(&state, &session).await;
        assert!(session.open("pc", "jupyter", relay::Proto::Http, "GET", "/a", vec![]).await.is_ok());
        set(&pool, "tunnels_kinds = ARRAY['latex']").await;
        revalidate(&state, &session).await;
        assert!(session.is_closed());
        drop(held);
        // Désactivation depuis l'administration : coupe tout de suite.
        set(&pool, "tunnels_enabled = TRUE, tunnels_kinds = ARRAY['jupyter','latex','http']").await;
        let (mut b, id2) = connect(&pool, user, ws, "dis", pc("latex")).await;
        assert!(matches!(text(&mut b).await, Some(Ctl::Welcome { .. })));
        let before = policy(&pool).await.unwrap();
        set(&pool, "tunnels_enabled = FALSE").await;
        let after = policy(&pool).await.unwrap();
        // Seulement la session de ce test : le registre est partagé avec les autres tests du processus.
        settings_changed_for(&pool, user, &before, &after, relay::connected(id2).into_iter().collect()).await;
        assert!(matches!(text(&mut b).await, Some(Ctl::Bye { code, .. }) if code == "tunnels_disabled"));
        assert!(relay::connected(id2).is_none());
        // Plus de nouvelle connexion : 403 explicite.
        let url = server(pool.clone()).await;
        let e = tokio_tungstenite::connect_async(request(&url, Some("lty_tun_whatever"))).await.err().unwrap();
        assert!(matches!(e, tungstenite::Error::Http(r) if r.status() == 403));
        let events: Vec<String> = sqlx::query_scalar("SELECT event FROM tunnel_audit ORDER BY id").fetch_all(&pool).await.unwrap();
        for ev in ["refused", "connected", "settings_changed"] {
            assert!(events.iter().any(|e| e == ev), "{ev} in {events:?}");
        }
    }

    #[sqlx::test]
    async fn revoked_suspended_expired_and_deleted_tunnels_are_cut_at_revalidation(pool: sqlx::PgPool) {
        let state = AppState::new(pool.clone(), "http://localhost".into());
        set(&pool, "tunnels_enabled = TRUE").await;
        let (user, ws) = seed(&pool).await;
        for (i, (sql, reason)) in [
            ("UPDATE tunnel_tokens SET revoked_at = NOW() WHERE id = $1", "revoked"),
            ("UPDATE tunnel_tokens SET expires_at = NOW() - interval '1 minute' WHERE id = $1", "expired"),
            ("UPDATE users SET is_suspended = TRUE WHERE id = (SELECT user_id FROM tunnel_tokens WHERE id = $1)", "suspended"),
            ("DELETE FROM tunnel_tokens WHERE id = $1", "revoked"),
        ]
        .into_iter()
        .enumerate()
        {
            let (mut a, id) = connect(&pool, user, ws, &format!("r{i}"), vec![]).await;
            assert!(matches!(text(&mut a).await, Some(Ctl::Welcome { .. })));
            let session = relay::connected(id).unwrap();
            revalidate(&state, &session).await;
            assert!(!session.is_closed(), "still valid");
            sqlx::query(sqlx::AssertSqlSafe(sql.to_string())).bind(id).execute(&pool).await.unwrap();
            revalidate(&state, &session).await;
            assert!(session.is_closed(), "{reason}");
            assert_eq!(session.reason(), reason);
            assert!(matches!(text(&mut a).await, Some(Ctl::Bye { code, .. }) if code == reason));
            sqlx::query("UPDATE users SET is_suspended = FALSE").execute(&pool).await.unwrap();
        }
    }

    #[sqlx::test]
    async fn owner_only_tunnels_refuse_other_members_in_the_relay_and_the_api(pool: sqlx::PgPool) {
        let state = AppState::new(pool.clone(), "http://localhost".into());
        set(&pool, "tunnels_enabled = TRUE").await;
        let (_, ws) = seed(&pool).await;
        let ana = member(&pool, ws, "ana@example.org").await;
        let ben = member(&pool, ws, "ben@example.org").await;
        let Json(v) = create(State(state.clone()), ana, Json(body(ws, json!({ "owner_only": true })))).await.unwrap();
        let id: Uuid = serde_json::from_value(v["id"].clone()).unwrap();
        let t = token_row(&state, id).await.unwrap().unwrap();
        assert!(may_use(&t, ana.id) && !may_use(&t, ben.id));
        let (tx, _rx) = mpsc::channel::<Out>(8);
        let session = Session::new(id, ana.id, "x".into(), Utc::now() + chrono::Duration::days(1), vec![], tx);
        session.set_owner_only(true);
        relay::register(session.clone());
        assert!(relay::connected_for(id, ana.id).is_ok());
        assert_eq!(relay::connected_for(id, ben.id).err(), Some(relay::TunnelError::OwnerOnly));
        // Seul le propriétaire ouvre le tunnel aux membres, y compris en direct.
        assert!(update(State(state.clone()), ben, Path(id), Json(UpdateBody { owner_only: false })).await.is_err());
        update(State(state.clone()), ana, Path(id), Json(UpdateBody { owner_only: false })).await.unwrap();
        assert!(relay::connected_for(id, ben.id).is_ok());
        // Seul le propriétaire révoque ; l'audit garde la trace.
        assert!(revoke(State(state.clone()), ben, Path(id)).await.is_err());
        revoke(State(state.clone()), ana, Path(id)).await.unwrap();
        assert!(relay::connected(id).is_none());
        let events: Vec<String> = sqlx::query_scalar("SELECT event FROM tunnel_audit ORDER BY id").fetch_all(&pool).await.unwrap();
        assert_eq!(events, ["created", "owner_only_changed", "revoked"]);
    }

    #[sqlx::test]
    async fn the_audit_log_is_for_admins_and_its_retention_is_bounded(pool: sqlx::PgPool) {
        let state = AppState::new(pool.clone(), "http://localhost".into());
        set(&pool, "tunnels_enabled = TRUE, tunnels_audit_days = 7").await;
        let (_, ws) = seed(&pool).await;
        let ana = member(&pool, ws, "ana@example.org").await;
        let q = || AuditQuery { limit: None, before: None, tunnel: None };
        sqlx::query("INSERT INTO tunnel_audit (event, at) VALUES ('old', NOW() - interval '30 days')").execute(&pool).await.unwrap();
        create(State(state.clone()), ana, Json(body(ws, json!({})))).await.unwrap();
        assert!(matches!(audit_list(State(state.clone()), ana, Query(q())).await, Err(ApiError::Forbidden)));
        let admin = AuthUser { id: ana.id, is_super_admin: true };
        let Json(v) = audit_list(State(state.clone()), admin, Query(q())).await.unwrap();
        let events = v["events"].as_array().unwrap();
        assert_eq!((v["retention_days"].clone(), events.len(), events[0]["event"].clone(), events[0]["owner_email"].clone()), (json!(7), 1, json!("created"), json!("ana@example.org")));
    }

    #[sqlx::test]
    async fn giant_frames_and_undeclared_services_are_refused(pool: sqlx::PgPool) {
        set(&pool, "tunnels_enabled = TRUE").await;
        let (user, ws) = seed(&pool).await;
        let (mut a, id) = connect(&pool, user, ws, "giant", vec![ServiceDecl { name: "pc".into(), kind: "jupyter".into() }]).await;
        assert!(matches!(text(&mut a).await, Some(Ctl::Welcome { .. })));
        let session = relay::connected(id).unwrap();
        assert_eq!(session.open("other", "jupyter", relay::Proto::Http, "GET", "/", vec![]).await.err(), Some(relay::TunnelError::NoService));
        assert_eq!(session.open("pc", "latex", relay::Proto::Http, "GET", "/", vec![]).await.err(), Some(relay::TunnelError::NoService));
        // Trame géante venant de l'agent : la connexion est coupée.
        let _ = a.send(T::binary(vec![0u8; relay::MAX_FRAME * 4])).await;
        for _ in 0..60 {
            if relay::connected(id).is_none() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        assert!(relay::connected(id).is_none());
    }

    #[tokio::test]
    async fn the_bandwidth_cap_slows_a_tunnel_down() {
        let (tx, _rx) = mpsc::channel::<Out>(8);
        let s = Session::new(Uuid::new_v4(), Uuid::new_v4(), "x".into(), Utc::now() + chrono::Duration::days(1), vec![], tx);
        let start = std::time::Instant::now();
        s.pace(1_000_000).await;
        assert!(start.elapsed() < std::time::Duration::from_millis(50), "no cap: no wait");
        s.apply(&Live { max_streams: 4, bytes_per_sec: 100_000, kinds: vec!["jupyter".into()] });
        s.pace(30_000).await;
        s.pace(30_000).await;
        assert!(start.elapsed() >= std::time::Duration::from_millis(250), "{:?}", start.elapsed());
    }
}

//! Tunnel PC : jetons, point d'entrée des agents `looty-bridge` et statut (ADR 0020).
//!
//! Une personne crée un jeton de tunnel (lié à elle et à un espace, éventuellement à un projet),
//! lance l'agent sur son PC avec ce jeton, et l'agent ouvre une WebSocket sortante vers
//! `/api/tunnels/agent`. Le projet peut alors cibler « le tunnel de X » au lieu d'une URL de serveur
//! Jupyter (`jupyter.rs`). Le protocole de multiplexage est dans `tunnel_relay.rs` et
//! `docs/tunnel-protocol.md`.
//!
//! - Jeton : `lty_tun_` + 32 octets aléatoires, montré une seule fois, stocké haché (SHA-256),
//!   durée limitée (30 jours par défaut, 365 au plus), révocable. La révocation coupe la connexion.
//! - Désactivé tant que `LOOTY_TUNNELS` n'est pas `on` (aucune route ne répond, hors liste vide).
//! - Seul le propriétaire du jeton l'attache à un projet qu'il peut modifier ; les autres membres
//!   l'utilisent selon les droits du projet (`jupyter.rs`).

use axum::{
    extract::{
        ws::{CloseFrame, Message as AxMsg, WebSocket, WebSocketUpgrade},
        Path, State,
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
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::access::{hash_token, AuthUser};
use crate::error::{ApiError, ApiResult};
use crate::projects::{project_for, Need};
use crate::state::AppState;
use crate::tunnel_relay::{self as relay, Ctl, Out, Session};

pub const TOKEN_PREFIX: &str = "lty_tun_";
const MAX_ACTIVE_TOKENS: i64 = 20;

pub fn enabled() -> bool {
    std::env::var("LOOTY_TUNNELS").is_ok_and(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "on" | "1" | "true" | "yes"))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/agent", get(agent))
        .route("/{id}", delete(revoke))
        .route("/{id}/status", get(status))
}

fn available(state: &AppState) -> ApiResult<()> {
    if state.demo {
        return Err(ApiError::DemoUnavailable);
    }
    if !enabled() {
        return Err(ApiError::bad_request("tunnels_disabled"));
    }
    Ok(())
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
}

const COLS: &str = "id, user_id, workspace_id, project_id, name, expires_at, revoked_at, last_connected_at, created_at";

impl TokenRow {
    pub fn usable(&self) -> bool {
        self.revoked_at.is_none() && self.expires_at > Utc::now()
    }
}

pub async fn token_row(state: &AppState, id: Uuid) -> ApiResult<Option<TokenRow>> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT {COLS} FROM tunnel_tokens WHERE id = $1"))).bind(id).fetch_optional(&state.pool).await?)
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
        "status": live_json(t.id),
    })
}

async fn list(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<Value>> {
    if state.demo || !enabled() {
        return Ok(Json(json!({ "enabled": false, "tunnels": [] })));
    }
    let rows: Vec<TokenRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT {COLS} FROM tunnel_tokens WHERE user_id = $1 AND revoked_at IS NULL ORDER BY created_at DESC")))
        .bind(user.id)
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(json!({ "enabled": true, "tunnels": rows.iter().map(token_json).collect::<Vec<_>>() })))
}

#[derive(Deserialize)]
struct CreateBody {
    name: String,
    workspace_id: Uuid,
    project_id: Option<Uuid>,
    expires_in_days: Option<i64>,
}

async fn create(State(state): State<AppState>, user: AuthUser, Json(b): Json<CreateBody>) -> ApiResult<Json<Value>> {
    available(&state)?;
    let name = b.name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(ApiError::bad_request("invalid_value"));
    }
    let days = b.expires_in_days.unwrap_or(30);
    if !(1..=365).contains(&days) {
        return Err(ApiError::bad_request("invalid_value"));
    }
    crate::access::require_member(&state.pool, &user, b.workspace_id).await?;
    if let Some(p) = b.project_id {
        let (project, _) = project_for(&state, &user, p, Need::Write).await?;
        if project.workspace_id != b.workspace_id {
            return Err(ApiError::bad_request("tunnel_wrong_workspace"));
        }
    }
    let active: i64 = sqlx::query_scalar("SELECT count(*) FROM tunnel_tokens WHERE user_id = $1 AND revoked_at IS NULL AND expires_at > NOW()").bind(user.id).fetch_one(&state.pool).await?;
    if active >= MAX_ACTIVE_TOKENS {
        return Err(ApiError::bad_request("tunnel_too_many_tokens"));
    }
    let raw: [u8; 32] = rand::random();
    let token = format!("{TOKEN_PREFIX}{}", base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw));
    let row: TokenRow = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "INSERT INTO tunnel_tokens (user_id, workspace_id, project_id, name, token_hash, expires_at)
         VALUES ($1, $2, $3, $4, $5, NOW() + make_interval(days => $6::int)) RETURNING {COLS}"
    )))
    .bind(user.id)
    .bind(b.workspace_id)
    .bind(b.project_id)
    .bind(name)
    .bind(hash_token(&token))
    .bind(days as i32)
    .fetch_one(&state.pool)
    .await?;
    tracing::info!(tunnel = %row.id, user = %user.id, "tunnel token created");
    let mut v = token_json(&row);
    // Seule fois où le jeton est visible.
    v["token"] = json!(token);
    Ok(Json(v))
}

async fn revoke(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<StatusCode> {
    let done = sqlx::query("UPDATE tunnel_tokens SET revoked_at = NOW() WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL").bind(id).bind(user.id).execute(&state.pool).await?;
    if done.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    relay::disconnect(id, 4001, "revoked");
    tracing::info!(tunnel = %id, user = %user.id, "tunnel token revoked");
    Ok(StatusCode::NO_CONTENT)
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

/// Jeton valide pour une connexion : non révoqué, non expiré, compte non suspendu.
pub async fn authenticate(state: &AppState, raw: &str) -> Result<TokenRow, &'static str> {
    if !raw.starts_with(TOKEN_PREFIX) || raw.len() > 128 {
        return Err("tunnel_token_invalid");
    }
    let row: Option<(TokenRow, bool)> = {
        let r: Option<(Uuid, Uuid, Uuid, Option<Uuid>, String, DateTime<Utc>, Option<DateTime<Utc>>, Option<DateTime<Utc>>, DateTime<Utc>, bool)> = sqlx::query_as(
            "SELECT t.id, t.user_id, t.workspace_id, t.project_id, t.name, t.expires_at, t.revoked_at, t.last_connected_at, t.created_at, u.is_suspended
               FROM tunnel_tokens t JOIN users u ON u.id = t.user_id WHERE t.token_hash = $1",
        )
        .bind(hash_token(raw))
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| "internal_error")?;
        r.map(|(id, user_id, workspace_id, project_id, name, expires_at, revoked_at, last_connected_at, created_at, suspended)| {
            (TokenRow { id, user_id, workspace_id, project_id, name, expires_at, revoked_at, last_connected_at, created_at }, suspended)
        })
    };
    let Some((t, suspended)) = row else { return Err("tunnel_token_invalid") };
    if t.revoked_at.is_some() {
        return Err("tunnel_token_revoked");
    }
    if t.expires_at <= Utc::now() {
        return Err("tunnel_token_expired");
    }
    if suspended {
        return Err("tunnel_token_invalid");
    }
    Ok(t)
}

async fn agent(ws: WebSocketUpgrade, State(state): State<AppState>, headers: HeaderMap) -> Response {
    if state.demo || !enabled() {
        return refuse(StatusCode::NOT_FOUND, "not_found");
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
    ws.protocols([relay::SUBPROTOCOL]).max_message_size(relay::MAX_FRAME + 64).max_frame_size(relay::MAX_FRAME + 64).on_upgrade(move |socket| serve(socket, token))
}

async fn close_with(socket: &mut WebSocket, code: u16, reason: &str) {
    let _ = socket.send(AxMsg::Close(Some(CloseFrame { code, reason: reason.to_string().into() }))).await;
}

async fn serve(mut socket: WebSocket, token: TokenRow) {
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
    let _ = session.welcome().await;
    run(&session, &mut stream).await;
    session.shutdown();
    relay::unregister(&session);
    // Laisse partir le `bye` et la fermeture avant de couper l'écriture.
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), writer).await;
    tracing::info!(tunnel = %token.id, user = %token.user_id, secs = session.since.elapsed().as_secs(), bytes_in = session.bytes_in.load(std::sync::atomic::Ordering::Relaxed), bytes_out = session.bytes_out.load(std::sync::atomic::Ordering::Relaxed), "tunnel disconnected");
}

async fn run(session: &Arc<Session>, stream: &mut (impl futures::Stream<Item = Result<AxMsg, axum::Error>> + Unpin)) {
    let mut tick = tokio::time::interval(relay::PING_EVERY);
    tick.tick().await;
    loop {
        tokio::select! {
            msg = stream.next() => {
                let verdict = match msg {
                    Some(Ok(AxMsg::Text(t))) => session.on_text(t.as_str()).await,
                    Some(Ok(AxMsg::Binary(b))) => session.on_binary(&b),
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
                if session.expires_at <= Utc::now() {
                    session.close(4002, "expired");
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
}

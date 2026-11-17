//! Agents IA : connexions par personne, conversations, autorisations d'écriture et chat en flux.
//!
//! La clé d'API est chiffrée en base (`crypto`) et ne ressort jamais de l'API (seulement un
//! indice masqué). Les outils et leurs garde-fous sont dans `ai_tools` ; docs/adr/0009.

use std::convert::Infallible;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
    routing::{delete, get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use futures::Stream;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::access::{require_member_or_guest, require_page_read, require_page_write, AuthUser};
use crate::ai_pages::Op;
use crate::ai_provider::{self, ChatRequest, Conn, Msg, StreamEvent, ToolCall};
use crate::ai_tools;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

const PROVIDERS: [&str; 5] = ["openai", "anthropic", "gemini", "perplexity", "openai_compatible"];
const MAX_TOOL_ROUNDS: usize = 8;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/connections", get(list_connections).post(create_connection))
        .route("/connections/{id}", axum::routing::patch(update_connection).delete(delete_connection))
        .route("/conversations", get(list_conversations).post(create_conversation))
        .route("/conversations/{id}", get(get_conversation).patch(update_conversation).delete(delete_conversation))
        .route("/conversations/{id}/messages", post(send_message))
        .route("/conversations/{id}/grants", get(list_grants).post(create_grant))
        .route("/conversations/{id}/grants/{grant_id}", delete(revoke_grant))
        .route("/suggestions/{id}/accept", post(accept_suggestion))
        .route("/suggestions/{id}/reject", post(reject_suggestion))
}

fn no_demo(state: &AppState) -> ApiResult<()> {
    if state.demo {
        return Err(ApiError::DemoUnavailable);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Connexions
// ---------------------------------------------------------------------------

#[derive(Serialize, sqlx::FromRow)]
struct ConnRow {
    id: Uuid,
    provider: String,
    label: String,
    model: String,
    base_url: Option<String>,
    #[serde(skip)]
    api_key_enc: Option<String>,
    created_at: DateTime<Utc>,
}

#[derive(Serialize)]
struct ConnDto {
    #[serde(flatten)]
    row: ConnRow,
    has_key: bool,
    /// Fin de la clé (« ••••ab12 ») : jamais la clé elle-même.
    key_hint: Option<String>,
}

fn mask(key: &str) -> String {
    let tail: String = key.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
    if key.chars().count() < 12 {
        "••••".into()
    } else {
        format!("••••{tail}")
    }
}

fn dto(row: ConnRow) -> ConnDto {
    let key = row.api_key_enc.as_deref().and_then(crate::crypto::decrypt);
    ConnDto { has_key: key.is_some(), key_hint: key.as_deref().map(mask), row }
}

const CONN_COLS: &str = "id, provider, label, model, base_url, api_key_enc, created_at";

async fn list_connections(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<Vec<ConnDto>>> {
    let rows: Vec<ConnRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT {CONN_COLS} FROM ai_connections WHERE user_id = $1 ORDER BY created_at")))
        .bind(user.id)
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(rows.into_iter().map(dto).collect()))
}

/// Adresse d'un serveur compatible : http(s) ; hors réseau privé autorisé, https et adresse publique.
async fn check_base_url(raw: &str) -> ApiResult<String> {
    let raw = raw.trim().trim_end_matches('/');
    if raw.len() > 300 {
        return Err(ApiError::bad_request("invalid_base_url"));
    }
    let private_ok = crate::caldav::allow_private() || std::env::var("AI_ALLOW_PRIVATE").as_deref() == Ok("true");
    if private_ok {
        let url = reqwest::Url::parse(raw).map_err(|_| ApiError::bad_request("invalid_base_url"))?;
        if !matches!(url.scheme(), "http" | "https") || url.username() != "" {
            return Err(ApiError::bad_request("invalid_base_url"));
        }
    } else {
        crate::caldav::check_url(raw).await.map_err(|_| ApiError::bad_request("invalid_base_url"))?;
    }
    Ok(raw.to_string())
}

fn clean(s: &str, max: usize, err: &'static str) -> ApiResult<String> {
    let s = s.trim();
    if s.is_empty() || s.chars().count() > max {
        return Err(ApiError::bad_request(err));
    }
    Ok(s.to_string())
}

#[derive(Deserialize)]
struct ConnReq {
    provider: String,
    label: String,
    model: String,
    base_url: Option<String>,
    api_key: Option<String>,
}

async fn create_connection(State(state): State<AppState>, user: AuthUser, Json(p): Json<ConnReq>) -> ApiResult<(StatusCode, Json<ConnDto>)> {
    no_demo(&state)?;
    if !PROVIDERS.contains(&p.provider.as_str()) {
        return Err(ApiError::bad_request("invalid_provider"));
    }
    let key = p.api_key.as_deref().map(str::trim).filter(|k| !k.is_empty());
    if key.is_none() && p.provider != "openai_compatible" {
        return Err(ApiError::bad_request("api_key_required"));
    }
    let base = match p.base_url.as_deref().map(str::trim).filter(|b| !b.is_empty()) {
        Some(b) => Some(check_base_url(b).await?),
        None if p.provider == "openai_compatible" => return Err(ApiError::bad_request("base_url_required")),
        None => None,
    };
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM ai_connections WHERE user_id = $1").bind(user.id).fetch_one(&state.pool).await?;
    if count >= 20 {
        return Err(ApiError::bad_request("too_many_connections"));
    }
    let row: ConnRow = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "INSERT INTO ai_connections (user_id, provider, label, model, base_url, api_key_enc) VALUES ($1, $2, $3, $4, $5, $6) RETURNING {CONN_COLS}"
    )))
    .bind(user.id)
    .bind(&p.provider)
    .bind(clean(&p.label, 80, "invalid_label")?)
    .bind(clean(&p.model, 120, "invalid_model")?)
    .bind(base)
    .bind(key.map(crate::crypto::encrypt))
    .fetch_one(&state.pool)
    .await?;
    Ok((StatusCode::CREATED, Json(dto(row))))
}

#[derive(Deserialize)]
struct ConnPatch {
    label: Option<String>,
    model: Option<String>,
    base_url: Option<String>,
    /// Absente ou vide : la clé enregistrée est conservée.
    api_key: Option<String>,
}

async fn update_connection(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(p): Json<ConnPatch>) -> ApiResult<Json<ConnDto>> {
    let cur: ConnRow = sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT {CONN_COLS} FROM ai_connections WHERE id = $1 AND user_id = $2")))
        .bind(id)
        .bind(user.id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(ApiError::NotFound)?;
    let base = match p.base_url.as_deref().map(str::trim) {
        Some("") => None,
        Some(b) => Some(check_base_url(b).await?),
        None => cur.base_url.clone(),
    };
    if base.is_none() && cur.provider == "openai_compatible" {
        return Err(ApiError::bad_request("base_url_required"));
    }
    let key_enc = match p.api_key.as_deref().map(str::trim).filter(|k| !k.is_empty()) {
        Some(k) => Some(crate::crypto::encrypt(k)),
        None => cur.api_key_enc.clone(),
    };
    let row: ConnRow = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "UPDATE ai_connections SET label = $3, model = $4, base_url = $5, api_key_enc = $6, updated_at = now() WHERE id = $1 AND user_id = $2 RETURNING {CONN_COLS}"
    )))
    .bind(id)
    .bind(user.id)
    .bind(match &p.label { Some(l) => clean(l, 80, "invalid_label")?, None => cur.label })
    .bind(match &p.model { Some(m) => clean(m, 120, "invalid_model")?, None => cur.model })
    .bind(base)
    .bind(key_enc)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(dto(row)))
}

async fn delete_connection(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<StatusCode> {
    sqlx::query("DELETE FROM ai_connections WHERE id = $1 AND user_id = $2").bind(id).bind(user.id).execute(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Conversations
// ---------------------------------------------------------------------------

#[derive(Serialize, sqlx::FromRow)]
struct ConvRow {
    id: Uuid,
    connection_id: Option<Uuid>,
    context_kind: String,
    context_id: Option<Uuid>,
    title: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

const CONV_COLS: &str = "id, connection_id, context_kind, context_id, title, created_at, updated_at";

async fn own_conversation(pool: &PgPool, user: &AuthUser, id: Uuid) -> ApiResult<ConvRow> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT {CONV_COLS} FROM ai_conversations WHERE id = $1 AND user_id = $2")))
        .bind(id)
        .bind(user.id)
        .fetch_optional(pool)
        .await?
        .ok_or(ApiError::NotFound)
}

async fn own_connection(pool: &PgPool, user: &AuthUser, id: Uuid) -> ApiResult<()> {
    let ok: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM ai_connections WHERE id = $1 AND user_id = $2)").bind(id).bind(user.id).fetch_one(pool).await?;
    ok.then_some(()).ok_or(ApiError::NotFound)
}

/// Le contexte doit être lisible par la personne.
async fn check_context(pool: &PgPool, user: &AuthUser, kind: &str, id: Option<Uuid>) -> ApiResult<()> {
    match (kind, id) {
        ("none", _) => Ok(()),
        ("page", Some(p)) => require_page_read(pool, user, p).await.map(|_| ()),
        ("workspace", Some(w)) => require_member_or_guest(pool, user, w).await.map(|_| ()),
        ("project", Some(pr)) => {
            let ws: Uuid = sqlx::query_scalar("SELECT workspace_id FROM projects WHERE id = $1").bind(pr).fetch_optional(pool).await?.ok_or(ApiError::NotFound)?;
            require_member_or_guest(pool, user, ws).await.map(|_| ())
        }
        _ => Err(ApiError::bad_request("invalid_context")),
    }
}

#[derive(Deserialize)]
struct ListQuery {
    context_kind: Option<String>,
    context_id: Option<Uuid>,
}

async fn list_conversations(State(state): State<AppState>, user: AuthUser, Query(q): Query<ListQuery>) -> ApiResult<Json<Vec<ConvRow>>> {
    let rows: Vec<ConvRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {CONV_COLS} FROM ai_conversations WHERE user_id = $1
         AND ($2::text IS NULL OR context_kind = $2) AND ($3::uuid IS NULL OR context_id = $3)
         ORDER BY updated_at DESC LIMIT 50"
    )))
    .bind(user.id)
    .bind(q.context_kind)
    .bind(q.context_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

#[derive(Deserialize)]
struct NewConv {
    connection_id: Uuid,
    context_kind: Option<String>,
    context_id: Option<Uuid>,
}

async fn create_conversation(State(state): State<AppState>, user: AuthUser, Json(p): Json<NewConv>) -> ApiResult<(StatusCode, Json<ConvRow>)> {
    own_connection(&state.pool, &user, p.connection_id).await?;
    let kind = p.context_kind.unwrap_or_else(|| "none".into());
    check_context(&state.pool, &user, &kind, p.context_id).await?;
    let row: ConvRow = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "INSERT INTO ai_conversations (user_id, connection_id, context_kind, context_id) VALUES ($1, $2, $3, $4) RETURNING {CONV_COLS}"
    )))
    .bind(user.id)
    .bind(p.connection_id)
    .bind(&kind)
    .bind(if kind == "none" { None } else { p.context_id })
    .fetch_one(&state.pool)
    .await?;
    Ok((StatusCode::CREATED, Json(row)))
}

#[derive(Deserialize)]
struct ConvPatch {
    connection_id: Uuid,
}

async fn update_conversation(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(p): Json<ConvPatch>) -> ApiResult<StatusCode> {
    own_conversation(&state.pool, &user, id).await?;
    own_connection(&state.pool, &user, p.connection_id).await?;
    sqlx::query("UPDATE ai_conversations SET connection_id = $2 WHERE id = $1").bind(id).bind(p.connection_id).execute(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_conversation(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<StatusCode> {
    sqlx::query("DELETE FROM ai_conversations WHERE id = $1 AND user_id = $2").bind(id).bind(user.id).execute(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize, sqlx::FromRow)]
struct MsgRow {
    id: Uuid,
    role: String,
    content: String,
    data: Option<Value>,
    created_at: DateTime<Utc>,
}

#[derive(Serialize, sqlx::FromRow)]
struct GrantRow {
    id: Uuid,
    page_id: Uuid,
    page_title: String,
    workspace_id: Uuid,
    mode: String,
    expires_at: DateTime<Utc>,
}

async fn active_grants(pool: &PgPool, conv_id: Uuid) -> ApiResult<Vec<GrantRow>> {
    Ok(sqlx::query_as(
        "SELECT g.id, g.page_id, p.title AS page_title, p.workspace_id, g.mode, g.expires_at
         FROM ai_write_grants g JOIN pages p ON p.id = g.page_id
         WHERE g.conversation_id = $1 AND g.revoked_at IS NULL AND g.expires_at > now() ORDER BY g.created_at",
    )
    .bind(conv_id)
    .fetch_all(pool)
    .await?)
}

async fn pending_suggestions(pool: &PgPool, conv_id: Uuid) -> ApiResult<Vec<Value>> {
    let rows: Vec<(Uuid, Uuid, String, String)> = sqlx::query_as("SELECT id, page_id, before_md, after_md FROM ai_suggestions WHERE conversation_id = $1 AND status = 'pending' ORDER BY created_at")
        .bind(conv_id)
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(|(id, page_id, b, a)| json!({ "id": id, "page_id": page_id, "diff": crate::ai_pages::diff_lines(&b, &a).iter().map(|(o, t)| json!({ "op": o.to_string(), "text": t })).collect::<Vec<_>>() }))
        .collect())
}

async fn get_conversation(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    let conv = own_conversation(&state.pool, &user, id).await?;
    let msgs: Vec<MsgRow> = sqlx::query_as("SELECT id, role, content, data, created_at FROM ai_messages WHERE conversation_id = $1 ORDER BY created_at, id").bind(id).fetch_all(&state.pool).await?;
    Ok(Json(json!({
        "conversation": conv,
        "messages": msgs,
        "grants": active_grants(&state.pool, id).await?,
        "suggestions": pending_suggestions(&state.pool, id).await?,
    })))
}

// ---------------------------------------------------------------------------
// Autorisations d'écriture
// ---------------------------------------------------------------------------

async fn list_grants(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<Json<Vec<GrantRow>>> {
    own_conversation(&state.pool, &user, id).await?;
    Ok(Json(active_grants(&state.pool, id).await?))
}

#[derive(Deserialize)]
struct NewGrant {
    page_id: Uuid,
    mode: Option<String>,
    expires_in_minutes: Option<i64>,
}

async fn create_grant(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(p): Json<NewGrant>) -> ApiResult<(StatusCode, Json<Vec<GrantRow>>)> {
    own_conversation(&state.pool, &user, id).await?;
    let mode = p.mode.unwrap_or_else(|| "suggest".into());
    if !ai_tools::MODES.contains(&mode.as_str()) {
        return Err(ApiError::bad_request("invalid_mode"));
    }
    let minutes = p.expires_in_minutes.unwrap_or(60);
    if !(1..=1440).contains(&minutes) {
        return Err(ApiError::bad_request("invalid_expiry"));
    }
    // On n'accorde que ce que la personne peut elle-même faire (et le serveur le revérifie à chaque écriture).
    require_page_write(&state.pool, &user, p.page_id).await?;
    let mut tx = state.pool.begin().await?;
    sqlx::query("UPDATE ai_write_grants SET revoked_at = now() WHERE conversation_id = $1 AND page_id = $2 AND revoked_at IS NULL").bind(id).bind(p.page_id).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO ai_write_grants (conversation_id, page_id, mode, expires_at) VALUES ($1, $2, $3, now() + make_interval(mins => $4::int))")
        .bind(id)
        .bind(p.page_id)
        .bind(&mode)
        .bind(minutes as i32)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(active_grants(&state.pool, id).await?)))
}

async fn revoke_grant(State(state): State<AppState>, user: AuthUser, Path((id, grant_id)): Path<(Uuid, Uuid)>) -> ApiResult<StatusCode> {
    own_conversation(&state.pool, &user, id).await?;
    sqlx::query("UPDATE ai_write_grants SET revoked_at = now() WHERE id = $1 AND conversation_id = $2 AND revoked_at IS NULL").bind(grant_id).bind(id).execute(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Suggestions
// ---------------------------------------------------------------------------

async fn own_suggestion(pool: &PgPool, user: &AuthUser, id: Uuid) -> ApiResult<(Uuid, String, String)> {
    sqlx::query_as(
        "SELECT s.page_id, s.apply_mode, s.payload_md FROM ai_suggestions s JOIN ai_conversations c ON c.id = s.conversation_id
         WHERE s.id = $1 AND c.user_id = $2 AND s.status = 'pending'",
    )
    .bind(id)
    .bind(user.id)
    .fetch_optional(pool)
    .await?
    .ok_or(ApiError::NotFound)
}

async fn accept_suggestion(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    let (page_id, mode, md) = own_suggestion(&state.pool, &user, id).await?;
    let op = if mode == "append" { Op::Append } else { Op::Replace };
    // Accepter est un acte de la personne : ses droits sur la page sont revérifiés dans `apply_change`.
    let revision = ai_tools::apply_change(&state.pool, &user, page_id, op, &md).await?;
    sqlx::query("UPDATE ai_suggestions SET status = 'accepted' WHERE id = $1").bind(id).execute(&state.pool).await?;
    Ok(Json(json!({ "page_id": page_id, "revision_id": revision })))
}

async fn reject_suggestion(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<StatusCode> {
    own_suggestion(&state.pool, &user, id).await?;
    sqlx::query("UPDATE ai_suggestions SET status = 'rejected' WHERE id = $1").bind(id).execute(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Chat
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct SendReq {
    content: String,
}

async fn system_prompt(pool: &PgPool, user: &AuthUser, conv: &ConvRow, tools: bool) -> String {
    let mut s = format!(
        "You are an assistant inside Looty, a notes, documents and projects app. Today is {}.\n\
         Answer in the language of the user's message.\n\n\
         PERMISSIONS: you are READ-ONLY by default. You can only read what the user can access, and write only to a page the user explicitly granted to this conversation, within the granted mode (suggest = the user reviews and accepts your diff; append; replace). The server enforces this; a refused write means you must tell the user to grant it in the chat, never try another way. Never claim a page was changed unless the tool says it was.\n\n\
         {}\n",
        Utc::now().format("%Y-%m-%d"),
        crate::ai_pages::SYNTAX_GUIDE
    );
    match (conv.context_kind.as_str(), conv.context_id) {
        ("page", Some(p)) => {
            s.push_str(&format!("\nCONTEXT: the user is looking at the page {p}."));
            if !tools {
                if let Ok((_, _, _)) = require_page_read(pool, user, p).await {
                    if let Ok((title, md)) = ai_tools::page_markdown(pool, p).await {
                        let md: String = md.chars().take(20_000).collect();
                        s.push_str(&format!("\nTitle: {title}\nContent:\n{md}"));
                    }
                }
            }
        }
        ("project", Some(p)) => s.push_str(&format!("\nCONTEXT: the user is working in the project {p} (its pages belong to its workspace).")),
        ("workspace", Some(w)) => s.push_str(&format!("\nCONTEXT: the user is in the workspace {w}; list_pages lists its pages.")),
        _ => s.push_str("\nCONTEXT: none (global chat). Use list_pages to discover workspaces and pages."),
    }
    if !tools {
        s.push_str("\nThis model cannot call tools: you can only answer in text.");
    }
    s
}

async fn load_history(pool: &PgPool, conv_id: Uuid) -> ApiResult<Vec<Msg>> {
    let rows: Vec<(String, String, Option<Value>)> = sqlx::query_as(
        "SELECT role, content, data FROM (SELECT role, content, data, created_at, id FROM ai_messages WHERE conversation_id = $1 ORDER BY created_at DESC, id DESC LIMIT 60) t ORDER BY created_at, id",
    )
    .bind(conv_id)
    .fetch_all(pool)
    .await?;
    let mut out = vec![];
    // Une coupe de l'historique ne doit pas commencer par un résultat d'outil orphelin.
    let mut started = false;
    for (role, content, data) in rows {
        match role.as_str() {
            "user" => {
                started = true;
                out.push(Msg::User(content));
            }
            "assistant" if started => {
                let calls: Vec<ToolCall> = data
                    .as_ref()
                    .and_then(|d| d["calls"].as_array())
                    .map(|a| a.iter().map(|c| ToolCall { id: c["id"].as_str().unwrap_or("").into(), name: c["name"].as_str().unwrap_or("").into(), args: c["args"].clone() }).collect())
                    .unwrap_or_default();
                out.push(Msg::Assistant { text: content, calls });
            }
            "tool" if started => {
                let d = data.unwrap_or(Value::Null);
                out.push(Msg::ToolResult { call_id: d["call_id"].as_str().unwrap_or("").into(), name: d["name"].as_str().unwrap_or("").into(), content });
            }
            _ => {}
        }
    }
    Ok(out)
}

async fn store(pool: &PgPool, conv_id: Uuid, role: &str, content: &str, data: Option<Value>) -> ApiResult<()> {
    sqlx::query("INSERT INTO ai_messages (conversation_id, role, content, data) VALUES ($1, $2, $3, $4)").bind(conv_id).bind(role).bind(content).bind(data).execute(pool).await?;
    sqlx::query("UPDATE ai_conversations SET updated_at = now(), title = CASE WHEN title = '' AND $2 = 'user' THEN left($3, 60) ELSE title END WHERE id = $1").bind(conv_id).bind(role).bind(content).execute(pool).await?;
    Ok(())
}

fn sse(v: Value) -> Event {
    Event::default().data(v.to_string())
}

async fn send_message(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(p): Json<SendReq>) -> ApiResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    no_demo(&state)?;
    let conv = own_conversation(&state.pool, &user, id).await?;
    let content = p.content.trim().to_string();
    if content.is_empty() || content.len() > 100_000 {
        return Err(ApiError::bad_request("invalid_message"));
    }
    let conn_id = conv.connection_id.ok_or_else(|| ApiError::bad_request("no_connection"))?;
    let row: (String, String, Option<String>, Option<String>) = sqlx::query_as("SELECT provider, model, base_url, api_key_enc FROM ai_connections WHERE id = $1 AND user_id = $2")
        .bind(conn_id)
        .bind(user.id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::bad_request("no_connection"))?;
    let api_key = row.3.as_deref().and_then(crate::crypto::decrypt);
    if row.3.is_some() && api_key.is_none() {
        return Err(ApiError::bad_request("api_key_unreadable"));
    }
    let tools_ok = ai_provider::supports_tools(&row.0);
    let provider = ai_provider::build(&Conn { provider: row.0, model: row.1, base_url: row.2, api_key }).map_err(ApiError::bad_request)?;
    store(&state.pool, id, "user", &content, None).await?;

    let (tx, rx) = mpsc::channel::<Event>(64);
    let pool = state.pool.clone();
    tokio::spawn(async move {
        let system = system_prompt(&pool, &user, &conv, tools_ok).await;
        let specs = if tools_ok { ai_tools::tool_specs() } else { vec![] };
        let result = run_agent(&pool, &user, id, provider.as_ref(), &system, &specs, &tx).await;
        match result {
            Ok(()) => {
                let _ = tx.send(sse(json!({ "type": "done" }))).await;
            }
            Err(msg) => {
                tracing::warn!("ai chat: {msg}");
                let _ = tx.send(sse(json!({ "type": "error", "message": msg }))).await;
            }
        }
    });
    let stream = futures::stream::unfold(rx, |mut rx| async move { rx.recv().await.map(|e| (Ok::<_, Infallible>(e), rx)) });
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

/// Boucle agent : le modèle répond, appelle des outils, reçoit leurs résultats, jusqu'à une réponse finale.
async fn run_agent(pool: &PgPool, user: &AuthUser, conv_id: Uuid, provider: &dyn ai_provider::Provider, system: &str, specs: &[ai_provider::ToolSpec], out: &mpsc::Sender<Event>) -> Result<(), String> {
    for _ in 0..MAX_TOOL_ROUNDS {
        if out.is_closed() {
            return Ok(());
        }
        let history = load_history(pool, conv_id).await.map_err(|_| "internal_error".to_string())?;
        let req = ChatRequest { system, messages: &history, tools: specs };
        let (etx, mut erx) = mpsc::channel::<StreamEvent>(64);
        let produce = async {
            let etx = etx;
            provider.stream(&req, &etx).await
        };
        let consume = async {
            let (mut text, mut calls) = (String::new(), vec![]);
            while let Some(ev) = erx.recv().await {
                match ev {
                    StreamEvent::Text(t) => {
                        text.push_str(&t);
                        let _ = out.send(sse(json!({ "type": "text", "delta": t }))).await;
                    }
                    StreamEvent::Call(c) => calls.push(c),
                }
            }
            (text, calls)
        };
        let (res, (text, calls)) = tokio::join!(produce, consume);
        // Ce qui a déjà été dit est conservé même si le fournisseur échoue ensuite.
        if !text.is_empty() || !calls.is_empty() {
            let data = (!calls.is_empty()).then(|| json!({ "calls": calls.iter().map(|c| json!({ "id": c.id, "name": c.name, "args": c.args })).collect::<Vec<_>>() }));
            store(pool, conv_id, "assistant", &text, data).await.map_err(|_| "internal_error".to_string())?;
        }
        res?;
        if calls.is_empty() {
            return Ok(());
        }
        for c in calls {
            let o = ai_tools::execute(pool, user, conv_id, &c.name, &c.args).await;
            let _ = out.send(sse(json!({ "type": "tool", "name": c.name, "ok": !o.text.starts_with("error"), "page_id": c.args["page_id"] }))).await;
            if let Some(s) = &o.suggestion {
                let _ = out.send(sse(json!({ "type": "suggestion", "suggestion": s }))).await;
            }
            if let Some(p) = o.changed_page {
                let _ = out.send(sse(json!({ "type": "page_changed", "page_id": p }))).await;
            }
            store(pool, conv_id, "tool", &o.text, Some(json!({ "call_id": c.id, "name": c.name }))).await.map_err(|_| "internal_error".to_string())?;
        }
    }
    Err("too_many_tool_rounds".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masking_never_reveals_more_than_the_last_four_characters() {
        assert_eq!(mask("sk-abcdefghijklmnop"), "••••mnop");
        assert_eq!(mask("short"), "••••");
    }

    #[test]
    fn connection_json_has_no_key_field() {
        let row = ConnRow { id: Uuid::nil(), provider: "openai".into(), label: "l".into(), model: "m".into(), base_url: None, api_key_enc: Some(crate::crypto::encrypt("sk-secretsecretsecret")), created_at: Utc::now() };
        let s = serde_json::to_string(&dto(row)).unwrap();
        assert!(!s.contains("secretsecret") && !s.contains("api_key"));
        assert!(s.contains("\"has_key\":true"));
    }
}

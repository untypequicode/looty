//! Commentaires sur les pages et les événements de l'espace (les tâches gardent les leurs,
//! voir board.rs). Fils de discussion à un niveau, passage cité optionnel (commentaire
//! ancré dans le texte), résolution, mentions et notifications des participants.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post, put},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::access::{require_member, require_page_read, AuthUser};
use crate::board::{extract_mentions, sanitize_rich_text};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub fn workspace_routes() -> Router<AppState> {
    Router::new().route("/", get(list).post(create)).route("/counts", get(counts))
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/:id", put(update).delete(remove)).route("/:id/resolve", post(resolve))
}

#[derive(Serialize, sqlx::FromRow)]
struct CommentDto {
    id: Uuid,
    target_kind: String,
    target_id: Uuid,
    parent_id: Option<Uuid>,
    user_id: Option<Uuid>,
    user_name: Option<String>,
    avatar_url: Option<String>,
    content: String,
    quote: Option<String>,
    resolved_at: Option<DateTime<Utc>>,
    resolved_by: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

const SELECT: &str = "SELECT c.id, c.target_kind, c.target_id, c.parent_id, c.user_id, u.name AS user_name, u.avatar_url,
       c.content, c.quote, c.resolved_at, c.resolved_by, c.created_at, c.updated_at
FROM item_comments c LEFT JOIN users u ON u.id = c.user_id";

#[derive(Deserialize)]
struct TargetQuery {
    kind: String,
    id: Uuid,
}

/// Vérifie que la cible existe dans l'espace et qu'elle est lisible ; renvoie son titre et
/// son auteur (destinataire des notifications).
async fn check_target(state: &AppState, user: &AuthUser, ws: Uuid, kind: &str, id: Uuid) -> ApiResult<(String, Option<Uuid>)> {
    match kind {
        "page" => {
            let (page_ws, _, _) = require_page_read(&state.pool, user, id).await?;
            if page_ws != ws {
                return Err(ApiError::NotFound);
            }
            let (title, author): (String, Option<Uuid>) =
                sqlx::query_as("SELECT title, created_by FROM pages WHERE id = $1").bind(id).fetch_one(&state.pool).await?;
            Ok((title, author))
        }
        "event" => {
            require_member(&state.pool, user, ws).await?;
            let row: Option<(String, Option<Uuid>)> = sqlx::query_as("SELECT summary, created_by FROM workspace_events WHERE id = $1 AND workspace_id = $2")
                .bind(id)
                .bind(ws)
                .fetch_optional(&state.pool)
                .await?;
            row.ok_or(ApiError::NotFound)
        }
        _ => Err(ApiError::bad_request("invalid_kind")),
    }
}

async fn list(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>, Query(q): Query<TargetQuery>) -> ApiResult<Json<Vec<CommentDto>>> {
    check_target(&state, &user, ws, &q.kind, q.id).await?;
    let rows: Vec<CommentDto> = sqlx::query_as(&format!("{SELECT} WHERE c.workspace_id = $1 AND c.target_kind = $2 AND c.target_id = $3 ORDER BY c.created_at"))
        .bind(ws)
        .bind(&q.kind)
        .bind(q.id)
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(rows))
}

#[derive(Deserialize)]
struct CountsQuery {
    kind: String,
}

/// Nombre de commentaires non résolus par élément (pastilles dans les listes).
async fn counts(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>, Query(q): Query<CountsQuery>) -> ApiResult<Json<serde_json::Value>> {
    require_member(&state.pool, &user, ws).await?;
    let rows: Vec<(Uuid, i64)> = sqlx::query_as(
        "SELECT target_id, count(*) FROM item_comments WHERE workspace_id = $1 AND target_kind = $2 AND resolved_at IS NULL AND parent_id IS NULL GROUP BY target_id",
    )
    .bind(ws)
    .bind(&q.kind)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(serde_json::Value::Object(rows.into_iter().map(|(id, n)| (id.to_string(), n.into())).collect())))
}

#[derive(Deserialize)]
struct CreateReq {
    kind: String,
    id: Uuid,
    content: String,
    parent_id: Option<Uuid>,
    quote: Option<String>,
}

fn clean_content(raw: &str) -> ApiResult<String> {
    if raw.len() > 100_000 {
        return Err(ApiError::bad_request("comment_too_long"));
    }
    let clean = sanitize_rich_text(raw);
    if clean.replace("<p></p>", "").trim().is_empty() {
        return Err(ApiError::bad_request("empty_comment"));
    }
    Ok(clean)
}

async fn changed(state: &AppState, ws: Uuid, kind: &str, id: Uuid) {
    state.emit(ws, serde_json::json!({ "type": "COMMENTS_UPDATED", "kind": kind, "id": id })).await;
}

async fn create(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>, Json(p): Json<CreateReq>) -> ApiResult<Json<CommentDto>> {
    let (title, author) = check_target(&state, &user, ws, &p.kind, p.id).await?;
    let content = clean_content(&p.content)?;
    let quote = p.quote.map(|q| q.trim().chars().take(500).collect::<String>()).filter(|q| !q.is_empty());
    // Réponse : toujours rattachée au commentaire racine du fil.
    let parent = match p.parent_id {
        Some(pid) => {
            let row: Option<(Option<Uuid>,)> = sqlx::query_as("SELECT parent_id FROM item_comments WHERE id = $1 AND target_id = $2")
                .bind(pid)
                .bind(p.id)
                .fetch_optional(&state.pool)
                .await?;
            Some(row.ok_or(ApiError::NotFound)?.0.unwrap_or(pid))
        }
        None => None,
    };
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO item_comments (workspace_id, target_kind, target_id, parent_id, user_id, content, quote) VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
    )
    .bind(ws)
    .bind(&p.kind)
    .bind(p.id)
    .bind(parent)
    .bind(user.id)
    .bind(&content)
    .bind(&quote)
    .fetch_one(&state.pool)
    .await?;

    // Notifications : mentions, puis auteur de l'élément et participants du fil.
    let target = format!("{}:{}", p.kind, p.id);
    let text = crate::ical::html_to_text(&content);
    let snippet: String = text.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(140).collect();
    let data = serde_json::json!({ "title": title, "snippet": snippet, "comment_id": id });
    let mentioned = extract_mentions(&content);
    for m in &mentioned {
        crate::notifications::notify(&state.pool, &state.hub, ws, *m, Some(user.id), "mention", &target, data.clone()).await;
    }
    let mut others: Vec<Uuid> = sqlx::query_scalar(
        "SELECT DISTINCT user_id FROM item_comments WHERE target_id = $1 AND user_id IS NOT NULL AND ($2::uuid IS NULL OR id = $2 OR parent_id = $2)",
    )
    .bind(p.id)
    .bind(parent)
    .fetch_all(&state.pool)
    .await?;
    if let Some(a) = author {
        others.push(a);
    }
    others.sort();
    others.dedup();
    for u in others {
        if !mentioned.contains(&u) {
            crate::notifications::notify(&state.pool, &state.hub, ws, u, Some(user.id), "comment", &target, data.clone()).await;
        }
    }
    changed(&state, ws, &p.kind, p.id).await;
    let row: CommentDto = sqlx::query_as(&format!("{SELECT} WHERE c.id = $1")).bind(id).fetch_one(&state.pool).await?;
    Ok(Json(row))
}

/// Commentaire, après contrôle d'accès à sa cible.
async fn load(state: &AppState, user: &AuthUser, id: Uuid) -> ApiResult<(Uuid, String, Uuid, Option<Uuid>, bool)> {
    let row: Option<(Uuid, String, Uuid, Option<Uuid>)> =
        sqlx::query_as("SELECT workspace_id, target_kind, target_id, user_id FROM item_comments WHERE id = $1").bind(id).fetch_optional(&state.pool).await?;
    let (ws, kind, target, author) = row.ok_or(ApiError::NotFound)?;
    check_target(state, user, ws, &kind, target).await?;
    let admin = require_member(&state.pool, user, ws).await?.is_admin();
    Ok((ws, kind, target, author, admin))
}

#[derive(Deserialize)]
struct UpdateReq {
    content: String,
}

async fn update(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(p): Json<UpdateReq>) -> ApiResult<StatusCode> {
    let (ws, kind, target, author, _) = load(&state, &user, id).await?;
    if author != Some(user.id) {
        return Err(ApiError::Forbidden);
    }
    let content = clean_content(&p.content)?;
    sqlx::query("UPDATE item_comments SET content = $2, updated_at = NOW() WHERE id = $1").bind(id).bind(content).execute(&state.pool).await?;
    changed(&state, ws, &kind, target).await;
    Ok(StatusCode::NO_CONTENT)
}

async fn remove(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<StatusCode> {
    let (ws, kind, target, author, admin) = load(&state, &user, id).await?;
    if author != Some(user.id) && !admin {
        return Err(ApiError::Forbidden);
    }
    sqlx::query("DELETE FROM item_comments WHERE id = $1").bind(id).execute(&state.pool).await?;
    changed(&state, ws, &kind, target).await;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct ResolveReq {
    resolved: bool,
}

async fn resolve(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(p): Json<ResolveReq>) -> ApiResult<StatusCode> {
    let (ws, kind, target, _, _) = load(&state, &user, id).await?;
    sqlx::query("UPDATE item_comments SET resolved_at = CASE WHEN $2 THEN NOW() END, resolved_by = CASE WHEN $2 THEN $3 END WHERE id = $1 AND parent_id IS NULL")
        .bind(id)
        .bind(p.resolved)
        .bind(user.id)
        .execute(&state.pool)
        .await?;
    changed(&state, ws, &kind, target).await;
    Ok(StatusCode::NO_CONTENT)
}

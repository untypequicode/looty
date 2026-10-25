//! Fil d'activité de l'espace (ou d'un membre) : actions sur les tâches, commentaires et
//! modifications des pages, du plus récent au plus ancien. Les pages inaccessibles au
//! lecteur (droits restreints) n'y figurent pas.

use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::access::{require_member, visible_pages, AuthUser, PageAccess};
use crate::error::ApiResult;
use crate::state::AppState;

pub fn workspace_routes() -> Router<AppState> {
    Router::new().route("/", get(feed))
}

/// Enregistre une action sur une page. Les modifications de contenu d'une même
/// personne sont regroupées par quart d'heure (l'éditeur enregistre en continu).
pub async fn log_page(pool: &sqlx::PgPool, page_id: Uuid, workspace_id: Uuid, user_id: Uuid, kind: &str) {
    let res = if kind == "edit" {
        sqlx::query(
            "WITH recent AS (
                UPDATE page_activities SET created_at = NOW()
                WHERE id = (SELECT id FROM page_activities WHERE page_id = $1 AND user_id = $3 AND kind = 'edit' AND created_at > NOW() - interval '15 minutes'
                            ORDER BY created_at DESC LIMIT 1)
                RETURNING id
             )
             INSERT INTO page_activities (page_id, workspace_id, user_id, kind)
             SELECT $1, $2, $3, 'edit' WHERE NOT EXISTS (SELECT 1 FROM recent)",
        )
        .bind(page_id)
        .bind(workspace_id)
        .bind(user_id)
        .execute(pool)
        .await
    } else {
        sqlx::query("INSERT INTO page_activities (page_id, workspace_id, user_id, kind) VALUES ($1, $2, $3, $4)")
            .bind(page_id)
            .bind(workspace_id)
            .bind(user_id)
            .bind(kind)
            .execute(pool)
            .await
    };
    if let Err(e) = res {
        tracing::debug!("page activity: {e}");
    }
}

#[derive(Deserialize)]
struct FeedQuery {
    user: Option<Uuid>,
    before: Option<DateTime<Utc>>,
    limit: Option<i64>,
}

#[derive(Serialize)]
struct Item {
    id: Uuid,
    /// task | comment | page
    source: &'static str,
    #[serde(rename = "type")]
    kind: String,
    created_at: DateTime<Utc>,
    user_id: Uuid,
    user_name: Option<String>,
    target_id: Uuid,
    title: String,
    icon: Option<String>,
    board_id: Option<Uuid>,
    board_name: Option<String>,
    old_value: Option<serde_json::Value>,
    new_value: Option<serde_json::Value>,
    snippet: Option<String>,
}

type TaskRow = (Uuid, String, DateTime<Utc>, Uuid, Option<String>, Uuid, String, Uuid, String, Option<serde_json::Value>, Option<serde_json::Value>);
type CommentRow = (Uuid, DateTime<Utc>, Uuid, Option<String>, Uuid, String, Uuid, String, String);
type PageRow = (Uuid, String, DateTime<Utc>, Uuid, Option<String>, Uuid, String, Option<String>);

async fn feed(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>, Query(q): Query<FeedQuery>) -> ApiResult<Json<serde_json::Value>> {
    let role = require_member(&state.pool, &user, ws).await?;
    // Tâches et commentaires de tâches : seulement avec l'accès aux tableaux.
    let see_boards = crate::access::member_perms(&state.pool, user.id, ws).await?.is_some_and(|(_, p)| p.boards > crate::access::level::NONE);
    let limit = q.limit.unwrap_or(50).clamp(1, 100);
    let before = q.before.unwrap_or_else(Utc::now);
    let who = q.user.map(|u| u.to_string());

    let tasks: Vec<TaskRow> = sqlx::query_as(
        "SELECT a.id, a.activity_type, a.created_at, a.user_id, u.name, t.id, t.title, b.id, b.name, a.old_value, a.new_value
         FROM task_activities a
         JOIN tasks t ON t.id = a.task_id
         JOIN board_columns c ON c.id = t.column_id JOIN boards b ON b.id = c.board_id
         LEFT JOIN users u ON u.id = a.user_id
         WHERE b.workspace_id = $1 AND a.created_at < $2 AND ($3::uuid IS NULL OR a.user_id = $3) AND $5
         ORDER BY a.created_at DESC LIMIT $4",
    )
    .bind(ws)
    .bind(before)
    .bind(q.user)
    .bind(limit)
    .bind(see_boards)
    .fetch_all(&state.pool)
    .await?;

    let comments: Vec<CommentRow> = sqlx::query_as(
        "SELECT cm.id, cm.created_at, cm.user_id::uuid, u.name, t.id, t.title, b.id, b.name, cm.content
         FROM task_comments cm
         JOIN tasks t ON t.id = cm.task_id
         JOIN board_columns c ON c.id = t.column_id JOIN boards b ON b.id = c.board_id
         LEFT JOIN users u ON u.id::text = cm.user_id::text
         WHERE b.workspace_id = $1 AND cm.created_at < $2 AND ($3::text IS NULL OR cm.user_id::text = $3) AND $5
         ORDER BY cm.created_at DESC LIMIT $4",
    )
    .bind(ws)
    .bind(before)
    .bind(&who)
    .bind(limit)
    .bind(see_boards)
    .fetch_all(&state.pool)
    .await?;

    let visible = visible_pages(&state.pool, &user, ws, role).await?;
    let pages: Vec<PageRow> = sqlx::query_as(
        "SELECT pa.id, pa.kind, pa.created_at, pa.user_id, u.name, p.id, p.title, p.icon
         FROM page_activities pa
         JOIN pages_all p ON p.id = pa.page_id
         LEFT JOIN users u ON u.id = pa.user_id
         WHERE pa.workspace_id = $1 AND pa.created_at < $2 AND ($3::uuid IS NULL OR pa.user_id = $3)
         ORDER BY pa.created_at DESC LIMIT $4",
    )
    .bind(ws)
    .bind(before)
    .bind(q.user)
    .bind(limit * 2)
    .fetch_all(&state.pool)
    .await?;

    let mut items: Vec<Item> = Vec::with_capacity(tasks.len() + comments.len() + pages.len());
    for (id, kind, at, uid, name, tid, title, bid, bname, old, new) in tasks {
        items.push(Item { id, source: "task", kind, created_at: at, user_id: uid, user_name: name, target_id: tid, title, icon: None, board_id: Some(bid), board_name: Some(bname), old_value: old, new_value: new, snippet: None });
    }
    for (id, at, uid, name, tid, title, bid, bname, content) in comments {
        let text = crate::ical::html_to_text(&content);
        let snippet: String = text.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(160).collect();
        items.push(Item { id, source: "comment", kind: "comment".into(), created_at: at, user_id: uid, user_name: name, target_id: tid, title, icon: None, board_id: Some(bid), board_name: Some(bname), old_value: None, new_value: None, snippet: Some(snippet) });
    }
    for (id, kind, at, uid, name, pid, title, icon) in pages {
        // Page supprimée (corbeille) : seule la suppression reste visible, sans lien.
        let readable = matches!(visible.get(&pid), Some(PageAccess::Read | PageAccess::Write));
        if !readable && kind != "delete" {
            continue;
        }
        if !readable && !role.is_admin() {
            continue;
        }
        items.push(Item { id, source: "page", kind, created_at: at, user_id: uid, user_name: name, target_id: pid, title, icon, board_id: None, board_name: None, old_value: None, new_value: None, snippet: None });
    }
    items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    let has_more = items.len() as i64 > limit;
    items.truncate(limit as usize);
    Ok(Json(serde_json::json!({ "items": items, "has_more": has_more })))
}

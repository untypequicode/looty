//! Corbeille des tâches et des pages.
//!
//! Supprimer une tâche ou une page la place dans la corbeille (avec ses sous-tâches ou
//! sous-pages) pendant la durée choisie par l'espace (30 jours par défaut, 0 = sans
//! corbeille). Les vues `tasks` et `pages` ne montrent que les éléments non supprimés ;
//! ce module travaille sur les tables `tasks_all` et `pages_all` (cf. migration).
//!
//! Droits : les tâches de la corbeille sont visibles par les membres pouvant écrire ; les
//! pages (qui peuvent avoir des accès restreints) par les administrateurs et la personne
//! qui les a supprimées. Restaurer : membre pouvant écrire ; supprimer définitivement :
//! administrateur ou auteur de la suppression.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post, put},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

use crate::access::{require_member, require_writer, task_ctx, AuthUser};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub async fn retention_days(pool: &sqlx::PgPool, workspace_id: Uuid) -> ApiResult<i32> {
    Ok(sqlx::query_scalar("SELECT trash_retention_days FROM workspaces WHERE id = $1").bind(workspace_id).fetch_one(pool).await?)
}

/// Met une tâche (et ses sous-tâches) à la corbeille, ou la supprime si l'espace n'a pas
/// de corbeille. Sans contrôle d'accès : l'appelant l'a fait.
pub async fn trash_task(state: &AppState, task_id: Uuid, by: Option<Uuid>) -> ApiResult<()> {
    let ctx = task_ctx(&state.pool, task_id).await?;
    let ids: Vec<Uuid> = sqlx::query_scalar(
        "WITH RECURSIVE sub AS (SELECT id FROM tasks WHERE id = $1 UNION ALL SELECT t.id FROM tasks t JOIN sub ON t.parent_id = sub.id)
         SELECT id FROM sub",
    )
    .bind(task_id)
    .fetch_all(&state.pool)
    .await?;
    if retention_days(&state.pool, ctx.workspace_id).await? == 0 {
        purge_tasks(state, &ids).await?;
    } else {
        let mut tx = state.pool.begin().await?;
        sqlx::query("UPDATE tasks_all SET deleted_at = NOW(), deleted_by = $2, trash_batch = $3 WHERE id = ANY($1)")
            .bind(&ids)
            .bind(by)
            .bind(Uuid::new_v4())
            .execute(&mut *tx)
            .await?;
        // Retirée des calendriers CalDAV externes (le déclencheur met la suppression en file).
        sqlx::query("DELETE FROM caldav_task_links WHERE task_id = ANY($1)").bind(&ids).execute(&mut *tx).await?;
        tx.commit().await?;
    }
    for id in ids.iter().rev() {
        state
            .emit(ctx.workspace_id, serde_json::json!({ "type": "TASK_DELETED", "task_id": id, "column_id": ctx.column_id, "board_id": ctx.board_id }))
            .await;
    }
    Ok(())
}

/// Suppression définitive (fichiers joints compris).
async fn purge_tasks(state: &AppState, ids: &[Uuid]) -> ApiResult<()> {
    if ids.is_empty() {
        return Ok(());
    }
    let files: Vec<String> = sqlx::query_scalar(
        "WITH RECURSIVE sub AS (SELECT id FROM tasks_all WHERE id = ANY($1) UNION SELECT t.id FROM tasks_all t JOIN sub ON t.parent_id = sub.id)
         SELECT file_path FROM task_attachments WHERE task_id IN (SELECT id FROM sub)",
    )
    .bind(ids)
    .fetch_all(&state.pool)
    .await?;
    sqlx::query("DELETE FROM tasks_all WHERE id = ANY($1)").bind(ids).execute(&state.pool).await?;
    for f in files {
        crate::upload::remove_upload(&f).await;
    }
    Ok(())
}

/// Met une page (et ses sous-pages) à la corbeille. Sans contrôle d'accès.
pub async fn trash_page(state: &AppState, workspace_id: Uuid, page_id: Uuid, by: Option<Uuid>) -> ApiResult<()> {
    if let Some(u) = by {
        crate::activity::log_page(&state.pool, page_id, workspace_id, u, "delete").await;
    }
    let ids: Vec<Uuid> = sqlx::query_scalar(
        "WITH RECURSIVE sub AS (SELECT id FROM pages WHERE id = $1 UNION ALL SELECT p.id FROM pages p JOIN sub ON p.parent_id = sub.id)
         SELECT id FROM sub",
    )
    .bind(page_id)
    .fetch_all(&state.pool)
    .await?;
    if retention_days(&state.pool, workspace_id).await? == 0 {
        sqlx::query("DELETE FROM pages_all WHERE id = $1").bind(page_id).execute(&state.pool).await?;
    } else {
        sqlx::query("UPDATE pages_all SET deleted_at = NOW(), deleted_by = $2, trash_batch = $3 WHERE id = ANY($1)")
            .bind(&ids)
            .bind(by)
            .bind(Uuid::new_v4())
            .execute(&state.pool)
            .await?;
    }
    for id in ids.iter().rev() {
        state.emit(workspace_id, serde_json::json!({ "type": "PAGE_DELETED", "page_id": id })).await;
    }
    Ok(())
}

/// Vidage automatique : éléments plus anciens que la durée de conservation de l'espace.
pub fn start_purge_loop(state: AppState) {
    tokio::spawn(async move {
        loop {
            let expired: Vec<Uuid> = sqlx::query_scalar(
                "SELECT t.id FROM tasks_all t
                 JOIN board_columns c ON c.id = t.column_id JOIN boards b ON b.id = c.board_id JOIN workspaces w ON w.id = b.workspace_id
                 WHERE t.deleted_at < NOW() - make_interval(days => w.trash_retention_days)",
            )
            .fetch_all(&state.pool)
            .await
            .unwrap_or_default();
            if let Err(e) = purge_tasks(&state, &expired).await {
                tracing::warn!("trash purge (tasks): {e:?}");
            }
            if let Err(e) = sqlx::query(
                "DELETE FROM pages_all p USING workspaces w
                 WHERE p.workspace_id = w.id AND p.deleted_at < NOW() - make_interval(days => w.trash_retention_days)",
            )
            .execute(&state.pool)
            .await
            {
                tracing::warn!("trash purge (pages): {e:?}");
            }
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
    });
}

// ---------------------------------------------------------------------------
// API
// ---------------------------------------------------------------------------

pub fn workspace_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list).delete(empty))
        .route("/settings", put(update_settings))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/tasks/:id/restore", post(restore_task))
        .route("/tasks/:id", delete(delete_task))
        .route("/pages/:id/restore", post(restore_page))
        .route("/pages/:id", delete(delete_page))
}

#[derive(Serialize, sqlx::FromRow)]
struct TrashedTask {
    id: Uuid,
    title: String,
    board_id: Uuid,
    board_name: String,
    deleted_at: DateTime<Utc>,
    deleted_by: Option<Uuid>,
    deleted_by_name: Option<String>,
    subtasks: i64,
    expires_at: DateTime<Utc>,
}

#[derive(Serialize, sqlx::FromRow)]
struct TrashedPage {
    id: Uuid,
    title: String,
    icon: Option<String>,
    parent_title: Option<String>,
    deleted_at: DateTime<Utc>,
    deleted_by: Option<Uuid>,
    deleted_by_name: Option<String>,
    subpages: i64,
    expires_at: DateTime<Utc>,
}

async fn list(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>) -> ApiResult<Json<serde_json::Value>> {
    let role = require_member(&state.pool, &user, ws).await?;
    let days = retention_days(&state.pool, ws).await?;
    let tasks: Vec<TrashedTask> = if role.can_write() {
        sqlx::query_as(
            "SELECT t.id, t.title, b.id AS board_id, b.name AS board_name, t.deleted_at, t.deleted_by, u.name AS deleted_by_name,
                (SELECT count(*) - 1 FROM tasks_all x WHERE x.trash_batch = t.trash_batch) AS subtasks,
                t.deleted_at + make_interval(days => $2) AS expires_at
             FROM tasks_all t
             JOIN board_columns c ON c.id = t.column_id JOIN boards b ON b.id = c.board_id
             LEFT JOIN users u ON u.id = t.deleted_by
             WHERE b.workspace_id = $1 AND t.deleted_at IS NOT NULL
               AND NOT EXISTS (SELECT 1 FROM tasks_all p WHERE p.id = t.parent_id AND p.trash_batch = t.trash_batch)
             ORDER BY t.deleted_at DESC LIMIT 500",
        )
        .bind(ws)
        .bind(days)
        .fetch_all(&state.pool)
        .await?
    } else {
        vec![]
    };
    let pages: Vec<TrashedPage> = sqlx::query_as(
        "SELECT p.id, p.title, p.icon, pp.title AS parent_title, p.deleted_at, p.deleted_by, u.name AS deleted_by_name,
            (SELECT count(*) - 1 FROM pages_all x WHERE x.trash_batch = p.trash_batch) AS subpages,
            p.deleted_at + make_interval(days => $2) AS expires_at
         FROM pages_all p
         LEFT JOIN pages_all pp ON pp.id = p.parent_id
         LEFT JOIN users u ON u.id = p.deleted_by
         WHERE p.workspace_id = $1 AND p.deleted_at IS NOT NULL
           AND NOT EXISTS (SELECT 1 FROM pages_all q WHERE q.id = p.parent_id AND q.trash_batch = p.trash_batch)
           AND ($3 OR p.deleted_by = $4)
         ORDER BY p.deleted_at DESC LIMIT 500",
    )
    .bind(ws)
    .bind(days)
    .bind(role.is_admin())
    .bind(user.id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(serde_json::json!({ "retention_days": days, "can_manage": role.is_admin(), "tasks": tasks, "pages": pages })))
}

#[derive(Deserialize)]
struct SettingsReq {
    retention_days: i32,
}

async fn update_settings(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>, Json(p): Json<SettingsReq>) -> ApiResult<StatusCode> {
    crate::access::require_ws_admin(&state.pool, &user, ws).await?;
    if !(0..=365).contains(&p.retention_days) {
        return Err(ApiError::bad_request("invalid_value"));
    }
    sqlx::query("UPDATE workspaces SET trash_retention_days = $2 WHERE id = $1").bind(ws).bind(p.retention_days).execute(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn empty(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>) -> ApiResult<StatusCode> {
    crate::access::require_ws_admin(&state.pool, &user, ws).await?;
    let ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT t.id FROM tasks_all t JOIN board_columns c ON c.id = t.column_id JOIN boards b ON b.id = c.board_id
         WHERE b.workspace_id = $1 AND t.deleted_at IS NOT NULL",
    )
    .bind(ws)
    .fetch_all(&state.pool)
    .await?;
    purge_tasks(&state, &ids).await?;
    sqlx::query("DELETE FROM pages_all WHERE workspace_id = $1 AND deleted_at IS NOT NULL").bind(ws).execute(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(sqlx::FromRow)]
struct TaskEntry {
    workspace_id: Uuid,
    parent_id: Option<Uuid>,
    trash_batch: Option<Uuid>,
    deleted_by: Option<Uuid>,
}

async fn trashed_task(state: &AppState, id: Uuid) -> ApiResult<TaskEntry> {
    sqlx::query_as(
        "SELECT b.workspace_id, t.parent_id, t.trash_batch, t.deleted_by FROM tasks_all t
         JOIN board_columns c ON c.id = t.column_id JOIN boards b ON b.id = c.board_id
         WHERE t.id = $1 AND t.deleted_at IS NOT NULL",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(ApiError::NotFound)
}

async fn restore_task(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<StatusCode> {
    let e = trashed_task(&state, id).await?;
    require_writer(&state.pool, &user, e.workspace_id).await?;
    let ids: Vec<Uuid> = sqlx::query_scalar(
        "UPDATE tasks_all SET deleted_at = NULL, deleted_by = NULL, trash_batch = NULL
         WHERE trash_batch = $1 OR id = $2 RETURNING id",
    )
    .bind(e.trash_batch)
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    // Tâche parente toujours à la corbeille : la tâche revient au premier niveau.
    if let Some(parent) = e.parent_id {
        let visible: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tasks WHERE id = $1)").bind(parent).fetch_one(&state.pool).await?;
        if !visible {
            sqlx::query("UPDATE tasks_all SET parent_id = NULL WHERE id = $1").bind(id).execute(&state.pool).await?;
        }
    }
    for tid in ids {
        crate::board::emit_task(&state, e.workspace_id, "TASK_CREATED", tid).await;
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_task(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<StatusCode> {
    let e = trashed_task(&state, id).await?;
    let role = require_member(&state.pool, &user, e.workspace_id).await?;
    if !role.is_admin() && e.deleted_by != Some(user.id) {
        return Err(ApiError::Forbidden);
    }
    let ids: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM tasks_all WHERE trash_batch = $1 OR id = $2").bind(e.trash_batch).bind(id).fetch_all(&state.pool).await?;
    purge_tasks(&state, &ids).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(sqlx::FromRow)]
struct PageEntry {
    workspace_id: Uuid,
    parent_id: Option<Uuid>,
    trash_batch: Option<Uuid>,
    deleted_by: Option<Uuid>,
}

async fn trashed_page(state: &AppState, user: &AuthUser, id: Uuid) -> ApiResult<(PageEntry, crate::access::Role)> {
    let e: PageEntry = sqlx::query_as("SELECT workspace_id, parent_id, trash_batch, deleted_by FROM pages_all WHERE id = $1 AND deleted_at IS NOT NULL")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(ApiError::NotFound)?;
    let role = require_member(&state.pool, user, e.workspace_id).await?;
    // Pages : visibles des administrateurs et de l'auteur de la suppression seulement.
    if !role.is_admin() && e.deleted_by != Some(user.id) {
        return Err(ApiError::NotFound);
    }
    Ok((e, role))
}

async fn restore_page(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<StatusCode> {
    let (e, role) = trashed_page(&state, &user, id).await?;
    if !role.can_write() {
        return Err(ApiError::Forbidden);
    }
    let ids: Vec<Uuid> = sqlx::query_scalar(
        "UPDATE pages_all SET deleted_at = NULL, deleted_by = NULL, trash_batch = NULL WHERE trash_batch = $1 OR id = $2 RETURNING id",
    )
    .bind(e.trash_batch)
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    if let Some(parent) = e.parent_id {
        let visible: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pages WHERE id = $1)").bind(parent).fetch_one(&state.pool).await?;
        if !visible {
            sqlx::query("UPDATE pages_all SET parent_id = NULL WHERE id = $1").bind(id).execute(&state.pool).await?;
        }
    }
    crate::activity::log_page(&state.pool, id, e.workspace_id, user.id, "restore").await;
    let pages: Vec<crate::page::Page> = sqlx::query_as("SELECT * FROM pages WHERE id = ANY($1)").bind(&ids).fetch_all(&state.pool).await?;
    for mut p in pages {
        p.content = None;
        state.emit(e.workspace_id, serde_json::json!({ "type": "PAGE_CREATED", "page": p })).await;
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_page(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<StatusCode> {
    let _ = trashed_page(&state, &user, id).await?;
    sqlx::query("DELETE FROM pages_all WHERE id = $1").bind(id).execute(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT)
}

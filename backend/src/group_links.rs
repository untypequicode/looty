//! Groupes reliés (GLOSSARY) : un groupe ajouté à une tâche ou à un événement y reste
//! relié. Ajouter quelqu'un au groupe l'assigne (ou le mentionne) ; l'en retirer le
//! désassigne, sauf s'il l'était aussi individuellement ou par un autre groupe relié
//! (`via_group` faux). Les tâches terminées ne bougent plus. Supprimer le groupe coupe
//! le lien et laisse les personnes en place.

use axum::{
    extract::{Path, State},
    http::StatusCode,
};
use uuid::Uuid;

use crate::access::{task_ctx, AuthUser};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

/// Membres actuels (de l'espace) d'un groupe de l'espace ; NotFound si le groupe n'y est pas.
async fn group_people(state: &AppState, ws: Uuid, group: Uuid) -> ApiResult<Vec<Uuid>> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM member_groups WHERE id = $1 AND workspace_id = $2)")
        .bind(group)
        .bind(ws)
        .fetch_one(&state.pool)
        .await?;
    if !exists {
        return Err(ApiError::NotFound);
    }
    Ok(sqlx::query_scalar(
        "SELECT gm.user_id FROM member_group_members gm
         JOIN workspace_members wm ON wm.workspace_id = $2 AND wm.user_id = gm.user_id::text
         WHERE gm.group_id = $1",
    )
    .bind(group)
    .bind(ws)
    .fetch_all(&state.pool)
    .await?)
}

/// Assigne des personnes à une tâche au titre d'un groupe ; prévient les nouvelles.
async fn assign_via_group(state: &AppState, ws: Uuid, task: Uuid, people: &[Uuid], actor: Uuid) -> ApiResult<()> {
    let (title, board_id): (String, Uuid) = sqlx::query_as(
        "SELECT t.title, c.board_id FROM tasks t JOIN board_columns c ON c.id = t.column_id WHERE t.id = $1",
    )
    .bind(task)
    .fetch_one(&state.pool)
    .await?;
    for &u in people {
        let n = sqlx::query("INSERT INTO task_assignees (task_id, user_id, via_group) VALUES ($1, $2, TRUE) ON CONFLICT DO NOTHING")
            .bind(task)
            .bind(u.to_string())
            .execute(&state.pool)
            .await?
            .rows_affected();
        if n > 0 {
            crate::board::log_activity(&state.pool, task, actor, "add_assignee", None, Some(serde_json::json!({ "user_id": u.to_string() }))).await;
            if u != actor {
                crate::notifications::notify(&state.pool, &state.hub, ws, u, Some(actor), "assigned", &format!("task:{task}"),
                    serde_json::json!({ "title": title, "board_id": board_id })).await;
            }
        }
    }
    Ok(())
}

/// Retire d'une tâche les personnes assignées seulement par un groupe et qui ne sont dans
/// aucun groupe encore relié (`only` : limiter à ces personnes).
async fn drop_orphans(state: &AppState, task: Uuid, only: Option<&[Uuid]>) -> ApiResult<()> {
    let only: Option<Vec<String>> = only.map(|o| o.iter().map(Uuid::to_string).collect());
    sqlx::query(
        "DELETE FROM task_assignees ta WHERE ta.task_id = $1 AND ta.via_group
           AND ($2::text[] IS NULL OR ta.user_id = ANY($2))
           AND NOT EXISTS (SELECT 1 FROM task_groups tg JOIN member_group_members gm ON gm.group_id = tg.group_id
                            WHERE tg.task_id = ta.task_id AND gm.user_id::text = ta.user_id)",
    )
    .bind(task)
    .bind(only)
    .execute(&state.pool)
    .await?;
    Ok(())
}

/// Relie un groupe à une tâche : ses membres du moment y sont assignés.
pub async fn add_task_group(State(state): State<AppState>, user: AuthUser, Path((task, group)): Path<(Uuid, Uuid)>) -> ApiResult<StatusCode> {
    let ctx = task_ctx(&state.pool, task).await?;
    crate::access::require_perm(&state.pool, &user, ctx.workspace_id, |p| p.boards >= crate::access::level::EDIT).await?;
    let people = group_people(&state, ctx.workspace_id, group).await?;
    sqlx::query("INSERT INTO task_groups (task_id, group_id) VALUES ($1, $2) ON CONFLICT DO NOTHING").bind(task).bind(group).execute(&state.pool).await?;
    assign_via_group(&state, ctx.workspace_id, task, &people, user.id).await?;
    crate::board::emit_task(&state, ctx.workspace_id, "TASK_UPDATED", task).await;
    Ok(StatusCode::NO_CONTENT)
}

/// Délie un groupe d'une tâche : les personnes venues seulement par lui sont désassignées.
pub async fn remove_task_group(State(state): State<AppState>, user: AuthUser, Path((task, group)): Path<(Uuid, Uuid)>) -> ApiResult<StatusCode> {
    let ctx = task_ctx(&state.pool, task).await?;
    crate::access::require_perm(&state.pool, &user, ctx.workspace_id, |p| p.boards >= crate::access::level::EDIT).await?;
    sqlx::query("DELETE FROM task_groups WHERE task_id = $1 AND group_id = $2").bind(task).bind(group).execute(&state.pool).await?;
    drop_orphans(&state, task, None).await?;
    crate::board::emit_task(&state, ctx.workspace_id, "TASK_UPDATED", task).await;
    Ok(StatusCode::NO_CONTENT)
}

/// Relie des groupes à une tâche qui vient d'être créée.
pub async fn link_new_task(state: &AppState, ws: Uuid, task: Uuid, groups: &[Uuid], actor: Uuid) -> ApiResult<()> {
    for &g in groups.iter().take(50) {
        let Ok(people) = group_people(state, ws, g).await else { continue };
        sqlx::query("INSERT INTO task_groups (task_id, group_id) VALUES ($1, $2) ON CONFLICT DO NOTHING").bind(task).bind(g).execute(&state.pool).await?;
        assign_via_group(state, ws, task, &people, actor).await?;
    }
    Ok(())
}

/// Membres d'un groupe modifiés : les tâches (non terminées) et événements reliés suivent.
pub async fn group_members_changed(state: &AppState, ws: Uuid, group: Uuid, added: &[Uuid], removed: &[Uuid], actor: Uuid) -> ApiResult<()> {
    if added.is_empty() && removed.is_empty() {
        return Ok(());
    }
    let tasks: Vec<Uuid> = sqlx::query_scalar(
        "SELECT t.id FROM task_groups tg JOIN tasks t ON t.id = tg.task_id WHERE tg.group_id = $1 AND NOT t.is_completed",
    )
    .bind(group)
    .fetch_all(&state.pool)
    .await?;
    for &task in &tasks {
        assign_via_group(state, ws, task, added, actor).await?;
        if !removed.is_empty() {
            drop_orphans(state, task, Some(removed)).await?;
        }
        crate::board::emit_task(state, ws, "TASK_UPDATED", task).await;
    }

    let events: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT e.id, e.summary FROM event_groups eg JOIN workspace_events e ON e.id = eg.event_id WHERE eg.group_id = $1",
    )
    .bind(group)
    .fetch_all(&state.pool)
    .await?;
    for (event, title) in &events {
        for &u in added {
            let n = sqlx::query("INSERT INTO event_members (event_id, user_id, via_group) VALUES ($1, $2, TRUE) ON CONFLICT DO NOTHING")
                .bind(event)
                .bind(u)
                .execute(&state.pool)
                .await?
                .rows_affected();
            if n > 0 && u != actor {
                crate::notifications::notify(&state.pool, &state.hub, ws, u, Some(actor), "event_tag", &format!("event:{event}"), serde_json::json!({ "title": title })).await;
            }
        }
        sqlx::query(
            "DELETE FROM event_members em WHERE em.event_id = $1 AND em.via_group AND em.user_id = ANY($2)
               AND NOT EXISTS (SELECT 1 FROM event_groups eg JOIN member_group_members gm ON gm.group_id = eg.group_id
                                WHERE eg.event_id = em.event_id AND gm.user_id = em.user_id)",
        )
        .bind(event)
        .bind(removed)
        .execute(&state.pool)
        .await?;
    }
    if !events.is_empty() {
        state.emit(ws, serde_json::json!({ "type": "CALENDAR_EVENTS_UPDATED" })).await;
    }
    Ok(())
}

/// Groupe supprimé (avant la suppression) : les personnes venues par lui restent, comme
/// assignées individuellement, sur les tâches et événements qu'il ne relie plus.
pub async fn group_deleted(state: &AppState, group: Uuid) -> ApiResult<()> {
    sqlx::query(
        "UPDATE task_assignees ta SET via_group = FALSE FROM task_groups tg
          WHERE tg.group_id = $1 AND tg.task_id = ta.task_id AND ta.via_group
            AND NOT EXISTS (SELECT 1 FROM task_groups o JOIN member_group_members gm ON gm.group_id = o.group_id
                             WHERE o.task_id = ta.task_id AND o.group_id <> $1 AND gm.user_id::text = ta.user_id)",
    )
    .bind(group)
    .execute(&state.pool)
    .await?;
    sqlx::query(
        "UPDATE event_members em SET via_group = FALSE FROM event_groups eg
          WHERE eg.group_id = $1 AND eg.event_id = em.event_id AND em.via_group
            AND NOT EXISTS (SELECT 1 FROM event_groups o JOIN member_group_members gm ON gm.group_id = o.group_id
                             WHERE o.event_id = em.event_id AND o.group_id <> $1 AND gm.user_id = em.user_id)",
    )
    .bind(group)
    .execute(&state.pool)
    .await?;
    Ok(())
}

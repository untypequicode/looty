//! Calendriers Looty de l'espace : le calendrier par défaut, des calendriers thématiques et
//! un calendrier par tableau (vue calendrier du tableau). Les événements se déplacent d'un
//! calendrier à l'autre ; supprimer un calendrier renvoie ses événements au calendrier par
//! défaut (rien n'est perdu).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, patch},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::access::{level, require_member, require_perm, AuthUser};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub fn workspace_routes() -> Router<AppState> {
    Router::new().route("/calendars", get(list).post(create)).route("/calendars/{id}", patch(update).delete(remove))
}

#[derive(Serialize, sqlx::FromRow)]
struct LootyCalendar {
    id: Uuid,
    name: String,
    color: String,
    board_id: Option<Uuid>,
    board_name: Option<String>,
    is_default: bool,
    position: i32,
    event_count: i64,
}

async fn list(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>) -> ApiResult<Json<Vec<LootyCalendar>>> {
    require_member(&state.pool, &user, ws).await?;
    let rows: Vec<LootyCalendar> = sqlx::query_as(
        "SELECT c.id, c.name, c.color, c.board_id, b.name AS board_name, c.is_default, c.position,
                (SELECT count(*) FROM workspace_events e WHERE e.calendar_id = c.id) AS event_count
         FROM looty_calendars c LEFT JOIN boards b ON b.id = c.board_id
         WHERE c.workspace_id = $1 ORDER BY c.is_default DESC, c.position, c.created_at",
    )
    .bind(ws)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

#[derive(Deserialize)]
struct CalReq {
    name: Option<String>,
    color: Option<String>,
    /// Créer (ou retrouver) le calendrier d'un tableau.
    board_id: Option<Uuid>,
    position: Option<i32>,
}

fn clean_color(c: &str) -> ApiResult<String> {
    if c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|x| x.is_ascii_hexdigit()) {
        Ok(c.to_string())
    } else {
        Err(ApiError::bad_request("invalid_value"))
    }
}

fn clean_name(n: &str) -> ApiResult<String> {
    let n = n.trim();
    if n.is_empty() || n.chars().count() > 120 {
        return Err(ApiError::bad_request("invalid_name"));
    }
    Ok(n.to_string())
}

async fn changed(state: &AppState, ws: Uuid) {
    state.emit(ws, serde_json::json!({ "type": "CALENDAR_EVENTS_UPDATED" })).await;
}

async fn create(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>, Json(p): Json<CalReq>) -> ApiResult<Json<serde_json::Value>> {
    require_perm(&state.pool, &user, ws, |p| p.calendar >= level::EDIT).await?;
    let color = clean_color(p.color.as_deref().unwrap_or("#4cb782"))?;
    if let Some(board) = p.board_id {
        // Un seul calendrier par tableau : renvoie celui qui existe déjà.
        let board_name: Option<String> = sqlx::query_scalar("SELECT name FROM boards WHERE id = $1 AND workspace_id = $2").bind(board).bind(ws).fetch_optional(&state.pool).await?;
        let board_name = board_name.ok_or(ApiError::NotFound)?;
        if let Some(id) = sqlx::query_scalar::<_, Uuid>("SELECT id FROM looty_calendars WHERE board_id = $1").bind(board).fetch_optional(&state.pool).await? {
            return Ok(Json(serde_json::json!({ "id": id })));
        }
        let name = p.name.as_deref().map(clean_name).transpose()?.unwrap_or(board_name);
        let id: Uuid = sqlx::query_scalar("INSERT INTO looty_calendars (workspace_id, board_id, name, color, created_by) VALUES ($1, $2, $3, $4, $5) RETURNING id")
            .bind(ws)
            .bind(board)
            .bind(name)
            .bind(color)
            .bind(user.id)
            .fetch_one(&state.pool)
            .await?;
        changed(&state, ws).await;
        return Ok(Json(serde_json::json!({ "id": id })));
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM looty_calendars WHERE workspace_id = $1").bind(ws).fetch_one(&state.pool).await?;
    if count >= 100 {
        return Err(ApiError::bad_request("calendar_too_many"));
    }
    let id: Uuid = sqlx::query_scalar("INSERT INTO looty_calendars (workspace_id, name, color, position, created_by) VALUES ($1, $2, $3, $4, $5) RETURNING id")
        .bind(ws)
        .bind(clean_name(p.name.as_deref().unwrap_or(""))?)
        .bind(color)
        .bind(count as i32)
        .bind(user.id)
        .fetch_one(&state.pool)
        .await?;
    changed(&state, ws).await;
    Ok(Json(serde_json::json!({ "id": id })))
}

async fn update(State(state): State<AppState>, user: AuthUser, Path((ws, id)): Path<(Uuid, Uuid)>, Json(p): Json<CalReq>) -> ApiResult<StatusCode> {
    require_perm(&state.pool, &user, ws, |p| p.calendar >= level::EDIT).await?;
    let n = sqlx::query(
        "UPDATE looty_calendars SET name = COALESCE($3, name), color = COALESCE($4, color), position = COALESCE($5, position) WHERE id = $1 AND workspace_id = $2",
    )
    .bind(id)
    .bind(ws)
    .bind(p.name.as_deref().map(clean_name).transpose()?)
    .bind(p.color.as_deref().map(clean_color).transpose()?)
    .bind(p.position)
    .execute(&state.pool)
    .await?
    .rows_affected();
    if n == 0 {
        return Err(ApiError::NotFound);
    }
    changed(&state, ws).await;
    Ok(StatusCode::NO_CONTENT)
}

async fn remove(State(state): State<AppState>, user: AuthUser, Path((ws, id)): Path<(Uuid, Uuid)>) -> ApiResult<StatusCode> {
    require_perm(&state.pool, &user, ws, |p| p.calendar >= level::EDIT).await?;
    let default: bool = sqlx::query_scalar("SELECT is_default FROM looty_calendars WHERE id = $1 AND workspace_id = $2")
        .bind(id)
        .bind(ws)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(ApiError::NotFound)?;
    if default {
        return Err(ApiError::bad_request("default_calendar"));
    }
    let mut tx = state.pool.begin().await?;
    // Les événements rejoignent le calendrier par défaut.
    sqlx::query("UPDATE workspace_events SET calendar_id = (SELECT id FROM looty_calendars WHERE workspace_id = $2 AND is_default) WHERE calendar_id = $1")
        .bind(id)
        .bind(ws)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM looty_calendars WHERE id = $1").bind(id).execute(&mut *tx).await?;
    tx.commit().await?;
    changed(&state, ws).await;
    Ok(StatusCode::NO_CONTENT)
}

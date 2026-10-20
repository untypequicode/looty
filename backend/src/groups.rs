//! Groupes de membres : ensembles nommés de membres d'un espace, sans aucun droit. Ils
//! servent à mentionner plusieurs personnes d'un coup (`@groupe`) et à les ajouter ensemble
//! à une tâche ou un événement (l'interface ajoute alors les membres du moment).
//! Tout membre les voit ; la permission « groupes » permet de les gérer.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::access::{require_member, require_perm, AuthUser};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub fn workspace_routes() -> Router<AppState> {
    Router::new().route("/", get(list).post(create)).route("/{group_id}", put(update).delete(remove))
}

#[derive(Serialize, sqlx::FromRow)]
struct GroupDto {
    id: Uuid,
    name: String,
    color: String,
    /// Membres actuels de l'espace qui font partie du groupe.
    members: Vec<Uuid>,
}

const SELECT: &str = "SELECT g.id, g.name, g.color,
        ARRAY(SELECT gm.user_id FROM member_group_members gm
               JOIN workspace_members wm ON wm.workspace_id = g.workspace_id AND wm.user_id = gm.user_id::text
              WHERE gm.group_id = g.id ORDER BY gm.user_id) AS members
    FROM member_groups g";

async fn list(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>) -> ApiResult<Json<Vec<GroupDto>>> {
    require_member(&state.pool, &user, ws).await?;
    let rows = sqlx::query_as(sqlx::AssertSqlSafe(format!("{SELECT} WHERE g.workspace_id = $1 ORDER BY lower(g.name)")))
        .bind(ws)
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(rows))
}

#[derive(Deserialize)]
struct GroupReq {
    name: Option<String>,
    color: Option<String>,
    members: Option<Vec<Uuid>>,
}

async fn require_manage(state: &AppState, user: &AuthUser, ws: Uuid) -> ApiResult<()> {
    require_perm(&state.pool, user, ws, |p| p.groups || p.admin).await.map(|_| ())
}

fn clean_name(raw: &str) -> ApiResult<String> {
    let name: String = raw.trim().trim_start_matches('@').trim().chars().take(60).collect();
    if name.is_empty() {
        return Err(ApiError::bad_request("name_required"));
    }
    Ok(name)
}

fn clean_color(raw: Option<&str>) -> Option<String> {
    raw.filter(|c| c.len() <= 9 && c.starts_with('#') && c[1..].chars().all(|x| x.is_ascii_hexdigit())).map(str::to_string)
}

async fn set_members(pool: &PgPool, ws: Uuid, group: Uuid, members: &[Uuid]) -> ApiResult<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM member_group_members WHERE group_id = $1").bind(group).execute(&mut *tx).await?;
    // Seuls les membres de l'espace entrent dans le groupe.
    sqlx::query(
        "INSERT INTO member_group_members (group_id, user_id)
         SELECT $1, u FROM unnest($3::uuid[]) AS u
          WHERE EXISTS (SELECT 1 FROM workspace_members wm WHERE wm.workspace_id = $2 AND wm.user_id = u::text)
         ON CONFLICT DO NOTHING",
    )
    .bind(group)
    .bind(ws)
    .bind(members)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

fn unique_violation(e: sqlx::Error) -> ApiError {
    match &e {
        sqlx::Error::Database(d) if d.is_unique_violation() => ApiError::bad_request("name_taken"),
        _ => e.into(),
    }
}

async fn one(pool: &PgPool, ws: Uuid, id: Uuid) -> ApiResult<GroupDto> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!("{SELECT} WHERE g.id = $1 AND g.workspace_id = $2")))
        .bind(id)
        .bind(ws)
        .fetch_optional(pool)
        .await?
        .ok_or(ApiError::NotFound)
}

async fn changed(state: &AppState, ws: Uuid) {
    state.emit(ws, serde_json::json!({ "type": "GROUPS_UPDATED" })).await;
}

async fn create(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>, Json(p): Json<GroupReq>) -> ApiResult<Json<GroupDto>> {
    require_manage(&state, &user, ws).await?;
    let name = clean_name(p.name.as_deref().unwrap_or(""))?;
    let color = clean_color(p.color.as_deref()).unwrap_or_else(|| "#5e6ad2".into());
    let id: Uuid = sqlx::query_scalar("INSERT INTO member_groups (workspace_id, name, color, created_by) VALUES ($1, $2, $3, $4) RETURNING id")
        .bind(ws)
        .bind(&name)
        .bind(&color)
        .bind(user.id)
        .fetch_one(&state.pool)
        .await
        .map_err(unique_violation)?;
    set_members(&state.pool, ws, id, p.members.as_deref().unwrap_or(&[])).await?;
    changed(&state, ws).await;
    Ok(Json(one(&state.pool, ws, id).await?))
}

async fn update(
    State(state): State<AppState>,
    user: AuthUser,
    Path((ws, id)): Path<(Uuid, Uuid)>,
    Json(p): Json<GroupReq>,
) -> ApiResult<Json<GroupDto>> {
    require_manage(&state, &user, ws).await?;
    one(&state.pool, ws, id).await?;
    if let Some(name) = p.name.as_deref() {
        let name = clean_name(name)?;
        sqlx::query("UPDATE member_groups SET name = $2 WHERE id = $1").bind(id).bind(&name).execute(&state.pool).await.map_err(unique_violation)?;
    }
    if let Some(color) = clean_color(p.color.as_deref()) {
        sqlx::query("UPDATE member_groups SET color = $2 WHERE id = $1").bind(id).bind(&color).execute(&state.pool).await?;
    }
    if let Some(members) = p.members.as_deref() {
        set_members(&state.pool, ws, id, members).await?;
    }
    changed(&state, ws).await;
    Ok(Json(one(&state.pool, ws, id).await?))
}

async fn remove(State(state): State<AppState>, user: AuthUser, Path((ws, id)): Path<(Uuid, Uuid)>) -> ApiResult<StatusCode> {
    require_manage(&state, &user, ws).await?;
    let n = sqlx::query("DELETE FROM member_groups WHERE id = $1 AND workspace_id = $2").bind(id).bind(ws).execute(&state.pool).await?.rows_affected();
    if n == 0 {
        return Err(ApiError::NotFound);
    }
    changed(&state, ws).await;
    Ok(StatusCode::NO_CONTENT)
}

/// Personnes visées par des mentions : les membres mentionnés, et les membres des groupes
/// mentionnés (une seule fois chacun). Les identifiants inconnus de l'espace sont ignorés.
pub async fn mentioned_people(pool: &PgPool, ws: Uuid, ids: &[Uuid]) -> ApiResult<Vec<Uuid>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let people: Vec<Uuid> = sqlx::query_scalar(
        "SELECT DISTINCT p FROM (
            SELECT u AS p FROM unnest($2::uuid[]) AS u
            UNION
            SELECT gm.user_id FROM member_groups g JOIN member_group_members gm ON gm.group_id = g.id
             WHERE g.workspace_id = $1 AND g.id = ANY($2)
         ) x
         WHERE EXISTS (SELECT 1 FROM workspace_members wm WHERE wm.workspace_id = $1 AND wm.user_id = x.p::text)",
    )
    .bind(ws)
    .bind(ids)
    .fetch_all(pool)
    .await?;
    // Ordre des mentions conservé (notifications dans l'ordre du texte).
    let mut out: Vec<Uuid> = ids.iter().copied().filter(|i| people.contains(i)).collect();
    for p in people {
        if !out.contains(&p) {
            out.push(p);
        }
    }
    Ok(out)
}

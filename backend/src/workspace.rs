use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::access::{
    hash_token, require_member, require_ws_admin, visible_pages, AuthUser, PageAccess, Role,
};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

#[derive(Serialize, sqlx::FromRow)]
pub struct Workspace {
    pub id: Uuid,
    pub name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct WorkspaceResponse {
    pub id: Uuid,
    pub name: String,
    pub role: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
pub struct NamePayload {
    pub name: String,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct WorkspaceMemberResponse {
    pub user_id: String,
    pub name: String,
    pub email: String,
    pub avatar_url: Option<String>,
    pub role: String,
    pub joined_at: chrono::DateTime<chrono::Utc>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_workspaces).post(create_workspace))
        .route("/:workspace_id", get(get_workspace).put(update_workspace).delete(delete_workspace))
        .route("/:workspace_id/members", get(list_members))
        .route("/:workspace_id/members/:member_id", put(update_member_role).delete(remove_member))
        .route("/:workspace_id/leave", post(leave_workspace))
        .route("/:workspace_id/invites", get(list_invites).post(create_invite))
        .route("/:workspace_id/invites/:invite_id", delete(revoke_invite))
        .route("/:workspace_id/my_tasks", get(get_my_tasks))
        .route("/:workspace_id/overview", get(get_overview))
        .route("/:workspace_id/search", get(search_workspace))
}

pub fn invite_routes() -> Router<AppState> {
    Router::new()
        .route("/:token", get(preview_invite))
        .route("/:token/accept", post(accept_invite))
}

fn validate_name(name: &str) -> ApiResult<String> {
    let n = name.trim();
    if n.is_empty() || n.chars().count() > 120 {
        return Err(ApiError::bad_request("invalid_name"));
    }
    Ok(n.to_string())
}

async fn list_workspaces(State(pool): State<PgPool>, user: AuthUser) -> ApiResult<Json<serde_json::Value>> {
    let ws: Vec<WorkspaceResponse> = sqlx::query_as(
        "SELECT w.id, w.name, wm.role, w.created_at, w.updated_at
         FROM workspaces w JOIN workspace_members wm ON w.id = wm.workspace_id
         WHERE wm.user_id = $1 ORDER BY w.created_at ASC",
    )
    .bind(user.id.to_string())
    .fetch_all(&pool)
    .await?;
    Ok(Json(serde_json::json!({ "workspaces": ws })))
}

async fn get_workspace(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path(workspace_id): Path<Uuid>,
) -> ApiResult<Json<WorkspaceResponse>> {
    let role = require_member(&pool, &user, workspace_id).await?;
    let w: Workspace = sqlx::query_as("SELECT id, name, created_at, updated_at FROM workspaces WHERE id = $1")
        .bind(workspace_id)
        .fetch_one(&pool)
        .await?;
    Ok(Json(WorkspaceResponse {
        id: w.id,
        name: w.name,
        role: role.as_str().into(),
        created_at: w.created_at,
        updated_at: w.updated_at,
    }))
}

async fn create_workspace(
    State(pool): State<PgPool>,
    user: AuthUser,
    Json(payload): Json<NamePayload>,
) -> ApiResult<Json<serde_json::Value>> {
    let name = validate_name(&payload.name)?;
    let mut tx = pool.begin().await?;
    let workspace: Workspace =
        sqlx::query_as("INSERT INTO workspaces (name) VALUES ($1) RETURNING id, name, created_at, updated_at")
            .bind(&name)
            .fetch_one(&mut *tx)
            .await?;
    sqlx::query("INSERT INTO workspace_members (workspace_id, user_id, role) VALUES ($1, $2, 'owner')")
        .bind(workspace.id)
        .bind(user.id.to_string())
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(serde_json::json!({ "workspace": workspace })))
}

async fn update_workspace(
    State(state): State<AppState>,
    user: AuthUser,
    Path(workspace_id): Path<Uuid>,
    Json(payload): Json<NamePayload>,
) -> ApiResult<Json<serde_json::Value>> {
    require_ws_admin(&state.pool, &user, workspace_id).await?;
    let name = validate_name(&payload.name)?;
    let ws: Workspace = sqlx::query_as(
        "UPDATE workspaces SET name = $1, updated_at = NOW() WHERE id = $2 RETURNING id, name, created_at, updated_at",
    )
    .bind(name)
    .bind(workspace_id)
    .fetch_one(&state.pool)
    .await?;
    state
        .emit(workspace_id, serde_json::json!({ "type": "WORKSPACE_UPDATED", "workspace": &ws }))
        .await;
    Ok(Json(serde_json::json!({ "workspace": ws })))
}

async fn delete_workspace(
    State(state): State<AppState>,
    user: AuthUser,
    Path(workspace_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    // Seul un propriétaire peut supprimer un workspace.
    if require_member(&state.pool, &user, workspace_id).await? != Role::Owner {
        return Err(ApiError::Forbidden);
    }
    state.emit(workspace_id, serde_json::json!({ "type": "WORKSPACE_DELETED", "workspace_id": workspace_id })).await;
    sqlx::query("DELETE FROM workspaces WHERE id = $1").bind(workspace_id).execute(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_members(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path(workspace_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    require_member(&pool, &user, workspace_id).await?;
    let members: Vec<WorkspaceMemberResponse> = sqlx::query_as(
        "SELECT wm.user_id, u.name, u.email, u.avatar_url, wm.role, wm.joined_at
         FROM workspace_members wm JOIN users u ON wm.user_id = u.id::varchar
         WHERE wm.workspace_id = $1
         ORDER BY CASE wm.role WHEN 'owner' THEN 0 WHEN 'admin' THEN 1 WHEN 'member' THEN 2 ELSE 3 END, u.name",
    )
    .bind(workspace_id)
    .fetch_all(&pool)
    .await?;
    Ok(Json(serde_json::json!({ "members": members })))
}

async fn owner_count(pool: &PgPool, workspace_id: Uuid) -> ApiResult<i64> {
    Ok(sqlx::query_scalar("SELECT count(*) FROM workspace_members WHERE workspace_id = $1 AND role = 'owner'")
        .bind(workspace_id)
        .fetch_one(pool)
        .await?)
}

#[derive(Deserialize)]
pub struct UpdateMemberRolePayload {
    pub role: String,
}

async fn update_member_role(
    State(state): State<AppState>,
    user: AuthUser,
    Path((workspace_id, member_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<UpdateMemberRolePayload>,
) -> ApiResult<StatusCode> {
    let pool = &state.pool;
    let my_role = require_ws_admin(pool, &user, workspace_id).await?;
    let new_role = Role::parse(&payload.role).ok_or_else(|| ApiError::bad_request("invalid_role"))?;
    let target_role = crate::access::workspace_role(pool, member_id, workspace_id)
        .await?
        .ok_or(ApiError::NotFound)?;

    // Un admin ne peut ni promouvoir en owner, ni toucher à un owner/admin.
    if my_role != Role::Owner && (new_role >= Role::Admin || target_role >= Role::Admin) {
        return Err(ApiError::Forbidden);
    }
    // On ne retire jamais le dernier propriétaire.
    if target_role == Role::Owner && new_role != Role::Owner && owner_count(pool, workspace_id).await? <= 1 {
        return Err(ApiError::Conflict("last_owner".into()));
    }

    sqlx::query("UPDATE workspace_members SET role = $1 WHERE workspace_id = $2 AND user_id = $3")
        .bind(new_role.as_str())
        .bind(workspace_id)
        .bind(member_id.to_string())
        .execute(pool)
        .await?;
    state
        .emit(
            workspace_id,
            serde_json::json!({ "type": "MEMBERS_UPDATED", "user_id": member_id, "role": new_role.as_str() }),
        )
        .await;
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_member(
    State(state): State<AppState>,
    user: AuthUser,
    Path((workspace_id, member_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    let pool = &state.pool;
    let my_role = require_ws_admin(pool, &user, workspace_id).await?;
    let target_role = crate::access::workspace_role(pool, member_id, workspace_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    if my_role != Role::Owner && target_role >= Role::Admin {
        return Err(ApiError::Forbidden);
    }
    if target_role == Role::Owner && owner_count(pool, workspace_id).await? <= 1 {
        return Err(ApiError::Conflict("last_owner".into()));
    }
    sqlx::query("DELETE FROM workspace_members WHERE workspace_id = $1 AND user_id = $2")
        .bind(workspace_id)
        .bind(member_id.to_string())
        .execute(pool)
        .await?;
    state
        .emit(workspace_id, serde_json::json!({ "type": "MEMBERS_UPDATED", "user_id": member_id, "removed": true }))
        .await;
    Ok(StatusCode::NO_CONTENT)
}

async fn leave_workspace(
    State(state): State<AppState>,
    user: AuthUser,
    Path(workspace_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let role = require_member(&state.pool, &user, workspace_id).await?;
    if role == Role::Owner && owner_count(&state.pool, workspace_id).await? <= 1 {
        return Err(ApiError::Conflict("last_owner".into()));
    }
    sqlx::query("DELETE FROM workspace_members WHERE workspace_id = $1 AND user_id = $2")
        .bind(workspace_id)
        .bind(user.id.to_string())
        .execute(&state.pool)
        .await?;
    state
        .emit(workspace_id, serde_json::json!({ "type": "MEMBERS_UPDATED", "user_id": user.id, "removed": true }))
        .await;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Invitations
// ---------------------------------------------------------------------------

#[derive(Serialize, sqlx::FromRow)]
struct InviteDto {
    id: Uuid,
    role: String,
    created_by_name: Option<String>,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
    max_uses: Option<i32>,
    uses: i32,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
struct CreateInvite {
    role: Option<String>,
    expires_in_days: Option<i64>,
    max_uses: Option<i32>,
}

async fn list_invites(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path(workspace_id): Path<Uuid>,
) -> ApiResult<Json<Vec<InviteDto>>> {
    require_ws_admin(&pool, &user, workspace_id).await?;
    Ok(Json(
        sqlx::query_as(
            "SELECT i.id, i.role, u.name AS created_by_name, i.expires_at, i.max_uses, i.uses, i.created_at
             FROM workspace_invites i LEFT JOIN users u ON u.id = i.created_by
             WHERE i.workspace_id = $1 AND i.revoked_at IS NULL
               AND (i.expires_at IS NULL OR i.expires_at > NOW())
               AND (i.max_uses IS NULL OR i.uses < i.max_uses)
             ORDER BY i.created_at DESC",
        )
        .bind(workspace_id)
        .fetch_all(&pool)
        .await?,
    ))
}

async fn create_invite(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path(workspace_id): Path<Uuid>,
    Json(body): Json<CreateInvite>,
) -> ApiResult<Json<serde_json::Value>> {
    let my_role = require_ws_admin(&pool, &user, workspace_id).await?;
    let role = Role::parse(body.role.as_deref().unwrap_or("member")).ok_or_else(|| ApiError::bad_request("invalid_role"))?;
    if role == Role::Owner || (role == Role::Admin && my_role != Role::Owner) {
        return Err(ApiError::Forbidden);
    }
    let days = body.expires_in_days.unwrap_or(7).clamp(0, 365);
    let expires = (days > 0).then(|| chrono::Utc::now() + chrono::Duration::days(days));
    let max_uses = body.max_uses.filter(|m| *m > 0);

    let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO workspace_invites (workspace_id, token_hash, role, created_by, expires_at, max_uses)
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(workspace_id)
    .bind(hash_token(&token))
    .bind(role.as_str())
    .bind(user.id)
    .bind(expires)
    .bind(max_uses)
    .fetch_one(&pool)
    .await?;
    // Le jeton brut n'est renvoyé qu'une seule fois.
    Ok(Json(serde_json::json!({ "id": id, "token": token, "role": role.as_str(), "expires_at": expires })))
}

async fn revoke_invite(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path((workspace_id, invite_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    require_ws_admin(&pool, &user, workspace_id).await?;
    sqlx::query("UPDATE workspace_invites SET revoked_at = NOW() WHERE id = $1 AND workspace_id = $2")
        .bind(invite_id)
        .bind(workspace_id)
        .execute(&pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(sqlx::FromRow)]
struct ValidInvite {
    id: Uuid,
    workspace_id: Uuid,
    workspace_name: String,
    role: String,
    member_count: i64,
}

async fn find_invite(pool: &PgPool, token: &str) -> ApiResult<ValidInvite> {
    sqlx::query_as(
        "SELECT i.id, i.workspace_id, w.name AS workspace_name, i.role,
                (SELECT count(*) FROM workspace_members m WHERE m.workspace_id = w.id) AS member_count
         FROM workspace_invites i JOIN workspaces w ON w.id = i.workspace_id
         WHERE i.token_hash = $1 AND i.revoked_at IS NULL
           AND (i.expires_at IS NULL OR i.expires_at > NOW())
           AND (i.max_uses IS NULL OR i.uses < i.max_uses)",
    )
    .bind(hash_token(token))
    .fetch_optional(pool)
    .await?
    .ok_or(ApiError::NotFound)
}

async fn preview_invite(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path(token): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let inv = find_invite(&pool, &token).await?;
    let already = crate::access::workspace_role(&pool, user.id, inv.workspace_id).await?.is_some();
    Ok(Json(serde_json::json!({
        "workspace_id": inv.workspace_id,
        "workspace_name": inv.workspace_name,
        "role": inv.role,
        "member_count": inv.member_count,
        "already_member": already,
    })))
}

async fn accept_invite(
    State(state): State<AppState>,
    user: AuthUser,
    Path(token): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let inv = find_invite(&state.pool, &token).await?;
    let mut tx = state.pool.begin().await?;
    let inserted = sqlx::query(
        "INSERT INTO workspace_members (workspace_id, user_id, role) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(inv.workspace_id)
    .bind(user.id.to_string())
    .bind(&inv.role)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if inserted > 0 {
        sqlx::query("UPDATE workspace_invites SET uses = uses + 1 WHERE id = $1")
            .bind(inv.id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    if inserted > 0 {
        state
            .emit(inv.workspace_id, serde_json::json!({ "type": "MEMBERS_UPDATED", "user_id": user.id }))
            .await;
    }
    Ok(Json(serde_json::json!({ "workspace_id": inv.workspace_id })))
}

// ---------------------------------------------------------------------------
// Mes tâches & recherche
// ---------------------------------------------------------------------------

#[derive(Serialize, sqlx::FromRow)]
pub struct MyTaskResponse {
    pub id: Uuid,
    pub title: String,
    pub due_date: Option<chrono::DateTime<chrono::Utc>>,
    pub priority: i32,
    pub is_completed: bool,
    pub board_id: Uuid,
    pub board_name: String,
    pub column_name: String,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

async fn get_my_tasks(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path(workspace_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    require_member(&pool, &user, workspace_id).await?;
    let tasks: Vec<MyTaskResponse> = sqlx::query_as(
        "SELECT t.id, t.title, t.due_date, t.priority, t.is_completed, b.id AS board_id, b.name AS board_name,
                c.name AS column_name, t.updated_at
         FROM tasks t
         JOIN task_assignees ta ON ta.task_id = t.id
         JOIN board_columns c ON c.id = t.column_id
         JOIN boards b ON b.id = c.board_id
         WHERE ta.user_id = $1 AND b.workspace_id = $2 AND t.is_completed = false
         ORDER BY t.due_date ASC NULLS LAST, t.priority = 0, t.priority ASC",
    )
    .bind(user.id.to_string())
    .bind(workspace_id)
    .fetch_all(&pool)
    .await?;
    Ok(Json(serde_json::json!({ "tasks": tasks })))
}

#[derive(Serialize, sqlx::FromRow)]
pub struct SearchResult {
    pub id: Uuid,
    pub title: String,
    pub r#type: String,
    pub icon: Option<String>,
    pub snippet: Option<String>,
    pub parent_id: Option<Uuid>,
}

#[derive(Deserialize)]
struct SearchQuery {
    q: Option<String>,
}

/// Échappe les métacaractères LIKE (`%`, `_`, `\`).
fn like_pattern(q: &str) -> String {
    let escaped = q.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
    format!("%{}%", escaped)
}

async fn search_workspace(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path(workspace_id): Path<Uuid>,
    Query(params): Query<SearchQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let role = require_member(&pool, &user, workspace_id).await?;
    let q = params.q.unwrap_or_default();
    let q = q.trim();
    if q.is_empty() {
        return Ok(Json(serde_json::json!({ "results": [] })));
    }
    let pattern = like_pattern(&q.chars().take(200).collect::<String>());

    let visible = visible_pages(&pool, &user, workspace_id, role).await?;
    // Le contenu HTML est nettoyé des balises pour l'extrait.
    let pages: Vec<SearchResult> = sqlx::query_as(
        "SELECT id, title, 'page' AS type, icon,
                substring(regexp_replace(coalesce(content, ''), '<[^>]*>', ' ', 'g') from 1 for 160) AS snippet,
                parent_id
         FROM pages
         WHERE workspace_id = $1 AND (title ILIKE $2 OR content ILIKE $2)
         ORDER BY (title ILIKE $2) DESC, updated_at DESC
         LIMIT 40",
    )
    .bind(workspace_id)
    .bind(&pattern)
    .fetch_all(&pool)
    .await?;
    let pages = pages
        .into_iter()
        .filter(|p| visible.get(&p.id).is_some_and(|a| *a != PageAccess::None))
        .take(20);

    let tasks: Vec<SearchResult> = sqlx::query_as(
        "SELECT t.id, t.title, 'task' AS type, NULL::varchar AS icon,
                substring(regexp_replace(coalesce(t.description, ''), '<[^>]*>', ' ', 'g') from 1 for 160) AS snippet,
                b.id AS parent_id
         FROM tasks t JOIN board_columns c ON c.id = t.column_id JOIN boards b ON b.id = c.board_id
         WHERE b.workspace_id = $1 AND (t.title ILIKE $2 OR t.description ILIKE $2)
         ORDER BY (t.title ILIKE $2) DESC, t.updated_at DESC
         LIMIT 20",
    )
    .bind(workspace_id)
    .bind(&pattern)
    .fetch_all(&pool)
    .await?;

    let mut results: Vec<SearchResult> = pages.collect();
    results.extend(tasks);
    Ok(Json(serde_json::json!({ "results": results })))
}

// ---------------------------------------------------------------------------
// Vue d'ensemble (page d'accueil)
// ---------------------------------------------------------------------------

#[derive(Serialize, sqlx::FromRow)]
struct BoardStats {
    id: Uuid,
    name: String,
    total: i64,
    done: i64,
    overdue: i64,
    due_soon: i64,
    updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Serialize, sqlx::FromRow)]
struct ActivityItem {
    id: Uuid,
    task_id: Uuid,
    task_title: String,
    board_id: Uuid,
    board_name: String,
    user_id: Uuid,
    user_name: Option<String>,
    activity_type: String,
    new_value: Option<serde_json::Value>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize, sqlx::FromRow)]
struct MyCounts {
    open: i64,
    overdue: i64,
    due_week: i64,
    completed_week: i64,
}

async fn get_overview(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path(workspace_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    let role = require_member(&pool, &user, workspace_id).await?;
    let boards: Vec<BoardStats> = sqlx::query_as(
        "SELECT b.id, b.name,
            count(t.id) FILTER (WHERE t.parent_id IS NULL) AS total,
            count(t.id) FILTER (WHERE t.parent_id IS NULL AND t.is_completed) AS done,
            count(t.id) FILTER (WHERE t.parent_id IS NULL AND NOT t.is_completed AND t.due_date < NOW()) AS overdue,
            count(t.id) FILTER (WHERE t.parent_id IS NULL AND NOT t.is_completed AND t.due_date BETWEEN NOW() AND NOW() + INTERVAL '7 days') AS due_soon,
            max(t.updated_at) AS updated_at
         FROM boards b
         LEFT JOIN board_columns c ON c.board_id = b.id
         LEFT JOIN tasks t ON t.column_id = c.id
         WHERE b.workspace_id = $1
         GROUP BY b.id ORDER BY max(t.updated_at) DESC NULLS LAST, b.created_at",
    )
    .bind(workspace_id)
    .fetch_all(&pool)
    .await?;

    let me = user.id.to_string();
    let counts: MyCounts = sqlx::query_as(
        "SELECT
            count(*) FILTER (WHERE NOT t.is_completed) AS open,
            count(*) FILTER (WHERE NOT t.is_completed AND t.due_date < NOW()) AS overdue,
            count(*) FILTER (WHERE NOT t.is_completed AND t.due_date BETWEEN NOW() AND NOW() + INTERVAL '7 days') AS due_week,
            count(*) FILTER (WHERE t.is_completed AND t.updated_at > NOW() - INTERVAL '7 days') AS completed_week
         FROM tasks t
         JOIN task_assignees ta ON ta.task_id = t.id AND ta.user_id = $2
         JOIN board_columns c ON c.id = t.column_id JOIN boards b ON b.id = c.board_id
         WHERE b.workspace_id = $1",
    )
    .bind(workspace_id)
    .bind(&me)
    .fetch_one(&pool)
    .await?;

    let activity: Vec<ActivityItem> = sqlx::query_as(
        "SELECT a.id, a.task_id, t.title AS task_title, b.id AS board_id, b.name AS board_name,
                a.user_id, u.name AS user_name, a.activity_type, a.new_value, a.created_at
         FROM task_activities a
         JOIN tasks t ON t.id = a.task_id
         JOIN board_columns c ON c.id = t.column_id JOIN boards b ON b.id = c.board_id
         LEFT JOIN users u ON u.id = a.user_id
         WHERE b.workspace_id = $1 AND a.activity_type IN ('create', 'move_column', 'toggle_validation', 'add_assignee', 'update_title')
         ORDER BY a.created_at DESC LIMIT 25",
    )
    .bind(workspace_id)
    .fetch_all(&pool)
    .await?;

    let visible = visible_pages(&pool, &user, workspace_id, role).await?;
    let week_ago = chrono::Utc::now() - chrono::Duration::days(7);
    let recent_pages: Vec<(Uuid, chrono::DateTime<chrono::Utc>)> =
        sqlx::query_as("SELECT id, updated_at FROM pages WHERE workspace_id = $1 AND updated_at > $2")
            .bind(workspace_id)
            .bind(week_ago)
            .fetch_all(&pool)
            .await?;
    let docs_updated_week = recent_pages
        .iter()
        .filter(|(id, _)| visible.get(id).is_some_and(|a| *a != PageAccess::None))
        .count();

    Ok(Json(serde_json::json!({
        "boards": boards,
        "my": counts,
        "activity": activity,
        "docs_updated_week": docs_updated_week,
    })))
}

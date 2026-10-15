use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::access::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/providers", get(list_providers).post(create_provider))
        .route("/providers/{id}", put(update_provider).delete(delete_provider))
        .route("/users", get(list_users))
        .route("/users/{id}", delete(delete_user))
        .route("/users/{id}/role", put(update_user_role))
        .route("/users/{id}/suspend", put(suspend_user))
        .route("/settings", get(get_settings).put(update_settings))
        .route("/workspaces", get(list_workspaces))
        .route("/workspaces/{id}", delete(delete_workspace))
        .route("/stats", get(get_stats))
}

pub(crate) fn require_super_admin(user: &AuthUser) -> ApiResult<()> {
    if user.is_super_admin { Ok(()) } else { Err(ApiError::Forbidden) }
}

/// URL acceptée pour un logo / script : chemin d'upload local ou https.
fn check_url(url: &Option<String>, allow_local: bool) -> ApiResult<Option<String>> {
    match url.as_deref().map(str::trim) {
        None | Some("") => Ok(None),
        Some(u) if u.starts_with("https://") && u.len() <= 1024 => Ok(Some(u.to_string())),
        Some(u) if allow_local && u.starts_with("/uploads/") && !u.contains("..") => Ok(Some(u.to_string())),
        _ => Err(ApiError::bad_request("invalid_url")),
    }
}

// ---------------------------------------------------------------------------
// Fournisseurs OAuth
// ---------------------------------------------------------------------------

#[derive(Serialize, sqlx::FromRow)]
pub struct OAuthProviderDto {
    pub id: Uuid,
    pub name: String,
    pub issuer_url: String,
    pub client_id: String,
    pub has_secret: bool,
    pub is_active: bool,
    pub user_count: i64,
}

const PROVIDER_SELECT: &str = "SELECT p.id, p.name, p.issuer_url, p.client_id, (p.client_secret <> '') AS has_secret, p.is_active,
    (SELECT count(DISTINCT i.user_id) FROM identities i WHERE i.provider = p.id::text) AS user_count
    FROM oauth_providers p";

async fn list_providers(State(pool): State<PgPool>, user: AuthUser) -> ApiResult<Json<Vec<OAuthProviderDto>>> {
    require_super_admin(&user)?;
    Ok(Json(sqlx::query_as(sqlx::AssertSqlSafe(format!("{PROVIDER_SELECT} ORDER BY p.name"))).fetch_all(&pool).await?))
}

#[derive(Deserialize)]
struct ProviderReq {
    name: String,
    issuer_url: String,
    client_id: String,
    /// Vide / absent à la mise à jour = secret inchangé.
    client_secret: Option<String>,
    is_active: bool,
}

fn validate_provider(p: &ProviderReq) -> ApiResult<()> {
    if p.name.trim().is_empty() || p.client_id.trim().is_empty() {
        return Err(ApiError::bad_request("missing_fields"));
    }
    if !(p.issuer_url.starts_with("https://") || p.issuer_url.starts_with("http://localhost")) {
        return Err(ApiError::bad_request("invalid_issuer"));
    }
    Ok(())
}

async fn create_provider(
    State(pool): State<PgPool>,
    user: AuthUser,
    Json(p): Json<ProviderReq>,
) -> ApiResult<Json<OAuthProviderDto>> {
    require_super_admin(&user)?;
    validate_provider(&p)?;
    let secret = p.client_secret.unwrap_or_default();
    if secret.trim().is_empty() {
        return Err(ApiError::bad_request("missing_secret"));
    }
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO oauth_providers (name, issuer_url, client_id, client_secret, is_active) VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(p.name.trim())
    .bind(p.issuer_url.trim())
    .bind(p.client_id.trim())
    .bind(secret.trim())
    .bind(p.is_active)
    .fetch_one(&pool)
    .await?;
    Ok(Json(sqlx::query_as(sqlx::AssertSqlSafe(format!("{PROVIDER_SELECT} WHERE p.id = $1"))).bind(id).fetch_one(&pool).await?))
}

async fn update_provider(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(p): Json<ProviderReq>,
) -> ApiResult<Json<OAuthProviderDto>> {
    require_super_admin(&user)?;
    validate_provider(&p)?;
    let secret = p.client_secret.filter(|s| !s.trim().is_empty());
    let n = sqlx::query(
        "UPDATE oauth_providers SET name = $1, issuer_url = $2, client_id = $3,
            client_secret = COALESCE($4, client_secret), is_active = $5, updated_at = NOW() WHERE id = $6",
    )
    .bind(p.name.trim())
    .bind(p.issuer_url.trim())
    .bind(p.client_id.trim())
    .bind(secret.map(|s| s.trim().to_string()))
    .bind(p.is_active)
    .bind(id)
    .execute(&state.pool)
    .await?
    .rows_affected();
    if n == 0 {
        return Err(ApiError::NotFound);
    }
    state.oidc_clients.remove(&id);
    Ok(Json(sqlx::query_as(sqlx::AssertSqlSafe(format!("{PROVIDER_SELECT} WHERE p.id = $1"))).bind(id).fetch_one(&state.pool).await?))
}

async fn delete_provider(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<StatusCode> {
    require_super_admin(&user)?;
    let active_others: i64 =
        sqlx::query_scalar("SELECT count(*) FROM oauth_providers WHERE is_active = TRUE AND id <> $1")
            .bind(id)
            .fetch_one(&state.pool)
            .await?;
    if active_others == 0 {
        // Supprimer le dernier fournisseur actif bloquerait toute connexion.
        return Err(ApiError::Conflict("last_provider".into()));
    }
    sqlx::query("DELETE FROM oauth_providers WHERE id = $1").bind(id).execute(&state.pool).await?;
    state.oidc_clients.remove(&id);
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Utilisateurs
// ---------------------------------------------------------------------------

#[derive(Serialize, sqlx::FromRow)]
struct UserInfo {
    id: Uuid,
    name: String,
    email: String,
    avatar_url: Option<String>,
    is_super_admin: bool,
    is_suspended: bool,
    created_at: chrono::DateTime<chrono::Utc>,
    workspace_count: i64,
}

async fn list_users(State(pool): State<PgPool>, user: AuthUser) -> ApiResult<Json<Vec<UserInfo>>> {
    require_super_admin(&user)?;
    Ok(Json(
        sqlx::query_as(
            "SELECT u.id, u.name, u.email, u.avatar_url, u.is_super_admin, u.is_suspended, u.created_at,
                (SELECT count(*) FROM workspace_members m WHERE m.user_id = u.id::varchar) AS workspace_count
             FROM users u ORDER BY u.created_at ASC",
        )
        .fetch_all(&pool)
        .await?,
    ))
}

async fn super_admin_count(pool: &PgPool) -> ApiResult<i64> {
    Ok(sqlx::query_scalar("SELECT count(*) FROM users WHERE is_super_admin = TRUE AND is_suspended = FALSE")
        .fetch_one(pool)
        .await?)
}

#[derive(Deserialize)]
struct RoleUpdate {
    is_super_admin: bool,
}

async fn update_user_role(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<RoleUpdate>,
) -> ApiResult<StatusCode> {
    require_super_admin(&user)?;
    if id == user.id && !payload.is_super_admin {
        return Err(ApiError::bad_request("cannot_demote_self"));
    }
    sqlx::query("UPDATE users SET is_super_admin = $1, updated_at = NOW() WHERE id = $2")
        .bind(payload.is_super_admin)
        .bind(id)
        .execute(&pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct SuspendUserRequest {
    is_suspended: bool,
}

async fn suspend_user(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<SuspendUserRequest>,
) -> ApiResult<StatusCode> {
    require_super_admin(&user)?;
    if id == user.id {
        return Err(ApiError::bad_request("cannot_suspend_self"));
    }
    let target_admin: bool = sqlx::query_scalar("SELECT is_super_admin FROM users WHERE id = $1")
        .bind(id)
        .fetch_optional(&pool)
        .await?
        .ok_or(ApiError::NotFound)?;
    if payload.is_suspended && target_admin && super_admin_count(&pool).await? <= 1 {
        return Err(ApiError::Conflict("last_super_admin".into()));
    }
    sqlx::query("UPDATE users SET is_suspended = $1, updated_at = NOW() WHERE id = $2")
        .bind(payload.is_suspended)
        .bind(id)
        .execute(&pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_user(State(pool): State<PgPool>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<StatusCode> {
    require_super_admin(&user)?;
    if id == user.id {
        return Err(ApiError::bad_request("cannot_delete_self"));
    }
    let uid = id.to_string();
    let mut tx = pool.begin().await?;
    // Les tables avec user_id textuel n'ont pas de clé étrangère : nettoyage explicite.
    for sql in [
        "DELETE FROM workspace_members WHERE user_id = $1",
        "DELETE FROM task_assignees WHERE user_id = $1",
        "DELETE FROM task_validations WHERE user_id = $1",
    ] {
        sqlx::query(sql).bind(&uid).execute(&mut *tx).await?;
    }
    // Les pages créées sont conservées et réattribuées à l'administrateur qui supprime.
    sqlx::query("UPDATE pages SET created_by = $1 WHERE created_by = $2")
        .bind(user.id)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    let n = sqlx::query("DELETE FROM users WHERE id = $1").bind(id).execute(&mut *tx).await?.rows_affected();
    if n == 0 {
        return Err(ApiError::NotFound);
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Paramètres globaux
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct GlobalSettings {
    pub app_name: String,
    pub favicon_url: Option<String>,
    pub allow_public_signups: bool,
    pub umami_website_id: Option<String>,
    pub umami_script_url: Option<String>,
}

const SETTINGS_SELECT: &str =
    "SELECT app_name, favicon_url, allow_public_signups, umami_website_id, umami_script_url FROM global_settings WHERE id = 1";

async fn load_settings(pool: &PgPool) -> ApiResult<GlobalSettings> {
    Ok(sqlx::query_as(SETTINGS_SELECT).fetch_optional(pool).await?.unwrap_or(GlobalSettings {
        app_name: "Looty".into(),
        favicon_url: None,
        allow_public_signups: true,
        umami_website_id: None,
        umami_script_url: None,
    }))
}

async fn get_settings(State(pool): State<PgPool>, user: AuthUser) -> ApiResult<Json<GlobalSettings>> {
    require_super_admin(&user)?;
    Ok(Json(load_settings(&pool).await?))
}

async fn update_settings(
    State(pool): State<PgPool>,
    user: AuthUser,
    Json(p): Json<GlobalSettings>,
) -> ApiResult<Json<GlobalSettings>> {
    require_super_admin(&user)?;
    let name = p.app_name.trim();
    if name.is_empty() || name.chars().count() > 60 {
        return Err(ApiError::bad_request("invalid_name"));
    }
    let website_id = p.umami_website_id.as_deref().map(str::trim).filter(|s| !s.is_empty());
    if let Some(w) = website_id {
        if w.len() > 64 || !w.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(ApiError::bad_request("invalid_umami_id"));
        }
    }
    sqlx::query(
        "INSERT INTO global_settings (id, app_name, favicon_url, allow_public_signups, umami_website_id, umami_script_url, updated_at)
         VALUES (1, $1, $2, $3, $4, $5, NOW())
         ON CONFLICT (id) DO UPDATE SET app_name = $1, favicon_url = $2, allow_public_signups = $3,
            umami_website_id = $4, umami_script_url = $5, updated_at = NOW()",
    )
    .bind(name)
    .bind(check_url(&p.favicon_url, true)?)
    .bind(p.allow_public_signups)
    .bind(website_id)
    .bind(check_url(&p.umami_script_url, false)?)
    .execute(&pool)
    .await?;
    Ok(Json(load_settings(&pool).await?))
}

pub async fn get_public_settings(State(pool): State<PgPool>) -> ApiResult<Json<GlobalSettings>> {
    Ok(Json(load_settings(&pool).await?))
}

// ---------------------------------------------------------------------------
// Workspaces & statistiques
// ---------------------------------------------------------------------------

#[derive(Serialize, sqlx::FromRow)]
pub struct AdminWorkspaceDto {
    pub id: Uuid,
    pub name: String,
    pub members_count: i64,
    pub boards_count: i64,
    pub pages_count: i64,
    pub owner_name: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

async fn list_workspaces(State(pool): State<PgPool>, user: AuthUser) -> ApiResult<Json<Vec<AdminWorkspaceDto>>> {
    require_super_admin(&user)?;
    Ok(Json(
        sqlx::query_as(
            "SELECT w.id, w.name, w.created_at,
                (SELECT count(*) FROM workspace_members m WHERE m.workspace_id = w.id) AS members_count,
                (SELECT count(*) FROM boards b WHERE b.workspace_id = w.id) AS boards_count,
                (SELECT count(*) FROM pages p WHERE p.workspace_id = w.id) AS pages_count,
                (SELECT u.name FROM workspace_members m JOIN users u ON u.id::varchar = m.user_id
                 WHERE m.workspace_id = w.id AND m.role = 'owner' ORDER BY m.joined_at LIMIT 1) AS owner_name
             FROM workspaces w ORDER BY w.created_at DESC",
        )
        .fetch_all(&pool)
        .await?,
    ))
}

async fn delete_workspace(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<StatusCode> {
    require_super_admin(&user)?;
    state.emit(id, serde_json::json!({ "type": "WORKSPACE_DELETED", "workspace_id": id })).await;
    sqlx::query("DELETE FROM workspaces WHERE id = $1").bind(id).execute(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize, sqlx::FromRow)]
pub struct AdminStatsDto {
    pub total_users: i64,
    pub active_users_7d: i64,
    pub new_users_30d: i64,
    pub suspended_users: i64,
    pub total_workspaces: i64,
    pub total_boards: i64,
    pub total_tasks: i64,
    pub open_tasks: i64,
    pub total_pages: i64,
    pub total_attachments: i64,
    pub total_attachments_size: i64,
}

#[derive(Serialize, sqlx::FromRow)]
struct DailyCount {
    day: chrono::NaiveDate,
    tasks: i64,
    pages: i64,
}

async fn get_stats(State(pool): State<PgPool>, user: AuthUser) -> ApiResult<Json<serde_json::Value>> {
    require_super_admin(&user)?;
    let stats: AdminStatsDto = sqlx::query_as(
        "SELECT
            (SELECT count(*) FROM users) AS total_users,
            (SELECT count(DISTINCT user_id) FROM task_activities WHERE created_at > NOW() - INTERVAL '7 days') AS active_users_7d,
            (SELECT count(*) FROM users WHERE created_at > NOW() - INTERVAL '30 days') AS new_users_30d,
            (SELECT count(*) FROM users WHERE is_suspended) AS suspended_users,
            (SELECT count(*) FROM workspaces) AS total_workspaces,
            (SELECT count(*) FROM boards) AS total_boards,
            (SELECT count(*) FROM tasks) AS total_tasks,
            (SELECT count(*) FROM tasks WHERE NOT is_completed) AS open_tasks,
            (SELECT count(*) FROM pages) AS total_pages,
            (SELECT count(*) FROM task_attachments) AS total_attachments,
            (SELECT COALESCE(SUM(size_bytes), 0)::bigint FROM task_attachments) AS total_attachments_size",
    )
    .fetch_one(&pool)
    .await?;
    // Activité des 30 derniers jours (tâches et pages créées par jour).
    let activity: Vec<DailyCount> = sqlx::query_as(
        "SELECT d::date AS day,
            (SELECT count(*) FROM tasks t WHERE t.created_at::date = d::date) AS tasks,
            (SELECT count(*) FROM pages p WHERE p.created_at::date = d::date) AS pages
         FROM generate_series(CURRENT_DATE - INTERVAL '29 days', CURRENT_DATE, INTERVAL '1 day') d
         ORDER BY d",
    )
    .fetch_all(&pool)
    .await?;
    Ok(Json(serde_json::json!({ "stats": stats, "activity": activity })))
}

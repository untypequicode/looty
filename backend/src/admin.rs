use axum::{
    extract::{Path, State},
    response::IntoResponse,
    routing::{get, put, delete, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;
use uuid::Uuid;

use crate::auth::AuthState;

#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct OAuthProvider {
    pub id: Uuid,
    pub name: String,
    pub issuer_url: String,
    pub client_id: String,
    pub client_secret: String,
    pub is_active: bool,
}

pub fn admin_routes(state: AuthState) -> Router {
    Router::new()
        .route("/providers", get(list_providers).post(create_provider))
        .route("/providers/:id", put(update_provider).delete(delete_provider))
        .route("/users", get(list_users))
        .route("/users/:id", delete(delete_user))
        .route("/users/:id/role", put(update_user_role))
        .route("/users/:id/suspend", put(suspend_user))
        .route("/settings", get(get_settings).put(update_settings))
        .route("/workspaces", get(list_workspaces))
        .route("/workspaces/:id", delete(delete_workspace))
        .route("/stats", get(get_stats))
        .with_state(state)
}

async fn is_super_admin(session: &Session, pool: &sqlx::PgPool) -> bool {
    let user_id: Option<Uuid> = session.get("user_id").await.unwrap_or(None);
    if let Some(uid) = user_id {
        let is_admin: Option<bool> = sqlx::query_scalar("SELECT is_super_admin FROM users WHERE id = $1")
            .bind(uid)
            .fetch_optional(pool)
            .await
            .unwrap_or(Some(false));
        return is_admin.unwrap_or(false);
    }
    false
}

async fn list_providers(session: Session, State(state): State<AuthState>) -> impl IntoResponse {
    if !is_super_admin(&session, &state.pool).await {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    
    let providers = sqlx::query_as::<_, OAuthProvider>(
        "SELECT id, name, issuer_url, client_id, client_secret, is_active FROM oauth_providers"
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    Json(providers).into_response()
}

#[derive(Deserialize)]
struct CreateProviderReq {
    name: String,
    issuer_url: String,
    client_id: String,
    client_secret: String,
    is_active: bool,
}

async fn create_provider(
    session: Session,
    State(state): State<AuthState>,
    Json(payload): Json<CreateProviderReq>,
) -> impl IntoResponse {
    if !is_super_admin(&session, &state.pool).await {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }

    let provider = sqlx::query_as::<_, OAuthProvider>(
        "INSERT INTO oauth_providers (name, issuer_url, client_id, client_secret, is_active) VALUES ($1, $2, $3, $4, $5) RETURNING id, name, issuer_url, client_id, client_secret, is_active"
    )
    .bind(&payload.name).bind(&payload.issuer_url).bind(&payload.client_id).bind(&payload.client_secret).bind(payload.is_active)
    .fetch_one(&state.pool)
    .await;

    match provider {
        Ok(p) => Json(p).into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
    }
}

async fn update_provider(
    session: Session,
    State(state): State<AuthState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<CreateProviderReq>,
) -> impl IntoResponse {
    if !is_super_admin(&session, &state.pool).await {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }

    let provider = sqlx::query_as::<_, OAuthProvider>(
        "UPDATE oauth_providers SET name = $1, issuer_url = $2, client_id = $3, client_secret = $4, is_active = $5, updated_at = NOW() WHERE id = $6 RETURNING id, name, issuer_url, client_id, client_secret, is_active"
    )
    .bind(&payload.name).bind(&payload.issuer_url).bind(&payload.client_id).bind(&payload.client_secret).bind(payload.is_active).bind(id)
    .fetch_one(&state.pool)
    .await;

    match provider {
        Ok(p) => {
            // Remove from cache if it exists
            state.oidc_clients.remove(&id);
            Json(p).into_response()
        },
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
    }
}

async fn delete_provider(
    session: Session,
    State(state): State<AuthState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    if !is_super_admin(&session, &state.pool).await {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }

    sqlx::query("DELETE FROM oauth_providers WHERE id = $1").bind(id)
        .execute(&state.pool)
        .await
        .ok();
    
    state.oidc_clients.remove(&id);

    (axum::http::StatusCode::OK).into_response()
}

#[derive(Serialize, sqlx::FromRow)]
struct UserInfo {
    id: Uuid,
    name: String,
    email: String,
    is_super_admin: bool,
    is_suspended: bool,
}

async fn list_users(session: Session, State(state): State<AuthState>) -> impl IntoResponse {
    if !is_super_admin(&session, &state.pool).await {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    
    let users = sqlx::query_as::<_, UserInfo>(
        "SELECT id, name, email, is_super_admin, is_suspended FROM users"
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    Json(users).into_response()
}

#[derive(Deserialize)]
struct RoleUpdate {
    is_super_admin: bool,
}

async fn update_user_role(
    session: Session,
    State(state): State<AuthState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<RoleUpdate>,
) -> impl IntoResponse {
    if !is_super_admin(&session, &state.pool).await {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }

    let result = sqlx::query("UPDATE users SET is_super_admin = $1 WHERE id = $2")
        .bind(payload.is_super_admin)
        .bind(id)
        .execute(&state.pool)
        .await;

    match result {
        Ok(_) => (axum::http::StatusCode::OK).into_response(),
        Err(_) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Failed").into_response(),
    }
}

#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct GlobalSettings {
    pub app_name: String,
    pub favicon_url: Option<String>,
    pub allow_public_signups: bool,
    pub umami_website_id: Option<String>,
    pub umami_script_url: Option<String>,
}

async fn get_settings(session: Session, State(state): State<AuthState>) -> impl IntoResponse {
    if !is_super_admin(&session, &state.pool).await {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    
    let settings = sqlx::query_as::<_, GlobalSettings>(
        "SELECT app_name, favicon_url, allow_public_signups, umami_website_id, umami_script_url FROM global_settings WHERE id = 1"
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(GlobalSettings {
        app_name: "Looty".to_string(),
        favicon_url: None,
        allow_public_signups: true,
        umami_website_id: None,
        umami_script_url: None,
    });

    Json(settings).into_response()
}

async fn update_settings(session: Session, State(state): State<AuthState>, Json(payload): Json<GlobalSettings>) -> impl IntoResponse {
    if !is_super_admin(&session, &state.pool).await {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    
    sqlx::query(
        "UPDATE global_settings SET app_name = $1, favicon_url = $2, allow_public_signups = $3, umami_website_id = $4, umami_script_url = $5, updated_at = NOW() WHERE id = 1"
    )
    .bind(&payload.app_name)
    .bind(&payload.favicon_url)
    .bind(payload.allow_public_signups)
    .bind(&payload.umami_website_id)
    .bind(&payload.umami_script_url)
    .execute(&state.pool)
    .await
    .ok();

    Json(payload).into_response()
}

#[derive(Deserialize)]
pub struct SuspendUserRequest {
    pub is_suspended: bool,
}

async fn suspend_user(session: Session, State(state): State<AuthState>, Path(id): Path<Uuid>, Json(payload): Json<SuspendUserRequest>) -> impl IntoResponse {
    if !is_super_admin(&session, &state.pool).await {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }

    let user_id: Option<Uuid> = session.get("user_id").await.unwrap_or(None);
    if user_id == Some(id) {
        return (axum::http::StatusCode::BAD_REQUEST, "Cannot suspend yourself").into_response();
    }

    sqlx::query("UPDATE users SET is_suspended = $1 WHERE id = $2")
        .bind(payload.is_suspended)
        .bind(id)
        .execute(&state.pool)
        .await
        .ok();

    (axum::http::StatusCode::OK, "Updated").into_response()
}

async fn delete_user(session: Session, State(state): State<AuthState>, Path(id): Path<Uuid>) -> impl IntoResponse {
    if !is_super_admin(&session, &state.pool).await {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }

    let user_id: Option<Uuid> = session.get("user_id").await.unwrap_or(None);
    if user_id == Some(id) {
        return (axum::http::StatusCode::BAD_REQUEST, "Cannot delete yourself").into_response();
    }

    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await
        .ok();

    (axum::http::StatusCode::OK, "Deleted").into_response()
}


#[derive(Serialize, sqlx::FromRow)]
pub struct AdminWorkspaceDto {
    pub id: Uuid,
    pub name: String,
    pub members_count: Option<i64>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

async fn list_workspaces(session: Session, State(state): State<AuthState>) -> impl IntoResponse {
    if !is_super_admin(&session, &state.pool).await {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }

    let workspaces = sqlx::query_as::<_, AdminWorkspaceDto>(
        "SELECT w.id, w.name, w.created_at, (SELECT count(*) FROM workspace_members m WHERE m.workspace_id = w.id) as members_count FROM workspaces w"
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    Json(workspaces).into_response()
}

async fn delete_workspace(session: Session, State(state): State<AuthState>, Path(id): Path<Uuid>) -> impl IntoResponse {
    if !is_super_admin(&session, &state.pool).await {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }

    sqlx::query("DELETE FROM workspaces WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await
        .ok();

    (axum::http::StatusCode::OK, "Deleted").into_response()
}

#[derive(Serialize)]
pub struct AdminStatsDto {
    pub total_users: i64,
    pub total_workspaces: i64,
    pub total_pages: i64,
    pub total_attachments_size: i64,
}

async fn get_stats(session: Session, State(state): State<AuthState>) -> impl IntoResponse {
    if !is_super_admin(&session, &state.pool).await {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }

    let total_users: i64 = sqlx::query_scalar("SELECT count(*) FROM users").fetch_one(&state.pool).await.unwrap_or(0);
    let total_workspaces: i64 = sqlx::query_scalar("SELECT count(*) FROM workspaces").fetch_one(&state.pool).await.unwrap_or(0);
    let total_pages: i64 = sqlx::query_scalar("SELECT count(*) FROM pages").fetch_one(&state.pool).await.unwrap_or(0);
    let total_attachments_size: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(file_size), 0) FROM attachments").fetch_one(&state.pool).await.unwrap_or(0);

    Json(AdminStatsDto {
        total_users,
        total_workspaces,
        total_pages,
        total_attachments_size,
    }).into_response()
}

#[derive(Serialize, sqlx::FromRow)]
pub struct PublicSettings {
    pub app_name: String,
    pub favicon_url: Option<String>,
    pub allow_public_signups: bool,
    pub umami_website_id: Option<String>,
    pub umami_script_url: Option<String>,
}

pub async fn get_public_settings(State(state): State<AuthState>) -> impl IntoResponse {
    let settings = sqlx::query_as::<_, PublicSettings>(
        "SELECT app_name, favicon_url, allow_public_signups, umami_website_id, umami_script_url FROM global_settings WHERE id = 1"
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(PublicSettings {
        app_name: "Looty".to_string(),
        favicon_url: None,
        allow_public_signups: true,
        umami_website_id: None,
        umami_script_url: None,
    });

    Json(settings).into_response()
}

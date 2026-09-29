use axum::{
    extract::{Path, State, Extension},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, FromRow};
use uuid::Uuid;
use tower_sessions::Session;
use crate::auth::AuthState;
use crate::workspace::get_workspace_role;
use std::sync::Arc;

#[derive(Serialize, Deserialize, FromRow)]
pub struct Page {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub icon: Option<String>,
    pub title: String,
    pub content: Option<String>,
    pub created_by: Uuid,
    pub updated_by: Option<Uuid>,
    pub position: f64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub share_token: Option<Uuid>,
    pub is_public: Option<bool>,
    // Champs calculés par utilisateur : absents des colonnes de `pages`
    // (donc `sqlx(default)` pour les `RETURNING *` / `SELECT *`)
    #[sqlx(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_favorite: Option<bool>,
    #[sqlx(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_count: Option<i32>,
    #[sqlx(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_viewed_at: Option<chrono::DateTime<chrono::Utc>>,
    #[sqlx(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_view_count: Option<i64>,
}

#[derive(Deserialize)]
pub struct CreatePage {
    pub title: String,
    pub icon: Option<String>,
    pub parent_id: Option<Uuid>,
    pub content: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdatePage {
    pub title: Option<String>,
    pub icon: Option<String>,
    pub parent_id: Option<Uuid>,
    pub content: Option<String>,
    pub position: Option<f64>,
}

pub fn page_routes(state: AuthState) -> Router {
    Router::new()
        .route("/", get(list_pages).post(create_page))
        .route("/:page_id", get(get_page).put(update_page).delete(delete_page))
        .route("/:page_id/favorite", post(toggle_favorite))
        .route("/:page_id/view", post(record_view))
        .route("/:page_id/share", post(share_page).delete(revoke_share))
        .route("/:page_id/permissions", get(get_page_permissions).put(set_page_permission))
        .with_state(state.pool)
}

async fn get_user_id(session: &Session) -> Option<String> {
    session.get("user_id").await.unwrap_or(None)
}

async fn list_pages(
    State(pool): State<PgPool>,
    Path(workspace_id): Path<Uuid>,
    session: Session,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let user_id = get_user_id(&session).await.ok_or((StatusCode::UNAUTHORIZED, "Unauthorized".to_string()))?;
    
    let role = get_workspace_role(&pool, &user_id, &workspace_id).await;
    if role.is_none() {
        return Err((StatusCode::FORBIDDEN, "Forbidden".to_string()));
    }
    
    let is_admin = role == Some("owner".to_string()) || role == Some("admin".to_string());
    let user_uuid = Uuid::parse_str(&user_id).unwrap_or_default();

    let pages = sqlx::query_as::<_, Page>(
        "SELECT p.*, 
            CASE WHEN pf.page_id IS NOT NULL THEN true ELSE false END as is_favorite,
            pv.view_count,
            pv.last_viewed_at,
            (SELECT COALESCE(SUM(view_count), 0) FROM page_views WHERE page_id = p.id) as total_view_count
         FROM pages p
         LEFT JOIN page_permissions pp ON pp.page_id = p.id AND pp.user_id = $2
         LEFT JOIN page_favorites pf ON pf.page_id = p.id AND pf.user_id = $2
         LEFT JOIN page_views pv ON pv.page_id = p.id AND pv.user_id = $2
         WHERE p.workspace_id = $1 AND ($3 OR pp.role IS NULL OR pp.role != 'blocked')
         ORDER BY p.position ASC, p.created_at ASC"
    )
    .bind(workspace_id)
    .bind(user_uuid)
    .bind(is_admin)
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(pages))
}

async fn get_page(
    State(pool): State<PgPool>,
    Path((workspace_id, page_id)): Path<(Uuid, Uuid)>,
    session: Session,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let user_id = get_user_id(&session).await.ok_or((StatusCode::UNAUTHORIZED, "Unauthorized".to_string()))?;
    
    let role = get_workspace_role(&pool, &user_id, &workspace_id).await;
    if role.is_none() {
        return Err((StatusCode::FORBIDDEN, "Forbidden".to_string()));
    }
    let is_admin = role == Some("owner".to_string()) || role == Some("admin".to_string());
    let user_uuid = Uuid::parse_str(&user_id).unwrap_or_default();

    let page = sqlx::query_as::<_, Page>(
        "SELECT p.*,
            CASE WHEN pf.page_id IS NOT NULL THEN true ELSE false END as is_favorite,
            pv.view_count,
            pv.last_viewed_at,
            (SELECT COALESCE(SUM(view_count), 0) FROM page_views WHERE page_id = p.id) as total_view_count
         FROM pages p
         LEFT JOIN page_permissions pp ON pp.page_id = p.id AND pp.user_id = $3
         LEFT JOIN page_favorites pf ON pf.page_id = p.id AND pf.user_id = $3
         LEFT JOIN page_views pv ON pv.page_id = p.id AND pv.user_id = $3
         WHERE p.id = $1 AND p.workspace_id = $2 AND ($4 OR pp.role IS NULL OR pp.role != 'blocked')"
    )
    .bind(page_id)
    .bind(workspace_id)
    .bind(user_uuid)
    .bind(is_admin)
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "Page not found".to_string()))?;

    Ok(Json(page))
}

async fn create_page(
    State(pool): State<PgPool>,
    Path(workspace_id): Path<Uuid>,
    Extension(ws_state): Extension<Arc<crate::ws::AppState>>,
    session: Session,
    Json(payload): Json<CreatePage>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let user_id = get_user_id(&session).await.ok_or((StatusCode::UNAUTHORIZED, "Unauthorized".to_string()))?;
    let user_uuid = Uuid::parse_str(&user_id).map_err(|_| (StatusCode::BAD_REQUEST, "Invalid User ID".to_string()))?;

    let role = get_workspace_role(&pool, &user_id, &workspace_id).await;
    if role.is_none() || role == Some("viewer".to_string()) {
        return Err((StatusCode::FORBIDDEN, "Forbidden: Viewers cannot create pages".to_string()));
    }

    let page = sqlx::query_as::<_, Page>(
        "INSERT INTO pages (workspace_id, parent_id, icon, title, content, created_by)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING *"
    )
    .bind(workspace_id)
    .bind(payload.parent_id)
    .bind(payload.icon)
    .bind(payload.title)
    .bind(payload.content)
    .bind(user_uuid)
    .fetch_one(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let ws_msg = serde_json::json!({
        "type": "PAGE_CREATED",
        "page": &page
    }).to_string();
    let tx = ws_state.get_or_create_room(&workspace_id.to_string()).await;
    let _ = tx.send(ws_msg);

    Ok((StatusCode::CREATED, Json(page)))
}

async fn update_page(
    State(pool): State<PgPool>,
    Path((workspace_id, page_id)): Path<(Uuid, Uuid)>,
    Extension(ws_state): Extension<Arc<crate::ws::AppState>>,
    session: Session,
    Json(payload): Json<UpdatePage>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let user_id = get_user_id(&session).await.ok_or((StatusCode::UNAUTHORIZED, "Unauthorized".to_string()))?;
    let user_uuid = Uuid::parse_str(&user_id).map_err(|_| (StatusCode::BAD_REQUEST, "Invalid User ID".to_string()))?;
    
    let role = get_workspace_role(&pool, &user_id, &workspace_id).await;
    if role.is_none() || role == Some("viewer".to_string()) {
        return Err((StatusCode::FORBIDDEN, "Forbidden: Viewers cannot edit pages".to_string()));
    }

    let page = sqlx::query_as::<_, Page>(
        "UPDATE pages SET 
            title = COALESCE($1, title),
            icon = COALESCE($2, icon),
            parent_id = $3,
            content = COALESCE($4, content),
            position = COALESCE($5, position),
            updated_by = $6,
            updated_at = CURRENT_TIMESTAMP
         WHERE id = $7 AND workspace_id = $8
         RETURNING *"
    )
    .bind(payload.title)
    .bind(payload.icon)
    .bind(payload.parent_id)
    .bind(payload.content)
    .bind(payload.position)
    .bind(user_uuid)
    .bind(page_id)
    .bind(workspace_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let ws_msg = serde_json::json!({
        "type": "PAGE_UPDATED",
        "page": &page
    }).to_string();
    let tx = ws_state.get_or_create_room(&workspace_id.to_string()).await;
    let _ = tx.send(ws_msg);

    Ok(Json(page))
}

async fn delete_page(
    State(pool): State<PgPool>,
    Path((workspace_id, page_id)): Path<(Uuid, Uuid)>,
    Extension(ws_state): Extension<Arc<crate::ws::AppState>>,
    session: Session,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let user_id = get_user_id(&session).await.ok_or((StatusCode::UNAUTHORIZED, "Unauthorized".to_string()))?;
    
    let role = get_workspace_role(&pool, &user_id, &workspace_id).await;
    if role.is_none() || role == Some("viewer".to_string()) {
        return Err((StatusCode::FORBIDDEN, "Forbidden: Viewers cannot delete pages".to_string()));
    }

    let result = sqlx::query(
        "DELETE FROM pages WHERE id = $1 AND workspace_id = $2"
    )
    .bind(page_id)
    .bind(workspace_id)
    .execute(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Page not found".to_string()));
    }

    let ws_msg = serde_json::json!({
        "type": "PAGE_DELETED",
        "page_id": page_id
    }).to_string();
    let tx = ws_state.get_or_create_room(&workspace_id.to_string()).await;
    let _ = tx.send(ws_msg);

    Ok(StatusCode::NO_CONTENT)
}

pub async fn share_page(
    State(pool): State<PgPool>,
    Path((workspace_id, page_id)): Path<(Uuid, Uuid)>,
    session: Session,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let user_id = get_user_id(&session).await.ok_or((StatusCode::UNAUTHORIZED, "Unauthorized".to_string()))?;
    let role = get_workspace_role(&pool, &user_id, &workspace_id).await.ok_or((StatusCode::FORBIDDEN, "Forbidden".to_string()))?;
    
    if role == "viewer" {
        return Err((StatusCode::FORBIDDEN, "Viewers cannot share pages".to_string()));
    }

    let token = Uuid::new_v4();
    let page = sqlx::query_as::<_, Page>(
        "UPDATE pages SET share_token = $1, is_public = true WHERE id = $2 AND workspace_id = $3 RETURNING *"
    )
    .bind(token)
    .bind(page_id)
    .bind(workspace_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(page))
}

pub async fn revoke_share(
    State(pool): State<PgPool>,
    Path((workspace_id, page_id)): Path<(Uuid, Uuid)>,
    session: Session,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let user_id = get_user_id(&session).await.ok_or((StatusCode::UNAUTHORIZED, "Unauthorized".to_string()))?;
    let role = get_workspace_role(&pool, &user_id, &workspace_id).await.ok_or((StatusCode::FORBIDDEN, "Forbidden".to_string()))?;
    
    if role == "viewer" {
        return Err((StatusCode::FORBIDDEN, "Viewers cannot revoke share".to_string()));
    }

    let page = sqlx::query_as::<_, Page>(
        "UPDATE pages SET share_token = NULL, is_public = false WHERE id = $1 AND workspace_id = $2 RETURNING *"
    )
    .bind(page_id)
    .bind(workspace_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(page))
}

pub async fn get_public_page(
    State(pool): State<PgPool>,
    Path(token): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let page = sqlx::query_as::<_, Page>(
        "SELECT * FROM pages WHERE share_token = $1 AND is_public = true"
    )
    .bind(token)
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "Page not found or not public".to_string()))?;

    Ok(Json(page))
}

#[derive(Serialize, sqlx::FromRow)]
pub struct PagePermission {
    pub id: Uuid,
    pub page_id: Uuid,
    pub user_id: Uuid,
    pub role: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
pub struct SetPagePermissionReq {
    pub user_id: Uuid,
    pub role: String,
}

async fn get_page_permissions(
    State(pool): State<PgPool>,
    Path((workspace_id, page_id)): Path<(Uuid, Uuid)>,
    session: Session,
) -> Result<Json<Vec<PagePermission>>, (StatusCode, String)> {
    let user_id = get_user_id(&session).await.ok_or((StatusCode::UNAUTHORIZED, "Unauthorized".to_string()))?;
    
    let role = get_workspace_role(&pool, &user_id, &workspace_id).await;
    if role.is_none() {
        return Err((StatusCode::FORBIDDEN, "Forbidden".to_string()));
    }

    let perms = sqlx::query_as::<_, PagePermission>(
        "SELECT id, page_id, user_id, role, created_at FROM page_permissions WHERE page_id = $1"
    )
    .bind(page_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    Ok(Json(perms))
}

async fn set_page_permission(
    State(pool): State<PgPool>,
    Path((workspace_id, page_id)): Path<(Uuid, Uuid)>,
    session: Session,
    Json(payload): Json<SetPagePermissionReq>,
) -> Result<Json<PagePermission>, (StatusCode, String)> {
    let user_id = get_user_id(&session).await.ok_or((StatusCode::UNAUTHORIZED, "Unauthorized".to_string()))?;
    
    let role = get_workspace_role(&pool, &user_id, &workspace_id).await;
    if role != Some("admin".to_string()) && role != Some("owner".to_string()) {
        return Err((StatusCode::FORBIDDEN, "Forbidden: Only admins can change permissions".to_string()));
    }

    if payload.role == "none" {
        sqlx::query("DELETE FROM page_permissions WHERE page_id = $1 AND user_id = $2")
            .bind(page_id).bind(payload.user_id).execute(&pool).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        return Err((StatusCode::NO_CONTENT, "".to_string())); // Special return just to signal success for delete
    }

    let perm = sqlx::query_as::<_, PagePermission>(
        "INSERT INTO page_permissions (page_id, user_id, role) VALUES ($1, $2, $3) 
         ON CONFLICT (page_id, user_id) DO UPDATE SET role = $3 RETURNING *"
    )
    .bind(page_id)
    .bind(payload.user_id)
    .bind(payload.role)
    .fetch_one(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(perm))
}

pub async fn toggle_favorite(
    State(pool): State<PgPool>,
    Path((_workspace_id, page_id)): Path<(Uuid, Uuid)>,
    session: Session,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let user_id = get_user_id(&session).await.ok_or((StatusCode::UNAUTHORIZED, "Unauthorized".to_string()))?;
    let user_uuid = Uuid::parse_str(&user_id).unwrap_or_default();

    let exists: Option<bool> = sqlx::query_scalar("SELECT true FROM page_favorites WHERE page_id = $1 AND user_id = $2")
        .bind(page_id).bind(user_uuid).fetch_optional(&pool).await.unwrap_or(None);

    if exists.is_some() {
        sqlx::query("DELETE FROM page_favorites WHERE page_id = $1 AND user_id = $2")
            .bind(page_id).bind(user_uuid).execute(&pool).await.ok();
    } else {
        sqlx::query("INSERT INTO page_favorites (page_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
            .bind(page_id).bind(user_uuid).execute(&pool).await.ok();
    }

    Ok(StatusCode::OK)
}

pub async fn record_view(
    State(pool): State<PgPool>,
    Path((_workspace_id, page_id)): Path<(Uuid, Uuid)>,
    session: Session,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let user_id = get_user_id(&session).await.ok_or((StatusCode::UNAUTHORIZED, "Unauthorized".to_string()))?;
    let user_uuid = Uuid::parse_str(&user_id).unwrap_or_default();

    sqlx::query(
        "INSERT INTO page_views (page_id, user_id, last_viewed_at, view_count) 
         VALUES ($1, $2, CURRENT_TIMESTAMP, 1) 
         ON CONFLICT (page_id, user_id) 
         DO UPDATE SET last_viewed_at = CURRENT_TIMESTAMP, view_count = page_views.view_count + 1"
    )
    .bind(page_id)
    .bind(user_uuid)
    .execute(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::OK)
}
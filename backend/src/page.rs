use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, put, delete},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, FromRow};
use uuid::Uuid;
use tower_sessions::Session;
use crate::auth::AuthState;

#[derive(Serialize, Deserialize, FromRow)]
pub struct Page {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub title: String,
    pub content: Option<String>,
    pub created_by: Uuid,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
pub struct CreatePage {
    pub title: String,
    pub parent_id: Option<Uuid>,
    pub content: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdatePage {
    pub title: Option<String>,
    pub parent_id: Option<Uuid>,
    pub content: Option<String>,
}

pub fn page_routes(state: AuthState) -> Router {
    Router::new()
        .route("/", get(list_pages).post(create_page))
        .route("/:page_id", get(get_page).put(update_page).delete(delete_page))
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
    let _user_id = get_user_id(&session).await.ok_or((StatusCode::UNAUTHORIZED, "Unauthorized".to_string()))?;

    let pages = sqlx::query_as::<_, Page>(
        "SELECT * FROM pages WHERE workspace_id = $1 ORDER BY title ASC"
    )
    .bind(workspace_id)
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
    let _user_id = get_user_id(&session).await.ok_or((StatusCode::UNAUTHORIZED, "Unauthorized".to_string()))?;

    let page = sqlx::query_as::<_, Page>(
        "SELECT * FROM pages WHERE id = $1 AND workspace_id = $2"
    )
    .bind(page_id)
    .bind(workspace_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "Page not found".to_string()))?;

    Ok(Json(page))
}

async fn create_page(
    State(pool): State<PgPool>,
    Path(workspace_id): Path<Uuid>,
    session: Session,
    Json(payload): Json<CreatePage>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let user_id = get_user_id(&session).await.ok_or((StatusCode::UNAUTHORIZED, "Unauthorized".to_string()))?;
    let user_uuid = Uuid::parse_str(&user_id).map_err(|_| (StatusCode::BAD_REQUEST, "Invalid User ID".to_string()))?;

    let page = sqlx::query_as::<_, Page>(
        "INSERT INTO pages (workspace_id, parent_id, title, content, created_by)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING *"
    )
    .bind(workspace_id)
    .bind(payload.parent_id)
    .bind(payload.title)
    .bind(payload.content)
    .bind(user_uuid)
    .fetch_one(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((StatusCode::CREATED, Json(page)))
}

async fn update_page(
    State(pool): State<PgPool>,
    Path((workspace_id, page_id)): Path<(Uuid, Uuid)>,
    session: Session,
    Json(payload): Json<UpdatePage>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let _user_id = get_user_id(&session).await.ok_or((StatusCode::UNAUTHORIZED, "Unauthorized".to_string()))?;

    let page = sqlx::query_as::<_, Page>(
        "UPDATE pages SET 
            title = COALESCE($1, title),
            parent_id = $2,
            content = COALESCE($3, content),
            updated_at = CURRENT_TIMESTAMP
         WHERE id = $4 AND workspace_id = $5
         RETURNING *"
    )
    .bind(payload.title)
    .bind(payload.parent_id)
    .bind(payload.content)
    .bind(page_id)
    .bind(workspace_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(page))
}

async fn delete_page(
    State(pool): State<PgPool>,
    Path((workspace_id, page_id)): Path<(Uuid, Uuid)>,
    session: Session,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let _user_id = get_user_id(&session).await.ok_or((StatusCode::UNAUTHORIZED, "Unauthorized".to_string()))?;

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

    Ok(StatusCode::NO_CONTENT)
}

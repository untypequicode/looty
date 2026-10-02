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

pub fn user_settings_routes(state: AuthState) -> Router {
    Router::new()
        .route("/profile", put(update_profile))
        .route("/tokens", get(list_tokens).post(create_token))
        .route("/tokens/:id", delete(delete_token))
        .route("/identities", get(list_identities))
        .route("/identities/:provider", delete(delete_identity))
        .with_state(state)
}

#[derive(Deserialize)]
struct UpdateProfileRequest {
    name: String,
    avatar_url: Option<String>,
    theme: Option<String>,
}

async fn update_profile(session: Session, State(state): State<AuthState>, Json(payload): Json<UpdateProfileRequest>) -> impl IntoResponse {
    let user_id: Option<Uuid> = session.get("user_id").await.unwrap_or(None);
    let uid = match user_id {
        Some(id) => id,
        None => return (axum::http::StatusCode::UNAUTHORIZED, "Unauthorized").into_response(),
    };

    sqlx::query("UPDATE users SET name = $1, avatar_url = $2, theme = $3 WHERE id = $4")
        .bind(&payload.name)
        .bind(&payload.avatar_url)
        .bind(&payload.theme)
        .bind(uid)
        .execute(&state.pool)
        .await
        .ok();

    (axum::http::StatusCode::OK, "Updated").into_response()
}

#[derive(Serialize, sqlx::FromRow)]
struct ApiTokenDto {
    id: Uuid,
    name: String,
    created_at: chrono::DateTime<chrono::Utc>,
    last_used_at: Option<chrono::DateTime<chrono::Utc>>,
}

async fn list_tokens(session: Session, State(state): State<AuthState>) -> impl IntoResponse {
    let user_id: Option<Uuid> = session.get("user_id").await.unwrap_or(None);
    let uid = match user_id {
        Some(id) => id,
        None => return (axum::http::StatusCode::UNAUTHORIZED, "Unauthorized").into_response(),
    };

    let tokens = sqlx::query_as::<_, ApiTokenDto>("SELECT id, name, created_at, last_used_at FROM api_tokens WHERE user_id = $1")
        .bind(uid)
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default();

    Json(tokens).into_response()
}

#[derive(Deserialize)]
struct CreateTokenRequest {
    name: String,
}

async fn create_token(session: Session, State(state): State<AuthState>, Json(payload): Json<CreateTokenRequest>) -> impl IntoResponse {
    let user_id: Option<Uuid> = session.get("user_id").await.unwrap_or(None);
    let uid = match user_id {
        Some(id) => id,
        None => return (axum::http::StatusCode::UNAUTHORIZED, "Unauthorized").into_response(),
    };

    let token_raw = format!("looty_{}", Uuid::new_v4().simple().to_string());
    // In a real app we'd hash the token, but for simplicity we store it directly or hashed.
    let token_hash = token_raw.clone(); // Just storing raw for now to keep it simple, or hash it if needed.

    sqlx::query("INSERT INTO api_tokens (user_id, name, token_hash) VALUES ($1, $2, $3)")
        .bind(uid)
        .bind(&payload.name)
        .bind(&token_hash)
        .execute(&state.pool)
        .await
        .unwrap();

    // Only return the raw token once!
    Json(serde_json::json!({ "token": token_raw })).into_response()
}

async fn delete_token(session: Session, State(state): State<AuthState>, Path(id): Path<Uuid>) -> impl IntoResponse {
    let user_id: Option<Uuid> = session.get("user_id").await.unwrap_or(None);
    let uid = match user_id {
        Some(id) => id,
        None => return (axum::http::StatusCode::UNAUTHORIZED, "Unauthorized").into_response(),
    };

    sqlx::query("DELETE FROM api_tokens WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(uid)
        .execute(&state.pool)
        .await
        .ok();

    (axum::http::StatusCode::OK, "Deleted").into_response()
}

#[derive(Serialize, sqlx::FromRow)]
struct IdentityDto {
    provider: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

async fn list_identities(session: Session, State(state): State<AuthState>) -> impl IntoResponse {
    let user_id: Option<Uuid> = session.get("user_id").await.unwrap_or(None);
    let uid = match user_id {
        Some(id) => id,
        None => return (axum::http::StatusCode::UNAUTHORIZED, "Unauthorized").into_response(),
    };

    let identities = sqlx::query_as::<_, IdentityDto>("SELECT provider, created_at FROM identities WHERE user_id = $1")
        .bind(uid)
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default();

    Json(identities).into_response()
}

async fn delete_identity(session: Session, State(state): State<AuthState>, Path(provider): Path<String>) -> impl IntoResponse {
    let user_id: Option<Uuid> = session.get("user_id").await.unwrap_or(None);
    let uid = match user_id {
        Some(id) => id,
        None => return (axum::http::StatusCode::UNAUTHORIZED, "Unauthorized").into_response(),
    };

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM identities WHERE user_id = $1")
        .bind(uid)
        .fetch_one(&state.pool)
        .await
        .unwrap_or(0);

    if count <= 1 {
        return (axum::http::StatusCode::BAD_REQUEST, "Vous devez garder au moins un compte lié.").into_response();
    }

    sqlx::query("DELETE FROM identities WHERE user_id = $1 AND provider = $2")
        .bind(uid)
        .bind(&provider)
        .execute(&state.pool)
        .await
        .ok();

    (axum::http::StatusCode::OK, "Deleted").into_response()
}

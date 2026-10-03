use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::access::{hash_token, AuthUser};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/profile", put(update_profile))
        .route("/preferences", put(update_preferences))
        .route("/tokens", get(list_tokens).post(create_token))
        .route("/tokens/:id", delete(delete_token))
        .route("/identities", get(list_identities))
        .route("/identities/:provider", delete(delete_identity))
}

fn check_avatar(url: &Option<String>) -> ApiResult<Option<String>> {
    match url.as_deref().map(str::trim) {
        None | Some("") => Ok(None),
        Some(u) if (u.starts_with("/uploads/") && !u.contains("..")) || (u.starts_with("https://") && u.len() <= 1024) => {
            Ok(Some(u.to_string()))
        }
        _ => Err(ApiError::bad_request("invalid_avatar_url")),
    }
}

#[derive(Deserialize)]
struct UpdateProfileRequest {
    name: String,
    avatar_url: Option<String>,
}

async fn update_profile(
    State(pool): State<PgPool>,
    user: AuthUser,
    Json(p): Json<UpdateProfileRequest>,
) -> ApiResult<StatusCode> {
    let name = p.name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(ApiError::bad_request("invalid_name"));
    }
    sqlx::query("UPDATE users SET name = $1, avatar_url = $2, updated_at = NOW() WHERE id = $3")
        .bind(name)
        .bind(check_avatar(&p.avatar_url)?)
        .bind(user.id)
        .execute(&pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct PreferencesRequest {
    theme: Option<String>,
    locale: Option<String>,
}

async fn update_preferences(
    State(pool): State<PgPool>,
    user: AuthUser,
    Json(p): Json<PreferencesRequest>,
) -> ApiResult<StatusCode> {
    if let Some(t) = &p.theme {
        if !matches!(t.as_str(), "system" | "light" | "dark") {
            return Err(ApiError::bad_request("invalid_theme"));
        }
    }
    if let Some(l) = &p.locale {
        if !matches!(l.as_str(), "fr" | "en") {
            return Err(ApiError::bad_request("invalid_locale"));
        }
    }
    sqlx::query("UPDATE users SET theme = COALESCE($1, theme), locale = COALESCE($2, locale), updated_at = NOW() WHERE id = $3")
        .bind(p.theme)
        .bind(p.locale)
        .bind(user.id)
        .execute(&pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize, sqlx::FromRow)]
struct ApiTokenDto {
    id: Uuid,
    name: String,
    scope: String,
    created_at: chrono::DateTime<chrono::Utc>,
    last_used_at: Option<chrono::DateTime<chrono::Utc>>,
}

async fn list_tokens(State(pool): State<PgPool>, user: AuthUser) -> ApiResult<Json<Vec<ApiTokenDto>>> {
    Ok(Json(
        sqlx::query_as("SELECT id, name, scope, created_at, last_used_at FROM api_tokens WHERE user_id = $1 ORDER BY created_at DESC")
            .bind(user.id)
            .fetch_all(&pool)
            .await?,
    ))
}

#[derive(Deserialize)]
struct CreateTokenRequest {
    name: String,
    /// 'api' (par défaut) ou 'caldav' (mot de passe d'application CalDAV).
    scope: Option<String>,
}

async fn create_token(
    State(pool): State<PgPool>,
    user: AuthUser,
    Json(p): Json<CreateTokenRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    let name = p.name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(ApiError::bad_request("invalid_name"));
    }
    let scope = p.scope.as_deref().unwrap_or("api");
    if !matches!(scope, "api" | "caldav") {
        return Err(ApiError::bad_request("invalid_scope"));
    }
    let raw = format!("looty_{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let id: Uuid = sqlx::query_scalar("INSERT INTO api_tokens (user_id, name, token_hash, scope) VALUES ($1, $2, $3, $4) RETURNING id")
        .bind(user.id)
        .bind(name)
        .bind(hash_token(&raw))
        .bind(scope)
        .fetch_one(&pool)
        .await?;
    // Le jeton brut n'est renvoyé qu'une seule fois : seul son hash est stocké.
    Ok(Json(serde_json::json!({ "id": id, "token": raw })))
}

async fn delete_token(State(pool): State<PgPool>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<StatusCode> {
    sqlx::query("DELETE FROM api_tokens WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user.id)
        .execute(&pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize, sqlx::FromRow)]
struct IdentityDto {
    provider: String,
    provider_name: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

async fn list_identities(State(pool): State<PgPool>, user: AuthUser) -> ApiResult<Json<Vec<IdentityDto>>> {
    Ok(Json(
        sqlx::query_as(
            "SELECT i.provider, p.name AS provider_name, i.created_at FROM identities i
             LEFT JOIN oauth_providers p ON p.id::text = i.provider WHERE i.user_id = $1 ORDER BY i.created_at",
        )
        .bind(user.id)
        .fetch_all(&pool)
        .await?,
    ))
}

async fn delete_identity(State(pool): State<PgPool>, user: AuthUser, Path(provider): Path<String>) -> ApiResult<StatusCode> {
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM identities WHERE user_id = $1")
        .bind(user.id)
        .fetch_one(&pool)
        .await?;
    if count <= 1 {
        return Err(ApiError::Conflict("last_identity".into()));
    }
    sqlx::query("DELETE FROM identities WHERE user_id = $1 AND provider = $2")
        .bind(user.id)
        .bind(&provider)
        .execute(&pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

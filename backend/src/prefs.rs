//! Préférences synchronisées entre appareils et bloc-notes personnel.
//!
//! - `users.prefs` : réglages globaux (correcteur orthographique, lecture…) ;
//! - `workspace_members.prefs` : réglages propres à un espace (tableau de bord, barre latérale…) ;
//! - `user_notepads` : un seul bloc-notes par personne, le même sur toutes les pages.
//!
//! Les mises à jour sont fusionnées clé par clé ; une clé à `null` est supprimée.

use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::access::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

const MAX_PREFS: usize = 64 * 1024;
const MAX_NOTEPAD: usize = 512 * 1024;

pub fn user_routes() -> Router<AppState> {
    Router::new().route("/prefs", get(get_user).patch(patch_user)).route("/notepad", get(get_notepad).put(put_notepad))
}

pub fn workspace_routes() -> Router<AppState> {
    Router::new().route("/", get(get_member).patch(patch_member))
}

fn check(patch: &serde_json::Value) -> ApiResult<()> {
    if !patch.is_object() {
        return Err(ApiError::bad_request("invalid_value"));
    }
    Ok(())
}

async fn get_user(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<serde_json::Value>> {
    let v: serde_json::Value = sqlx::query_scalar("SELECT prefs FROM users WHERE id = $1").bind(user.id).fetch_one(&state.pool).await?;
    Ok(Json(v))
}

async fn patch_user(State(state): State<AppState>, user: AuthUser, Json(patch): Json<serde_json::Value>) -> ApiResult<Json<serde_json::Value>> {
    check(&patch)?;
    let v: serde_json::Value = sqlx::query_scalar("UPDATE users SET prefs = jsonb_strip_nulls(prefs || $2) WHERE id = $1 RETURNING prefs")
        .bind(user.id)
        .bind(&patch)
        .fetch_one(&state.pool)
        .await?;
    if v.to_string().len() > MAX_PREFS {
        return Err(ApiError::bad_request("too_large"));
    }
    Ok(Json(v))
}

async fn get_member(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>) -> ApiResult<Json<serde_json::Value>> {
    // Invité : pas de préférences d'espace (rien n'est enregistré).
    if crate::access::require_member_or_guest(&state.pool, &user, ws).await? == crate::access::Role::Guest {
        return Ok(Json(serde_json::json!({})));
    }
    let v: serde_json::Value = sqlx::query_scalar("SELECT prefs FROM workspace_members WHERE workspace_id = $1 AND user_id = $2")
        .bind(ws)
        .bind(user.id.to_string())
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(v))
}

async fn patch_member(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>, Json(patch): Json<serde_json::Value>) -> ApiResult<Json<serde_json::Value>> {
    if crate::access::require_member_or_guest(&state.pool, &user, ws).await? == crate::access::Role::Guest {
        return Ok(Json(serde_json::json!({})));
    }
    check(&patch)?;
    if patch.to_string().len() > MAX_PREFS {
        return Err(ApiError::bad_request("too_large"));
    }
    let v: serde_json::Value = sqlx::query_scalar(
        "UPDATE workspace_members SET prefs = jsonb_strip_nulls(prefs || $3) WHERE workspace_id = $1 AND user_id = $2 RETURNING prefs",
    )
    .bind(ws)
    .bind(user.id.to_string())
    .bind(&patch)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(v))
}

async fn get_notepad(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<serde_json::Value>> {
    let row: Option<(String, DateTime<Utc>)> = sqlx::query_as("SELECT content, updated_at FROM user_notepads WHERE user_id = $1").bind(user.id).fetch_optional(&state.pool).await?;
    let (content, updated_at) = row.map(|(c, u)| (c, Some(u))).unwrap_or_default();
    Ok(Json(serde_json::json!({ "content": content, "updated_at": updated_at })))
}

#[derive(Deserialize)]
struct NotepadReq {
    content: String,
}

async fn put_notepad(State(state): State<AppState>, user: AuthUser, Json(p): Json<NotepadReq>) -> ApiResult<Json<serde_json::Value>> {
    if p.content.len() > MAX_NOTEPAD {
        return Err(ApiError::bad_request("too_large"));
    }
    let updated_at: DateTime<Utc> = sqlx::query_scalar(
        "INSERT INTO user_notepads (user_id, content) VALUES ($1, $2)
         ON CONFLICT (user_id) DO UPDATE SET content = EXCLUDED.content, updated_at = NOW() RETURNING updated_at",
    )
    .bind(user.id)
    .bind(&p.content)
    .fetch_one(&state.pool)
    .await?;
    // Les autres appareils / onglets ouverts rechargent le bloc-notes.
    state.emit_user(user.id, serde_json::json!({ "type": "NOTEPAD_UPDATED", "updated_at": updated_at })).await;
    Ok(Json(serde_json::json!({ "updated_at": updated_at })))
}

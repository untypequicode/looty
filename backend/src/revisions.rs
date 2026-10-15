//! Historique des versions d'une page.
//!
//! Le serveur de collaboration enregistre une version par session d'édition (même auteur,
//! dix minutes au plus) ; on peut aussi nommer une version. La restauration se fait côté
//! client, dans l'éditeur collaboratif : le contenu de l'ancienne version y remplace le
//! contenu actuel (les autres personnes connectées voient le changement en direct), après
//! un instantané « avant restauration » créé ici.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use base64::Engine;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::access::{require_page_read, require_page_write, AuthUser};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/{page_id}/revisions", get(list).post(create))
        .route("/{page_id}/revisions/{rev_id}", get(get_one).patch(rename).delete(remove))
}

#[derive(Serialize, sqlx::FromRow)]
struct RevisionDto {
    id: Uuid,
    kind: String,
    label: Option<String>,
    title: String,
    created_by: Option<Uuid>,
    author_name: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    size: i32,
}

async fn list(State(state): State<AppState>, user: AuthUser, Path(page_id): Path<Uuid>) -> ApiResult<Json<Vec<RevisionDto>>> {
    require_page_read(&state.pool, &user, page_id).await?;
    let rows: Vec<RevisionDto> = sqlx::query_as(
        "SELECT r.id, r.kind, r.label, r.title, r.created_by, u.name AS author_name, r.created_at, r.updated_at, length(r.yjs_state) AS size
         FROM page_revisions r LEFT JOIN users u ON u.id = r.created_by
         WHERE r.page_id = $1 ORDER BY r.updated_at DESC LIMIT 500",
    )
    .bind(page_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

async fn get_one(State(state): State<AppState>, user: AuthUser, Path((page_id, rev_id)): Path<(Uuid, Uuid)>) -> ApiResult<Json<serde_json::Value>> {
    require_page_read(&state.pool, &user, page_id).await?;
    let row: Option<(Vec<u8>, String)> = sqlx::query_as("SELECT yjs_state, title FROM page_revisions WHERE id = $1 AND page_id = $2")
        .bind(rev_id)
        .bind(page_id)
        .fetch_optional(&state.pool)
        .await?;
    let (state_bytes, title) = row.ok_or(ApiError::NotFound)?;
    Ok(Json(serde_json::json!({ "id": rev_id, "title": title, "yjs_state": base64::engine::general_purpose::STANDARD.encode(state_bytes) })))
}

#[derive(Deserialize)]
struct CreateReq {
    /// manual (version nommée) ou restore (instantané avant une restauration).
    kind: Option<String>,
    label: Option<String>,
}

fn clean_label(l: Option<String>) -> Option<String> {
    l.map(|s| s.trim().chars().take(120).collect::<String>()).filter(|s| !s.is_empty())
}

/// Instantané de l'état actuel de la page (tel qu'enregistré par le serveur de collaboration).
async fn create(State(state): State<AppState>, user: AuthUser, Path(page_id): Path<Uuid>, Json(p): Json<CreateReq>) -> ApiResult<Json<serde_json::Value>> {
    require_page_write(&state.pool, &user, page_id).await?;
    let kind = p.kind.unwrap_or_else(|| "manual".into());
    if !matches!(kind.as_str(), "manual" | "restore") {
        return Err(ApiError::bad_request("invalid_kind"));
    }
    let id: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO page_revisions (page_id, yjs_state, title, kind, label, created_by)
         SELECT id, yjs_state, title, $2, $3, $4 FROM pages WHERE id = $1 AND yjs_state IS NOT NULL RETURNING id",
    )
    .bind(page_id)
    .bind(&kind)
    .bind(clean_label(p.label))
    .bind(user.id)
    .fetch_optional(&state.pool)
    .await?;
    Ok(Json(serde_json::json!({ "id": id })))
}

#[derive(Deserialize)]
struct RenameReq {
    label: Option<String>,
}

/// Nommer une version : elle devient « manuelle » et n'est plus jamais élaguée.
async fn rename(State(state): State<AppState>, user: AuthUser, Path((page_id, rev_id)): Path<(Uuid, Uuid)>, Json(p): Json<RenameReq>) -> ApiResult<StatusCode> {
    require_page_write(&state.pool, &user, page_id).await?;
    let label = clean_label(p.label);
    sqlx::query("UPDATE page_revisions SET label = $3, kind = CASE WHEN $3 IS NOT NULL THEN 'manual' ELSE kind END WHERE id = $1 AND page_id = $2")
        .bind(rev_id)
        .bind(page_id)
        .bind(label)
        .execute(&state.pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn remove(State(state): State<AppState>, user: AuthUser, Path((page_id, rev_id)): Path<(Uuid, Uuid)>) -> ApiResult<StatusCode> {
    require_page_write(&state.pool, &user, page_id).await?;
    sqlx::query("DELETE FROM page_revisions WHERE id = $1 AND page_id = $2 AND kind <> 'auto'").bind(rev_id).bind(page_id).execute(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Élagage quotidien des versions automatiques : toutes pendant 7 jours, puis une par
/// jour jusqu'à 90 jours, puis une par semaine. Les versions nommées sont gardées.
pub async fn prune(pool: &sqlx::PgPool) {
    let res = sqlx::query(
        "DELETE FROM page_revisions r USING (
            SELECT id, row_number() OVER (
                PARTITION BY page_id,
                CASE WHEN updated_at > NOW() - interval '90 days' THEN date_trunc('day', updated_at) ELSE date_trunc('week', updated_at) END
                ORDER BY updated_at DESC) AS n
            FROM page_revisions WHERE kind <> 'manual' AND updated_at < NOW() - interval '7 days'
         ) old
         WHERE r.id = old.id AND old.n > 1",
    )
    .execute(pool)
    .await;
    if let Err(e) = res {
        tracing::warn!("revision prune: {e}");
    }
}

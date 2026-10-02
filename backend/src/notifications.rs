use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::access::{require_member, AuthUser};
use crate::error::ApiResult;
use crate::state::AppState;
use crate::ws::Hub;

#[derive(Serialize, sqlx::FromRow)]
pub struct Notification {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub actor_id: Option<Uuid>,
    pub actor_name: Option<String>,
    pub kind: String,
    pub target: String,
    pub data: serde_json::Value,
    pub read_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Crée une notification et la pousse en temps réel au destinataire.
/// Les erreurs sont journalisées mais jamais propagées (effet de bord non critique).
#[allow(clippy::too_many_arguments)]
pub async fn notify(
    pool: &PgPool,
    hub: &Hub,
    workspace_id: Uuid,
    user_id: Uuid,
    actor_id: Option<Uuid>,
    kind: &str,
    target: &str,
    data: serde_json::Value,
) {
    if Some(user_id) == actor_id {
        return;
    }
    let res = sqlx::query_as::<_, Notification>(
        "WITH n AS (
            INSERT INTO notifications (user_id, workspace_id, actor_id, kind, target, data)
            VALUES ($1, $2, $3, $4, $5, $6) RETURNING *
         )
         SELECT n.id, n.workspace_id, n.actor_id, u.name AS actor_name, n.kind, n.target, n.data, n.read_at, n.created_at
         FROM n LEFT JOIN users u ON u.id = n.actor_id",
    )
    .bind(user_id)
    .bind(workspace_id)
    .bind(actor_id)
    .bind(kind)
    .bind(target)
    .bind(data)
    .fetch_one(pool)
    .await;
    match res {
        Ok(n) => {
            hub.send_to(workspace_id, user_id, serde_json::json!({ "type": "NOTIFICATION", "notification": n }))
                .await
        }
        Err(e) => tracing::warn!("notify failed: {}", e),
    }
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list))
        .route("/read-all", post(read_all))
        .route("/:id/read", put(mark_read))
}

#[derive(Deserialize)]
struct ListQuery {
    workspace_id: Uuid,
}

async fn list(
    State(pool): State<PgPool>,
    user: AuthUser,
    Query(q): Query<ListQuery>,
) -> ApiResult<Json<Vec<Notification>>> {
    require_member(&pool, &user, q.workspace_id).await?;
    let rows = sqlx::query_as::<_, Notification>(
        "SELECT n.id, n.workspace_id, n.actor_id, u.name AS actor_name, n.kind, n.target, n.data, n.read_at, n.created_at
         FROM notifications n LEFT JOIN users u ON u.id = n.actor_id
         WHERE n.user_id = $1 AND n.workspace_id = $2
         ORDER BY n.created_at DESC LIMIT 100",
    )
    .bind(user.id)
    .bind(q.workspace_id)
    .fetch_all(&pool)
    .await?;
    Ok(Json(rows))
}

#[derive(Deserialize)]
struct MarkRead {
    read: bool,
}

async fn mark_read(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<MarkRead>,
) -> ApiResult<StatusCode> {
    sqlx::query(
        "UPDATE notifications SET read_at = CASE WHEN $3 THEN NOW() ELSE NULL END WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(user.id)
    .bind(body.read)
    .execute(&pool)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn read_all(
    State(pool): State<PgPool>,
    user: AuthUser,
    Query(q): Query<ListQuery>,
) -> ApiResult<StatusCode> {
    sqlx::query("UPDATE notifications SET read_at = NOW() WHERE user_id = $1 AND workspace_id = $2 AND read_at IS NULL")
        .bind(user.id)
        .bind(q.workspace_id)
        .execute(&pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

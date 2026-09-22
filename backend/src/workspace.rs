use axum::{
    extract::State,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tower_sessions::Session;
use uuid::Uuid;
use crate::auth::AuthState; // We can reuse AuthState since it has the DB pool

#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct Workspace {
    pub id: Uuid,
    pub name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
pub struct CreateWorkspacePayload {
    pub name: String,
}

pub fn workspace_routes(state: AuthState) -> Router {
    Router::new()
        .route("/", get(list_workspaces).post(create_workspace))
        .with_state(state)
}

async fn list_workspaces(
    session: Session,
    State(state): State<AuthState>,
) -> impl IntoResponse {
    let user_id_opt: Option<Uuid> = session.get("user_id").await.unwrap_or(None);
    
    let user_id = match user_id_opt {
        Some(uid) => uid,
        None => return (axum::http::StatusCode::UNAUTHORIZED, "Unauthorized").into_response(),
    };

    let workspaces = sqlx::query_as::<_, Workspace>(
        r#"
        SELECT w.id, w.name, w.created_at, w.updated_at
        FROM workspaces w
        JOIN workspace_members wm ON w.id = wm.workspace_id
        WHERE wm.user_id = $1
        ORDER BY w.created_at DESC
        "#
    )
    .bind(user_id.to_string()) // We store user_id as VARCHAR(255) in workspace_members
    .fetch_all(&state.pool)
    .await;

    match workspaces {
        Ok(ws) => Json(serde_json::json!({ "workspaces": ws })).into_response(),
        Err(e) => {
            println!("Failed to fetch workspaces: {:?}", e);
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response()
        }
    }
}

async fn create_workspace(
    session: Session,
    State(state): State<AuthState>,
    Json(payload): Json<CreateWorkspacePayload>,
) -> impl IntoResponse {
    let user_id_opt: Option<Uuid> = session.get("user_id").await.unwrap_or(None);
    
    let user_id = match user_id_opt {
        Some(uid) => uid,
        None => return (axum::http::StatusCode::UNAUTHORIZED, "Unauthorized").into_response(),
    };

    if payload.name.trim().is_empty() {
        return (axum::http::StatusCode::BAD_REQUEST, "Workspace name cannot be empty").into_response();
    }

    let mut tx = match state.pool.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            println!("Failed to start transaction: {:?}", e);
            return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Transaction error").into_response();
        }
    };

    let workspace_res = sqlx::query_as::<_, Workspace>(
        "INSERT INTO workspaces (name) VALUES ($1) RETURNING id, name, created_at, updated_at"
    )
    .bind(&payload.name)
    .fetch_one(&mut *tx)
    .await;

    let workspace = match workspace_res {
        Ok(ws) => ws,
        Err(e) => {
            println!("Failed to insert workspace: {:?}", e);
            let _ = tx.rollback().await;
            return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response();
        }
    };

    let member_res = sqlx::query(
        "INSERT INTO workspace_members (workspace_id, user_id, role) VALUES ($1, $2, $3)"
    )
    .bind(workspace.id)
    .bind(user_id.to_string())
    .bind("owner")
    .execute(&mut *tx)
    .await;

    if let Err(e) = member_res {
        println!("Failed to insert workspace member: {:?}", e);
        let _ = tx.rollback().await;
        return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response();
    }

    if let Err(e) = tx.commit().await {
        println!("Failed to commit transaction: {:?}", e);
        return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Transaction commit error").into_response();
    }

    Json(serde_json::json!({ "workspace": workspace })).into_response()
}

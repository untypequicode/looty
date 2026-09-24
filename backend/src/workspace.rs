use axum::{
    extract::State,
    response::IntoResponse,
    routing::{get, post, put, delete},
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

#[derive(Serialize, sqlx::FromRow)]
pub struct WorkspaceResponse {
    pub id: Uuid,
    pub name: String,
    pub role: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
pub struct UpdateWorkspacePayload {
    pub name: String,
}



#[derive(Serialize, sqlx::FromRow)]
pub struct WorkspaceMemberResponse {
    pub user_id: String,
    pub name: String,
    pub email: String,
    pub role: String,
}

#[derive(Deserialize)]
pub struct UpdateMemberRolePayload {
    pub role: String,
}

pub async fn get_workspace_role(pool: &PgPool, user_id: &str, workspace_id: &Uuid) -> Option<String> {
    sqlx::query_scalar("SELECT role FROM workspace_members WHERE workspace_id = $1 AND user_id = $2")
        .bind(workspace_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None)
}

pub fn workspace_routes(state: AuthState) -> Router {
    Router::new()
        .route("/", get(list_workspaces).post(create_workspace))
        .route("/:workspace_id", put(update_workspace).delete(delete_workspace))
        .route("/:workspace_id/join", post(join_workspace))
        .route("/:workspace_id/members", get(list_members))
        .route("/:workspace_id/members/:member_id", put(update_member_role).delete(remove_member))
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

    let workspaces = sqlx::query_as::<_, WorkspaceResponse>(
        r#"
        SELECT w.id, w.name, wm.role, w.created_at, w.updated_at
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

async fn join_workspace(
    session: Session,
    State(state): State<AuthState>,
    axum::extract::Path(workspace_id): axum::extract::Path<Uuid>,
) -> impl IntoResponse {
    let user_id_opt: Option<Uuid> = session.get("user_id").await.unwrap_or(None);
    
    let user_id = match user_id_opt {
        Some(uid) => uid,
        None => return (axum::http::StatusCode::UNAUTHORIZED, "Unauthorized").into_response(),
    };

    // Check if workspace exists
    let ws_exists: Option<Uuid> = sqlx::query_scalar("SELECT id FROM workspaces WHERE id = $1")
        .bind(workspace_id)
        .fetch_optional(&state.pool)
        .await
        .unwrap_or(None);

    if ws_exists.is_none() {
        return (axum::http::StatusCode::NOT_FOUND, "Workspace not found").into_response();
    }

    // Insert user as member (ignore if already exists)
    let res = sqlx::query(
        "INSERT INTO workspace_members (workspace_id, user_id, role) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING"
    )
    .bind(workspace_id)
    .bind(user_id.to_string())
    .bind("member")
    .execute(&state.pool)
    .await;

    match res {
        Ok(_) => (axum::http::StatusCode::OK, "Joined successfully").into_response(),
        Err(e) => {
            println!("Failed to join workspace: {:?}", e);
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response()
        }
    }
}

async fn update_workspace(
    session: Session,
    State(state): State<AuthState>,
    axum::extract::Path(workspace_id): axum::extract::Path<Uuid>,
    Json(payload): Json<UpdateWorkspacePayload>,
) -> impl IntoResponse {
    let user_id_opt: Option<Uuid> = session.get("user_id").await.unwrap_or(None);
    let user_id = match user_id_opt {
        Some(uid) => uid,
        None => return (axum::http::StatusCode::UNAUTHORIZED, "Unauthorized").into_response(),
    };

    if payload.name.trim().is_empty() {
        return (axum::http::StatusCode::BAD_REQUEST, "Workspace name cannot be empty").into_response();
    }

    if payload.name.trim().is_empty() {
        return (axum::http::StatusCode::BAD_REQUEST, "Workspace name cannot be empty").into_response();
    }

    let role = get_workspace_role(&state.pool, &user_id.to_string(), &workspace_id).await;
    if role != Some("owner".to_string()) && role != Some("admin".to_string()) {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden: Owners or Admins only").into_response();
    }

    let workspace_res = sqlx::query_as::<_, Workspace>(
        "UPDATE workspaces SET name = $1, updated_at = NOW() WHERE id = $2 RETURNING id, name, created_at, updated_at"
    )
    .bind(payload.name).bind(workspace_id)
    .fetch_one(&state.pool).await;

    match workspace_res {
        Ok(ws) => Json(serde_json::json!({ "workspace": ws })).into_response(),
        Err(_) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response()
    }
}

async fn delete_workspace(
    session: Session,
    State(state): State<AuthState>,
    axum::extract::Path(workspace_id): axum::extract::Path<Uuid>,
) -> impl IntoResponse {
    let user_id_opt: Option<Uuid> = session.get("user_id").await.unwrap_or(None);
    let user_id = match user_id_opt {
        Some(uid) => uid,
        None => return (axum::http::StatusCode::UNAUTHORIZED, "Unauthorized").into_response(),
    };

    let role = get_workspace_role(&state.pool, &user_id.to_string(), &workspace_id).await;
    if role != Some("owner".to_string()) && role != Some("admin".to_string()) {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden: Owners or Admins only").into_response();
    }

    let res = sqlx::query("DELETE FROM workspaces WHERE id = $1")
        .bind(workspace_id)
        .execute(&state.pool).await;

    match res {
        Ok(_) => (axum::http::StatusCode::NO_CONTENT).into_response(),
        Err(_) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response()
    }
}

async fn list_members(
    session: Session,
    State(state): State<AuthState>,
    axum::extract::Path(workspace_id): axum::extract::Path<Uuid>,
) -> impl IntoResponse {
    let user_id_opt: Option<Uuid> = session.get("user_id").await.unwrap_or(None);
    let user_id = match user_id_opt {
        Some(uid) => uid,
        None => return (axum::http::StatusCode::UNAUTHORIZED, "Unauthorized").into_response(),
    };

    if get_workspace_role(&state.pool, &user_id.to_string(), &workspace_id).await.is_none() {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden").into_response();
    }

    let members = sqlx::query_as::<_, WorkspaceMemberResponse>(
        r#"SELECT wm.user_id, u.name, u.email, wm.role 
           FROM workspace_members wm 
           JOIN users u ON wm.user_id = u.id::varchar 
           WHERE wm.workspace_id = $1"#
    )
    .bind(workspace_id)
    .fetch_all(&state.pool)
    .await;

    match members {
        Ok(m) => Json(serde_json::json!({ "members": m })).into_response(),
        Err(_) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response()
    }
}

async fn update_member_role(
    session: Session,
    State(state): State<AuthState>,
    axum::extract::Path((workspace_id, member_id)): axum::extract::Path<(Uuid, String)>,
    Json(payload): Json<UpdateMemberRolePayload>,
) -> impl IntoResponse {
    let user_id_opt: Option<Uuid> = session.get("user_id").await.unwrap_or(None);
    let user_id = match user_id_opt {
        Some(uid) => uid,
        None => return (axum::http::StatusCode::UNAUTHORIZED, "Unauthorized").into_response(),
    };

    if get_workspace_role(&state.pool, &user_id.to_string(), &workspace_id).await != Some("owner".to_string()) {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden: Owners only").into_response();
    }
    
    if payload.role != "owner" && payload.role != "admin" && payload.role != "member" && payload.role != "viewer" {
        return (axum::http::StatusCode::BAD_REQUEST, "Invalid role").into_response();
    }

    let res = sqlx::query("UPDATE workspace_members SET role = $1 WHERE workspace_id = $2 AND user_id = $3")
        .bind(&payload.role).bind(workspace_id).bind(&member_id)
        .execute(&state.pool).await;

    match res {
        Ok(_) => (axum::http::StatusCode::OK).into_response(),
        Err(_) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response()
    }
}

async fn remove_member(
    session: Session,
    State(state): State<AuthState>,
    axum::extract::Path((workspace_id, member_id)): axum::extract::Path<(Uuid, String)>,
) -> impl IntoResponse {
    let user_id_opt: Option<Uuid> = session.get("user_id").await.unwrap_or(None);
    let user_id = match user_id_opt {
        Some(uid) => uid,
        None => return (axum::http::StatusCode::UNAUTHORIZED, "Unauthorized").into_response(),
    };

    if get_workspace_role(&state.pool, &user_id.to_string(), &workspace_id).await != Some("owner".to_string()) {
        return (axum::http::StatusCode::FORBIDDEN, "Forbidden: Owners only").into_response();
    }

    let res = sqlx::query("DELETE FROM workspace_members WHERE workspace_id = $1 AND user_id = $2")
        .bind(workspace_id).bind(&member_id)
        .execute(&state.pool).await;

    match res {
        Ok(_) => (axum::http::StatusCode::NO_CONTENT).into_response(),
        Err(_) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response()
    }
}

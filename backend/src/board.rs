use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, put, delete},
    Json, Router, Extension,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tower_sessions::Session;
use uuid::Uuid;
use crate::auth::AuthState;
use chrono::{DateTime, Utc};
use std::sync::Arc;

#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct Board {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct BoardColumn {
    pub id: Uuid,
    pub board_id: Uuid,
    pub name: String,
    pub position: f64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct Task {
    pub id: Uuid,
    pub column_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub position: f64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Full board representation
#[derive(Serialize)]
pub struct BoardFull {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
    pub columns: Vec<ColumnWithTasks>,
}

#[derive(Serialize)]
pub struct ColumnWithTasks {
    #[serde(flatten)]
    pub column: BoardColumn,
    pub tasks: Vec<Task>,
}

#[derive(Deserialize)]
pub struct CreateBoardReq {
    pub name: String,
}

#[derive(Deserialize)]
pub struct UpdateBoardReq {
    pub name: String,
}

#[derive(Deserialize)]
pub struct CreateColumnReq {
    pub name: String,
}

#[derive(Deserialize)]
pub struct UpdateColumnReq {
    pub name: String,
}

#[derive(Deserialize)]
pub struct CreateTaskReq {
    pub title: String,
    pub description: Option<String>,
}

#[derive(Deserialize)]
pub struct MoveTaskReq {
    pub column_id: Uuid,
    pub position: f64,
}

#[derive(Deserialize)]
pub struct UpdateTaskReq {
    pub title: String,
    pub description: Option<String>,
}

// Routes
pub fn workspace_board_routes(state: AuthState) -> Router {
    Router::new()
        .route("/", get(list_boards).post(create_board))
        .with_state(state)
}

pub fn board_routes(state: AuthState) -> Router {
    Router::new()
        .route("/:board_id", get(get_board).put(update_board).delete(delete_board))
        .route("/:board_id/columns", post(create_column))
        .route("/columns/:column_id", put(update_column).delete(delete_column))
        .route("/columns/:column_id/tasks", post(create_task))
        .route("/tasks/:task_id", put(update_task).delete(delete_task))
        .route("/tasks/:task_id/move", put(move_task))
        .with_state(state)
}

// Handlers
async fn check_workspace_access(pool: &PgPool, user_id: &str, workspace_id: &Uuid) -> Option<String> {
    sqlx::query_scalar("SELECT role FROM workspace_members WHERE user_id = $1 AND workspace_id = $2")
    .bind(user_id)
    .bind(workspace_id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None)
}

async fn get_user_id(session: &Session) -> Option<String> {
    let user_id_opt: Option<Uuid> = session.get("user_id").await.unwrap_or(None);
    user_id_opt.map(|u| u.to_string())
}

async fn list_boards(
    State(state): State<AuthState>,
    Path(workspace_id): Path<Uuid>,
    session: Session,
) -> Result<Json<Vec<Board>>, StatusCode> {
    let user_id = get_user_id(&session).await.ok_or(StatusCode::UNAUTHORIZED)?;
    
    if check_workspace_access(&state.pool, &user_id, &workspace_id).await.is_none() {
        return Err(StatusCode::FORBIDDEN);
    }

    let boards = sqlx::query_as::<_, Board>(
        "SELECT id, workspace_id, name, created_at, updated_at FROM boards WHERE workspace_id = $1 ORDER BY created_at ASC"
    )
    .bind(workspace_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(boards))
}

async fn create_board(
    State(state): State<AuthState>,
    Path(workspace_id): Path<Uuid>,
    Extension(ws_state): Extension<Arc<crate::ws::AppState>>,
    session: Session,
    Json(req): Json<CreateBoardReq>,
) -> Result<Json<Board>, StatusCode> {
    let user_id = get_user_id(&session).await.ok_or(StatusCode::UNAUTHORIZED)?;
    
    let role = check_workspace_access(&state.pool, &user_id, &workspace_id).await;
    if role != Some("owner".to_string()) && role != Some("admin".to_string()) {
        return Err(StatusCode::FORBIDDEN);
    }

    let board = sqlx::query_as::<_, Board>(
        "INSERT INTO boards (workspace_id, name) VALUES ($1, $2) RETURNING id, workspace_id, name, created_at, updated_at"
    )
    .bind(workspace_id)
    .bind(req.name)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let ws_msg = serde_json::json!({
        "type": "BOARD_CREATED",
        "board": &board
    }).to_string();
    let tx = ws_state.get_or_create_room(&workspace_id.to_string()).await;
    let _ = tx.send(ws_msg);

    Ok(Json(board))
}

async fn get_board(
    State(state): State<AuthState>,
    Path(board_id): Path<Uuid>,
    session: Session,
) -> Result<Json<BoardFull>, StatusCode> {
    let user_id = get_user_id(&session).await.ok_or(StatusCode::UNAUTHORIZED)?;
    
    let board = sqlx::query_as::<_, Board>(
        "SELECT id, workspace_id, name, created_at, updated_at FROM boards WHERE id = $1"
    )
    .bind(board_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    if check_workspace_access(&state.pool, &user_id, &board.workspace_id).await.is_none() {
        return Err(StatusCode::FORBIDDEN);
    }

    let columns = sqlx::query_as::<_, BoardColumn>(
        "SELECT id, board_id, name, position, created_at, updated_at FROM board_columns WHERE board_id = $1 ORDER BY position ASC"
    )
    .bind(board_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let mut full_columns = Vec::new();
    for col in columns {
        let tasks = sqlx::query_as::<_, Task>(
            "SELECT id, column_id, title, description, position, created_at, updated_at FROM tasks WHERE column_id = $1 ORDER BY position ASC"
        )
        .bind(col.id)
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default();
        
        full_columns.push(ColumnWithTasks { column: col, tasks });
    }

    Ok(Json(BoardFull {
        id: board.id,
        workspace_id: board.workspace_id,
        name: board.name,
        columns: full_columns,
    }))
}

async fn create_column(
    State(state): State<AuthState>,
    Path(board_id): Path<Uuid>,
    Extension(ws_state): Extension<Arc<crate::ws::AppState>>,
    session: Session,
    Json(req): Json<CreateColumnReq>,
) -> Result<Json<BoardColumn>, StatusCode> {
    let user_id = get_user_id(&session).await.ok_or(StatusCode::UNAUTHORIZED)?;
    
    #[derive(sqlx::FromRow)]
    struct WorkspaceId { workspace_id: Uuid }
    
    let board = sqlx::query_as::<_, WorkspaceId>("SELECT workspace_id FROM boards WHERE id = $1")
        .bind(board_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
        
    let role = check_workspace_access(&state.pool, &user_id, &board.workspace_id).await;
    if role != Some("owner".to_string()) && role != Some("admin".to_string()) { return Err(StatusCode::FORBIDDEN); }

    let max_pos: Option<f64> = sqlx::query_scalar("SELECT MAX(position) FROM board_columns WHERE board_id = $1")
        .bind(board_id)
        .fetch_one(&state.pool).await.unwrap_or(Some(0.0));
    let next_pos = max_pos.unwrap_or(0.0) + 65536.0;

    let col = sqlx::query_as::<_, BoardColumn>(
        "INSERT INTO board_columns (board_id, name, position) VALUES ($1, $2, $3) RETURNING id, board_id, name, position, created_at, updated_at"
    )
    .bind(board_id).bind(req.name).bind(next_pos)
    .fetch_one(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let ws_msg = serde_json::json!({
        "type": "COLUMN_CREATED",
        "column": &col
    }).to_string();
    let tx = ws_state.get_or_create_room(&board.workspace_id.to_string()).await;
    let _ = tx.send(ws_msg);

    Ok(Json(col))
}

async fn create_task(
    State(state): State<AuthState>,
    Path(column_id): Path<Uuid>,
    Extension(ws_state): Extension<Arc<crate::ws::AppState>>,
    session: Session,
    Json(req): Json<CreateTaskReq>,
) -> Result<Json<Task>, StatusCode> {
    let user_id = get_user_id(&session).await.ok_or(StatusCode::UNAUTHORIZED)?;
    
    #[derive(sqlx::FromRow)]
    struct WorkspaceId { workspace_id: Uuid }
    
    let board = sqlx::query_as::<_, WorkspaceId>(
        "SELECT b.workspace_id FROM boards b JOIN board_columns c ON b.id = c.board_id WHERE c.id = $1"
    )
    .bind(column_id)
    .fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    
    if check_workspace_access(&state.pool, &user_id, &board.workspace_id).await.is_none() { return Err(StatusCode::FORBIDDEN); }

    let max_pos: Option<f64> = sqlx::query_scalar("SELECT MAX(position) FROM tasks WHERE column_id = $1")
        .bind(column_id)
        .fetch_one(&state.pool).await.unwrap_or(Some(0.0));
    let next_pos = max_pos.unwrap_or(0.0) + 65536.0;

    let task = sqlx::query_as::<_, Task>(
        "INSERT INTO tasks (column_id, title, description, position) VALUES ($1, $2, $3, $4) RETURNING id, column_id, title, description, position, created_at, updated_at"
    )
    .bind(column_id).bind(req.title).bind(req.description).bind(next_pos)
    .fetch_one(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let ws_msg = serde_json::json!({
        "type": "TASK_CREATED",
        "task": &task
    }).to_string();
    let tx = ws_state.get_or_create_room(&board.workspace_id.to_string()).await;
    let _ = tx.send(ws_msg);

    Ok(Json(task))
}

async fn move_task(
    State(state): State<AuthState>,
    Path(task_id): Path<Uuid>,
    Extension(ws_state): Extension<Arc<crate::ws::AppState>>,
    session: Session,
    Json(req): Json<MoveTaskReq>,
) -> Result<Json<Task>, StatusCode> {
    let user_id = get_user_id(&session).await.ok_or(StatusCode::UNAUTHORIZED)?;
    
    #[derive(sqlx::FromRow)]
    struct WorkspaceId { workspace_id: Uuid }
    
    let board = sqlx::query_as::<_, WorkspaceId>(
        "SELECT b.workspace_id FROM boards b JOIN board_columns c ON b.id = c.board_id JOIN tasks t ON c.id = t.column_id WHERE t.id = $1"
    )
    .bind(task_id)
    .fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    
    if check_workspace_access(&state.pool, &user_id, &board.workspace_id).await.is_none() { return Err(StatusCode::FORBIDDEN); }

    let new_board_check = sqlx::query_as::<_, WorkspaceId>(
        "SELECT b.workspace_id FROM boards b JOIN board_columns c ON b.id = c.board_id WHERE c.id = $1"
    )
    .bind(req.column_id)
    .fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    
    if new_board_check.workspace_id != board.workspace_id {
        return Err(StatusCode::BAD_REQUEST);
    }

    let task = sqlx::query_as::<_, Task>(
        "UPDATE tasks SET column_id = $1, position = $2, updated_at = NOW() WHERE id = $3 RETURNING id, column_id, title, description, position, created_at, updated_at"
    )
    .bind(req.column_id).bind(req.position).bind(task_id)
    .fetch_one(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let ws_msg = serde_json::json!({
        "type": "TASK_MOVED",
        "task": &task
    }).to_string();
    let tx = ws_state.get_or_create_room(&board.workspace_id.to_string()).await;
    let _ = tx.send(ws_msg);

    Ok(Json(task))
}

async fn update_task(
    State(state): State<AuthState>,
    Path(task_id): Path<Uuid>,
    Extension(ws_state): Extension<Arc<crate::ws::AppState>>,
    session: Session,
    Json(req): Json<UpdateTaskReq>,
) -> Result<Json<Task>, StatusCode> {
    let user_id = get_user_id(&session).await.ok_or(StatusCode::UNAUTHORIZED)?;
    
    #[derive(sqlx::FromRow)]
    struct WorkspaceId { workspace_id: Uuid }
    
    let board = sqlx::query_as::<_, WorkspaceId>(
        "SELECT b.workspace_id FROM boards b JOIN board_columns c ON b.id = c.board_id JOIN tasks t ON c.id = t.column_id WHERE t.id = $1"
    )
    .bind(task_id)
    .fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    
    if check_workspace_access(&state.pool, &user_id, &board.workspace_id).await.is_none() { return Err(StatusCode::FORBIDDEN); }

    let task = sqlx::query_as::<_, Task>(
        "UPDATE tasks SET title = $1, description = $2, updated_at = NOW() WHERE id = $3 RETURNING id, column_id, title, description, position, created_at, updated_at"
    )
    .bind(req.title).bind(req.description).bind(task_id)
    .fetch_one(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let ws_msg = serde_json::json!({
        "type": "TASK_UPDATED",
        "task": &task
    }).to_string();
    let tx = ws_state.get_or_create_room(&board.workspace_id.to_string()).await;
    let _ = tx.send(ws_msg);

    Ok(Json(task))
}

async fn delete_task(
    State(state): State<AuthState>,
    Path(task_id): Path<Uuid>,
    Extension(ws_state): Extension<Arc<crate::ws::AppState>>,
    session: Session,
) -> Result<StatusCode, StatusCode> {
    let user_id = get_user_id(&session).await.ok_or(StatusCode::UNAUTHORIZED)?;
    
    #[derive(sqlx::FromRow)]
    struct WorkspaceId { workspace_id: Uuid, column_id: Uuid }
    
    let board = sqlx::query_as::<_, WorkspaceId>(
        "SELECT b.workspace_id, t.column_id FROM boards b JOIN board_columns c ON b.id = c.board_id JOIN tasks t ON c.id = t.column_id WHERE t.id = $1"
    )
    .bind(task_id)
    .fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    
    if check_workspace_access(&state.pool, &user_id, &board.workspace_id).await.is_none() { return Err(StatusCode::FORBIDDEN); }

    sqlx::query("DELETE FROM tasks WHERE id = $1")
        .bind(task_id)
        .execute(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let ws_msg = serde_json::json!({
        "type": "TASK_DELETED",
        "task_id": task_id,
        "column_id": board.column_id
    }).to_string();
    let tx = ws_state.get_or_create_room(&board.workspace_id.to_string()).await;
    let _ = tx.send(ws_msg);

    Ok(StatusCode::NO_CONTENT)
}

async fn update_board(
    State(state): State<AuthState>,
    Path(board_id): Path<Uuid>,
    Extension(ws_state): Extension<Arc<crate::ws::AppState>>,
    session: Session,
    Json(req): Json<UpdateBoardReq>,
) -> Result<Json<Board>, StatusCode> {
    let user_id = get_user_id(&session).await.ok_or(StatusCode::UNAUTHORIZED)?;
    
    let board_opt = sqlx::query_as::<_, Board>("SELECT * FROM boards WHERE id = $1")
        .bind(board_id).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let board = board_opt.ok_or(StatusCode::NOT_FOUND)?;

    let role = check_workspace_access(&state.pool, &user_id, &board.workspace_id).await;
    if role != Some("owner".to_string()) && role != Some("admin".to_string()) { return Err(StatusCode::FORBIDDEN); }
    if req.name.trim().is_empty() { return Err(StatusCode::BAD_REQUEST); }

    let updated_board = sqlx::query_as::<_, Board>(
        "UPDATE boards SET name = $1, updated_at = NOW() WHERE id = $2 RETURNING id, workspace_id, name, created_at, updated_at"
    )
    .bind(req.name).bind(board_id)
    .fetch_one(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let ws_msg = serde_json::json!({ "type": "BOARD_UPDATED", "board": &updated_board }).to_string();
    let tx = ws_state.get_or_create_room(&updated_board.workspace_id.to_string()).await;
    let _ = tx.send(ws_msg);

    Ok(Json(updated_board))
}

async fn delete_board(
    State(state): State<AuthState>,
    Path(board_id): Path<Uuid>,
    Extension(ws_state): Extension<Arc<crate::ws::AppState>>,
    session: Session,
) -> Result<StatusCode, StatusCode> {
    let user_id = get_user_id(&session).await.ok_or(StatusCode::UNAUTHORIZED)?;
    
    #[derive(sqlx::FromRow)]
    struct WorkspaceId { workspace_id: Uuid }
    
    let board = sqlx::query_as::<_, WorkspaceId>("SELECT workspace_id FROM boards WHERE id = $1")
        .bind(board_id).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
        
    let role = check_workspace_access(&state.pool, &user_id, &board.workspace_id).await;
    if role != Some("owner".to_string()) && role != Some("admin".to_string()) { return Err(StatusCode::FORBIDDEN); }

    sqlx::query("DELETE FROM boards WHERE id = $1").bind(board_id).execute(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let ws_msg = serde_json::json!({ "type": "BOARD_DELETED", "board_id": board_id }).to_string();
    let tx = ws_state.get_or_create_room(&board.workspace_id.to_string()).await;
    let _ = tx.send(ws_msg);

    Ok(StatusCode::NO_CONTENT)
}

async fn update_column(
    State(state): State<AuthState>,
    Path(column_id): Path<Uuid>,
    Extension(ws_state): Extension<Arc<crate::ws::AppState>>,
    session: Session,
    Json(req): Json<UpdateColumnReq>,
) -> Result<Json<BoardColumn>, StatusCode> {
    let user_id = get_user_id(&session).await.ok_or(StatusCode::UNAUTHORIZED)?;
    
    #[derive(sqlx::FromRow)]
    struct WorkspaceId { workspace_id: Uuid }
    
    let board = sqlx::query_as::<_, WorkspaceId>(
        "SELECT b.workspace_id FROM boards b JOIN board_columns c ON b.id = c.board_id WHERE c.id = $1"
    )
    .bind(column_id).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    
    let role = check_workspace_access(&state.pool, &user_id, &board.workspace_id).await;
    if role != Some("owner".to_string()) && role != Some("admin".to_string()) { return Err(StatusCode::FORBIDDEN); }
    if req.name.trim().is_empty() { return Err(StatusCode::BAD_REQUEST); }

    let col = sqlx::query_as::<_, BoardColumn>(
        "UPDATE board_columns SET name = $1, updated_at = NOW() WHERE id = $2 RETURNING id, board_id, name, position, created_at, updated_at"
    )
    .bind(req.name).bind(column_id)
    .fetch_one(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let ws_msg = serde_json::json!({ "type": "COLUMN_UPDATED", "column": &col }).to_string();
    let tx = ws_state.get_or_create_room(&board.workspace_id.to_string()).await;
    let _ = tx.send(ws_msg);

    Ok(Json(col))
}

async fn delete_column(
    State(state): State<AuthState>,
    Path(column_id): Path<Uuid>,
    Extension(ws_state): Extension<Arc<crate::ws::AppState>>,
    session: Session,
) -> Result<StatusCode, StatusCode> {
    let user_id = get_user_id(&session).await.ok_or(StatusCode::UNAUTHORIZED)?;
    
    #[derive(sqlx::FromRow)]
    struct WorkspaceId { workspace_id: Uuid, board_id: Uuid }
    
    let board = sqlx::query_as::<_, WorkspaceId>(
        "SELECT b.workspace_id, c.board_id FROM boards b JOIN board_columns c ON b.id = c.board_id WHERE c.id = $1"
    )
    .bind(column_id).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    
    let role = check_workspace_access(&state.pool, &user_id, &board.workspace_id).await;
    if role != Some("owner".to_string()) && role != Some("admin".to_string()) { return Err(StatusCode::FORBIDDEN); }

    sqlx::query("DELETE FROM board_columns WHERE id = $1").bind(column_id).execute(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let ws_msg = serde_json::json!({ "type": "COLUMN_DELETED", "column_id": column_id, "board_id": board.board_id }).to_string();
    let tx = ws_state.get_or_create_room(&board.workspace_id.to_string()).await;
    let _ = tx.send(ws_msg);

    Ok(StatusCode::NO_CONTENT)
}

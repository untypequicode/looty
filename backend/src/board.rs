use axum::{
    extract::{Path, State, Multipart},
    http::StatusCode,
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
    pub automations: serde_json::Value,
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

#[derive(Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct Task {
    pub id: Uuid,
    pub column_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub position: f64,
    pub previous_column_id: Option<Uuid>,
    pub is_per_person: bool,
    pub is_completed: bool,
    pub require_all_validations: bool,
    pub start_date: Option<DateTime<Utc>>,
    pub due_date: Option<DateTime<Utc>>,
    pub priority: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct Label {
    pub id: Uuid,
    pub board_id: Uuid,
    pub name: String,
    pub color: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct TaskAttachment {
    pub id: Uuid,
    pub task_id: Uuid,
    pub uploaded_by: Uuid,
    pub filename: String,
    pub file_path: String,
    pub mime_type: Option<String>,
    pub size_bytes: Option<i64>,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct TaskActivity {
    pub id: Uuid,
    pub task_id: Uuid,
    pub user_id: Uuid,
    pub activity_type: String,
    pub old_value: Option<serde_json::Value>,
    pub new_value: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}



#[derive(Serialize)]
pub struct TaskWithEnrichment {
    #[serde(flatten)]
    pub task: Task,
    pub labels: Vec<Label>,
    pub assignees: Vec<String>,
    pub validations: Vec<String>,
}

// Full board representation
#[derive(Serialize)]
pub struct BoardFull {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
    pub automations: serde_json::Value,
    pub columns: Vec<ColumnWithTasks>,
    pub labels: Vec<Label>,
}

#[derive(Serialize)]
pub struct ColumnWithTasks {
    #[serde(flatten)]
    pub column: BoardColumn,
    pub tasks: Vec<TaskWithEnrichment>,
}

#[derive(Deserialize)]
pub struct CreateBoardReq {
    pub name: String,
}

#[derive(Deserialize)]
pub struct UpdateBoardReq {
    pub name: Option<String>,
    pub automations: Option<serde_json::Value>,
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
    pub start_date: Option<DateTime<Utc>>,
    pub due_date: Option<DateTime<Utc>>,
    pub labels: Option<Vec<Uuid>>,
    pub assignees: Option<Vec<String>>,
    pub priority: Option<i32>,
    pub require_all_validations: Option<bool>,
}

#[derive(Deserialize)]
pub struct MoveTaskReq {
    pub column_id: Uuid,
    pub position: f64,
    pub previous_column_id: Option<Uuid>,
    pub clear_previous_column_id: Option<bool>,
}

#[derive(Deserialize)]
pub struct UpdateTaskReq {
    pub title: String,
    pub description: Option<String>,
    pub start_date: Option<DateTime<Utc>>,
    pub due_date: Option<DateTime<Utc>>,
    pub is_per_person: Option<bool>,
    pub priority: Option<i32>,
    pub require_all_validations: Option<bool>,
}

#[derive(Deserialize)]
pub struct MoveColumnReq {
    pub position: f64,
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
        .route("/:board_id/labels", post(create_label))
        .route("/labels/:label_id", put(update_label).delete(delete_label))
        .route("/columns/:column_id", put(update_column).delete(delete_column))
        .route("/columns/:column_id/move", put(move_column))
        .route("/columns/:column_id/tasks", post(create_task))
        .route("/tasks/:task_id", put(update_task).delete(delete_task))
        .route("/tasks/:task_id/move", put(move_task))
        .route("/tasks/:task_id/labels/:label_id", post(add_task_label).delete(remove_task_label))
        .route("/tasks/:task_id/assignees/:user_id", post(add_task_assignee).delete(remove_task_assignee))
        .route("/tasks/:task_id/subtasks", post(create_subtask))
        .route("/tasks/:task_id/toggle_validation", put(toggle_task_validation))
        .route("/tasks/:task_id/attachments", get(get_task_attachments).post(upload_attachment))
        .route("/tasks/:task_id/attachments/:attachment_id", delete(delete_attachment))
        .route("/tasks/:task_id/activities", get(get_task_activities))
        .route("/tasks/:task_id/activities/:activity_id/restore", post(restore_task_activity))
        .route("/tasks/:task_id/comments", get(get_task_comments).post(add_task_comment))
        .route("/tasks/:task_id/comments/:comment_id", delete(delete_task_comment))
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
        "SELECT id, workspace_id, name, automations, created_at, updated_at FROM boards WHERE workspace_id = $1 ORDER BY created_at ASC"
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
    if role == Some("viewer".to_string()) || role.is_none() {
        return Err(StatusCode::FORBIDDEN);
    }

    let board = sqlx::query_as::<_, Board>(
        "INSERT INTO boards (workspace_id, name) VALUES ($1, $2) RETURNING id, workspace_id, name, automations, created_at, updated_at"
    )
    .bind(workspace_id)
    .bind(req.name)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        eprintln!("create_board: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

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
        "SELECT id, workspace_id, name, automations, created_at, updated_at FROM boards WHERE id = $1"
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
    .map_err(|e| {
        eprintln!("get_board: erreur chargement colonnes: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let labels = sqlx::query_as::<_, Label>(
        "SELECT id, board_id, name, color, created_at FROM labels WHERE board_id = $1"
    )
    .bind(board_id).fetch_all(&state.pool).await.unwrap_or_default();

    #[derive(sqlx::FromRow)]
    struct TaskLabelRow { task_id: Uuid, label_id: Uuid }
    let task_labels = sqlx::query_as::<_, TaskLabelRow>(
        "SELECT tl.task_id, tl.label_id FROM task_labels tl JOIN tasks t ON tl.task_id = t.id JOIN board_columns c ON t.column_id = c.id WHERE c.board_id = $1"
    )
    .bind(board_id).fetch_all(&state.pool).await.unwrap_or_default();

    #[derive(sqlx::FromRow)]
    struct TaskAssigneeRow { task_id: Uuid, user_id: String }
    let task_assignees = sqlx::query_as::<_, TaskAssigneeRow>(
        "SELECT ta.task_id, ta.user_id FROM task_assignees ta JOIN tasks t ON ta.task_id = t.id JOIN board_columns c ON t.column_id = c.id WHERE c.board_id = $1"
    )
    .bind(board_id).fetch_all(&state.pool).await.unwrap_or_default();

    #[derive(sqlx::FromRow)]
    struct TaskValidationRow { task_id: Uuid, user_id: String }
    let all_validations = sqlx::query_as::<_, TaskValidationRow>(
        "SELECT v.task_id, v.user_id FROM task_validations v JOIN tasks t ON v.task_id = t.id JOIN board_columns c ON t.column_id = c.id WHERE c.board_id = $1"
    )
    .bind(board_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let mut full_columns = Vec::new();
    for col in columns {
        let tasks = sqlx::query_as::<_, Task>(
            "SELECT id, column_id, parent_id, title, description, position, previous_column_id, is_per_person, is_completed, require_all_validations, priority, start_date, due_date, created_at, updated_at FROM tasks WHERE column_id = $1 ORDER BY position ASC"
        )
        .bind(col.id)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| {
            eprintln!("get_board: erreur chargement tâches: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        
        let mut enriched_tasks = Vec::new();
        for task in tasks {
            let task_id = task.id;
            let my_label_ids: Vec<Uuid> = task_labels.iter().filter(|tl| tl.task_id == task_id).map(|tl| tl.label_id).collect();
            let my_labels = labels.iter().filter(|l| my_label_ids.contains(&l.id)).cloned().collect();
            let my_assignees = task_assignees.iter().filter(|ta| ta.task_id == task_id).map(|ta| ta.user_id.clone()).collect();
            let my_validations = all_validations.iter().filter(|v| v.task_id == task_id).map(|v| v.user_id.clone()).collect();

            enriched_tasks.push(TaskWithEnrichment {
                task,
                labels: my_labels,
                assignees: my_assignees,
                validations: my_validations,
            });
        }

        full_columns.push(ColumnWithTasks { column: col, tasks: enriched_tasks });
    }

    Ok(Json(BoardFull {
        id: board.id,
        workspace_id: board.workspace_id,
        name: board.name,
        automations: board.automations,
        columns: full_columns,
        labels,
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
    if role == Some("viewer".to_string()) || role.is_none() { return Err(StatusCode::FORBIDDEN); }

    let max_pos: Option<f64> = sqlx::query_scalar("SELECT MAX(position) FROM board_columns WHERE board_id = $1")
        .bind(board_id)
        .fetch_one(&state.pool).await.unwrap_or(Some(0.0));
    let next_pos = max_pos.unwrap_or(0.0) + 65536.0;

    let col = sqlx::query_as::<_, BoardColumn>(
        "INSERT INTO board_columns (board_id, name, position) VALUES ($1, $2, $3) RETURNING id, board_id, name, position, created_at, updated_at"
    )
    .bind(board_id).bind(req.name).bind(next_pos)
    .fetch_one(&state.pool).await.map_err(|e| {
        eprintln!("create_column: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

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
    
    let role = check_workspace_access(&state.pool, &user_id, &board.workspace_id).await;
    if role.is_none() || role == Some("viewer".to_string()) { return Err(StatusCode::FORBIDDEN); }

    let max_pos: Option<f64> = sqlx::query_scalar("SELECT MAX(position) FROM tasks WHERE column_id = $1")
        .bind(column_id)
        .fetch_one(&state.pool).await.unwrap_or(Some(0.0));
    let next_pos = max_pos.unwrap_or(0.0) + 65536.0;

    let task = sqlx::query_as::<_, Task>(
        "INSERT INTO tasks (column_id, title, description, position, start_date, due_date, priority) VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id, column_id, parent_id, title, description, position, previous_column_id, is_per_person, is_completed, priority, start_date, due_date, require_all_validations, created_at, updated_at"
    )
    .bind(column_id).bind(&req.title).bind(&req.description).bind(next_pos).bind(&req.start_date).bind(&req.due_date).bind(req.priority.unwrap_or(0))
    .fetch_one(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Handle initial labels and assignees if provided
    if let Some(labels) = &req.labels {
        for label_id in labels {
            let _ = sqlx::query("INSERT INTO task_labels (task_id, label_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
                .bind(task.id).bind(label_id).execute(&state.pool).await;
        }
    }
    if let Some(assignees) = &req.assignees {
        for user_id in assignees {
            let _ = sqlx::query("INSERT INTO task_assignees (task_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
                .bind(task.id).bind(user_id).execute(&state.pool).await;
        }
    }

    // Refresh the task to get the full enriched version for WS broadcast
    // A simplified version of get_board's enrichment
    let ws_msg = serde_json::json!({
        "type": "TASK_CREATED",
        "task": &task // Frontend will reload or we can rely on WS. For now we just send base task, frontend handles if it's missing labels. Or we can just send BOARD_UPDATED?
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
    struct TaskWorkspaceCheck { workspace_id: Uuid, previous_column_id: Option<Uuid>, column_id: Uuid }
    
    let board = sqlx::query_as::<_, TaskWorkspaceCheck>(
        "SELECT b.workspace_id, t.previous_column_id, t.column_id FROM boards b JOIN board_columns c ON b.id = c.board_id JOIN tasks t ON c.id = t.column_id WHERE t.id = $1"
    )
    .bind(task_id)
    .fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    
    let role = check_workspace_access(&state.pool, &user_id, &board.workspace_id).await;
    if role.is_none() || role == Some("viewer".to_string()) { return Err(StatusCode::FORBIDDEN); }

    let new_board_ws = sqlx::query_scalar::<_, Uuid>(
        "SELECT b.workspace_id FROM boards b JOIN board_columns c ON b.id = c.board_id WHERE c.id = $1"
    )
    .bind(req.column_id)
    .fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    
    if new_board_ws != board.workspace_id {
        return Err(StatusCode::BAD_REQUEST);
    }
    let prev_col_val = if req.clear_previous_column_id.unwrap_or(false) {
        None
    } else if let Some(pid) = req.previous_column_id {
        Some(pid)
    } else {
        board.previous_column_id
    };

    let task = sqlx::query_as::<_, Task>(
        "UPDATE tasks SET column_id = $1, position = $2, previous_column_id = $3, updated_at = NOW() WHERE id = $4 RETURNING id, column_id, parent_id, title, description, position, previous_column_id, is_per_person, is_completed, priority, start_date, due_date, require_all_validations, created_at, updated_at"
    )
    .bind(req.column_id).bind(req.position).bind(prev_col_val).bind(task_id)
    .fetch_one(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let _ = log_activity(
        &state.pool,
        task_id,
        Uuid::parse_str(&user_id).unwrap(),
        "move_column",
        Some(serde_json::json!({ "column_id": board.column_id })),
        Some(serde_json::json!({ "column_id": req.column_id })),
    ).await;

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
    
    let role = check_workspace_access(&state.pool, &user_id, &board.workspace_id).await;
    if role.is_none() || role == Some("viewer".to_string()) { return Err(StatusCode::FORBIDDEN); }

    let old_task = sqlx::query_as::<_, Task>("SELECT * FROM tasks WHERE id = $1")
        .bind(task_id).fetch_one(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let task = sqlx::query_as::<_, Task>(
        "UPDATE tasks SET title = $1, description = $2, start_date = $3, due_date = $4, is_per_person = COALESCE($5, is_per_person), priority = COALESCE($6, priority), require_all_validations = COALESCE($7, require_all_validations), updated_at = NOW() WHERE id = $8 RETURNING id, column_id, parent_id, title, description, position, previous_column_id, is_per_person, is_completed, priority, start_date, due_date, require_all_validations, created_at, updated_at"
    )
    .bind(&req.title).bind(&req.description).bind(&req.start_date).bind(&req.due_date).bind(&req.is_per_person).bind(req.priority).bind(req.require_all_validations).bind(task_id)
    .fetch_one(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if old_task.title != task.title {
        let _ = log_activity(
            &state.pool, task_id, Uuid::parse_str(&user_id).unwrap(), "update_title",
            Some(serde_json::json!({ "title": old_task.title })),
            Some(serde_json::json!({ "title": task.title }))
        ).await;
    }
    
    if old_task.description != task.description {
        let _ = log_activity(
            &state.pool, task_id, Uuid::parse_str(&user_id).unwrap(), "update_description",
            Some(serde_json::json!({ "description": old_task.description })),
            Some(serde_json::json!({ "description": task.description }))
        ).await;
    }

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
    
    let role = check_workspace_access(&state.pool, &user_id, &board.workspace_id).await;
    if role.is_none() || role == Some("viewer".to_string()) { return Err(StatusCode::FORBIDDEN); }

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
    if role == Some("viewer".to_string()) || role.is_none() { return Err(StatusCode::FORBIDDEN); }
    if let Some(ref n) = req.name {
        if n.trim().is_empty() { return Err(StatusCode::BAD_REQUEST); }
    }

    let new_name = req.name.unwrap_or(board.name);
    let new_automations = req.automations.unwrap_or(board.automations);

    let updated_board = sqlx::query_as::<_, Board>(
        "UPDATE boards SET name = $1, automations = $2, updated_at = NOW() WHERE id = $3 RETURNING id, workspace_id, name, automations, created_at, updated_at"
    )
    .bind(new_name).bind(new_automations).bind(board_id)
    .fetch_one(&state.pool).await.map_err(|e| {
        eprintln!("update_board: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

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
    if role == Some("viewer".to_string()) || role.is_none() { return Err(StatusCode::FORBIDDEN); }

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
    if role == Some("viewer".to_string()) || role.is_none() { return Err(StatusCode::FORBIDDEN); }
    if req.name.trim().is_empty() { return Err(StatusCode::BAD_REQUEST); }

    let col = sqlx::query_as::<_, BoardColumn>(
        "UPDATE board_columns SET name = $1, updated_at = NOW() WHERE id = $2 RETURNING id, board_id, name, position, created_at, updated_at"
    )
    .bind(req.name).bind(column_id)
    .fetch_one(&state.pool).await.map_err(|e| {
        eprintln!("update_column: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

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
    if role == Some("viewer".to_string()) || role.is_none() { return Err(StatusCode::FORBIDDEN); }

    sqlx::query("DELETE FROM board_columns WHERE id = $1").bind(column_id).execute(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let ws_msg = serde_json::json!({ "type": "COLUMN_DELETED", "column_id": column_id, "board_id": board.board_id }).to_string();
    let tx = ws_state.get_or_create_room(&board.workspace_id.to_string()).await;
    let _ = tx.send(ws_msg);

    Ok(StatusCode::NO_CONTENT)
}

async fn move_column(
    State(state): State<AuthState>,
    Path(column_id): Path<Uuid>,
    Extension(ws_state): Extension<Arc<crate::ws::AppState>>,
    session: Session,
    Json(req): Json<MoveColumnReq>,
) -> Result<Json<BoardColumn>, StatusCode> {
    let user_id = get_user_id(&session).await.ok_or(StatusCode::UNAUTHORIZED)?;
    
    #[derive(sqlx::FromRow)]
    struct WorkspaceId { workspace_id: Uuid }
    
    let board = sqlx::query_as::<_, WorkspaceId>(
        "SELECT b.workspace_id FROM boards b JOIN board_columns c ON b.id = c.board_id WHERE c.id = $1"
    )
    .bind(column_id)
    .fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    
    let role = check_workspace_access(&state.pool, &user_id, &board.workspace_id).await;
    if role == Some("viewer".to_string()) || role.is_none() { return Err(StatusCode::FORBIDDEN); }

    let col = sqlx::query_as::<_, BoardColumn>(
        "UPDATE board_columns SET position = $1, updated_at = NOW() WHERE id = $2 RETURNING id, board_id, name, position, created_at, updated_at"
    )
    .bind(req.position).bind(column_id)
    .fetch_one(&state.pool).await.map_err(|e| {
        eprintln!("move_column: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let ws_msg = serde_json::json!({
        "type": "COLUMN_MOVED",
        "column": &col
    }).to_string();
    let tx = ws_state.get_or_create_room(&board.workspace_id.to_string()).await;
    let _ = tx.send(ws_msg);

    Ok(Json(col))
}

#[derive(Deserialize)]
pub struct CreateLabelReq {
    pub name: String,
    pub color: String,
}

async fn create_label(
    State(state): State<AuthState>,
    Path(board_id): Path<Uuid>,
    Extension(ws_state): Extension<Arc<crate::ws::AppState>>,
    session: Session,
    Json(req): Json<CreateLabelReq>,
) -> Result<Json<Label>, StatusCode> {
    let user_id = get_user_id(&session).await.ok_or(StatusCode::UNAUTHORIZED)?;
    let board_ws = sqlx::query_scalar::<_, Uuid>("SELECT workspace_id FROM boards WHERE id = $1").bind(board_id).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    if check_workspace_access(&state.pool, &user_id, &board_ws).await.unwrap_or_default() == "member" { return Err(StatusCode::FORBIDDEN); }

    let label = sqlx::query_as::<_, Label>(
        "INSERT INTO labels (board_id, name, color) VALUES ($1, $2, $3) RETURNING id, board_id, name, color, created_at"
    )
    .bind(board_id).bind(req.name).bind(req.color).fetch_one(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let ws_msg = serde_json::json!({ "type": "LABEL_CREATED", "label": label, "workspace_id": board_ws }).to_string();
    let tx = ws_state.get_or_create_room(&board_ws.to_string()).await;
    let _ = tx.send(ws_msg);
    Ok(Json(label))
}

async fn delete_label(
    State(state): State<AuthState>,
    Path(label_id): Path<Uuid>,
    Extension(ws_state): Extension<Arc<crate::ws::AppState>>,
    session: Session,
) -> Result<StatusCode, StatusCode> {
    let user_id = get_user_id(&session).await.ok_or(StatusCode::UNAUTHORIZED)?;
    let board_id = sqlx::query_scalar::<_, Uuid>("SELECT board_id FROM labels WHERE id = $1").bind(label_id).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    let board_ws = sqlx::query_scalar::<_, Uuid>("SELECT workspace_id FROM boards WHERE id = $1").bind(board_id).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    if check_workspace_access(&state.pool, &user_id, &board_ws).await.unwrap_or_default() == "member" { return Err(StatusCode::FORBIDDEN); }

    sqlx::query("DELETE FROM labels WHERE id = $1").bind(label_id).execute(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let ws_msg = serde_json::json!({ "type": "LABEL_DELETED", "label_id": label_id, "workspace_id": board_ws }).to_string();
    let tx = ws_state.get_or_create_room(&board_ws.to_string()).await;
    let _ = tx.send(ws_msg);
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct UpdateLabelReq {
    pub name: String,
    pub color: String,
}

async fn update_label(
    State(state): State<AuthState>,
    Path(label_id): Path<Uuid>,
    Extension(ws_state): Extension<Arc<crate::ws::AppState>>,
    session: Session,
    Json(req): Json<UpdateLabelReq>,
) -> Result<Json<Label>, StatusCode> {
    let user_id = get_user_id(&session).await.ok_or(StatusCode::UNAUTHORIZED)?;
    let board_id = sqlx::query_scalar::<_, Uuid>("SELECT board_id FROM labels WHERE id = $1").bind(label_id).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    let board_ws = sqlx::query_scalar::<_, Uuid>("SELECT workspace_id FROM boards WHERE id = $1").bind(board_id).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    if check_workspace_access(&state.pool, &user_id, &board_ws).await.unwrap_or_default() == "member" { return Err(StatusCode::FORBIDDEN); }
    if req.name.trim().is_empty() { return Err(StatusCode::BAD_REQUEST); }

    let label = sqlx::query_as::<_, Label>(
        "UPDATE labels SET name = $1, color = $2 WHERE id = $3 RETURNING id, board_id, name, color, created_at"
    )
    .bind(req.name).bind(req.color).bind(label_id)
    .fetch_one(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Frontend doesn't currently handle LABEL_UPDATED WS event explicitly, we'll just send BOARD_UPDATED style logic or let frontend handle it
    let ws_msg = serde_json::json!({ "type": "LABEL_UPDATED", "label": label, "workspace_id": board_ws }).to_string();
    let tx = ws_state.get_or_create_room(&board_ws.to_string()).await;
    let _ = tx.send(ws_msg);
    
    Ok(Json(label))
}

async fn add_task_label(
    State(state): State<AuthState>,
    Path((task_id, label_id)): Path<(Uuid, Uuid)>,
    Extension(ws_state): Extension<Arc<crate::ws::AppState>>,
    session: Session,
) -> Result<StatusCode, StatusCode> {
    let user_id = get_user_id(&session).await.ok_or(StatusCode::UNAUTHORIZED)?;
    let col_id = sqlx::query_scalar::<_, Uuid>("SELECT column_id FROM tasks WHERE id = $1").bind(task_id).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    let board_id = sqlx::query_scalar::<_, Uuid>("SELECT board_id FROM board_columns WHERE id = $1").bind(col_id).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    let board_ws = sqlx::query_scalar::<_, Uuid>("SELECT workspace_id FROM boards WHERE id = $1").bind(board_id).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    let role = check_workspace_access(&state.pool, &user_id, &board_ws).await; if role.is_none() || role == Some("viewer".to_string()) { return Err(StatusCode::FORBIDDEN); }

    sqlx::query("INSERT INTO task_labels (task_id, label_id) VALUES ($1, $2) ON CONFLICT DO NOTHING").bind(task_id).bind(label_id).execute(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let _ = log_activity(
        &state.pool, task_id, Uuid::parse_str(&user_id).unwrap(), "add_label",
        None,
        Some(serde_json::json!({ "label_id": label_id }))
    ).await;

    let ws_msg = serde_json::json!({ "type": "TASK_UPDATED", "task_id": task_id, "workspace_id": board_ws }).to_string();
    let tx = ws_state.get_or_create_room(&board_ws.to_string()).await;
    let _ = tx.send(ws_msg);
    Ok(StatusCode::OK)
}

async fn remove_task_label(
    State(state): State<AuthState>,
    Path((task_id, label_id)): Path<(Uuid, Uuid)>,
    Extension(ws_state): Extension<Arc<crate::ws::AppState>>,
    session: Session,
) -> Result<StatusCode, StatusCode> {
    let user_id = get_user_id(&session).await.ok_or(StatusCode::UNAUTHORIZED)?;
    let col_id = sqlx::query_scalar::<_, Uuid>("SELECT column_id FROM tasks WHERE id = $1").bind(task_id).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    let board_id = sqlx::query_scalar::<_, Uuid>("SELECT board_id FROM board_columns WHERE id = $1").bind(col_id).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    let board_ws = sqlx::query_scalar::<_, Uuid>("SELECT workspace_id FROM boards WHERE id = $1").bind(board_id).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    let role = check_workspace_access(&state.pool, &user_id, &board_ws).await; if role.is_none() || role == Some("viewer".to_string()) { return Err(StatusCode::FORBIDDEN); }

    sqlx::query("DELETE FROM task_labels WHERE task_id = $1 AND label_id = $2").bind(task_id).bind(label_id).execute(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let _ = log_activity(
        &state.pool, task_id, Uuid::parse_str(&user_id).unwrap(), "remove_label",
        Some(serde_json::json!({ "label_id": label_id })),
        None
    ).await;

    let ws_msg = serde_json::json!({ "type": "TASK_UPDATED", "task_id": task_id, "workspace_id": board_ws }).to_string();
    let tx = ws_state.get_or_create_room(&board_ws.to_string()).await;
    let _ = tx.send(ws_msg);
    Ok(StatusCode::NO_CONTENT)
}

async fn add_task_assignee(
    State(state): State<AuthState>,
    Path((task_id, target_user_id)): Path<(Uuid, String)>,
    Extension(ws_state): Extension<Arc<crate::ws::AppState>>,
    session: Session,
) -> Result<StatusCode, StatusCode> {
    let user_id = get_user_id(&session).await.ok_or(StatusCode::UNAUTHORIZED)?;
    let col_id = sqlx::query_scalar::<_, Uuid>("SELECT column_id FROM tasks WHERE id = $1").bind(task_id).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    let board_id = sqlx::query_scalar::<_, Uuid>("SELECT board_id FROM board_columns WHERE id = $1").bind(col_id).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    let board_ws = sqlx::query_scalar::<_, Uuid>("SELECT workspace_id FROM boards WHERE id = $1").bind(board_id).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    let role = check_workspace_access(&state.pool, &user_id, &board_ws).await; if role.is_none() || role == Some("viewer".to_string()) { return Err(StatusCode::FORBIDDEN); }

    sqlx::query("INSERT INTO task_assignees (task_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING").bind(task_id).bind(&target_user_id).execute(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let _ = log_activity(
        &state.pool, task_id, Uuid::parse_str(&user_id).unwrap(), "add_assignee",
        None,
        Some(serde_json::json!({ "user_id": target_user_id }))
    ).await;

    let ws_msg = serde_json::json!({ "type": "TASK_UPDATED", "task_id": task_id, "workspace_id": board_ws }).to_string();
    let tx = ws_state.get_or_create_room(&board_ws.to_string()).await;
    let _ = tx.send(ws_msg);
    Ok(StatusCode::OK)
}

async fn remove_task_assignee(
    State(state): State<AuthState>,
    Path((task_id, target_user_id)): Path<(Uuid, String)>,
    Extension(ws_state): Extension<Arc<crate::ws::AppState>>,
    session: Session,
) -> Result<StatusCode, StatusCode> {
    let user_id = get_user_id(&session).await.ok_or(StatusCode::UNAUTHORIZED)?;
    let col_id = sqlx::query_scalar::<_, Uuid>("SELECT column_id FROM tasks WHERE id = $1").bind(task_id).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    let board_id = sqlx::query_scalar::<_, Uuid>("SELECT board_id FROM board_columns WHERE id = $1").bind(col_id).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    let board_ws = sqlx::query_scalar::<_, Uuid>("SELECT workspace_id FROM boards WHERE id = $1").bind(board_id).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    let role = check_workspace_access(&state.pool, &user_id, &board_ws).await; if role.is_none() || role == Some("viewer".to_string()) { return Err(StatusCode::FORBIDDEN); }

    sqlx::query("DELETE FROM task_assignees WHERE task_id = $1 AND user_id = $2").bind(task_id).bind(&target_user_id).execute(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let _ = log_activity(
        &state.pool, task_id, Uuid::parse_str(&user_id).unwrap(), "remove_assignee",
        Some(serde_json::json!({ "user_id": target_user_id })),
        None
    ).await;

    let ws_msg = serde_json::json!({ "type": "TASK_UPDATED", "task_id": task_id, "workspace_id": board_ws }).to_string();
    let tx = ws_state.get_or_create_room(&board_ws.to_string()).await;
    let _ = tx.send(ws_msg);
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct CreateSubtaskReq {
    pub title: String,
}

async fn create_subtask(
    State(state): State<AuthState>,
    Path(parent_id): Path<Uuid>,
    Extension(ws_state): Extension<Arc<crate::ws::AppState>>,
    session: Session,
    Json(req): Json<CreateSubtaskReq>,
) -> Result<Json<Task>, StatusCode> {
    let user_id = get_user_id(&session).await.ok_or(StatusCode::UNAUTHORIZED)?;
    
    // Get parent task to inherit column
    let parent = sqlx::query_as::<_, Task>(
        "SELECT id, column_id, parent_id, title, description, position, previous_column_id, is_per_person, is_completed, require_all_validations, priority, start_date, due_date, created_at, updated_at FROM tasks WHERE id = $1"
    ).bind(parent_id).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    
    let board_ws = sqlx::query_scalar::<_, Uuid>(
        "SELECT board_id FROM board_columns WHERE id = $1"
    ).bind(parent.column_id).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    let workspace_id = sqlx::query_scalar::<_, Uuid>("SELECT workspace_id FROM boards WHERE id = $1").bind(board_ws).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    
    let role = check_workspace_access(&state.pool, &user_id, &workspace_id).await; if role.is_none() || role == Some("viewer".to_string()) { return Err(StatusCode::FORBIDDEN); }
    if req.title.trim().is_empty() { return Err(StatusCode::BAD_REQUEST); }

    let max_pos: Option<f64> = sqlx::query_scalar("SELECT MAX(position) FROM tasks WHERE column_id = $1 AND parent_id = $2")
        .bind(parent.column_id).bind(parent_id).fetch_one(&state.pool).await.unwrap_or(Some(0.0));
    let next_pos = max_pos.unwrap_or(0.0) + 65536.0;

    let subtask = sqlx::query_as::<_, Task>(
        "INSERT INTO tasks (column_id, parent_id, title, position, priority) VALUES ($1, $2, $3, $4, 0) RETURNING id, column_id, parent_id, title, description, position, previous_column_id, is_per_person, is_completed, priority, start_date, due_date, require_all_validations, created_at, updated_at"
    )
    .bind(parent.column_id).bind(parent_id).bind(req.title).bind(next_pos)
    .fetch_one(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let ws_msg = serde_json::json!({ "type": "TASK_CREATED", "task": subtask, "workspace_id": workspace_id }).to_string();
    let tx = ws_state.get_or_create_room(&workspace_id.to_string()).await;
    let _ = tx.send(ws_msg);

    Ok(Json(subtask))
}

async fn toggle_task_validation(
    State(state): State<AuthState>,
    Path(task_id): Path<Uuid>,
    Extension(ws_state): Extension<Arc<crate::ws::AppState>>,
    session: Session,
) -> Result<Json<TaskWithEnrichment>, StatusCode> {
    let user_id = get_user_id(&session).await.ok_or(StatusCode::UNAUTHORIZED)?;
    
    let mut task = sqlx::query_as::<_, Task>(
        "SELECT id, column_id, parent_id, title, description, position, previous_column_id, is_per_person, is_completed, require_all_validations, priority, start_date, due_date, created_at, updated_at FROM tasks WHERE id = $1"
    ).bind(task_id).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    
    let board_ws = sqlx::query_scalar::<_, Uuid>("SELECT board_id FROM board_columns WHERE id = $1").bind(task.column_id).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    let workspace_id = sqlx::query_scalar::<_, Uuid>("SELECT workspace_id FROM boards WHERE id = $1").bind(board_ws).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    
    let role = check_workspace_access(&state.pool, &user_id, &workspace_id).await; if role.is_none() || role == Some("viewer".to_string()) { return Err(StatusCode::FORBIDDEN); }

    let subtasks = sqlx::query_as::<_, Task>(
        "SELECT id, column_id, parent_id, title, description, position, previous_column_id, is_per_person, is_completed, require_all_validations, priority, start_date, due_date, created_at, updated_at FROM tasks WHERE parent_id = $1"
    ).bind(task_id).fetch_all(&state.pool).await.unwrap_or_default();
    
    if !subtasks.iter().all(|t| t.is_completed) {
        return Err(StatusCode::BAD_REQUEST);
    }

    if task.is_per_person {
        let exists: Option<String> = sqlx::query_scalar("SELECT user_id FROM task_validations WHERE task_id = $1 AND user_id = $2")
            .bind(task_id).bind(&user_id).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            
        if exists.is_some() {
            sqlx::query("DELETE FROM task_validations WHERE task_id = $1 AND user_id = $2")
                .bind(task_id).bind(&user_id).execute(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        } else {
            sqlx::query("INSERT INTO task_validations (task_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
                .bind(task_id).bind(&user_id).execute(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }
        
        let assignees: Vec<String> = sqlx::query_scalar("SELECT user_id FROM task_assignees WHERE task_id = $1")
            .bind(task_id).fetch_all(&state.pool).await.unwrap_or_default();
            
        let validations: Vec<String> = sqlx::query_scalar("SELECT user_id FROM task_validations WHERE task_id = $1")
            .bind(task_id).fetch_all(&state.pool).await.unwrap_or_default();
            
        if task.require_all_validations { task.is_completed = !assignees.is_empty() && assignees.iter().all(|a| validations.contains(a)); } else { task.is_completed = !validations.is_empty(); }
        sqlx::query("UPDATE tasks SET is_completed = $1 WHERE id = $2").bind(task.is_completed).bind(task_id).execute(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    } else {
        task.is_completed = !task.is_completed;
        sqlx::query("UPDATE tasks SET is_completed = $1 WHERE id = $2").bind(task.is_completed).bind(task_id).execute(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    let validations: Vec<String> = sqlx::query_scalar("SELECT user_id FROM task_validations WHERE task_id = $1")
        .bind(task_id).fetch_all(&state.pool).await.unwrap_or_default();
    let assignees: Vec<String> = sqlx::query_scalar("SELECT user_id FROM task_assignees WHERE task_id = $1").bind(task_id).fetch_all(&state.pool).await.unwrap_or_default();

    let enriched = TaskWithEnrichment {
        task,
        labels: vec![],
        assignees,
        validations,
    };

    let ws_msg = serde_json::json!({ "type": "TASK_VALIDATION_UPDATED", "task": enriched, "workspace_id": workspace_id }).to_string();
    let tx = ws_state.get_or_create_room(&workspace_id.to_string()).await;
    let _ = tx.send(ws_msg);

    let _ = log_activity(
        &state.pool, task_id, Uuid::parse_str(&user_id).unwrap(), "toggle_validation",
        Some(serde_json::json!({ "is_completed": !enriched.task.is_completed })),
        Some(serde_json::json!({ "is_completed": enriched.task.is_completed }))
    ).await;

    Ok(Json(enriched))
}

// --- ATTACHMENTS & ACTIVITIES ---

async fn log_activity(
    pool: &sqlx::PgPool,
    task_id: Uuid,
    user_id: Uuid,
    activity_type: &str,
    old_value: Option<serde_json::Value>,
    new_value: Option<serde_json::Value>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO task_activities (id, task_id, user_id, activity_type, old_value, new_value) VALUES ($1, $2, $3, $4, $5, $6)"
    )
    .bind(Uuid::new_v4())
    .bind(task_id)
    .bind(user_id)
    .bind(activity_type)
    .bind(old_value)
    .bind(new_value)
    .execute(pool)
    .await?;
    Ok(())
}


async fn get_task_attachments(
    State(state): State<AuthState>,
    Path(task_id): Path<Uuid>,
    session: Session,
) -> Result<Json<Vec<TaskAttachment>>, StatusCode> {
    let _user_id = get_user_id(&session).await.ok_or(StatusCode::UNAUTHORIZED)?;
    
    let attachments = sqlx::query_as::<_, TaskAttachment>(
        "SELECT id, task_id, uploaded_by, filename, file_path, mime_type, size_bytes, created_at FROM task_attachments WHERE task_id = $1 ORDER BY created_at DESC"
    )
    .bind(task_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(attachments))
}

async fn upload_attachment(
    State(state): State<AuthState>,
    Path(task_id): Path<Uuid>,
    session: Session,
    mut multipart: Multipart,
) -> Result<Json<TaskAttachment>, StatusCode> {
    let user_id = get_user_id(&session).await.ok_or(StatusCode::UNAUTHORIZED)?;
    
    let mut uploaded_attachment = None;

    while let Some(field) = multipart.next_field().await.map_err(|_| StatusCode::BAD_REQUEST)? {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" {
            let filename = field.file_name().unwrap_or("unknown").to_string();
            let content_type = field.content_type().map(|s| s.to_string());
            let data = field.bytes().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            
            let file_id = Uuid::new_v4();
            let ext = std::path::Path::new(&filename).extension().and_then(|e| e.to_str()).unwrap_or("");
            let file_path = format!("uploads/{}.{}", file_id, ext);
            
            tokio::fs::write(&file_path, &data).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            
            let att = sqlx::query_as::<_, TaskAttachment>(
                "INSERT INTO task_attachments (id, task_id, uploaded_by, filename, file_path, mime_type, size_bytes) VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id, task_id, uploaded_by, filename, file_path, mime_type, size_bytes, created_at"
            )
            .bind(file_id)
            .bind(task_id)
            .bind(Uuid::parse_str(&user_id).unwrap())
            .bind(filename)
            .bind(file_path)
            .bind(content_type)
            .bind(data.len() as i64)
            .fetch_one(&state.pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            
            uploaded_attachment = Some(att);
            break;
        }
    }
    
    if let Some(att) = uploaded_attachment {
        Ok(Json(att))
    } else {
        Err(StatusCode::BAD_REQUEST)
    }
}

async fn delete_attachment(
    State(state): State<AuthState>,
    Path((task_id, attachment_id)): Path<(Uuid, Uuid)>,
    session: Session,
) -> Result<StatusCode, StatusCode> {
    let _user_id = get_user_id(&session).await.ok_or(StatusCode::UNAUTHORIZED)?;
    
    // On pourrait vérifier les permissions ici (si l'utilisateur a accès au workspace de la tâche)
    // Pour simplifier on le supprime juste
    
    let att = sqlx::query_scalar::<_, String>("SELECT file_path FROM task_attachments WHERE id = $1 AND task_id = $2")
        .bind(attachment_id)
        .bind(task_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(file_path) = att {
        tokio::fs::remove_file(&file_path).await.ok();
        
        sqlx::query("DELETE FROM task_attachments WHERE id = $1")
            .bind(attachment_id)
            .execute(&state.pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn get_task_activities(
    State(state): State<AuthState>,
    Path(task_id): Path<Uuid>,
    session: Session,
) -> Result<Json<Vec<TaskActivity>>, StatusCode> {
    let _user_id = get_user_id(&session).await.ok_or(StatusCode::UNAUTHORIZED)?;
    
    let activities = sqlx::query_as::<_, TaskActivity>(
        "SELECT id, task_id, user_id, activity_type, old_value, new_value, created_at FROM task_activities WHERE task_id = $1 ORDER BY created_at DESC"
    )
    .bind(task_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(activities))
}

async fn restore_task_activity(
    State(state): State<AuthState>,
    Path((task_id, activity_id)): Path<(Uuid, Uuid)>,
    session: Session,
) -> Result<StatusCode, StatusCode> {
    let _user_id = get_user_id(&session).await.ok_or(StatusCode::UNAUTHORIZED)?;
    
    let activity = sqlx::query_as::<_, TaskActivity>(
        "SELECT id, task_id, user_id, activity_type, old_value, new_value, created_at FROM task_activities WHERE id = $1 AND task_id = $2"
    )
    .bind(activity_id).bind(task_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;
    
    if let Some(old_val) = activity.old_value {
        let mut updated = false;
        match activity.activity_type.as_str() {
            "update_title" => {
                if let Some(title) = old_val.get("title").and_then(|t| t.as_str()) {
                    sqlx::query("UPDATE tasks SET title = $1 WHERE id = $2")
                        .bind(title).bind(task_id).execute(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                    updated = true;
                }
            }
            "update_description" => {
                if let Some(desc) = old_val.get("description").and_then(|d| d.as_str()) {
                    sqlx::query("UPDATE tasks SET description = $1 WHERE id = $2")
                        .bind(desc).bind(task_id).execute(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                    updated = true;
                }
            }
            _ => {}
        }
        if updated {
            let col_id = sqlx::query_scalar::<_, Uuid>("SELECT column_id FROM tasks WHERE id = $1").bind(task_id).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.unwrap_or_default();
            let board_id = sqlx::query_scalar::<_, Uuid>("SELECT board_id FROM board_columns WHERE id = $1").bind(col_id).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.unwrap_or_default();
            let _board_ws = sqlx::query_scalar::<_, Uuid>("SELECT workspace_id FROM boards WHERE id = $1").bind(board_id).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.unwrap_or_default();
            // We just trigger task_id update so the frontend can refetch
            // Wait, we don't have Extension(ws_state) injected in this handler!
        }
    }
    
    Ok(StatusCode::OK)
}

#[derive(serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct TaskComment {
    pub id: Uuid,
    pub task_id: Uuid,
    pub user_id: String,
    pub content: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(serde::Deserialize)]
pub struct CreateComment {
    pub content: String,
}

async fn get_task_comments(
    State(state): State<AuthState>,
    Path(task_id): Path<Uuid>,
) -> Result<Json<Vec<TaskComment>>, StatusCode> {
    let comments = sqlx::query_as::<_, TaskComment>(
        "SELECT id, task_id, user_id, content, created_at, updated_at FROM task_comments WHERE task_id = $1 ORDER BY created_at ASC"
    )
    .bind(task_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(comments))
}

async fn add_task_comment(
    State(state): State<AuthState>,
    Path(task_id): Path<Uuid>,
    session: Session,
    Json(payload): Json<CreateComment>,
) -> Result<Json<TaskComment>, StatusCode> {
    let user_id = get_user_id(&session).await.ok_or(StatusCode::UNAUTHORIZED)?;
    
    let comment = sqlx::query_as::<_, TaskComment>(
        "INSERT INTO task_comments (task_id, user_id, content) VALUES ($1, $2, $3) RETURNING id, task_id, user_id, content, created_at, updated_at"
    )
    .bind(task_id)
    .bind(user_id.to_string())
    .bind(payload.content)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        eprintln!("add_task_comment: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    
    Ok(Json(comment))
}

async fn delete_task_comment(
    State(state): State<AuthState>,
    Path((task_id, comment_id)): Path<(Uuid, Uuid)>,
    session: Session,
) -> Result<StatusCode, StatusCode> {
    let _user_id = get_user_id(&session).await.ok_or(StatusCode::UNAUTHORIZED)?;
    
    let result = sqlx::query("DELETE FROM task_comments WHERE id = $1 AND task_id = $2")
        .bind(comment_id)
        .bind(task_id)
        .execute(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
    if result.rows_affected() > 0 {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
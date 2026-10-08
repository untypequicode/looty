use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    routing::{delete, get, post, put},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

use crate::access::{
    board_workspace, column_ctx, is_member, label_ctx, require_member, task_ctx, AuthUser,
};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Modèles
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct Board {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
    pub automations: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct BoardColumn {
    pub id: Uuid,
    pub board_id: Uuid,
    pub name: String,
    pub position: f64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub const TASK_COLUMNS: &str = "id, column_id, parent_id, title, description, position, previous_column_id, \
    is_per_person, is_completed, require_all_validations, start_date, due_date, priority, created_at, updated_at, event_id, event_start";

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
    /// Événement lié (ex. la séance où la tâche est à rendre) et occurrence précise.
    #[sqlx(default)]
    pub event_id: Option<Uuid>,
    #[sqlx(default)]
    pub event_start: Option<DateTime<Utc>>,
}

#[derive(Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct Label {
    pub id: Uuid,
    pub board_id: Uuid,
    pub name: String,
    pub color: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, sqlx::FromRow, Clone)]
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

#[derive(Serialize, sqlx::FromRow, Clone)]
pub struct TaskActivity {
    pub id: Uuid,
    pub task_id: Uuid,
    pub user_id: Uuid,
    pub activity_type: String,
    pub old_value: Option<serde_json::Value>,
    pub new_value: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, Clone)]
pub struct TaskWithEnrichment {
    #[serde(flatten)]
    pub task: Task,
    pub labels: Vec<Label>,
    pub assignees: Vec<String>,
    pub validations: Vec<String>,
}

#[derive(Serialize)]
pub struct ColumnWithTasks {
    #[serde(flatten)]
    pub column: BoardColumn,
    pub tasks: Vec<TaskWithEnrichment>,
}

#[derive(Serialize)]
pub struct BoardFull {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
    pub automations: serde_json::Value,
    pub columns: Vec<ColumnWithTasks>,
    pub labels: Vec<Label>,
}

#[derive(Deserialize, Default)]
struct Automations {
    on_complete_move_to: Option<String>,
    #[serde(default)]
    apply_to_subtasks: bool,
    #[serde(default)]
    auto_validate_on_move: bool,
}

impl Automations {
    fn from_json(v: &serde_json::Value) -> Self {
        serde_json::from_value(v.clone()).unwrap_or_default()
    }
    fn done_column(&self) -> Option<Uuid> {
        self.on_complete_move_to.as_deref().and_then(|s| Uuid::parse_str(s).ok())
    }
}

fn double_option<'de, D, T>(de: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(de).map(Some)
}

// ---------------------------------------------------------------------------
// Routes
// ---------------------------------------------------------------------------

pub fn workspace_routes() -> Router<AppState> {
    Router::new().route("/", get(list_boards).post(create_board))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/:board_id", get(get_board).put(update_board).delete(delete_board))
        .route("/:board_id/columns", post(create_column))
        .route("/:board_id/labels", post(create_label))
        .route("/labels/:label_id", put(update_label).delete(delete_label))
        .route("/columns/:column_id", put(update_column).delete(delete_column))
        .route("/columns/:column_id/move", put(move_column))
        .route("/columns/:column_id/tasks", post(create_task))
        .route("/tasks/:task_id", get(get_task).put(update_task).delete(delete_task))
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
        .route("/tasks/:task_id/comments/:comment_id", put(edit_task_comment).delete(delete_task_comment))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn check_name(name: &str, max: usize) -> ApiResult<String> {
    let n = name.trim();
    if n.is_empty() || n.chars().count() > max {
        return Err(ApiError::bad_request("invalid_name"));
    }
    Ok(n.to_string())
}

fn check_color(color: &str) -> ApiResult<String> {
    let c = color.trim();
    let ok = c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|ch| ch.is_ascii_hexdigit());
    if !ok {
        return Err(ApiError::bad_request("invalid_color"));
    }
    Ok(c.to_lowercase())
}

fn check_priority(p: i32) -> ApiResult<i32> {
    if !(0..=4).contains(&p) {
        return Err(ApiError::bad_request("invalid_priority"));
    }
    Ok(p)
}

async fn fetch_task(pool: &PgPool, task_id: Uuid) -> ApiResult<Task> {
    Ok(sqlx::query_as(&format!("SELECT {TASK_COLUMNS} FROM tasks WHERE id = $1"))
        .bind(task_id)
        .fetch_optional(pool)
        .await?
        .ok_or(ApiError::NotFound)?)
}

/// Tâche + labels + assignés + validations, prête à être envoyée au client.
pub async fn load_task_enriched(pool: &PgPool, task_id: Uuid) -> ApiResult<TaskWithEnrichment> {
    let task = fetch_task(pool, task_id).await?;
    let labels: Vec<Label> = sqlx::query_as(
        "SELECT l.id, l.board_id, l.name, l.color, l.created_at FROM labels l
         JOIN task_labels tl ON tl.label_id = l.id WHERE tl.task_id = $1 ORDER BY l.name",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await?;
    let assignees: Vec<String> = sqlx::query_scalar("SELECT user_id FROM task_assignees WHERE task_id = $1")
        .bind(task_id)
        .fetch_all(pool)
        .await?;
    let validations: Vec<String> = sqlx::query_scalar("SELECT user_id FROM task_validations WHERE task_id = $1")
        .bind(task_id)
        .fetch_all(pool)
        .await?;
    Ok(TaskWithEnrichment { task, labels, assignees, validations })
}

pub(crate) async fn emit_task(state: &AppState, workspace_id: Uuid, kind: &str, task_id: Uuid) {
    if let Ok(t) = load_task_enriched(&state.pool, task_id).await {
        state.emit(workspace_id, serde_json::json!({ "type": kind, "task": t })).await;
    }
}

async fn log_activity(
    pool: &PgPool,
    task_id: Uuid,
    user_id: Uuid,
    activity_type: &str,
    old_value: Option<serde_json::Value>,
    new_value: Option<serde_json::Value>,
) {
    let res = sqlx::query(
        "INSERT INTO task_activities (task_id, user_id, activity_type, old_value, new_value) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(task_id)
    .bind(user_id)
    .bind(activity_type)
    .bind(old_value)
    .bind(new_value)
    .execute(pool)
    .await;
    if let Err(e) = res {
        tracing::warn!("log_activity failed: {}", e);
    }
}

/// Prévient les personnes assignées (sauf l'auteur) qu'une tâche vient d'être terminée.
async fn notify_completed(state: &AppState, task_id: Uuid, actor: Uuid) {
    let pool = &state.pool;
    let Ok(ctx) = task_ctx(pool, task_id).await else { return };
    let title: String = sqlx::query_scalar("SELECT title FROM tasks WHERE id = $1").bind(task_id).fetch_one(pool).await.unwrap_or_default();
    let assignees: Vec<String> = sqlx::query_scalar("SELECT user_id FROM task_assignees WHERE task_id = $1").bind(task_id).fetch_all(pool).await.unwrap_or_default();
    for a in assignees.iter().filter_map(|a| Uuid::parse_str(a).ok()) {
        crate::notifications::notify(pool, &state.hub, ctx.workspace_id, a, Some(actor), "completed", &format!("task:{task_id}"),
            serde_json::json!({ "title": title, "board_id": ctx.board_id })).await;
    }
}

async fn next_task_position(pool: &PgPool, column_id: Uuid) -> ApiResult<f64> {
    Ok(sqlx::query_scalar("SELECT COALESCE(MAX(position), 0) + 65536 FROM tasks WHERE column_id = $1")
        .bind(column_id)
        .fetch_one(pool)
        .await?)
}

async fn board_automations(pool: &PgPool, board_id: Uuid) -> ApiResult<Automations> {
    let v: serde_json::Value = sqlx::query_scalar("SELECT automations FROM boards WHERE id = $1")
        .bind(board_id)
        .fetch_one(pool)
        .await?;
    Ok(Automations::from_json(&v))
}

/// Applique les automatisations de complétion après un changement d'état :
/// - complétée -> déplacée vers la colonne « terminé » (en mémorisant la colonne d'origine) ;
/// - ré-ouverte -> renvoyée dans sa colonne d'origine.
async fn apply_completion_automation(pool: &PgPool, board_id: Uuid, task: &Task) -> ApiResult<()> {
    let auto = board_automations(pool, board_id).await?;
    let Some(done) = auto.done_column() else { return Ok(()) };
    if task.parent_id.is_some() && !auto.apply_to_subtasks {
        return Ok(());
    }
    // La colonne cible doit appartenir au même tableau.
    let valid: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM board_columns WHERE id = $1 AND board_id = $2)")
        .bind(done)
        .bind(board_id)
        .fetch_one(pool)
        .await?;
    if !valid {
        return Ok(());
    }
    if task.is_completed && task.column_id != done {
        let pos = next_task_position(pool, done).await?;
        sqlx::query("UPDATE tasks SET previous_column_id = column_id, column_id = $1, position = $2, updated_at = NOW() WHERE id = $3")
            .bind(done)
            .bind(pos)
            .bind(task.id)
            .execute(pool)
            .await?;
    } else if !task.is_completed && task.column_id == done {
        if let Some(prev) = task.previous_column_id {
            let pos = next_task_position(pool, prev).await?;
            sqlx::query(
                "UPDATE tasks SET column_id = $1, position = $2, previous_column_id = NULL, updated_at = NOW()
                 WHERE id = $3 AND EXISTS(SELECT 1 FROM board_columns WHERE id = $1 AND board_id = $4)",
            )
            .bind(prev)
            .bind(pos)
            .bind(task.id)
            .bind(board_id)
            .execute(pool)
            .await?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tableaux
// ---------------------------------------------------------------------------

async fn list_boards(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path(workspace_id): Path<Uuid>,
) -> ApiResult<Json<Vec<Board>>> {
    let Some((_, perms)) = crate::access::member_perms(&pool, user.id, workspace_id).await? else {
        // Invité : aucun tableau.
        return if crate::access::is_guest(&pool, user.id, workspace_id).await? { Ok(Json(Vec::new())) } else { Err(ApiError::NotFound) };
    };
    if perms.boards == crate::access::level::NONE {
        return Ok(Json(Vec::new()));
    }
    Ok(Json(
        sqlx::query_as(
            "SELECT id, workspace_id, name, automations, created_at, updated_at FROM boards
             WHERE workspace_id = $1 ORDER BY created_at ASC",
        )
        .bind(workspace_id)
        .fetch_all(&pool)
        .await?,
    ))
}

#[derive(Deserialize)]
pub struct CreateBoardReq {
    pub name: String,
    /// Crée les colonnes par défaut (À faire / En cours / Terminé).
    pub with_default_columns: Option<bool>,
}

async fn create_board(
    State(state): State<AppState>,
    user: AuthUser,
    Path(workspace_id): Path<Uuid>,
    Json(req): Json<CreateBoardReq>,
) -> ApiResult<Json<Board>> {
    crate::access::require_perm(&state.pool, &user, workspace_id, |p| p.boards >= crate::access::level::MANAGE).await?;
    let name = check_name(&req.name, 120)?;
    let mut tx = state.pool.begin().await?;
    let mut board: Board = sqlx::query_as(
        "INSERT INTO boards (workspace_id, name) VALUES ($1, $2)
         RETURNING id, workspace_id, name, automations, created_at, updated_at",
    )
    .bind(workspace_id)
    .bind(name)
    .fetch_one(&mut *tx)
    .await?;
    if req.with_default_columns.unwrap_or(true) {
        let mut done_id = None;
        for (i, col) in ["Backlog", "À faire", "En cours", "Terminé"].iter().enumerate() {
            let id: Uuid = sqlx::query_scalar(
                "INSERT INTO board_columns (board_id, name, position) VALUES ($1, $2, $3) RETURNING id",
            )
            .bind(board.id)
            .bind(col)
            .bind((i as f64 + 1.0) * 65536.0)
            .fetch_one(&mut *tx)
            .await?;
            if i == 3 {
                done_id = Some(id);
            }
        }
        if let Some(done) = done_id {
            board.automations = serde_json::json!({
                "on_complete_move_to": done.to_string(),
                "apply_to_subtasks": false,
                "auto_validate_on_move": true
            });
            sqlx::query("UPDATE boards SET automations = $1 WHERE id = $2")
                .bind(&board.automations)
                .bind(board.id)
                .execute(&mut *tx)
                .await?;
        }
    }
    tx.commit().await?;
    state.emit(workspace_id, serde_json::json!({ "type": "BOARD_CREATED", "board": &board })).await;
    Ok(Json(board))
}

async fn get_board(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path(board_id): Path<Uuid>,
) -> ApiResult<Json<BoardFull>> {
    let board: Board = sqlx::query_as(
        "SELECT id, workspace_id, name, automations, created_at, updated_at FROM boards WHERE id = $1",
    )
    .bind(board_id)
    .fetch_optional(&pool)
    .await?
    .ok_or(ApiError::NotFound)?;
    crate::access::require_perm(&pool, &user, board.workspace_id, |p| p.boards >= crate::access::level::READ).await?;

    let columns: Vec<BoardColumn> = sqlx::query_as(
        "SELECT id, board_id, name, position, created_at, updated_at FROM board_columns
         WHERE board_id = $1 ORDER BY position ASC",
    )
    .bind(board_id)
    .fetch_all(&pool)
    .await?;
    let labels: Vec<Label> =
        sqlx::query_as("SELECT id, board_id, name, color, created_at FROM labels WHERE board_id = $1 ORDER BY name")
            .bind(board_id)
            .fetch_all(&pool)
            .await?;
    let tasks: Vec<Task> = sqlx::query_as(&format!(
        "SELECT {} FROM tasks t WHERE t.column_id IN (SELECT id FROM board_columns WHERE board_id = $1)
         ORDER BY t.position ASC",
        TASK_COLUMNS.split(", ").map(|c| format!("t.{c}")).collect::<Vec<_>>().join(", ")
    ))
    .bind(board_id)
    .fetch_all(&pool)
    .await?;

    let task_labels: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT tl.task_id, tl.label_id FROM task_labels tl JOIN labels l ON l.id = tl.label_id WHERE l.board_id = $1",
    )
    .bind(board_id)
    .fetch_all(&pool)
    .await?;
    let assignees: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT ta.task_id, ta.user_id FROM task_assignees ta JOIN tasks t ON ta.task_id = t.id
         JOIN board_columns c ON t.column_id = c.id WHERE c.board_id = $1",
    )
    .bind(board_id)
    .fetch_all(&pool)
    .await?;
    let validations: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT v.task_id, v.user_id FROM task_validations v JOIN tasks t ON v.task_id = t.id
         JOIN board_columns c ON t.column_id = c.id WHERE c.board_id = $1",
    )
    .bind(board_id)
    .fetch_all(&pool)
    .await?;

    let label_by_id: HashMap<Uuid, &Label> = labels.iter().map(|l| (l.id, l)).collect();
    let mut labels_of: HashMap<Uuid, Vec<Label>> = HashMap::new();
    for (t, l) in task_labels {
        if let Some(lbl) = label_by_id.get(&l) {
            labels_of.entry(t).or_default().push((*lbl).clone());
        }
    }
    let mut assignees_of: HashMap<Uuid, Vec<String>> = HashMap::new();
    for (t, u) in assignees {
        assignees_of.entry(t).or_default().push(u);
    }
    let mut validations_of: HashMap<Uuid, Vec<String>> = HashMap::new();
    for (t, u) in validations {
        validations_of.entry(t).or_default().push(u);
    }

    let mut tasks_of: HashMap<Uuid, Vec<TaskWithEnrichment>> = HashMap::new();
    for task in tasks {
        let id = task.id;
        tasks_of.entry(task.column_id).or_default().push(TaskWithEnrichment {
            task,
            labels: labels_of.remove(&id).unwrap_or_default(),
            assignees: assignees_of.remove(&id).unwrap_or_default(),
            validations: validations_of.remove(&id).unwrap_or_default(),
        });
    }
    let columns = columns
        .into_iter()
        .map(|c| {
            let tasks = tasks_of.remove(&c.id).unwrap_or_default();
            ColumnWithTasks { column: c, tasks }
        })
        .collect();

    Ok(Json(BoardFull {
        id: board.id,
        workspace_id: board.workspace_id,
        name: board.name,
        automations: board.automations,
        columns,
        labels,
    }))
}

#[derive(Deserialize)]
pub struct UpdateBoardReq {
    pub name: Option<String>,
    pub automations: Option<serde_json::Value>,
}

async fn update_board(
    State(state): State<AppState>,
    user: AuthUser,
    Path(board_id): Path<Uuid>,
    Json(req): Json<UpdateBoardReq>,
) -> ApiResult<Json<Board>> {
    let ws = board_workspace(&state.pool, board_id).await?;
    crate::access::require_perm(&state.pool, &user, ws, |p| p.boards >= crate::access::level::MANAGE).await?;
    let name = req.name.as_deref().map(|n| check_name(n, 120)).transpose()?;
    if let Some(a) = &req.automations {
        if !a.is_object() {
            return Err(ApiError::bad_request("invalid_automations"));
        }
    }
    let board: Board = sqlx::query_as(
        "UPDATE boards SET name = COALESCE($1, name), automations = COALESCE($2, automations), updated_at = NOW()
         WHERE id = $3 RETURNING id, workspace_id, name, automations, created_at, updated_at",
    )
    .bind(name)
    .bind(req.automations)
    .bind(board_id)
    .fetch_one(&state.pool)
    .await?;
    state.emit(ws, serde_json::json!({ "type": "BOARD_UPDATED", "board": &board })).await;
    Ok(Json(board))
}

async fn delete_board(
    State(state): State<AppState>,
    user: AuthUser,
    Path(board_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let ws = board_workspace(&state.pool, board_id).await?;
    // Supprimer un tableau entier est réservé aux admins du workspace.
    crate::access::require_ws_admin(&state.pool, &user, ws).await?;
    sqlx::query("DELETE FROM boards WHERE id = $1").bind(board_id).execute(&state.pool).await?;
    state.emit(ws, serde_json::json!({ "type": "BOARD_DELETED", "board_id": board_id })).await;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Colonnes
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct ColumnNameReq {
    pub name: String,
}

async fn create_column(
    State(state): State<AppState>,
    user: AuthUser,
    Path(board_id): Path<Uuid>,
    Json(req): Json<ColumnNameReq>,
) -> ApiResult<Json<BoardColumn>> {
    let ws = board_workspace(&state.pool, board_id).await?;
    crate::access::require_perm(&state.pool, &user, ws, |p| p.boards >= crate::access::level::MANAGE).await?;
    let name = check_name(&req.name, 80)?;
    let col: BoardColumn = sqlx::query_as(
        "INSERT INTO board_columns (board_id, name, position)
         VALUES ($1, $2, (SELECT COALESCE(MAX(position), 0) + 65536 FROM board_columns WHERE board_id = $1))
         RETURNING id, board_id, name, position, created_at, updated_at",
    )
    .bind(board_id)
    .bind(name)
    .fetch_one(&state.pool)
    .await?;
    state.emit(ws, serde_json::json!({ "type": "COLUMN_CREATED", "column": &col })).await;
    Ok(Json(col))
}

async fn update_column(
    State(state): State<AppState>,
    user: AuthUser,
    Path(column_id): Path<Uuid>,
    Json(req): Json<ColumnNameReq>,
) -> ApiResult<Json<BoardColumn>> {
    let ctx = column_ctx(&state.pool, column_id).await?;
    crate::access::require_perm(&state.pool, &user, ctx.workspace_id, |p| p.boards >= crate::access::level::MANAGE).await?;
    let name = check_name(&req.name, 80)?;
    let col: BoardColumn = sqlx::query_as(
        "UPDATE board_columns SET name = $1, updated_at = NOW() WHERE id = $2
         RETURNING id, board_id, name, position, created_at, updated_at",
    )
    .bind(name)
    .bind(column_id)
    .fetch_one(&state.pool)
    .await?;
    state.emit(ctx.workspace_id, serde_json::json!({ "type": "COLUMN_UPDATED", "column": &col })).await;
    Ok(Json(col))
}

async fn delete_column(
    State(state): State<AppState>,
    user: AuthUser,
    Path(column_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let ctx = column_ctx(&state.pool, column_id).await?;
    crate::access::require_perm(&state.pool, &user, ctx.workspace_id, |p| p.boards >= crate::access::level::MANAGE).await?;
    sqlx::query("DELETE FROM board_columns WHERE id = $1").bind(column_id).execute(&state.pool).await?;
    state
        .emit(
            ctx.workspace_id,
            serde_json::json!({ "type": "COLUMN_DELETED", "column_id": column_id, "board_id": ctx.board_id }),
        )
        .await;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct MoveColumnReq {
    pub position: f64,
}

async fn move_column(
    State(state): State<AppState>,
    user: AuthUser,
    Path(column_id): Path<Uuid>,
    Json(req): Json<MoveColumnReq>,
) -> ApiResult<Json<BoardColumn>> {
    let ctx = column_ctx(&state.pool, column_id).await?;
    crate::access::require_perm(&state.pool, &user, ctx.workspace_id, |p| p.boards >= crate::access::level::MANAGE).await?;
    if !req.position.is_finite() {
        return Err(ApiError::bad_request("invalid_position"));
    }
    let col: BoardColumn = sqlx::query_as(
        "UPDATE board_columns SET position = $1, updated_at = NOW() WHERE id = $2
         RETURNING id, board_id, name, position, created_at, updated_at",
    )
    .bind(req.position)
    .bind(column_id)
    .fetch_one(&state.pool)
    .await?;
    state.emit(ctx.workspace_id, serde_json::json!({ "type": "COLUMN_MOVED", "column": &col })).await;
    Ok(Json(col))
}

// ---------------------------------------------------------------------------
// Labels
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct LabelReq {
    pub name: String,
    pub color: String,
}

async fn create_label(
    State(state): State<AppState>,
    user: AuthUser,
    Path(board_id): Path<Uuid>,
    Json(req): Json<LabelReq>,
) -> ApiResult<Json<Label>> {
    let ws = board_workspace(&state.pool, board_id).await?;
    crate::access::require_perm(&state.pool, &user, ws, |p| p.boards >= crate::access::level::MANAGE).await?;
    let label: Label = sqlx::query_as(
        "INSERT INTO labels (board_id, name, color) VALUES ($1, $2, $3) RETURNING id, board_id, name, color, created_at",
    )
    .bind(board_id)
    .bind(check_name(&req.name, 50)?)
    .bind(check_color(&req.color)?)
    .fetch_one(&state.pool)
    .await?;
    state.emit(ws, serde_json::json!({ "type": "LABEL_CREATED", "label": &label })).await;
    Ok(Json(label))
}

async fn update_label(
    State(state): State<AppState>,
    user: AuthUser,
    Path(label_id): Path<Uuid>,
    Json(req): Json<LabelReq>,
) -> ApiResult<Json<Label>> {
    let (ws, _) = label_ctx(&state.pool, label_id).await?;
    crate::access::require_perm(&state.pool, &user, ws, |p| p.boards >= crate::access::level::MANAGE).await?;
    let label: Label = sqlx::query_as(
        "UPDATE labels SET name = $1, color = $2 WHERE id = $3 RETURNING id, board_id, name, color, created_at",
    )
    .bind(check_name(&req.name, 50)?)
    .bind(check_color(&req.color)?)
    .bind(label_id)
    .fetch_one(&state.pool)
    .await?;
    state.emit(ws, serde_json::json!({ "type": "LABEL_UPDATED", "label": &label })).await;
    Ok(Json(label))
}

async fn delete_label(
    State(state): State<AppState>,
    user: AuthUser,
    Path(label_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let (ws, board_id) = label_ctx(&state.pool, label_id).await?;
    crate::access::require_perm(&state.pool, &user, ws, |p| p.boards >= crate::access::level::MANAGE).await?;
    sqlx::query("DELETE FROM labels WHERE id = $1").bind(label_id).execute(&state.pool).await?;
    state
        .emit(ws, serde_json::json!({ "type": "LABEL_DELETED", "label_id": label_id, "board_id": board_id }))
        .await;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Tâches
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct CreateTaskReq {
    /// Identifiant choisi par le client (application hors ligne) : création idempotente.
    pub id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub start_date: Option<DateTime<Utc>>,
    pub due_date: Option<DateTime<Utc>>,
    pub labels: Option<Vec<Uuid>>,
    pub assignees: Option<Vec<String>>,
    pub priority: Option<i32>,
    pub is_per_person: Option<bool>,
    pub require_all_validations: Option<bool>,
    /// Insère la tâche en haut de la colonne plutôt qu'en bas.
    pub at_top: Option<bool>,
    pub event_id: Option<Uuid>,
    pub event_start: Option<DateTime<Utc>>,
}

async fn get_task(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path(task_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    let ctx = task_ctx(&pool, task_id).await?;
    crate::access::require_perm(&pool, &user, ctx.workspace_id, |p| p.boards >= crate::access::level::READ).await?;
    let t = load_task_enriched(&pool, task_id).await?;
    Ok(Json(serde_json::json!({ "task": t, "board_id": ctx.board_id, "workspace_id": ctx.workspace_id })))
}

async fn create_task(
    State(state): State<AppState>,
    user: AuthUser,
    Path(column_id): Path<Uuid>,
    Json(req): Json<CreateTaskReq>,
) -> ApiResult<Json<TaskWithEnrichment>> {
    let pool = &state.pool;
    let ctx = column_ctx(pool, column_id).await?;
    crate::access::require_perm(pool, &user, ctx.workspace_id, |p| p.boards >= crate::access::level::EDIT).await?;
    // Rejeu d'une création faite hors ligne : la tâche existe déjà ?
    if let Some(id) = req.id {
        if let Ok(existing) = task_ctx(pool, id).await {
            return if existing.workspace_id == ctx.workspace_id { Ok(Json(load_task_enriched(pool, id).await?)) } else { Err(ApiError::Conflict("id_taken".into())) };
        }
    }
    let title = check_name(&req.title, 255)?;
    let priority = check_priority(req.priority.unwrap_or(0))?;

    let position: f64 = if req.at_top.unwrap_or(false) {
        sqlx::query_scalar("SELECT COALESCE(MIN(position), 131072) / 2 FROM tasks WHERE column_id = $1")
            .bind(column_id)
            .fetch_one(pool)
            .await?
    } else {
        next_task_position(pool, column_id).await?
    };

    let mut tx = pool.begin().await?;
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO tasks (id, column_id, title, description, position, start_date, due_date, priority, is_per_person, require_all_validations, event_id, event_start)
         VALUES (COALESCE($10, gen_random_uuid()), $1, $2, $3, $4, $5, $6, $7, COALESCE($8, false), COALESCE($9, true),
                 (SELECT id FROM workspace_events WHERE id = $11 AND workspace_id = $13), CASE WHEN $11 IS NULL THEN NULL ELSE $12 END) RETURNING id",
    )
    .bind(column_id)
    .bind(&title)
    .bind(&req.description)
    .bind(position)
    .bind(req.start_date)
    .bind(req.due_date)
    .bind(priority)
    .bind(req.is_per_person)
    .bind(req.require_all_validations)
    .bind(req.id)
    .bind(req.event_id)
    .bind(req.event_start)
    .bind(ctx.workspace_id)
    .fetch_one(&mut *tx)
    .await?;

    for label_id in req.labels.iter().flatten() {
        // Seuls les labels du même tableau sont acceptés.
        sqlx::query(
            "INSERT INTO task_labels (task_id, label_id)
             SELECT $1, id FROM labels WHERE id = $2 AND board_id = $3 ON CONFLICT DO NOTHING",
        )
        .bind(task_id)
        .bind(label_id)
        .bind(ctx.board_id)
        .execute(&mut *tx)
        .await?;
    }
    let mut assigned = Vec::new();
    for uid in req.assignees.iter().flatten() {
        let n = sqlx::query(
            "INSERT INTO task_assignees (task_id, user_id)
             SELECT $1, $2 WHERE EXISTS(SELECT 1 FROM workspace_members WHERE workspace_id = $3 AND user_id = $2)
             ON CONFLICT DO NOTHING",
        )
        .bind(task_id)
        .bind(uid)
        .bind(ctx.workspace_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if n > 0 {
            assigned.push(uid.clone());
        }
    }
    tx.commit().await?;

    log_activity(pool, task_id, user.id, "create", None, Some(serde_json::json!({ "title": title }))).await;
    for uid in assigned {
        if let Ok(target) = Uuid::parse_str(&uid) {
            crate::notifications::notify(
                pool,
                &state.hub,
                ctx.workspace_id,
                target,
                Some(user.id),
                "assigned",
                &format!("task:{task_id}"),
                serde_json::json!({ "title": title, "board_id": ctx.board_id }),
            )
            .await;
        }
    }

    let t = load_task_enriched(pool, task_id).await?;
    state.emit(ctx.workspace_id, serde_json::json!({ "type": "TASK_CREATED", "task": &t })).await;
    Ok(Json(t))
}

#[derive(Deserialize)]
pub struct UpdateTaskReq {
    pub title: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    pub description: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub start_date: Option<Option<DateTime<Utc>>>,
    #[serde(default, deserialize_with = "double_option")]
    pub due_date: Option<Option<DateTime<Utc>>>,
    pub is_per_person: Option<bool>,
    pub priority: Option<i32>,
    pub require_all_validations: Option<bool>,
    /// Uniquement pour les tâches sans validation par personne.
    pub is_completed: Option<bool>,
    #[serde(default, deserialize_with = "double_option")]
    pub event_id: Option<Option<Uuid>>,
    #[serde(default, deserialize_with = "double_option")]
    pub event_start: Option<Option<DateTime<Utc>>>,
}

async fn update_task(
    State(state): State<AppState>,
    user: AuthUser,
    Path(task_id): Path<Uuid>,
    Json(req): Json<UpdateTaskReq>,
) -> ApiResult<Json<TaskWithEnrichment>> {
    let pool = &state.pool;
    let ctx = task_ctx(pool, task_id).await?;
    crate::access::require_perm(pool, &user, ctx.workspace_id, |p| p.boards >= crate::access::level::EDIT).await?;
    let old = fetch_task(pool, task_id).await?;

    let title = req.title.as_deref().map(|t| check_name(t, 255)).transpose()?;
    let priority = req.priority.map(check_priority).transpose()?;
    if req.is_completed.is_some() && old.is_per_person && req.is_per_person != Some(false) {
        return Err(ApiError::bad_request("use_toggle_validation"));
    }

    let task: Task = sqlx::query_as(&format!(
        "UPDATE tasks SET
            title = COALESCE($1, title),
            description = CASE WHEN $2 THEN $3 ELSE description END,
            start_date = CASE WHEN $4 THEN $5 ELSE start_date END,
            due_date = CASE WHEN $6 THEN $7 ELSE due_date END,
            is_per_person = COALESCE($8, is_per_person),
            priority = COALESCE($9, priority),
            require_all_validations = COALESCE($10, require_all_validations),
            is_completed = COALESCE($11, is_completed),
            event_id = CASE WHEN $13 THEN (SELECT id FROM workspace_events WHERE id = $14 AND workspace_id = $17) ELSE event_id END,
            event_start = CASE WHEN $15 THEN $16 WHEN $13 AND $14 IS NULL THEN NULL ELSE event_start END,
            updated_at = NOW()
         WHERE id = $12 RETURNING {TASK_COLUMNS}"
    ))
    .bind(title)
    .bind(req.description.is_some())
    .bind(req.description.clone().flatten())
    .bind(req.start_date.is_some())
    .bind(req.start_date.flatten())
    .bind(req.due_date.is_some())
    .bind(req.due_date.flatten())
    .bind(req.is_per_person)
    .bind(priority)
    .bind(req.require_all_validations)
    .bind(req.is_completed)
    .bind(task_id)
    .bind(req.event_id.is_some())
    .bind(req.event_id.flatten())
    .bind(req.event_start.is_some())
    .bind(req.event_start.flatten())
    .bind(ctx.workspace_id)
    .fetch_one(pool)
    .await?;

    if old.title != task.title {
        log_activity(pool, task_id, user.id, "update_title",
            Some(serde_json::json!({ "title": old.title })), Some(serde_json::json!({ "title": task.title }))).await;
    }
    if old.description != task.description {
        log_activity(pool, task_id, user.id, "update_description",
            Some(serde_json::json!({ "description": old.description })),
            Some(serde_json::json!({ "description": task.description }))).await;
    }
    if old.priority != task.priority {
        log_activity(pool, task_id, user.id, "update_priority",
            Some(serde_json::json!({ "priority": old.priority })), Some(serde_json::json!({ "priority": task.priority }))).await;
    }
    if old.due_date != task.due_date {
        log_activity(pool, task_id, user.id, "update_due_date",
            Some(serde_json::json!({ "due_date": old.due_date })), Some(serde_json::json!({ "due_date": task.due_date }))).await;
    }
    if old.is_completed != task.is_completed {
        log_activity(pool, task_id, user.id, "toggle_validation",
            Some(serde_json::json!({ "is_completed": old.is_completed })),
            Some(serde_json::json!({ "is_completed": task.is_completed }))).await;
        apply_completion_automation(pool, ctx.board_id, &task).await?;
        if task.is_completed {
            notify_completed(&state, task_id, user.id).await;
        }
    }

    let t = load_task_enriched(pool, task_id).await?;
    state.emit(ctx.workspace_id, serde_json::json!({ "type": "TASK_UPDATED", "task": &t })).await;
    Ok(Json(t))
}

async fn delete_task(
    State(state): State<AppState>,
    user: AuthUser,
    Path(task_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let ctx = task_ctx(&state.pool, task_id).await?;
    crate::access::require_perm(&state.pool, &user, ctx.workspace_id, |p| p.boards >= crate::access::level::EDIT).await?;
    crate::trash::trash_task(&state, task_id, Some(user.id)).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Supprime une tâche (corbeille de l'espace) sans contrôle d'accès : l'appelant l'a fait.
pub async fn remove_task(state: &AppState, task_id: Uuid) -> ApiResult<()> {
    crate::trash::trash_task(state, task_id, None).await
}

#[derive(Deserialize)]
pub struct MoveTaskReq {
    pub column_id: Uuid,
    pub position: f64,
}

async fn move_task(
    State(state): State<AppState>,
    user: AuthUser,
    Path(task_id): Path<Uuid>,
    Json(req): Json<MoveTaskReq>,
) -> ApiResult<Json<TaskWithEnrichment>> {
    let pool = &state.pool;
    let ctx = task_ctx(pool, task_id).await?;
    crate::access::require_perm(pool, &user, ctx.workspace_id, |p| p.boards >= crate::access::level::EDIT).await?;
    let dest = column_ctx(pool, req.column_id).await?;
    if dest.workspace_id != ctx.workspace_id {
        return Err(ApiError::bad_request("invalid_column"));
    }
    if !req.position.is_finite() {
        return Err(ApiError::bad_request("invalid_position"));
    }
    let old = fetch_task(pool, task_id).await?;

    // Les sous-tâches suivent leur parente.
    sqlx::query(
        "WITH RECURSIVE sub AS (SELECT id FROM tasks WHERE id = $3 UNION ALL SELECT t.id FROM tasks t JOIN sub ON t.parent_id = sub.id)
         UPDATE tasks SET column_id = $1, position = CASE WHEN id = $3 THEN $2 ELSE position END, updated_at = NOW()
         WHERE id IN (SELECT id FROM sub)",
    )
    .bind(req.column_id)
    .bind(req.position)
    .bind(task_id)
    .execute(pool)
    .await?;

    if old.column_id != req.column_id {
        log_activity(pool, task_id, user.id, "move_column",
            Some(serde_json::json!({ "column_id": old.column_id })), Some(serde_json::json!({ "column_id": req.column_id }))).await;

        // Validation automatique à l'entrée / la sortie de la colonne « terminé ».
        let auto = board_automations(pool, dest.board_id).await?;
        if auto.auto_validate_on_move {
            if let Some(done) = auto.done_column() {
                if req.column_id == done && !old.is_completed {
                    sqlx::query("UPDATE tasks SET is_completed = true, previous_column_id = $2 WHERE id = $1")
                        .bind(task_id)
                        .bind(old.column_id)
                        .execute(pool)
                        .await?;
                    notify_completed(&state, task_id, user.id).await;
                } else if old.column_id == done && req.column_id != done && old.is_completed {
                    sqlx::query("UPDATE tasks SET is_completed = false, previous_column_id = NULL WHERE id = $1")
                        .bind(task_id)
                        .execute(pool)
                        .await?;
                    if old.is_per_person {
                        sqlx::query("DELETE FROM task_validations WHERE task_id = $1").bind(task_id).execute(pool).await?;
                    }
                }
            }
        }
    }

    let t = load_task_enriched(pool, task_id).await?;
    state.emit(ctx.workspace_id, serde_json::json!({ "type": "TASK_MOVED", "task": &t, "from_column_id": old.column_id })).await;
    Ok(Json(t))
}

async fn add_task_label(
    State(state): State<AppState>,
    user: AuthUser,
    Path((task_id, label_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    let ctx = task_ctx(&state.pool, task_id).await?;
    crate::access::require_perm(&state.pool, &user, ctx.workspace_id, |p| p.boards >= crate::access::level::EDIT).await?;
    let (_, label_board) = label_ctx(&state.pool, label_id).await?;
    if label_board != ctx.board_id {
        return Err(ApiError::bad_request("label_not_in_board"));
    }
    sqlx::query("INSERT INTO task_labels (task_id, label_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
        .bind(task_id)
        .bind(label_id)
        .execute(&state.pool)
        .await?;
    log_activity(&state.pool, task_id, user.id, "add_label", None, Some(serde_json::json!({ "label_id": label_id }))).await;
    emit_task(&state, ctx.workspace_id, "TASK_UPDATED", task_id).await;
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_task_label(
    State(state): State<AppState>,
    user: AuthUser,
    Path((task_id, label_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    let ctx = task_ctx(&state.pool, task_id).await?;
    crate::access::require_perm(&state.pool, &user, ctx.workspace_id, |p| p.boards >= crate::access::level::EDIT).await?;
    sqlx::query("DELETE FROM task_labels WHERE task_id = $1 AND label_id = $2")
        .bind(task_id)
        .bind(label_id)
        .execute(&state.pool)
        .await?;
    log_activity(&state.pool, task_id, user.id, "remove_label", Some(serde_json::json!({ "label_id": label_id })), None).await;
    emit_task(&state, ctx.workspace_id, "TASK_UPDATED", task_id).await;
    Ok(StatusCode::NO_CONTENT)
}

async fn add_task_assignee(
    State(state): State<AppState>,
    user: AuthUser,
    Path((task_id, target_user_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    let ctx = task_ctx(&state.pool, task_id).await?;
    crate::access::require_perm(&state.pool, &user, ctx.workspace_id, |p| p.boards >= crate::access::level::EDIT).await?;
    let target = target_user_id.to_string();
    if !is_member(&state.pool, ctx.workspace_id, &target).await? {
        return Err(ApiError::bad_request("not_a_member"));
    }
    let n = sqlx::query("INSERT INTO task_assignees (task_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
        .bind(task_id)
        .bind(&target)
        .execute(&state.pool)
        .await?
        .rows_affected();
    if n > 0 {
        log_activity(&state.pool, task_id, user.id, "add_assignee", None, Some(serde_json::json!({ "user_id": target }))).await;
        let title: String = sqlx::query_scalar("SELECT title FROM tasks WHERE id = $1").bind(task_id).fetch_one(&state.pool).await?;
        crate::notifications::notify(
            &state.pool,
            &state.hub,
            ctx.workspace_id,
            target_user_id,
            Some(user.id),
            "assigned",
            &format!("task:{task_id}"),
            serde_json::json!({ "title": title, "board_id": ctx.board_id }),
        )
        .await;
    }
    emit_task(&state, ctx.workspace_id, "TASK_UPDATED", task_id).await;
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_task_assignee(
    State(state): State<AppState>,
    user: AuthUser,
    Path((task_id, target_user_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    let ctx = task_ctx(&state.pool, task_id).await?;
    crate::access::require_perm(&state.pool, &user, ctx.workspace_id, |p| p.boards >= crate::access::level::EDIT).await?;
    let target = target_user_id.to_string();
    sqlx::query("DELETE FROM task_assignees WHERE task_id = $1 AND user_id = $2")
        .bind(task_id)
        .bind(&target)
        .execute(&state.pool)
        .await?;
    log_activity(&state.pool, task_id, user.id, "remove_assignee", Some(serde_json::json!({ "user_id": target })), None).await;
    emit_task(&state, ctx.workspace_id, "TASK_UPDATED", task_id).await;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct CreateSubtaskReq {
    pub title: String,
}

async fn create_subtask(
    State(state): State<AppState>,
    user: AuthUser,
    Path(parent_id): Path<Uuid>,
    Json(req): Json<CreateSubtaskReq>,
) -> ApiResult<Json<TaskWithEnrichment>> {
    let pool = &state.pool;
    let ctx = task_ctx(pool, parent_id).await?;
    crate::access::require_perm(pool, &user, ctx.workspace_id, |p| p.boards >= crate::access::level::EDIT).await?;
    let title = check_name(&req.title, 255)?;
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO tasks (column_id, parent_id, title, position, priority)
         VALUES ($1, $2, $3, (SELECT COALESCE(MAX(position), 0) + 65536 FROM tasks WHERE parent_id = $2), 0)
         RETURNING id",
    )
    .bind(ctx.column_id)
    .bind(parent_id)
    .bind(title)
    .fetch_one(pool)
    .await?;
    let t = load_task_enriched(pool, id).await?;
    state.emit(ctx.workspace_id, serde_json::json!({ "type": "TASK_CREATED", "task": &t })).await;
    Ok(Json(t))
}

async fn toggle_task_validation(
    State(state): State<AppState>,
    user: AuthUser,
    Path(task_id): Path<Uuid>,
) -> ApiResult<Json<TaskWithEnrichment>> {
    let pool = &state.pool;
    let ctx = task_ctx(pool, task_id).await?;
    crate::access::require_perm(pool, &user, ctx.workspace_id, |p| p.boards >= crate::access::level::VALIDATE).await?;
    let mut task = fetch_task(pool, task_id).await?;
    let was_completed = task.is_completed;

    // On ne peut pas terminer une tâche dont des sous-tâches sont ouvertes.
    if !was_completed {
        let open_subtasks: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tasks WHERE parent_id = $1 AND is_completed = false)")
                .bind(task_id)
                .fetch_one(pool)
                .await?;
        if open_subtasks && !task.is_per_person {
            return Err(ApiError::Conflict("open_subtasks".into()));
        }
    }

    let me = user.id.to_string();
    if task.is_per_person {
        let removed = sqlx::query("DELETE FROM task_validations WHERE task_id = $1 AND user_id = $2")
            .bind(task_id)
            .bind(&me)
            .execute(pool)
            .await?
            .rows_affected();
        if removed == 0 {
            sqlx::query("INSERT INTO task_validations (task_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
                .bind(task_id)
                .bind(&me)
                .execute(pool)
                .await?;
        }
        // Membres qui ne participent pas aux validations collectives : leur validation n'est
        // pas attendue (la tâche reste ouverte pour eux seuls, voir doneFor côté client).
        let assignees: Vec<String> = sqlx::query_scalar(
            "SELECT a.user_id FROM task_assignees a
             JOIN tasks t ON t.id = a.task_id JOIN board_columns c ON c.id = t.column_id JOIN boards b ON b.id = c.board_id
             LEFT JOIN workspace_members m ON m.workspace_id = b.workspace_id AND m.user_id = a.user_id
             WHERE a.task_id = $1 AND COALESCE(m.group_tasks, TRUE)")
            .bind(task_id)
            .fetch_all(pool)
            .await?;
        let validations: Vec<String> = sqlx::query_scalar("SELECT user_id FROM task_validations WHERE task_id = $1")
            .bind(task_id)
            .fetch_all(pool)
            .await?;
        task.is_completed = if task.require_all_validations && !assignees.is_empty() {
            assignees.iter().all(|a| validations.contains(a))
        } else {
            !validations.is_empty()
        };
    } else {
        task.is_completed = !task.is_completed;
    }
    sqlx::query("UPDATE tasks SET is_completed = $1, updated_at = NOW() WHERE id = $2")
        .bind(task.is_completed)
        .bind(task_id)
        .execute(pool)
        .await?;

    if task.is_completed != was_completed {
        log_activity(pool, task_id, user.id, "toggle_validation",
            Some(serde_json::json!({ "is_completed": was_completed })),
            Some(serde_json::json!({ "is_completed": task.is_completed }))).await;
        apply_completion_automation(pool, ctx.board_id, &task).await?;
        if task.is_completed {
            notify_completed(&state, task_id, user.id).await;
        }
    }

    let t = load_task_enriched(pool, task_id).await?;
    state.emit(ctx.workspace_id, serde_json::json!({ "type": "TASK_UPDATED", "task": &t })).await;
    Ok(Json(t))
}

// ---------------------------------------------------------------------------
// Pièces jointes & activité
// ---------------------------------------------------------------------------

async fn get_task_attachments(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path(task_id): Path<Uuid>,
) -> ApiResult<Json<Vec<TaskAttachment>>> {
    let ctx = task_ctx(&pool, task_id).await?;
    require_member(&pool, &user, ctx.workspace_id).await?;
    Ok(Json(
        sqlx::query_as(
            "SELECT id, task_id, uploaded_by, filename, file_path, mime_type, size_bytes, created_at
             FROM task_attachments WHERE task_id = $1 ORDER BY created_at DESC",
        )
        .bind(task_id)
        .fetch_all(&pool)
        .await?,
    ))
}

async fn upload_attachment(
    State(state): State<AppState>,
    user: AuthUser,
    Path(task_id): Path<Uuid>,
    mut multipart: Multipart,
) -> ApiResult<Json<TaskAttachment>> {
    let pool = &state.pool;
    let ctx = task_ctx(pool, task_id).await?;
    crate::access::require_perm(pool, &user, ctx.workspace_id, |p| p.boards >= crate::access::level::EDIT).await?;

    while let Some(field) = multipart.next_field().await.map_err(|e| ApiError::BadRequest(e.to_string()))? {
        if field.name() != Some("file") {
            continue;
        }
        let saved = crate::upload::save_field(field).await?;
        let att: TaskAttachment = sqlx::query_as(
            "INSERT INTO task_attachments (task_id, uploaded_by, filename, file_path, mime_type, size_bytes)
             VALUES ($1, $2, $3, $4, $5, $6)
             RETURNING id, task_id, uploaded_by, filename, file_path, mime_type, size_bytes, created_at",
        )
        .bind(task_id)
        .bind(user.id)
        .bind(&saved.original_name)
        .bind(&saved.path)
        .bind(&saved.mime)
        .bind(saved.size as i64)
        .fetch_one(pool)
        .await?;
        log_activity(pool, task_id, user.id, "upload_attachment", None, Some(serde_json::json!({ "filename": att.filename }))).await;
        state
            .emit(ctx.workspace_id, serde_json::json!({ "type": "TASK_ATTACHMENTS_UPDATED", "task_id": task_id }))
            .await;
        return Ok(Json(att));
    }
    Err(ApiError::bad_request("no_file"))
}

async fn delete_attachment(
    State(state): State<AppState>,
    user: AuthUser,
    Path((task_id, attachment_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    let ctx = task_ctx(&state.pool, task_id).await?;
    crate::access::require_perm(&state.pool, &user, ctx.workspace_id, |p| p.boards >= crate::access::level::EDIT).await?;
    let path: String = sqlx::query_scalar("DELETE FROM task_attachments WHERE id = $1 AND task_id = $2 RETURNING file_path")
        .bind(attachment_id)
        .bind(task_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(ApiError::NotFound)?;
    crate::upload::remove_upload(&path).await;
    state
        .emit(ctx.workspace_id, serde_json::json!({ "type": "TASK_ATTACHMENTS_UPDATED", "task_id": task_id }))
        .await;
    Ok(StatusCode::NO_CONTENT)
}

async fn get_task_activities(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path(task_id): Path<Uuid>,
) -> ApiResult<Json<Vec<TaskActivity>>> {
    let ctx = task_ctx(&pool, task_id).await?;
    require_member(&pool, &user, ctx.workspace_id).await?;
    Ok(Json(
        sqlx::query_as(
            "SELECT id, task_id, user_id, activity_type, old_value, new_value, created_at
             FROM task_activities WHERE task_id = $1 ORDER BY created_at DESC LIMIT 200",
        )
        .bind(task_id)
        .fetch_all(&pool)
        .await?,
    ))
}

async fn restore_task_activity(
    State(state): State<AppState>,
    user: AuthUser,
    Path((task_id, activity_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<TaskWithEnrichment>> {
    let pool = &state.pool;
    let ctx = task_ctx(pool, task_id).await?;
    crate::access::require_perm(pool, &user, ctx.workspace_id, |p| p.boards >= crate::access::level::EDIT).await?;
    let activity: TaskActivity = sqlx::query_as(
        "SELECT id, task_id, user_id, activity_type, old_value, new_value, created_at
         FROM task_activities WHERE id = $1 AND task_id = $2",
    )
    .bind(activity_id)
    .bind(task_id)
    .fetch_optional(pool)
    .await?
    .ok_or(ApiError::NotFound)?;

    let old = activity.old_value.unwrap_or_default();
    let current = fetch_task(pool, task_id).await?;
    match activity.activity_type.as_str() {
        "update_title" => {
            if let Some(title) = old.get("title").and_then(|t| t.as_str()) {
                sqlx::query("UPDATE tasks SET title = $1, updated_at = NOW() WHERE id = $2")
                    .bind(title)
                    .bind(task_id)
                    .execute(pool)
                    .await?;
                log_activity(pool, task_id, user.id, "update_title",
                    Some(serde_json::json!({ "title": current.title })), Some(serde_json::json!({ "title": title }))).await;
            }
        }
        "update_description" => {
            let desc = old.get("description").and_then(|d| d.as_str());
            sqlx::query("UPDATE tasks SET description = $1, updated_at = NOW() WHERE id = $2")
                .bind(desc)
                .bind(task_id)
                .execute(pool)
                .await?;
            log_activity(pool, task_id, user.id, "update_description",
                Some(serde_json::json!({ "description": current.description })),
                Some(serde_json::json!({ "description": desc }))).await;
        }
        _ => return Err(ApiError::bad_request("not_restorable")),
    }
    let t = load_task_enriched(pool, task_id).await?;
    state.emit(ctx.workspace_id, serde_json::json!({ "type": "TASK_UPDATED", "task": &t })).await;
    Ok(Json(t))
}

// ---------------------------------------------------------------------------
// Commentaires
// ---------------------------------------------------------------------------

#[derive(Serialize, sqlx::FromRow)]
pub struct TaskComment {
    pub id: Uuid,
    pub task_id: Uuid,
    pub user_id: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct CommentBody {
    /// Identifiant choisi par le client (application hors ligne) : création idempotente.
    pub id: Option<Uuid>,
    pub content: String,
}

/// Nettoie le HTML produit par l'éditeur (liste blanche stricte).
pub fn sanitize_rich_text(html: &str) -> String {
    ammonia::Builder::default()
        .add_tags(&["mark", "s", "u"])
        .add_generic_attributes(&["data-type", "data-id", "data-label", "data-page-id", "class"])
        .add_tag_attributes("span", &["latex"])
        .add_tag_attributes("div", &["latex"])
        .url_schemes(["http", "https", "mailto"].into_iter().collect())
        .link_rel(Some("noopener noreferrer nofollow"))
        .clean(html)
        .to_string()
}

fn comment_content(raw: &str) -> ApiResult<String> {
    if raw.len() > 100_000 {
        return Err(ApiError::bad_request("comment_too_long"));
    }
    let clean = sanitize_rich_text(raw);
    let text_len = clean.replace("<p></p>", "").trim().len();
    if text_len == 0 {
        return Err(ApiError::bad_request("empty_comment"));
    }
    Ok(clean)
}

/// Extrait les identifiants des mentions (`<span data-type="mention" data-id="...">`).
pub fn extract_mentions(html: &str) -> Vec<Uuid> {
    let mut out = Vec::new();
    for part in html.split("data-id=\"").skip(1) {
        if let Some(end) = part.find('"') {
            if let Ok(id) = Uuid::parse_str(&part[..end]) {
                if !out.contains(&id) {
                    out.push(id);
                }
            }
        }
    }
    out
}

async fn get_task_comments(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path(task_id): Path<Uuid>,
) -> ApiResult<Json<Vec<TaskComment>>> {
    let ctx = task_ctx(&pool, task_id).await?;
    require_member(&pool, &user, ctx.workspace_id).await?;
    Ok(Json(
        sqlx::query_as(
            "SELECT id, task_id, user_id, content, created_at, updated_at FROM task_comments
             WHERE task_id = $1 ORDER BY created_at ASC",
        )
        .bind(task_id)
        .fetch_all(&pool)
        .await?,
    ))
}

async fn add_task_comment(
    State(state): State<AppState>,
    user: AuthUser,
    Path(task_id): Path<Uuid>,
    Json(payload): Json<CommentBody>,
) -> ApiResult<Json<TaskComment>> {
    let pool = &state.pool;
    let ctx = task_ctx(pool, task_id).await?;
    // Les lecteurs peuvent commenter (comme dans Linear), mais doivent être membres.
    crate::access::require_perm(pool, &user, ctx.workspace_id, |p| p.boards >= crate::access::level::COMMENT).await?;
    let content = comment_content(&payload.content)?;
    // Rejeu d'un commentaire écrit hors ligne : déjà enregistré ?
    if let Some(id) = payload.id {
        let existing: Option<TaskComment> = sqlx::query_as("SELECT id, task_id, user_id, content, created_at, updated_at FROM task_comments WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await?;
        if let Some(c) = existing {
            return if c.task_id == task_id && c.user_id == user.id.to_string() { Ok(Json(c)) } else { Err(ApiError::Conflict("id_taken".into())) };
        }
    }
    let comment: TaskComment = sqlx::query_as(
        "INSERT INTO task_comments (id, task_id, user_id, content) VALUES (COALESCE($4, gen_random_uuid()), $1, $2, $3)
         RETURNING id, task_id, user_id, content, created_at, updated_at",
    )
    .bind(task_id)
    .bind(user.id.to_string())
    .bind(&content)
    .bind(payload.id)
    .fetch_one(pool)
    .await?;

    // Notifications : personnes mentionnées + assignés de la tâche.
    let title: String = sqlx::query_scalar("SELECT title FROM tasks WHERE id = $1").bind(task_id).fetch_one(pool).await?;
    let target = format!("task:{task_id}");
    let data = serde_json::json!({ "title": title, "board_id": ctx.board_id, "comment_id": comment.id });
    let mentioned = extract_mentions(&content);
    for m in &mentioned {
        if is_member(pool, ctx.workspace_id, &m.to_string()).await? {
            crate::notifications::notify(pool, &state.hub, ctx.workspace_id, *m, Some(user.id), "mention", &target, data.clone()).await;
        }
    }
    let assignees: Vec<String> = sqlx::query_scalar("SELECT user_id FROM task_assignees WHERE task_id = $1")
        .bind(task_id)
        .fetch_all(pool)
        .await?;
    for a in assignees.iter().filter_map(|a| Uuid::parse_str(a).ok()) {
        if !mentioned.contains(&a) {
            crate::notifications::notify(pool, &state.hub, ctx.workspace_id, a, Some(user.id), "comment", &target, data.clone()).await;
        }
    }

    state
        .emit(ctx.workspace_id, serde_json::json!({ "type": "COMMENT_CREATED", "comment": &comment }))
        .await;
    Ok(Json(comment))
}

async fn edit_task_comment(
    State(state): State<AppState>,
    user: AuthUser,
    Path((task_id, comment_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<CommentBody>,
) -> ApiResult<Json<TaskComment>> {
    let ctx = task_ctx(&state.pool, task_id).await?;
    require_member(&state.pool, &user, ctx.workspace_id).await?;
    let content = comment_content(&payload.content)?;
    // Seul l'auteur peut modifier son commentaire.
    let comment: TaskComment = sqlx::query_as(
        "UPDATE task_comments SET content = $1, updated_at = NOW() WHERE id = $2 AND task_id = $3 AND user_id = $4
         RETURNING id, task_id, user_id, content, created_at, updated_at",
    )
    .bind(content)
    .bind(comment_id)
    .bind(task_id)
    .bind(user.id.to_string())
    .fetch_optional(&state.pool)
    .await?
    .ok_or(ApiError::NotFound)?;
    state
        .emit(ctx.workspace_id, serde_json::json!({ "type": "COMMENT_UPDATED", "comment": &comment }))
        .await;
    Ok(Json(comment))
}

async fn delete_task_comment(
    State(state): State<AppState>,
    user: AuthUser,
    Path((task_id, comment_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    let ctx = task_ctx(&state.pool, task_id).await?;
    let role = require_member(&state.pool, &user, ctx.workspace_id).await?;
    // L'auteur ou un admin du workspace.
    let n = sqlx::query("DELETE FROM task_comments WHERE id = $1 AND task_id = $2 AND (user_id = $3 OR $4)")
        .bind(comment_id)
        .bind(task_id)
        .bind(user.id.to_string())
        .bind(role.is_admin())
        .execute(&state.pool)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(ApiError::NotFound);
    }
    state
        .emit(
            ctx.workspace_id,
            serde_json::json!({ "type": "COMMENT_DELETED", "comment_id": comment_id, "task_id": task_id }),
        )
        .await;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Synchronisation CalDAV : modifications venant d'un calendrier externe
// ---------------------------------------------------------------------------

/// Changements détectés côté calendrier (seuls les champs `Some` sont appliqués).
#[derive(Default, Debug)]
pub struct RemoteChanges {
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub due: Option<Option<DateTime<Utc>>>,
    pub start: Option<Option<DateTime<Utc>>>,
    pub priority: Option<i32>,
    pub completed: Option<bool>,
}

impl RemoteChanges {
    pub fn is_empty(&self) -> bool {
        self.title.is_none() && self.description.is_none() && self.due.is_none() && self.start.is_none()
            && self.priority.is_none() && self.completed.is_none()
    }
}

/// Crée une tâche dans la première colonne d'un tableau depuis un client CalDAV
/// (droits vérifiés ici), puis applique l'état « terminé » éventuel.
pub async fn dav_create_task(state: &AppState, user_id: Uuid, board_id: Uuid, f: &crate::ical::TodoFields) -> ApiResult<Uuid> {
    let pool = &state.pool;
    let workspace_id: Uuid = sqlx::query_scalar("SELECT workspace_id FROM boards WHERE id = $1").bind(board_id).fetch_optional(pool).await?.ok_or(ApiError::NotFound)?;
    let role = crate::access::workspace_role(pool, user_id, workspace_id).await?.ok_or(ApiError::Forbidden)?;
    if !role.can_write() {
        return Err(ApiError::Forbidden);
    }
    let column_id: Uuid = sqlx::query_scalar("SELECT id FROM board_columns WHERE board_id = $1 ORDER BY position ASC LIMIT 1")
        .bind(board_id).fetch_optional(pool).await?.ok_or_else(|| ApiError::bad_request("board_has_no_column"))?;
    let title = check_name(if f.summary.trim().is_empty() { "Untitled" } else { f.summary.trim() }, 255)?;
    let description = crate::ical::text_to_html(&f.description);
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO tasks (column_id, title, description, position, start_date, due_date, priority) VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
    )
    .bind(column_id)
    .bind(&title)
    .bind(if description.is_empty() { None } else { Some(description) })
    .bind(next_task_position(pool, column_id).await?)
    .bind(f.start)
    .bind(f.due)
    .bind(check_priority(f.priority).unwrap_or(0))
    .fetch_one(pool)
    .await?;
    log_activity(pool, task_id, user_id, "create", None, Some(serde_json::json!({ "title": title }))).await;
    let t = load_task_enriched(pool, task_id).await?;
    state.emit(workspace_id, serde_json::json!({ "type": "TASK_CREATED", "task": &t })).await;
    if f.completed {
        sync_apply_remote(state, task_id, user_id, RemoteChanges { completed: Some(true), ..Default::default() }).await?;
    }
    Ok(task_id)
}

/// Retire `user_id` des personnes assignées (suppression depuis le calendrier « Assignées »).
pub async fn dav_unassign(state: &AppState, task_id: Uuid, user_id: Uuid) -> ApiResult<()> {
    let pool = &state.pool;
    let ctx = task_ctx(pool, task_id).await?;
    crate::access::workspace_role(pool, user_id, ctx.workspace_id).await?.ok_or(ApiError::Forbidden)?;
    sqlx::query("DELETE FROM task_assignees WHERE task_id = $1 AND user_id = $2").bind(task_id).bind(user_id.to_string()).execute(pool).await?;
    sqlx::query("UPDATE tasks SET updated_at = NOW() WHERE id = $1").bind(task_id).execute(pool).await?;
    let t = load_task_enriched(pool, task_id).await?;
    state.emit(ctx.workspace_id, serde_json::json!({ "type": "TASK_UPDATED", "task": &t })).await;
    Ok(())
}

/// Applique des modifications faites dans le calendrier de `user_id`, avec les mêmes
/// règles que l'interface : droits, historique, validation par personne, automatisations.
pub async fn sync_apply_remote(state: &AppState, task_id: Uuid, user_id: Uuid, ch: RemoteChanges) -> ApiResult<()> {
    if ch.is_empty() {
        return Ok(());
    }
    let pool = &state.pool;
    let ctx = task_ctx(pool, task_id).await?;
    let role = crate::access::workspace_role(pool, user_id, ctx.workspace_id).await?.ok_or(ApiError::Forbidden)?;
    if !role.can_write() {
        return Err(ApiError::Forbidden);
    }
    let old = fetch_task(pool, task_id).await?;
    let title = ch.title.as_deref().map(|t| check_name(t, 255)).transpose().ok().flatten();
    let priority = ch.priority.and_then(|p| check_priority(p).ok());

    sqlx::query(
        "UPDATE tasks SET
            title = COALESCE($1, title),
            description = CASE WHEN $2 THEN $3 ELSE description END,
            due_date = CASE WHEN $4 THEN $5 ELSE due_date END,
            start_date = CASE WHEN $6 THEN $7 ELSE start_date END,
            priority = COALESCE($8, priority),
            updated_at = NOW()
         WHERE id = $9",
    )
    .bind(&title)
    .bind(ch.description.is_some())
    .bind(ch.description.clone().flatten())
    .bind(ch.due.is_some())
    .bind(ch.due.flatten())
    .bind(ch.start.is_some())
    .bind(ch.start.flatten())
    .bind(priority)
    .bind(task_id)
    .execute(pool)
    .await?;

    if let Some(t) = &title {
        if *t != old.title {
            log_activity(pool, task_id, user_id, "update_title", Some(serde_json::json!({ "title": old.title })), Some(serde_json::json!({ "title": t }))).await;
        }
    }
    if ch.description.is_some() {
        log_activity(pool, task_id, user_id, "update_description", Some(serde_json::json!({ "description": old.description })), Some(serde_json::json!({ "description": ch.description.clone().flatten() }))).await;
    }
    if let Some(d) = ch.due {
        if d != old.due_date {
            log_activity(pool, task_id, user_id, "update_due_date", Some(serde_json::json!({ "due_date": old.due_date })), Some(serde_json::json!({ "due_date": d }))).await;
        }
    }
    if let Some(p) = priority {
        if p != old.priority {
            log_activity(pool, task_id, user_id, "update_priority", Some(serde_json::json!({ "priority": old.priority })), Some(serde_json::json!({ "priority": p }))).await;
        }
    }

    if let Some(done) = ch.completed {
        let mut task = fetch_task(pool, task_id).await?;
        let was = task.is_completed;
        if task.is_per_person {
            let me = user_id.to_string();
            if done {
                sqlx::query("INSERT INTO task_validations (task_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
                    .bind(task_id).bind(&me).execute(pool).await?;
            } else {
                sqlx::query("DELETE FROM task_validations WHERE task_id = $1 AND user_id = $2")
                    .bind(task_id).bind(&me).execute(pool).await?;
            }
            // Membres qui ne participent pas aux validations collectives : leur validation n'est
        // pas attendue (la tâche reste ouverte pour eux seuls, voir doneFor côté client).
        let assignees: Vec<String> = sqlx::query_scalar(
            "SELECT a.user_id FROM task_assignees a
             JOIN tasks t ON t.id = a.task_id JOIN board_columns c ON c.id = t.column_id JOIN boards b ON b.id = c.board_id
             LEFT JOIN workspace_members m ON m.workspace_id = b.workspace_id AND m.user_id = a.user_id
             WHERE a.task_id = $1 AND COALESCE(m.group_tasks, TRUE)").bind(task_id).fetch_all(pool).await?;
            let validations: Vec<String> = sqlx::query_scalar("SELECT user_id FROM task_validations WHERE task_id = $1").bind(task_id).fetch_all(pool).await?;
            task.is_completed = if task.require_all_validations && !assignees.is_empty() {
                assignees.iter().all(|a| validations.contains(a))
            } else {
                !validations.is_empty()
            };
        } else {
            task.is_completed = done;
        }
        if task.is_completed != was {
            sqlx::query("UPDATE tasks SET is_completed = $1, updated_at = NOW() WHERE id = $2").bind(task.is_completed).bind(task_id).execute(pool).await?;
            log_activity(pool, task_id, user_id, "toggle_validation", Some(serde_json::json!({ "is_completed": was })), Some(serde_json::json!({ "is_completed": task.is_completed }))).await;
            apply_completion_automation(pool, ctx.board_id, &task).await?;
            if task.is_completed {
                notify_completed(state, task_id, user_id).await;
            }
        }
    }

    let t = load_task_enriched(pool, task_id).await?;
    state.emit(ctx.workspace_id, serde_json::json!({ "type": "TASK_UPDATED", "task": &t })).await;
    Ok(())
}

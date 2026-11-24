//! Modèles de tableaux et de documents (GLOSSARY : Modèle) : « Nouveau depuis un modèle ».
//!
//! - Des modèles fournis (suivi de projet, rétrospective, compte rendu de réunion, cours, TP), dans la
//!   langue de la personne ; et ceux que les administrateurs d'un espace y enregistrent.
//! - Un modèle de tableau garde les colonnes, les étiquettes et des tâches types (titre, description,
//!   priorité, étiquettes, sous-tâches), jamais les personnes, les dates ni l'avancement.
//! - Un modèle de document garde le titre, l'icône et le contenu.
//! - Les éléments personnels ne deviennent jamais des modèles (ADR 0002). Les modèles de l'espace sont
//!   vus des membres qui peuvent créer ce type d'élément (tableaux : gérer les tableaux ; documents : écrire).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::access::{level, member_perms, require_ws_admin, AuthUser};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

const MAX_PER_WORKSPACE: i64 = 50;
const MAX_TASKS: usize = 200;
const MAX_PAGE_HTML: usize = 200_000;

pub fn routes() -> Router<AppState> {
    Router::new().route("/", get(list).post(create)).route("/{id}", delete(remove))
}

// ---------------------------------------------------------------------------
// Contenu d'un modèle
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
pub struct TaskTpl {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub priority: i32,
    /// Noms d'étiquettes du modèle.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subtasks: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
pub struct ColumnTpl {
    pub name: String,
    /// Colonne où vont les tâches terminées (automatisation du tableau).
    #[serde(default)]
    pub done: bool,
    #[serde(default)]
    pub tasks: Vec<TaskTpl>,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
pub struct LabelTpl {
    pub name: String,
    pub color: String,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
pub struct BoardTpl {
    #[serde(default)]
    pub labels: Vec<LabelTpl>,
    pub columns: Vec<ColumnTpl>,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
pub struct PageTpl {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub content: String,
}

/// Le modèle choisi, prêt à servir.
pub enum Chosen {
    Board(BoardTpl),
    Page(PageTpl),
}

// ---------------------------------------------------------------------------
// Modèles fournis
// ---------------------------------------------------------------------------

fn pick(fr: bool, f: &str, e: &str) -> String {
    (if fr { f } else { e }).to_string()
}

fn task(fr: bool, f: &str, e: &str, priority: i32, labels: &[&str]) -> TaskTpl {
    TaskTpl { title: pick(fr, f, e), priority, labels: labels.iter().map(|s| s.to_string()).collect(), ..Default::default() }
}

fn col(fr: bool, f: &str, e: &str, done: bool, tasks: Vec<TaskTpl>) -> ColumnTpl {
    ColumnTpl { name: pick(fr, f, e), done, tasks }
}

fn label(fr: bool, f: &str, e: &str, color: &str) -> LabelTpl {
    LabelTpl { name: pick(fr, f, e), color: color.to_string() }
}

/// (identifiant, genre, nom) des modèles fournis.
fn builtin_list(fr: bool) -> Vec<(&'static str, &'static str, String)> {
    vec![
        ("project-tracking", "board", pick(fr, "Suivi de projet", "Project tracking")),
        ("retrospective", "board", pick(fr, "Rétrospective", "Retrospective")),
        ("meeting-notes", "page", pick(fr, "Compte rendu de réunion", "Meeting notes")),
        ("course-notes", "page", pick(fr, "Notes de cours", "Course notes")),
        ("lab-report", "page", pick(fr, "Compte rendu de TP", "Lab report")),
    ]
}

fn builtin(id: &str, fr: bool) -> Option<Chosen> {
    Some(match id {
        "project-tracking" => {
            let feature = pick(fr, "Fonctionnalité", "Feature");
            Chosen::Board(BoardTpl {
                labels: vec![label(fr, "Bug", "Bug", "#ef4444"), label(fr, "Fonctionnalité", "Feature", "#3b82f6"), label(fr, "Amélioration", "Improvement", "#22c55e")],
                columns: vec![
                    col(fr, "Backlog", "Backlog", false, vec![task(fr, "Lister les idées et les besoins", "List ideas and needs", 0, &[])]),
                    col(fr, "À faire", "To do", false, vec![task(fr, "Définir les objectifs du projet", "Define the project goals", 3, &[feature.as_str()]), task(fr, "Découper le travail en tâches", "Break the work into tasks", 2, &[])]),
                    col(fr, "En cours", "In progress", false, vec![]),
                    col(fr, "En revue", "In review", false, vec![]),
                    col(fr, "Terminé", "Done", true, vec![]),
                ],
            })
        }
        "retrospective" => Chosen::Board(BoardTpl {
            labels: vec![],
            columns: vec![
                col(fr, "Ce qui a bien marché", "What went well", false, vec![task(fr, "Exemple : une réussite à répéter", "Example: a win to repeat", 0, &[])]),
                col(fr, "À améliorer", "What to improve", false, vec![task(fr, "Exemple : un blocage rencontré", "Example: a blocker we hit", 0, &[])]),
                col(fr, "Idées", "Ideas", false, vec![]),
                col(fr, "Actions", "Actions", false, vec![task(fr, "Exemple : une action, une personne, une date", "Example: one action, one owner, one date", 2, &[])]),
                col(fr, "Fait", "Done", true, vec![]),
            ],
        }),
        "meeting-notes" => Chosen::Page(PageTpl {
            title: pick(fr, "Compte rendu de réunion", "Meeting notes"),
            icon: Some("📝".into()),
            content: if fr {
                "<p><strong>Date :</strong> </p><p><strong>Participants :</strong> </p><h2>Ordre du jour</h2><ul><li><p></p></li></ul><h2>Décisions</h2><ul><li><p></p></li></ul><h2>Actions</h2><ul><li><p>Qui : quoi, pour quand</p></li></ul><h2>Notes</h2><p></p>"
            } else {
                "<p><strong>Date:</strong> </p><p><strong>Attendees:</strong> </p><h2>Agenda</h2><ul><li><p></p></li></ul><h2>Decisions</h2><ul><li><p></p></li></ul><h2>Action items</h2><ul><li><p>Who: what, by when</p></li></ul><h2>Notes</h2><p></p>"
            }
            .into(),
        }),
        "course-notes" => Chosen::Page(PageTpl {
            title: pick(fr, "Notes de cours", "Course notes"),
            icon: Some("🎓".into()),
            content: if fr {
                "<p><strong>Matière :</strong> </p><p><strong>Date :</strong> </p><h2>Objectifs</h2><ul><li><p></p></li></ul><h2>Notions clés</h2><ul><li><p></p></li></ul><h2>Définitions et formules</h2><p></p><h2>Exemples</h2><p></p><h2>Questions à revoir</h2><ul><li><p></p></li></ul>"
            } else {
                "<p><strong>Subject:</strong> </p><p><strong>Date:</strong> </p><h2>Goals</h2><ul><li><p></p></li></ul><h2>Key ideas</h2><ul><li><p></p></li></ul><h2>Definitions and formulas</h2><p></p><h2>Examples</h2><p></p><h2>Questions to review</h2><ul><li><p></p></li></ul>"
            }
            .into(),
        }),
        "lab-report" => Chosen::Page(PageTpl {
            title: pick(fr, "Compte rendu de TP", "Lab report"),
            icon: Some("🧪".into()),
            content: if fr {
                "<p><strong>TP :</strong> </p><p><strong>Binôme :</strong> </p><h2>Objectif</h2><p></p><h2>Matériel et méthode</h2><p></p><h2>Résultats</h2><p></p><h2>Analyse</h2><p></p><h2>Conclusion</h2><p></p>"
            } else {
                "<p><strong>Lab:</strong> </p><p><strong>Partner:</strong> </p><h2>Goal</h2><p></p><h2>Materials and method</h2><p></p><h2>Results</h2><p></p><h2>Analysis</h2><p></p><h2>Conclusion</h2><p></p>"
            }
            .into(),
        }),
        _ => return None,
    })
}

async fn is_french(pool: &sqlx::PgPool, user: Uuid) -> bool {
    let locale: Option<String> = sqlx::query_scalar("SELECT locale FROM users WHERE id = $1").bind(user).fetch_optional(pool).await.ok().flatten().flatten();
    locale.as_deref().map(|l| l.starts_with("fr")).unwrap_or(true)
}

// ---------------------------------------------------------------------------
// Lecture et application
// ---------------------------------------------------------------------------

/// Le modèle demandé à la création (identifiant d'un modèle de l'espace, ou nom d'un modèle fourni),
/// du genre attendu. Même réponse pour un modèle inconnu ou d'un autre espace.
pub async fn resolve(state: &AppState, user: Uuid, ws: Uuid, id: &str, kind: &str) -> ApiResult<Chosen> {
    if let Ok(uuid) = Uuid::parse_str(id) {
        let row: Option<(String, serde_json::Value)> =
            sqlx::query_as("SELECT kind, data FROM workspace_templates WHERE id = $1 AND workspace_id = $2").bind(uuid).bind(ws).fetch_optional(&state.pool).await?;
        let (k, data) = row.ok_or(ApiError::NotFound)?;
        if k != kind {
            return Err(ApiError::bad_request("template_wrong_kind"));
        }
        return match kind {
            "board" => Ok(Chosen::Board(serde_json::from_value(data).map_err(|_| ApiError::bad_request("template_invalid"))?)),
            _ => Ok(Chosen::Page(serde_json::from_value(data).map_err(|_| ApiError::bad_request("template_invalid"))?)),
        };
    }
    let fr = is_french(&state.pool, user).await;
    let found = builtin(id, fr).ok_or(ApiError::NotFound)?;
    match (&found, kind) {
        (Chosen::Board(_), "board") | (Chosen::Page(_), "page") => Ok(found),
        _ => Err(ApiError::bad_request("template_wrong_kind")),
    }
}

/// Crée colonnes, étiquettes et tâches dans un tableau tout neuf ; renvoie la colonne « terminé », s'il y en a une.
pub async fn apply_board(conn: &mut PgConnection, board: Uuid, tpl: &BoardTpl) -> ApiResult<Option<Uuid>> {
    let mut label_ids: std::collections::HashMap<String, Uuid> = Default::default();
    for l in tpl.labels.iter().take(50) {
        let id: Uuid = sqlx::query_scalar("INSERT INTO labels (board_id, name, color) VALUES ($1, $2, $3) RETURNING id").bind(board).bind(&l.name).bind(&l.color).fetch_one(&mut *conn).await?;
        label_ids.insert(l.name.to_lowercase(), id);
    }
    let mut done = None;
    let mut count = 0;
    for (i, c) in tpl.columns.iter().take(30).enumerate() {
        let col_id: Uuid = sqlx::query_scalar("INSERT INTO board_columns (board_id, name, position) VALUES ($1, $2, $3) RETURNING id")
            .bind(board)
            .bind(&c.name)
            .bind((i as f64 + 1.0) * 65536.0)
            .fetch_one(&mut *conn)
            .await?;
        if c.done && done.is_none() {
            done = Some(col_id);
        }
        for (j, t) in c.tasks.iter().enumerate() {
            count += 1;
            if count > MAX_TASKS {
                break;
            }
            let task_id: Uuid = sqlx::query_scalar("INSERT INTO tasks (column_id, title, description, position, priority) VALUES ($1, $2, $3, $4, $5) RETURNING id")
                .bind(col_id)
                .bind(&t.title)
                .bind(&t.description)
                .bind((j as f64 + 1.0) * 65536.0)
                .bind(t.priority.clamp(0, 4))
                .fetch_one(&mut *conn)
                .await?;
            for name in &t.labels {
                if let Some(l) = label_ids.get(&name.to_lowercase()) {
                    sqlx::query("INSERT INTO task_labels (task_id, label_id) VALUES ($1, $2) ON CONFLICT DO NOTHING").bind(task_id).bind(l).execute(&mut *conn).await?;
                }
            }
            for (k, s) in t.subtasks.iter().take(50).enumerate() {
                sqlx::query("INSERT INTO tasks (column_id, parent_id, title, position, priority) VALUES ($1, $2, $3, $4, 0)")
                    .bind(col_id)
                    .bind(task_id)
                    .bind(s)
                    .bind((k as f64 + 1.0) * 65536.0)
                    .execute(&mut *conn)
                    .await?;
            }
        }
    }
    Ok(done)
}

// ---------------------------------------------------------------------------
// Liste, enregistrement, suppression
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct TemplateDto {
    /// Identifiant d'un modèle de l'espace, ou nom d'un modèle fourni.
    id: String,
    kind: String,
    name: String,
    builtin: bool,
    /// Modèle de tableau : noms des colonnes (aperçu).
    #[serde(skip_serializing_if = "Option::is_none")]
    columns: Option<Vec<String>>,
}

async fn list(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>) -> ApiResult<Json<Vec<TemplateDto>>> {
    let (_, perms) = member_perms(&state.pool, user.id, ws).await?.ok_or(ApiError::NotFound)?;
    let (boards, pages) = (perms.boards >= level::MANAGE, perms.docs >= level::EDIT);
    let fr = is_french(&state.pool, user.id).await;
    let mut out = Vec::new();
    for (id, kind, name) in builtin_list(fr) {
        if (kind == "board" && !boards) || (kind == "page" && !pages) {
            continue;
        }
        let columns = match builtin(id, fr) {
            Some(Chosen::Board(b)) => Some(b.columns.iter().map(|c| c.name.clone()).collect()),
            _ => None,
        };
        out.push(TemplateDto { id: id.into(), kind: kind.into(), name, builtin: true, columns });
    }
    let rows: Vec<(Uuid, String, String, serde_json::Value)> = sqlx::query_as("SELECT id, kind, name, data FROM workspace_templates WHERE workspace_id = $1 ORDER BY name").bind(ws).fetch_all(&state.pool).await?;
    for (id, kind, name, data) in rows {
        if (kind == "board" && !boards) || (kind == "page" && !pages) {
            continue;
        }
        let columns = (kind == "board").then(|| serde_json::from_value::<BoardTpl>(data).map(|b| b.columns.into_iter().map(|c| c.name).collect()).unwrap_or_default());
        out.push(TemplateDto { id: id.to_string(), kind, name, builtin: false, columns });
    }
    Ok(Json(out))
}

#[derive(Deserialize)]
struct CreateReq {
    kind: String,
    source_id: Uuid,
    name: Option<String>,
}

async fn create(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>, Json(p): Json<CreateReq>) -> ApiResult<Json<serde_json::Value>> {
    let pool = &state.pool;
    require_ws_admin(pool, &user, ws).await?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM workspace_templates WHERE workspace_id = $1").bind(ws).fetch_one(pool).await?;
    if count >= MAX_PER_WORKSPACE {
        return Err(ApiError::bad_request("template_too_many"));
    }
    let (default_name, data) = match p.kind.as_str() {
        "board" => {
            let row: Option<(String, Option<Uuid>, serde_json::Value)> = sqlx::query_as("SELECT name, private_to, automations FROM boards WHERE id = $1 AND workspace_id = $2").bind(p.source_id).bind(ws).fetch_optional(pool).await?;
            let (name, private, automations) = row.ok_or(ApiError::NotFound)?;
            if private.is_some() {
                return Err(ApiError::Forbidden);
            }
            (name, serde_json::to_value(read_board(pool, p.source_id, &automations).await?).map_err(|e| ApiError::Internal(e.to_string()))?)
        }
        "page" => {
            let row: Option<(String, Option<String>, Option<String>, Option<Vec<u8>>, Option<Uuid>)> =
                sqlx::query_as("SELECT title, icon, content, yjs_state, private_to FROM pages WHERE id = $1 AND workspace_id = $2").bind(p.source_id).bind(ws).fetch_optional(pool).await?;
            let (title, icon, html, state_bytes, private) = row.ok_or(ApiError::NotFound)?;
            if private.is_some() {
                return Err(ApiError::Forbidden);
            }
            // Le texte courant vient de l'état collaboratif ; l'ancien HTML sert quand il n'y en a pas.
            let content = match state_bytes.filter(|s| !s.is_empty()) {
                Some(s) => crate::ai_pages::state_to_html(&s).unwrap_or_default(),
                None => html.unwrap_or_default(),
            };
            let content = ammonia::clean(&content);
            if content.len() > MAX_PAGE_HTML {
                return Err(ApiError::bad_request("template_too_large"));
            }
            (title.clone(), serde_json::to_value(PageTpl { title, icon, content }).map_err(|e| ApiError::Internal(e.to_string()))?)
        }
        _ => return Err(ApiError::bad_request("invalid_kind")),
    };
    let name = p.name.as_deref().map(str::trim).filter(|n| !n.is_empty()).map(str::to_string).unwrap_or(default_name);
    if name.chars().count() > 120 {
        return Err(ApiError::bad_request("invalid_name"));
    }
    let id: Uuid = sqlx::query_scalar("INSERT INTO workspace_templates (workspace_id, kind, name, data, created_by) VALUES ($1, $2, $3, $4, $5) RETURNING id")
        .bind(ws)
        .bind(&p.kind)
        .bind(name)
        .bind(data)
        .bind(user.id)
        .fetch_one(pool)
        .await?;
    Ok(Json(serde_json::json!({ "id": id })))
}

async fn read_board(pool: &sqlx::PgPool, board: Uuid, automations: &serde_json::Value) -> ApiResult<BoardTpl> {
    let done = automations["on_complete_move_to"].as_str().and_then(|s| Uuid::parse_str(s).ok());
    let labels: Vec<(Uuid, String, String)> = sqlx::query_as("SELECT id, name, color FROM labels WHERE board_id = $1 ORDER BY name").bind(board).fetch_all(pool).await?;
    let columns: Vec<(Uuid, String)> = sqlx::query_as("SELECT id, name FROM board_columns WHERE board_id = $1 ORDER BY position").bind(board).fetch_all(pool).await?;
    // Tâches de tête du tableau : ni personnelles, ni terminées (un modèle ne reprend pas l'avancement).
    let tasks: Vec<(Uuid, Uuid, String, Option<String>, i32)> = sqlx::query_as(
        "SELECT t.id, t.column_id, t.title, t.description, t.priority FROM tasks t JOIN board_columns c ON c.id = t.column_id
         WHERE c.board_id = $1 AND t.parent_id IS NULL AND t.private_to IS NULL AND NOT t.is_completed ORDER BY t.position LIMIT $2",
    )
    .bind(board)
    .bind(MAX_TASKS as i64)
    .fetch_all(pool)
    .await?;
    let ids: Vec<Uuid> = tasks.iter().map(|t| t.0).collect();
    let task_labels: Vec<(Uuid, Uuid)> = sqlx::query_as("SELECT task_id, label_id FROM task_labels WHERE task_id = ANY($1)").bind(&ids).fetch_all(pool).await?;
    let subs: Vec<(Uuid, String)> = sqlx::query_as("SELECT parent_id, title FROM tasks WHERE parent_id = ANY($1) AND private_to IS NULL ORDER BY position").bind(&ids).fetch_all(pool).await?;
    let label_name = |id: &Uuid| labels.iter().find(|l| l.0 == *id).map(|l| l.1.clone());
    Ok(BoardTpl {
        labels: labels.iter().map(|l| LabelTpl { name: l.1.clone(), color: l.2.clone() }).collect(),
        columns: columns
            .into_iter()
            .map(|(id, name)| ColumnTpl {
                name,
                done: Some(id) == done,
                tasks: tasks
                    .iter()
                    .filter(|t| t.1 == id)
                    .map(|t| TaskTpl {
                        title: t.2.clone(),
                        description: t.3.clone(),
                        priority: t.4,
                        labels: task_labels.iter().filter(|l| l.0 == t.0).filter_map(|l| label_name(&l.1)).collect(),
                        subtasks: subs.iter().filter(|s| s.0 == t.0).map(|s| s.1.clone()).collect(),
                    })
                    .collect(),
            })
            .collect(),
    })
}

async fn remove(State(state): State<AppState>, user: AuthUser, Path((ws, id)): Path<(Uuid, Uuid)>) -> ApiResult<StatusCode> {
    require_ws_admin(&state.pool, &user, ws).await?;
    let n = sqlx::query("DELETE FROM workspace_templates WHERE id = $1 AND workspace_id = $2").bind(id).bind(ws).execute(&state.pool).await?.rows_affected();
    if n == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_ins_exist_in_both_languages_and_have_the_right_kind() {
        for fr in [true, false] {
            for (id, kind, name) in builtin_list(fr) {
                assert!(!name.is_empty());
                match (builtin(id, fr), kind) {
                    (Some(Chosen::Board(b)), "board") => {
                        assert!(b.columns.iter().any(|c| c.done), "{id} has a done column");
                        // Les étiquettes citées par les tâches existent dans la langue choisie.
                        for t in b.columns.iter().flat_map(|c| &c.tasks).flat_map(|t| &t.labels) {
                            assert!(b.labels.iter().any(|l| l.name == *t), "{id}/{fr}: {t}");
                        }
                    }
                    (Some(Chosen::Page(p)), "page") => assert!(p.content.contains("<h2>")),
                    _ => panic!("{id}"),
                }
            }
        }
    }
}

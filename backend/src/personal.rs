//! Éléments personnels (GLOSSARY) : dans un espace partagé, un document, un projet, un
//! fichier du Drive, un tableau ou une tâche réservé à une personne (`private_to`). Les
//! contrôles d'accès vivent dans access.rs ; ici : passer un élément de « Partagé » à
//! « Personnel » et inversement, et, quand la personne quitte l'espace, emporter ses
//! éléments personnels dans son espace perso (regroupés sous le nom de l'ancien espace).

use axum::{
    extract::{Path, State},
    routing::put,
    Json, Router,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::access::{level, member_perms, AuthUser};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new().route("/{kind}/{id}", put(set_personal))
}

#[derive(Deserialize)]
struct PersonalReq {
    personal: bool,
}

const SUBPAGES: &str = "WITH RECURSIVE sub AS (SELECT id, 0 AS depth FROM pages_all WHERE id = $1
    UNION ALL SELECT p.id, sub.depth + 1 FROM pages_all p JOIN sub ON p.parent_id = sub.id WHERE sub.depth < 64)";
const SUBTASKS: &str = "WITH RECURSIVE sub AS (SELECT id FROM tasks_all WHERE id = $1
    UNION ALL SELECT t.id FROM tasks_all t JOIN sub ON t.parent_id = sub.id)";

/// Passe un élément en personnel (`personal`) ou en partagé. Le rendre personnel est
/// réservé à son auteur ; le repartager, à son propriétaire.
async fn set_personal(State(state): State<AppState>, user: AuthUser, Path((kind, id)): Path<(String, Uuid)>, Json(req): Json<PersonalReq>) -> ApiResult<Json<serde_json::Value>> {
    let owner = req.personal.then_some(user.id);
    match kind.as_str() {
        "page" => set_page(&state, &user, id, owner).await?,
        "board" => set_board(&state, &user, id, owner).await?,
        "task" => set_task(&state, &user, id, owner).await?,
        "project" => set_project(&state, &user, id, owner).await?,
        "file" => set_file(&state, &user, id, owner).await?,
        _ => return Err(ApiError::NotFound),
    }
    Ok(Json(serde_json::json!({ "personal": req.personal })))
}

/// Contrôle commun : la personne peut modifier l'élément, et en est l'autrice (pour le rendre
/// personnel) ou la propriétaire (pour le repartager).
fn check_change(current: Option<Uuid>, author: Option<Uuid>, admin_fallback: bool, user: &AuthUser, owner: Option<Uuid>) -> ApiResult<bool> {
    match (current, owner) {
        (Some(o), _) if o != user.id => Err(ApiError::NotFound),
        (Some(_), Some(_)) | (None, None) => Ok(false),
        (Some(_), None) => Ok(true),
        (None, Some(_)) => match author {
            Some(a) if a == user.id => Ok(true),
            None if admin_fallback => Ok(true),
            _ => Err(ApiError::bad_request("personal_not_author")),
        },
    }
}

async fn set_page(state: &AppState, user: &AuthUser, id: Uuid, owner: Option<Uuid>) -> ApiResult<()> {
    let pool = &state.pool;
    let (ws, _) = crate::access::require_page_write(pool, user, id).await?;
    let (current, author, parent_owner): (Option<Uuid>, Uuid, Option<Uuid>) = sqlx::query_as(
        "SELECT p.private_to, p.created_by, pp.private_to FROM pages p LEFT JOIN pages pp ON pp.id = p.parent_id WHERE p.id = $1",
    )
    .bind(id)
    .fetch_one(pool)
    .await?;
    if !check_change(current, Some(author), false, user, owner)? {
        return Ok(());
    }
    // Sous une page personnelle, une page l'est forcément.
    if owner.is_none() && parent_owner.is_some() {
        return Err(ApiError::bad_request("personal_parent"));
    }
    let ids: Vec<Uuid> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!("{SUBPAGES} UPDATE pages_all SET private_to = $2 WHERE id IN (SELECT id FROM sub) RETURNING id")))
        .bind(id)
        .bind(owner)
        .fetch_all(pool)
        .await?;
    // Projets portés par ces pages : mêmes droits.
    sqlx::query("UPDATE projects SET private_to = $2 WHERE id IN (SELECT project_id FROM pages_all WHERE id = ANY($1))").bind(&ids).bind(owner).execute(pool).await?;
    if owner.is_some() {
        // Les partages individuels n'ont plus cours.
        sqlx::query("DELETE FROM page_permissions WHERE page_id = ANY($1)").bind(&ids).execute(pool).await?;
        for p in &ids {
            state.emit(ws, serde_json::json!({ "type": "PAGE_DELETED", "page_id": p, "except": owner })).await;
        }
    }
    // Listes rechargées ; documents ouverts ailleurs revérifiés par la collaboration.
    state.emit(ws, serde_json::json!({ "type": "PAGE_PERMISSIONS_UPDATED", "page_id": id })).await;
    Ok(())
}

/// Tâches d'un tableau ou d'une arborescence devenues personnelles : seul le propriétaire
/// reste assigné, les groupes reliés sont déliés.
async fn keep_only_owner(pool: &sqlx::PgPool, tasks: &[Uuid], owner: Uuid) -> ApiResult<()> {
    sqlx::query("DELETE FROM task_assignees WHERE task_id = ANY($1) AND user_id <> $2").bind(tasks).bind(owner.to_string()).execute(pool).await?;
    sqlx::query("DELETE FROM task_groups WHERE task_id = ANY($1)").bind(tasks).execute(pool).await?;
    Ok(())
}

async fn emit_tasks(state: &AppState, ws: Uuid, tasks: &[Uuid], owner: Option<Uuid>) {
    let personal = owner.is_some();
    for &t in tasks {
        if personal {
            state.emit(ws, serde_json::json!({ "type": "TASK_DELETED", "task_id": t, "except": owner })).await;
        }
        crate::board::emit_task(state, ws, if personal { "TASK_UPDATED" } else { "TASK_CREATED" }, t).await;
    }
}

async fn set_board(state: &AppState, user: &AuthUser, id: Uuid, owner: Option<Uuid>) -> ApiResult<()> {
    let pool = &state.pool;
    let ws = crate::access::board_workspace_for(pool, user, id).await?;
    let (_, perms) = member_perms(pool, user.id, ws).await?.ok_or(ApiError::NotFound)?;
    if perms.boards < level::MANAGE {
        return Err(ApiError::Forbidden);
    }
    let (current, author): (Option<Uuid>, Option<Uuid>) = sqlx::query_as("SELECT private_to, created_by FROM boards WHERE id = $1").bind(id).fetch_one(pool).await?;
    // Ancien tableau sans auteur connu : un administrateur peut le rendre personnel.
    if !check_change(current, author, perms.admin, user, owner)? {
        return Ok(());
    }
    sqlx::query("UPDATE boards SET private_to = $2 WHERE id = $1").bind(id).bind(owner).execute(pool).await?;
    let tasks: Vec<Uuid> = sqlx::query_scalar(
        "UPDATE tasks_all SET private_to = $2 WHERE column_id IN (SELECT id FROM board_columns WHERE board_id = $1) RETURNING id",
    )
    .bind(id)
    .bind(owner)
    .fetch_all(pool)
    .await?;
    if let Some(o) = owner {
        keep_only_owner(pool, &tasks, o).await?;
    }
    state.emit(ws, serde_json::json!({ "type": "BOARD_DELETED", "board_id": id })).await;
    emit_tasks(state, ws, &tasks, owner).await;
    Ok(())
}

async fn set_task(state: &AppState, user: &AuthUser, id: Uuid, owner: Option<Uuid>) -> ApiResult<()> {
    let pool = &state.pool;
    let ctx = crate::access::task_ctx_for(pool, user, id).await?;
    let (_, perms) = member_perms(pool, user.id, ctx.workspace_id).await?.ok_or(ApiError::NotFound)?;
    if perms.boards < level::EDIT {
        return Err(ApiError::Forbidden);
    }
    let (current, parent_owner, board_owner): (Option<Uuid>, Option<Uuid>, Option<Uuid>) = sqlx::query_as(
        "SELECT t.private_to, p.private_to, b.private_to FROM tasks t LEFT JOIN tasks p ON p.id = t.parent_id JOIN boards b ON b.id = $2 WHERE t.id = $1",
    )
    .bind(id)
    .bind(ctx.board_id)
    .fetch_one(pool)
    .await?;
    // Autrice : celle qui l'a créée (journal de la tâche).
    let author: Option<Uuid> = sqlx::query_scalar("SELECT user_id FROM task_activities WHERE task_id = $1 AND activity_type = 'create' ORDER BY created_at LIMIT 1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    if !check_change(current, author, perms.admin, user, owner)? {
        return Ok(());
    }
    if owner.is_none() && (parent_owner.is_some() || board_owner.is_some()) {
        return Err(ApiError::bad_request("personal_parent"));
    }
    let tasks: Vec<Uuid> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!("{SUBTASKS} UPDATE tasks_all SET private_to = $2 WHERE id IN (SELECT id FROM sub) RETURNING id")))
        .bind(id)
        .bind(owner)
        .fetch_all(pool)
        .await?;
    if let Some(o) = owner {
        keep_only_owner(pool, &tasks, o).await?;
    }
    emit_tasks(state, ctx.workspace_id, &tasks, owner).await;
    Ok(())
}

async fn set_project(state: &AppState, user: &AuthUser, id: Uuid, owner: Option<Uuid>) -> ApiResult<()> {
    let pool = &state.pool;
    let (project, _) = crate::projects::project_for(state, user, id, crate::projects::Need::Write).await?;
    // Projet porté par une page : c'est la page qu'on rend personnelle.
    let page: Option<Uuid> = sqlx::query_scalar("SELECT id FROM pages_all WHERE project_id = $1").bind(id).fetch_optional(pool).await?;
    if let Some(page) = page {
        return set_page(state, user, page, owner).await;
    }
    let current: Option<Uuid> = sqlx::query_scalar("SELECT private_to FROM projects WHERE id = $1").bind(id).fetch_one(pool).await?;
    if !check_change(current, project.created_by, false, user, owner)? {
        return Ok(());
    }
    sqlx::query("UPDATE projects SET private_to = $2 WHERE id = $1").bind(id).bind(owner).execute(pool).await?;
    state.emit(project.workspace_id, serde_json::json!({ "type": "PROJECT_FILES_CHANGED", "project_id": id })).await;
    Ok(())
}

async fn set_file(state: &AppState, user: &AuthUser, id: Uuid, owner: Option<Uuid>) -> ApiResult<()> {
    let pool = &state.pool;
    let row: Option<(Uuid, String, String, bool, Option<Uuid>, Option<Uuid>)> =
        sqlx::query_as("SELECT workspace_id, folder, name, is_dir, uploaded_by, private_to FROM drive_files WHERE id = $1").bind(id).fetch_optional(pool).await?;
    let (ws, folder, name, is_dir, author, current) = row.ok_or(ApiError::NotFound)?;
    let (_, perms) = member_perms(pool, user.id, ws).await?.ok_or(ApiError::NotFound)?;
    if perms.docs < level::EDIT {
        return Err(ApiError::Forbidden);
    }
    if !check_change(current, author, false, user, owner)? {
        return Ok(());
    }
    // Dossier : son contenu suit.
    let path = if folder.is_empty() { name } else { format!("{folder}/{name}") };
    sqlx::query("UPDATE drive_files SET private_to = $2 WHERE id = $1 OR ($3 AND workspace_id = $4 AND (folder = $5 OR folder LIKE $6))")
        .bind(id)
        .bind(owner)
        .bind(is_dir)
        .bind(ws)
        .bind(&path)
        .bind(format!("{}/%", path.replace('%', "\\%").replace('_', "\\_")))
        .execute(pool)
        .await?;
    state.emit(ws, serde_json::json!({ "type": "DRIVE_CHANGED" })).await;
    Ok(())
}

// ---------------------------------------------------------------------------
// Départ d'un espace : les éléments personnels partent dans l'espace perso.
// ---------------------------------------------------------------------------

/// Espace perso de la personne (créé au besoin).
pub async fn personal_space(pool: &sqlx::PgPool, user: Uuid) -> ApiResult<Uuid> {
    sqlx::query("INSERT INTO workspaces (name, personal_of) VALUES ('Perso', $1) ON CONFLICT (personal_of) DO NOTHING").bind(user).execute(pool).await?;
    let id: Uuid = sqlx::query_scalar("SELECT id FROM workspaces WHERE personal_of = $1").bind(user).fetch_one(pool).await?;
    sqlx::query("INSERT INTO workspace_members (workspace_id, user_id, role) VALUES ($1, $2, 'owner') ON CONFLICT DO NOTHING")
        .bind(id)
        .bind(user.to_string())
        .execute(pool)
        .await?;
    Ok(id)
}

/// Emporte les éléments personnels de `user` de l'espace `ws` dans son espace perso :
/// documents sous une page au nom de l'espace (créée ou réutilisée), tableaux et projets
/// tels quels, tâches personnelles d'un tableau partagé dans un tableau au nom de l'espace,
/// fichiers du Drive dans un dossier au nom de l'espace. À appeler avant de retirer la
/// personne (ou de supprimer l'espace).
pub async fn evacuate(state: &AppState, ws: Uuid, user: Uuid) -> ApiResult<()> {
    let pool = &state.pool;
    let ws_name: String = sqlx::query_scalar("SELECT name FROM workspaces WHERE id = $1").bind(ws).fetch_one(pool).await?;
    let any: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pages_all WHERE workspace_id = $1 AND private_to = $2)
             OR EXISTS (SELECT 1 FROM boards WHERE workspace_id = $1 AND private_to = $2)
             OR EXISTS (SELECT 1 FROM tasks_all t JOIN board_columns c ON c.id = t.column_id JOIN boards b ON b.id = c.board_id
                         WHERE b.workspace_id = $1 AND t.private_to = $2)
             OR EXISTS (SELECT 1 FROM projects WHERE workspace_id = $1 AND private_to = $2)
             OR EXISTS (SELECT 1 FROM drive_files WHERE workspace_id = $1 AND private_to = $2)",
    )
    .bind(ws)
    .bind(user)
    .fetch_one(pool)
    .await?;
    if !any {
        return Ok(());
    }
    let home = personal_space(pool, user).await?;

    // Documents : chaque page personnelle « racine » (parent partagé ou absent) et ses
    // sous-pages, sous une page au nom de l'espace.
    let roots: Vec<Uuid> = sqlx::query_scalar(
        "SELECT p.id FROM pages_all p LEFT JOIN pages_all pp ON pp.id = p.parent_id
          WHERE p.workspace_id = $1 AND p.private_to = $2 AND (pp.id IS NULL OR pp.private_to IS DISTINCT FROM $2)
          ORDER BY p.position",
    )
    .bind(ws)
    .bind(user)
    .fetch_all(pool)
    .await?;
    if !roots.is_empty() {
        let container = folder_page(pool, home, &ws_name, user).await?;
        for root in roots {
            let ids: Vec<Uuid> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!("{SUBPAGES} SELECT id FROM sub ORDER BY depth"))).bind(root).fetch_all(pool).await?;
            crate::page::move_tree(pool, ws, root, &ids, home, Some(container), user).await?;
            sqlx::query("UPDATE pages_all SET private_to = NULL WHERE id = ANY($1)").bind(&ids).execute(pool).await?;
            sqlx::query("UPDATE projects SET private_to = NULL WHERE id IN (SELECT project_id FROM pages_all WHERE id = ANY($1))").bind(&ids).execute(pool).await?;
            for p in &ids {
                state.emit(ws, serde_json::json!({ "type": "PAGE_DELETED", "page_id": p })).await;
            }
        }
    }

    // Tableaux personnels : déplacés avec leur calendrier.
    let boards: Vec<Uuid> = sqlx::query_scalar("UPDATE boards SET workspace_id = $3, private_to = NULL WHERE workspace_id = $1 AND private_to = $2 RETURNING id")
        .bind(ws)
        .bind(user)
        .bind(home)
        .fetch_all(pool)
        .await?;
    if !boards.is_empty() {
        sqlx::query("UPDATE tasks_all SET private_to = NULL WHERE column_id IN (SELECT id FROM board_columns WHERE board_id = ANY($1))").bind(&boards).execute(pool).await?;
        sqlx::query("UPDATE workspace_events SET workspace_id = $2 WHERE calendar_id IN (SELECT id FROM looty_calendars WHERE board_id = ANY($1))").bind(&boards).bind(home).execute(pool).await?;
        sqlx::query("UPDATE looty_calendars SET workspace_id = $2 WHERE board_id = ANY($1)").bind(&boards).bind(home).execute(pool).await?;
        for b in &boards {
            state.emit(ws, serde_json::json!({ "type": "BOARD_DELETED", "board_id": b })).await;
        }
    }

    // Tâches personnelles d'un tableau partagé : dans un tableau au nom de l'espace.
    let roots: Vec<Uuid> = sqlx::query_scalar(
        "SELECT t.id FROM tasks_all t JOIN board_columns c ON c.id = t.column_id JOIN boards b ON b.id = c.board_id
          LEFT JOIN tasks_all p ON p.id = t.parent_id
          WHERE b.workspace_id = $1 AND t.private_to = $2 AND (p.id IS NULL OR p.private_to IS DISTINCT FROM $2)",
    )
    .bind(ws)
    .bind(user)
    .fetch_all(pool)
    .await?;
    if !roots.is_empty() {
        let column = folder_board(pool, home, &ws_name, user).await?;
        for root in roots {
            let ids: Vec<Uuid> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                "{SUBTASKS} UPDATE tasks_all SET column_id = $2, private_to = NULL, event_id = NULL, event_start = NULL WHERE id IN (SELECT id FROM sub) RETURNING id"
            )))
            .bind(root)
            .bind(column)
            .fetch_all(pool)
            .await?;
            // Étiquettes et groupes appartenaient à l'ancien espace.
            sqlx::query("DELETE FROM task_labels WHERE task_id = ANY($1)").bind(&ids).execute(pool).await?;
            sqlx::query("DELETE FROM task_groups WHERE task_id = ANY($1)").bind(&ids).execute(pool).await?;
            for t in &ids {
                state.emit(ws, serde_json::json!({ "type": "TASK_DELETED", "task_id": t })).await;
            }
        }
    }

    // Projets personnels (hors documents).
    sqlx::query("UPDATE projects SET workspace_id = $3, private_to = NULL WHERE workspace_id = $1 AND private_to = $2")
        .bind(ws)
        .bind(user)
        .bind(home)
        .execute(pool)
        .await?;

    // Fichiers du Drive : dans un dossier au nom de l'espace (le dossier racine est créé ;
    // un nom déjà pris dans l'espace perso reçoit un suffixe).
    let files: Vec<(Uuid, String, String)> =
        sqlx::query_as("SELECT id, folder, name FROM drive_files WHERE workspace_id = $1 AND private_to = $2 ORDER BY length(folder), is_dir DESC")
            .bind(ws)
            .bind(user)
            .fetch_all(pool)
            .await?;
    if !files.is_empty() {
        let root = drive_root(pool, home, &ws_name, user).await?;
        for (id, folder, name) in files {
            let folder = if folder.is_empty() { root.clone() } else { format!("{root}/{folder}") };
            let mut candidate = name.clone();
            let mut n = 2;
            while sqlx::query_scalar::<_, bool>("SELECT EXISTS (SELECT 1 FROM drive_files WHERE workspace_id = $1 AND folder = $2 AND name = $3)")
                .bind(home)
                .bind(&folder)
                .bind(&candidate)
                .fetch_one(pool)
                .await?
            {
                candidate = format!("{name} ({n})");
                n += 1;
            }
            sqlx::query("UPDATE drive_files SET workspace_id = $2, folder = $3, name = $4, private_to = NULL WHERE id = $1")
                .bind(id)
                .bind(home)
                .bind(&folder)
                .bind(&candidate)
                .execute(pool)
                .await?;
        }
        state.emit(ws, serde_json::json!({ "type": "DRIVE_CHANGED" })).await;
    }
    state.emit_user(user, serde_json::json!({ "type": "PERSONAL_ITEMS_MOVED", "workspace_id": home })).await;
    Ok(())
}

/// Page au nom de l'ancien espace, à la racine de l'espace perso (réutilisée si elle existe).
async fn folder_page(pool: &sqlx::PgPool, home: Uuid, name: &str, user: Uuid) -> ApiResult<Uuid> {
    if let Some(id) = sqlx::query_scalar("SELECT id FROM pages WHERE workspace_id = $1 AND parent_id IS NULL AND title = $2 ORDER BY created_at LIMIT 1")
        .bind(home)
        .bind(name)
        .fetch_optional(pool)
        .await?
    {
        return Ok(id);
    }
    Ok(sqlx::query_scalar(
        "INSERT INTO pages (workspace_id, title, icon, created_by, updated_by, position)
         VALUES ($1, $2, '🗂️', $3, $3, COALESCE((SELECT MAX(position) FROM pages WHERE workspace_id = $1 AND parent_id IS NULL), 0) + 65536) RETURNING id",
    )
    .bind(home)
    .bind(name)
    .bind(user)
    .fetch_one(pool)
    .await?)
}

/// Première colonne du tableau au nom de l'ancien espace, dans l'espace perso (créé au besoin).
async fn folder_board(pool: &sqlx::PgPool, home: Uuid, name: &str, user: Uuid) -> ApiResult<Uuid> {
    let board: Uuid = match sqlx::query_scalar("SELECT id FROM boards WHERE workspace_id = $1 AND name = $2 ORDER BY created_at LIMIT 1")
        .bind(home)
        .bind(name)
        .fetch_optional(pool)
        .await?
    {
        Some(b) => b,
        None => {
            let b: Uuid = sqlx::query_scalar("INSERT INTO boards (workspace_id, name, created_by) VALUES ($1, $2, $3) RETURNING id")
                .bind(home)
                .bind(name)
                .bind(user)
                .fetch_one(pool)
                .await?;
            for (i, col) in ["À faire", "En cours", "Terminé"].iter().enumerate() {
                sqlx::query("INSERT INTO board_columns (board_id, name, position) VALUES ($1, $2, $3)").bind(b).bind(col).bind((i as f64 + 1.0) * 65536.0).execute(pool).await?;
            }
            b
        }
    };
    Ok(sqlx::query_scalar("SELECT id FROM board_columns WHERE board_id = $1 ORDER BY position LIMIT 1").bind(board).fetch_one(pool).await?)
}

/// Dossier racine du Drive au nom de l'ancien espace (créé au besoin) ; renvoie son chemin.
async fn drive_root(pool: &sqlx::PgPool, home: Uuid, name: &str, user: Uuid) -> ApiResult<String> {
    let name: String = name.replace('/', "-").chars().take(200).collect();
    sqlx::query("INSERT INTO drive_files (workspace_id, folder, name, is_dir, uploaded_by) VALUES ($1, '', $2, TRUE, $3) ON CONFLICT DO NOTHING")
        .bind(home)
        .bind(&name)
        .bind(user)
        .execute(pool)
        .await?;
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(id: Uuid) -> AuthUser {
        AuthUser { id, is_super_admin: false }
    }

    #[test]
    fn only_author_makes_personal_and_owner_shares_back() {
        let (me, other) = (Uuid::new_v4(), Uuid::new_v4());
        // Rendre personnel : l'autrice seulement (ou un admin si l'autrice est inconnue).
        assert!(check_change(None, Some(me), false, &user(me), Some(me)).unwrap());
        assert!(check_change(None, Some(other), true, &user(me), Some(me)).is_err());
        assert!(check_change(None, None, true, &user(me), Some(me)).unwrap());
        assert!(check_change(None, None, false, &user(me), Some(me)).is_err());
        // Repartager : la propriétaire seulement ; un autre ne le voit même pas.
        assert!(check_change(Some(me), Some(me), false, &user(me), None).unwrap());
        assert!(matches!(check_change(Some(other), Some(other), true, &user(me), None), Err(ApiError::NotFound)));
        // Déjà dans l'état demandé : rien à faire.
        assert!(!check_change(None, Some(me), false, &user(me), None).unwrap());
        assert!(!check_change(Some(me), Some(me), false, &user(me), Some(me)).unwrap());
    }
}

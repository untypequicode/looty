//! Graphe de l'espace (comme Obsidian) : pages, tâches et tableaux, reliés par
//! - la hiérarchie (sous-pages, sous-tâches, tâche → tableau),
//! - les liens présents dans le contenu (lien vers une page ou une tâche dans un
//!   document, une description ou un commentaire),
//! - les relations créées à la main (table `item_links`).
//! Les pages inaccessibles au lecteur (droits restreints) n'apparaissent pas.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::access::{require_member, require_writer, visible_pages, AuthUser, PageAccess};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub fn workspace_routes() -> Router<AppState> {
    Router::new()
        .route("/graph", get(graph))
        .route("/graph/related", get(related))
        .route("/links", post(create_link))
        .route("/links/:id", delete(delete_link))
}

#[derive(Serialize, Clone)]
pub struct Node {
    id: Uuid,
    /// page | task | board | event
    kind: &'static str,
    label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    board_id: Option<Uuid>,
}

#[derive(Serialize, Clone)]
pub struct Edge {
    source: Uuid,
    target: Uuid,
    /// parent | board | link | relation
    kind: &'static str,
    /// Relation manuelle : identifiant (pour la supprimer).
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Uuid>,
}

/// Identifiants de pages et de tâches cités dans un texte (liens Looty).
fn referenced_ids(text: &str) -> Vec<(&'static str, Uuid)> {
    let mut out = Vec::new();
    for (marker, kind) in [("/doc/", "page"), ("/task/", "task"), ("task=", "task")] {
        let mut rest = text;
        while let Some(i) = rest.find(marker) {
            let tail = &rest[i + marker.len()..];
            if let Some(id) = tail.get(..36).and_then(|s| Uuid::parse_str(s).ok()) {
                out.push((kind, id));
            }
            rest = &tail[tail.len().min(1)..];
        }
    }
    out
}

struct Built {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
}

async fn build(state: &AppState, user: &AuthUser, ws: Uuid) -> ApiResult<Built> {
    let role = require_member(&state.pool, user, ws).await?;
    let visible = visible_pages(&state.pool, user, ws, role).await?;
    let pages: Vec<(Uuid, String, Option<String>, Option<Uuid>, Option<String>, Option<Vec<u8>>)> =
        sqlx::query_as("SELECT id, title, icon, parent_id, content, yjs_state FROM pages WHERE workspace_id = $1").bind(ws).fetch_all(&state.pool).await?;
    let pages: Vec<_> = pages.into_iter().filter(|p| matches!(visible.get(&p.0), Some(PageAccess::Read | PageAccess::Write))).collect();
    let boards: Vec<(Uuid, String)> = sqlx::query_as("SELECT id, name FROM boards WHERE workspace_id = $1").bind(ws).fetch_all(&state.pool).await?;
    let tasks: Vec<(Uuid, String, bool, Option<Uuid>, Uuid, Option<String>)> = sqlx::query_as(
        "SELECT t.id, t.title, t.is_completed, t.parent_id, b.id, t.description FROM tasks t
         JOIN board_columns c ON c.id = t.column_id JOIN boards b ON b.id = c.board_id
         WHERE b.workspace_id = $1 ORDER BY t.updated_at DESC LIMIT 5000",
    )
    .bind(ws)
    .fetch_all(&state.pool)
    .await?;
    let comments: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT cm.task_id, cm.content FROM task_comments cm JOIN tasks t ON t.id = cm.task_id
         JOIN board_columns c ON c.id = t.column_id JOIN boards b ON b.id = c.board_id
         WHERE b.workspace_id = $1 AND (cm.content LIKE '%/doc/%' OR cm.content LIKE '%task%')",
    )
    .bind(ws)
    .fetch_all(&state.pool)
    .await?;
    let relations: Vec<(Uuid, Uuid, Uuid)> =
        sqlx::query_as("SELECT id, source_id, target_id FROM item_links WHERE workspace_id = $1").bind(ws).fetch_all(&state.pool).await?;
    // Commentaires des pages et des événements (liens cités dans la discussion).
    let item_comments: Vec<(Uuid, String)> =
        sqlx::query_as("SELECT target_id, content FROM item_comments WHERE workspace_id = $1 AND (content LIKE '%/doc/%' OR content LIKE '%task%')")
            .bind(ws)
            .fetch_all(&state.pool)
            .await?;
    // Événements de l'espace : présents dans le graphe s'ils citent un élément (description,
    // commentaires) ou ont une relation manuelle.
    let linked_events: HashSet<Uuid> = relations.iter().flat_map(|(_, s, t)| [*s, *t]).chain(item_comments.iter().map(|(t, _)| *t)).collect();
    let event_rows: Vec<(Uuid, String)> = sqlx::query_as("SELECT id, ics FROM workspace_events WHERE workspace_id = $1 LIMIT 5000").bind(ws).fetch_all(&state.pool).await?;
    let mut events: Vec<(Uuid, String, Vec<(&'static str, Uuid)>)> = Vec::new();
    for (id, ics) in &event_rows {
        let Some(m) = crate::calendar::master_event(ics) else { continue };
        let refs = referenced_ids(m.description.as_deref().unwrap_or(""));
        if !refs.is_empty() || linked_events.contains(id) {
            events.push((*id, m.summary, refs));
        }
    }

    let mut nodes = Vec::with_capacity(pages.len() + boards.len() + tasks.len());
    let mut kinds: HashMap<Uuid, &'static str> = HashMap::new();
    for (id, title, icon, ..) in &pages {
        kinds.insert(*id, "page");
        nodes.push(Node { id: *id, kind: "page", label: title.clone(), icon: icon.clone(), done: false, board_id: None });
    }
    for (id, name) in &boards {
        kinds.insert(*id, "board");
        nodes.push(Node { id: *id, kind: "board", label: name.clone(), icon: None, done: false, board_id: None });
    }
    for (id, title, done, _, board, _) in &tasks {
        kinds.insert(*id, "task");
        nodes.push(Node { id: *id, kind: "task", label: title.clone(), icon: None, done: *done, board_id: Some(*board) });
    }
    for (id, title, _) in &events {
        kinds.insert(*id, "event");
        nodes.push(Node { id: *id, kind: "event", label: title.clone(), icon: None, done: false, board_id: None });
    }

    let mut edges = Vec::new();
    let mut seen: HashSet<(Uuid, Uuid)> = HashSet::new();
    let mut push = |edges: &mut Vec<Edge>, source: Uuid, target: Uuid, kind: &'static str, id: Option<Uuid>| {
        if source == target || !kinds.contains_key(&source) || !kinds.contains_key(&target) {
            return;
        }
        // Une seule arête par paire (la relation manuelle prime : elle est supprimable).
        let key = if source < target { (source, target) } else { (target, source) };
        if seen.insert(key) {
            edges.push(Edge { source, target, kind, id });
        } else if id.is_some() {
            if let Some(e) = edges.iter_mut().find(|e| (e.source == key.0 && e.target == key.1) || (e.source == key.1 && e.target == key.0)) {
                e.kind = kind;
                e.id = id;
            }
        }
    };
    for (id, s, t) in &relations {
        push(&mut edges, *s, *t, "relation", Some(*id));
    }
    for (id, _, _, parent, content, yjs) in &pages {
        if let Some(p) = parent {
            push(&mut edges, *p, *id, "parent", None);
        }
        let mut text = content.clone().unwrap_or_default();
        if let Some(b) = yjs {
            text.push_str(&String::from_utf8_lossy(b));
        }
        for (_, target) in referenced_ids(&text) {
            push(&mut edges, *id, target, "link", None);
        }
    }
    for (id, _, _, parent, board, desc) in &tasks {
        if let Some(p) = parent {
            push(&mut edges, *p, *id, "parent", None);
        }
        push(&mut edges, *board, *id, "board", None);
        for (_, target) in referenced_ids(desc.as_deref().unwrap_or("")) {
            push(&mut edges, *id, target, "link", None);
        }
    }
    for (task, content) in &comments {
        for (_, target) in referenced_ids(content) {
            push(&mut edges, *task, target, "link", None);
        }
    }
    for (item, content) in &item_comments {
        for (_, target) in referenced_ids(content) {
            push(&mut edges, *item, target, "link", None);
        }
    }
    for (id, _, refs) in &events {
        for (_, target) in refs {
            push(&mut edges, *id, *target, "link", None);
        }
    }
    Ok(Built { nodes, edges })
}

async fn graph(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>) -> ApiResult<Json<serde_json::Value>> {
    let b = build(&state, &user, ws).await?;
    Ok(Json(serde_json::json!({ "nodes": b.nodes, "edges": b.edges })))
}

#[derive(Deserialize)]
struct RelatedQuery {
    id: Uuid,
}

/// Éléments reliés à une page ou une tâche (panneau « Liés »), hors hiérarchie.
async fn related(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>, Query(q): Query<RelatedQuery>) -> ApiResult<Json<serde_json::Value>> {
    let b = build(&state, &user, ws).await?;
    let by_id: HashMap<Uuid, &Node> = b.nodes.iter().map(|n| (n.id, n)).collect();
    let mut out = Vec::new();
    for e in b.edges.iter().filter(|e| matches!(e.kind, "link" | "relation") && (e.source == q.id || e.target == q.id)) {
        let (other, incoming) = if e.source == q.id { (e.target, false) } else { (e.source, true) };
        if let Some(n) = by_id.get(&other) {
            out.push(serde_json::json!({ "node": n, "kind": e.kind, "incoming": incoming, "link_id": e.id }));
        }
    }
    Ok(Json(serde_json::json!({ "items": out })))
}

#[derive(Deserialize)]
struct LinkReq {
    source_kind: String,
    source_id: Uuid,
    target_kind: String,
    target_id: Uuid,
}

/// L'élément appartient à l'espace (et la page est lisible par l'utilisateur).
async fn check_item(state: &AppState, user: &AuthUser, ws: Uuid, kind: &str, id: Uuid) -> ApiResult<()> {
    let ok = match kind {
        "page" => {
            let (pws, _, access) = crate::access::page_access(&state.pool, user, id).await?;
            pws == ws && !matches!(access, PageAccess::None)
        }
        "task" => crate::access::task_ctx(&state.pool, id).await?.workspace_id == ws,
        "event" => sqlx::query_scalar::<_, i64>("SELECT count(*) FROM workspace_events WHERE id = $1 AND workspace_id = $2").bind(id).bind(ws).fetch_one(&state.pool).await? > 0,
        _ => false,
    };
    if ok { Ok(()) } else { Err(ApiError::bad_request("link_invalid")) }
}

async fn create_link(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>, Json(p): Json<LinkReq>) -> ApiResult<Json<serde_json::Value>> {
    require_writer(&state.pool, &user, ws).await?;
    if p.source_id == p.target_id {
        return Err(ApiError::bad_request("link_invalid"));
    }
    check_item(&state, &user, ws, &p.source_kind, p.source_id).await?;
    check_item(&state, &user, ws, &p.target_kind, p.target_id).await?;
    // Relation non orientée : déjà présente dans un sens ou dans l'autre ?
    let existing: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM item_links WHERE workspace_id = $1 AND ((source_id = $2 AND target_id = $3) OR (source_id = $3 AND target_id = $2))",
    )
    .bind(ws)
    .bind(p.source_id)
    .bind(p.target_id)
    .fetch_optional(&state.pool)
    .await?;
    let id = match existing {
        Some(id) => id,
        None => sqlx::query_scalar(
            "INSERT INTO item_links (workspace_id, source_kind, source_id, target_kind, target_id, created_by) VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
        )
        .bind(ws)
        .bind(&p.source_kind)
        .bind(p.source_id)
        .bind(&p.target_kind)
        .bind(p.target_id)
        .bind(user.id)
        .fetch_one(&state.pool)
        .await?,
    };
    state.emit(ws, serde_json::json!({ "type": "GRAPH_UPDATED" })).await;
    Ok(Json(serde_json::json!({ "id": id })))
}

async fn delete_link(State(state): State<AppState>, user: AuthUser, Path((ws, id)): Path<(Uuid, Uuid)>) -> ApiResult<StatusCode> {
    require_writer(&state.pool, &user, ws).await?;
    let n = sqlx::query("DELETE FROM item_links WHERE id = $1 AND workspace_id = $2").bind(id).bind(ws).execute(&state.pool).await?.rows_affected();
    if n == 0 {
        return Err(ApiError::NotFound);
    }
    state.emit(ws, serde_json::json!({ "type": "GRAPH_UPDATED" })).await;
    Ok(StatusCode::NO_CONTENT)
}

/// Relations dont un élément a été supprimé définitivement (appelé par la purge).
pub async fn prune_links(pool: &sqlx::PgPool) {
    let _ = sqlx::query(
        "DELETE FROM item_links l WHERE
            (l.source_kind = 'page' AND NOT EXISTS (SELECT 1 FROM pages_all p WHERE p.id = l.source_id))
         OR (l.target_kind = 'page' AND NOT EXISTS (SELECT 1 FROM pages_all p WHERE p.id = l.target_id))
         OR (l.source_kind = 'task' AND NOT EXISTS (SELECT 1 FROM tasks_all t WHERE t.id = l.source_id))
         OR (l.target_kind = 'task' AND NOT EXISTS (SELECT 1 FROM tasks_all t WHERE t.id = l.target_id))
         OR (l.source_kind = 'event' AND NOT EXISTS (SELECT 1 FROM workspace_events e WHERE e.id = l.source_id))
         OR (l.target_kind = 'event' AND NOT EXISTS (SELECT 1 FROM workspace_events e WHERE e.id = l.target_id))",
    )
    .execute(pool)
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_page_and_task_links() {
        let p = "8c3f6a0e-1b2c-4d5e-8f90-123456789abc";
        let t = "11111111-2222-4333-8444-555555555555";
        let html = format!(r#"<a href="/w/x/doc/{p}">Doc</a> and <a href="https://looty.x/w/y/board/z?task={t}">T</a> /doc/not-a-uuid"#);
        let ids = referenced_ids(&html);
        assert_eq!(ids, vec![("page", Uuid::parse_str(p).unwrap()), ("task", Uuid::parse_str(t).unwrap())]);
    }
}

//! Données du compte et des espaces (GLOSSARY : *Export*, *Suppression de compte*) : archive zip
//! de ce qu'une personne a écrit ou de tout un espace, et suppression d'un compte après un délai
//! de grâce. Les éléments partagés restent dans leurs espaces, leur auteur devient le compte
//! technique « Compte supprimé » ; les éléments personnels et l'espace perso disparaissent.
//! Voir docs/adr/0013-account-export-and-deletion.md.

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderValue},
    response::Response,
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use tokio::sync::Semaphore;
use uuid::Uuid;

use crate::access::{require_ws_admin, AuthUser};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use crate::status::DELETED_USER;
use crate::upload::UPLOAD_DIR;

pub fn user_routes() -> Router<AppState> {
    Router::new().route("/export", get(export_account)).route("/deletion", get(deletion_status).post(request_deletion).delete(cancel_deletion))
}

pub fn workspace_routes() -> Router<AppState> {
    Router::new().route("/", get(export_space))
}

// ---------------------------------------------------------------------------
// Archive
// ---------------------------------------------------------------------------

enum Entry {
    Bytes(String, Vec<u8>),
    File(String, PathBuf),
}

/// Contenu de l'archive et noms déjà pris (sans tenir compte de la casse, pour les systèmes de fichiers qui l'ignorent).
#[derive(Default)]
struct Archive {
    entries: Vec<Entry>,
    used: HashSet<String>,
}

/// Un nom de fichier ou de dossier sûr : ni séparateur, ni caractère de contrôle, ni « .. ».
fn seg(s: &str) -> String {
    let c: String = s.chars().map(|c| if c.is_control() || "/\\:*?\"<>|".contains(c) { '_' } else { c }).collect();
    let c: String = c.trim().trim_matches('.').trim().chars().take(80).collect();
    if c.is_empty() { "untitled".into() } else { c }
}

/// Chemin relatif sous le dossier d'upload (fichiers du Drive et des projets), sans sortie possible.
fn blob_ok(rel: &str) -> bool {
    !rel.is_empty() && !rel.starts_with('/') && !rel.contains('\\') && rel.split('/').all(|s| !s.is_empty() && s != "." && s != "..")
}

impl Archive {
    /// Chemin libre : « a/b.md » devient « a/b (2).md » s'il est pris.
    fn unique(&mut self, path: String) -> String {
        let mut candidate = path.clone();
        let (stem, ext) = match path.rfind('.').filter(|i| path.rfind('/').is_none_or(|s| *i > s + 1)) {
            Some(i) => (path[..i].to_string(), path[i..].to_string()),
            None => (path.clone(), String::new()),
        };
        let mut n = 2;
        while !self.used.insert(candidate.to_lowercase()) {
            candidate = format!("{stem} ({n}){ext}");
            n += 1;
        }
        candidate
    }
    fn bytes(&mut self, path: String, data: Vec<u8>) -> String {
        let p = self.unique(path);
        self.entries.push(Entry::Bytes(p.clone(), data));
        p
    }
    fn file(&mut self, path: String, src: PathBuf) {
        let p = self.unique(path);
        self.entries.push(Entry::File(p, src));
    }
    /// Fichier d'upload (image d'un document, pièce jointe) cité par un texte, une seule fois par espace.
    fn upload(&mut self, dir: &str, name: &str) {
        let path = format!("{dir}/files/uploads/{name}");
        if crate::upload::is_safe_name(name) && !self.used.contains(&path.to_lowercase()) {
            let src = PathBuf::from(UPLOAD_DIR).join(name);
            if src.is_file() {
                self.file(path, src);
            }
        }
    }
}

fn write_zip(entries: Vec<Entry>, target: &std::path::Path) -> std::io::Result<()> {
    let mut zw = zip::ZipWriter::new(std::fs::File::create(target)?);
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).large_file(true);
    for e in entries {
        match e {
            Entry::Bytes(name, data) => {
                zw.start_file(name, opts).map_err(std::io::Error::other)?;
                zw.write_all(&data)?;
            }
            Entry::File(name, src) => {
                // Un fichier disparu entre-temps n'empêche pas l'export.
                let Ok(mut f) = std::fs::File::open(&src) else { continue };
                zw.start_file(name, opts).map_err(std::io::Error::other)?;
                std::io::copy(&mut f, &mut zw)?;
            }
        }
    }
    zw.finish().map_err(std::io::Error::other)?;
    Ok(())
}

/// « /uploads/x » devient « <retour>/files/uploads/x » pour que l'archive se lise hors de Looty.
fn relink(text: &str, up: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for (i, _) in text.match_indices("/uploads/") {
        let before = text[..i].chars().next_back();
        if before.is_none_or(|c| matches!(c, '(' | '"' | '\'' | ' ' | '\n' | '[')) {
            out.push_str(&text[last..i]);
            out.push_str(up);
            out.push_str("files/uploads/");
            last = i + "/uploads/".len();
        }
    }
    out.push_str(&text[last..]);
    out
}

// ---------------------------------------------------------------------------
// Contenu
// ---------------------------------------------------------------------------

struct Space {
    id: Uuid,
    name: String,
    personal: bool,
}

/// Qui exporte quoi : `space` (administrateur : tout ce qui est partagé), `own` (une personne : ses éléments
/// personnels et ceux qu'elle a écrits), `all` (son espace perso : tout).
fn mode_for(space: &Space, mode: &'static str) -> &'static str {
    if space.personal { "all" } else { mode }
}

fn own_cond(alias: &str, author: &str) -> String {
    format!("CASE $3 WHEN 'space' THEN {alias}.private_to IS NULL WHEN 'all' THEN TRUE ELSE ({alias}.private_to = $2 OR ({alias}.private_to IS NULL AND {alias}.{author} = $2)) END")
}

#[derive(sqlx::FromRow)]
struct PageRow {
    id: Uuid,
    parent_id: Option<Uuid>,
    title: String,
    content: Option<String>,
    yjs_state: Option<Vec<u8>>,
    /// Plan textuel d'une présentation (ADR 0022) : son contenu dans l'archive.
    slides_text: Option<String>,
}

/// Présentation exportée en texte : une rubrique « Diapositive n » par diapositive (Markdown, HTML).
fn slides_formats(title: &str, outline: &str) -> (String, String) {
    let md: String = outline.split("\n\n").map(|s| s.replacen("## ", "## Slide ", 1)).collect::<Vec<_>>().join("\n\n");
    let html = format!("<!doctype html>\n<html><head><meta charset=\"utf-8\"><title>{t}</title></head><body>\n<pre>{b}</pre>\n</body></html>\n", t = escape_html(title), b = escape_html(&md));
    (format!("# {title}\n\n{md}\n"), html)
}

/// (Markdown, HTML) d'une page : l'état collaboratif, ou à défaut l'ancien HTML.
fn page_formats(title: &str, yjs: Option<&[u8]>, html: Option<&str>) -> (String, String) {
    let converted = std::panic::catch_unwind(|| {
        let state = yjs.filter(|s| !s.is_empty()).map(|s| s.to_vec()).or_else(|| html.and_then(|h| crate::ai_tools::legacy_state(h).ok()));
        let md = match &state {
            Some(s) => crate::ai_pages::state_to_markdown(s).ok(),
            None => None,
        };
        let out_html = match (html.filter(|h| !h.trim().is_empty()), &state) {
            (Some(h), _) => Some(h.to_string()),
            (None, Some(s)) => crate::ai_pages::state_to_html(s).ok(),
            _ => None,
        };
        (md, out_html)
    })
    .unwrap_or((None, None));
    let html = converted.1.unwrap_or_default();
    let md = converted.0.unwrap_or_else(|| crate::ai_pages::html_to_text(&html).trim().to_string());
    (format!("# {title}\n\n{md}\n"), format!("<!doctype html>\n<html><head><meta charset=\"utf-8\"><title>{}</title></head><body>\n{html}\n</body></html>\n", escape_html(title)))
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

#[derive(Default, Serialize)]
struct Counts {
    documents: usize,
    tasks: usize,
    events: usize,
    files: usize,
    projects: usize,
}

async fn add_documents(pool: &sqlx::PgPool, ar: &mut Archive, dir: &str, space: &Space, me: Uuid, mode: &'static str) -> ApiResult<usize> {
    let sql = format!(
        "SELECT p.id, p.parent_id, p.title, p.content, p.yjs_state, p.slides_text FROM pages p WHERE p.workspace_id = $1 AND {} ORDER BY p.position",
        own_cond("p", "created_by")
    );
    let rows: Vec<PageRow> = sqlx::query_as(sqlx::AssertSqlSafe(sql)).bind(space.id).bind(me).bind(mode_for(space, mode)).fetch_all(pool).await?;
    let by_id: HashMap<Uuid, (Option<Uuid>, String)> = rows.iter().map(|r| (r.id, (r.parent_id, r.title.clone()))).collect();
    let n = rows.len();
    for r in rows {
        // Dossiers : les pages parentes qui font partie de l'export.
        let mut folders = Vec::new();
        let mut cur = r.parent_id;
        while let Some(p) = cur.filter(|_| folders.len() < 64) {
            let Some((parent, title)) = by_id.get(&p) else { break };
            folders.push(seg(title));
            cur = *parent;
        }
        folders.reverse();
        let rel = if folders.is_empty() { seg(&r.title) } else { format!("{}/{}", folders.join("/"), seg(&r.title)) };
        let (title, state, html, slides) = (r.title.clone(), r.yjs_state, r.content, r.slides_text.filter(|t| !t.trim().is_empty()));
        let (md, html_out) = tokio::task::spawn_blocking(move || match slides {
            Some(s) if state.as_deref().is_none_or(|b| b.is_empty()) && html.as_deref().is_none_or(|h| h.trim().is_empty()) => slides_formats(&title, &s),
            _ => page_formats(&title, state.as_deref(), html.as_deref()),
        })
            .await
            .map_err(|_| ApiError::Internal("export_conversion_failed".into()))?;
        let up = "../".repeat(rel.matches('/').count() + 1);
        for name in crate::storage::upload_refs(&md).into_iter().chain(crate::storage::upload_refs(&html_out)) {
            ar.upload(dir, name);
        }
        ar.bytes(format!("{dir}/documents/{rel}.md"), relink(&md, &up).into_bytes());
        ar.bytes(format!("{dir}/documents/{rel}.html"), relink(&html_out, &up).into_bytes());
    }
    Ok(n)
}

#[derive(sqlx::FromRow)]
struct TaskRow {
    id: Uuid,
    board: String,
    column_name: String,
    title: String,
    description: Option<String>,
    is_completed: bool,
    priority: i32,
    start_date: Option<DateTime<Utc>>,
    due_date: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    parent_id: Option<Uuid>,
    repeat: Option<String>,
    labels: Option<String>,
    assignees: Option<String>,
    reminders: Option<serde_json::Value>,
}

#[derive(sqlx::FromRow)]
struct CommentRow {
    task_id: Uuid,
    content: String,
    author: Option<String>,
    created_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct AttachmentRow {
    task_id: Uuid,
    filename: String,
    file_path: String,
}

const TASKS_SQL: &str = "SELECT t.id, b.name AS board, c.name AS column_name, t.title, t.description, t.is_completed, t.priority, t.start_date, t.due_date,
        t.created_at, t.updated_at, t.parent_id, t.repeat,
        (SELECT string_agg(l.name, ', ' ORDER BY l.name) FROM task_labels tl JOIN labels l ON l.id = tl.label_id WHERE tl.task_id = t.id) AS labels,
        (SELECT string_agg(u.name, ', ' ORDER BY u.name) FROM task_assignees ta JOIN users u ON u.id::text = ta.user_id WHERE ta.task_id = t.id) AS assignees,
        t.reminders
     FROM tasks t JOIN board_columns c ON c.id = t.column_id JOIN boards b ON b.id = c.board_id
     WHERE b.workspace_id = $1 AND ";

async fn add_tasks(pool: &sqlx::PgPool, ar: &mut Archive, dir: &str, space: &Space, me: Uuid, mode: &'static str) -> ApiResult<usize> {
    // Les tâches d'une personne : celles qui lui sont assignées et ses tâches personnelles.
    let cond = "CASE $3 WHEN 'space' THEN t.private_to IS NULL WHEN 'all' THEN TRUE
        ELSE (t.private_to = $2 OR (t.private_to IS NULL AND EXISTS (SELECT 1 FROM task_assignees ta WHERE ta.task_id = t.id AND ta.user_id = $2::text))) END";
    let mode = mode_for(space, mode);
    let rows: Vec<TaskRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!("{TASKS_SQL}{cond} ORDER BY b.name, c.position, t.position")))
        .bind(space.id)
        .bind(me)
        .bind(mode)
        .fetch_all(pool)
        .await?;
    if rows.is_empty() {
        return Ok(0);
    }
    let ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
    let comments: Vec<CommentRow> = sqlx::query_as(
        "SELECT c.task_id, c.content, u.name AS author, c.created_at FROM task_comments c LEFT JOIN users u ON u.id::text = c.user_id
         WHERE c.task_id = ANY($1) AND ($2 <> 'own' OR c.user_id = $3::text) ORDER BY c.created_at",
    )
    .bind(&ids)
    .bind(mode)
    .bind(me)
    .fetch_all(pool)
    .await?;
    let attachments: Vec<AttachmentRow> = sqlx::query_as("SELECT task_id, filename, file_path FROM task_attachments WHERE task_id = ANY($1) ORDER BY created_at").bind(&ids).fetch_all(pool).await?;

    let mut attached: HashMap<Uuid, Vec<serde_json::Value>> = HashMap::new();
    for a in attachments {
        let name = a.file_path.rsplit('/').next().unwrap_or("").to_string();
        if !crate::upload::is_safe_name(&name) || !PathBuf::from(UPLOAD_DIR).join(&name).is_file() {
            continue;
        }
        let path = ar.unique(format!("{dir}/files/attachments/{}-{}", &a.task_id.to_string()[..8], seg(&a.filename)));
        ar.entries.push(Entry::File(path.clone(), PathBuf::from(UPLOAD_DIR).join(&name)));
        attached.entry(a.task_id).or_default().push(serde_json::json!({ "filename": a.filename, "file": path.strip_prefix(&format!("{dir}/")).unwrap_or(&path) }));
    }
    let mut said: HashMap<Uuid, Vec<serde_json::Value>> = HashMap::new();
    for c in comments {
        for name in crate::storage::upload_refs(&c.content) {
            ar.upload(dir, name);
        }
        said.entry(c.task_id).or_default().push(serde_json::json!({ "author": c.author, "at": c.created_at, "text": c.content }));
    }

    let mut csv = String::from('\u{feff}');
    csv.push_str(&crate::board_export::line(
        &["board", "column", "title", "done", "priority", "start", "due", "labels", "assignees", "parent", "repeat", "created", "description"].map(String::from),
    ));
    let titles: HashMap<Uuid, &str> = rows.iter().map(|r| (r.id, r.title.as_str())).collect();
    let mut json = Vec::new();
    for r in &rows {
        for name in crate::storage::upload_refs(r.description.as_deref().unwrap_or("")) {
            ar.upload(dir, name);
        }
        let date = |d: &Option<DateTime<Utc>>| d.map(|d| d.to_rfc3339()).unwrap_or_default();
        csv.push_str(&crate::board_export::line(&[
            r.board.clone(),
            r.column_name.clone(),
            r.title.clone(),
            if r.is_completed { "yes" } else { "no" }.into(),
            r.priority.to_string(),
            date(&r.start_date),
            date(&r.due_date),
            r.labels.clone().unwrap_or_default(),
            r.assignees.clone().unwrap_or_default(),
            r.parent_id.and_then(|p| titles.get(&p)).map(|t| t.to_string()).unwrap_or_default(),
            r.repeat.clone().unwrap_or_default(),
            r.created_at.to_rfc3339(),
            r.description.as_deref().map(crate::ai_pages::html_to_text).unwrap_or_default().trim().to_string(),
        ]));
        json.push(serde_json::json!({
            "id": r.id, "board": r.board, "column": r.column_name, "title": r.title, "description": r.description,
            "done": r.is_completed, "priority": r.priority, "start": r.start_date, "due": r.due_date, "created": r.created_at,
            "updated": r.updated_at, "parent": r.parent_id, "repeat": r.repeat, "reminders": r.reminders,
            "labels": r.labels.as_deref().map(|l| l.split(", ").collect::<Vec<_>>()).unwrap_or_default(),
            "assignees": r.assignees.as_deref().map(|l| l.split(", ").collect::<Vec<_>>()).unwrap_or_default(),
            "comments": said.remove(&r.id).unwrap_or_default(), "attachments": attached.remove(&r.id).unwrap_or_default(),
        }));
    }
    ar.bytes(format!("{dir}/tasks.csv"), csv.into_bytes());
    ar.bytes(format!("{dir}/tasks.json"), serde_json::to_vec_pretty(&json).unwrap_or_default());
    Ok(rows.len())
}

async fn add_events(pool: &sqlx::PgPool, ar: &mut Archive, dir: &str, space: &Space, me: Uuid, mode: &'static str) -> ApiResult<usize> {
    let own = mode_for(space, mode) == "own";
    let events: Vec<String> = sqlx::query_scalar("SELECT ics FROM workspace_events WHERE workspace_id = $1 AND (NOT $3 OR created_by = $2) ORDER BY starts_at")
        .bind(space.id)
        .bind(me)
        .bind(own)
        .fetch_all(pool)
        .await?;
    if events.is_empty() {
        return Ok(0);
    }
    ar.bytes(format!("{dir}/events.ics"), crate::ics_feed::build(&space.name, &events, &[]).into_bytes());
    Ok(events.len())
}

async fn add_files(pool: &sqlx::PgPool, ar: &mut Archive, dir: &str, space: &Space, me: Uuid, mode: &'static str) -> ApiResult<(usize, usize)> {
    let sql = format!(
        "SELECT d.folder, d.name, d.blob_path FROM drive_files d WHERE d.workspace_id = $1 AND NOT d.is_dir AND d.blob_path IS NOT NULL
         AND (d.expires_at IS NULL OR d.expires_at > NOW()) AND {}",
        own_cond("d", "uploaded_by")
    );
    let mode = mode_for(space, mode);
    let drive: Vec<(String, String, String)> = sqlx::query_as(sqlx::AssertSqlSafe(sql)).bind(space.id).bind(me).bind(mode).fetch_all(pool).await?;
    let mut files = 0;
    for (folder, name, blob) in drive {
        let src = PathBuf::from(UPLOAD_DIR).join(&blob);
        if !blob_ok(&blob) || !src.is_file() {
            continue;
        }
        let folders: Vec<String> = folder.split('/').filter(|s| !s.is_empty()).map(seg).collect();
        let rel = if folders.is_empty() { seg(&name) } else { format!("{}/{}", folders.join("/"), seg(&name)) };
        ar.file(format!("{dir}/files/drive/{rel}"), src);
        files += 1;
    }

    let sql = format!("SELECT p.id, p.name FROM projects p WHERE p.workspace_id = $1 AND {} ORDER BY p.created_at", own_cond("p", "created_by"));
    let projects: Vec<(Uuid, String)> = sqlx::query_as(sqlx::AssertSqlSafe(sql)).bind(space.id).bind(me).bind(mode).fetch_all(pool).await?;
    for (id, name) in &projects {
        let root = ar.unique(format!("{dir}/projects/{}/", seg(name))).trim_end_matches('/').to_string();
        // Le dossier réserve son nom ; ses fichiers sont rangés dessous.
        let rows: Vec<(String, Option<String>, Option<String>)> = sqlx::query_as("SELECT path, content, blob_path FROM project_files WHERE project_id = $1 AND NOT is_dir ORDER BY path").bind(id).fetch_all(pool).await?;
        for (path, content, blob) in rows {
            let rel: Vec<String> = path.split('/').filter(|s| !s.is_empty()).map(seg).collect();
            if rel.is_empty() {
                continue;
            }
            let target = format!("{root}/{}", rel.join("/"));
            match (blob, content) {
                (Some(b), _) if blob_ok(&b) && PathBuf::from(UPLOAD_DIR).join(&b).is_file() => ar.file(target, PathBuf::from(UPLOAD_DIR).join(&b)),
                (None, Some(text)) => {
                    ar.bytes(target, text.into_bytes());
                }
                _ => {}
            }
        }
    }
    Ok((files, projects.len()))
}

async fn spaces_of(pool: &sqlx::PgPool, user: Uuid) -> ApiResult<Vec<Space>> {
    let rows: Vec<(Uuid, String, bool)> = sqlx::query_as(
        "SELECT w.id, w.name, w.personal_of IS NOT NULL FROM workspaces w JOIN workspace_members m ON m.workspace_id = w.id AND m.user_id = $1 ORDER BY w.personal_of IS NULL, w.name",
    )
    .bind(user.to_string())
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|(id, name, personal)| Space { id, name, personal }).collect())
}

/// Construit l'archive dans un fichier temporaire (jamais en mémoire) et renvoie son chemin.
async fn build(pool: &sqlx::PgPool, kind: &str, who: Uuid, spaces: Vec<Space>, mode: &'static str, header: serde_json::Value) -> ApiResult<PathBuf> {
    let mut ar = Archive::default();
    let mut summary = Vec::new();
    let mut dirs = HashSet::new();
    for space in &spaces {
        let mut dir = seg(&space.name);
        if !dirs.insert(dir.to_lowercase()) {
            dir = format!("{dir}-{}", &space.id.to_string()[..8]);
            dirs.insert(dir.to_lowercase());
        }
        let c = Counts {
            documents: add_documents(pool, &mut ar, &dir, space, who, mode).await?,
            tasks: add_tasks(pool, &mut ar, &dir, space, who, mode).await?,
            events: add_events(pool, &mut ar, &dir, space, who, mode).await?,
            ..Default::default()
        };
        let (files, projects) = add_files(pool, &mut ar, &dir, space, who, mode).await?;
        summary.push(serde_json::json!({ "folder": dir, "name": space.name, "personal": space.personal, "counts": Counts { files, projects, ..c } }));
    }
    let manifest = serde_json::json!({
        "format": "looty-export/1", "kind": kind, "exported_at": Utc::now(), "looty_version": env!("CARGO_PKG_VERSION"),
        "about": "documents/ holds Markdown and HTML (images under files/uploads/), tasks.csv and tasks.json the tasks, events.ics the calendar, files/ and projects/ the files.",
        "workspaces": summary, "info": header,
    });
    ar.entries.insert(0, Entry::Bytes("manifest.json".into(), serde_json::to_vec_pretty(&manifest).unwrap_or_default()));
    let dir = PathBuf::from(UPLOAD_DIR).join(".exports");
    tokio::fs::create_dir_all(&dir).await.map_err(|e| ApiError::Internal(format!("export dir: {e}")))?;
    let target = dir.join(format!("{}.zip", Uuid::new_v4()));
    let t2 = target.clone();
    let res = tokio::task::spawn_blocking(move || write_zip(ar.entries, &t2)).await;
    match res {
        Ok(Ok(())) => Ok(target),
        Ok(Err(e)) => {
            let _ = tokio::fs::remove_file(&target).await;
            Err(ApiError::Internal(format!("export zip: {e}")))
        }
        Err(_) => {
            let _ = tokio::fs::remove_file(&target).await;
            Err(ApiError::Internal("export_failed".into()))
        }
    }
}

/// Réponse qui envoie le zip puis le supprime : le fichier est détaché dès l'ouverture.
async fn serve_zip(path: PathBuf, filename: &str) -> ApiResult<Response> {
    let file = tokio::fs::File::open(&path).await.map_err(|e| ApiError::Internal(format!("export open: {e}")))?;
    let len = file.metadata().await.map(|m| m.len()).unwrap_or(0);
    let _ = tokio::fs::remove_file(&path).await;
    let mut resp = Response::new(Body::from_stream(tokio_util::io::ReaderStream::new(file)));
    let h = resp.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/zip"));
    h.insert(header::CONTENT_LENGTH, HeaderValue::from(len));
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("private, no-store"));
    h.insert(header::CONTENT_DISPOSITION, HeaderValue::from_str(&format!("attachment; filename=\"{filename}\"")).unwrap_or(HeaderValue::from_static("attachment")));
    Ok(resp)
}

/// Deux exports à la fois au plus : l'archive est lourde à construire.
fn permit() -> ApiResult<tokio::sync::OwnedSemaphorePermit> {
    static S: OnceLock<std::sync::Arc<Semaphore>> = OnceLock::new();
    S.get_or_init(|| std::sync::Arc::new(Semaphore::new(2))).clone().try_acquire_owned().map_err(|_| ApiError::Conflict("export_busy".into()))
}

fn ascii_name(s: &str) -> String {
    let n: String = s.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
    let n = n.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    if n.is_empty() { "space".into() } else { n.chars().take(40).collect() }
}

/// `GET /api/user/export` : l'archive de la personne.
async fn export_account(State(state): State<AppState>, user: AuthUser) -> ApiResult<Response> {
    let _permit = permit()?;
    let (name, email, created, locale, tz, prefs): (String, String, DateTime<Utc>, Option<String>, Option<String>, serde_json::Value) =
        sqlx::query_as("SELECT name, email, created_at, locale, timezone, prefs FROM users WHERE id = $1").bind(user.id).fetch_one(&state.pool).await?;
    let spaces = spaces_of(&state.pool, user.id).await?;
    let info = serde_json::json!({ "account": { "name": name, "email": email, "created_at": created, "locale": locale, "timezone": tz, "preferences": prefs } });
    let path = build(&state.pool, "account", user.id, spaces, "own", info).await?;
    serve_zip(path, &format!("looty-account-{}.zip", Utc::now().format("%Y%m%d"))).await
}

/// `GET /api/workspaces/{id}/export` : tout ce qui est partagé dans l'espace (administrateurs de l'espace).
async fn export_space(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>) -> ApiResult<Response> {
    require_ws_admin(&state.pool, &user, ws).await?;
    let (name, personal): (String, bool) = sqlx::query_as("SELECT name, personal_of IS NOT NULL FROM workspaces WHERE id = $1").bind(ws).fetch_optional(&state.pool).await?.ok_or(ApiError::NotFound)?;
    // L'espace perso est à son propriétaire : il l'exporte avec son compte.
    if personal {
        return Err(ApiError::Forbidden);
    }
    let _permit = permit()?;
    let spaces = vec![Space { id: ws, name: name.clone(), personal: false }];
    let path = build(&state.pool, "space", user.id, spaces, "space", serde_json::json!({ "space": name })).await?;
    serve_zip(path, &format!("looty-{}-{}.zip", ascii_name(&name), Utc::now().format("%Y%m%d"))).await
}

// ---------------------------------------------------------------------------
// Suppression du compte
// ---------------------------------------------------------------------------

/// Délai de grâce, en jours (`ACCOUNT_DELETION_GRACE_DAYS`, 14 par défaut, de 1 à 90).
fn grace_days() -> i64 {
    std::env::var("ACCOUNT_DELETION_GRACE_DAYS").ok().and_then(|v| v.trim().parse::<i64>().ok()).unwrap_or(14).clamp(1, 90)
}

#[derive(Serialize, sqlx::FromRow)]
struct Blocker {
    id: Uuid,
    name: String,
}

/// Espaces partagés dont la personne est la seule propriétaire alors que d'autres y sont membres : elle doit
/// d'abord transmettre la propriété (ou les supprimer), sinon ils n'auraient plus personne pour les administrer.
async fn blockers(pool: &sqlx::PgPool, user: Uuid) -> ApiResult<Vec<Blocker>> {
    Ok(sqlx::query_as(
        "SELECT w.id, w.name FROM workspaces w JOIN workspace_members m ON m.workspace_id = w.id AND m.user_id = $1 AND m.role = 'owner'
         WHERE w.personal_of IS NULL
           AND NOT EXISTS (SELECT 1 FROM workspace_members o WHERE o.workspace_id = w.id AND o.role = 'owner' AND o.user_id <> $1)
           AND EXISTS (SELECT 1 FROM workspace_members o WHERE o.workspace_id = w.id AND o.user_id <> $1)
         ORDER BY w.name",
    )
    .bind(user.to_string())
    .fetch_all(pool)
    .await?)
}

async fn last_super_admin(pool: &sqlx::PgPool, user: &AuthUser) -> ApiResult<bool> {
    if !user.is_super_admin {
        return Ok(false);
    }
    let others: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE is_super_admin AND NOT is_suspended AND id <> $1").bind(user.id).fetch_one(pool).await?;
    Ok(others == 0)
}

async fn deletion_info(pool: &sqlx::PgPool, user: &AuthUser) -> ApiResult<serde_json::Value> {
    let scheduled: Option<DateTime<Utc>> = sqlx::query_scalar("SELECT deletion_scheduled_at FROM users WHERE id = $1").bind(user.id).fetch_one(pool).await?;
    Ok(serde_json::json!({
        "scheduled_at": scheduled, "grace_days": grace_days(),
        "blockers": blockers(pool, user.id).await?, "last_super_admin": last_super_admin(pool, user).await?,
    }))
}

async fn deletion_status(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(deletion_info(&state.pool, &user).await?))
}

#[derive(Deserialize)]
struct DeletionReq {
    /// L'adresse e-mail du compte, retapée pour confirmer.
    email: String,
}

async fn request_deletion(State(state): State<AppState>, user: AuthUser, Json(p): Json<DeletionReq>) -> ApiResult<Json<serde_json::Value>> {
    if state.demo {
        return Err(ApiError::DemoUnavailable);
    }
    let email: String = sqlx::query_scalar("SELECT email FROM users WHERE id = $1").bind(user.id).fetch_one(&state.pool).await?;
    if !email.eq_ignore_ascii_case(p.email.trim()) {
        return Err(ApiError::bad_request("email_mismatch"));
    }
    if !blockers(&state.pool, user.id).await?.is_empty() {
        return Err(ApiError::Conflict("sole_owner".into()));
    }
    if last_super_admin(&state.pool, &user).await? {
        return Err(ApiError::Conflict("last_super_admin".into()));
    }
    // Une demande déjà faite garde sa date.
    sqlx::query("UPDATE users SET deletion_scheduled_at = COALESCE(deletion_scheduled_at, NOW() + make_interval(days => $2::int)) WHERE id = $1")
        .bind(user.id)
        .bind(grace_days() as i32)
        .execute(&state.pool)
        .await?;
    Ok(Json(deletion_info(&state.pool, &user).await?))
}

async fn cancel_deletion(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<serde_json::Value>> {
    sqlx::query("UPDATE users SET deletion_scheduled_at = NULL WHERE id = $1").bind(user.id).execute(&state.pool).await?;
    Ok(Json(deletion_info(&state.pool, &user).await?))
}

/// Supprime le compte : voir l'ADR 0013. Sans effet (et `Conflict`) si la personne est encore la seule propriétaire
/// d'un espace partagé.
pub async fn erase_user(pool: &sqlx::PgPool, id: Uuid) -> ApiResult<()> {
    if id.to_string() == DELETED_USER {
        return Err(ApiError::Forbidden);
    }
    if !blockers(pool, id).await?.is_empty() {
        return Err(ApiError::Conflict("sole_owner".into()));
    }
    let uid = id.to_string();
    // Espaces qui disparaissent : l'espace perso et les espaces partagés où elle est seule.
    let doomed: Vec<Uuid> = sqlx::query_scalar(
        "SELECT w.id FROM workspaces w WHERE w.personal_of = $1
         OR (w.personal_of IS NULL AND EXISTS (SELECT 1 FROM workspace_members m WHERE m.workspace_id = w.id AND m.user_id = $2)
             AND NOT EXISTS (SELECT 1 FROM workspace_members o WHERE o.workspace_id = w.id AND o.user_id <> $2))",
    )
    .bind(id)
    .bind(&uid)
    .fetch_all(pool)
    .await?;
    // Fichiers à retirer du disque une fois la base nettoyée.
    let mut names = crate::storage::files_only_in(pool, &doomed).await?;
    let mut blobs: Vec<String> = sqlx::query_scalar(
        "SELECT blob_path FROM drive_files WHERE blob_path IS NOT NULL AND (private_to = $1 OR workspace_id = ANY($2))
         UNION SELECT f.blob_path FROM project_files f JOIN projects p ON p.id = f.project_id WHERE f.blob_path IS NOT NULL AND (p.private_to = $1 OR p.workspace_id = ANY($2))",
    )
    .bind(id)
    .bind(&doomed)
    .fetch_all(pool)
    .await?;
    // L'avatar est propre à la personne.
    let avatar: Option<String> = sqlx::query_scalar("SELECT avatar_url FROM users WHERE id = $1").bind(id).fetch_optional(pool).await?.flatten();
    names.extend(crate::storage::upload_refs(avatar.as_deref().unwrap_or("")).into_iter().map(String::from));

    let mut tx = pool.begin().await?;
    sqlx::query("INSERT INTO users (id, email, name, is_suspended) VALUES ($1, 'deleted-user@looty.invalid', 'Deleted user', TRUE) ON CONFLICT (id) DO NOTHING")
        .bind(Uuid::nil())
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM workspaces WHERE id = ANY($1)").bind(&doomed).execute(&mut *tx).await?;
    for sql in [
        "DELETE FROM workspace_members WHERE user_id = $1",
        "DELETE FROM task_assignees WHERE user_id = $1",
        "DELETE FROM task_validations WHERE user_id = $1",
        "UPDATE task_comments SET user_id = '00000000-0000-0000-0000-000000000000' WHERE user_id = $1",
    ] {
        sqlx::query(sql).bind(&uid).execute(&mut *tx).await?;
    }
    // Les documents partagés gardent leur place (auteur anonymisé) ; les éléments personnels partent avec le compte
    // (clé étrangère), mais leur auteur ne peut pas devenir « nul » avant (colonne obligatoire).
    sqlx::query("UPDATE pages_all SET created_by = $2 WHERE created_by = $1").bind(id).bind(Uuid::nil()).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM users WHERE id = $1").bind(id).execute(&mut *tx).await?;
    tx.commit().await?;

    let _ = sqlx::query("SELECT pg_notify('looty_access', '')").execute(pool).await;
    for n in names {
        if crate::upload::is_safe_name(&n) {
            let _ = tokio::fs::remove_file(PathBuf::from(UPLOAD_DIR).join(&n)).await;
        }
    }
    blobs.sort();
    blobs.dedup();
    for b in blobs.into_iter().filter(|b| blob_ok(b)) {
        let _ = tokio::fs::remove_file(PathBuf::from(UPLOAD_DIR).join(&b)).await;
    }
    for ws in doomed {
        let _ = tokio::fs::remove_dir_all(PathBuf::from(UPLOAD_DIR).join("drive").join(ws.to_string())).await;
    }
    tracing::info!("account {id} deleted");
    Ok(())
}

/// Supprime, toutes les heures, les comptes dont le délai de grâce est écoulé. Nettoie aussi les archives
/// d'export laissées par un arrêt brutal.
pub fn start_deletion_loop(state: AppState) {
    tokio::spawn(async move {
        let _ = tokio::fs::remove_dir_all(PathBuf::from(UPLOAD_DIR).join(".exports")).await;
        loop {
            crate::bg::tick("account deletion");
            let due: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM users WHERE deletion_scheduled_at IS NOT NULL AND deletion_scheduled_at <= NOW() ORDER BY deletion_scheduled_at LIMIT 20")
                .fetch_all(&state.pool)
                .await
                .unwrap_or_default();
            for id in due {
                match crate::bg::guard_result("account deletion", erase_user(&state.pool, id)).await {
                    Some(Err(ApiError::Conflict(_))) => {
                        // Devenue seule propriétaire d'un espace entre-temps : la demande est annulée, pas la personne.
                        let _ = sqlx::query("UPDATE users SET deletion_scheduled_at = NULL WHERE id = $1").bind(id).execute(&state.pool).await;
                    }
                    Some(Err(e)) => tracing::warn!("account deletion {id}: {e:?}"),
                    _ => {}
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_cannot_climb_out_of_the_archive() {
        assert_eq!(seg("a/b\\c:d"), "a_b_c_d");
        assert_eq!(seg(".."), "untitled");
        assert_eq!(seg("  "), "untitled");
        assert_eq!(seg("TP 1: réseaux?"), "TP 1_ réseaux_");
        assert!(blob_ok("drive/abc/def") && !blob_ok("../x") && !blob_ok("/etc/passwd") && !blob_ok("a/../b") && !blob_ok(""));
    }

    #[test]
    fn taken_names_get_a_number() {
        let mut ar = Archive::default();
        assert_eq!(ar.unique("d/Notes.md".into()), "d/Notes.md");
        assert_eq!(ar.unique("d/notes.md".into()), "d/notes (2).md");
        assert_eq!(ar.unique("d/Notes.md".into()), "d/Notes (3).md");
        assert_eq!(ar.unique("d.v1/Makefile".into()), "d.v1/Makefile");
        assert_eq!(ar.unique("d.v1/Makefile".into()), "d.v1/Makefile (2)");
    }

    #[test]
    fn upload_links_become_relative_to_the_document() {
        let md = "![a](/uploads/x.png) and https://other.org/uploads/y.png and <img src=\"/uploads/z.png\">";
        assert_eq!(
            relink(md, "../"),
            "![a](../files/uploads/x.png) and https://other.org/uploads/y.png and <img src=\"../files/uploads/z.png\">"
        );
        assert_eq!(relink("/uploads/a", ""), "files/uploads/a");
    }

    #[test]
    fn a_page_is_exported_as_markdown_and_html() {
        let (md, html) = page_formats("Cours <1>", None, Some("<h2>Intro</h2><p>Bonjour <strong>toi</strong></p>"));
        assert!(md.starts_with("# Cours <1>\n\n"), "{md}");
        assert!(md.contains("Intro") && md.contains("Bonjour"), "{md}");
        assert!(html.contains("<title>Cours &lt;1&gt;</title>") && html.contains("<h2>Intro</h2>"));
    }

    #[test]
    fn a_presentation_is_exported_as_a_slide_outline() {
        let (md, html) = slides_formats("Deck <1>", "## 1\nHello\n\n## 2\nWorld");
        assert_eq!(md, "# Deck <1>\n\n## Slide 1\nHello\n\n## Slide 2\nWorld\n");
        assert!(html.contains("<title>Deck &lt;1&gt;</title>") && html.contains("## Slide 2"));
    }

    #[test]
    fn the_zip_holds_what_was_added() {
        let dir = std::env::temp_dir().join(format!("looty-zip-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("src.txt");
        std::fs::write(&src, "from disk").unwrap();
        let mut ar = Archive::default();
        ar.bytes("a/b.md".into(), b"hello".to_vec());
        ar.file("a/c.txt".into(), src);
        ar.file("a/gone.txt".into(), dir.join("missing"));
        let out = dir.join("out.zip");
        write_zip(ar.entries, &out).unwrap();
        let mut z = zip::ZipArchive::new(std::fs::File::open(&out).unwrap()).unwrap();
        let mut names: Vec<String> = (0..z.len()).map(|i| z.by_index(i).unwrap().name().unwrap().to_string()).collect();
        names.sort();
        assert_eq!(names, vec!["a/b.md", "a/c.txt"]);
        let mut s = String::new();
        std::io::Read::read_to_string(&mut z.by_name("a/c.txt").unwrap(), &mut s).unwrap();
        assert_eq!(s, "from disk");
        std::fs::remove_dir_all(dir).unwrap();
    }

    async fn user(pool: &sqlx::PgPool, name: &str) -> Uuid {
        sqlx::query_scalar("INSERT INTO users (email, name) VALUES ($1, $2) RETURNING id").bind(format!("{name}@example.org")).bind(name).fetch_one(pool).await.unwrap()
    }

    async fn space(pool: &sqlx::PgPool, name: &str, personal_of: Option<Uuid>) -> Uuid {
        sqlx::query_scalar("INSERT INTO workspaces (name, personal_of) VALUES ($1, $2) RETURNING id").bind(name).bind(personal_of).fetch_one(pool).await.unwrap()
    }

    async fn join(pool: &sqlx::PgPool, ws: Uuid, user: Uuid, role: &str) {
        sqlx::query("INSERT INTO workspace_members (workspace_id, user_id, role) VALUES ($1, $2, $3)").bind(ws).bind(user.to_string()).bind(role).execute(pool).await.unwrap();
    }

    async fn page(pool: &sqlx::PgPool, ws: Uuid, by: Uuid, title: &str, private_to: Option<Uuid>) -> Uuid {
        sqlx::query_scalar("INSERT INTO pages (workspace_id, title, content, created_by, private_to) VALUES ($1, $2, $3, $4, $5) RETURNING id")
            .bind(ws)
            .bind(title)
            .bind(format!("<p>{title} text</p>"))
            .bind(by)
            .bind(private_to)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    async fn task(pool: &sqlx::PgPool, ws: Uuid, title: &str, assignee: Option<Uuid>) -> Uuid {
        let board: Uuid = sqlx::query_scalar("INSERT INTO boards (workspace_id, name) VALUES ($1, $2) RETURNING id").bind(ws).bind(format!("Board of {title}")).fetch_one(pool).await.unwrap();
        let col: Uuid = sqlx::query_scalar("INSERT INTO board_columns (board_id, name, position) VALUES ($1, 'To do', 1) RETURNING id").bind(board).fetch_one(pool).await.unwrap();
        let t: Uuid = sqlx::query_scalar("INSERT INTO tasks (column_id, title, position) VALUES ($1, $2, 1) RETURNING id").bind(col).bind(title).fetch_one(pool).await.unwrap();
        if let Some(a) = assignee {
            sqlx::query("INSERT INTO task_assignees (task_id, user_id) VALUES ($1, $2)").bind(t).bind(a.to_string()).execute(pool).await.unwrap();
        }
        t
    }

    #[sqlx::test]
    async fn deleting_an_account_keeps_shared_work_and_drops_personal_items(pool: sqlx::PgPool) {
        let (ana, ben) = (user(&pool, "ana").await, user(&pool, "ben").await);
        let team = space(&pool, "Team", None).await;
        join(&pool, team, ana, "owner").await;
        join(&pool, team, ben, "owner").await;
        let mine = space(&pool, "Perso", Some(ana)).await;
        join(&pool, mine, ana, "owner").await;
        let alone = space(&pool, "Alone", None).await;
        join(&pool, alone, ana, "owner").await;
        let shared = page(&pool, team, ana, "Shared", None).await;
        let secret = page(&pool, team, ana, "Secret", Some(ana)).await;
        let in_perso = page(&pool, mine, ana, "Diary", None).await;
        let t = task(&pool, team, "Task", Some(ana)).await;
        sqlx::query("INSERT INTO task_comments (task_id, user_id, content) VALUES ($1, $2, 'hi')").bind(t).bind(ana.to_string()).execute(&pool).await.unwrap();

        erase_user(&pool, ana).await.unwrap();

        let author: Uuid = sqlx::query_scalar("SELECT created_by FROM pages WHERE id = $1").bind(shared).fetch_one(&pool).await.unwrap();
        assert_eq!(author.to_string(), DELETED_USER, "the shared page stays, its author is anonymised");
        for gone in [secret, in_perso] {
            let n: i64 = sqlx::query_scalar("SELECT count(*) FROM pages_all WHERE id = $1").bind(gone).fetch_one(&pool).await.unwrap();
            assert_eq!(n, 0);
        }
        let spaces: i64 = sqlx::query_scalar("SELECT count(*) FROM workspaces WHERE id = ANY($1)").bind(vec![mine, alone]).fetch_one(&pool).await.unwrap();
        assert_eq!(spaces, 0, "the personal space and the space she was alone in are deleted");
        let team_left: i64 = sqlx::query_scalar("SELECT count(*) FROM workspaces WHERE id = $1").bind(team).fetch_one(&pool).await.unwrap();
        assert_eq!(team_left, 1);
        let who: String = sqlx::query_scalar("SELECT user_id FROM task_comments WHERE task_id = $1").bind(t).fetch_one(&pool).await.unwrap();
        assert_eq!(who, DELETED_USER);
        let assigned: i64 = sqlx::query_scalar("SELECT count(*) FROM task_assignees WHERE task_id = $1").bind(t).fetch_one(&pool).await.unwrap();
        assert_eq!(assigned, 0);
        let users: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE id = ANY($1)").bind(vec![ana, ben]).fetch_one(&pool).await.unwrap();
        assert_eq!(users, 1);
        assert!(matches!(erase_user(&pool, Uuid::nil()).await, Err(ApiError::Forbidden)));
    }

    #[sqlx::test]
    async fn the_only_owner_of_a_space_with_other_members_must_hand_it_over_first(pool: sqlx::PgPool) {
        let (ana, ben) = (user(&pool, "ana").await, user(&pool, "ben").await);
        let team = space(&pool, "Team", None).await;
        join(&pool, team, ana, "owner").await;
        join(&pool, team, ben, "member").await;
        assert_eq!(blockers(&pool, ana).await.unwrap().len(), 1);
        assert!(matches!(erase_user(&pool, ana).await, Err(ApiError::Conflict(_))));
        sqlx::query("UPDATE workspace_members SET role = 'owner' WHERE user_id = $1").bind(ben.to_string()).execute(&pool).await.unwrap();
        assert!(blockers(&pool, ana).await.unwrap().is_empty());
        erase_user(&pool, ana).await.unwrap();
        let left: i64 = sqlx::query_scalar("SELECT count(*) FROM workspaces WHERE id = $1").bind(team).fetch_one(&pool).await.unwrap();
        assert_eq!(left, 1);
    }

    async fn names_in(zip_path: &std::path::Path) -> Vec<String> {
        let mut z = zip::ZipArchive::new(std::fs::File::open(zip_path).unwrap()).unwrap();
        (0..z.len()).map(|i| z.by_index(i).unwrap().name().unwrap().to_string()).collect()
    }

    #[sqlx::test]
    async fn an_account_export_holds_the_persons_items_and_nobody_elses(pool: sqlx::PgPool) {
        let (ana, ben) = (user(&pool, "ana").await, user(&pool, "ben").await);
        let team = space(&pool, "Team", None).await;
        join(&pool, team, ana, "member").await;
        join(&pool, team, ben, "owner").await;
        let mine = space(&pool, "Perso", Some(ana)).await;
        join(&pool, mine, ana, "owner").await;
        page(&pool, team, ana, "Ana shared", None).await;
        page(&pool, team, ana, "Ana private", Some(ana)).await;
        page(&pool, team, ben, "Ben shared", None).await;
        page(&pool, team, ben, "Ben private", Some(ben)).await;
        page(&pool, mine, ana, "Diary", None).await;
        task(&pool, team, "Ana task", Some(ana)).await;
        task(&pool, team, "Ben task", Some(ben)).await;

        let path = build(&pool, "account", ana, spaces_of(&pool, ana).await.unwrap(), "own", serde_json::json!({})).await.unwrap();
        let names = names_in(&path).await;
        let _ = std::fs::remove_file(&path);
        assert!(names.contains(&"manifest.json".to_string()));
        for expected in ["Team/documents/Ana shared.md", "Team/documents/Ana shared.html", "Team/documents/Ana private.md", "Perso/documents/Diary.md", "Team/tasks.csv", "Team/tasks.json"] {
            assert!(names.contains(&expected.to_string()), "{expected} missing from {names:?}");
        }
        assert!(!names.iter().any(|n| n.contains("Ben")), "{names:?}");
    }

    #[sqlx::test]
    async fn a_space_export_leaves_out_personal_items(pool: sqlx::PgPool) {
        let (ana, ben) = (user(&pool, "ana").await, user(&pool, "ben").await);
        let team = space(&pool, "Team", None).await;
        join(&pool, team, ana, "owner").await;
        join(&pool, team, ben, "member").await;
        let parent = page(&pool, team, ana, "Course", None).await;
        let child = page(&pool, team, ben, "Lab", None).await;
        sqlx::query("UPDATE pages SET parent_id = $2 WHERE id = $1").bind(child).bind(parent).execute(&pool).await.unwrap();
        page(&pool, team, ben, "Ben private", Some(ben)).await;

        let members = vec![Space { id: team, name: "Team".into(), personal: false }];
        let path = build(&pool, "space", ana, members, "space", serde_json::json!({})).await.unwrap();
        let names = names_in(&path).await;
        let _ = std::fs::remove_file(&path);
        assert!(names.contains(&"Team/documents/Course.md".to_string()) && names.contains(&"Team/documents/Course/Lab.md".to_string()), "{names:?}");
        assert!(!names.iter().any(|n| n.contains("Ben private")), "{names:?}");
    }

    #[test]
    fn the_grace_period_is_bounded() {
        assert_eq!(ascii_name("Promo Info 2026 — TP"), "promo-info-2026-tp");
        assert_eq!(ascii_name("—"), "space");
        assert!((1..=90).contains(&grace_days()));
    }
}

//! Stockage (administration) : taille réelle du dossier d'upload, espace disque, usage de
//! chaque fichier (pièce jointe, document, description, commentaire, avatar, logo) et
//! fichiers orphelins, que l'on peut supprimer.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

use crate::access::AuthUser;
use crate::admin::require_super_admin;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use crate::upload::UPLOAD_DIR;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(overview))
        .route("/orphans", delete(delete_orphans))
        .route("/files/{name}", delete(delete_file))
}

/// Où un fichier est utilisé.
#[derive(Serialize, Clone)]
struct Usage {
    /// attachment | page | task | comment | avatar | branding
    kind: &'static str,
    label: String,
    workspace_id: Option<Uuid>,
    /// Identifiants pour construire le lien (tâche + tableau, ou page).
    target_id: Option<Uuid>,
    board_id: Option<Uuid>,
    in_trash: bool,
}

struct FileInfo {
    name: String,
    size: u64,
    modified: Option<DateTime<Utc>>,
}

fn list_files() -> Vec<FileInfo> {
    let Ok(dir) = std::fs::read_dir(UPLOAD_DIR) else { return vec![] };
    dir.filter_map(|e| e.ok())
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            if !meta.is_file() {
                return None;
            }
            Some(FileInfo {
                name: e.file_name().to_string_lossy().into_owned(),
                size: meta.len(),
                modified: meta.modified().ok().map(DateTime::<Utc>::from),
            })
        })
        .collect()
}

/// Noms de fichiers cités dans un texte (« /uploads/<nom> »).
fn upload_refs(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find("/uploads/") {
        let tail = &rest[i + 9..];
        let end = tail.find(|c: char| !(c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')).unwrap_or(tail.len());
        if end > 0 {
            out.push(&tail[..end]);
        }
        rest = &tail[end..];
    }
    out
}

fn file_type(name: &str) -> &'static str {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "avif" | "bmp" | "ico" | "heic" => "image",
        "pdf" => "pdf",
        "mp4" | "webm" | "mov" | "mkv" | "avi" => "video",
        "mp3" | "ogg" | "wav" | "m4a" | "flac" | "opus" => "audio",
        "zip" | "tar" | "gz" | "7z" | "rar" | "xz" => "archive",
        "doc" | "docx" | "odt" | "xls" | "xlsx" | "ods" | "ppt" | "pptx" | "odp" | "txt" | "md" | "csv" => "document",
        _ => "other",
    }
}

/// Recense toutes les références aux fichiers d'upload.
async fn collect_usages(pool: &sqlx::PgPool) -> ApiResult<HashMap<String, Vec<Usage>>> {
    let mut map: HashMap<String, Vec<Usage>> = HashMap::new();
    let mut add = |name: &str, u: Usage| map.entry(name.to_string()).or_default().push(u);

    let rows: Vec<(String, String, Uuid, Uuid, Uuid, bool)> = sqlx::query_as(
        "SELECT ta.file_path, t.title, t.id, b.id, b.workspace_id, t.deleted_at IS NOT NULL
         FROM task_attachments ta JOIN tasks_all t ON t.id = ta.task_id
         JOIN board_columns c ON c.id = t.column_id JOIN boards b ON b.id = c.board_id",
    )
    .fetch_all(pool)
    .await?;
    for (path, title, task, board, ws, trash) in rows {
        let name = path.rsplit('/').next().unwrap_or(&path).to_string();
        add(&name, Usage { kind: "attachment", label: title, workspace_id: Some(ws), target_id: Some(task), board_id: Some(board), in_trash: trash });
    }

    // Documents : HTML, icône, et état collaboratif (Yjs) — certaines pages n'ont pas (ou
    // pas encore) de copie HTML ; les chaînes y sont stockées en UTF-8 lisible.
    let rows: Vec<(Uuid, Uuid, String, Option<String>, Option<String>, Option<Vec<u8>>, bool)> = sqlx::query_as(
        "SELECT id, workspace_id, title, content, icon, yjs_state, deleted_at IS NOT NULL FROM pages_all
         WHERE content LIKE '%/uploads/%' OR icon LIKE '%/uploads/%' OR position('/uploads/'::bytea in yjs_state) > 0",
    )
    .fetch_all(pool)
    .await?;
    for (id, ws, title, content, icon, yjs, trash) in rows {
        let yjs = yjs.map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
        let mut names: Vec<&str> = upload_refs(content.as_deref().unwrap_or(""));
        names.extend(upload_refs(icon.as_deref().unwrap_or("")));
        names.extend(upload_refs(&yjs));
        names.sort_unstable();
        names.dedup();
        for name in names {
            add(name, Usage { kind: "page", label: title.clone(), workspace_id: Some(ws), target_id: Some(id), board_id: None, in_trash: trash });
        }
    }

    let rows: Vec<(Uuid, String, Option<String>, Uuid, Uuid, bool)> = sqlx::query_as(
        "SELECT t.id, t.title, t.description, b.id, b.workspace_id, t.deleted_at IS NOT NULL
         FROM tasks_all t JOIN board_columns c ON c.id = t.column_id JOIN boards b ON b.id = c.board_id
         WHERE t.description LIKE '%/uploads/%'",
    )
    .fetch_all(pool)
    .await?;
    for (id, title, desc, board, ws, trash) in rows {
        for name in upload_refs(desc.as_deref().unwrap_or("")) {
            add(name, Usage { kind: "task", label: title.clone(), workspace_id: Some(ws), target_id: Some(id), board_id: Some(board), in_trash: trash });
        }
    }

    let rows: Vec<(String, Uuid, String, Uuid, Uuid, bool)> = sqlx::query_as(
        "SELECT cm.content, t.id, t.title, b.id, b.workspace_id, t.deleted_at IS NOT NULL
         FROM task_comments cm JOIN tasks_all t ON t.id = cm.task_id
         JOIN board_columns c ON c.id = t.column_id JOIN boards b ON b.id = c.board_id
         WHERE cm.content LIKE '%/uploads/%'",
    )
    .fetch_all(pool)
    .await?;
    for (content, id, title, board, ws, trash) in rows {
        for name in upload_refs(&content) {
            add(name, Usage { kind: "comment", label: title.clone(), workspace_id: Some(ws), target_id: Some(id), board_id: Some(board), in_trash: trash });
        }
    }

    let rows: Vec<(String, String)> = sqlx::query_as("SELECT name, avatar_url FROM users WHERE avatar_url LIKE '/uploads/%'").fetch_all(pool).await?;
    for (user, url) in rows {
        for name in upload_refs(&url) {
            add(name, Usage { kind: "avatar", label: user.clone(), workspace_id: None, target_id: None, board_id: None, in_trash: false });
        }
    }
    let favicon: Option<String> = sqlx::query_scalar("SELECT favicon_url FROM global_settings WHERE id = 1").fetch_optional(pool).await?.flatten();
    for name in upload_refs(favicon.as_deref().unwrap_or("")) {
        add(name, Usage { kind: "branding", label: "Favicon".into(), workspace_id: None, target_id: None, board_id: None, in_trash: false });
    }
    Ok(map)
}

#[derive(Serialize)]
struct Bucket {
    key: &'static str,
    bytes: u64,
    count: u64,
}

#[derive(Serialize)]
struct FileDto {
    name: String,
    size: u64,
    modified: Option<DateTime<Utc>>,
    file_type: &'static str,
    usages: Vec<Usage>,
}

fn disk_space(path: &str) -> Option<(u64, u64)> {
    let c = std::ffi::CString::new(path).ok()?;
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY : chemin C valide, structure initialisée par l'appel.
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        return None;
    }
    let block = s.f_frsize as u64;
    Some((s.f_blocks as u64 * block, s.f_bavail as u64 * block))
}

async fn overview(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<serde_json::Value>> {
    require_super_admin(&user)?;
    let usages = collect_usages(&state.pool).await?;
    let files = tokio::task::spawn_blocking(list_files).await.map_err(|e| ApiError::Internal(e.to_string()))?;

    let mut total = 0u64;
    let mut by_usage: HashMap<&'static str, (u64, u64)> = HashMap::new();
    let mut by_type: HashMap<&'static str, (u64, u64)> = HashMap::new();
    let mut orphans: Vec<FileDto> = Vec::new();
    let mut all: Vec<FileDto> = Vec::with_capacity(files.len());
    for f in files {
        total += f.size;
        let u = usages.get(&f.name).cloned().unwrap_or_default();
        // Rangé selon son premier usage actif (sinon « corbeille », sinon « orphelin »).
        let key = match u.iter().find(|x| !x.in_trash) {
            Some(x) => x.kind,
            None if !u.is_empty() => "trash",
            None => "orphan",
        };
        let e = by_usage.entry(key).or_default();
        e.0 += f.size;
        e.1 += 1;
        let t = file_type(&f.name);
        let e = by_type.entry(t).or_default();
        e.0 += f.size;
        e.1 += 1;
        let dto = FileDto { name: f.name, size: f.size, modified: f.modified, file_type: t, usages: u };
        if key == "orphan" {
            orphans.push(FileDto { name: dto.name.clone(), size: dto.size, modified: dto.modified, file_type: t, usages: vec![] });
        }
        all.push(dto);
    }
    let file_count = all.len();
    all.sort_by(|a, b| b.size.cmp(&a.size));
    all.truncate(100);
    orphans.sort_by(|a, b| b.size.cmp(&a.size));
    let orphan_bytes: u64 = orphans.iter().map(|o| o.size).sum();
    let orphan_count = orphans.len();
    orphans.truncate(200);

    let bucket = |m: HashMap<&'static str, (u64, u64)>| {
        let mut v: Vec<Bucket> = m.into_iter().map(|(key, (bytes, count))| Bucket { key, bytes, count }).collect();
        v.sort_by(|a, b| b.bytes.cmp(&a.bytes));
        v
    };
    let disk = disk_space(UPLOAD_DIR);
    let db_size: i64 = sqlx::query_scalar("SELECT pg_database_size(current_database())").fetch_one(&state.pool).await.unwrap_or(0);
    // Ce que la base croit stocker (pièces jointes), pour comparaison avec le disque.
    let recorded: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(size_bytes), 0)::bigint FROM task_attachments").fetch_one(&state.pool).await?;

    Ok(Json(serde_json::json!({
        "upload_bytes": total,
        "file_count": file_count,
        "attachments_recorded_bytes": recorded,
        "db_bytes": db_size,
        "disk": disk.map(|(t, a)| serde_json::json!({ "total": t, "available": a })),
        "by_usage": bucket(by_usage),
        "by_type": bucket(by_type),
        "largest": all,
        "orphans": orphans,
        "orphan_count": orphan_count,
        "orphan_bytes": orphan_bytes,
    })))
}

/// Les fichiers très récents ne sont jamais supprimés : ils peuvent être en cours d'envoi
/// (image collée dans un document pas encore enregistré).
fn old_enough(f: &FileInfo) -> bool {
    f.modified.is_some_and(|m| Utc::now() - m > chrono::Duration::hours(1))
}

async fn delete_orphans(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<serde_json::Value>> {
    require_super_admin(&user)?;
    let usages = collect_usages(&state.pool).await?;
    let files = tokio::task::spawn_blocking(list_files).await.map_err(|e| ApiError::Internal(e.to_string()))?;
    let (mut count, mut bytes) = (0u64, 0u64);
    for f in files.iter().filter(|f| !usages.contains_key(&f.name) && old_enough(f)) {
        if tokio::fs::remove_file(PathBuf::from(UPLOAD_DIR).join(&f.name)).await.is_ok() {
            count += 1;
            bytes += f.size;
        }
    }
    tracing::info!("storage: {count} orphan files removed ({bytes} bytes)");
    Ok(Json(serde_json::json!({ "count": count, "bytes": bytes })))
}

async fn delete_file(State(state): State<AppState>, user: AuthUser, Path(name): Path<String>) -> ApiResult<StatusCode> {
    require_super_admin(&user)?;
    if name.is_empty() || name.starts_with('.') || name.contains('/') || name.contains("..") {
        return Err(ApiError::bad_request("invalid_value"));
    }
    if collect_usages(&state.pool).await?.contains_key(&name) {
        return Err(ApiError::Conflict("file_in_use".into()));
    }
    tokio::fs::remove_file(PathBuf::from(UPLOAD_DIR).join(&name)).await.map_err(|_| ApiError::NotFound)?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_upload_names() {
        let html = r#"<img src="/uploads/a-1.png"><a href="https://x.org/uploads/b_2.pdf">x</a> /uploads/ end"#;
        assert_eq!(upload_refs(html), vec!["a-1.png", "b_2.pdf"]);
    }
}

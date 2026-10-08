//! Drive temporaire de l'espace : dépôt de fichiers qui s'effacent tout seuls.
//!
//! - Durée maximale fixée par les administrateurs (1 à 365 jours) ; à l'envoi, chacun
//!   peut choisir une durée plus courte, et prolonger ensuite (jusqu'au maximum, compté
//!   à partir de maintenant). Quota de l'espace (Mo).
//! - Dossiers, renommage, déplacement, lien public de téléchargement valable jusqu'à
//!   l'expiration.
//! - Droits : lecture des documents pour voir et télécharger, modification pour déposer ;
//!   supprimer le fichier de quelqu'un d'autre demande la gestion des documents.
//! - Toutes les 10 minutes : fichiers expirés supprimés (base et disque).

use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use chrono::{DateTime, Duration, Utc};
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::access::{level, member_perms, AuthUser, Perms};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub fn workspace_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list))
        .route("/upload", post(upload).layer(DefaultBodyLimit::max(MAX_REQUEST)))
        .route("/folders", post(create_folder))
        .route("/settings", put(put_settings))
        .route("/files/:id", put(update).delete(remove))
        .route("/files/:id/download", get(download))
        .route("/files/:id/share", post(share).delete(unshare))
}

pub fn public_routes() -> Router<AppState> {
    Router::new().route("/:token", get(public_download))
}

const MAX_REQUEST: usize = 2 * 1024 * 1024 * 1024;
pub const DAY_CHOICES: &[i32] = &[1, 3, 7, 14, 30, 60, 90, 180, 365];

fn drive_dir(ws: Uuid) -> PathBuf {
    PathBuf::from(crate::upload::UPLOAD_DIR).join("drive").join(ws.to_string())
}

async fn perms_for(state: &AppState, user: &AuthUser, ws: Uuid) -> ApiResult<Perms> {
    let (_, p) = member_perms(&state.pool, user.id, ws).await?.ok_or(ApiError::NotFound)?;
    if p.docs < level::READ {
        return Err(ApiError::NotFound);
    }
    Ok(p)
}

fn can_upload(p: &Perms) -> bool {
    p.docs >= level::EDIT
}

fn can_manage(p: &Perms) -> bool {
    p.docs >= level::MANAGE || p.admin
}

fn clean_name(raw: &str) -> ApiResult<String> {
    let n = raw.trim();
    if n.is_empty() || n.len() > 200 || n == "." || n == ".." || n.chars().any(|c| c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')) {
        return Err(ApiError::bad_request("drive_invalid_name"));
    }
    Ok(n.to_string())
}

fn clean_folder(raw: &str) -> ApiResult<String> {
    let t = raw.trim().trim_matches('/');
    if t.is_empty() {
        return Ok(String::new());
    }
    let parts: Vec<String> = t.split('/').map(clean_name).collect::<ApiResult<_>>()?;
    if parts.len() > 10 {
        return Err(ApiError::bad_request("drive_invalid_name"));
    }
    Ok(parts.join("/"))
}

fn join(folder: &str, name: &str) -> String {
    if folder.is_empty() { name.to_string() } else { format!("{folder}/{name}") }
}

#[derive(sqlx::FromRow)]
struct Settings {
    drive_max_days: i32,
    drive_quota_mb: i32,
}

async fn settings_of(state: &AppState, ws: Uuid) -> ApiResult<Settings> {
    Ok(sqlx::query_as("SELECT drive_max_days, drive_quota_mb FROM workspaces WHERE id = $1").bind(ws).fetch_one(&state.pool).await?)
}

async fn used_bytes(state: &AppState, ws: Uuid) -> ApiResult<i64> {
    Ok(sqlx::query_scalar("SELECT COALESCE(SUM(size), 0)::bigint FROM drive_files WHERE workspace_id = $1 AND NOT is_dir").bind(ws).fetch_one(&state.pool).await?)
}

#[derive(Serialize, sqlx::FromRow)]
struct Item {
    id: Uuid,
    folder: String,
    name: String,
    is_dir: bool,
    mime: Option<String>,
    size: i64,
    created_at: DateTime<Utc>,
    expires_at: Option<DateTime<Utc>>,
    uploaded_by: Option<Uuid>,
    uploaded_by_name: Option<String>,
    shared: bool,
    /// Dossier : nombre d'éléments directs.
    children: i64,
}

#[derive(Deserialize)]
struct ListQuery {
    folder: Option<String>,
    q: Option<String>,
}

async fn list(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>, Query(q): Query<ListQuery>) -> ApiResult<Json<Value>> {
    let perms = perms_for(&state, &user, ws).await?;
    let folder = clean_folder(q.folder.as_deref().unwrap_or(""))?;
    let search = q.q.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(|s| format!("%{}%", s.to_lowercase().replace(['%', '_'], "")));
    let items: Vec<Item> = sqlx::query_as(
        "SELECT f.id, f.folder, f.name, f.is_dir, f.mime, f.size, f.created_at, f.expires_at, f.uploaded_by, u.name AS uploaded_by_name,
                (f.share_token IS NOT NULL) AS shared,
                CASE WHEN f.is_dir THEN (SELECT count(*) FROM drive_files c WHERE c.workspace_id = f.workspace_id
                     AND c.folder = CASE WHEN f.folder = '' THEN f.name ELSE f.folder || '/' || f.name END) ELSE 0 END AS children
         FROM drive_files f LEFT JOIN users u ON u.id = f.uploaded_by
         WHERE f.workspace_id = $1 AND (($3::text IS NULL AND f.folder = $2) OR ($3::text IS NOT NULL AND lower(f.name) LIKE $3))
           AND (f.expires_at IS NULL OR f.expires_at > NOW())
         ORDER BY f.is_dir DESC, lower(f.name)",
    )
    .bind(ws)
    .bind(&folder)
    .bind(&search)
    .fetch_all(&state.pool)
    .await?;
    let s = settings_of(&state, ws).await?;
    Ok(Json(json!({
        "folder": folder,
        "items": items,
        "max_days": s.drive_max_days,
        "quota_bytes": s.drive_quota_mb as i64 * 1024 * 1024,
        "used_bytes": used_bytes(&state, ws).await?,
        "day_choices": DAY_CHOICES,
        "can_upload": can_upload(&perms),
        "can_manage": can_manage(&perms),
        "user_id": user.id,
    })))
}

async fn ensure_folder(state: &AppState, ws: Uuid, folder: &str, user: Uuid) -> ApiResult<()> {
    if folder.is_empty() {
        return Ok(());
    }
    let mut parent = String::new();
    for seg in folder.split('/') {
        let exists: Option<bool> = sqlx::query_scalar("SELECT is_dir FROM drive_files WHERE workspace_id = $1 AND folder = $2 AND name = $3").bind(ws).bind(&parent).bind(seg).fetch_optional(&state.pool).await?;
        match exists {
            Some(true) => {}
            Some(false) => return Err(ApiError::bad_request("drive_name_taken")),
            None => {
                sqlx::query("INSERT INTO drive_files (workspace_id, folder, name, is_dir, uploaded_by) VALUES ($1, $2, $3, TRUE, $4) ON CONFLICT DO NOTHING").bind(ws).bind(&parent).bind(seg).bind(user).execute(&state.pool).await?;
            }
        }
        parent = join(&parent, seg);
    }
    Ok(())
}

/// Nom libre dans un dossier : « rapport.pdf », sinon « rapport (2).pdf »…
async fn free_name(state: &AppState, ws: Uuid, folder: &str, name: &str) -> ApiResult<String> {
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_string(), format!(".{e}")),
        _ => (name.to_string(), String::new()),
    };
    let mut candidate = name.to_string();
    for i in 2..1000 {
        let taken: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM drive_files WHERE workspace_id = $1 AND folder = $2 AND name = $3)").bind(ws).bind(folder).bind(&candidate).fetch_one(&state.pool).await?;
        if !taken {
            return Ok(candidate);
        }
        candidate = format!("{stem} ({i}){ext}");
    }
    Err(ApiError::bad_request("drive_name_taken"))
}

fn days_or_max(days: Option<i32>, max: i32) -> i32 {
    days.filter(|d| *d >= 1).map(|d| d.min(max)).unwrap_or(max)
}

/// Envoi (flux écrit directement sur le disque) : champs `folder`, `days`, puis fichiers.
async fn upload(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>, mut mp: Multipart) -> ApiResult<Json<Value>> {
    let perms = perms_for(&state, &user, ws).await?;
    if !can_upload(&perms) {
        return Err(ApiError::Forbidden);
    }
    let s = settings_of(&state, ws).await?;
    let quota = s.drive_quota_mb as i64 * 1024 * 1024;
    let mut used = used_bytes(&state, ws).await?;
    let mut folder = String::new();
    let mut days: Option<i32> = None;
    let mut saved = Vec::new();
    let dir = drive_dir(ws);
    tokio::fs::create_dir_all(&dir).await.map_err(|e| ApiError::Internal(e.to_string()))?;
    while let Some(mut field) = mp.next_field().await.map_err(|e| ApiError::BadRequest(e.to_string()))? {
        match field.name() {
            Some("folder") => {
                folder = clean_folder(&field.text().await.unwrap_or_default())?;
                ensure_folder(&state, ws, &folder, user.id).await?;
                continue;
            }
            Some("days") => {
                days = field.text().await.ok().and_then(|v| v.trim().parse().ok());
                continue;
            }
            _ => {}
        }
        let Some(fname) = field.file_name().map(str::to_string) else { continue };
        // Envoi d'un dossier : « sous/dossier/fichier.txt ».
        let (sub, base) = match fname.replace('\\', "/").rsplit_once('/') {
            Some((d, n)) => (clean_folder(&join(&folder, d))?, clean_name(n)?),
            None => (folder.clone(), clean_name(&fname)?),
        };
        ensure_folder(&state, ws, &sub, user.id).await?;
        let rel = format!("drive/{ws}/{}", Uuid::new_v4());
        let path = PathBuf::from(crate::upload::UPLOAD_DIR).join(&rel);
        let mut file = tokio::fs::File::create(&path).await.map_err(|e| ApiError::Internal(e.to_string()))?;
        let mut size: i64 = 0;
        let write = async {
            while let Some(chunk) = field.chunk().await.map_err(|e| ApiError::BadRequest(e.to_string()))? {
                size += chunk.len() as i64;
                if used + size > quota {
                    return Err(ApiError::bad_request("drive_quota_exceeded"));
                }
                file.write_all(&chunk).await.map_err(|e| ApiError::Internal(e.to_string()))?;
            }
            file.flush().await.map_err(|e| ApiError::Internal(e.to_string()))?;
            Ok(())
        }
        .await;
        if let Err(e) = write {
            let _ = tokio::fs::remove_file(&path).await;
            return Err(e);
        }
        used += size;
        let name = free_name(&state, ws, &sub, &base).await?;
        let expires = Utc::now() + Duration::days(days_or_max(days, s.drive_max_days) as i64);
        let mime = mime_guess::from_path(&name).first_or_octet_stream().essence_str().to_string();
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO drive_files (workspace_id, folder, name, blob_path, mime, size, uploaded_by, expires_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING id",
        )
        .bind(ws)
        .bind(&sub)
        .bind(&name)
        .bind(&rel)
        .bind(&mime)
        .bind(size)
        .bind(user.id)
        .bind(expires)
        .fetch_one(&state.pool)
        .await?;
        saved.push(json!({ "id": id, "folder": sub, "name": name, "size": size, "expires_at": expires }));
    }
    state.emit(ws, json!({ "type": "DRIVE_CHANGED" })).await;
    Ok(Json(json!({ "files": saved })))
}

#[derive(Deserialize)]
struct FolderReq {
    folder: Option<String>,
    name: String,
}

async fn create_folder(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>, Json(b): Json<FolderReq>) -> ApiResult<Json<Value>> {
    let perms = perms_for(&state, &user, ws).await?;
    if !can_upload(&perms) {
        return Err(ApiError::Forbidden);
    }
    let folder = clean_folder(b.folder.as_deref().unwrap_or(""))?;
    let name = clean_name(&b.name)?;
    ensure_folder(&state, ws, &folder, user.id).await?;
    let id: Option<Uuid> = sqlx::query_scalar("INSERT INTO drive_files (workspace_id, folder, name, is_dir, uploaded_by) VALUES ($1, $2, $3, TRUE, $4) ON CONFLICT DO NOTHING RETURNING id")
        .bind(ws)
        .bind(&folder)
        .bind(&name)
        .bind(user.id)
        .fetch_optional(&state.pool)
        .await?;
    let id = id.ok_or_else(|| ApiError::bad_request("drive_name_taken"))?;
    state.emit(ws, json!({ "type": "DRIVE_CHANGED" })).await;
    Ok(Json(json!({ "id": id, "folder": folder, "name": name })))
}

#[derive(sqlx::FromRow)]
struct Row {
    folder: String,
    name: String,
    is_dir: bool,
    blob_path: Option<String>,
    mime: Option<String>,
    uploaded_by: Option<Uuid>,
    expires_at: Option<DateTime<Utc>>,
}

async fn row(state: &AppState, ws: Uuid, id: Uuid) -> ApiResult<Row> {
    sqlx::query_as("SELECT id, folder, name, is_dir, blob_path, mime, uploaded_by, expires_at FROM drive_files WHERE id = $1 AND workspace_id = $2 AND (expires_at IS NULL OR expires_at > NOW())")
        .bind(id)
        .bind(ws)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(ApiError::NotFound)
}

fn like_prefix(path: &str) -> String {
    format!("{}/%", path.replace('%', "\\%").replace('_', "\\_"))
}

#[derive(Deserialize)]
struct UpdateReq {
    name: Option<String>,
    /// Dossier de destination (déplacer).
    folder: Option<String>,
    /// Nouvelle durée à partir de maintenant (au plus le maximum de l'espace).
    days: Option<i32>,
}

async fn update(State(state): State<AppState>, user: AuthUser, Path((ws, id)): Path<(Uuid, Uuid)>, Json(b): Json<UpdateReq>) -> ApiResult<Json<Value>> {
    let perms = perms_for(&state, &user, ws).await?;
    let r = row(&state, ws, id).await?;
    if !can_upload(&perms) || (r.uploaded_by != Some(user.id) && !can_manage(&perms)) {
        return Err(ApiError::Forbidden);
    }
    let name = match b.name.as_deref() {
        Some(n) => clean_name(n)?,
        None => r.name.clone(),
    };
    let folder = match b.folder.as_deref() {
        Some(f) => clean_folder(f)?,
        None => r.folder.clone(),
    };
    let old_path = join(&r.folder, &r.name);
    let new_path = join(&folder, &name);
    if new_path != old_path {
        if r.is_dir && (folder == old_path || folder.starts_with(&format!("{old_path}/"))) {
            return Err(ApiError::bad_request("drive_invalid_move"));
        }
        ensure_folder(&state, ws, &folder, user.id).await?;
        let mut tx = state.pool.begin().await?;
        let res = sqlx::query("UPDATE drive_files SET folder = $3, name = $4 WHERE id = $1 AND workspace_id = $2").bind(id).bind(ws).bind(&folder).bind(&name).execute(&mut *tx).await;
        if let Err(sqlx::Error::Database(e)) = &res {
            if e.is_unique_violation() {
                return Err(ApiError::bad_request("drive_name_taken"));
            }
        }
        res?;
        if r.is_dir {
            // Contenu du dossier : « ancien/… » → « nouveau/… ».
            sqlx::query("UPDATE drive_files SET folder = $3 || substr(folder, length($2) + 1) WHERE workspace_id = $1 AND (folder = $2 OR folder LIKE $4)")
                .bind(ws)
                .bind(&old_path)
                .bind(&new_path)
                .bind(like_prefix(&old_path))
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
    }
    if let (Some(days), false) = (b.days, r.is_dir) {
        let s = settings_of(&state, ws).await?;
        let expires = Utc::now() + Duration::days(days_or_max(Some(days), s.drive_max_days) as i64);
        sqlx::query("UPDATE drive_files SET expires_at = $2 WHERE id = $1").bind(id).bind(expires).execute(&state.pool).await?;
    }
    state.emit(ws, json!({ "type": "DRIVE_CHANGED" })).await;
    Ok(Json(json!({ "id": id, "folder": folder, "name": name })))
}

async fn delete_blobs(blobs: Vec<Option<String>>) {
    for b in blobs.into_iter().flatten() {
        let _ = tokio::fs::remove_file(PathBuf::from(crate::upload::UPLOAD_DIR).join(b)).await;
    }
}

async fn remove(State(state): State<AppState>, user: AuthUser, Path((ws, id)): Path<(Uuid, Uuid)>) -> ApiResult<StatusCode> {
    let perms = perms_for(&state, &user, ws).await?;
    let r = row(&state, ws, id).await?;
    if !can_upload(&perms) || (r.uploaded_by != Some(user.id) && !can_manage(&perms)) {
        return Err(ApiError::Forbidden);
    }
    let path = join(&r.folder, &r.name);
    let blobs: Vec<Option<String>> = sqlx::query_scalar(
        "DELETE FROM drive_files WHERE workspace_id = $1 AND (id = $2 OR ($3 AND (folder = $4 OR folder LIKE $5))) RETURNING blob_path",
    )
    .bind(ws)
    .bind(id)
    .bind(r.is_dir)
    .bind(&path)
    .bind(like_prefix(&path))
    .fetch_all(&state.pool)
    .await?;
    delete_blobs(blobs).await;
    state.emit(ws, json!({ "type": "DRIVE_CHANGED" })).await;
    Ok(StatusCode::NO_CONTENT)
}

async fn file_response(r: &Row, inline: bool) -> ApiResult<Response> {
    let blob = r.blob_path.as_deref().ok_or(ApiError::NotFound)?;
    let file = tokio::fs::File::open(PathBuf::from(crate::upload::UPLOAD_DIR).join(blob)).await.map_err(|_| ApiError::NotFound)?;
    let len = file.metadata().await.map(|m| m.len()).unwrap_or(0);
    let stream = tokio_util::io::ReaderStream::new(file).map_err(std::io::Error::other);
    let mime = r.mime.clone().unwrap_or_else(|| "application/octet-stream".into());
    // Affichage dans le navigateur seulement pour les types sans risque (images, PDF, texte).
    let safe = inline && (mime.starts_with("image/") && !mime.contains("svg") || mime == "application/pdf" || mime.starts_with("video/") || mime.starts_with("audio/"));
    let disposition = format!("{}; filename*=UTF-8''{}", if safe { "inline" } else { "attachment" }, crate::mailclient::pct(&r.name));
    Ok((
        [
            (header::CONTENT_TYPE, mime),
            (header::CONTENT_DISPOSITION, disposition),
            (header::CONTENT_LENGTH, len.to_string()),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".into()),
            (header::CONTENT_SECURITY_POLICY, "sandbox".into()),
        ],
        Body::from_stream(stream),
    )
        .into_response())
}

#[derive(Deserialize)]
struct DownloadQuery {
    inline: Option<u8>,
}

async fn download(State(state): State<AppState>, user: AuthUser, Path((ws, id)): Path<(Uuid, Uuid)>, Query(q): Query<DownloadQuery>) -> ApiResult<Response> {
    perms_for(&state, &user, ws).await?;
    let r = row(&state, ws, id).await?;
    file_response(&r, q.inline == Some(1)).await
}

async fn share(State(state): State<AppState>, user: AuthUser, Path((ws, id)): Path<(Uuid, Uuid)>) -> ApiResult<Json<Value>> {
    let perms = perms_for(&state, &user, ws).await?;
    let r = row(&state, ws, id).await?;
    if r.is_dir || !(perms.share || perms.admin) {
        return Err(ApiError::Forbidden);
    }
    let token: String = (0..40).map(|_| format!("{:x}", rand::random::<u8>() % 16)).collect();
    let existing: Option<String> = sqlx::query_scalar("SELECT share_token FROM drive_files WHERE id = $1").bind(id).fetch_one(&state.pool).await?;
    let token = match existing {
        Some(t) => t,
        None => {
            sqlx::query("UPDATE drive_files SET share_token = $2 WHERE id = $1").bind(id).bind(&token).execute(&state.pool).await?;
            token
        }
    };
    Ok(Json(json!({ "url": format!("{}/api/drive/public/{token}", state.base_url.trim_end_matches('/')), "expires_at": r.expires_at })))
}

async fn unshare(State(state): State<AppState>, user: AuthUser, Path((ws, id)): Path<(Uuid, Uuid)>) -> ApiResult<StatusCode> {
    let perms = perms_for(&state, &user, ws).await?;
    row(&state, ws, id).await?;
    if !(perms.share || perms.admin) {
        return Err(ApiError::Forbidden);
    }
    sqlx::query("UPDATE drive_files SET share_token = NULL WHERE id = $1").bind(id).execute(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn public_download(State(state): State<AppState>, Path(token): Path<String>) -> ApiResult<Response> {
    if token.len() != 40 || !token.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(ApiError::NotFound);
    }
    let r: Row = sqlx::query_as("SELECT id, folder, name, is_dir, blob_path, mime, uploaded_by, expires_at FROM drive_files WHERE share_token = $1 AND NOT is_dir AND expires_at > NOW()")
        .bind(&token)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(ApiError::NotFound)?;
    file_response(&r, false).await
}

#[derive(Deserialize)]
struct SettingsReq {
    max_days: i32,
    quota_mb: i32,
}

async fn put_settings(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>, Json(b): Json<SettingsReq>) -> ApiResult<StatusCode> {
    crate::access::require_ws_admin(&state.pool, &user, ws).await?;
    if !DAY_CHOICES.contains(&b.max_days) || !(50..=1_000_000).contains(&b.quota_mb) {
        return Err(ApiError::bad_request("invalid_value"));
    }
    sqlx::query("UPDATE workspaces SET drive_max_days = $2, drive_quota_mb = $3 WHERE id = $1").bind(ws).bind(b.max_days).bind(b.quota_mb).execute(&state.pool).await?;
    // Durée réduite : les fichiers dépassant le nouveau maximum sont raccourcis.
    sqlx::query("UPDATE drive_files SET expires_at = LEAST(expires_at, NOW() + make_interval(days => $2)) WHERE workspace_id = $1 AND expires_at IS NOT NULL")
        .bind(ws)
        .bind(b.max_days)
        .execute(&state.pool)
        .await?;
    state.emit(ws, json!({ "type": "DRIVE_CHANGED" })).await;
    Ok(StatusCode::NO_CONTENT)
}

/// Suppression des fichiers expirés, puis des dossiers vides restés plus longtemps que
/// le maximum de leur espace.
pub async fn purge_expired(pool: &sqlx::PgPool) -> ApiResult<usize> {
    let blobs: Vec<Option<String>> = sqlx::query_scalar("DELETE FROM drive_files WHERE expires_at IS NOT NULL AND expires_at <= NOW() RETURNING blob_path").fetch_all(pool).await?;
    let n = blobs.len();
    delete_blobs(blobs).await;
    sqlx::query(
        "DELETE FROM drive_files d USING workspaces w WHERE d.workspace_id = w.id AND d.is_dir
           AND d.created_at < NOW() - make_interval(days => w.drive_max_days)
           AND NOT EXISTS (SELECT 1 FROM drive_files c WHERE c.workspace_id = d.workspace_id
                           AND (c.folder = CASE WHEN d.folder = '' THEN d.name ELSE d.folder || '/' || d.name END
                                OR c.folder LIKE (CASE WHEN d.folder = '' THEN d.name ELSE d.folder || '/' || d.name END) || '/%'))",
    )
    .execute(pool)
    .await?;
    Ok(n)
}

pub fn start_purge_loop(state: AppState) {
    tokio::spawn(async move {
        loop {
            match purge_expired(&state.pool).await {
                Ok(n) if n > 0 => tracing::info!("drive: {n} expired file(s) deleted"),
                Err(e) => tracing::warn!("drive purge: {e:?}"),
                _ => {}
            }
            tokio::time::sleep(std::time::Duration::from_secs(600)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert!(clean_name("rapport final.pdf").is_ok());
        assert!(clean_name("a/b").is_err() && clean_name("..").is_err() && clean_name("").is_err());
        assert_eq!(clean_folder("/Cours/TD1/").unwrap(), "Cours/TD1");
        assert!(clean_folder("Cours/../x").is_err());
        assert_eq!(days_or_max(Some(30), 7), 7);
        assert_eq!(days_or_max(Some(3), 7), 3);
        assert_eq!(days_or_max(None, 14), 14);
    }
}

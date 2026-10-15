//! Sauvegarde et restauration complètes de l'instance (administration).
//!
//! Une sauvegarde est une archive `.tar.gz` contenant :
//! - `manifest.json` : version de Looty, dernière migration, colonnes de chaque table ;
//! - `db/<table>.copy` : chaque table du schéma `public`, au format texte de `COPY` ;
//! - `uploads/…` : tous les fichiers envoyés (dont `.secret.key`, la clé de chiffrement).
//!
//! Aucune dépendance externe (pas de `pg_dump`) : export et import passent par `COPY`.
//! La restauration crée d'abord une sauvegarde « avant restauration », puis remplace les
//! données dans une transaction (contraintes et déclencheurs suspendus le temps de l'import).
//! Une sauvegarde d'une version plus ancienne se restaure : les colonnes ajoutées depuis
//! prennent leur valeur par défaut. Une sauvegarde plus récente que le serveur est refusée.

use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::{header, HeaderValue, StatusCode},
    response::Response,
    routing::{delete, get, post},
    Json, Router,
};
use chrono::{DateTime, Timelike, Utc};
use futures::{StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolCopyExt;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path as FsPath, PathBuf};
use tokio::io::AsyncWriteExt;

use crate::access::AuthUser;
use crate::admin::require_super_admin;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create_now))
        .route("/settings", axum::routing::put(update_settings))
        .route("/upload", post(upload).layer(DefaultBodyLimit::max(8 * 1024 * 1024 * 1024)))
        .route("/{name}", delete(remove))
        .route("/{name}/download", get(download))
        .route("/{name}/restore", post(restore_route))
}

fn backup_dir() -> PathBuf {
    PathBuf::from(std::env::var("BACKUP_DIR").unwrap_or_else(|_| "backups".into()))
}

/// Nom de fichier sûr (aucun chemin) : looty-AAAAMMJJ-HHMMSS-<type>.tar.gz
fn safe_name(name: &str) -> bool {
    name.starts_with("looty-") && name.ends_with(".tar.gz") && name.len() < 120 && name.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
}

#[derive(Serialize, Deserialize)]
struct Manifest {
    format: u32,
    app_version: String,
    created_at: DateTime<Utc>,
    kind: String,
    migration: i64,
    tables: BTreeMap<String, Vec<String>>,
    rows: BTreeMap<String, i64>,
    uploads: u64,
    uploads_bytes: u64,
}

#[derive(Serialize)]
struct BackupFile {
    name: String,
    size: u64,
    created_at: Option<DateTime<Utc>>,
    kind: String,
}

fn kind_of(name: &str) -> String {
    name.trim_end_matches(".tar.gz").rsplit('-').next().unwrap_or("manual").to_string()
}

async fn list_files() -> Vec<BackupFile> {
    let mut out = Vec::new();
    let Ok(mut rd) = tokio::fs::read_dir(backup_dir()).await else { return out };
    while let Ok(Some(e)) = rd.next_entry().await {
        let name = e.file_name().to_string_lossy().to_string();
        if !safe_name(&name) {
            continue;
        }
        let meta = e.metadata().await.ok();
        out.push(BackupFile {
            size: meta.as_ref().map(|m| m.len()).unwrap_or(0),
            created_at: meta.and_then(|m| m.modified().ok()).map(DateTime::<Utc>::from),
            kind: kind_of(&name),
            name,
        });
    }
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    out
}

async fn list(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<serde_json::Value>> {
    require_super_admin(&user)?;
    let (schedule, hour, keep, last): (String, i16, i16, Option<DateTime<Utc>>) =
        sqlx::query_as("SELECT backup_schedule, backup_hour, backup_keep, backup_last_at FROM global_settings WHERE id = 1").fetch_one(&state.pool).await?;
    Ok(Json(serde_json::json!({
        "backups": list_files().await,
        "directory": backup_dir().canonicalize().unwrap_or_else(|_| backup_dir()).display().to_string(),
        "schedule": schedule, "hour": hour, "keep": keep, "last_at": last,
        "timezone": std::env::var("CALENDAR_DEFAULT_TZ").unwrap_or_else(|_| "UTC".into()),
    })))
}

#[derive(Deserialize)]
struct SettingsReq {
    schedule: String,
    hour: i16,
    keep: i16,
}

async fn update_settings(State(state): State<AppState>, user: AuthUser, Json(p): Json<SettingsReq>) -> ApiResult<StatusCode> {
    require_super_admin(&user)?;
    if !matches!(p.schedule.as_str(), "off" | "daily" | "weekly") || !(0..24).contains(&p.hour) || !(1..=100).contains(&p.keep) {
        return Err(ApiError::bad_request("invalid_value"));
    }
    sqlx::query("UPDATE global_settings SET backup_schedule = $1, backup_hour = $2, backup_keep = $3 WHERE id = 1")
        .bind(&p.schedule)
        .bind(p.hour)
        .bind(p.keep)
        .execute(&state.pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn create_now(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<serde_json::Value>> {
    require_super_admin(&user)?;
    let name = create_backup(&state.pool, "manual").await?;
    Ok(Json(serde_json::json!({ "name": name })))
}

async fn current_migration(pool: &sqlx::PgPool) -> ApiResult<i64> {
    Ok(sqlx::query_scalar("SELECT COALESCE(max(version), 0) FROM _sqlx_migrations WHERE success").fetch_one(pool).await?)
}

/// Tables du schéma public et leurs colonnes (ordre de création).
async fn tables(pool: &sqlx::PgPool) -> ApiResult<BTreeMap<String, Vec<String>>> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT c.table_name, c.column_name FROM information_schema.columns c
         JOIN information_schema.tables t ON t.table_schema = c.table_schema AND t.table_name = c.table_name
         WHERE c.table_schema = 'public' AND t.table_type = 'BASE TABLE'
         ORDER BY c.table_name, c.ordinal_position",
    )
    .fetch_all(pool)
    .await?;
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (t, c) in rows {
        out.entry(t).or_default().push(c);
    }
    Ok(out)
}

fn quote(id: &str) -> String {
    format!("\"{}\"", id.replace('"', "\"\""))
}

/// Crée une sauvegarde complète ; renvoie son nom de fichier.
pub async fn create_backup(pool: &sqlx::PgPool, kind: &str) -> ApiResult<String> {
    let dir = backup_dir();
    tokio::fs::create_dir_all(&dir).await.map_err(|e| ApiError::Internal(format!("backup dir: {e}")))?;
    let now = Utc::now();
    let name = format!("looty-{}-{kind}.tar.gz", now.format("%Y%m%d-%H%M%S"));
    let work = std::env::temp_dir().join(format!("looty-backup-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(work.join("db")).await.map_err(|e| ApiError::Internal(e.to_string()))?;

    let all = tables(pool).await?;
    let mut rows = BTreeMap::new();
    for (table, cols) in &all {
        let cols_sql = cols.iter().map(|c| quote(c)).collect::<Vec<_>>().join(", ");
        let mut stream = pool.copy_out_raw(&format!("COPY public.{} ({cols_sql}) TO STDOUT", quote(table))).await?;
        let mut f = tokio::fs::File::create(work.join("db").join(format!("{table}.copy"))).await.map_err(|e| ApiError::Internal(e.to_string()))?;
        let mut n = 0i64;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            n += chunk.iter().filter(|b| **b == b'\n').count() as i64;
            f.write_all(&chunk).await.map_err(|e| ApiError::Internal(e.to_string()))?;
        }
        f.flush().await.ok();
        rows.insert(table.clone(), n);
    }
    let migration = current_migration(pool).await?;
    let target = dir.join(&name);
    let work2 = work.clone();
    let kind_s = kind.to_string();
    let res = tokio::task::spawn_blocking(move || -> std::io::Result<()> {
        // Fichiers envoyés (récursif), comptés pour le manifeste.
        let uploads = FsPath::new(crate::upload::UPLOAD_DIR);
        let mut files: Vec<PathBuf> = Vec::new();
        if uploads.exists() {
            for e in walkdir(uploads)? {
                files.push(e);
            }
        }
        let bytes: u64 = files.iter().filter_map(|f| std::fs::metadata(f).ok()).map(|m| m.len()).sum();
        let manifest = Manifest {
            format: 1,
            app_version: env!("CARGO_PKG_VERSION").into(),
            created_at: now,
            kind: kind_s,
            migration,
            tables: all,
            rows,
            uploads: files.len() as u64,
            uploads_bytes: bytes,
        };
        std::fs::write(work2.join("manifest.json"), serde_json::to_vec_pretty(&manifest)?)?;
        let tmp = target.with_extension("partial");
        let out = std::fs::File::create(&tmp)?;
        let gz = flate2::write::GzEncoder::new(out, flate2::Compression::fast());
        let mut tar = tar::Builder::new(gz);
        tar.append_path_with_name(work2.join("manifest.json"), "manifest.json")?;
        tar.append_dir_all("db", work2.join("db"))?;
        for f in &files {
            let rel = f.strip_prefix(uploads).unwrap_or(f);
            tar.append_path_with_name(f, FsPath::new("uploads").join(rel))?;
        }
        tar.into_inner()?.finish()?;
        std::fs::rename(&tmp, &target)?;
        Ok(())
    })
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;
    let _ = tokio::fs::remove_dir_all(&work).await;
    res.map_err(|e| ApiError::Internal(format!("backup: {e}")))?;
    if kind == "auto" || kind == "manual" {
        sqlx::query("UPDATE global_settings SET backup_last_at = NOW() WHERE id = 1").execute(pool).await?;
    }
    tracing::info!("backup created: {name}");
    Ok(name)
}

/// Liste récursive des fichiers d'un dossier.
fn walkdir(root: &FsPath) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d)? {
            let e = e?;
            let ft = e.file_type()?;
            if ft.is_dir() {
                stack.push(e.path());
            } else if ft.is_file() {
                out.push(e.path());
            }
        }
    }
    Ok(out)
}

async fn download(user: AuthUser, Path(name): Path<String>) -> ApiResult<Response> {
    require_super_admin(&user)?;
    if !safe_name(&name) {
        return Err(ApiError::NotFound);
    }
    let path = backup_dir().join(&name);
    let file = tokio::fs::File::open(&path).await.map_err(|_| ApiError::NotFound)?;
    let len = file.metadata().await.map(|m| m.len()).unwrap_or(0);
    let stream = tokio_util::io::ReaderStream::new(file);
    let mut resp = Response::new(Body::from_stream(stream));
    let h = resp.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/gzip"));
    h.insert(header::CONTENT_LENGTH, HeaderValue::from(len));
    h.insert(header::CONTENT_DISPOSITION, HeaderValue::from_str(&format!("attachment; filename=\"{name}\"")).unwrap_or(HeaderValue::from_static("attachment")));
    Ok(resp)
}

async fn remove(user: AuthUser, Path(name): Path<String>) -> ApiResult<StatusCode> {
    require_super_admin(&user)?;
    if !safe_name(&name) {
        return Err(ApiError::NotFound);
    }
    tokio::fs::remove_file(backup_dir().join(&name)).await.map_err(|_| ApiError::NotFound)?;
    Ok(StatusCode::NO_CONTENT)
}

/// Import d'une archive (ex. d'un autre serveur) dans le dossier des sauvegardes.
async fn upload(user: AuthUser, mut multipart: Multipart) -> ApiResult<Json<serde_json::Value>> {
    require_super_admin(&user)?;
    let dir = backup_dir();
    tokio::fs::create_dir_all(&dir).await.map_err(|e| ApiError::Internal(e.to_string()))?;
    while let Some(field) = multipart.next_field().await.map_err(|_| ApiError::bad_request("invalid_upload"))? {
        if field.name() != Some("file") {
            continue;
        }
        let name = format!("looty-{}-upload.tar.gz", Utc::now().format("%Y%m%d-%H%M%S"));
        let path = dir.join(&name);
        let mut f = tokio::fs::File::create(&path).await.map_err(|e| ApiError::Internal(e.to_string()))?;
        let mut stream = field.map_err(|e| std::io::Error::other(e.to_string()));
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| ApiError::bad_request("invalid_upload"))?;
            f.write_all(&chunk).await.map_err(|e| ApiError::Internal(e.to_string()))?;
        }
        f.flush().await.ok();
        // Vérifie que c'est bien une sauvegarde Looty lisible.
        let check = path.clone();
        let ok = tokio::task::spawn_blocking(move || read_manifest(&check).is_ok()).await.unwrap_or(false);
        if !ok {
            let _ = tokio::fs::remove_file(&path).await;
            return Err(ApiError::bad_request("backup_invalid"));
        }
        return Ok(Json(serde_json::json!({ "name": name })));
    }
    Err(ApiError::bad_request("invalid_upload"))
}

fn read_manifest(path: &FsPath) -> std::io::Result<Manifest> {
    let gz = flate2::read::GzDecoder::new(std::fs::File::open(path)?);
    let mut ar = tar::Archive::new(gz);
    for e in ar.entries()? {
        let mut e = e?;
        if e.path()?.as_os_str() == "manifest.json" {
            let m: Manifest = serde_json::from_reader(&mut e).map_err(std::io::Error::other)?;
            return Ok(m);
        }
    }
    Err(std::io::Error::other("manifest missing"))
}

async fn restore_route(State(state): State<AppState>, user: AuthUser, Path(name): Path<String>) -> ApiResult<Json<serde_json::Value>> {
    require_super_admin(&user)?;
    if !safe_name(&name) {
        return Err(ApiError::NotFound);
    }
    let path = backup_dir().join(&name);
    if !path.exists() {
        return Err(ApiError::NotFound);
    }
    let safety = create_backup(&state.pool, "prerestore").await?;
    let report = restore(&state, &path).await?;
    Ok(Json(serde_json::json!({ "restored": name, "safety_backup": safety, "report": report })))
}

/// Restaure une archive : données, puis fichiers envoyés.
async fn restore(state: &AppState, path: &FsPath) -> ApiResult<serde_json::Value> {
    let work = std::env::temp_dir().join(format!("looty-restore-{}", uuid::Uuid::new_v4()));
    let p = path.to_path_buf();
    let w = work.clone();
    let manifest = tokio::task::spawn_blocking(move || -> std::io::Result<Manifest> {
        let gz = flate2::read::GzDecoder::new(std::fs::File::open(&p)?);
        let mut ar = tar::Archive::new(gz);
        ar.unpack(&w)?;
        let m: Manifest = serde_json::from_slice(&std::fs::read(w.join("manifest.json"))?).map_err(std::io::Error::other)?;
        Ok(m)
    })
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .map_err(|_| ApiError::bad_request("backup_invalid"))?;

    let current = current_migration(&state.pool).await?;
    if manifest.migration > current {
        let _ = tokio::fs::remove_dir_all(&work).await;
        return Err(ApiError::bad_request("backup_newer"));
    }
    let now_tables = tables(&state.pool).await?;
    // Une colonne présente dans la sauvegarde mais disparue du schéma empêcherait l'import.
    for (t, cols) in &manifest.tables {
        if let Some(cur) = now_tables.get(t) {
            if let Some(c) = cols.iter().find(|c| !cur.contains(c)) {
                let _ = tokio::fs::remove_dir_all(&work).await;
                return Err(ApiError::BadRequest(format!("backup_incompatible: {t}.{c}")));
            }
        }
    }

    let mut tx = state.pool.begin().await?;
    sqlx::query("SET LOCAL session_replication_role = replica").execute(&mut *tx).await?;
    let to_clear: Vec<String> = now_tables.keys().filter(|t| t.as_str() != "_sqlx_migrations").map(|t| format!("public.{}", quote(t))).collect();
    if !to_clear.is_empty() {
        sqlx::query(sqlx::AssertSqlSafe(format!("TRUNCATE {} RESTART IDENTITY CASCADE", to_clear.join(", ")))).execute(&mut *tx).await?;
    }
    let mut restored: HashMap<String, i64> = HashMap::new();
    for (table, cols) in &manifest.tables {
        if table == "_sqlx_migrations" || !now_tables.contains_key(table) {
            continue;
        }
        let file = work.join("db").join(format!("{table}.copy"));
        let data = tokio::fs::read(&file).await.map_err(|e| ApiError::Internal(e.to_string()))?;
        let cols_sql = cols.iter().map(|c| quote(c)).collect::<Vec<_>>().join(", ");
        let mut copy = tx.copy_in_raw(&format!("COPY public.{} ({cols_sql}) FROM STDIN", quote(table))).await?;
        for chunk in data.chunks(1 << 20) {
            copy.send(chunk).await?;
        }
        let n = copy.finish().await?;
        restored.insert(table.clone(), n as i64);
    }
    // Séquences (identifiants auto-incrémentés) recalées sur les données importées.
    let seqs: Vec<(String, String)> = sqlx::query_as(
        "SELECT c.table_name, c.column_name FROM information_schema.columns c
         WHERE c.table_schema = 'public' AND c.column_default LIKE 'nextval(%'",
    )
    .fetch_all(&mut *tx)
    .await?;
    for (t, c) in seqs {
        let sql = format!(
            "SELECT setval(pg_get_serial_sequence('public.{}', '{}'), COALESCE((SELECT max({}) FROM public.{}), 0) + 1, false)",
            quote(&t).replace('\'', "''"), c.replace('\'', "''"), quote(&c), quote(&t)
        );
        let _ = sqlx::query(sqlx::AssertSqlSafe(sql.as_str())).execute(&mut *tx).await;
    }
    tx.commit().await?;

    // Fichiers : le dossier des envois reprend exactement le contenu de la sauvegarde.
    let uploads = FsPath::new(crate::upload::UPLOAD_DIR).to_path_buf();
    let src = work.join("uploads");
    let keep_key = std::env::var("LOOTY_SECRET_KEY").is_ok();
    let key_changed = tokio::task::spawn_blocking(move || -> std::io::Result<bool> {
        let old_key = std::fs::read(uploads.join(".secret.key")).ok();
        if uploads.exists() {
            for f in walkdir(&uploads)? {
                if keep_key || f.file_name().is_none_or(|n| n != ".secret.key") || src.join(".secret.key").exists() {
                    let _ = std::fs::remove_file(&f);
                }
            }
        }
        std::fs::create_dir_all(&uploads)?;
        if src.exists() {
            for f in walkdir(&src)? {
                let rel = f.strip_prefix(&src).unwrap_or(&f);
                let dest = uploads.join(rel);
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::rename(&f, &dest).or_else(|_| std::fs::copy(&f, &dest).map(|_| ()))?;
            }
        }
        Ok(old_key != std::fs::read(uploads.join(".secret.key")).ok())
    })
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .map_err(|e| ApiError::Internal(format!("restore uploads: {e}")))?;
    let _ = tokio::fs::remove_dir_all(&work).await;

    // Serveur de collaboration : oublie les documents en mémoire ; clients : rechargement.
    let _ = sqlx::query("NOTIFY looty_restore").execute(&state.pool).await;
    let spaces: Vec<uuid::Uuid> = sqlx::query_scalar("SELECT id FROM workspaces").fetch_all(&state.pool).await.unwrap_or_default();
    for ws in spaces {
        state.emit(ws, serde_json::json!({ "type": "INSTANCE_RESTORED" })).await;
    }
    tracing::warn!("instance restored from backup created {} (migration {})", manifest.created_at, manifest.migration);
    Ok(serde_json::json!({
        "backup_created_at": manifest.created_at, "backup_version": manifest.app_version, "tables": restored.len(),
        "rows": restored.values().sum::<i64>(), "uploads": manifest.uploads, "restart_required": key_changed && !keep_key,
    }))
}

/// Sauvegardes programmées : vérification toutes les 5 minutes.
pub fn start_scheduler(pool: sqlx::PgPool) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(300));
        loop {
            tick.tick().await;
            let row: Result<(String, i16, i16, Option<DateTime<Utc>>), _> =
                sqlx::query_as("SELECT backup_schedule, backup_hour, backup_keep, backup_last_at FROM global_settings WHERE id = 1").fetch_one(&pool).await;
            let Ok((schedule, hour, keep, last)) = row else { continue };
            if schedule == "off" {
                continue;
            }
            let tz: chrono_tz::Tz = std::env::var("CALENDAR_DEFAULT_TZ").ok().and_then(|z| z.parse().ok()).unwrap_or(chrono_tz::UTC);
            let local = Utc::now().with_timezone(&tz);
            let min_gap = if schedule == "weekly" { chrono::Duration::hours(6 * 24 + 12) } else { chrono::Duration::hours(20) };
            let due = local.hour() as i16 == hour && last.is_none_or(|l| Utc::now() - l > min_gap);
            if !due {
                continue;
            }
            match create_backup(&pool, "auto").await {
                Ok(_) => prune_auto(keep as usize).await,
                Err(e) => tracing::error!("scheduled backup failed: {e:?}"),
            }
        }
    });
}

/// Garde les N sauvegardes automatiques les plus récentes (les autres types sont conservés).
async fn prune_auto(keep: usize) {
    let autos: Vec<BackupFile> = list_files().await.into_iter().filter(|b| b.kind == "auto").collect();
    for b in autos.into_iter().skip(keep) {
        let _ = tokio::fs::remove_file(backup_dir().join(&b.name)).await;
    }
}

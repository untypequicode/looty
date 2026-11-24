//! État de l'instance pour les administrateurs : espace utilisé (base, fichiers, par espace),
//! comptes, espaces et sessions, tâches de fond (dernier passage, erreurs), dernière
//! sauvegarde, version et mise à jour disponible. Le même état sort en format Prometheus sur
//! `/metrics`, protégé par un jeton (`LOOTY_METRICS_TOKEN`).

use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::access::AuthUser;
use crate::admin::require_super_admin;
use crate::bg::{self, JobStat};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use crate::upload::UPLOAD_DIR;

/// Compte technique qui prend la place des auteurs de comptes supprimés (voir account.rs).
pub const DELETED_USER: &str = "00000000-0000-0000-0000-000000000000";

pub fn routes() -> Router<AppState> {
    Router::new().route("/", get(status))
}

#[derive(Serialize, sqlx::FromRow)]
struct SpaceBytes {
    id: uuid::Uuid,
    name: String,
    bytes: i64,
}

#[derive(Serialize, sqlx::FromRow)]
struct Counts {
    accounts: i64,
    accounts_suspended: i64,
    accounts_active_30d: i64,
    accounts_pending_deletion: i64,
    workspaces: i64,
    personal_spaces: i64,
    sessions: i64,
}

#[derive(Serialize)]
struct Snapshot {
    version: &'static str,
    counts: Counts,
    db_bytes: i64,
    upload_bytes: u64,
    upload_files: u64,
    disk: Option<(u64, u64)>,
    /// Octets de fichiers par espace partagé, les plus gros d'abord.
    spaces: Vec<SpaceBytes>,
    personal_bytes: i64,
    jobs: Vec<(String, JobStat)>,
    backup_last_at: Option<DateTime<Utc>>,
    backup_schedule: String,
}

/// Taille des fichiers d'un espace : pièces jointes, Drive et fichiers binaires des projets.
const SPACE_BYTES: &str = "SELECT w.id, w.name,
        (COALESCE((SELECT SUM(a.size_bytes) FROM task_attachments a JOIN tasks_all t ON t.id = a.task_id
                   JOIN board_columns c ON c.id = t.column_id JOIN boards b ON b.id = c.board_id WHERE b.workspace_id = w.id), 0)
       + COALESCE((SELECT SUM(d.size) FROM drive_files d WHERE d.workspace_id = w.id AND NOT d.is_dir), 0)
       + COALESCE((SELECT SUM(f.size) FROM project_files f JOIN projects p ON p.id = f.project_id
                   WHERE p.workspace_id = w.id AND f.blob_path IS NOT NULL), 0))::bigint AS bytes
     FROM workspaces w";

async fn snapshot(state: &AppState) -> ApiResult<Snapshot> {
    let pool = &state.pool;
    let counts: Counts = sqlx::query_as(
        "SELECT
            (SELECT count(*) FROM users WHERE id::text <> $1) AS accounts,
            (SELECT count(*) FROM users WHERE is_suspended AND id::text <> $1) AS accounts_suspended,
            (SELECT count(DISTINCT user_id) FROM analytics_events WHERE created_at > NOW() - INTERVAL '30 days') AS accounts_active_30d,
            (SELECT count(*) FROM users WHERE deletion_scheduled_at IS NOT NULL) AS accounts_pending_deletion,
            (SELECT count(*) FROM workspaces WHERE personal_of IS NULL) AS workspaces,
            (SELECT count(*) FROM workspaces WHERE personal_of IS NOT NULL) AS personal_spaces,
            (SELECT count(*) FROM tower_sessions.session WHERE expiry_date > NOW()) AS sessions",
    )
    .bind(DELETED_USER)
    .fetch_one(pool)
    .await?;
    let db_bytes: i64 = sqlx::query_scalar("SELECT pg_database_size(current_database())").fetch_one(pool).await.unwrap_or(0);
    let files = crate::storage::list_files();
    let upload_bytes = files.iter().map(|f| f.size).sum();
    let mut spaces: Vec<SpaceBytes> =
        sqlx::query_as(sqlx::AssertSqlSafe(format!("{SPACE_BYTES} WHERE w.personal_of IS NULL"))).fetch_all(pool).await?;
    spaces.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.name.cmp(&b.name)));
    spaces.truncate(50);
    let personal_bytes: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT COALESCE(SUM(bytes), 0)::bigint FROM ({SPACE_BYTES} WHERE w.personal_of IS NOT NULL) s")))
        .fetch_one(pool)
        .await?;
    let (backup_last_at, backup_schedule): (Option<DateTime<Utc>>, String) =
        sqlx::query_as("SELECT backup_last_at, backup_schedule FROM global_settings WHERE id = 1").fetch_optional(pool).await?.unwrap_or((None, "off".into()));
    Ok(Snapshot {
        version: env!("CARGO_PKG_VERSION"),
        counts,
        db_bytes,
        upload_bytes,
        upload_files: files.len() as u64,
        disk: crate::storage::disk_space(UPLOAD_DIR),
        spaces,
        personal_bytes,
        jobs: bg::snapshot(),
        backup_last_at,
        backup_schedule,
    })
}

async fn status(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<serde_json::Value>> {
    require_super_admin(&user)?;
    let s = snapshot(&state).await?;
    let update = update_status().await;
    Ok(Json(serde_json::json!({
        "version": s.version,
        "update": update,
        "counts": s.counts,
        "storage": {
            "db_bytes": s.db_bytes,
            "upload_bytes": s.upload_bytes,
            "upload_files": s.upload_files,
            "disk": s.disk.map(|(total, available)| serde_json::json!({ "total": total, "available": available })),
            "spaces": s.spaces,
            "personal_bytes": s.personal_bytes,
        },
        "jobs": s.jobs.into_iter().map(|(name, j)| serde_json::json!({
            "name": name, "runs": j.runs, "errors": j.errors, "last_run": j.last_run, "last_ok": j.last_ok,
            "last_error": j.last_error, "last_error_at": j.last_error_at,
        })).collect::<Vec<_>>(),
        "backup": { "last_at": s.backup_last_at, "schedule": s.backup_schedule },
        "metrics_enabled": metrics_token().is_some(),
    })))
}

// ---------------------------------------------------------------------------
// Mise à jour disponible (facultatif : LOOTY_UPDATE_CHECK=true)
// ---------------------------------------------------------------------------

/// Numéro de la dernière version web publiée parmi les versions d'un dépôt GitHub (étiquettes `web-vX.Y.Z`).
fn latest_web_tag(releases: &serde_json::Value) -> Option<String> {
    releases
        .as_array()?
        .iter()
        .filter(|r| !r["draft"].as_bool().unwrap_or(false) && !r["prerelease"].as_bool().unwrap_or(false))
        .filter_map(|r| r["tag_name"].as_str()?.strip_prefix("web-v").map(str::to_string))
        .max_by_key(|v| crate::releases::semver(v))
}

/// `https://github.com/<proprio>/<dépôt>` → point d'accès de l'API des versions.
fn api_url(repository: &str) -> Option<String> {
    let path = repository.trim_end_matches('/').strip_prefix("https://github.com/")?;
    (path.split('/').count() == 2).then(|| format!("https://api.github.com/repos/{path}/releases?per_page=30"))
}

fn update_cache() -> &'static Mutex<Option<(Instant, Option<String>)>> {
    static C: std::sync::OnceLock<Mutex<Option<(Instant, Option<String>)>>> = std::sync::OnceLock::new();
    C.get_or_init(Default::default)
}

/// La vérification sort vers GitHub : elle est désactivée par défaut et mise en cache six heures.
async fn update_status() -> serde_json::Value {
    let enabled = std::env::var("LOOTY_UPDATE_CHECK").is_ok_and(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes"));
    if !enabled {
        return serde_json::json!({ "checked": false });
    }
    let current = env!("CARGO_PKG_VERSION");
    let cached = update_cache().lock().unwrap_or_else(|e| e.into_inner()).clone().filter(|(at, _)| at.elapsed() < Duration::from_secs(6 * 3600));
    let latest = match cached {
        Some((_, v)) => v,
        None => {
            let v = fetch_latest().await;
            *update_cache().lock().unwrap_or_else(|e| e.into_inner()) = Some((Instant::now(), v.clone()));
            v
        }
    };
    match latest {
        Some(l) => serde_json::json!({ "checked": true, "latest": l, "available": crate::releases::semver(&l) > crate::releases::semver(current), "url": crate::releases::release_url("web", &l) }),
        None => serde_json::json!({ "checked": true, "error": true }),
    }
}

async fn fetch_latest() -> Option<String> {
    let url = api_url(&crate::releases::repository())?;
    let client = reqwest::Client::builder().timeout(Duration::from_secs(8)).user_agent("looty-instance").build().ok()?;
    let body: serde_json::Value = client.get(url).send().await.ok()?.error_for_status().ok()?.json().await.ok()?;
    latest_web_tag(&body)
}

// ---------------------------------------------------------------------------
// /metrics (Prometheus)
// ---------------------------------------------------------------------------

fn metrics_token() -> Option<String> {
    std::env::var("LOOTY_METRICS_TOKEN").ok().map(|t| t.trim().to_string()).filter(|t| t.len() >= 16)
}

fn token_matches(expected: &str, given: &str) -> bool {
    // Empreintes de même longueur : la comparaison ne dépend pas de la valeur donnée.
    let (a, b) = (Sha256::digest(expected.as_bytes()), Sha256::digest(given.as_bytes()));
    a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn label(v: &str) -> String {
    v.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', " ")
}

fn render_metrics(s: &Snapshot) -> String {
    let mut o = String::new();
    let mut put = |name: &str, help: &str, kind: &str, lines: Vec<String>| {
        o.push_str(&format!("# HELP {name} {help}\n# TYPE {name} {kind}\n"));
        for l in lines {
            o.push_str(&format!("{name}{l}\n"));
        }
    };
    put("looty_info", "Looty version.", "gauge", vec![format!("{{version=\"{}\"}} 1", s.version)]);
    put("looty_accounts", "Accounts, by state.", "gauge", vec![
        format!("{{state=\"all\"}} {}", s.counts.accounts),
        format!("{{state=\"suspended\"}} {}", s.counts.accounts_suspended),
        format!("{{state=\"active_30d\"}} {}", s.counts.accounts_active_30d),
        format!("{{state=\"pending_deletion\"}} {}", s.counts.accounts_pending_deletion),
    ]);
    put("looty_workspaces", "Shared workspaces.", "gauge", vec![format!(" {}", s.counts.workspaces)]);
    put("looty_sessions", "Open sessions.", "gauge", vec![format!(" {}", s.counts.sessions)]);
    put("looty_database_bytes", "Database size.", "gauge", vec![format!(" {}", s.db_bytes)]);
    put("looty_upload_bytes", "Size of the upload folder.", "gauge", vec![format!(" {}", s.upload_bytes)]);
    if let Some((total, available)) = s.disk {
        put("looty_disk_bytes", "Disk holding the uploads.", "gauge", vec![format!("{{kind=\"total\"}} {total}"), format!("{{kind=\"available\"}} {available}")]);
    }
    put("looty_job_runs_total", "Background job items processed.", "counter", s.jobs.iter().map(|(n, j)| format!("{{job=\"{}\"}} {}", label(n), j.runs)).collect());
    put("looty_job_errors_total", "Background job items that failed.", "counter", s.jobs.iter().map(|(n, j)| format!("{{job=\"{}\"}} {}", label(n), j.errors)).collect());
    put("looty_job_last_run_timestamp_seconds", "Last pass of a background job.", "gauge",
        s.jobs.iter().filter_map(|(n, j)| j.last_run.map(|t| format!("{{job=\"{}\"}} {}", label(n), t.timestamp()))).collect());
    if let Some(t) = s.backup_last_at {
        put("looty_backup_last_timestamp_seconds", "Last backup.", "gauge", vec![format!(" {}", t.timestamp())]);
    }
    o
}

/// `GET /metrics` : 404 tant que `LOOTY_METRICS_TOKEN` (16 caractères au moins) n'est pas défini, 401 sans le bon jeton.
pub async fn metrics(State(state): State<AppState>, headers: HeaderMap) -> Result<Response, ApiError> {
    let Some(expected) = metrics_token() else { return Err(ApiError::NotFound) };
    let given = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer ")).unwrap_or("");
    if !token_matches(&expected, given.trim()) {
        return Ok((StatusCode::UNAUTHORIZED, [(header::WWW_AUTHENTICATE, "Bearer")], "unauthorized").into_response());
    }
    let body = render_metrics(&snapshot(&state).await?);
    Ok(([(header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8"), (header::CACHE_CONTROL, "no-store")], body).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test]
    async fn the_snapshot_counts_real_accounts_and_sizes_each_space(pool: sqlx::PgPool) {
        crate::session_store::PgSessionStore::new(pool.clone()).migrate().await.unwrap();
        let ana: uuid::Uuid = sqlx::query_scalar("INSERT INTO users (email, name) VALUES ('a@example.org', 'a') RETURNING id").fetch_one(&pool).await.unwrap();
        sqlx::query("INSERT INTO users (email, name, is_suspended, deletion_scheduled_at) VALUES ('b@example.org', 'b', TRUE, NOW())").execute(&pool).await.unwrap();
        let team: uuid::Uuid = sqlx::query_scalar("INSERT INTO workspaces (name) VALUES ('Team') RETURNING id").fetch_one(&pool).await.unwrap();
        let perso: uuid::Uuid = sqlx::query_scalar("INSERT INTO workspaces (name, personal_of) VALUES ('Perso', $1) RETURNING id").bind(ana).fetch_one(&pool).await.unwrap();
        for (ws, size) in [(team, 100i64), (team, 20), (perso, 7)] {
            sqlx::query("INSERT INTO drive_files (workspace_id, name, blob_path, size) VALUES ($1, gen_random_uuid()::text, 'drive/x', $2)").bind(ws).bind(size).execute(&pool).await.unwrap();
        }
        let s = snapshot(&AppState::new(pool, "http://localhost".into())).await.unwrap();
        // Le compte technique des auteurs supprimés ne compte pas.
        assert_eq!((s.counts.accounts, s.counts.accounts_suspended, s.counts.accounts_pending_deletion), (2, 1, 1));
        assert_eq!((s.counts.workspaces, s.counts.personal_spaces), (1, 1));
        assert_eq!(s.spaces.len(), 1);
        assert_eq!((s.spaces[0].name.as_str(), s.spaces[0].bytes), ("Team", 120));
        assert_eq!(s.personal_bytes, 7);
        assert!(s.db_bytes > 0);
    }

    #[test]
    fn picks_the_newest_published_web_release() {
        let r = serde_json::json!([
            { "tag_name": "android-v9.0.0" },
            { "tag_name": "web-v0.9.0" },
            { "tag_name": "web-v0.10.0" },
            { "tag_name": "web-v1.0.0", "draft": true },
            { "tag_name": "web-v1.1.0", "prerelease": true },
        ]);
        assert_eq!(latest_web_tag(&r).as_deref(), Some("0.10.0"));
        assert_eq!(latest_web_tag(&serde_json::json!([])), None);
    }

    #[test]
    fn only_github_repositories_are_queried() {
        assert_eq!(api_url("https://github.com/a/b/").as_deref(), Some("https://api.github.com/repos/a/b/releases?per_page=30"));
        assert_eq!(api_url("https://example.org/a/b"), None);
        assert_eq!(api_url("https://github.com/a"), None);
    }

    #[test]
    fn the_metrics_token_is_compared_whole() {
        assert!(token_matches("0123456789abcdef", "0123456789abcdef"));
        assert!(!token_matches("0123456789abcdef", "0123456789abcdeX"));
        assert!(!token_matches("0123456789abcdef", ""));
    }

    #[test]
    fn metrics_use_the_prometheus_text_format() {
        let mut jobs = vec![("caldav sync".to_string(), JobStat { runs: 3, errors: 1, last_run: Some(Utc::now()), ..Default::default() })];
        jobs.push(("odd \"name\"".into(), JobStat::default()));
        let s = Snapshot {
            version: "1.2.3",
            counts: Counts { accounts: 5, accounts_suspended: 1, accounts_active_30d: 4, accounts_pending_deletion: 0, workspaces: 2, personal_spaces: 5, sessions: 7 },
            db_bytes: 10,
            upload_bytes: 20,
            upload_files: 2,
            disk: Some((100, 40)),
            spaces: vec![],
            personal_bytes: 0,
            jobs,
            backup_last_at: None,
            backup_schedule: "off".into(),
        };
        let m = render_metrics(&s);
        assert!(m.contains("looty_info{version=\"1.2.3\"} 1\n"));
        assert!(m.contains("looty_accounts{state=\"suspended\"} 1\n"));
        assert!(m.contains("looty_job_errors_total{job=\"caldav sync\"} 1\n"));
        assert!(m.contains("job=\"odd \\\"name\\\"\""));
        assert!(m.contains("# TYPE looty_job_runs_total counter\n"));
        assert!(!m.contains("looty_backup_last_timestamp_seconds"));
        assert!(m.lines().all(|l| l.starts_with('#') || l.starts_with("looty_")));
    }
}

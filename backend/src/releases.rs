//! Versions et notes de version (web, application de bureau, Android), tirées de
//! `releases.json` (embarqué à la compilation). Sert la fenêtre « Nouveautés » après une
//! mise à jour et l'annonce d'une nouvelle version des applications.

use axum::{extract::State, http::StatusCode, routing::{get, put}, Json, Router};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

use crate::access::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

#[derive(Deserialize, Serialize, Clone)]
pub struct Text {
    pub en: String,
    pub fr: String,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Notes {
    pub en: Vec<String>,
    pub fr: Vec<String>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Release {
    pub platform: String,
    pub version: String,
    pub date: NaiveDate,
    pub title: Text,
    pub notes: Notes,
}

#[derive(Deserialize)]
struct File {
    repository: String,
    releases: Vec<Release>,
}

fn file() -> &'static File {
    static F: OnceLock<File> = OnceLock::new();
    F.get_or_init(|| {
        let mut f: File = serde_json::from_str(include_str!("../releases.json")).expect("releases.json invalide");
        f.releases.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| semver(&b.version).cmp(&semver(&a.version))));
        f
    })
}

/// « 1.2.3 » → (1, 2, 3) pour comparer les versions.
pub fn semver(v: &str) -> (u32, u32, u32) {
    let mut it = v.trim().trim_start_matches('v').split(['.', '-']).map(|p| p.parse::<u32>().unwrap_or(0));
    (it.next().unwrap_or(0), it.next().unwrap_or(0), it.next().unwrap_or(0))
}

fn latest(platform: &str) -> Option<&'static Release> {
    file().releases.iter().filter(|r| r.platform == platform).max_by_key(|r| semver(&r.version))
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/", get(list))
}

pub fn user_routes() -> Router<AppState> {
    Router::new().route("/seen", put(mark_seen))
}

async fn list(State(state): State<AppState>, user: Option<AuthUser>) -> ApiResult<Json<serde_json::Value>> {
    let f = file();
    let server = env!("CARGO_PKG_VERSION");
    let download = |platform: &str| latest(platform).map(|r| serde_json::json!({ "version": r.version, "date": r.date, "url": format!("{}/releases/tag/{platform}-v{}", f.repository, r.version) }));
    // « Nouveautés » : à chaque nouvelle version web, sauf pour un compte créé après celle-ci.
    let (last_seen, show) = match user {
        Some(u) => {
            let row: Option<(Option<String>, DateTime<Utc>)> =
                sqlx::query_as("SELECT last_seen_release, created_at FROM users WHERE id = $1").bind(u.id).fetch_optional(&state.pool).await?;
            match row {
                Some((seen, created)) => {
                    let current = latest("web");
                    let released = current.map(|r| r.date.and_hms_opt(23, 59, 59).unwrap().and_utc());
                    let show = seen.as_deref() != Some(server)
                        && seen.as_deref().is_none_or(|s| semver(s) < semver(server))
                        && (seen.is_some() || released.is_some_and(|d| created < d));
                    (seen, show)
                }
                None => (None, false),
            }
        }
        None => (None, false),
    };
    Ok(Json(serde_json::json!({
        "server_version": server,
        "repository": f.repository,
        "latest": { "web": download("web"), "desktop": download("desktop"), "android": download("android") },
        "releases": f.releases,
        "last_seen": last_seen,
        "show_whats_new": show,
    })))
}

#[derive(Deserialize)]
struct SeenReq {
    version: String,
}

async fn mark_seen(State(state): State<AppState>, user: AuthUser, Json(p): Json<SeenReq>) -> ApiResult<StatusCode> {
    if !file().releases.iter().any(|r| r.platform == "web" && r.version == p.version) {
        return Err(ApiError::bad_request("invalid_value"));
    }
    sqlx::query("UPDATE users SET last_seen_release = $2 WHERE id = $1").bind(user.id).bind(&p.version).execute(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_match_the_server_version() {
        // Publier une version : mettre à jour Cargo.toml ET releases.json.
        assert_eq!(latest("web").map(|r| r.version.as_str()), Some(env!("CARGO_PKG_VERSION")), "releases.json et Cargo.toml doivent annoncer la même version web");
        for r in &file().releases {
            assert!(["web", "desktop", "android"].contains(&r.platform.as_str()), "plateforme inconnue : {}", r.platform);
            assert!(!r.notes.en.is_empty() && r.notes.en.len() == r.notes.fr.len(), "notes {} {} : FR et EN doivent avoir autant d'entrées", r.platform, r.version);
            assert!(!r.title.en.is_empty() && !r.title.fr.is_empty());
        }
        assert!(semver("0.10.0") > semver("0.9.3") && semver("v1.0.0") == (1, 0, 0));
    }

    #[test]
    fn desktop_and_android_versions_match_their_builds() {
        let tauri: serde_json::Value = serde_json::from_str(include_str!("../../desktop/src-tauri/tauri.conf.json")).unwrap();
        assert_eq!(latest("desktop").map(|r| r.version.as_str()), tauri["version"].as_str(), "releases.json et tauri.conf.json");
        let gradle = include_str!("../../android/app/build.gradle.kts");
        let name = gradle.lines().find_map(|l| l.trim().strip_prefix("versionName = \"")).and_then(|l| l.strip_suffix('"'));
        assert_eq!(latest("android").map(|r| r.version.as_str()), name, "releases.json et build.gradle.kts");
    }
}

//! Moodle hors ligne : chaque réponse obtenue de la plateforme pour une personne (liste des
//! cours, contenu d'un cours, page, devoir, échéances, messages, fichier) est gardée. Si la
//! plateforme ne répond plus (panne, maintenance), la dernière copie est servie, avec
//! l'en-tête `x-looty-offline: <date de la copie>` ; seul ce que la personne a déjà ouvert
//! est disponible. La copie est sur le serveur : elle sert sur tous ses appareils.
//!
//! Fichiers : copiés sur disque jusqu'à `MOODLE_CACHE_FILE_MB` (25 Mo) chacun, dans la
//! limite de `MOODLE_CACHE_MB` (500 Mo) par personne, les plus anciens étant oubliés.

use axum::{
    http::HeaderValue,
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub const OFFLINE_HEADER: &str = "x-looty-offline";

fn mb(var: &str, default: u64) -> u64 {
    std::env::var(var).ok().and_then(|v| v.parse().ok()).unwrap_or(default) * 1024 * 1024
}
pub fn max_file() -> u64 {
    mb("MOODLE_CACHE_FILE_MB", 25)
}
fn quota() -> u64 {
    mb("MOODLE_CACHE_MB", 500)
}

/// La plateforme est injoignable ou en maintenance (et non : jeton refusé, élément absent).
pub fn is_outage(e: &ApiError) -> bool {
    match e {
        ApiError::BadRequest(m) => {
            m == "moodle_unreachable"
                || m == "moodle_bad_response"
                || m == "moodle_timeout"
                || m.starts_with("moodle_http_error: 5")
                || (m.starts_with("moodle_error") && m.to_lowercase().contains("maintenance"))
        }
        _ => false,
    }
}

pub async fn remember(state: &AppState, ws: Uuid, user: Uuid, key: &str, body: &Value) {
    let _ = sqlx::query(
        "INSERT INTO moodle_cache (workspace_id, user_id, key, body, fetched_at) VALUES ($1, $2, $3, $4, NOW())
         ON CONFLICT (workspace_id, user_id, key) DO UPDATE SET body = $4, fetched_at = NOW()",
    )
    .bind(ws)
    .bind(user)
    .bind(key)
    .bind(body)
    .execute(&state.pool)
    .await;
}

pub async fn recall(state: &AppState, ws: Uuid, user: Uuid, key: &str) -> Option<(Value, DateTime<Utc>)> {
    sqlx::query_as::<_, (Option<Value>, DateTime<Utc>)>("SELECT body, fetched_at FROM moodle_cache WHERE workspace_id = $1 AND user_id = $2 AND key = $3")
        .bind(ws)
        .bind(user)
        .bind(key)
        .fetch_optional(&state.pool)
        .await
        .ok()
        .flatten()
        .and_then(|(b, at)| b.map(|b| (b, at)))
}

/// Réponse JSON : en direct (et gardée), ou la copie si la plateforme ne répond pas.
pub async fn json_or_cached(state: &AppState, ws: Uuid, user: Uuid, key: &str, live: ApiResult<Value>) -> ApiResult<Response> {
    match live {
        Ok(v) => {
            remember(state, ws, user, key, &v).await;
            Ok(Json(v).into_response())
        }
        Err(e) if is_outage(&e) => match recall(state, ws, user, key).await {
            Some((v, at)) => Ok(offline(Json(v).into_response(), at)),
            None => Err(e),
        },
        Err(e) => Err(e),
    }
}

/// Marque une réponse servie depuis la copie.
pub fn offline(mut r: Response, at: DateTime<Utc>) -> Response {
    if let Ok(v) = HeaderValue::from_str(&at.to_rfc3339()) {
        r.headers_mut().insert(OFFLINE_HEADER, v);
    }
    r
}

fn dir(user: Uuid) -> PathBuf {
    PathBuf::from(crate::upload::UPLOAD_DIR).join("moodle-cache").join(user.to_string())
}

/// Garde un fichier (déjà lu en entier) et fait de la place si besoin.
pub async fn remember_file(state: &AppState, ws: Uuid, user: Uuid, url: &str, mime: &str, name: &str, bytes: &[u8]) {
    let d = dir(user);
    if tokio::fs::create_dir_all(&d).await.is_err() {
        return;
    }
    let fname = hex::encode(Sha256::digest(format!("{ws}:{url}").as_bytes()));
    let path = d.join(&fname);
    if tokio::fs::write(&path, bytes).await.is_err() {
        return;
    }
    let body = serde_json::json!({ "mime": mime, "name": name });
    let _ = sqlx::query(
        "INSERT INTO moodle_cache (workspace_id, user_id, key, body, file_path, size, fetched_at) VALUES ($1, $2, $3, $4, $5, $6, NOW())
         ON CONFLICT (workspace_id, user_id, key) DO UPDATE SET body = $4, file_path = $5, size = $6, fetched_at = NOW()",
    )
    .bind(ws)
    .bind(user)
    .bind(format!("file:{url}"))
    .bind(body)
    .bind(path.to_string_lossy().to_string())
    .bind(bytes.len() as i64)
    .execute(&state.pool)
    .await;
    evict(state, user).await;
}

/// Fichier gardé : (type, nom, contenu, date de la copie).
pub async fn recall_file(state: &AppState, ws: Uuid, user: Uuid, url: &str) -> Option<(String, String, Vec<u8>, DateTime<Utc>)> {
    let (body, path, at): (Option<Value>, Option<String>, DateTime<Utc>) =
        sqlx::query_as("SELECT body, file_path, fetched_at FROM moodle_cache WHERE workspace_id = $1 AND user_id = $2 AND key = $3")
            .bind(ws)
            .bind(user)
            .bind(format!("file:{url}"))
            .fetch_optional(&state.pool)
            .await
            .ok()
            .flatten()?;
    let bytes = tokio::fs::read(path?).await.ok()?;
    let body = body.unwrap_or_default();
    Some((body["mime"].as_str().unwrap_or("application/octet-stream").to_string(), body["name"].as_str().unwrap_or("file").to_string(), bytes, at))
}

/// Au-delà du quota de la personne : les fichiers les plus anciens sont oubliés.
async fn evict(state: &AppState, user: Uuid) {
    let rows: Vec<(Uuid, String, String, i64)> = sqlx::query_as(
        "SELECT workspace_id, key, file_path, size FROM moodle_cache WHERE user_id = $1 AND file_path IS NOT NULL ORDER BY fetched_at DESC",
    )
    .bind(user)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();
    let mut used: u64 = 0;
    for (ws, key, path, size) in rows {
        used += size.max(0) as u64;
        if used > quota() {
            let _ = tokio::fs::remove_file(&path).await;
            let _ = sqlx::query("DELETE FROM moodle_cache WHERE workspace_id = $1 AND user_id = $2 AND key = $3").bind(ws).bind(user).bind(&key).execute(&state.pool).await;
        }
    }
}

/// Moodle déconnecté de l'espace : la copie est effacée.
pub async fn forget(state: &AppState, ws: Uuid, user: Uuid) {
    let files: Vec<String> = sqlx::query_scalar("DELETE FROM moodle_cache WHERE workspace_id = $1 AND user_id = $2 RETURNING file_path")
        .bind(ws)
        .bind(user)
        .fetch_all(&state.pool)
        .await
        .map(|v: Vec<Option<String>>| v.into_iter().flatten().collect())
        .unwrap_or_default();
    for f in files {
        let _ = tokio::fs::remove_file(f).await;
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outages() {
        assert!(is_outage(&ApiError::bad_request("moodle_unreachable")));
        assert!(is_outage(&ApiError::BadRequest("moodle_http_error: 503".into())));
        assert!(is_outage(&ApiError::BadRequest("moodle_error: Site is in maintenance mode".into())));
        assert!(!is_outage(&ApiError::bad_request("moodle_invalid_token")));
        assert!(!is_outage(&ApiError::BadRequest("moodle_http_error: 404".into())));
        assert!(!is_outage(&ApiError::NotFound));
    }
}

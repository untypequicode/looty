//! Emploi du temps → documents.
//!
//! Un calendrier de l'espace (souvent un abonnement ICS à l'emploi du temps) contient les
//! cours. Des règles relient les cours dont le titre correspond à un filtre (« Algèbre »,
//! « TD analyse, TP analyse ») à un document, avec ou sans ses sous-pages. L'accueil met en
//! avant le cours en cours et ses documents, puis les prochains cours de la journée.
//!
//! Filtre : des variantes séparées par des virgules ; dans une variante, tous les mots
//! doivent figurer dans le titre (sans tenir compte des majuscules ni des accents).

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, patch},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::HashMap;
use uuid::Uuid;

use crate::access::{require_member, require_perm, visible_pages, AuthUser, PageAccess};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub fn workspace_routes() -> Router<AppState> {
    Router::new()
        .route("/class-links", get(list_links).post(create_link))
        .route("/class-links/{id}", patch(update_link).delete(delete_link))
        .route("/classes", get(classes))
}

/// Minuscules sans accents, pour comparer titres et filtres.
pub fn fold(s: &str) -> String {
    s.chars()
        .flat_map(|c| c.to_lowercase())
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
            'ç' => 'c',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ñ' => 'n',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            'ý' | 'ÿ' => 'y',
            'œ' => 'o',
            'æ' => 'a',
            c => c,
        })
        .collect()
}

/// Le titre correspond-il au filtre (une des variantes, tous ses mots) ?
pub fn matches(pattern: &str, title: &str) -> bool {
    let title = fold(title);
    pattern.split([',', ';']).map(str::trim).filter(|v| !v.is_empty()).any(|variant| {
        let words: Vec<String> = variant.split_whitespace().map(fold).collect();
        !words.is_empty() && words.iter().all(|w| title.contains(w.as_str()))
    })
}

#[derive(sqlx::FromRow)]
struct LinkRow {
    id: Uuid,
    calendar_id: Option<Uuid>,
    pattern: String,
    page_id: Uuid,
    include_subpages: bool,
}

/// Calendriers visibles : ceux de l'espace et les miens.
async fn my_calendars(state: &AppState, ws: Uuid, user: Uuid) -> ApiResult<HashMap<Uuid, (String, String)>> {
    let rows: Vec<(Uuid, String, String)> = sqlx::query_as("SELECT id, name, color FROM calendars WHERE workspace_id = $1 OR user_id = $2")
        .bind(ws)
        .bind(user)
        .fetch_all(&state.pool)
        .await?;
    Ok(rows.into_iter().map(|(id, n, c)| (id, (n, c))).collect())
}

async fn list_links(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>) -> ApiResult<Json<Vec<serde_json::Value>>> {
    let role = require_member(&state.pool, &user, ws).await?;
    let visible = visible_pages(&state.pool, &user, ws, role).await?;
    let cals = my_calendars(&state, ws, user.id).await?;
    let rows: Vec<(Uuid, Option<Uuid>, String, Uuid, bool, String, Option<String>)> = sqlx::query_as(
        "SELECT l.id, l.calendar_id, l.pattern, l.page_id, l.include_subpages, p.title, p.icon FROM class_links l
         JOIN pages p ON p.id = l.page_id WHERE l.workspace_id = $1 ORDER BY l.position, l.created_at",
    )
    .bind(ws)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(
        rows.into_iter()
            .filter(|r| visible.get(&r.3).is_some_and(|a| *a != PageAccess::None))
            .map(|(id, cal, pattern, page, sub, title, icon)| {
                serde_json::json!({
                    "id": id, "calendar_id": cal, "calendar_name": cal.and_then(|c| cals.get(&c).map(|x| x.0.clone())),
                    "pattern": pattern, "page_id": page, "page_title": title, "page_icon": icon, "include_subpages": sub,
                })
            })
            .collect(),
    ))
}

#[derive(Deserialize)]
struct LinkReq {
    calendar_id: Option<Uuid>,
    pattern: Option<String>,
    page_id: Option<Uuid>,
    include_subpages: Option<bool>,
}

fn clean_pattern(p: &str) -> ApiResult<String> {
    let p = p.trim();
    if p.is_empty() || p.chars().count() > 200 || p.split([',', ';']).all(|v| v.trim().is_empty()) {
        return Err(ApiError::bad_request("class_pattern_invalid"));
    }
    Ok(p.to_string())
}

async fn check_refs(state: &AppState, user: &AuthUser, ws: Uuid, calendar: Option<Uuid>, page: Option<Uuid>) -> ApiResult<()> {
    if let Some(c) = calendar {
        if !my_calendars(state, ws, user.id).await?.contains_key(&c) {
            return Err(ApiError::bad_request("calendar_invalid"));
        }
    }
    if let Some(p) = page {
        let (pw, _, _) = crate::access::require_page_read(&state.pool, user, p).await?;
        if pw != ws {
            return Err(ApiError::NotFound);
        }
    }
    Ok(())
}

async fn create_link(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>, Json(p): Json<LinkReq>) -> ApiResult<Json<serde_json::Value>> {
    require_perm(&state.pool, &user, ws, |x| x.calendar >= crate::access::level::EDIT).await?;
    let pattern = clean_pattern(p.pattern.as_deref().unwrap_or(""))?;
    let page = p.page_id.ok_or_else(|| ApiError::bad_request("page_required"))?;
    check_refs(&state, &user, ws, p.calendar_id, Some(page)).await?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM class_links WHERE workspace_id = $1").bind(ws).fetch_one(&state.pool).await?;
    if count >= 500 {
        return Err(ApiError::bad_request("class_too_many"));
    }
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO class_links (workspace_id, calendar_id, pattern, page_id, include_subpages, position, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
    )
    .bind(ws)
    .bind(p.calendar_id)
    .bind(&pattern)
    .bind(page)
    .bind(p.include_subpages.unwrap_or(true))
    .bind(count as i32)
    .bind(user.id)
    .fetch_one(&state.pool)
    .await?;
    changed(&state, ws).await;
    Ok(Json(serde_json::json!({ "id": id })))
}

async fn update_link(State(state): State<AppState>, user: AuthUser, Path((ws, id)): Path<(Uuid, Uuid)>, Json(p): Json<LinkReq>) -> ApiResult<StatusCode> {
    require_perm(&state.pool, &user, ws, |x| x.calendar >= crate::access::level::EDIT).await?;
    let pattern = p.pattern.as_deref().map(clean_pattern).transpose()?;
    check_refs(&state, &user, ws, p.calendar_id, p.page_id).await?;
    let n = sqlx::query(
        "UPDATE class_links SET pattern = COALESCE($3, pattern), page_id = COALESCE($4, page_id),
                include_subpages = COALESCE($5, include_subpages), calendar_id = CASE WHEN $6 THEN $7 ELSE calendar_id END
         WHERE id = $1 AND workspace_id = $2",
    )
    .bind(id)
    .bind(ws)
    .bind(pattern)
    .bind(p.page_id)
    .bind(p.include_subpages)
    .bind(p.calendar_id.is_some())
    .bind(p.calendar_id)
    .execute(&state.pool)
    .await?
    .rows_affected();
    if n == 0 {
        return Err(ApiError::NotFound);
    }
    changed(&state, ws).await;
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_link(State(state): State<AppState>, user: AuthUser, Path((ws, id)): Path<(Uuid, Uuid)>) -> ApiResult<StatusCode> {
    require_perm(&state.pool, &user, ws, |x| x.calendar >= crate::access::level::EDIT).await?;
    sqlx::query("DELETE FROM class_links WHERE id = $1 AND workspace_id = $2").bind(id).bind(ws).execute(&state.pool).await?;
    changed(&state, ws).await;
    Ok(StatusCode::NO_CONTENT)
}

async fn changed(state: &AppState, ws: Uuid) {
    state.emit(ws, serde_json::json!({ "type": "CLASS_LINKS_UPDATED" })).await;
}

#[derive(Deserialize)]
struct RangeQuery {
    from: DateTime<Utc>,
    to: DateTime<Utc>,
}

/// Cours de la période reliés à des documents (cours en cours et suivants pour l'accueil).
async fn classes(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>, Query(q): Query<RangeQuery>) -> ApiResult<Json<Vec<serde_json::Value>>> {
    let role = require_member(&state.pool, &user, ws).await?;
    if q.to <= q.from || q.to - q.from > chrono::Duration::days(14) {
        return Err(ApiError::bad_request("range_invalid"));
    }
    let links: Vec<LinkRow> = sqlx::query_as(
        "SELECT id, calendar_id, pattern, page_id, include_subpages FROM class_links WHERE workspace_id = $1 ORDER BY position, created_at",
    )
    .bind(ws)
    .fetch_all(&state.pool)
    .await?;
    if links.is_empty() {
        return Ok(Json(vec![]));
    }
    let visible = visible_pages(&state.pool, &user, ws, role).await?;
    let readable = |id: &Uuid| visible.get(id).is_some_and(|a| *a != PageAccess::None);
    let cals = my_calendars(&state, ws, user.id).await?;
    let cal_ids: Vec<Uuid> = cals.keys().copied().collect();
    let events: Vec<(Uuid, Uuid, String, Option<String>, DateTime<Utc>, DateTime<Utc>, bool)> = sqlx::query_as(
        "SELECT id, calendar_id, summary, location, starts_at, ends_at, all_day FROM calendar_events
         WHERE calendar_id = ANY($1) AND starts_at < $3 AND ends_at > $2 ORDER BY starts_at LIMIT 500",
    )
    .bind(&cal_ids)
    .bind(q.from)
    .bind(q.to)
    .fetch_all(&state.pool)
    .await?;
    // Pages et arborescence (pour les sous-pages).
    let tree: Vec<(Uuid, Option<Uuid>, String, Option<String>, f64)> =
        sqlx::query_as("SELECT id, parent_id, title, icon, position FROM pages WHERE workspace_id = $1").bind(ws).fetch_all(&state.pool).await?;
    let by_id: HashMap<Uuid, &(Uuid, Option<Uuid>, String, Option<String>, f64)> = tree.iter().map(|p| (p.0, p)).collect();
    let mut kids: HashMap<Uuid, Vec<&(Uuid, Option<Uuid>, String, Option<String>, f64)>> = HashMap::new();
    for p in &tree {
        if let Some(parent) = p.1 {
            kids.entry(parent).or_default().push(p);
        }
    }
    for v in kids.values_mut() {
        v.sort_by(|a, b| a.4.partial_cmp(&b.4).unwrap_or(std::cmp::Ordering::Equal));
    }
    let doc = |page: Uuid, sub: bool| -> Option<serde_json::Value> {
        let p = by_id.get(&page)?;
        if !readable(&page) {
            return None;
        }
        let mut children = Vec::new();
        if sub {
            // Sous-pages lisibles, dans l'ordre de l'arborescence (au plus 60).
            let mut stack: Vec<(Uuid, usize)> = kids.get(&page).map(|v| v.iter().rev().map(|c| (c.0, 1)).collect()).unwrap_or_default();
            while let Some((id, depth)) = stack.pop() {
                if children.len() >= 60 {
                    break;
                }
                if !readable(&id) {
                    continue;
                }
                let c = by_id[&id];
                children.push(serde_json::json!({ "id": id, "title": c.2, "icon": c.3, "depth": depth }));
                if depth < 6 {
                    if let Some(v) = kids.get(&id) {
                        stack.extend(v.iter().rev().map(|x| (x.0, depth + 1)));
                    }
                }
            }
        }
        Some(serde_json::json!({ "id": page, "title": p.2, "icon": p.3, "children": children }))
    };
    let mut out = Vec::new();
    for (id, cal, summary, location, start, end, all_day) in events {
        let docs: Vec<serde_json::Value> = links
            .iter()
            .filter(|l| l.calendar_id.is_none_or(|c| c == cal) && matches(&l.pattern, &summary))
            .filter_map(|l| doc(l.page_id, l.include_subpages).map(|mut d| {
                d["link_id"] = l.id.to_string().into();
                d
            }))
            .collect();
        if docs.is_empty() {
            continue;
        }
        let (cal_name, color) = cals.get(&cal).cloned().unwrap_or_default();
        out.push(serde_json::json!({
            "id": id, "calendar_id": cal, "calendar_name": cal_name, "color": color, "summary": summary, "location": location,
            "starts_at": start, "ends_at": end, "all_day": all_day, "documents": docs,
        }));
    }
    Ok(Json(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_variants_and_words() {
        assert!(matches("Algèbre", "CM ALGEBRE linéaire - Amphi A"));
        assert!(matches("td analyse, tp analyse", "TP Analyse numérique"));
        assert!(!matches("td analyse", "TP Analyse numérique"));
        assert!(matches("Élec", "electronique"));
        assert!(!matches(" , ", "anything"));
    }
}

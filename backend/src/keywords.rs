//! Mots-clés (GLOSSARY) : termes libres posés sur un élément de l'espace (document,
//! projet, fichier du Drive, tableau, tâche, événement) pour le retrouver par la
//! recherche. Contrairement aux étiquettes d'un tableau, ils sont transverses à l'espace,
//! sans couleur ni droit.
//!
//! Lire les mots-clés d'un élément demande de pouvoir le lire, les modifier de pouvoir le
//! modifier (mêmes règles que l'élément lui-même).

use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::access::{level, member_perms, AuthUser};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub const MAX_KEYWORDS: usize = 20;
pub const MAX_LEN: usize = 50;

pub fn routes() -> Router<AppState> {
    Router::new().route("/{kind}/{id}", get(get_keywords).put(put_keywords))
}

pub fn workspace_routes() -> Router<AppState> {
    Router::new().route("/", get(list_workspace))
}

/// Nettoie une liste saisie : espaces superflus et `#` initial retirés, vides ignorés,
/// doublons (sans casse ni accents) fusionnés ; casse conservée.
pub fn normalize(list: &[String]) -> ApiResult<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for raw in list {
        let k = raw.trim().trim_start_matches('#').split_whitespace().collect::<Vec<_>>().join(" ");
        if k.is_empty() {
            continue;
        }
        if k.chars().count() > MAX_LEN {
            return Err(ApiError::bad_request("keyword_too_long"));
        }
        if seen.insert(crate::classes::fold(&k)) {
            out.push(k);
        }
    }
    if out.len() > MAX_KEYWORDS {
        return Err(ApiError::bad_request("too_many_keywords"));
    }
    Ok(out)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Page,
    Task,
    Board,
    Project,
    File,
    Event,
}

impl Kind {
    fn parse(s: &str) -> ApiResult<Self> {
        Ok(match s {
            "page" => Kind::Page,
            "task" => Kind::Task,
            "board" => Kind::Board,
            "project" => Kind::Project,
            "file" => Kind::File,
            "event" => Kind::Event,
            _ => return Err(ApiError::NotFound),
        })
    }
    fn table(self) -> &'static str {
        match self {
            Kind::Page => "pages",
            Kind::Task => "tasks",
            Kind::Board => "boards",
            Kind::Project => "projects",
            Kind::File => "drive_files",
            Kind::Event => "workspace_events",
        }
    }
}

/// Vérifie l'accès à l'élément ; renvoie s'il est modifiable. NotFound si illisible.
async fn access(state: &AppState, user: &AuthUser, kind: Kind, id: Uuid) -> ApiResult<bool> {
    let pool = &state.pool;
    let by_domain = |ws: Option<Uuid>, domain: fn(&crate::access::Perms) -> u8| async move {
        let ws = ws.ok_or(ApiError::NotFound)?;
        let (_, perms) = member_perms(pool, user.id, ws).await?.ok_or(ApiError::NotFound)?;
        let l = domain(&perms);
        if l < level::READ {
            return Err(ApiError::NotFound);
        }
        Ok(l >= level::EDIT)
    };
    match kind {
        Kind::Page => {
            let (_, _, a) = crate::access::require_page_read(pool, user, id).await?;
            Ok(a == crate::access::PageAccess::Write)
        }
        Kind::Project => {
            let read = crate::projects::project_for(state, user, id, crate::projects::Need::Read).await;
            read?;
            Ok(crate::projects::project_for(state, user, id, crate::projects::Need::Write).await.is_ok())
        }
        Kind::Task => {
            let ws = crate::access::task_ctx(pool, id).await?.workspace_id;
            by_domain(Some(ws), |p| p.boards).await
        }
        Kind::Board => by_domain(Some(crate::access::board_workspace(pool, id).await?), |p| p.boards).await,
        Kind::File => {
            let ws = sqlx::query_scalar("SELECT workspace_id FROM drive_files WHERE id = $1").bind(id).fetch_optional(pool).await?;
            by_domain(ws, |p| p.docs).await
        }
        Kind::Event => {
            let ws = sqlx::query_scalar("SELECT workspace_id FROM workspace_events WHERE id = $1").bind(id).fetch_optional(pool).await?;
            by_domain(ws, |p| p.calendar).await
        }
    }
}

#[derive(Serialize)]
struct ItemKeywords {
    keywords: Vec<String>,
    can_edit: bool,
}

async fn get_keywords(State(state): State<AppState>, user: AuthUser, Path((kind, id)): Path<(String, Uuid)>) -> ApiResult<Json<ItemKeywords>> {
    let kind = Kind::parse(&kind)?;
    let can_edit = access(&state, &user, kind, id).await?;
    let keywords: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT keywords FROM {} WHERE id = $1", kind.table())))
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(ItemKeywords { keywords, can_edit }))
}

#[derive(Deserialize)]
struct PutKeywords {
    keywords: Vec<String>,
}

async fn put_keywords(
    State(state): State<AppState>,
    user: AuthUser,
    Path((kind, id)): Path<(String, Uuid)>,
    Json(req): Json<PutKeywords>,
) -> ApiResult<Json<ItemKeywords>> {
    let kind = Kind::parse(&kind)?;
    if !access(&state, &user, kind, id).await? {
        return Err(ApiError::Forbidden);
    }
    let keywords = normalize(&req.keywords)?;
    sqlx::query(sqlx::AssertSqlSafe(format!("UPDATE {} SET keywords = $2 WHERE id = $1", kind.table())))
        .bind(id)
        .bind(&keywords)
        .execute(&state.pool)
        .await?;
    Ok(Json(ItemKeywords { keywords, can_edit: true }))
}

#[derive(Serialize, sqlx::FromRow)]
struct KeywordUse {
    keyword: String,
    count: i64,
}

/// Mots-clés déjà employés dans l'espace (autocomplétion), les plus fréquents d'abord.
/// Une même forme pliée n'apparaît qu'une fois, sous son écriture la plus courante.
async fn list_workspace(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>) -> ApiResult<Json<Vec<KeywordUse>>> {
    crate::access::require_member(&state.pool, &user, ws).await?;
    let rows: Vec<KeywordUse> = sqlx::query_as(
        "WITH k AS (
            SELECT unnest(keywords) AS k FROM pages WHERE workspace_id = $1
            UNION ALL SELECT unnest(t.keywords) FROM tasks t JOIN board_columns c ON c.id = t.column_id JOIN boards b ON b.id = c.board_id WHERE b.workspace_id = $1
            UNION ALL SELECT unnest(keywords) FROM boards WHERE workspace_id = $1
            UNION ALL SELECT unnest(keywords) FROM projects WHERE workspace_id = $1
            UNION ALL SELECT unnest(keywords) FROM drive_files WHERE workspace_id = $1
            UNION ALL SELECT unnest(keywords) FROM workspace_events WHERE workspace_id = $1
         ), f AS (SELECT looty_fold(k) AS f, k, count(*) AS n FROM k GROUP BY 1, 2)
         SELECT DISTINCT ON (f) k AS keyword, sum(n) OVER (PARTITION BY f)::bigint AS count
         FROM f ORDER BY f, n DESC, k",
    )
    .bind(ws)
    .fetch_all(&state.pool)
    .await?;
    let mut rows = rows;
    rows.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.keyword.cmp(&b.keyword)));
    rows.truncate(500);
    Ok(Json(rows))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn normalize_trims_and_merges_duplicates() {
        let out = normalize(&s(&["  INF301 ", "#partiel", "", "Partiel", "TP   noté", "Algèbre", "algebre"])).unwrap();
        assert_eq!(out, s(&["INF301", "partiel", "TP noté", "Algèbre"]));
    }

    #[test]
    fn normalize_rejects_long_or_too_many() {
        assert!(normalize(&[("x".repeat(MAX_LEN + 1))]).is_err());
        assert!(normalize(&[("é".repeat(MAX_LEN))]).is_ok());
        let many: Vec<String> = (0..=MAX_KEYWORDS).map(|i| format!("k{i}")).collect();
        assert!(normalize(&many).is_err());
    }
}

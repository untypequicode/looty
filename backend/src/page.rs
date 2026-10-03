use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Deserializer, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::access::{
    is_member, require_member, require_page_read, require_page_write, require_writer, visible_pages, AuthUser,
    PageAccess,
};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

#[derive(Serialize, Deserialize, FromRow, Clone)]
pub struct Page {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub icon: Option<String>,
    pub title: String,
    pub content: Option<String>,
    pub created_by: Uuid,
    pub updated_by: Option<Uuid>,
    pub position: f64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub share_token: Option<Uuid>,
    pub is_public: Option<bool>,
    // Champs calculés par utilisateur (absents de la table `pages`).
    #[sqlx(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_favorite: Option<bool>,
    #[sqlx(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_count: Option<i32>,
    #[sqlx(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_viewed_at: Option<chrono::DateTime<chrono::Utc>>,
    #[sqlx(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_view_count: Option<i64>,
    #[sqlx(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub can_edit: Option<bool>,
}

/// Version allégée pour les listes / l'arbre (sans le contenu HTML).
const PAGE_LIST_COLUMNS: &str = "p.id, p.workspace_id, p.parent_id, p.icon, p.title, NULL::text AS content,
    p.created_by, p.updated_by, p.position, p.created_at, p.updated_at, p.share_token, p.is_public";

/// Distingue un champ absent (`None`) d'un champ explicitement `null` (`Some(None)`).
fn double_option<'de, D, T>(de: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(de).map(Some)
}

#[derive(Deserialize)]
pub struct CreatePage {
    pub title: Option<String>,
    pub icon: Option<String>,
    pub parent_id: Option<Uuid>,
    pub content: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdatePage {
    pub title: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    pub icon: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub parent_id: Option<Option<Uuid>>,
    pub content: Option<String>,
    pub position: Option<f64>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_pages).post(create_page))
        .route("/:page_id", get(get_page).put(update_page).delete(delete_page))
        .route("/:page_id/favorite", post(toggle_favorite))
        .route("/:page_id/view", post(record_view))
        .route("/:page_id/share", post(share_page).delete(revoke_share))
        .route("/:page_id/permissions", get(get_page_permissions).put(set_page_permission))
}

/// Routes hors workspace : contrôle d'accès pour le serveur de collaboration.
pub fn access_routes() -> Router<AppState> {
    Router::new().route("/:page_id/access", get(page_access_check))
}

fn check_title(t: &str) -> ApiResult<String> {
    let t = t.trim();
    if t.chars().count() > 255 {
        return Err(ApiError::bad_request("title_too_long"));
    }
    Ok(t.to_string())
}

fn check_icon(icon: &str) -> ApiResult<()> {
    if icon.chars().count() > 32 {
        return Err(ApiError::bad_request("invalid_icon"));
    }
    Ok(())
}

async fn ensure_in_workspace(pool: &PgPool, page_id: Uuid, workspace_id: Uuid) -> ApiResult<()> {
    if crate::access::page_workspace(pool, page_id).await? != workspace_id {
        return Err(ApiError::NotFound);
    }
    Ok(())
}

async fn list_pages(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path(workspace_id): Path<Uuid>,
) -> ApiResult<Json<Vec<Page>>> {
    let role = require_member(&pool, &user, workspace_id).await?;
    let visible = visible_pages(&pool, &user, workspace_id, role).await?;
    let sql = format!(
        "SELECT {PAGE_LIST_COLUMNS},
            (pf.page_id IS NOT NULL) AS is_favorite,
            pv.view_count, pv.last_viewed_at,
            (SELECT COALESCE(SUM(view_count), 0) FROM page_views WHERE page_id = p.id) AS total_view_count
         FROM pages p
         LEFT JOIN page_favorites pf ON pf.page_id = p.id AND pf.user_id = $2
         LEFT JOIN page_views pv ON pv.page_id = p.id AND pv.user_id = $2
         WHERE p.workspace_id = $1
         ORDER BY p.position ASC, p.created_at ASC"
    );
    let pages: Vec<Page> = sqlx::query_as(&sql).bind(workspace_id).bind(user.id).fetch_all(&pool).await?;
    Ok(Json(
        pages
            .into_iter()
            .filter_map(|mut p| match visible.get(&p.id) {
                Some(PageAccess::None) | None => None,
                Some(a) => {
                    p.can_edit = Some(*a == PageAccess::Write);
                    Some(p)
                }
            })
            .collect(),
    ))
}

async fn get_page(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path((workspace_id, page_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<Page>> {
    let (ws, _, access) = require_page_read(&pool, &user, page_id).await?;
    if ws != workspace_id {
        return Err(ApiError::NotFound);
    }
    let mut page: Page = sqlx::query_as(
        "SELECT p.*,
            (pf.page_id IS NOT NULL) AS is_favorite,
            pv.view_count, pv.last_viewed_at,
            (SELECT COALESCE(SUM(view_count), 0) FROM page_views WHERE page_id = p.id) AS total_view_count
         FROM pages p
         LEFT JOIN page_favorites pf ON pf.page_id = p.id AND pf.user_id = $2
         LEFT JOIN page_views pv ON pv.page_id = p.id AND pv.user_id = $2
         WHERE p.id = $1",
    )
    .bind(page_id)
    .bind(user.id)
    .fetch_one(&pool)
    .await?;
    page.can_edit = Some(access == PageAccess::Write);
    Ok(Json(page))
}

async fn create_page(
    State(state): State<AppState>,
    user: AuthUser,
    Path(workspace_id): Path<Uuid>,
    Json(payload): Json<CreatePage>,
) -> ApiResult<(StatusCode, Json<Page>)> {
    let pool = &state.pool;
    require_writer(pool, &user, workspace_id).await?;
    if let Some(parent) = payload.parent_id {
        let (ws, _) = require_page_write(pool, &user, parent).await?;
        if ws != workspace_id {
            return Err(ApiError::NotFound);
        }
    }
    let title = check_title(payload.title.as_deref().unwrap_or(""))?;
    if let Some(i) = &payload.icon {
        check_icon(i)?;
    }
    let position: f64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(position), 0) + 65536 FROM pages WHERE workspace_id = $1 AND parent_id IS NOT DISTINCT FROM $2",
    )
    .bind(workspace_id)
    .bind(payload.parent_id)
    .fetch_one(pool)
    .await?;

    let page: Page = sqlx::query_as(
        "INSERT INTO pages (workspace_id, parent_id, icon, title, content, created_by, updated_by, position)
         VALUES ($1, $2, $3, $4, $5, $6, $6, $7) RETURNING *",
    )
    .bind(workspace_id)
    .bind(payload.parent_id)
    .bind(payload.icon)
    .bind(title)
    .bind(payload.content)
    .bind(user.id)
    .bind(position)
    .fetch_one(pool)
    .await?;

    let mut lite = page.clone();
    lite.content = None;
    state.emit(workspace_id, serde_json::json!({ "type": "PAGE_CREATED", "page": lite })).await;
    Ok((StatusCode::CREATED, Json(page)))
}

async fn update_page(
    State(state): State<AppState>,
    user: AuthUser,
    Path((workspace_id, page_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<UpdatePage>,
) -> ApiResult<Json<Page>> {
    let pool = &state.pool;
    let (ws, _) = require_page_write(pool, &user, page_id).await?;
    if ws != workspace_id {
        return Err(ApiError::NotFound);
    }

    let title = payload.title.as_deref().map(check_title).transpose()?;
    if let Some(Some(i)) = &payload.icon {
        check_icon(i)?;
    }

    // Déplacement : le nouveau parent doit être modifiable, du même workspace,
    // et ne pas être la page elle-même ni l'un de ses descendants.
    if let Some(Some(new_parent)) = payload.parent_id {
        let (pws, _) = require_page_write(pool, &user, new_parent).await?;
        if pws != workspace_id || new_parent == page_id {
            return Err(ApiError::bad_request("invalid_parent"));
        }
        let creates_cycle: bool = sqlx::query_scalar(
            "WITH RECURSIVE anc AS (
                SELECT id, parent_id, 0 AS depth FROM pages WHERE id = $1
                UNION ALL
                SELECT p.id, p.parent_id, anc.depth + 1 FROM pages p JOIN anc ON p.id = anc.parent_id WHERE anc.depth < 64
             ) SELECT EXISTS(SELECT 1 FROM anc WHERE id = $2)",
        )
        .bind(new_parent)
        .bind(page_id)
        .fetch_one(pool)
        .await?;
        if creates_cycle {
            return Err(ApiError::bad_request("invalid_parent"));
        }
    }

    let page: Page = sqlx::query_as(
        "UPDATE pages SET
            title = COALESCE($1, title),
            icon = CASE WHEN $2 THEN $3 ELSE icon END,
            parent_id = CASE WHEN $4 THEN $5 ELSE parent_id END,
            content = COALESCE($6, content),
            position = COALESCE($7, position),
            updated_by = $8,
            updated_at = CURRENT_TIMESTAMP
         WHERE id = $9 RETURNING *",
    )
    .bind(title)
    .bind(payload.icon.is_some())
    .bind(payload.icon.clone().flatten())
    .bind(payload.parent_id.is_some())
    .bind(payload.parent_id.flatten())
    .bind(payload.content)
    .bind(payload.position)
    .bind(user.id)
    .bind(page_id)
    .fetch_one(pool)
    .await?;

    let mut lite = page.clone();
    lite.content = None;
    state.emit(workspace_id, serde_json::json!({ "type": "PAGE_UPDATED", "page": lite })).await;
    Ok(Json(page))
}

async fn delete_page(
    State(state): State<AppState>,
    user: AuthUser,
    Path((workspace_id, page_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    let (ws, _) = require_page_write(&state.pool, &user, page_id).await?;
    if ws != workspace_id {
        return Err(ApiError::NotFound);
    }
    sqlx::query("DELETE FROM pages WHERE id = $1").bind(page_id).execute(&state.pool).await?;
    state.emit(workspace_id, serde_json::json!({ "type": "PAGE_DELETED", "page_id": page_id })).await;
    Ok(StatusCode::NO_CONTENT)
}

async fn share_page(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path((workspace_id, page_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<Page>> {
    let (ws, _) = require_page_write(&pool, &user, page_id).await?;
    if ws != workspace_id {
        return Err(ApiError::NotFound);
    }
    Ok(Json(
        sqlx::query_as(
            "UPDATE pages SET share_token = COALESCE(share_token, gen_random_uuid()), is_public = true
             WHERE id = $1 RETURNING *",
        )
        .bind(page_id)
        .fetch_one(&pool)
        .await?,
    ))
}

async fn revoke_share(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path((workspace_id, page_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<Page>> {
    let (ws, _) = require_page_write(&pool, &user, page_id).await?;
    if ws != workspace_id {
        return Err(ApiError::NotFound);
    }
    Ok(Json(
        sqlx::query_as("UPDATE pages SET share_token = NULL, is_public = false WHERE id = $1 RETURNING *")
            .bind(page_id)
            .fetch_one(&pool)
            .await?,
    ))
}

#[derive(Serialize, FromRow)]
pub struct PublicPage {
    pub id: Uuid,
    pub title: String,
    pub icon: Option<String>,
    pub content: Option<String>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

pub async fn get_public_page(State(pool): State<PgPool>, Path(token): Path<Uuid>) -> ApiResult<Json<PublicPage>> {
    Ok(Json(
        sqlx::query_as(
            "SELECT id, title, icon, content, updated_at FROM pages WHERE share_token = $1 AND is_public = true",
        )
        .bind(token)
        .fetch_optional(&pool)
        .await?
        .ok_or(ApiError::NotFound)?,
    ))
}

#[derive(Serialize, FromRow)]
pub struct PagePermission {
    pub id: Uuid,
    pub page_id: Uuid,
    pub user_id: Uuid,
    pub role: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
pub struct SetPagePermissionReq {
    pub user_id: Uuid,
    pub role: String,
}

async fn get_page_permissions(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path((workspace_id, page_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<Vec<PagePermission>>> {
    let (ws, role, _) = require_page_read(&pool, &user, page_id).await?;
    if ws != workspace_id || !role.is_admin() {
        return Err(ApiError::Forbidden);
    }
    Ok(Json(
        sqlx::query_as("SELECT id, page_id, user_id, role, created_at FROM page_permissions WHERE page_id = $1")
            .bind(page_id)
            .fetch_all(&pool)
            .await?,
    ))
}

async fn set_page_permission(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path((workspace_id, page_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<SetPagePermissionReq>,
) -> ApiResult<StatusCode> {
    ensure_in_workspace(&pool, page_id, workspace_id).await?;
    let role = require_member(&pool, &user, workspace_id).await?;
    if !role.is_admin() {
        return Err(ApiError::Forbidden);
    }
    if !is_member(&pool, workspace_id, &payload.user_id.to_string()).await? {
        return Err(ApiError::bad_request("not_a_member"));
    }
    match payload.role.as_str() {
        "none" => {
            sqlx::query("DELETE FROM page_permissions WHERE page_id = $1 AND user_id = $2")
                .bind(page_id)
                .bind(payload.user_id)
                .execute(&pool)
                .await?;
        }
        "viewer" | "editor" | "blocked" => {
            sqlx::query(
                "INSERT INTO page_permissions (page_id, user_id, role) VALUES ($1, $2, $3)
                 ON CONFLICT (page_id, user_id) DO UPDATE SET role = $3",
            )
            .bind(page_id)
            .bind(payload.user_id)
            .bind(&payload.role)
            .execute(&pool)
            .await?;
        }
        _ => return Err(ApiError::bad_request("invalid_role")),
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn toggle_favorite(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path((workspace_id, page_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<serde_json::Value>> {
    let (ws, _, _) = require_page_read(&pool, &user, page_id).await?;
    if ws != workspace_id {
        return Err(ApiError::NotFound);
    }
    let removed = sqlx::query("DELETE FROM page_favorites WHERE page_id = $1 AND user_id = $2")
        .bind(page_id)
        .bind(user.id)
        .execute(&pool)
        .await?
        .rows_affected();
    if removed == 0 {
        sqlx::query("INSERT INTO page_favorites (page_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
            .bind(page_id)
            .bind(user.id)
            .execute(&pool)
            .await?;
    }
    Ok(Json(serde_json::json!({ "is_favorite": removed == 0 })))
}

async fn record_view(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path((workspace_id, page_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    let (ws, _, _) = require_page_read(&pool, &user, page_id).await?;
    if ws != workspace_id {
        return Err(ApiError::NotFound);
    }
    sqlx::query(
        "INSERT INTO page_views (page_id, user_id, last_viewed_at, view_count) VALUES ($1, $2, CURRENT_TIMESTAMP, 1)
         ON CONFLICT (page_id, user_id)
         DO UPDATE SET last_viewed_at = CURRENT_TIMESTAMP, view_count = page_views.view_count + 1",
    )
    .bind(page_id)
    .bind(user.id)
    .execute(&pool)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize)]
struct AccessCheck {
    user_id: Uuid,
    name: String,
    can_write: bool,
}

/// Utilisé par le serveur Hocuspocus (cookie de session transmis) pour autoriser
/// la connexion à un document et décider du mode lecture seule.
async fn page_access_check(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path(page_id): Path<Uuid>,
) -> ApiResult<Json<AccessCheck>> {
    let (_, _, access) = require_page_read(&pool, &user, page_id).await?;
    let name: String = sqlx::query_scalar("SELECT name FROM users WHERE id = $1")
        .bind(user.id)
        .fetch_one(&pool)
        .await?;
    Ok(Json(AccessCheck { user_id: user.id, name, can_write: access == PageAccess::Write }))
}

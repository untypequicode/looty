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
    is_member, require_page_read, require_page_write, visible_pages, AuthUser,
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
    /// Projet porté par la page (LaTeX, site, notebook).
    #[sqlx(default)]
    pub project_id: Option<Uuid>,
    #[sqlx(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_kind: Option<String>,
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
    /// Sous-pages permises par un partage (invités, rôles sans création de documents).
    #[sqlx(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub can_add_child: Option<bool>,
    /// Permissions particulières sur la page (icône dans la barre latérale).
    #[sqlx(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_permissions: Option<bool>,
}

/// Version allégée pour les listes / l'arbre (sans le contenu HTML).
const PAGE_LIST_COLUMNS: &str = "p.id, p.workspace_id, p.parent_id, p.icon, p.title, NULL::text AS content,
    p.created_by, p.updated_by, p.position, p.created_at, p.updated_at, p.share_token, p.is_public,
    p.project_id, (SELECT kind FROM projects pr WHERE pr.id = p.project_id) AS project_kind,
    EXISTS(SELECT 1 FROM page_permissions pp WHERE pp.page_id = p.id) AS has_permissions";

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
    /// Identifiant choisi par le client (application hors ligne) : création idempotente.
    pub id: Option<Uuid>,
    pub title: Option<String>,
    pub icon: Option<String>,
    pub parent_id: Option<Uuid>,
    pub content: Option<String>,
    /// Page qui porte un nouveau projet (LaTeX, site, notebook) créé avec elle.
    pub project: Option<NewProject>,
}

#[derive(Deserialize)]
pub struct NewProject {
    pub kind: String,
    pub template: Option<String>,
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
        .route("/{page_id}", get(get_page).put(update_page).delete(delete_page))
        .route("/{page_id}/favorite", post(toggle_favorite))
        .route("/{page_id}/view", post(record_view))
        .route("/{page_id}/share", post(share_page).delete(revoke_share))
        .route("/{page_id}/permissions", get(get_page_permissions).put(set_page_permission))
        .route("/{page_id}/project", post(make_project))
}

/// Routes hors workspace : contrôle d'accès pour le serveur de collaboration.
pub fn access_routes() -> Router<AppState> {
    Router::new().route("/{page_id}/access", get(page_access_check))
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
    let role = crate::access::require_member_or_guest(&pool, &user, workspace_id).await?;
    let visible = visible_pages(&pool, &user, workspace_id, role).await?;
    let grants = crate::access::subpage_grants(&pool, user.id, workspace_id).await?;
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
    let pages: Vec<Page> = sqlx::query_as(sqlx::AssertSqlSafe(sql.as_str())).bind(workspace_id).bind(user.id).fetch_all(&pool).await?;
    Ok(Json(
        pages
            .into_iter()
            .filter_map(|mut p| match visible.get(&p.id) {
                Some(PageAccess::None) | None => None,
                Some(a) => {
                    p.can_edit = Some(*a == PageAccess::Write);
                    if grants.contains(&p.id) {
                        p.can_add_child = Some(true);
                    }
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
        "SELECT p.*, (SELECT kind FROM projects pr WHERE pr.id = p.project_id) AS project_kind,
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
    if crate::access::subpage_grants(&pool, user.id, workspace_id).await?.contains(&page_id) {
        page.can_add_child = Some(true);
    }
    Ok(Json(page))
}

async fn create_page(
    State(state): State<AppState>,
    user: AuthUser,
    Path(workspace_id): Path<Uuid>,
    Json(payload): Json<CreatePage>,
) -> ApiResult<(StatusCode, Json<Page>)> {
    let pool = &state.pool;
    // Rôle qui crée des documents, ou partage « sous-pages permises » sur la page parente.
    let role_edit = crate::access::member_perms(pool, user.id, workspace_id)
        .await?
        .is_some_and(|(_, p)| p.docs >= crate::access::level::EDIT);
    let mut via_share = false;
    match payload.parent_id {
        Some(parent) if role_edit => {
            let (ws, _) = require_page_write(pool, &user, parent).await?;
            if ws != workspace_id {
                return Err(ApiError::NotFound);
            }
        }
        Some(parent) => {
            if !crate::access::subpage_grants(pool, user.id, workspace_id).await?.contains(&parent) {
                return Err(ApiError::Forbidden);
            }
            via_share = true;
        }
        None if role_edit => {}
        None => return Err(ApiError::Forbidden),
    }
    // Rejeu d'une création faite hors ligne : la page existe déjà ?
    if let Some(id) = payload.id {
        let existing: Option<Page> = sqlx::query_as("SELECT * FROM pages WHERE id = $1").bind(id).fetch_optional(pool).await?;
        if let Some(p) = existing {
            if p.workspace_id != workspace_id {
                return Err(ApiError::Conflict("id_taken".into()));
            }
            // Le contenu n'est renvoyé qu'à qui peut lire la page.
            return match require_page_read(pool, &user, p.id).await {
                Ok(_) => Ok((StatusCode::OK, Json(p))),
                Err(_) => Err(ApiError::Conflict("id_taken".into())),
            };
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

    let mut page: Page = sqlx::query_as(
        "INSERT INTO pages (id, workspace_id, parent_id, icon, title, content, created_by, updated_by, position)
         VALUES (COALESCE($8, gen_random_uuid()), $1, $2, $3, $4, $5, $6, $6, $7) RETURNING *",
    )
    .bind(workspace_id)
    .bind(payload.parent_id)
    .bind(payload.icon)
    .bind(title)
    .bind(payload.content)
    .bind(user.id)
    .bind(position)
    .bind(payload.id)
    .fetch_one(pool)
    .await?;

    if via_share {
        // La nouvelle page reste accessible à son auteur, même si le partage parent ne couvre
        // pas les sous-pages.
        sqlx::query(
            "INSERT INTO page_permissions (page_id, user_id, role, inherit, allow_subpages, created_by) VALUES ($1, $2, 'editor', true, true, $2)
             ON CONFLICT (page_id, user_id) DO NOTHING",
        )
        .bind(page.id)
        .bind(user.id)
        .execute(pool)
        .await?;
    }
    // Page qui porte un projet : créé avec elle, au nom de la page.
    if let Some(np) = &payload.project {
        let name = if page.title.trim().is_empty() { "Sans titre".to_string() } else { page.title.clone() };
        let project = crate::projects::insert_project(&state, workspace_id, user.id, &np.kind, &name, np.template.as_deref()).await?;
        sqlx::query("UPDATE pages SET project_id = $2 WHERE id = $1").bind(page.id).bind(project.id).execute(pool).await?;
        page.project_id = Some(project.id);
        page.project_kind = Some(project.kind);
    }
    crate::activity::log_page(pool, page.id, workspace_id, user.id, "create").await;
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

    // Remonter une page à la racine : réservé aux rôles qui créent des documents (un invité ou
    // un partage « sous-pages permises » sortirait sinon la page des droits hérités du parent).
    if payload.parent_id == Some(None) {
        let current_parent: Option<Uuid> = sqlx::query_scalar("SELECT parent_id FROM pages WHERE id = $1").bind(page_id).fetch_one(pool).await?;
        let role_edit = crate::access::member_perms(pool, user.id, workspace_id)
            .await?
            .is_some_and(|(_, p)| p.docs >= crate::access::level::EDIT);
        if current_parent.is_some() && !role_edit {
            return Err(ApiError::Forbidden);
        }
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

    let before: Option<(String, Option<Uuid>, Option<String>)> = sqlx::query_as("SELECT title, parent_id, content FROM pages WHERE id = $1").bind(page_id).fetch_optional(pool).await?;
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
    .bind(&title)
    .bind(payload.icon.is_some())
    .bind(payload.icon.clone().flatten())
    .bind(payload.parent_id.is_some())
    .bind(payload.parent_id.flatten())
    .bind(&payload.content)
    .bind(payload.position)
    .bind(user.id)
    .bind(page_id)
    .fetch_one(pool)
    .await?;
    if let Some((old_title, old_parent, old_content)) = before {
        if old_title != page.title {
            crate::activity::log_page(pool, page_id, workspace_id, user.id, "rename").await;
        }
        if old_parent != page.parent_id {
            crate::activity::log_page(pool, page_id, workspace_id, user.id, "move").await;
        }
        // L'éditeur réenregistre parfois un HTML identique à l'ouverture : pas une modification.
        if payload.content.is_some() && payload.content != old_content {
            crate::activity::log_page(pool, page_id, workspace_id, user.id, "edit").await;
        }
    }

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
    crate::trash::trash_page(&state, workspace_id, page_id, Some(user.id)).await?;
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
    crate::access::require_perm(&pool, &user, workspace_id, |p| p.share).await?;
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
    /// Préambule LaTeX de l'espace, pour un rendu identique des formules.
    pub latex_preamble: String,
}

pub async fn get_public_page(State(pool): State<PgPool>, Path(token): Path<Uuid>) -> ApiResult<Json<PublicPage>> {
    Ok(Json(
        sqlx::query_as(
            "SELECT p.id, p.title, p.icon, p.content, p.updated_at, w.latex_preamble
             FROM pages p JOIN workspaces w ON w.id = p.workspace_id
             WHERE p.share_token = $1 AND p.is_public = true",
        )
        .bind(token)
        .fetch_optional(&pool)
        .await?
        .ok_or(ApiError::NotFound)?,
    ))
}

#[derive(Deserialize)]
pub struct SetPagePermissionReq {
    pub user_id: Uuid,
    pub role: String,
    /// Sous-pages comprises (par défaut) ou page seule.
    pub inherit: Option<bool>,
    /// La personne peut créer des sous-pages (avec le rôle `editor`).
    pub allow_subpages: Option<bool>,
}

async fn get_page_permissions(
    State(pool): State<PgPool>,
    user: AuthUser,
    Path((workspace_id, page_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<Vec<serde_json::Value>>> {
    let (ws, _, _) = require_page_read(&pool, &user, page_id).await?;
    if ws != workspace_id {
        return Err(ApiError::NotFound);
    }
    crate::access::require_perm(&pool, &user, workspace_id, |p| p.admin).await?;
    // Permissions de la page et celles héritées d'un ancêtre (« sous-pages comprises »).
    let rows: Vec<(Uuid, String, String, Option<String>, String, bool, Uuid, String, i32, bool, bool)> = sqlx::query_as(
        "WITH RECURSIVE anc AS (
            SELECT id, parent_id, title, 0 AS depth FROM pages WHERE id = $1
            UNION ALL SELECT p.id, p.parent_id, p.title, anc.depth + 1 FROM pages p JOIN anc ON p.id = anc.parent_id WHERE anc.depth < 64
         )
         SELECT pp.user_id, u.name, u.email, u.avatar_url, pp.role, pp.inherit, anc.id, anc.title, anc.depth, pp.allow_subpages,
                EXISTS(SELECT 1 FROM workspace_members m WHERE m.workspace_id = $2 AND m.user_id = pp.user_id::varchar) AS is_member
         FROM anc JOIN page_permissions pp ON pp.page_id = anc.id JOIN users u ON u.id = pp.user_id
         WHERE anc.depth = 0 OR pp.inherit
         ORDER BY anc.depth, u.name",
    )
    .bind(page_id)
    .bind(workspace_id)
    .fetch_all(&pool)
    .await?;
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for (uid, name, email, avatar, role, inherit, from, from_title, depth, allow_subpages, is_member) in rows {
        // La permission la plus proche s'applique ; les plus lointaines sont masquées.
        if !seen.insert(uid) {
            continue;
        }
        out.push(serde_json::json!({
            "user_id": uid, "name": name, "email": email, "avatar_url": avatar, "role": role, "inherit": inherit, "allow_subpages": allow_subpages,
            "is_member": is_member, "inherited_from": if depth > 0 { Some(serde_json::json!({ "id": from, "title": from_title })) } else { None },
        }));
    }
    Ok(Json(out))
}

async fn set_page_permission(
    State(state): State<AppState>,
    user: AuthUser,
    Path((workspace_id, page_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<SetPagePermissionReq>,
) -> ApiResult<StatusCode> {
    let pool = &state.pool;
    ensure_in_workspace(pool, page_id, workspace_id).await?;
    crate::access::require_perm(pool, &user, workspace_id, |p| p.admin).await?;
    let member = is_member(pool, workspace_id, &payload.user_id.to_string()).await?;
    let inherit = payload.inherit.unwrap_or(true);
    let allow_subpages = payload.role == "editor" && payload.allow_subpages.unwrap_or(false);
    match payload.role.as_str() {
        "none" => {
            sqlx::query("DELETE FROM page_permissions WHERE page_id = $1 AND user_id = $2")
                .bind(page_id)
                .bind(payload.user_id)
                .execute(pool)
                .await?;
        }
        // Personne extérieure à l'espace : lecture ou écriture seulement (invité).
        "viewer" | "editor" | "blocked" => {
            if !member && payload.role == "blocked" {
                return Err(ApiError::bad_request("not_a_member"));
            }
            let exists: Option<Uuid> = sqlx::query_scalar("SELECT id FROM users WHERE id = $1").bind(payload.user_id).fetch_optional(pool).await?;
            exists.ok_or(ApiError::NotFound)?;
            let created: bool = sqlx::query_scalar(
                "INSERT INTO page_permissions (page_id, user_id, role, inherit, created_by, allow_subpages) VALUES ($1, $2, $3, $4, $5, $6)
                 ON CONFLICT (page_id, user_id) DO UPDATE SET role = $3, inherit = $4, allow_subpages = $6
                 RETURNING (xmax = 0)",
            )
            .bind(page_id)
            .bind(payload.user_id)
            .bind(&payload.role)
            .bind(inherit)
            .bind(user.id)
            .bind(allow_subpages)
            .fetch_one(pool)
            .await?;
            if created && !member {
                // Invité : prévenu par e-mail (page visible dans « Partagé avec moi »).
                let (title,): (String,) = sqlx::query_as("SELECT title FROM pages WHERE id = $1").bind(page_id).fetch_one(pool).await?;
                let (to, to_name): (String, String) = sqlx::query_as("SELECT email, name FROM users WHERE id = $1").bind(payload.user_id).fetch_one(pool).await?;
                let from: String = sqlx::query_scalar("SELECT name FROM users WHERE id = $1").bind(user.id).fetch_one(pool).await?;
                let link = format!("{}/shared/{page_id}", state.base_url);
                crate::roles::send_mail(&state, &to, &to_name, &format!("{from} a partagé « {title} » avec vous"),
                    &format!("{from} a partagé le document « {title} » avec vous sur Looty.\nOuvrir : {link}\n\n{from} shared “{title}” with you: {link}")).await;
            }
        }
        _ => return Err(ApiError::bad_request("invalid_role")),
    }
    state.emit(workspace_id, serde_json::json!({ "type": "PAGE_PERMISSIONS_UPDATED", "page_id": page_id })).await;
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
    crate::analytics::track(&pool, workspace_id, user.id, "view", Some(page_id)).await;
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

/// La page devient un projet (LaTeX, site, notebook) : son texte reste dans l'onglet Notes.
async fn make_project(
    State(state): State<AppState>,
    user: AuthUser,
    Path((workspace_id, page_id)): Path<(Uuid, Uuid)>,
    Json(np): Json<NewProject>,
) -> ApiResult<Json<Page>> {
    let pool = &state.pool;
    let (ws, _) = require_page_write(pool, &user, page_id).await?;
    if ws != workspace_id {
        return Err(ApiError::NotFound);
    }
    let (title, current): (String, Option<Uuid>) = sqlx::query_as("SELECT title, project_id FROM pages WHERE id = $1").bind(page_id).fetch_one(pool).await?;
    if current.is_some() {
        return Err(ApiError::Conflict("page_has_project".into()));
    }
    let name = if title.trim().is_empty() { "Sans titre".to_string() } else { title };
    let project = crate::projects::insert_project(&state, workspace_id, user.id, &np.kind, &name, np.template.as_deref()).await?;
    let mut page: Page = sqlx::query_as("UPDATE pages SET project_id = $2, updated_at = NOW() WHERE id = $1 RETURNING *")
        .bind(page_id)
        .bind(project.id)
        .fetch_one(pool)
        .await?;
    page.project_kind = Some(project.kind);
    let mut lite = page.clone();
    lite.content = None;
    state.emit(workspace_id, serde_json::json!({ "type": "PAGE_UPDATED", "page": lite })).await;
    Ok(Json(page))
}

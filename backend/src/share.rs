//! Partage de pages (ADR 0022, phases 3 et 4) : groupes de comptes de l'instance comme cibles,
//! qui a accès à une page et d'où vient chaque droit, demandes d'accès, « Partagé avec moi »,
//! personnes mentionnables, annuaire.
//!
//! Les groupes ne touchent pas au résolveur d'accès : un partage à un groupe est « déplié » en
//! lignes de `page_permissions` (une par membre, `via_group` renseigné) par `sync_group_grants`.
//! Un partage direct à une personne prime toujours sur celui d'un groupe.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post, put},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::access::{level, nearest_grant, resolve_level, AuthUser, Grant};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Routes
// ---------------------------------------------------------------------------

/// `/api/admin/share` : groupes d'instance et annuaire (administrateurs de l'instance).
pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/groups", get(admin_list_groups).post(admin_create_group))
        .route("/groups/{id}", put(admin_rename_group).delete(admin_delete_group))
        .route("/groups/{id}/members", get(admin_group_members).post(admin_add_member))
        .route("/groups/{id}/members/{user_id}", axum::routing::delete(admin_remove_member))
        .route("/directory", get(directory).put(admin_set_directory))
}

/// `/api/share` : ce dont le dialogue de partage et la page « demander l'accès » ont besoin.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/groups", get(search_groups))
        .route("/directory", get(directory))
        .route("/access-status/{page_id}", get(access_status))
        .route("/request-access", post(request_access))
}

// ---------------------------------------------------------------------------
// Annuaire
// ---------------------------------------------------------------------------

/// Portée de la recherche de personnes : `all`, `shared` ou `email` (voir la migration).
pub async fn directory_scope(pool: &PgPool) -> String {
    sqlx::query_scalar("SELECT scope FROM instance_directory WHERE id = 1").fetch_optional(pool).await.ok().flatten().unwrap_or_else(|| "all".into())
}

async fn directory(State(pool): State<PgPool>, _user: AuthUser) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({ "scope": directory_scope(&pool).await })))
}

#[derive(Deserialize)]
struct DirectoryReq {
    scope: String,
}

async fn admin_set_directory(State(pool): State<PgPool>, user: AuthUser, Json(p): Json<DirectoryReq>) -> ApiResult<Json<serde_json::Value>> {
    crate::admin::require_super_admin(&user)?;
    if !matches!(p.scope.as_str(), "all" | "shared" | "email") {
        return Err(ApiError::bad_request("invalid_scope"));
    }
    sqlx::query("UPDATE instance_directory SET scope = $1 WHERE id = 1").bind(&p.scope).execute(&pool).await?;
    Ok(Json(serde_json::json!({ "scope": p.scope })))
}

// ---------------------------------------------------------------------------
// Groupes d'instance
// ---------------------------------------------------------------------------

#[derive(Serialize, sqlx::FromRow)]
struct GroupDto {
    id: Uuid,
    name: String,
    member_count: i64,
}

const GROUP_SELECT: &str = "SELECT g.id, g.name, (SELECT count(*) FROM instance_group_members m WHERE m.group_id = g.id) AS member_count FROM instance_groups g";

async fn admin_list_groups(State(pool): State<PgPool>, user: AuthUser) -> ApiResult<Json<Vec<GroupDto>>> {
    crate::admin::require_super_admin(&user)?;
    Ok(Json(sqlx::query_as(sqlx::AssertSqlSafe(format!("{GROUP_SELECT} ORDER BY lower(g.name)"))).fetch_all(&pool).await?))
}

#[derive(Deserialize)]
struct GroupReq {
    name: String,
}

fn check_group_name(n: &str) -> ApiResult<String> {
    let n = n.trim();
    if n.is_empty() || n.chars().count() > 80 {
        return Err(ApiError::bad_request("invalid_name"));
    }
    Ok(n.to_string())
}

async fn admin_create_group(State(pool): State<PgPool>, user: AuthUser, Json(p): Json<GroupReq>) -> ApiResult<Json<GroupDto>> {
    crate::admin::require_super_admin(&user)?;
    let name = check_group_name(&p.name)?;
    let taken: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM instance_groups WHERE lower(name) = lower($1))").bind(&name).fetch_one(&pool).await?;
    if taken {
        return Err(ApiError::Conflict("group_name_taken".into()));
    }
    let id: Uuid = sqlx::query_scalar("INSERT INTO instance_groups (name, created_by) VALUES ($1, $2) RETURNING id").bind(&name).bind(user.id).fetch_one(&pool).await?;
    Ok(Json(GroupDto { id, name, member_count: 0 }))
}

async fn admin_rename_group(State(pool): State<PgPool>, user: AuthUser, Path(id): Path<Uuid>, Json(p): Json<GroupReq>) -> ApiResult<StatusCode> {
    crate::admin::require_super_admin(&user)?;
    let name = check_group_name(&p.name)?;
    let taken: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM instance_groups WHERE lower(name) = lower($1) AND id <> $2)").bind(&name).bind(id).fetch_one(&pool).await?;
    if taken {
        return Err(ApiError::Conflict("group_name_taken".into()));
    }
    let n = sqlx::query("UPDATE instance_groups SET name = $1 WHERE id = $2").bind(&name).bind(id).execute(&pool).await?.rows_affected();
    if n == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Pages et espaces touchés par les partages d'un groupe : pour prévenir le temps réel après un changement.
async fn group_pages(pool: &PgPool, group_id: Uuid) -> ApiResult<Vec<(Uuid, Uuid)>> {
    Ok(sqlx::query_as("SELECT gg.page_id, p.workspace_id FROM page_group_grants gg JOIN pages p ON p.id = gg.page_id WHERE gg.group_id = $1").bind(group_id).fetch_all(pool).await?)
}

async fn resync_group(state: &AppState, pages: &[(Uuid, Uuid)]) -> ApiResult<()> {
    for (page, ws) in pages {
        sync_group_grants(&state.pool, *page).await?;
        state.emit(*ws, serde_json::json!({ "type": "PAGE_PERMISSIONS_UPDATED", "page_id": page })).await;
    }
    Ok(())
}

async fn admin_delete_group(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<StatusCode> {
    crate::admin::require_super_admin(&user)?;
    let pages = group_pages(&state.pool, id).await?;
    // Les lignes dépliées (`via_group`) et les partages partent avec le groupe (clés étrangères en cascade).
    let n = sqlx::query("DELETE FROM instance_groups WHERE id = $1").bind(id).execute(&state.pool).await?.rows_affected();
    if n == 0 {
        return Err(ApiError::NotFound);
    }
    resync_group(&state, &pages).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize, sqlx::FromRow)]
struct MemberDto {
    id: Uuid,
    name: String,
    email: String,
    avatar_url: Option<String>,
}

async fn admin_group_members(State(pool): State<PgPool>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<Json<Vec<MemberDto>>> {
    crate::admin::require_super_admin(&user)?;
    Ok(Json(
        sqlx::query_as("SELECT u.id, u.name, u.email, u.avatar_url FROM instance_group_members m JOIN users u ON u.id = m.user_id WHERE m.group_id = $1 ORDER BY lower(u.name)")
            .bind(id)
            .fetch_all(&pool)
            .await?,
    ))
}

#[derive(Deserialize)]
struct AddMemberReq {
    user_id: Uuid,
}

async fn admin_add_member(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(p): Json<AddMemberReq>) -> ApiResult<StatusCode> {
    crate::admin::require_super_admin(&user)?;
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM instance_groups WHERE id = $1) AND EXISTS(SELECT 1 FROM users WHERE id = $2)").bind(id).bind(p.user_id).fetch_one(&state.pool).await?;
    if !exists {
        return Err(ApiError::NotFound);
    }
    sqlx::query("INSERT INTO instance_group_members (group_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING").bind(id).bind(p.user_id).execute(&state.pool).await?;
    let pages = group_pages(&state.pool, id).await?;
    resync_group(&state, &pages).await?;
    // La personne qui rejoint un groupe est prévenue pour chaque page qu'il lui ouvre.
    for (page, ws) in &pages {
        notify_shared(&state, *ws, *page, p.user_id, user.id, None).await;
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn admin_remove_member(State(state): State<AppState>, user: AuthUser, Path((id, uid)): Path<(Uuid, Uuid)>) -> ApiResult<StatusCode> {
    crate::admin::require_super_admin(&user)?;
    sqlx::query("DELETE FROM instance_group_members WHERE group_id = $1 AND user_id = $2").bind(id).bind(uid).execute(&state.pool).await?;
    let pages = group_pages(&state.pool, id).await?;
    resync_group(&state, &pages).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct GroupSearch {
    #[serde(default)]
    q: String,
}

/// Groupes que la personne peut viser en partageant : tous si l'annuaire est ouvert, sinon les siens.
async fn search_groups(State(pool): State<PgPool>, user: AuthUser, Query(q): Query<GroupSearch>) -> ApiResult<Json<Vec<GroupDto>>> {
    let term = q.q.trim().to_lowercase();
    let like = format!("%{}%", term.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
    let open = user.is_super_admin || directory_scope(&pool).await == "all";
    let rows: Vec<GroupDto> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "{GROUP_SELECT} WHERE lower(g.name) LIKE $1 AND ($2 OR EXISTS (SELECT 1 FROM instance_group_members m WHERE m.group_id = g.id AND m.user_id = $3))
         ORDER BY lower(g.name) LIMIT 8"
    )))
    .bind(like)
    .bind(open)
    .bind(user.id)
    .fetch_all(&pool)
    .await?;
    Ok(Json(rows))
}

/// Recalcule les lignes `page_permissions` venues des groupes pour une page : une par membre,
/// le refus l'emporte sur un niveau, sinon le niveau le plus haut ; un partage direct est conservé.
pub async fn sync_group_grants(pool: &PgPool, page_id: Uuid) -> ApiResult<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM page_permissions WHERE page_id = $1 AND via_group IS NOT NULL").bind(page_id).execute(&mut *tx).await?;
    let rows: Vec<(Uuid, String, bool, bool, Uuid, Option<Uuid>)> = sqlx::query_as(
        "SELECT gm.user_id, gg.role, gg.inherit, gg.allow_subpages, gg.group_id, gg.created_by
         FROM page_group_grants gg JOIN instance_groups ig ON ig.id = gg.group_id JOIN instance_group_members gm ON gm.group_id = gg.group_id
         WHERE gg.page_id = $1 ORDER BY lower(ig.name), gm.user_id",
    )
    .bind(page_id)
    .fetch_all(&mut *tx)
    .await?;
    let rank = |role: &str| match Grant::parse(role) {
        Some(Grant::Level(l)) => l as i32,
        _ => 100,
    };
    let mut best: HashMap<Uuid, (String, bool, bool, Uuid, Option<Uuid>)> = HashMap::new();
    for (uid, role, inherit, sub, gid, by) in rows {
        match best.get(&uid) {
            Some(cur) if rank(&cur.0) >= rank(&role) => {}
            _ => {
                best.insert(uid, (role, inherit, sub, gid, by));
            }
        }
    }
    for (uid, (role, inherit, sub, gid, by)) in best {
        sqlx::query(
            "INSERT INTO page_permissions (page_id, user_id, role, inherit, allow_subpages, created_by, via_group) VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (page_id, user_id) DO NOTHING",
        )
        .bind(page_id)
        .bind(uid)
        .bind(role)
        .bind(inherit)
        .bind(sub)
        .bind(by)
        .bind(gid)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Partage : une personne, un groupe
// ---------------------------------------------------------------------------

/// Notification dans l'application (et push / e-mail selon les préférences) : une page est partagée.
pub async fn notify_shared(state: &AppState, ws: Uuid, page_id: Uuid, to: Uuid, by: Uuid, lv: Option<&str>) {
    let data = serde_json::json!({ "level": lv });
    crate::notifications::notify(&state.pool, &state.hub, ws, to, Some(by), "share", &format!("page:{page_id}"), data).await;
}

/// Donne (ou change) le partage direct d'une personne. Enregistre l'activité, prévient la personne
/// dans l'application, et par e-mail la première fois si elle est extérieure à l'espace.
#[allow(clippy::too_many_arguments)]
pub async fn grant_user(state: &AppState, actor: Uuid, ws: Uuid, page_id: Uuid, target: Uuid, stored: &str, inherit: bool, allow_subpages: bool) -> ApiResult<()> {
    let pool = &state.pool;
    let exists: Option<Uuid> = sqlx::query_scalar("SELECT id FROM users WHERE id = $1").bind(target).fetch_optional(pool).await?;
    exists.ok_or(ApiError::NotFound)?;
    let member = crate::access::is_member(pool, ws, &target.to_string()).await?;
    let before: Option<(String, Option<Uuid>)> = sqlx::query_as("SELECT role, via_group FROM page_permissions WHERE page_id = $1 AND user_id = $2").bind(page_id).bind(target).fetch_optional(pool).await?;
    sqlx::query(
        "INSERT INTO page_permissions (page_id, user_id, role, inherit, created_by, allow_subpages, via_group) VALUES ($1, $2, $3, $4, $5, $6, NULL)
         ON CONFLICT (page_id, user_id) DO UPDATE SET role = $3, inherit = $4, allow_subpages = $6, via_group = NULL",
    )
    .bind(page_id)
    .bind(target)
    .bind(stored)
    .bind(inherit)
    .bind(actor)
    .bind(allow_subpages)
    .execute(pool)
    .await?;
    crate::activity::log_page(pool, page_id, ws, actor, "share").await;
    let changed = before.as_ref().is_none_or(|(r, g)| r != stored || g.is_some());
    if changed && stored != "deny" {
        notify_shared(state, ws, page_id, target, actor, Some(stored)).await;
    }
    if before.is_none() && !member && stored != "deny" {
        // Personne extérieure : prévenue aussi par e-mail (page visible dans « Partagé avec moi »).
        let (title,): (String,) = sqlx::query_as("SELECT title FROM pages WHERE id = $1").bind(page_id).fetch_one(pool).await?;
        let (to, to_name): (String, String) = sqlx::query_as("SELECT email, name FROM users WHERE id = $1").bind(target).fetch_one(pool).await?;
        let from: String = sqlx::query_scalar("SELECT name FROM users WHERE id = $1").bind(actor).fetch_one(pool).await?;
        let link = format!("{}/shared/{page_id}", state.base_url);
        crate::roles::send_mail(
            state,
            &to,
            &to_name,
            &format!("{from} a partagé « {title} » avec vous"),
            &format!("{from} a partagé le document « {title} » avec vous sur Looty.\nOuvrir : {link}\n\n{from} shared “{title}” with you: {link}"),
        )
        .await;
    }
    Ok(())
}

/// Retire le partage direct d'une personne (celui d'un groupe, s'il y en a un, reprend).
pub async fn revoke_user(state: &AppState, actor: Uuid, ws: Uuid, page_id: Uuid, target: Uuid) -> ApiResult<()> {
    let removed = sqlx::query("DELETE FROM page_permissions WHERE page_id = $1 AND user_id = $2 AND via_group IS NULL").bind(page_id).bind(target).execute(&state.pool).await?.rows_affected();
    if removed > 0 {
        crate::activity::log_page(&state.pool, page_id, ws, actor, "unshare").await;
        sync_group_grants(&state.pool, page_id).await?;
    }
    Ok(())
}

/// Donne (ou retire, `stored` = `none`) le partage d'un groupe d'instance sur la page.
#[allow(clippy::too_many_arguments)]
pub async fn grant_group(state: &AppState, actor: Uuid, ws: Uuid, page_id: Uuid, group: Uuid, stored: &str, inherit: bool, allow_subpages: bool) -> ApiResult<()> {
    let pool = &state.pool;
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM instance_groups WHERE id = $1)").bind(group).fetch_one(pool).await?;
    if !exists {
        return Err(ApiError::NotFound);
    }
    if stored == "none" {
        let removed = sqlx::query("DELETE FROM page_group_grants WHERE page_id = $1 AND group_id = $2").bind(page_id).bind(group).execute(pool).await?.rows_affected();
        sync_group_grants(pool, page_id).await?;
        if removed > 0 {
            crate::activity::log_page(pool, page_id, ws, actor, "unshare").await;
        }
        return Ok(());
    }
    let before: Option<String> = sqlx::query_scalar("SELECT role FROM page_group_grants WHERE page_id = $1 AND group_id = $2").bind(page_id).bind(group).fetch_optional(pool).await?;
    sqlx::query(
        "INSERT INTO page_group_grants (page_id, group_id, role, inherit, allow_subpages, created_by) VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (page_id, group_id) DO UPDATE SET role = $3, inherit = $4, allow_subpages = $5",
    )
    .bind(page_id)
    .bind(group)
    .bind(stored)
    .bind(inherit)
    .bind(allow_subpages)
    .bind(actor)
    .execute(pool)
    .await?;
    sync_group_grants(pool, page_id).await?;
    crate::activity::log_page(pool, page_id, ws, actor, "share").await;
    if before.as_deref() != Some(stored) && stored != "deny" {
        let members: Vec<Uuid> = sqlx::query_scalar("SELECT user_id FROM instance_group_members WHERE group_id = $1 LIMIT 500").bind(group).fetch_all(pool).await?;
        for m in members {
            notify_shared(state, ws, page_id, m, actor, Some(stored)).await;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Qui a accès à une page
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Person {
    pub user_id: Uuid,
    pub name: String,
    pub email: String,
    pub avatar_url: Option<String>,
    pub level: u8,
    pub deny: bool,
    pub is_member: bool,
    /// `direct`, `inherited` (d'une page parente), `group` ou `space` (rôle dans l'espace).
    pub origin: &'static str,
    /// Page qui porte le partage (elle-même ou un ancêtre).
    pub from_page: Option<Uuid>,
    pub inherit: bool,
    pub allow_subpages: bool,
    pub via_group: Option<(Uuid, String)>,
}

/// Page, puis ses ancêtres du plus proche au plus lointain.
fn walk(start: Uuid, parents: &HashMap<Uuid, Option<Uuid>>) -> Vec<Uuid> {
    let mut out = vec![start];
    let mut cur = parents.get(&start).copied().flatten();
    while let Some(c) = cur {
        if out.len() > 64 || out.contains(&c) {
            break;
        }
        out.push(c);
        cur = parents.get(&c).copied().flatten();
    }
    out
}

/// Page qui porte le partage le plus proche (la même règle que `nearest_grant`).
fn grant_page(start: Uuid, parents: &HashMap<Uuid, Option<Uuid>>, grants: &HashMap<Uuid, (String, bool)>) -> Option<Uuid> {
    walk(start, parents).into_iter().find(|c| grants.get(c).is_some_and(|(_, inherit)| *c == start || *inherit))
}

/// Toutes les personnes qui ont (ou se sont vu retirer) l'accès à la page, avec l'origine de leur
/// droit : en une poignée de requêtes, quel que soit le nombre de personnes. Même règle que le
/// résolveur (`resolve_level` et `nearest_grant`). Renvoie aussi les titres de la page et de ses ancêtres.
pub async fn page_audience(pool: &PgPool, page_id: Uuid) -> ApiResult<(Vec<Person>, HashMap<Uuid, String>)> {
    let ws = crate::access::page_workspace(pool, page_id).await?;
    let owner: Option<Uuid> = sqlx::query_scalar("SELECT private_to FROM pages_all WHERE id = $1").bind(page_id).fetch_one(pool).await?;
    let chain = crate::access::ancestors(pool, page_id).await?;
    let ids: Vec<Uuid> = chain.iter().map(|(id, _)| *id).collect();
    let parents: HashMap<Uuid, Option<Uuid>> = chain.into_iter().collect();
    let titles: HashMap<Uuid, String> = sqlx::query_as::<_, (Uuid, String)>("SELECT id, title FROM pages_all WHERE id = ANY($1)").bind(&ids).fetch_all(pool).await?.into_iter().collect();
    let members = crate::access::workspace_perms(pool, ws).await?;
    let rows: Vec<(Uuid, Uuid, String, bool, bool, Option<Uuid>, Option<String>)> = sqlx::query_as(
        "SELECT pp.user_id, pp.page_id, pp.role, pp.inherit, pp.allow_subpages, pp.via_group, g.name
         FROM page_permissions pp LEFT JOIN instance_groups g ON g.id = pp.via_group WHERE pp.page_id = ANY($1)",
    )
    .bind(&ids)
    .fetch_all(pool)
    .await?;
    let mut grants: HashMap<Uuid, HashMap<Uuid, (String, bool)>> = HashMap::new();
    let mut extras: HashMap<(Uuid, Uuid), (bool, Option<(Uuid, String)>)> = HashMap::new();
    for (uid, pid, role, inherit, sub, gid, gname) in rows {
        grants.entry(uid).or_default().insert(pid, (role, inherit));
        extras.insert((uid, pid), (sub, gid.map(|g| (g, gname.unwrap_or_default()))));
    }
    let mut wanted: HashSet<Uuid> = members.keys().copied().chain(grants.keys().copied()).collect();
    if let Some(o) = owner {
        wanted.retain(|u| *u == o);
    }
    let wanted: Vec<Uuid> = wanted.into_iter().collect();
    let infos: Vec<(Uuid, String, String, Option<String>)> =
        sqlx::query_as("SELECT id, name, email, avatar_url FROM users WHERE id = ANY($1) AND COALESCE(is_suspended, FALSE) = FALSE").bind(&wanted).fetch_all(pool).await?;
    let empty = HashMap::new();
    let mut out = Vec::new();
    for (uid, name, email, avatar_url) in infos {
        let member = members.get(&uid);
        if owner.is_some() {
            if member.is_some() {
                out.push(Person { user_id: uid, name, email, avatar_url, level: level::MANAGE, deny: false, is_member: true, origin: "space", from_page: None, inherit: true, allow_subpages: false, via_group: None });
            }
            continue;
        }
        let ug = grants.get(&uid).unwrap_or(&empty);
        let grant = nearest_grant(page_id, &parents, ug);
        let lv = resolve_level(member.map(|(_, p)| p), grant);
        let deny = lv == level::NONE && grant == Some(Grant::Deny);
        if lv == level::NONE && !deny {
            continue;
        }
        let from = if member.is_some_and(|(_, p)| p.admin) { None } else { grant_page(page_id, &parents, ug) };
        let (sub, via_group) = from.and_then(|f| extras.get(&(uid, f)).cloned()).unwrap_or((false, None));
        let origin = match from {
            None => "space",
            Some(_) if via_group.is_some() => "group",
            Some(f) if f == page_id => "direct",
            Some(_) => "inherited",
        };
        let inherit = from.and_then(|f| ug.get(&f)).map(|(_, i)| *i).unwrap_or(true);
        out.push(Person { user_id: uid, name, email, avatar_url, level: lv, deny, is_member: member.is_some(), origin, from_page: from, inherit, allow_subpages: sub, via_group });
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.user_id.cmp(&b.user_id)));
    Ok((out, titles))
}

/// Personnes qui gèrent la page (administrateurs de l'espace compris) : elles reçoivent les demandes d'accès.
pub async fn page_managers(pool: &PgPool, page_id: Uuid) -> ApiResult<Vec<Uuid>> {
    Ok(page_audience(pool, page_id).await?.0.into_iter().filter(|p| p.level >= level::MANAGE).map(|p| p.user_id).collect())
}

/// Titre d'un ancêtre donné seulement si la personne qui regarde peut le voir (aucune fuite de titre).
fn visible_title(viewer: &HashMap<Uuid, u8>, titles: &HashMap<Uuid, String>, page: Uuid) -> serde_json::Value {
    if viewer.get(&page).is_some_and(|l| *l > level::NONE) {
        serde_json::json!({ "id": page, "title": titles.get(&page).cloned().unwrap_or_default() })
    } else {
        serde_json::json!({ "id": null, "title": "" })
    }
}

/// `GET /workspaces/{ws}/pages/{id}/access-summary` : le contenu du dialogue de partage.
pub async fn access_summary(State(pool): State<PgPool>, user: AuthUser, Path((workspace_id, page_id)): Path<(Uuid, Uuid)>) -> ApiResult<Json<serde_json::Value>> {
    let (ws, _) = crate::access::require_page_manage(&pool, &user, page_id).await?;
    if ws != workspace_id {
        return Err(ApiError::NotFound);
    }
    let (people, titles) = page_audience(&pool, page_id).await?;
    let viewer = crate::access::visible_levels(&pool, user.id, ws).await?;
    let (outside, with_access, space) = (
        people.iter().filter(|p| p.level > level::NONE && !p.is_member).count(),
        people.iter().filter(|p| p.level > level::NONE).count(),
        people.iter().filter(|p| p.level > level::NONE && p.origin == "space").count(),
    );
    let people_json: Vec<serde_json::Value> = people
        .iter()
        .map(|p| {
            serde_json::json!({
                "user_id": p.user_id, "name": p.name, "email": p.email, "avatar_url": p.avatar_url,
                "level": if p.deny { "deny" } else { level::name(p.level) }, "origin": p.origin, "is_member": p.is_member,
                "inherit": p.inherit, "allow_subpages": p.allow_subpages,
                "from": p.from_page.filter(|f| *f != page_id).map(|f| visible_title(&viewer, &titles, f)),
                "via_group": p.via_group.as_ref().map(|(id, name)| serde_json::json!({ "id": id, "name": name })),
            })
        })
        .collect();
    // Partages de groupes : le plus proche pour chaque groupe.
    let chain = crate::access::ancestors(&pool, page_id).await?;
    let parents: HashMap<Uuid, Option<Uuid>> = chain.into_iter().collect();
    let order = walk(page_id, &parents);
    let rows: Vec<(Uuid, Uuid, String, String, bool, bool, i64)> = sqlx::query_as(
        "SELECT gg.page_id, gg.group_id, ig.name, gg.role, gg.inherit, gg.allow_subpages, (SELECT count(*) FROM instance_group_members m WHERE m.group_id = gg.group_id)
         FROM page_group_grants gg JOIN instance_groups ig ON ig.id = gg.group_id WHERE gg.page_id = ANY($1)",
    )
    .bind(&order)
    .fetch_all(&pool)
    .await?;
    let mut groups: Vec<serde_json::Value> = Vec::new();
    let mut seen = HashSet::new();
    for pid in &order {
        for (rp, gid, name, role, inherit, sub, count) in rows.iter().filter(|r| r.0 == *pid) {
            if (*rp != page_id && !*inherit) || !seen.insert(*gid) {
                continue;
            }
            let grant = Grant::parse(role).unwrap_or(Grant::Deny);
            groups.push(serde_json::json!({
                "group_id": gid, "name": name, "member_count": count, "level": grant.name(), "inherit": inherit, "allow_subpages": sub,
                "from": if *rp == page_id { None } else { Some(visible_title(&viewer, &titles, *rp)) },
            }));
        }
    }
    let pending: i64 = sqlx::query_scalar("SELECT count(*) FROM page_access_requests WHERE page_id = $1 AND status = 'pending'").bind(page_id).fetch_one(&pool).await?;
    Ok(Json(serde_json::json!({
        "people": people_json,
        "groups": groups,
        "counts": { "total": with_access, "outside": outside, "space": space },
        "pending_requests": pending,
        "directory": directory_scope(&pool).await,
    })))
}

// ---------------------------------------------------------------------------
// Mentions
// ---------------------------------------------------------------------------

/// `GET /workspaces/{ws}/pages/{id}/mentionable` : les personnes qui peuvent lire la page, invités compris.
pub async fn mentionable(State(pool): State<PgPool>, user: AuthUser, Path((workspace_id, page_id)): Path<(Uuid, Uuid)>) -> ApiResult<Json<Vec<serde_json::Value>>> {
    let (ws, _, _) = crate::access::require_page_read(&pool, &user, page_id).await?;
    if ws != workspace_id {
        return Err(ApiError::NotFound);
    }
    let (people, _) = page_audience(&pool, page_id).await?;
    Ok(Json(
        people
            .into_iter()
            .filter(|p| p.level >= level::READ)
            .map(|p| serde_json::json!({ "user_id": p.user_id, "name": p.name, "email": p.email, "avatar_url": p.avatar_url, "is_member": p.is_member }))
            .collect(),
    ))
}

/// Parmi `ids`, les personnes extérieures à l'espace qui peuvent lire la page : on peut les mentionner.
pub async fn mentioned_outsiders(pool: &PgPool, page_id: Uuid, ids: &[Uuid]) -> Vec<Uuid> {
    if ids.is_empty() {
        return Vec::new();
    }
    let Ok((people, _)) = page_audience(pool, page_id).await else { return Vec::new() };
    ids.iter().copied().filter(|id| people.iter().any(|p| p.user_id == *id && !p.is_member && p.level >= level::READ)).collect()
}

// ---------------------------------------------------------------------------
// Demandes d'accès
// ---------------------------------------------------------------------------

/// `GET /share/access-status/{page}` : où en est la personne face à ce lien ? Une page inconnue,
/// personnelle ou sans demande répond comme une page sans demande (aucune fuite d'existence).
async fn access_status(State(pool): State<PgPool>, user: AuthUser, Path(page_id): Path<Uuid>) -> ApiResult<Json<serde_json::Value>> {
    if let Ok((ws, _, lv)) = crate::access::page_level(&pool, user.id, page_id).await {
        if lv > level::NONE {
            return Ok(Json(serde_json::json!({ "state": "ok", "workspace_id": ws })));
        }
    }
    let last: Option<(String, DateTime<Utc>)> =
        sqlx::query_as("SELECT status, created_at FROM page_access_requests WHERE page_id = $1 AND user_id = $2 ORDER BY created_at DESC LIMIT 1").bind(page_id).bind(user.id).fetch_optional(&pool).await?;
    let state = match last {
        Some((s, _)) if s == "pending" => "pending",
        Some((s, at)) if s == "denied" && at > Utc::now() - chrono::Duration::days(1) => "denied",
        _ => "none",
    };
    Ok(Json(serde_json::json!({ "state": state })))
}

#[derive(Deserialize)]
struct RequestAccessReq {
    page_id: Uuid,
    message: Option<String>,
}

/// `POST /share/request-access` : demande l'accès à une page dont on a le lien. Les personnes qui
/// gèrent la page reçoivent une notification et peuvent accepter ou refuser.
async fn request_access(State(state): State<AppState>, user: AuthUser, Json(p): Json<RequestAccessReq>) -> ApiResult<Json<serde_json::Value>> {
    let pool = &state.pool;
    let page_id = p.page_id;
    // Déjà l'accès : rien à demander.
    if let Ok((ws, _, lv)) = crate::access::page_level(pool, user.id, page_id).await {
        if lv > level::NONE {
            return Ok(Json(serde_json::json!({ "state": "ok", "workspace_id": ws })));
        }
    }
    // Page inconnue ou personnelle : même réponse que pour une vraie demande, sans rien créer.
    let page: Option<(Uuid, String, Option<Uuid>)> = sqlx::query_as("SELECT workspace_id, title, private_to FROM pages_all WHERE id = $1 AND deleted_at IS NULL").bind(page_id).fetch_optional(pool).await?;
    let Some((ws, title, None)) = page else { return Ok(Json(serde_json::json!({ "state": "pending" }))) };
    let suspended: bool = sqlx::query_scalar("SELECT COALESCE(is_suspended, FALSE) FROM users WHERE id = $1").bind(user.id).fetch_one(pool).await?;
    if suspended {
        return Err(ApiError::Forbidden);
    }
    let last: Option<(String, DateTime<Utc>)> =
        sqlx::query_as("SELECT status, created_at FROM page_access_requests WHERE page_id = $1 AND user_id = $2 ORDER BY created_at DESC LIMIT 1").bind(page_id).bind(user.id).fetch_optional(pool).await?;
    match last {
        Some((s, _)) if s == "pending" => return Ok(Json(serde_json::json!({ "state": "pending" }))),
        Some((s, at)) if s == "denied" && at > Utc::now() - chrono::Duration::days(1) => return Ok(Json(serde_json::json!({ "state": "denied" }))),
        _ => {}
    }
    let message: String = p.message.unwrap_or_default().trim().chars().take(500).collect();
    let request_id: Uuid = sqlx::query_scalar("INSERT INTO page_access_requests (page_id, user_id, message) VALUES ($1, $2, $3) RETURNING id").bind(page_id).bind(user.id).bind(&message).fetch_one(pool).await?;
    let requester: String = sqlx::query_scalar("SELECT name FROM users WHERE id = $1").bind(user.id).fetch_one(pool).await?;
    let data = serde_json::json!({ "title": title, "request_id": request_id, "requester_id": user.id, "requester_name": requester, "message": message });
    for m in page_managers(pool, page_id).await? {
        crate::notifications::notify(pool, &state.hub, ws, m, Some(user.id), "access_request", &format!("page:{page_id}"), data.clone()).await;
    }
    Ok(Json(serde_json::json!({ "state": "pending" })))
}

/// `GET /workspaces/{ws}/pages/{id}/access-requests` : demandes en attente (gestionnaires).
pub async fn list_requests(State(pool): State<PgPool>, user: AuthUser, Path((workspace_id, page_id)): Path<(Uuid, Uuid)>) -> ApiResult<Json<Vec<serde_json::Value>>> {
    let (ws, _) = crate::access::require_page_manage(&pool, &user, page_id).await?;
    if ws != workspace_id {
        return Err(ApiError::NotFound);
    }
    let rows: Vec<(Uuid, Uuid, String, String, Option<String>, String, DateTime<Utc>)> = sqlx::query_as(
        "SELECT r.id, u.id, u.name, u.email, u.avatar_url, r.message, r.created_at FROM page_access_requests r JOIN users u ON u.id = r.user_id
         WHERE r.page_id = $1 AND r.status = 'pending' ORDER BY r.created_at",
    )
    .bind(page_id)
    .fetch_all(&pool)
    .await?;
    Ok(Json(rows.into_iter().map(|(id, uid, name, email, avatar, message, at)| serde_json::json!({ "id": id, "user_id": uid, "name": name, "email": email, "avatar_url": avatar, "message": message, "created_at": at })).collect()))
}

#[derive(Deserialize)]
pub struct DecideReq {
    /// `approve` ou `deny`.
    decision: String,
    /// Niveau donné si la demande est acceptée (`read` par défaut).
    level: Option<String>,
    inherit: Option<bool>,
}

/// `POST /workspaces/{ws}/pages/{id}/access-requests/{rid}` : accepter (partage direct) ou refuser.
pub async fn decide_request(State(state): State<AppState>, user: AuthUser, Path((workspace_id, page_id, rid)): Path<(Uuid, Uuid, Uuid)>, Json(p): Json<DecideReq>) -> ApiResult<StatusCode> {
    let pool = &state.pool;
    let (ws, _) = crate::access::require_page_manage(pool, &user, page_id).await?;
    if ws != workspace_id {
        return Err(ApiError::NotFound);
    }
    let req: Option<(Uuid, String)> = sqlx::query_as("SELECT user_id, status FROM page_access_requests WHERE id = $1 AND page_id = $2").bind(rid).bind(page_id).fetch_optional(pool).await?;
    let (requester, status) = req.ok_or(ApiError::NotFound)?;
    if status != "pending" {
        return Err(ApiError::Conflict("already_decided".into()));
    }
    let approve = match p.decision.as_str() {
        "approve" => true,
        "deny" => false,
        _ => return Err(ApiError::bad_request("invalid_decision")),
    };
    if approve {
        let lv = p.level.as_deref().unwrap_or("read");
        // Par une demande, on ne donne ni la gestion ni un refus.
        if !matches!(lv, "read" | "comment" | "edit") {
            return Err(ApiError::bad_request("invalid_role"));
        }
        grant_user(&state, user.id, ws, page_id, requester, lv, p.inherit.unwrap_or(true), false).await?;
        state.emit(ws, serde_json::json!({ "type": "PAGE_PERMISSIONS_UPDATED", "page_id": page_id })).await;
    } else {
        crate::notifications::notify(pool, &state.hub, ws, requester, Some(user.id), "access_denied", &format!("page:{page_id}"), serde_json::json!({})).await;
    }
    sqlx::query("UPDATE page_access_requests SET status = $2, decided_by = $3, decided_at = NOW() WHERE id = $1").bind(rid).bind(if approve { "approved" } else { "denied" }).bind(user.id).execute(pool).await?;
    // Les autres gestionnaires n'ont plus rien à traiter.
    sqlx::query("UPDATE notifications SET read_at = NOW() WHERE kind = 'access_request' AND data->>'request_id' = $1 AND read_at IS NULL").bind(rid.to_string()).execute(pool).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Partagé avec moi
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct SharedPage {
    id: Uuid,
    workspace_id: Uuid,
    workspace_name: String,
    parent_id: Option<Uuid>,
    title: String,
    icon: Option<String>,
    updated_at: DateTime<Utc>,
    /// `viewer` ou `editor` (anciens clients) ; le niveau précis est dans `level`.
    role: String,
    level: String,
    /// Page partagée directement (sinon : sous-page d'une page partagée).
    root: bool,
    /// `direct`, `group` ou `inherited`.
    origin: &'static str,
    via_group: Option<String>,
    /// Page parente qui porte le partage (visible par construction).
    from: Option<serde_json::Value>,
    shared_by: Option<String>,
    shared_at: Option<DateTime<Utc>>,
    /// Ancêtres que la personne peut voir, du plus haut au plus proche ; `path_hidden` : il y en a d'autres, masqués.
    path: Vec<serde_json::Value>,
    path_hidden: bool,
}

/// `GET /shared` : les pages d'espaces dont je ne suis pas membre et auxquelles j'ai accès. Une
/// passe par espace (et non par page) ; chemin limité à ce que la personne peut voir.
pub async fn shared_with_me(State(pool): State<PgPool>, user: AuthUser) -> ApiResult<Json<Vec<SharedPage>>> {
    let suspended: bool = sqlx::query_scalar("SELECT COALESCE(is_suspended, FALSE) FROM users WHERE id = $1").bind(user.id).fetch_optional(&pool).await?.unwrap_or(true);
    if suspended {
        return Ok(Json(Vec::new()));
    }
    let spaces: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT DISTINCT w.id, w.name FROM page_permissions pp JOIN pages p ON p.id = pp.page_id JOIN workspaces w ON w.id = p.workspace_id
         WHERE pp.user_id = $1 AND pp.role NOT IN ('deny', 'blocked')
           AND NOT EXISTS (SELECT 1 FROM workspace_members m WHERE m.workspace_id = p.workspace_id AND m.user_id = $1::varchar)",
    )
    .bind(user.id)
    .fetch_all(&pool)
    .await?;
    let mut out = Vec::new();
    for (ws, ws_name) in spaces {
        let tree: Vec<(Uuid, Option<Uuid>, String, Option<String>, DateTime<Utc>, Option<Uuid>)> =
            sqlx::query_as("SELECT id, parent_id, title, icon, updated_at, private_to FROM pages WHERE workspace_id = $1").bind(ws).fetch_all(&pool).await?;
        #[allow(clippy::type_complexity)]
        let rows: Vec<(Uuid, String, bool, Option<String>, Option<String>, Option<DateTime<Utc>>)> = sqlx::query_as(
            "SELECT pp.page_id, pp.role, pp.inherit, g.name, u.name, pp.created_at FROM page_permissions pp JOIN pages p ON p.id = pp.page_id
             LEFT JOIN instance_groups g ON g.id = pp.via_group LEFT JOIN users u ON u.id = pp.created_by
             WHERE p.workspace_id = $1 AND pp.user_id = $2",
        )
        .bind(ws)
        .bind(user.id)
        .fetch_all(&pool)
        .await?;
        let parents: HashMap<Uuid, Option<Uuid>> = tree.iter().map(|(id, p, ..)| (*id, *p)).collect();
        let private: HashSet<Uuid> = tree.iter().filter(|t| t.5.is_some()).map(|t| t.0).collect();
        let grants: HashMap<Uuid, (String, bool)> = rows.iter().map(|r| (r.0, (r.1.clone(), r.2))).collect();
        #[allow(clippy::type_complexity)]
        let meta: HashMap<Uuid, (Option<String>, Option<String>, Option<DateTime<Utc>>)> = rows.iter().map(|r| (r.0, (r.3.clone(), r.4.clone(), r.5))).collect();
        // Niveau de chaque page, une fois.
        let levels: HashMap<Uuid, u8> =
            tree.iter().map(|(id, ..)| (*id, if private.contains(id) { level::NONE } else { resolve_level(None, nearest_grant(*id, &parents, &grants)) })).collect();
        let titles: HashMap<Uuid, &str> = tree.iter().map(|t| (t.0, t.2.as_str())).collect();
        for (id, parent, title, icon, updated_at, _) in &tree {
            let lv = levels[id];
            if lv == level::NONE {
                continue;
            }
            let from = grant_page(*id, &parents, &grants);
            let own = from == Some(*id);
            let (via_group, shared_by, shared_at) = from.and_then(|f| meta.get(&f).cloned()).unwrap_or((None, None, None));
            let anc: Vec<Uuid> = walk(*id, &parents).into_iter().skip(1).collect();
            let path: Vec<serde_json::Value> = anc.iter().rev().filter(|a| levels.get(a).is_some_and(|l| *l > level::NONE)).map(|a| serde_json::json!({ "id": a, "title": titles.get(a).copied().unwrap_or("") })).collect();
            let path_hidden = anc.iter().any(|a| levels.get(a).is_none_or(|l| *l == level::NONE));
            out.push(SharedPage {
                id: *id,
                workspace_id: ws,
                workspace_name: ws_name.clone(),
                parent_id: *parent,
                title: title.clone(),
                icon: icon.clone(),
                updated_at: *updated_at,
                role: if lv >= level::EDIT { "editor".into() } else { "viewer".into() },
                level: level::name(lv).into(),
                root: own,
                origin: if !own { "inherited" } else if via_group.is_some() { "group" } else { "direct" },
                via_group,
                from: from.filter(|f| !own && levels.get(f).is_some_and(|l| *l > level::NONE)).map(|f| serde_json::json!({ "id": f, "title": titles.get(&f).copied().unwrap_or("") })),
                shared_by,
                shared_at,
                path,
                path_hidden,
            });
        }
    }
    out.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(Json(out))
}

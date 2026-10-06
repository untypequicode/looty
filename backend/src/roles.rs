//! Rôles de l'espace (préréglages modifiables et rôles créés), attribution aux membres,
//! participation aux tâches collectives, ajout direct ou invitation par e-mail, et pages
//! partagées avec des personnes extérieures à l'espace (invités).

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, patch, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::access::{level, member_perms, require_member, require_perm, AuthUser, Perms, Role};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub fn workspace_routes() -> Router<AppState> {
    Router::new()
        .route("/roles", get(list).post(create))
        .route("/roles/:role_id", patch(update).delete(remove))
        .route("/me", get(me).patch(update_me))
        .route("/members/add", post(add_member))
}

pub fn user_routes() -> Router<AppState> {
    Router::new().route("/users/search", get(search_users)).route("/shared", get(shared_with_me))
}

#[derive(Serialize, sqlx::FromRow)]
struct RoleDto {
    id: Uuid,
    name: String,
    color: String,
    preset: Option<String>,
    permissions: serde_json::Value,
    position: i32,
    member_count: i64,
}

async fn list(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>) -> ApiResult<Json<Vec<RoleDto>>> {
    require_member(&state.pool, &user, ws).await?;
    let mut rows: Vec<RoleDto> = sqlx::query_as(
        "SELECT r.id, r.name, r.color, r.preset, r.permissions, r.position,
                (SELECT count(*) FROM workspace_members m WHERE m.workspace_id = r.workspace_id
                   AND (m.role_id = r.id OR (m.role_id IS NULL AND m.role = r.preset))) AS member_count
         FROM workspace_roles r WHERE r.workspace_id = $1 ORDER BY r.position, r.created_at",
    )
    .bind(ws)
    .fetch_all(&state.pool)
    .await?;
    // Valeurs normalisées (clés manquantes complétées).
    for r in &mut rows {
        let base = r.preset.as_deref().and_then(Role::parse).unwrap_or(Role::Viewer);
        r.permissions = if base == Role::Owner { Perms::all().to_json() } else { Perms::from_json(&r.permissions, Perms::preset(base)).to_json() };
    }
    Ok(Json(rows))
}

#[derive(Deserialize)]
struct RoleReq {
    name: Option<String>,
    color: Option<String>,
    permissions: Option<serde_json::Value>,
    position: Option<i32>,
}

fn clean_name(n: &str) -> ApiResult<String> {
    let n = n.trim();
    if n.is_empty() || n.chars().count() > 60 {
        return Err(ApiError::bad_request("invalid_name"));
    }
    Ok(n.to_string())
}

fn clean_color(c: &str) -> ApiResult<String> {
    if c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|x| x.is_ascii_hexdigit()) {
        Ok(c.to_string())
    } else {
        Err(ApiError::bad_request("invalid_value"))
    }
}

/// Seul le propriétaire peut créer ou modifier un rôle doté des droits d'administration.
fn check_admin_grant(my_role: Role, perms: &Perms) -> ApiResult<()> {
    if perms.admin && my_role != Role::Owner {
        return Err(ApiError::Forbidden);
    }
    Ok(())
}

async fn changed(state: &AppState, ws: Uuid) {
    state.emit(ws, serde_json::json!({ "type": "MEMBERS_UPDATED" })).await;
}

async fn create(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>, Json(p): Json<RoleReq>) -> ApiResult<Json<serde_json::Value>> {
    let (my_role, _) = require_perm(&state.pool, &user, ws, |p| p.admin).await?;
    let name = clean_name(p.name.as_deref().unwrap_or(""))?;
    let color = clean_color(p.color.as_deref().unwrap_or("#8a8f98"))?;
    let perms = Perms::from_json(p.permissions.as_ref().unwrap_or(&serde_json::json!({})), Perms::preset(Role::Viewer));
    check_admin_grant(my_role, &perms)?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM workspace_roles WHERE workspace_id = $1").bind(ws).fetch_one(&state.pool).await?;
    if count >= 50 {
        return Err(ApiError::bad_request("too_many_roles"));
    }
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO workspace_roles (workspace_id, name, color, permissions, position) VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(ws)
    .bind(name)
    .bind(color)
    .bind(perms.to_json())
    .bind(count as i32)
    .fetch_one(&state.pool)
    .await?;
    changed(&state, ws).await;
    Ok(Json(serde_json::json!({ "id": id })))
}

async fn load_role(state: &AppState, ws: Uuid, id: Uuid) -> ApiResult<(Option<String>, Perms)> {
    role_info(&state.pool, ws, id).await
}

/// Préréglage d'origine et permissions d'un rôle de l'espace.
pub async fn role_info(pool: &sqlx::PgPool, ws: Uuid, id: Uuid) -> ApiResult<(Option<String>, Perms)> {
    let row: Option<(Option<String>, serde_json::Value)> =
        sqlx::query_as("SELECT preset, permissions FROM workspace_roles WHERE id = $1 AND workspace_id = $2").bind(id).bind(ws).fetch_optional(pool).await?;
    let (preset, json) = row.ok_or(ApiError::NotFound)?;
    let base = preset.as_deref().and_then(Role::parse).unwrap_or(Role::Viewer);
    Ok((preset, if base == Role::Owner { Perms::all() } else { Perms::from_json(&json, Perms::preset(base)) }))
}

async fn update(State(state): State<AppState>, user: AuthUser, Path((ws, id)): Path<(Uuid, Uuid)>, Json(p): Json<RoleReq>) -> ApiResult<StatusCode> {
    let (my_role, _) = require_perm(&state.pool, &user, ws, |p| p.admin).await?;
    let (preset, old) = load_role(&state, ws, id).await?;
    // Un rôle d'administration ne se modifie que par le propriétaire.
    if old.admin && my_role != Role::Owner {
        return Err(ApiError::Forbidden);
    }
    let name = p.name.as_deref().map(clean_name).transpose()?;
    let color = p.color.as_deref().map(clean_color).transpose()?;
    let perms = match (&p.permissions, preset.as_deref()) {
        // Les droits du propriétaire ne changent pas.
        (Some(_), Some("owner")) => return Err(ApiError::bad_request("owner_fixed")),
        (Some(j), _) => {
            let np = Perms::from_json(j, old);
            check_admin_grant(my_role, &np)?;
            Some(np)
        }
        (None, _) => None,
    };
    sqlx::query(
        "UPDATE workspace_roles SET name = COALESCE($3, name), color = COALESCE($4, color), permissions = COALESCE($5, permissions), position = COALESCE($6, position)
         WHERE id = $1 AND workspace_id = $2",
    )
    .bind(id)
    .bind(ws)
    .bind(name)
    .bind(color)
    .bind(perms.map(|p| p.to_json()))
    .bind(p.position)
    .execute(&state.pool)
    .await?;
    // Rôle de base des membres (contrôles généraux « écriture » / « admin »).
    if let (Some(np), None) = (perms, preset.as_deref()) {
        sqlx::query("UPDATE workspace_members SET role = $3 WHERE workspace_id = $1 AND role_id = $2 AND role <> 'owner'")
            .bind(ws)
            .bind(id)
            .bind(np.base_role().as_str())
            .execute(&state.pool)
            .await?;
    }
    changed(&state, ws).await;
    Ok(StatusCode::NO_CONTENT)
}

/// Suppression d'un rôle créé : ses membres reviennent au préréglage de leur rôle de base.
async fn remove(State(state): State<AppState>, user: AuthUser, Path((ws, id)): Path<(Uuid, Uuid)>) -> ApiResult<StatusCode> {
    let (my_role, _) = require_perm(&state.pool, &user, ws, |p| p.admin).await?;
    let (preset, perms) = load_role(&state, ws, id).await?;
    if preset.is_some() {
        return Err(ApiError::bad_request("preset_role"));
    }
    if perms.admin && my_role != Role::Owner {
        return Err(ApiError::Forbidden);
    }
    let mut tx = state.pool.begin().await?;
    sqlx::query(
        "UPDATE workspace_members m SET role_id = r.id FROM workspace_roles r
         WHERE m.workspace_id = $1 AND m.role_id = $2 AND r.workspace_id = m.workspace_id AND r.preset = m.role",
    )
    .bind(ws)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM workspace_roles WHERE id = $1 AND workspace_id = $2").bind(id).bind(ws).execute(&mut *tx).await?;
    tx.commit().await?;
    changed(&state, ws).await;
    Ok(StatusCode::NO_CONTENT)
}

/// Attribue un rôle (préréglage ou créé) à un membre. Appelé par la route des membres.
pub async fn assign_role(state: &AppState, user: &AuthUser, ws: Uuid, member: Uuid, role_id: Uuid) -> ApiResult<()> {
    let pool = &state.pool;
    let (my_role, _) = require_perm(pool, user, ws, |p| p.admin).await?;
    let (preset, perms) = load_role(state, ws, role_id).await?;
    let target = crate::access::workspace_role(pool, member, ws).await?.ok_or(ApiError::NotFound)?;
    let new_base = match preset.as_deref() {
        Some(p) => Role::parse(p).unwrap_or(Role::Viewer),
        None => perms.base_role(),
    };
    // Un admin ne peut ni donner des droits d'administration, ni toucher à un admin / propriétaire.
    if my_role != Role::Owner && (perms.admin || new_base >= Role::Admin || target >= Role::Admin) {
        return Err(ApiError::Forbidden);
    }
    if target == Role::Owner && new_base != Role::Owner {
        let owners: i64 = sqlx::query_scalar("SELECT count(*) FROM workspace_members WHERE workspace_id = $1 AND role = 'owner'").bind(ws).fetch_one(pool).await?;
        if owners <= 1 {
            return Err(ApiError::Conflict("last_owner".into()));
        }
    }
    sqlx::query("UPDATE workspace_members SET role = $3, role_id = $4 WHERE workspace_id = $1 AND user_id = $2")
        .bind(ws)
        .bind(member.to_string())
        .bind(new_base.as_str())
        .bind(role_id)
        .execute(pool)
        .await?;
    state.emit(ws, serde_json::json!({ "type": "MEMBERS_UPDATED", "user_id": member })).await;
    Ok(())
}

/// Mon rôle et mes permissions dans l'espace (affichage de l'interface).
async fn me(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>) -> ApiResult<Json<serde_json::Value>> {
    let (role, perms) = member_perms(&state.pool, user.id, ws).await?.ok_or(ApiError::NotFound)?;
    let row: (Option<Uuid>, Option<String>, bool) = sqlx::query_as(
        "SELECT r.id, r.name, m.group_tasks FROM workspace_members m
         LEFT JOIN workspace_roles r ON r.workspace_id = m.workspace_id AND (r.id = m.role_id OR (m.role_id IS NULL AND r.preset = m.role))
         WHERE m.workspace_id = $1 AND m.user_id = $2 ORDER BY (r.id = m.role_id) DESC NULLS LAST LIMIT 1",
    )
    .bind(ws)
    .bind(user.id.to_string())
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(serde_json::json!({ "role": role.as_str(), "role_id": row.0, "role_name": row.1, "permissions": perms.to_json(), "group_tasks": row.2 })))
}

#[derive(Deserialize)]
struct MeReq {
    group_tasks: Option<bool>,
}

async fn update_me(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>, Json(p): Json<MeReq>) -> ApiResult<StatusCode> {
    require_member(&state.pool, &user, ws).await?;
    if let Some(g) = p.group_tasks {
        sqlx::query("UPDATE workspace_members SET group_tasks = $3 WHERE workspace_id = $1 AND user_id = $2")
            .bind(ws)
            .bind(user.id.to_string())
            .bind(g)
            .execute(&state.pool)
            .await?;
        changed(&state, ws).await;
    }
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Ajout de membres : personne existante choisie directement, ou e-mail
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
    /// Exclure les membres de cet espace.
    exclude_ws: Option<Uuid>,
}

#[derive(Serialize, sqlx::FromRow)]
struct UserHit {
    id: Uuid,
    name: String,
    email: String,
    avatar_url: Option<String>,
}

/// Recherche de comptes Looty (nom ou e-mail), pour inviter ou partager une page.
async fn search_users(State(state): State<AppState>, _user: AuthUser, Query(q): Query<SearchQuery>) -> ApiResult<Json<Vec<UserHit>>> {
    let term = q.q.trim().to_lowercase();
    if term.chars().count() < 2 {
        return Ok(Json(Vec::new()));
    }
    let like = format!("%{}%", term.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
    let rows: Vec<UserHit> = sqlx::query_as(
        "SELECT u.id, u.name, u.email, u.avatar_url FROM users u
         WHERE COALESCE(u.is_suspended, FALSE) = FALSE AND (lower(u.name) LIKE $1 OR lower(u.email) LIKE $1)
           AND ($2::uuid IS NULL OR NOT EXISTS (SELECT 1 FROM workspace_members m WHERE m.workspace_id = $2 AND m.user_id = u.id::varchar))
         ORDER BY (lower(u.email) = $3) DESC, u.name LIMIT 8",
    )
    .bind(like)
    .bind(q.exclude_ws)
    .bind(&term)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

#[derive(Deserialize)]
struct AddReq {
    user_id: Option<Uuid>,
    email: Option<String>,
    role_id: Option<Uuid>,
}

/// Ajoute directement une personne ayant déjà un compte (notifiée par e-mail si possible),
/// sinon envoie une invitation par e-mail (lien valable 14 jours, une utilisation).
async fn add_member(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>, Json(p): Json<AddReq>) -> ApiResult<Json<serde_json::Value>> {
    let pool = &state.pool;
    let (my_role, _) = require_perm(pool, &user, ws, |p| p.invite).await?;
    // Rôle donné : par défaut « membre » ; jamais plus élevé que le sien.
    let role_id = match p.role_id {
        Some(r) => r,
        None => sqlx::query_scalar("SELECT id FROM workspace_roles WHERE workspace_id = $1 AND preset = 'member'").bind(ws).fetch_one(pool).await?,
    };
    let (preset, perms) = load_role(&state, ws, role_id).await?;
    let base = match preset.as_deref() {
        Some(p) => Role::parse(p).unwrap_or(Role::Viewer),
        None => perms.base_role(),
    };
    if base == Role::Owner || (base >= Role::Admin && my_role != Role::Owner) {
        return Err(ApiError::Forbidden);
    }
    let ws_name: String = sqlx::query_scalar("SELECT name FROM workspaces WHERE id = $1").bind(ws).fetch_one(pool).await?;
    let inviter: String = sqlx::query_scalar("SELECT name FROM users WHERE id = $1").bind(user.id).fetch_one(pool).await?;

    let target: Option<(Uuid, String, String)> = match (p.user_id, p.email.as_deref()) {
        (Some(id), _) => sqlx::query_as("SELECT id, name, email FROM users WHERE id = $1").bind(id).fetch_optional(pool).await?,
        (None, Some(e)) => sqlx::query_as("SELECT id, name, email FROM users WHERE lower(email) = lower($1)").bind(e.trim()).fetch_optional(pool).await?,
        _ => return Err(ApiError::bad_request("invalid_value")),
    };
    if let Some((uid, name, email)) = target {
        let n = sqlx::query("INSERT INTO workspace_members (workspace_id, user_id, role, role_id) VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING")
            .bind(ws)
            .bind(uid.to_string())
            .bind(base.as_str())
            .bind(role_id)
            .execute(pool)
            .await?
            .rows_affected();
        if n == 0 {
            return Err(ApiError::Conflict("already_member".into()));
        }
        state.emit(ws, serde_json::json!({ "type": "MEMBERS_UPDATED", "user_id": uid })).await;
        let link = format!("{}/w/{ws}", state.base_url);
        send_mail(&state, &email, &name, &format!("{inviter} vous a ajouté à « {ws_name} » sur Looty"),
            &format!("{inviter} vous a ajouté à l’espace « {ws_name} ».\n\nOuvrir : {link}\n\n{inviter} added you to the “{ws_name}” workspace.")).await;
        return Ok(Json(serde_json::json!({ "added": uid })));
    }
    // Pas encore de compte : invitation par e-mail.
    let email = p.email.as_deref().map(str::trim).filter(|e| e.contains('@') && e.len() <= 255).ok_or_else(|| ApiError::bad_request("invalid_email"))?;
    let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    sqlx::query(
        "INSERT INTO workspace_invites (workspace_id, token_hash, role, role_id, created_by, expires_at, max_uses, email)
         VALUES ($1, $2, $3, $4, $5, NOW() + interval '14 days', 1, $6)",
    )
    .bind(ws)
    .bind(crate::access::hash_token(&token))
    .bind(base.as_str())
    .bind(role_id)
    .bind(user.id)
    .bind(email)
    .execute(pool)
    .await?;
    let link = format!("{}/invite/{token}", state.base_url);
    let sent = send_mail(&state, email, "", &format!("{inviter} vous invite sur Looty : « {ws_name} »"),
        &format!("{inviter} vous invite à rejoindre l’espace « {ws_name} ».\nAccepter l’invitation (valable 14 jours) : {link}\n\n{inviter} invites you to join the “{ws_name}” workspace: {link}")).await;
    Ok(Json(serde_json::json!({ "invited": email, "email_sent": sent, "link": link })))
}

/// Envoi d'un e-mail si le serveur SMTP est configuré ; renvoie s'il a été envoyé.
pub async fn send_mail(state: &AppState, to: &str, to_name: &str, subject: &str, text: &str) -> bool {
    let Some(cfg) = crate::mail::config(&state.pool).await else { return false };
    let html = format!("<div style=\"font-family:sans-serif;font-size:15px;line-height:1.5\">{}</div>",
        text.split('\n').map(|l| {
            let esc = l.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
            if let Some(i) = esc.find("http") { let (a, b) = esc.split_at(i); let url = b.split_whitespace().next().unwrap_or(b); format!("{a}<a href=\"{url}\">{url}</a>{}", &b[url.len()..]) } else { esc }
        }).collect::<Vec<_>>().join("<br>"));
    crate::mail::send(&cfg, &crate::mail::Email { to: to.into(), to_name: to_name.into(), subject: subject.into(), text: text.into(), html }).await.is_ok()
}

// ---------------------------------------------------------------------------
// Pages partagées avec moi (invité hors de l'espace)
// ---------------------------------------------------------------------------

#[derive(Serialize, sqlx::FromRow)]
struct SharedPage {
    id: Uuid,
    workspace_id: Uuid,
    workspace_name: String,
    parent_id: Option<Uuid>,
    title: String,
    icon: Option<String>,
    updated_at: DateTime<Utc>,
    role: String,
    /// Page partagée directement (sinon : sous-page d'une page partagée).
    root: bool,
}

async fn shared_with_me(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<Vec<SharedPage>>> {
    let rows: Vec<SharedPage> = sqlx::query_as(
        "SELECT DISTINCT ON (p.id) p.id, p.workspace_id, w.name AS workspace_name, p.parent_id, p.title, p.icon, p.updated_at, pp.role, TRUE AS root
         FROM page_permissions pp JOIN pages p ON p.id = pp.page_id JOIN workspaces w ON w.id = p.workspace_id
         WHERE pp.user_id = $1 AND pp.role IN ('viewer', 'editor')
           AND NOT EXISTS (SELECT 1 FROM workspace_members m WHERE m.workspace_id = p.workspace_id AND m.user_id = $1::varchar)
         ORDER BY p.id",
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await?;
    // Sous-pages : contrôle d'accès réel (permissions héritées, blocages éventuels).
    let mut out = rows;
    let roots: Vec<(Uuid, Uuid)> = out.iter().map(|r| (r.id, r.workspace_id)).collect();
    for (root, _) in roots {
        let children: Vec<(Uuid, Uuid, String, Option<Uuid>, String, Option<String>, DateTime<Utc>)> = sqlx::query_as(
            "WITH RECURSIVE d AS (
                SELECT id, 0 AS depth FROM pages WHERE parent_id = $1
                UNION ALL SELECT p.id, d.depth + 1 FROM pages p JOIN d ON p.parent_id = d.id WHERE d.depth < 32
             )
             SELECT p.id, p.workspace_id, w.name, p.parent_id, p.title, p.icon, p.updated_at FROM d JOIN pages p ON p.id = d.id JOIN workspaces w ON w.id = p.workspace_id",
        )
        .bind(root)
        .fetch_all(&state.pool)
        .await?;
        for (id, ws, wname, parent, title, icon, updated) in children {
            if out.iter().any(|x| x.id == id) {
                continue;
            }
            if let Ok((_, Role::Guest, access)) = crate::access::page_access(&state.pool, &user, id).await {
                let role = if access == crate::access::PageAccess::Write { "editor" } else { "viewer" };
                out.push(SharedPage { id, workspace_id: ws, workspace_name: wname, parent_id: parent, title, icon, updated_at: updated, role: role.into(), root: false });
            }
        }
    }
    out.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(Json(out))
}

/// Rôle de base correspondant à un rôle de l'espace.
pub fn base_of(preset: Option<&str>, perms: &Perms) -> Role {
    match preset {
        Some(p) => Role::parse(p).unwrap_or(Role::Viewer),
        None => perms.base_role(),
    }
}

/// Niveau minimal pour commenter une cible (pages : documents ; événements : calendrier).
pub fn can_comment(perms: &Perms, kind: &str) -> bool {
    match kind {
        "page" => perms.docs >= level::COMMENT,
        _ => perms.calendar >= level::READ,
    }
}

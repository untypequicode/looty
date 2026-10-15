//! Authentification (extracteur `AuthUser`) et contrôle d'accès centralisé.
//!
//! Toutes les routes protégées passent par ces helpers : aucun handler ne doit
//! faire confiance à un identifiant (tâche, colonne, page...) sans vérifier
//! d'abord à quel workspace il appartient et le rôle de l'utilisateur dedans.

use axum::{
    extract::{FromRef, FromRequestParts, OptionalFromRequestParts},
    http::{header::AUTHORIZATION, request::Parts},
};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::collections::HashMap;
use tower_sessions::Session;
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};

pub const SESSION_USER_KEY: &str = "user_id";

/// Utilisateur authentifié (session ou jeton API `Authorization: Bearer looty_...`).
#[derive(Clone, Copy, Debug)]
pub struct AuthUser {
    pub id: Uuid,
    pub is_super_admin: bool,
}

pub fn hash_token(raw: &str) -> String {
    hex::encode(Sha256::digest(raw.as_bytes()))
}

/// `Option<AuthUser>` : utilisateur connecté s'il y en a un (pages publiques, versions…).
impl<S> OptionalFromRequestParts<S> for AuthUser
where
    S: Send + Sync,
    PgPool: FromRef<S>,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Option<Self>, Self::Rejection> {
        Ok(<AuthUser as FromRequestParts<S>>::from_request_parts(parts, state).await.ok())
    }
}

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
    PgPool: FromRef<S>,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let pool = PgPool::from_ref(state);

        // 1. Jeton API
        if let Some(auth) = parts.headers.get(AUTHORIZATION).and_then(|h| h.to_str().ok()) {
            if let Some(raw) = auth.strip_prefix("Bearer ") {
                let row: Option<(Uuid, bool, bool)> = sqlx::query_as(
                    "UPDATE api_tokens t SET last_used_at = NOW()
                     FROM users u
                     WHERE t.token_hash = $1 AND t.scope = 'api' AND u.id = t.user_id
                     RETURNING u.id, u.is_super_admin, u.is_suspended",
                )
                .bind(hash_token(raw.trim()))
                .fetch_optional(&pool)
                .await?;
                return match row {
                    Some((id, is_super_admin, false)) => Ok(AuthUser { id, is_super_admin }),
                    _ => Err(ApiError::Unauthorized),
                };
            }
        }

        // 2. Session cookie
        let session = Session::from_request_parts(parts, state)
            .await
            .map_err(|_| ApiError::Unauthorized)?;
        let user_id: Uuid = session
            .get(SESSION_USER_KEY)
            .await
            .ok()
            .flatten()
            .ok_or(ApiError::Unauthorized)?;

        let row: Option<(bool, bool)> =
            sqlx::query_as("SELECT is_super_admin, is_suspended FROM users WHERE id = $1")
                .bind(user_id)
                .fetch_optional(&pool)
                .await?;
        match row {
            Some((is_super_admin, false)) => Ok(AuthUser { id: user_id, is_super_admin }),
            _ => {
                // Utilisateur supprimé ou suspendu : on invalide la session.
                let _ = session.flush().await;
                Err(ApiError::Unauthorized)
            }
        }
    }
}

/// Rôle d'un membre dans un workspace (ordre croissant de privilèges).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    /// Personne extérieure à l'espace, invitée sur certaines pages seulement.
    Guest,
    Viewer,
    Member,
    Admin,
    Owner,
}

impl Role {
    pub fn parse(s: &str) -> Option<Role> {
        match s {
            "viewer" => Some(Role::Viewer),
            "member" => Some(Role::Member),
            "admin" => Some(Role::Admin),
            "owner" => Some(Role::Owner),
            _ => None,
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Guest => "guest",
            Role::Viewer => "viewer",
            Role::Member => "member",
            Role::Admin => "admin",
            Role::Owner => "owner",
        }
    }
    pub fn can_write(&self) -> bool {
        *self >= Role::Member
    }
    pub fn is_admin(&self) -> bool {
        *self >= Role::Admin
    }
}

// ---------------------------------------------------------------------------
// Permissions fines (rôles de l'espace)
// ---------------------------------------------------------------------------

/// Niveaux d'accès d'un domaine (échelle commune ; les documents n'utilisent pas
/// `VALIDATE` ni `MANAGE`, le calendrier seulement NONE / READ / EDIT).
pub mod level {
    pub const NONE: u8 = 0;
    pub const READ: u8 = 1;
    pub const COMMENT: u8 = 2;
    /// Tableaux : valider / terminer des tâches (sans les modifier).
    pub const VALIDATE: u8 = 3;
    pub const EDIT: u8 = 4;
    /// Tableaux : créer et paramétrer les tableaux, colonnes, étiquettes.
    pub const MANAGE: u8 = 5;

    pub fn parse(s: &str) -> Option<u8> {
        Some(match s {
            "none" => NONE,
            "read" => READ,
            "comment" => COMMENT,
            "validate" => VALIDATE,
            "edit" => EDIT,
            "manage" => MANAGE,
            _ => return None,
        })
    }
    pub fn name(l: u8) -> &'static str {
        match l {
            NONE => "none",
            READ => "read",
            COMMENT => "comment",
            VALIDATE => "validate",
            EDIT => "edit",
            _ => "manage",
        }
    }
}

/// Permissions effectives d'un membre dans un espace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Perms {
    pub docs: u8,
    pub boards: u8,
    pub calendar: u8,
    pub invite: bool,
    pub share: bool,
    pub admin: bool,
}

impl Perms {
    pub fn all() -> Perms {
        Perms { docs: level::EDIT, boards: level::MANAGE, calendar: level::EDIT, invite: true, share: true, admin: true }
    }
    /// Permissions par défaut d'un préréglage (si le rôle n'existe pas en base).
    pub fn preset(role: Role) -> Perms {
        match role {
            Role::Owner | Role::Admin => Perms::all(),
            Role::Member => Perms { docs: level::EDIT, boards: level::MANAGE, calendar: level::EDIT, invite: false, share: true, admin: false },
            Role::Viewer => Perms { docs: level::COMMENT, boards: level::COMMENT, calendar: level::READ, invite: false, share: false, admin: false },
            Role::Guest => Perms { docs: level::NONE, boards: level::NONE, calendar: level::NONE, invite: false, share: false, admin: false },
        }
    }
    /// Lecture du JSON stocké ; les clés absentes prennent la valeur du préréglage `base`.
    pub fn from_json(v: &serde_json::Value, base: Perms) -> Perms {
        let lv = |k: &str, d: u8| v.get(k).and_then(|x| x.as_str()).and_then(level::parse).unwrap_or(d);
        let b = |k: &str, d: bool| v.get(k).and_then(|x| x.as_bool()).unwrap_or(d);
        Perms {
            docs: lv("docs", base.docs).min(level::EDIT),
            boards: lv("boards", base.boards),
            calendar: lv("calendar", base.calendar).min(level::EDIT),
            invite: b("invite", base.invite),
            share: b("share", base.share),
            admin: b("admin", base.admin),
        }
    }
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "docs": level::name(self.docs), "boards": level::name(self.boards), "calendar": level::name(self.calendar),
            "invite": self.invite, "share": self.share, "admin": self.admin,
        })
    }
    /// Rôle de base équivalent (anciens contrôles « écriture » / « admin »).
    pub fn base_role(&self) -> Role {
        if self.admin {
            Role::Admin
        } else if self.docs >= level::EDIT || self.boards >= level::EDIT || self.calendar >= level::EDIT {
            Role::Member
        } else {
            Role::Viewer
        }
    }
}

/// Rôle et permissions d'un membre (None : pas membre de l'espace).
pub async fn member_perms(pool: &PgPool, user_id: Uuid, workspace_id: Uuid) -> ApiResult<Option<(Role, Perms)>> {
    let row: Option<(String, Option<serde_json::Value>)> = sqlx::query_as(
        "SELECT m.role, r.permissions FROM workspace_members m
         LEFT JOIN workspace_roles r ON r.workspace_id = m.workspace_id AND (r.id = m.role_id OR (m.role_id IS NULL AND r.preset = m.role))
         WHERE m.workspace_id = $1 AND m.user_id = $2
         ORDER BY (r.id = m.role_id) DESC NULLS LAST LIMIT 1",
    )
    .bind(workspace_id)
    .bind(user_id.to_string())
    .fetch_optional(pool)
    .await?;
    let Some((role, perms)) = row else { return Ok(None) };
    let Some(role) = Role::parse(&role) else { return Ok(None) };
    let perms = if role == Role::Owner { Perms::all() } else { perms.map(|p| Perms::from_json(&p, Perms::preset(role))).unwrap_or(Perms::preset(role)) };
    Ok(Some((role, perms)))
}

/// Exige d'être membre et une permission (`check`) ; NotFound si non membre, Forbidden sinon.
pub async fn require_perm(pool: &PgPool, user: &AuthUser, workspace_id: Uuid, check: impl Fn(&Perms) -> bool) -> ApiResult<(Role, Perms)> {
    let (role, perms) = member_perms(pool, user.id, workspace_id).await?.ok_or(ApiError::NotFound)?;
    if !check(&perms) {
        return Err(ApiError::Forbidden);
    }
    Ok((role, perms))
}

pub async fn workspace_role(pool: &PgPool, user_id: Uuid, workspace_id: Uuid) -> ApiResult<Option<Role>> {
    let role: Option<String> =
        sqlx::query_scalar("SELECT role FROM workspace_members WHERE workspace_id = $1 AND user_id = $2")
            .bind(workspace_id)
            .bind(user_id.to_string())
            .fetch_optional(pool)
            .await?;
    Ok(role.as_deref().and_then(Role::parse))
}

/// Exige que l'utilisateur soit membre (n'importe quel rôle). Renvoie NotFound
/// plutôt que Forbidden pour ne pas révéler l'existence de la ressource.
pub async fn require_member(pool: &PgPool, user: &AuthUser, workspace_id: Uuid) -> ApiResult<Role> {
    workspace_role(pool, user.id, workspace_id).await?.ok_or(ApiError::NotFound)
}

/// Invité d'un espace : personne extérieure avec au moins une page partagée dans l'espace.
pub async fn is_guest(pool: &PgPool, user_id: Uuid, workspace_id: Uuid) -> ApiResult<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM page_permissions pp JOIN pages p ON p.id = pp.page_id
          WHERE p.workspace_id = $1 AND pp.user_id = $2 AND pp.role IN ('viewer', 'editor'))",
    )
    .bind(workspace_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?)
}

/// Membre, ou invité (`Role::Guest`) pour les quelques lectures dont l'interface de
/// l'espace a besoin (espace, pages visibles, droits). Les autres routes gardent
/// `require_member` : un invité n'y voit rien.
pub async fn require_member_or_guest(pool: &PgPool, user: &AuthUser, workspace_id: Uuid) -> ApiResult<Role> {
    if let Some(role) = workspace_role(pool, user.id, workspace_id).await? {
        return Ok(role);
    }
    if is_guest(pool, user.id, workspace_id).await? {
        return Ok(Role::Guest);
    }
    Err(ApiError::NotFound)
}

pub async fn require_writer(pool: &PgPool, user: &AuthUser, workspace_id: Uuid) -> ApiResult<Role> {
    let role = require_member(pool, user, workspace_id).await?;
    if !role.can_write() {
        return Err(ApiError::Forbidden);
    }
    Ok(role)
}

pub async fn require_ws_admin(pool: &PgPool, user: &AuthUser, workspace_id: Uuid) -> ApiResult<Role> {
    let role = require_member(pool, user, workspace_id).await?;
    if !role.is_admin() {
        return Err(ApiError::Forbidden);
    }
    Ok(role)
}

// ---------------------------------------------------------------------------
// Résolution des ressources -> workspace
// ---------------------------------------------------------------------------

pub async fn board_workspace(pool: &PgPool, board_id: Uuid) -> ApiResult<Uuid> {
    sqlx::query_scalar("SELECT workspace_id FROM boards WHERE id = $1")
        .bind(board_id)
        .fetch_optional(pool)
        .await?
        .ok_or(ApiError::NotFound)
}

#[derive(sqlx::FromRow, Clone, Copy)]
pub struct ColumnCtx {
    pub workspace_id: Uuid,
    pub board_id: Uuid,
}

pub async fn column_ctx(pool: &PgPool, column_id: Uuid) -> ApiResult<ColumnCtx> {
    sqlx::query_as(
        "SELECT b.workspace_id, c.board_id FROM board_columns c JOIN boards b ON b.id = c.board_id WHERE c.id = $1",
    )
    .bind(column_id)
    .fetch_optional(pool)
    .await?
    .ok_or(ApiError::NotFound)
}

#[derive(sqlx::FromRow, Clone, Copy)]
pub struct TaskCtx {
    pub workspace_id: Uuid,
    pub board_id: Uuid,
    pub column_id: Uuid,
}

pub async fn task_ctx(pool: &PgPool, task_id: Uuid) -> ApiResult<TaskCtx> {
    sqlx::query_as(
        "SELECT b.workspace_id, c.board_id, t.column_id
         FROM tasks t JOIN board_columns c ON c.id = t.column_id JOIN boards b ON b.id = c.board_id
         WHERE t.id = $1",
    )
    .bind(task_id)
    .fetch_optional(pool)
    .await?
    .ok_or(ApiError::NotFound)
}

pub async fn label_ctx(pool: &PgPool, label_id: Uuid) -> ApiResult<(Uuid, Uuid)> {
    sqlx::query_as("SELECT b.workspace_id, l.board_id FROM labels l JOIN boards b ON b.id = l.board_id WHERE l.id = $1")
        .bind(label_id)
        .fetch_optional(pool)
        .await?
        .ok_or(ApiError::NotFound)
}

/// Vérifie qu'un identifiant utilisateur est bien membre du workspace (pour les assignations).
pub async fn is_member(pool: &PgPool, workspace_id: Uuid, user_id: &str) -> ApiResult<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM workspace_members WHERE workspace_id = $1 AND user_id = $2)",
    )
    .bind(workspace_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?)
}

// ---------------------------------------------------------------------------
// Pages : permissions par page (héritées des parents)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageAccess {
    None,
    Read,
    Write,
}

/// Calcule l'accès effectif à une page.
///
/// - un admin/owner du workspace a toujours accès en écriture ;
/// - sinon, la permission explicite la plus proche (page ou ancêtre) s'applique :
///   `blocked` -> aucun accès, `viewer` -> lecture, `editor` -> écriture ;
/// - à défaut, le rôle workspace s'applique (viewer -> lecture, member -> écriture).
pub fn effective_page_access(perms: &Perms, override_role: Option<&str>) -> PageAccess {
    if perms.admin {
        return PageAccess::Write;
    }
    match override_role {
        Some("blocked") => PageAccess::None,
        Some("viewer") => PageAccess::Read,
        Some("editor") => PageAccess::Write,
        _ => match perms.docs {
            level::NONE => PageAccess::None,
            l if l >= level::EDIT => PageAccess::Write,
            _ => PageAccess::Read,
        },
    }
}

pub async fn page_workspace(pool: &PgPool, page_id: Uuid) -> ApiResult<Uuid> {
    sqlx::query_scalar("SELECT workspace_id FROM pages WHERE id = $1")
        .bind(page_id)
        .fetch_optional(pool)
        .await?
        .ok_or(ApiError::NotFound)
}

/// Permission explicite la plus proche dans la chaîne d'ancêtres d'une page.
pub async fn nearest_page_override(pool: &PgPool, page_id: Uuid, user_id: Uuid) -> ApiResult<Option<String>> {
    Ok(sqlx::query_scalar(
        "WITH RECURSIVE anc AS (
            SELECT id, parent_id, 0 AS depth FROM pages WHERE id = $1
            UNION ALL
            SELECT p.id, p.parent_id, anc.depth + 1 FROM pages p JOIN anc ON p.id = anc.parent_id
            WHERE anc.depth < 64
         )
         SELECT pp.role FROM anc JOIN page_permissions pp ON pp.page_id = anc.id AND pp.user_id = $2
         WHERE anc.depth = 0 OR pp.inherit
         ORDER BY anc.depth LIMIT 1",
    )
    .bind(page_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?)
}

/// Accès effectif d'un utilisateur à une page (vérifie aussi l'appartenance au workspace).
pub async fn page_access(pool: &PgPool, user: &AuthUser, page_id: Uuid) -> ApiResult<(Uuid, Role, PageAccess)> {
    let workspace_id = page_workspace(pool, page_id).await?;
    match member_perms(pool, user.id, workspace_id).await? {
        Some((role, perms)) => {
            let ov = if perms.admin { None } else { nearest_page_override(pool, page_id, user.id).await? };
            Ok((workspace_id, role, effective_page_access(&perms, ov.as_deref())))
        }
        // Invité : seulement les pages partagées explicitement avec lui (et leurs sous-pages).
        None => match nearest_page_override(pool, page_id, user.id).await?.as_deref() {
            Some("editor") => Ok((workspace_id, Role::Guest, PageAccess::Write)),
            Some("viewer") => Ok((workspace_id, Role::Guest, PageAccess::Read)),
            _ => Err(ApiError::NotFound),
        },
    }
}

pub async fn require_page_read(pool: &PgPool, user: &AuthUser, page_id: Uuid) -> ApiResult<(Uuid, Role, PageAccess)> {
    let r = page_access(pool, user, page_id).await?;
    if r.2 == PageAccess::None {
        return Err(ApiError::NotFound);
    }
    Ok(r)
}

pub async fn require_page_write(pool: &PgPool, user: &AuthUser, page_id: Uuid) -> ApiResult<(Uuid, Role)> {
    let (ws, role, access) = page_access(pool, user, page_id).await?;
    match access {
        PageAccess::Write => Ok((ws, role)),
        PageAccess::Read => Err(ApiError::Forbidden),
        PageAccess::None => Err(ApiError::NotFound),
    }
}

/// Calcule l'accès de l'utilisateur à toutes les pages d'un workspace en une passe
/// (utilisé pour filtrer les listes, la recherche, l'accueil...).
pub async fn visible_pages(pool: &PgPool, user: &AuthUser, workspace_id: Uuid, _role: Role) -> ApiResult<HashMap<Uuid, PageAccess>> {
    let perms = member_perms(pool, user.id, workspace_id).await?.map(|(_, p)| p).unwrap_or(Perms::preset(Role::Guest));
    let tree: Vec<(Uuid, Option<Uuid>)> =
        sqlx::query_as("SELECT id, parent_id FROM pages WHERE workspace_id = $1")
            .bind(workspace_id)
            .fetch_all(pool)
            .await?;
    let overrides: HashMap<Uuid, (String, bool)> = if perms.admin {
        HashMap::new()
    } else {
        sqlx::query_as::<_, (Uuid, String, bool)>(
            "SELECT pp.page_id, pp.role, pp.inherit FROM page_permissions pp JOIN pages p ON p.id = pp.page_id
             WHERE p.workspace_id = $1 AND pp.user_id = $2",
        )
        .bind(workspace_id)
        .bind(user.id)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|(id, r, i)| (id, (r, i)))
        .collect()
    };
    let parents: HashMap<Uuid, Option<Uuid>> = tree.into_iter().collect();
    let mut out = HashMap::with_capacity(parents.len());
    for &id in parents.keys() {
        // Remonte la chaîne jusqu'à trouver une permission explicite.
        let mut cur = Some(id);
        let mut found: Option<&str> = None;
        let mut guard = 0;
        while let Some(c) = cur {
            // Permission sur la page elle-même, ou héritée d'un ancêtre « sous-pages comprises ».
            if let Some((r, inherit)) = overrides.get(&c) {
                if c == id || *inherit {
                    found = Some(r.as_str());
                    break;
                }
            }
            cur = parents.get(&c).copied().flatten();
            guard += 1;
            if guard > 64 {
                break;
            }
        }
        out.insert(id, effective_page_access(&perms, found));
    }
    Ok(out)
}

/// Pages où la personne peut créer des sous-pages grâce à un partage « sous-pages permises »
/// (invités, rôles sans création de documents) : la permission la plus proche (la page ou un
/// ancêtre « sous-pages comprises ») est `editor` avec l'option.
pub async fn subpage_grants(pool: &PgPool, user_id: Uuid, workspace_id: Uuid) -> ApiResult<std::collections::HashSet<Uuid>> {
    let grants: Vec<(Uuid, String, bool, bool)> = sqlx::query_as(
        "SELECT pp.page_id, pp.role, pp.inherit, pp.allow_subpages FROM page_permissions pp JOIN pages p ON p.id = pp.page_id
         WHERE p.workspace_id = $1 AND pp.user_id = $2",
    )
    .bind(workspace_id)
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    let mut out = std::collections::HashSet::new();
    if !grants.iter().any(|g| g.3) {
        return Ok(out);
    }
    let overrides: HashMap<Uuid, (String, bool, bool)> = grants.into_iter().map(|(id, r, i, s)| (id, (r, i, s))).collect();
    let parents: HashMap<Uuid, Option<Uuid>> = sqlx::query_as::<_, (Uuid, Option<Uuid>)>("SELECT id, parent_id FROM pages WHERE workspace_id = $1")
        .bind(workspace_id)
        .fetch_all(pool)
        .await?
        .into_iter()
        .collect();
    for &id in parents.keys() {
        let mut cur = Some(id);
        let mut guard = 0;
        while let Some(c) = cur {
            if let Some((role, inherit, subpages)) = overrides.get(&c) {
                if c == id || *inherit {
                    if role == "editor" && *subpages {
                        out.insert(id);
                    }
                    break;
                }
            }
            cur = parents.get(&c).copied().flatten();
            guard += 1;
            if guard > 64 {
                break;
            }
        }
    }
    Ok(out)
}

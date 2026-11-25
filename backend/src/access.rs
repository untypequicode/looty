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

/// Marque une extraction facultative : l'absence de connexion n'y est pas un refus à journaliser.
#[derive(Clone, Copy)]
struct QuietAuth;

/// `Option<AuthUser>` : utilisateur connecté s'il y en a un (pages publiques, versions…).
impl<S> OptionalFromRequestParts<S> for AuthUser
where
    S: Send + Sync,
    PgPool: FromRef<S>,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Option<Self>, Self::Rejection> {
        parts.extensions.insert(QuietAuth);
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
        let path = parts.uri.path().to_string();
        let quiet = parts.extensions.get::<QuietAuth>().is_some();

        // 1. Jeton API
        if let Some(auth) = parts.headers.get(AUTHORIZATION).and_then(|h| h.to_str().ok()) {
            if let Some(raw) = auth.strip_prefix("Bearer ") {
                let row: Option<(Uuid, bool, bool)> = sqlx::query_as(
                    "UPDATE api_tokens t SET last_used_at = NOW()
                     FROM users u
                     WHERE t.token_hash = $1 AND t.scope = 'api' AND u.id = t.user_id
                       AND (t.expires_at IS NULL OR t.expires_at > NOW())
                     RETURNING u.id, u.is_super_admin, u.is_suspended",
                )
                .bind(hash_token(raw.trim()))
                .fetch_optional(&pool)
                .await?;
                return match row {
                    Some((id, is_super_admin, false)) => Ok(AuthUser { id, is_super_admin }),
                    Some(_) => Err(refuse("bearer_user_suspended", &path, None, quiet)),
                    None => Err(refuse("bearer_unknown_or_expired", &path, None, quiet)),
                };
            }
        }

        // 2. Session cookie
        let session = Session::from_request_parts(parts, state)
            .await
            .map_err(|_| refuse("session_extractor_failed", &path, None, quiet))?;
        let sid = session.id();
        let has_cookie = parts.headers.get(axum::http::header::COOKIE).is_some();
        // Erreur de lecture de la session (base indisponible…) : panne passagère (5xx), jamais un 401
        // qui passerait pour une session expirée (le client de collaboration abandonnerait).
        let user_id: Uuid = match session
            .get(SESSION_USER_KEY)
            .await
            .map_err(|e| ApiError::Internal(format!("session read: {e}")))?
        {
            Some(id) => id,
            None => {
                // Sans identifiant de session, le cookie est absent ou illisible ; avec, la session n'existe
                // plus en base (expirée, supprimée, autre base) ou ne contient pas d'utilisateur.
                let branch = match (sid, has_cookie) {
                    (None, false) => "no_cookie_header",
                    (None, true) => "no_session_cookie",
                    (Some(_), _) => "no_user_in_session",
                };
                return Err(refuse(branch, &path, sid, quiet));
            }
        };

        let row: Option<(bool, bool)> =
            sqlx::query_as("SELECT is_super_admin, is_suspended FROM users WHERE id = $1")
                .bind(user_id)
                .fetch_optional(&pool)
                .await?;
        match row {
            Some((is_super_admin, false)) => Ok(AuthUser { id: user_id, is_super_admin }),
            other => {
                // Utilisateur supprimé ou suspendu : on invalide la session.
                let _ = session.flush().await;
                let branch = if other.is_none() { "user_missing" } else { "user_suspended" };
                Err(refuse(branch, &path, sid, quiet))
            }
        }
    }
}

/// Préfixe court du hachage d'un identifiant de session (jamais l'identifiant lui-même).
fn session_tag(sid: Option<tower_sessions::session::Id>) -> String {
    match sid {
        Some(id) => hex::encode(Sha256::digest(id.to_string().as_bytes()))[..8].to_string(),
        None => "-".to_string(),
    }
}

/// Journalise un refus 401 (branche, route, hachage court de la session, conteneur qui répond) et
/// renvoie l'erreur avec la même raison dans le corps JSON.
fn refuse(branch: &'static str, path: &str, sid: Option<tower_sessions::session::Id>, quiet: bool) -> ApiError {
    if quiet {
        return ApiError::UnauthorizedReason(branch);
    }
    // Identifie le conteneur qui répond (utile si plusieurs backends sont joignables sous le même nom).
    static INSTANCE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let instance = INSTANCE.get_or_init(|| std::fs::read_to_string("/etc/hostname").unwrap_or_default().trim().to_string());
    tracing::warn!(branch, path, session_sha = %session_tag(sid), instance = %instance, "401 unauthorized");
    ApiError::UnauthorizedReason(branch)
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
    /// Créer, modifier et supprimer les groupes de membres.
    pub groups: bool,
    /// Publier, modifier et supprimer les annonces de l'espace.
    pub announcements: bool,
}

impl Perms {
    pub fn all() -> Perms {
        Perms { docs: level::EDIT, boards: level::MANAGE, calendar: level::EDIT, invite: true, share: true, admin: true, groups: true, announcements: true }
    }
    /// Permissions par défaut d'un préréglage (si le rôle n'existe pas en base).
    pub fn preset(role: Role) -> Perms {
        match role {
            Role::Owner | Role::Admin => Perms::all(),
            Role::Member => Perms { docs: level::EDIT, boards: level::MANAGE, calendar: level::EDIT, invite: false, share: true, admin: false, groups: false, announcements: false },
            Role::Viewer => Perms { docs: level::COMMENT, boards: level::COMMENT, calendar: level::READ, invite: false, share: false, admin: false, groups: false, announcements: false },
            Role::Guest => Perms { docs: level::NONE, boards: level::NONE, calendar: level::NONE, invite: false, share: false, admin: false, groups: false, announcements: false },
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
            groups: b("groups", base.groups),
            announcements: b("announcements", base.announcements),
        }
    }
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "docs": level::name(self.docs), "boards": level::name(self.boards), "calendar": level::name(self.calendar),
            "invite": self.invite, "share": self.share, "admin": self.admin, "groups": self.groups, "announcements": self.announcements,
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
          WHERE p.workspace_id = $1 AND pp.user_id = $2 AND pp.role NOT IN ('deny', 'blocked'))",
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

/// Écriture dans au moins un domaine (permissions effectives du rôle, pas son nom).
pub async fn require_writer(pool: &PgPool, user: &AuthUser, workspace_id: Uuid) -> ApiResult<Role> {
    let (role, perms) = member_perms(pool, user.id, workspace_id).await?.ok_or(ApiError::NotFound)?;
    if !perms.base_role().can_write() {
        return Err(ApiError::Forbidden);
    }
    Ok(role)
}

/// Administration de l'espace : le droit « admin » du rôle (un préréglage Admin auquel le
/// propriétaire l'a retiré ne le donne plus).
pub async fn require_ws_admin(pool: &PgPool, user: &AuthUser, workspace_id: Uuid) -> ApiResult<Role> {
    let (role, perms) = member_perms(pool, user.id, workspace_id).await?.ok_or(ApiError::NotFound)?;
    if !perms.admin {
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

/// Vue grossière de l'accès à une page (lecture / écriture) ; le niveau précis (`level`) vient
/// de `page_level` : lecture < commentaire < modification < gestion (`level::MANAGE`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageAccess {
    None,
    Read,
    Write,
}

/// Niveau d'accès (échelle `level`) -> vue grossière.
pub fn access_of(lv: u8) -> PageAccess {
    match lv {
        level::NONE => PageAccess::None,
        l if l >= level::EDIT => PageAccess::Write,
        _ => PageAccess::Read,
    }
}

/// Partage d'une page pour une personne : un niveau ou un refus explicite.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grant {
    Deny,
    Level(u8),
}

impl Grant {
    /// Valeur stockée dans `page_permissions.role`. Les anciennes valeurs (`viewer`, `editor`,
    /// `blocked`) restent comprises : `viewer` donne le commentaire, comme avant les niveaux.
    pub fn parse(role: &str) -> Option<Grant> {
        Some(match role {
            "read" => Grant::Level(level::READ),
            "comment" | "viewer" => Grant::Level(level::COMMENT),
            "edit" | "editor" => Grant::Level(level::EDIT),
            "manage" => Grant::Level(level::MANAGE),
            "deny" | "blocked" => Grant::Deny,
            _ => return None,
        })
    }
    pub fn name(&self) -> &'static str {
        match self {
            Grant::Deny => "deny",
            Grant::Level(l) => match *l {
                level::READ => "read",
                level::COMMENT => "comment",
                level::EDIT => "edit",
                _ => "manage",
            },
        }
    }
    /// Nom historique, pour les clients qui ne connaissent que `viewer` / `editor` / `blocked`.
    pub fn legacy_name(&self) -> &'static str {
        match self {
            Grant::Deny => "blocked",
            Grant::Level(l) if *l >= level::EDIT => "editor",
            Grant::Level(_) => "viewer",
        }
    }
}

/// Fragment SQL : partages qui donnent un accès (tout sauf le refus).
pub const GRANTING_ROLES: &str = "NOT IN ('deny', 'blocked')";

/// Niveau que donne le rôle de l'espace sur les documents (sans partage particulier).
fn role_level(perms: &Perms) -> u8 {
    match perms.docs {
        level::NONE => level::NONE,
        level::READ => level::READ,
        l if l >= level::EDIT => level::EDIT,
        _ => level::COMMENT,
    }
}

/// LA règle d'accès à une page, utilisée par la résolution d'une page (`page_level`), par celle
/// de tout l'espace (`visible_levels`) et par le temps réel : `perms` (None : personne extérieure
/// à l'espace) et le partage le plus proche (`grant`) donnent le niveau.
///
/// - un administrateur de l'espace garde la gestion de toutes les pages ;
/// - sinon le partage le plus proche remplace le rôle (refus : aucun accès, pour les membres comme
///   pour les personnes extérieures) ;
/// - sans partage, le rôle s'applique ; une personne extérieure n'a rien.
pub fn resolve_level(perms: Option<&Perms>, grant: Option<Grant>) -> u8 {
    if perms.is_some_and(|p| p.admin) {
        return level::MANAGE;
    }
    match grant {
        Some(Grant::Deny) => level::NONE,
        Some(Grant::Level(l)) => l,
        None => perms.map(role_level).unwrap_or(level::NONE),
    }
}

/// Partage le plus proche de `start` : sur la page elle-même, ou sur un ancêtre « sous-pages
/// comprises ». `grants` : page -> (valeur, sous-pages comprises). Valeur inconnue : refus.
pub fn nearest_grant(start: Uuid, parents: &HashMap<Uuid, Option<Uuid>>, grants: &HashMap<Uuid, (String, bool)>) -> Option<Grant> {
    let mut cur = Some(start);
    let mut guard = 0;
    while let Some(c) = cur {
        if let Some((role, inherit)) = grants.get(&c) {
            if c == start || *inherit {
                return Some(Grant::parse(role).unwrap_or(Grant::Deny));
            }
        }
        cur = parents.get(&c).copied().flatten();
        guard += 1;
        if guard > 64 {
            break;
        }
    }
    None
}

pub async fn page_workspace(pool: &PgPool, page_id: Uuid) -> ApiResult<Uuid> {
    sqlx::query_scalar("SELECT workspace_id FROM pages WHERE id = $1")
        .bind(page_id)
        .fetch_optional(pool)
        .await?
        .ok_or(ApiError::NotFound)
}

/// Compte suspendu : plus aucun partage ne joue (la résolution ne passe jamais par la session).
async fn is_suspended(pool: &PgPool, user_id: Uuid) -> ApiResult<bool> {
    Ok(sqlx::query_scalar("SELECT COALESCE(is_suspended, FALSE) FROM users WHERE id = $1").bind(user_id).fetch_optional(pool).await?.unwrap_or(true))
}

/// Chaîne d'ancêtres d'une page (elle comprise) : (page, parent).
async fn ancestors(pool: &PgPool, page_id: Uuid) -> ApiResult<Vec<(Uuid, Option<Uuid>)>> {
    Ok(sqlx::query_as(
        "WITH RECURSIVE anc AS (
            SELECT id, parent_id, 0 AS depth FROM pages WHERE id = $1
            UNION ALL
            SELECT p.id, p.parent_id, anc.depth + 1 FROM pages p JOIN anc ON p.id = anc.parent_id
            WHERE anc.depth < 64
         )
         SELECT id, parent_id FROM anc",
    )
    .bind(page_id)
    .fetch_all(pool)
    .await?)
}

/// Partages d'une personne sur un ensemble de pages : page -> (valeur, sous-pages comprises).
async fn grants_on(pool: &PgPool, user_id: Uuid, pages: &[Uuid]) -> ApiResult<HashMap<Uuid, (String, bool)>> {
    Ok(sqlx::query_as::<_, (Uuid, String, bool)>("SELECT page_id, role, inherit FROM page_permissions WHERE user_id = $1 AND page_id = ANY($2)")
        .bind(user_id)
        .bind(pages)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|(p, r, i)| (p, (r, i)))
        .collect())
}

/// Niveau d'accès (`level`) d'une personne à une page : la résolution unique d'une page.
/// Ne contrôle rien d'autre que le niveau ; erreur seulement si la page n'existe pas.
pub async fn page_level(pool: &PgPool, user_id: Uuid, page_id: Uuid) -> ApiResult<(Uuid, Role, u8)> {
    let workspace_id = page_workspace(pool, page_id).await?;
    if is_suspended(pool, user_id).await? {
        return Ok((workspace_id, Role::Guest, level::NONE));
    }
    // Page personnelle (GLOSSARY) : à son propriétaire seulement, administrateurs compris.
    let owner: Option<Uuid> = sqlx::query_scalar("SELECT private_to FROM pages_all WHERE id = $1").bind(page_id).fetch_one(pool).await?;
    let member = member_perms(pool, user_id, workspace_id).await?;
    if let Some(owner) = owner {
        return Ok(match member {
            Some((role, _)) if owner == user_id => (workspace_id, role, level::EDIT),
            Some((role, _)) => (workspace_id, role, level::NONE),
            None => (workspace_id, Role::Guest, level::NONE),
        });
    }
    let chain = ancestors(pool, page_id).await?;
    let ids: Vec<Uuid> = chain.iter().map(|(id, _)| *id).collect();
    let parents: HashMap<Uuid, Option<Uuid>> = chain.into_iter().collect();
    let grants = grants_on(pool, user_id, &ids).await?;
    let grant = nearest_grant(page_id, &parents, &grants);
    let (role, perms) = match member {
        Some((r, p)) => (r, Some(p)),
        None => (Role::Guest, None),
    };
    Ok((workspace_id, role, resolve_level(perms.as_ref(), grant)))
}

/// Accès effectif d'un utilisateur à une page, en niveau précis. Une personne extérieure sans
/// accès ne sait pas que la page existe (NotFound).
pub async fn page_access_level(pool: &PgPool, user: &AuthUser, page_id: Uuid) -> ApiResult<(Uuid, Role, u8)> {
    let (ws, role, lv) = page_level(pool, user.id, page_id).await?;
    if role == Role::Guest && lv == level::NONE {
        return Err(ApiError::NotFound);
    }
    Ok((ws, role, lv))
}

/// Accès effectif d'un utilisateur à une page (vue lecture / écriture).
pub async fn page_access(pool: &PgPool, user: &AuthUser, page_id: Uuid) -> ApiResult<(Uuid, Role, PageAccess)> {
    let (ws, role, lv) = page_access_level(pool, user, page_id).await?;
    Ok((ws, role, access_of(lv)))
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

/// Exige au moins le niveau `min` sur la page : NotFound sans lecture, Forbidden en dessous de `min`.
pub async fn require_page_level(pool: &PgPool, user: &AuthUser, page_id: Uuid, min: u8) -> ApiResult<(Uuid, Role, u8)> {
    let (ws, role, lv) = page_access_level(pool, user, page_id).await?;
    if lv == level::NONE {
        return Err(ApiError::NotFound);
    }
    if lv < min {
        return Err(ApiError::Forbidden);
    }
    Ok((ws, role, lv))
}

/// Gérer les partages de la page : administrateur de l'espace ou niveau « gestion » sur la page.
pub async fn require_page_manage(pool: &PgPool, user: &AuthUser, page_id: Uuid) -> ApiResult<(Uuid, Role)> {
    let (ws, role, _) = require_page_level(pool, user, page_id, level::MANAGE).await?;
    Ok((ws, role))
}

/// Mettre une page à la corbeille : modifier, et — pour qui n'a ce droit que par le partage de
/// cette page même (invité, rôle sans création de documents) — la gérer.
pub async fn require_page_delete(pool: &PgPool, user: &AuthUser, page_id: Uuid) -> ApiResult<(Uuid, Role)> {
    let (ws, role, lv) = require_page_level(pool, user, page_id, level::EDIT).await?;
    if lv < level::MANAGE {
        let role_edits = member_perms(pool, user.id, ws).await?.is_some_and(|(_, p)| p.docs >= level::EDIT);
        let own_grant: bool = sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT EXISTS(SELECT 1 FROM page_permissions WHERE page_id = $1 AND user_id = $2 AND role {GRANTING_ROLES})")))
            .bind(page_id)
            .bind(user.id)
            .fetch_one(pool)
            .await?;
        if !role_edits && own_grant {
            return Err(ApiError::Forbidden);
        }
    }
    Ok((ws, role))
}

/// Peut-on commenter (et résoudre les fils) sur une page ? Même règle pour tout le monde.
pub fn can_comment_level(lv: u8) -> bool {
    lv >= level::COMMENT
}

/// Niveau d'accès à toutes les pages d'un espace, en une passe : la même règle que `page_level`
/// (un test de propriété vérifie l'égalité) pour les listes, la recherche, l'accueil…
pub async fn visible_levels(pool: &PgPool, user_id: Uuid, workspace_id: Uuid) -> ApiResult<HashMap<Uuid, u8>> {
    let member = member_perms(pool, user_id, workspace_id).await?.map(|(_, p)| p);
    let tree: Vec<(Uuid, Option<Uuid>, Option<Uuid>)> = sqlx::query_as("SELECT id, parent_id, private_to FROM pages WHERE workspace_id = $1")
        .bind(workspace_id)
        .fetch_all(pool)
        .await?;
    if is_suspended(pool, user_id).await? {
        return Ok(tree.into_iter().map(|(id, _, _)| (id, level::NONE)).collect());
    }
    // Pages personnelles : invisibles aux autres, modifiables par leur propriétaire.
    let private: HashMap<Uuid, Uuid> = tree.iter().filter_map(|(id, _, o)| o.map(|o| (*id, o))).collect();
    let parents: HashMap<Uuid, Option<Uuid>> = tree.into_iter().map(|(id, p, _)| (id, p)).collect();
    let grants: HashMap<Uuid, (String, bool)> = if member.as_ref().is_some_and(|p| p.admin) {
        HashMap::new()
    } else {
        sqlx::query_as::<_, (Uuid, String, bool)>(
            "SELECT pp.page_id, pp.role, pp.inherit FROM page_permissions pp JOIN pages p ON p.id = pp.page_id
             WHERE p.workspace_id = $1 AND pp.user_id = $2",
        )
        .bind(workspace_id)
        .bind(user_id)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|(id, r, i)| (id, (r, i)))
        .collect()
    };
    let mut out = HashMap::with_capacity(parents.len());
    for &id in parents.keys() {
        let lv = match private.get(&id) {
            Some(owner) if *owner == user_id && member.is_some() => level::EDIT,
            Some(_) => level::NONE,
            None => resolve_level(member.as_ref(), nearest_grant(id, &parents, &grants)),
        };
        out.insert(id, lv);
    }
    Ok(out)
}

/// Calcule l'accès de l'utilisateur à toutes les pages d'un workspace en une passe
/// (utilisé pour filtrer les listes, la recherche, l'accueil...).
pub async fn visible_pages(pool: &PgPool, user: &AuthUser, workspace_id: Uuid, _role: Role) -> ApiResult<HashMap<Uuid, PageAccess>> {
    Ok(visible_levels(pool, user.id, workspace_id).await?.into_iter().map(|(id, lv)| (id, access_of(lv))).collect())
}

/// Pages où la personne peut créer des sous-pages grâce à un partage « sous-pages permises »
/// (invités, rôles sans création de documents) : la permission la plus proche (la page ou un
/// ancêtre « sous-pages comprises ») est de niveau modification avec l'option.
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
    let subpages: HashMap<Uuid, bool> = grants.iter().map(|(id, _, _, s)| (*id, *s)).collect();
    let overrides: HashMap<Uuid, (String, bool)> = grants.into_iter().map(|(id, r, i, _)| (id, (r, i))).collect();
    let parents: HashMap<Uuid, Option<Uuid>> = sqlx::query_as::<_, (Uuid, Option<Uuid>)>("SELECT id, parent_id FROM pages WHERE workspace_id = $1")
        .bind(workspace_id)
        .fetch_all(pool)
        .await?
        .into_iter()
        .collect();
    for &id in parents.keys() {
        // Le partage qui s'applique à `id` : le plus proche, puis son origine pour l'option.
        let mut cur = Some(id);
        let mut guard = 0;
        while let Some(c) = cur {
            if let Some((role, inherit)) = overrides.get(&c) {
                if c == id || *inherit {
                    if matches!(Grant::parse(role), Some(Grant::Level(l)) if l >= level::EDIT) && subpages.get(&c).copied().unwrap_or(false) {
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

/// Un changement d'accès à une page pour une personne (déplacement).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AccessChange {
    pub user_id: Uuid,
    pub name: String,
    /// Niveau avant / après : `none`, `read`, `comment`, `edit` ou `manage`.
    pub before: String,
    pub after: String,
}

/// Qui gagne ou perd l'accès à la page `page_id` si elle est rangée sous `new_parent` (None : à
/// la racine) ? Seules comptent les personnes qui ont un partage sur la page ou ses ancêtres
/// (avant ou après) : les autres tiennent leur accès de leur rôle, que le déplacement ne change pas.
/// Le niveau est celui de la page déplacée ; les partages posés sur ses sous-pages priment toujours.
pub async fn move_access_changes(pool: &PgPool, page_id: Uuid, new_parent: Option<Uuid>) -> ApiResult<Vec<AccessChange>> {
    let workspace_id = page_workspace(pool, page_id).await?;
    let old_chain = ancestors(pool, page_id).await?;
    let mut new_chain: Vec<(Uuid, Option<Uuid>)> = match new_parent {
        Some(p) => ancestors(pool, p).await?,
        None => Vec::new(),
    };
    new_chain.push((page_id, new_parent));
    let ids: Vec<Uuid> = old_chain.iter().chain(new_chain.iter()).map(|(id, _)| *id).collect();
    let candidates: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT DISTINCT u.id, u.name FROM page_permissions pp JOIN users u ON u.id = pp.user_id WHERE pp.page_id = ANY($1) ORDER BY u.name, u.id",
    )
    .bind(&ids)
    .fetch_all(pool)
    .await?;
    let old_parents: HashMap<Uuid, Option<Uuid>> = old_chain.into_iter().collect();
    let new_parents: HashMap<Uuid, Option<Uuid>> = new_chain.into_iter().collect();
    let mut out = Vec::new();
    for (uid, name) in candidates {
        let perms = member_perms(pool, uid, workspace_id).await?.map(|(_, p)| p);
        let grants = grants_on(pool, uid, &ids).await?;
        let before = resolve_level(perms.as_ref(), nearest_grant(page_id, &old_parents, &grants));
        let after = resolve_level(perms.as_ref(), nearest_grant(page_id, &new_parents, &grants));
        if before != after {
            out.push(AccessChange { user_id: uid, name, before: level::name(before).into(), after: level::name(after).into() });
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Éléments personnels (GLOSSARY) : `private_to` réserve l'élément à une personne, même
// face aux administrateurs. Pour un autre membre, l'élément n'existe pas (NotFound).
// ---------------------------------------------------------------------------

fn hidden(owner: Option<Uuid>, user: &AuthUser) -> bool {
    owner.is_some_and(|o| o != user.id)
}

/// `task_ctx`, en vérifiant que la tâche (ou son tableau) n'est pas personnelle à quelqu'un d'autre.
pub async fn task_ctx_for(pool: &PgPool, user: &AuthUser, task_id: Uuid) -> ApiResult<TaskCtx> {
    let ctx = task_ctx(pool, task_id).await?;
    let (task_owner, board_owner): (Option<Uuid>, Option<Uuid>) =
        sqlx::query_as("SELECT t.private_to, b.private_to FROM tasks t JOIN boards b ON b.id = $2 WHERE t.id = $1")
            .bind(task_id)
            .bind(ctx.board_id)
            .fetch_one(pool)
            .await?;
    if hidden(task_owner, user) || hidden(board_owner, user) {
        return Err(ApiError::NotFound);
    }
    Ok(ctx)
}

/// Le tableau n'est pas personnel à quelqu'un d'autre.
pub async fn check_board(pool: &PgPool, user: &AuthUser, board_id: Uuid) -> ApiResult<()> {
    let owner: Option<Option<Uuid>> = sqlx::query_scalar("SELECT private_to FROM boards WHERE id = $1").bind(board_id).fetch_optional(pool).await?;
    match owner {
        Some(o) if !hidden(o, user) => Ok(()),
        _ => Err(ApiError::NotFound),
    }
}

pub async fn board_workspace_for(pool: &PgPool, user: &AuthUser, board_id: Uuid) -> ApiResult<Uuid> {
    let ws = board_workspace(pool, board_id).await?;
    check_board(pool, user, board_id).await?;
    Ok(ws)
}

pub async fn column_ctx_for(pool: &PgPool, user: &AuthUser, column_id: Uuid) -> ApiResult<ColumnCtx> {
    let ctx = column_ctx(pool, column_id).await?;
    check_board(pool, user, ctx.board_id).await?;
    Ok(ctx)
}

pub async fn label_ctx_for(pool: &PgPool, user: &AuthUser, label_id: Uuid) -> ApiResult<(Uuid, Uuid)> {
    let ctx = label_ctx(pool, label_id).await?;
    check_board(pool, user, ctx.1).await?;
    Ok(ctx)
}

#[cfg(test)]
mod auth_tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use serde_json::Value;
    use time::{Duration, OffsetDateTime};
    use tower::ServiceExt;
    use tower_sessions::session::{Id, Record};
    use tower_sessions::SessionStore;

    async fn setup(pool: &PgPool) -> (axum::Router, crate::session_store::PgSessionStore) {
        let store = crate::session_store::PgSessionStore::new(pool.clone());
        store.migrate().await.unwrap();
        (crate::app(crate::state::AppState::new(pool.clone(), "http://localhost".into())), store)
    }

    async fn session_for(store: &crate::session_store::PgSessionStore, user: Option<Uuid>) -> String {
        let mut data = HashMap::new();
        if let Some(u) = user {
            data.insert(SESSION_USER_KEY.to_string(), serde_json::json!(u));
        }
        let mut record = Record { id: Id::default(), data, expiry_date: OffsetDateTime::now_utc() + Duration::days(1) };
        store.create(&mut record).await.unwrap();
        record.id.to_string()
    }

    async fn get(app: &axum::Router, path: &str, headers: &[(&str, String)]) -> (u16, Value) {
        let mut req = Request::builder().uri(path);
        for (k, v) in headers {
            req = req.header(*k, v);
        }
        let res = app.clone().oneshot(req.body(Body::empty()).unwrap()).await.unwrap();
        let status = res.status().as_u16();
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
    }

    #[sqlx::test]
    async fn a_bare_cookie_request_stays_authenticated_beside_concurrent_browser_requests(pool: PgPool) {
        let (app, store) = setup(&pool).await;
        let user: Uuid = sqlx::query_scalar("INSERT INTO users (email, name) VALUES ('ana@example.org', 'ana') RETURNING id").fetch_one(&pool).await.unwrap();
        let sid = session_for(&store, Some(user)).await;
        // La collaboration ne transmet que l'en-tête Cookie ; le navigateur ajoute Origin, Referer…
        let bare = vec![("cookie", format!("theme=dark; looty_session={sid}; other=1"))];
        let browser = vec![
            ("cookie", format!("looty_session={sid}")),
            ("origin", "http://localhost".to_string()),
            ("x-forwarded-for", "203.0.113.7".to_string()),
            ("x-forwarded-proto", "https".to_string()),
        ];
        let mut tasks = Vec::new();
        for i in 0..40 {
            let (app, h) = (app.clone(), if i % 2 == 0 { bare.clone() } else { browser.clone() });
            let path = if i % 4 == 0 { "/api/auth/me" } else { "/api/workspaces" };
            tasks.push(tokio::spawn(async move { get(&app, path, &h).await }));
        }
        for t in tasks {
            let (status, body) = t.await.unwrap();
            assert_eq!(status, 200, "{body}");
        }
    }

    #[sqlx::test]
    async fn each_refusal_names_its_branch_in_the_body(pool: PgPool) {
        let (app, store) = setup(&pool).await;
        let (status, body) = get(&app, "/api/workspaces", &[]).await;
        assert_eq!((status, body["error"].as_str(), body["reason"].as_str()), (401, Some("unauthorized"), Some("no_cookie_header")));
        let (_, body) = get(&app, "/api/workspaces", &[("cookie", "theme=dark".into())]).await;
        assert_eq!(body["reason"], "no_session_cookie");
        // Session inconnue de la base (expirée, supprimée, autre base).
        let unknown = Id::default().to_string();
        let (_, body) = get(&app, "/api/workspaces", &[("cookie", format!("looty_session={unknown}"))]).await;
        assert_eq!(body["reason"], "no_user_in_session");
        let empty = session_for(&store, None).await;
        let (_, body) = get(&app, "/api/workspaces", &[("cookie", format!("looty_session={empty}"))]).await;
        assert_eq!(body["reason"], "no_user_in_session");
        // Utilisateur supprimé : la session est invalidée.
        let gone = session_for(&store, Some(Uuid::new_v4())).await;
        let (_, body) = get(&app, "/api/workspaces", &[("cookie", format!("looty_session={gone}"))]).await;
        assert_eq!(body["reason"], "user_missing");
        let (_, body) = get(&app, "/api/workspaces", &[("cookie", format!("looty_session={gone}"))]).await;
        assert_eq!(body["reason"], "no_user_in_session");
        // Jeton inconnu.
        let (status, body) = get(&app, "/api/workspaces", &[("authorization", "Bearer looty_unknown".into())]).await;
        assert_eq!((status, body["reason"].as_str()), (401, Some("bearer_unknown_or_expired")));
    }
}

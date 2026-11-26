//! Accès aux fichiers envoyés (ADR 0022, phase 5, G1) : `/uploads/{nom}` n'est plus une URL capacité.
//!
//! Qui peut lire un fichier (dans l'ordre, la première règle qui passe) :
//! 1. mode de compatibilité `LOOTY_UPLOADS_PUBLIC=1` (ancien comportement, sans connexion) ;
//! 2. fichier public (`kind = 'public'` : avatars, favicon) : tout le monde ;
//! 3. lien public d'une page (`?share=<jeton>`) : le fichier doit figurer dans cette page ;
//! 4. lien signé à courte durée (`?sig=<expiration>.<mac>`, voir `signed_link`) ;
//! 5. personne connectée : administrateur de l'instance, personne qui a envoyé le fichier, lecteur de la
//!    page liée (même résolution que le reste : `page_level`), lecteur de la tâche liée, membre de l'espace
//!    lié, lecteur d'une page qui cite le fichier (`upload_refs`, revérifiée, puis recherche dans les pages).
//!
//! Tout refus donne 404 à une personne connectée (le fichier n'existe pas pour elle) et 401 sinon.

use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

use crate::access::{level, member_perms, page_level, task_ctx_for, AuthUser};
use crate::error::{ApiError, ApiResult};

/// Durée de vie d'un lien signé.
pub const LINK_TTL_SECS: i64 = 600;
const MAC_CONTEXT: &str = "upload-link";
/// Pages examinées au plus lors d'une recherche de citation.
const SCAN_LIMIT: i64 = 40;

/// Lots de pages examinés au plus lors d'une recherche de citation (`SCAN_LIMIT` pages chacun).
const SCAN_ROUNDS: usize = 5;
/// Recherches de citation par personne et par minute.
const SCANS_PER_MINUTE: u32 = 120;
/// Un refus récent pour (personne, fichier) n'est pas recherché de nouveau avant ce délai.
#[cfg(not(test))]
const NEGATIVE_TTL: std::time::Duration = std::time::Duration::from_secs(10);
#[cfg(test)]
const NEGATIVE_TTL: std::time::Duration = std::time::Duration::ZERO;

type Limiter = std::sync::Mutex<(HashMap<Uuid, (std::time::Instant, u32)>, HashMap<(Uuid, String), std::time::Instant>)>;
static LIMITER: std::sync::LazyLock<Limiter> = std::sync::LazyLock::new(Default::default);

/// Mémorise un refus complet (voir `scan_allowed`).
fn scan_denied(user: Uuid, name: &str) {
    let mut g = LIMITER.lock().unwrap_or_else(|e| e.into_inner());
    g.1.insert((user, name.to_string()), std::time::Instant::now());
}

/// Autorise une recherche coûteuse : limite par personne, et pas deux fois de suite pour le même refus.
/// Un refus mémorisé reste refusé jusqu'à `NEGATIVE_TTL` (un accès accordé entre-temps passe par les
/// règles rapides au-dessus, jamais par cette recherche).
fn scan_allowed(user: Uuid, name: &str) -> bool {
    let now = std::time::Instant::now();
    let mut g = LIMITER.lock().unwrap_or_else(|e| e.into_inner());
    let (counts, negatives) = &mut *g;
    if negatives.len() > 5000 {
        negatives.retain(|_, t| now.duration_since(*t) < NEGATIVE_TTL);
    }
    if counts.len() > 5000 {
        counts.retain(|_, (t, _)| now.duration_since(*t) < std::time::Duration::from_secs(60));
    }
    if negatives.get(&(user, name.to_string())).is_some_and(|t| now.duration_since(*t) < NEGATIVE_TTL) {
        return false;
    }
    let e = counts.entry(user).or_insert((now, 0));
    if now.duration_since(e.0) >= std::time::Duration::from_secs(60) {
        *e = (now, 0);
    }
    if e.1 >= SCANS_PER_MINUTE {
        return false;
    }
    e.1 += 1;
    true
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// Lisible sans connexion et partageable par un cache.
    Public,
    /// Lisible par cette requête ; réponse privée, revalidée à chaque fois.
    Allowed,
    Denied,
}

/// Rattachement d'un fichier à l'envoi.
#[derive(Clone, Copy, Debug, Default)]
pub struct Binding {
    pub workspace_id: Option<Uuid>,
    pub page_id: Option<Uuid>,
    pub task_id: Option<Uuid>,
}

pub fn legacy_public() -> bool {
    std::env::var("LOOTY_UPLOADS_PUBLIC").is_ok_and(|v| matches!(v.trim(), "1" | "true" | "yes"))
}

/// Nom du fichier d'une URL (`/uploads/nom?x`), vérifié sûr.
pub fn name_of_url(url: &str) -> Option<&str> {
    let tail = url.rsplit_once("/uploads/")?.1;
    let name = tail.split(['?', '#']).next()?;
    crate::upload::is_safe_name(name).then_some(name)
}

pub async fn register(pool: &PgPool, name: &str, uploader: Uuid, kind: &str, b: Binding) -> ApiResult<()> {
    sqlx::query(
        "INSERT INTO upload_files (name, uploader_id, workspace_id, page_id, task_id, kind) VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (name) DO NOTHING",
    )
    .bind(name)
    .bind(uploader)
    .bind(b.workspace_id)
    .bind(b.page_id)
    .bind(b.task_id)
    .bind(kind)
    .execute(pool)
    .await?;
    Ok(())
}

/// Un fichier devient public (avatar, favicon) : seulement par la personne qui l'a envoyé (ou un fichier
/// d'avant la migration sans propriétaire, ou un administrateur d'instance).
async fn row_exists(pool: &PgPool, name: &str) -> ApiResult<bool> {
    Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM upload_files WHERE name = $1)").bind(name).fetch_one(pool).await?)
}

pub async fn mark_public(pool: &PgPool, user: &AuthUser, url: &str) -> ApiResult<()> {
    let Some(name) = name_of_url(url) else { return Ok(()) };
    // Seulement un fichier que la personne peut lire (l'envoyeur pour une migration : voir ADR 0022 § 8).
    if row_exists(pool, name).await? && decide(pool, Some(user), name, None, None).await? == Decision::Denied {
        return Err(ApiError::bad_request("invalid_avatar_url"));
    }
    let row: Option<(Option<Uuid>, String)> = sqlx::query_as("SELECT uploader_id, kind FROM upload_files WHERE name = $1").bind(name).fetch_optional(pool).await?;
    match row {
        Some((_, kind)) if kind == "public" => Ok(()),
        Some((Some(up), _)) if up != user.id && !user.is_super_admin => Err(ApiError::bad_request("invalid_avatar_url")),
        Some((_, kind)) if kind == "task" && !user.is_super_admin => Err(ApiError::bad_request("invalid_avatar_url")),
        Some(_) => {
            sqlx::query("UPDATE upload_files SET kind = 'public' WHERE name = $1").bind(name).execute(pool).await?;
            Ok(())
        }
        None => register(pool, name, user.id, "public", Binding::default()).await,
    }
}

fn mac_of(name: &str, exp: i64) -> String {
    crate::crypto::mac(MAC_CONTEXT, &format!("{name}|{exp}"))
}

/// `exp.mac` valable `LINK_TTL_SECS` pour ce fichier.
pub fn signed_link(name: &str) -> String {
    let exp = chrono::Utc::now().timestamp() + LINK_TTL_SECS;
    format!("{exp}.{}", mac_of(name, exp))
}

fn sig_valid(name: &str, sig: &str) -> bool {
    let Some((exp, mac)) = sig.split_once('.') else { return false };
    let Ok(exp) = exp.parse::<i64>() else { return false };
    exp >= chrono::Utc::now().timestamp() && constant_eq(mac_of(name, exp).as_bytes(), mac.as_bytes())
}

fn constant_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}

/// Le fichier figure dans la page publique de ce jeton (HTML ou icône).
async fn shared_page_has(pool: &PgPool, token: Uuid, name: &str) -> ApiResult<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM pages WHERE share_token = $1 AND is_public = true
                       AND (strpos(coalesce(content, ''), $2) > 0 OR strpos(coalesce(icon, ''), $2) > 0))",
    )
    .bind(token)
    .bind(format!("/uploads/{name}"))
    .fetch_one(pool)
    .await?)
}

async fn page_readable(pool: &PgPool, user: &AuthUser, page: Uuid) -> bool {
    matches!(page_level(pool, user.id, page).await, Ok((_, _, lv)) if lv > level::NONE)
}

async fn task_readable(pool: &PgPool, user: &AuthUser, task: Uuid) -> bool {
    let Ok(ctx) = task_ctx_for(pool, user, task).await else { return false };
    matches!(member_perms(pool, user.id, ctx.workspace_id).await, Ok(Some((_, p))) if p.boards >= level::READ)
}

async fn page_cites(pool: &PgPool, page: Uuid, name: &str) -> ApiResult<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM pages WHERE id = $1
                       AND (strpos(coalesce(content, ''), $2) > 0 OR strpos(coalesce(icon, ''), $2) > 0 OR position($3::bytea in yjs_state) > 0))",
    )
    .bind(page)
    .bind(format!("/uploads/{name}"))
    .bind(format!("/uploads/{name}").into_bytes())
    .fetch_one(pool)
    .await?)
}

pub async fn decide(pool: &PgPool, user: Option<&AuthUser>, name: &str, share: Option<Uuid>, sig: Option<&str>) -> ApiResult<Decision> {
    if !crate::upload::is_safe_name(name) {
        return Ok(Decision::Denied);
    }
    if legacy_public() {
        return Ok(Decision::Public);
    }
    let row: Option<(Option<Uuid>, Option<Uuid>, Option<Uuid>, Option<Uuid>, String)> =
        sqlx::query_as("SELECT uploader_id, workspace_id, page_id, task_id, kind FROM upload_files WHERE name = $1").bind(name).fetch_optional(pool).await?;
    if row.as_ref().is_some_and(|r| r.4 == "public") {
        return Ok(Decision::Public);
    }
    if let Some(token) = share {
        if shared_page_has(pool, token, name).await? {
            return Ok(Decision::Allowed);
        }
    }
    if sig.is_some_and(|s| sig_valid(name, s)) {
        return Ok(Decision::Allowed);
    }
    let Some(user) = user else { return Ok(Decision::Denied) };
    if user.is_super_admin {
        return Ok(Decision::Allowed);
    }
    if let Some((uploader, ws, page, task, _)) = row {
        if uploader == Some(user.id) {
            return Ok(Decision::Allowed);
        }
        if let Some(page) = page {
            if page_readable(pool, user, page).await {
                return Ok(Decision::Allowed);
            }
        }
        if let Some(task) = task {
            if task_readable(pool, user, task).await {
                return Ok(Decision::Allowed);
            }
        }
        // Espace sans page ni tâche : tout membre.
        if let (Some(ws), None, None) = (ws, page, task) {
            if member_perms(pool, user.id, ws).await?.is_some() {
                return Ok(Decision::Allowed);
            }
        }
    }
    // Pages qui citent le fichier : lien connu, revérifié (la page le contient toujours, la personne la lit).
    let known: Vec<Uuid> = sqlx::query_scalar("SELECT page_id FROM upload_refs WHERE name = $1 ORDER BY page_id LIMIT 200").bind(name).fetch_all(pool).await?;
    for p in &known {
        if page_cites(pool, *p, name).await? && page_readable(pool, user, *p).await {
            return Ok(Decision::Allowed);
        }
    }
    // Recherche dans les contenus (fichier cité depuis l'envoi) : bornée par personne, résultat négatif
    // mémorisé quelques secondes, jamais pour un fichier absent du disque (voir `serve_upload`).
    if !scan_allowed(user.id, name) {
        return Ok(Decision::Denied);
    }
    let needle = format!("/uploads/{name}");
    // Pages : par lots triés, sans les pages déjà vérifiées, jusqu'à en trouver une que la personne lit.
    let mut seen: Vec<Uuid> = known;
    let mut last: Option<Uuid> = None;
    for _ in 0..SCAN_ROUNDS {
        let found: Vec<Uuid> = sqlx::query_scalar(
            "SELECT id FROM pages WHERE (strpos(coalesce(content, ''), $1) > 0 OR strpos(coalesce(icon, ''), $1) > 0 OR position($2::bytea in yjs_state) > 0)
               AND id <> ALL($3) AND ($4::uuid IS NULL OR id > $4) ORDER BY id LIMIT $5",
        )
        .bind(&needle)
        .bind(needle.clone().into_bytes())
        .bind(&seen)
        .bind(last)
        .bind(SCAN_LIMIT)
        .fetch_all(pool)
        .await?;
        let full = found.len() as i64 == SCAN_LIMIT;
        for p in found {
            last = Some(p);
            seen.push(p);
            let _ = sqlx::query("INSERT INTO upload_refs (name, page_id) VALUES ($1, $2) ON CONFLICT DO NOTHING").bind(name).bind(p).execute(pool).await;
            if page_readable(pool, user, p).await {
                return Ok(Decision::Allowed);
            }
        }
        if !full {
            break;
        }
    }
    // Commentaires de pages et d'événements, annonces, bloc-notes personnel.
    let comments: Vec<(String, Uuid, Uuid)> = sqlx::query_as(
        "SELECT target_kind, target_id, workspace_id FROM item_comments WHERE strpos(content, $1) > 0 ORDER BY created_at LIMIT $2",
    )
    .bind(&needle)
    .bind(SCAN_LIMIT)
    .fetch_all(pool)
    .await?;
    for (kind, target, ws) in comments {
        let ok = if kind == "page" {
            page_readable(pool, user, target).await
        } else {
            matches!(member_perms(pool, user.id, ws).await, Ok(Some((_, p))) if p.calendar >= level::READ)
        };
        if ok {
            return Ok(Decision::Allowed);
        }
    }
    let spaces: Vec<Uuid> = sqlx::query_scalar("SELECT DISTINCT workspace_id FROM announcements WHERE strpos(body, $1) > 0 LIMIT $2").bind(&needle).bind(SCAN_LIMIT).fetch_all(pool).await?;
    for ws in spaces {
        if member_perms(pool, user.id, ws).await?.is_some() {
            return Ok(Decision::Allowed);
        }
    }
    let own_note: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM user_notepads WHERE user_id = $1 AND strpos(content, $2) > 0)").bind(user.id).bind(&needle).fetch_one(pool).await?;
    if own_note {
        return Ok(Decision::Allowed);
    }
    // Tâches : description, commentaires, pièces jointes.
    let tasks: Vec<Uuid> = sqlx::query_scalar(
        "SELECT t.id FROM tasks t WHERE strpos(coalesce(t.description, ''), $1) > 0
         UNION SELECT cm.task_id FROM task_comments cm WHERE strpos(coalesce(cm.content, ''), $1) > 0
         UNION SELECT ta.task_id FROM task_attachments ta WHERE ta.file_path = $2
         LIMIT $3",
    )
    .bind(&needle)
    .bind(format!("uploads/{name}"))
    .bind(SCAN_LIMIT)
    .fetch_all(pool)
    .await?;
    for t in tasks {
        if task_readable(pool, user, t).await {
            return Ok(Decision::Allowed);
        }
    }
    scan_denied(user.id, name);
    Ok(Decision::Denied)
}

/// Dans le HTML d'une page publique, `/uploads/x` devient `/uploads/x?share=<jeton>` : les images se
/// chargent sans compte, pour cette page seulement.
pub fn tag_public_links(html: &str, token: Uuid) -> String {
    const P: &str = "/uploads/";
    let mut out = String::with_capacity(html.len() + 64);
    let mut rest = html;
    while let Some(i) = rest.find(P) {
        let tail = &rest[i + P.len()..];
        let end = tail.find(|c: char| !(c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')).unwrap_or(tail.len());
        out.push_str(&rest[..i + P.len() + end]);
        let after = &tail[end..];
        if end == 0 {
            rest = after;
        } else if let Some(q) = after.strip_prefix('?') {
            // Déjà une requête : le jeton s'ajoute devant.
            out.push_str(&format!("?share={token}&"));
            rest = q;
        } else {
            out.push_str(&format!("?share={token}"));
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

/// Au démarrage : fichiers sur disque sans propriétaire connu ni citation (avant la migration ou envoyés
/// hors de l'application) : rattachés à personne (`unattributed`, lisibles par l'administrateur de
/// l'instance seulement). Renvoie leur nombre total ; il est journalisé et affiché dans le stockage.
pub async fn report_unattributed(pool: &PgPool) -> usize {
    let files = crate::storage::list_files();
    let known: std::collections::HashSet<String> = match sqlx::query_scalar::<_, String>("SELECT name FROM upload_files").fetch_all(pool).await {
        Ok(v) => v.into_iter().collect(),
        Err(e) => {
            tracing::warn!("uploads: cannot read upload_files: {e}");
            return 0;
        }
    };
    let mut added = 0usize;
    for f in files.iter().filter(|f| !f.name.starts_with('.') && !known.contains(&f.name)) {
        let cited: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM upload_refs WHERE name = $1)").bind(&f.name).fetch_one(pool).await.unwrap_or(false);
        if cited {
            continue;
        }
        if sqlx::query("INSERT INTO upload_files (name, kind) VALUES ($1, 'unattributed') ON CONFLICT DO NOTHING").bind(&f.name).execute(pool).await.is_ok() {
            added += 1;
        }
    }
    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM upload_files WHERE kind = 'unattributed'").fetch_one(pool).await.unwrap_or(0);
    if total > 0 {
        tracing::warn!("uploads: {total} file(s) without owner or reference ({added} new), readable by instance admins only; count in the storage overview");
    }
    total as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_links() {
        let t = Uuid::nil();
        assert_eq!(tag_public_links(r#"<img src="/uploads/a-1.png"> /uploads/ x"#, t), format!(r#"<img src="/uploads/a-1.png?share={t}"> /uploads/ x"#));
        assert_eq!(tag_public_links(r#"<img src="/uploads/a.png?w=2">"#, t), format!(r#"<img src="/uploads/a.png?share={t}&w=2">"#));
    }

    #[test]
    fn link_roundtrip() {
        let s = signed_link("a.png");
        assert!(sig_valid("a.png", &s));
        assert!(!sig_valid("b.png", &s));
        assert!(!sig_valid("a.png", "1.deadbeef"));
    }

    #[test]
    fn scans_are_limited_per_person() {
        let u = Uuid::new_v4();
        let allowed = (0..SCANS_PER_MINUTE + 20).filter(|i| scan_allowed(u, &format!("n{i}.png"))).count();
        assert_eq!(allowed as u32, SCANS_PER_MINUTE);
        assert!(scan_allowed(Uuid::new_v4(), "n0.png"), "another person is not affected");
    }

    #[test]
    fn names() {
        assert_eq!(name_of_url("/uploads/a.png?x=1"), Some("a.png"));
        assert_eq!(name_of_url("https://h/uploads/a.png"), Some("a.png"));
        assert_eq!(name_of_url("/uploads/../x"), None);
    }
}

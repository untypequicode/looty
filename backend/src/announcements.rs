//! Annonces de l'espace (GLOSSARY) : une information que les responsables passent à tous
//! les membres. Publier, modifier et supprimer demandent le droit `announcements` ; tout
//! membre lit. Chacun a son état lu / non lu ; une annonce publiée notifie les membres.
//!
//! Ordre : épinglées d'abord, puis les plus récentes. Une annonce expirée n'est plus
//! montrée qu'aux personnes qui gèrent les annonces (pour la prolonger ou la supprimer).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post, put},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::access::{member_perms, AuthUser, Perms};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub fn workspace_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/read-all", post(read_all))
        .route("/{id}", put(update).delete(remove))
        .route("/{id}/read", post(mark_read))
        .route("/{id}/vote", post(crate::polls::vote))
        .route("/{id}/poll/close", post(close_poll))
}

#[derive(Serialize, sqlx::FromRow)]
pub struct Announcement {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub author_id: Option<Uuid>,
    pub author_name: Option<String>,
    pub title: String,
    pub body: String,
    pub pinned: bool,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub read: bool,
    pub expired: bool,
    /// Réservée à certains groupes ou personnes (sinon tout l'espace).
    pub targeted: bool,
    /// Le membre fait partie des destinataires (l'auteur et les gestionnaires voient aussi
    /// les annonces qui ne leur sont pas adressées).
    pub for_me: bool,
    /// Destinataires choisis, montrés à l'auteur et aux gestionnaires.
    #[sqlx(skip)]
    pub audience: Option<Audience>,
    /// Sondage porté par l'annonce (voir polls.rs).
    #[sqlx(skip)]
    pub poll: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct Audience {
    #[serde(default)]
    pub groups: Vec<Uuid>,
    #[serde(default)]
    pub users: Vec<Uuid>,
}

/// Le membre `$2` est destinataire de l'annonce `a`.
const FOR_ME: &str = "(NOT a.targeted
            OR EXISTS (SELECT 1 FROM announcement_users au WHERE au.announcement_id = a.id AND au.user_id = $2)
            OR EXISTS (SELECT 1 FROM announcement_groups ag JOIN member_group_members gm ON gm.group_id = ag.group_id
                        WHERE ag.announcement_id = a.id AND gm.user_id = $2))";

async fn perms_of(state: &AppState, user: &AuthUser, ws: Uuid) -> ApiResult<Perms> {
    Ok(member_perms(&state.pool, user.id, ws).await?.ok_or(ApiError::NotFound)?.1)
}

fn can_manage(p: &Perms) -> bool {
    p.announcements || p.admin
}

const SELECT: &str = "SELECT a.id, a.workspace_id, a.author_id, u.name AS author_name, a.title, a.body, a.pinned, a.expires_at,
        a.created_at, a.updated_at, (r.user_id IS NOT NULL) AS read, (a.expires_at IS NOT NULL AND a.expires_at <= NOW()) AS expired,
        a.targeted, (NOT a.targeted
            OR EXISTS (SELECT 1 FROM announcement_users au WHERE au.announcement_id = a.id AND au.user_id = $2)
            OR EXISTS (SELECT 1 FROM announcement_groups ag JOIN member_group_members gm ON gm.group_id = ag.group_id
                        WHERE ag.announcement_id = a.id AND gm.user_id = $2)) AS for_me
     FROM announcements a
     LEFT JOIN users u ON u.id = a.author_id
     LEFT JOIN announcement_reads r ON r.announcement_id = a.id AND r.user_id = $2";

async fn load(state: &AppState, id: Uuid, user: Uuid) -> ApiResult<Announcement> {
    let mut a: Announcement = sqlx::query_as(sqlx::AssertSqlSafe(format!("{SELECT} WHERE a.id = $1")))
        .bind(id)
        .bind(user)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(ApiError::NotFound)?;
    a.poll = crate::polls::load(&state.pool, &[id], user).await?.remove(&id);
    if a.targeted {
        a.audience = Some(audience_of(&state.pool, id).await?);
    }
    Ok(a)
}

async fn audience_of(pool: &sqlx::PgPool, id: Uuid) -> ApiResult<Audience> {
    Ok(Audience {
        groups: sqlx::query_scalar("SELECT group_id FROM announcement_groups WHERE announcement_id = $1").bind(id).fetch_all(pool).await?,
        users: sqlx::query_scalar("SELECT user_id FROM announcement_users WHERE announcement_id = $1").bind(id).fetch_all(pool).await?,
    })
}

/// L'annonce existe dans l'espace et le membre peut la voir (destinataire, auteur ou
/// gestionnaire) ; NotFound sinon. Sert aussi au vote des sondages.
pub async fn require_visible(state: &AppState, user: &AuthUser, ws: Uuid, id: Uuid) -> ApiResult<()> {
    let perms = perms_of(state, user, ws).await?;
    let ok: bool = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT EXISTS (SELECT 1 FROM announcements a WHERE a.id = $1 AND a.workspace_id = $3
                          AND ($4 OR a.author_id = $2 OR {FOR_ME}))"
    )))
    .bind(id)
    .bind(user.id)
    .bind(ws)
    .bind(can_manage(&perms))
    .fetch_one(&state.pool)
    .await?;
    if !ok {
        return Err(ApiError::NotFound);
    }
    Ok(())
}

/// Destinataires actuels (membres de l'espace) d'une annonce.
async fn recipients(pool: &sqlx::PgPool, ws: Uuid, id: Uuid) -> ApiResult<Vec<Uuid>> {
    let members: Vec<String> = sqlx::query_scalar("SELECT user_id FROM workspace_members WHERE workspace_id = $1").bind(ws).fetch_all(pool).await?;
    let mut out = Vec::new();
    for m in members.iter().filter_map(|m| Uuid::parse_str(m).ok()) {
        let mine: bool = sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT {FOR_ME} FROM announcements a WHERE a.id = $1")))
            .bind(id)
            .bind(m)
            .fetch_one(pool)
            .await?;
        if mine {
            out.push(m);
        }
    }
    Ok(out)
}

/// Enregistre les destinataires (vide : tout l'espace). Groupes et personnes hors de
/// l'espace ignorés.
async fn save_audience(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, ws: Uuid, id: Uuid, a: &Audience) -> ApiResult<()> {
    sqlx::query("DELETE FROM announcement_groups WHERE announcement_id = $1").bind(id).execute(&mut **tx).await?;
    sqlx::query("DELETE FROM announcement_users WHERE announcement_id = $1").bind(id).execute(&mut **tx).await?;
    let targeted = !a.groups.is_empty() || !a.users.is_empty();
    sqlx::query("UPDATE announcements SET targeted = $2 WHERE id = $1").bind(id).bind(targeted).execute(&mut **tx).await?;
    sqlx::query(
        "INSERT INTO announcement_groups (announcement_id, group_id)
         SELECT $1, g FROM unnest($2::uuid[]) g WHERE EXISTS (SELECT 1 FROM member_groups mg WHERE mg.id = g AND mg.workspace_id = $3)
         ON CONFLICT DO NOTHING",
    )
    .bind(id)
    .bind(&a.groups)
    .bind(ws)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "INSERT INTO announcement_users (announcement_id, user_id)
         SELECT $1, u FROM unnest($2::uuid[]) u WHERE EXISTS (SELECT 1 FROM workspace_members wm WHERE wm.workspace_id = $3 AND wm.user_id = u::text)
         ON CONFLICT DO NOTHING",
    )
    .bind(id)
    .bind(&a.users)
    .bind(ws)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn notify_recipients(state: &AppState, ws: Uuid, id: Uuid, title: &str, actor: Uuid, skip: &[Uuid]) -> ApiResult<()> {
    for m in recipients(&state.pool, ws, id).await?.into_iter().filter(|m| !skip.contains(m)) {
        crate::notifications::notify(&state.pool, &state.hub, ws, m, Some(actor), "announcement", &format!("announcement:{id}"), serde_json::json!({ "title": title }))
            .await;
    }
    Ok(())
}

async fn list(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>) -> ApiResult<Json<serde_json::Value>> {
    let perms = perms_of(&state, &user, ws).await?;
    let manage = can_manage(&perms);
    let items: Vec<Announcement> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "{SELECT} WHERE a.workspace_id = $1 AND ($3 OR a.expires_at IS NULL OR a.expires_at > NOW())
           AND ($3 OR a.author_id = $2 OR {FOR_ME})
         ORDER BY (a.pinned AND (a.expires_at IS NULL OR a.expires_at > NOW())) DESC, a.created_at DESC
         LIMIT 200"
    )))
    .bind(ws)
    .bind(user.id)
    .bind(manage)
    .fetch_all(&state.pool)
    .await?;
    let mut items = items;
    let mut polls = crate::polls::load(&state.pool, &items.iter().map(|a| a.id).collect::<Vec<_>>(), user.id).await?;
    for a in &mut items {
        a.poll = polls.remove(&a.id);
        if a.targeted && (manage || a.author_id == Some(user.id)) {
            a.audience = Some(audience_of(&state.pool, a.id).await?);
        }
    }
    let unread = items.iter().filter(|a| !a.read && !a.expired && a.for_me).count();
    Ok(Json(serde_json::json!({ "announcements": items, "unread": unread, "can_manage": manage })))
}

#[derive(Deserialize)]
struct AnnouncementReq {
    title: Option<String>,
    body: Option<String>,
    pinned: Option<bool>,
    #[serde(default, deserialize_with = "crate::board::double_option")]
    expires_at: Option<Option<DateTime<Utc>>>,
    /// Sondage : le créer ou le remplacer (`null` : le retirer).
    #[serde(default, deserialize_with = "crate::board::double_option")]
    poll: Option<Option<crate::polls::PollReq>>,
    /// Destinataires (groupes et personnes) ; vides : tout l'espace.
    audience: Option<Audience>,
}

fn clean_title(raw: &str) -> ApiResult<String> {
    let t = raw.trim();
    if t.is_empty() || t.chars().count() > 200 {
        return Err(ApiError::bad_request("invalid_title"));
    }
    Ok(t.to_string())
}

fn clean_body(raw: &str) -> ApiResult<String> {
    if raw.len() > 100_000 {
        return Err(ApiError::bad_request("announcement_too_long"));
    }
    Ok(crate::board::sanitize_rich_text(raw))
}

async fn changed(state: &AppState, ws: Uuid) {
    state.emit(ws, serde_json::json!({ "type": "ANNOUNCEMENTS_UPDATED" })).await;
}

async fn create(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>, Json(req): Json<AnnouncementReq>) -> ApiResult<Json<Announcement>> {
    let perms = perms_of(&state, &user, ws).await?;
    if !can_manage(&perms) {
        return Err(ApiError::Forbidden);
    }
    let title = clean_title(req.title.as_deref().unwrap_or(""))?;
    let body = clean_body(req.body.as_deref().unwrap_or(""))?;
    let mut tx = state.pool.begin().await?;
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO announcements (workspace_id, author_id, title, body, pinned, expires_at) VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(ws)
    .bind(user.id)
    .bind(&title)
    .bind(&body)
    .bind(req.pinned.unwrap_or(false))
    .bind(req.expires_at.flatten())
    .fetch_one(&mut *tx)
    .await?;
    if let Some(Some(p)) = &req.poll {
        crate::polls::save(&mut tx, id, p).await?;
    }
    if let Some(a) = &req.audience {
        save_audience(&mut tx, ws, id, a).await?;
    }
    tx.commit().await?;
    // L'auteur l'a lue.
    sqlx::query("INSERT INTO announcement_reads (announcement_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
        .bind(id)
        .bind(user.id)
        .execute(&state.pool)
        .await?;

    notify_recipients(&state, ws, id, &title, user.id, &[]).await?;
    changed(&state, ws).await;
    Ok(Json(load(&state, id, user.id).await?))
}

async fn update(State(state): State<AppState>, user: AuthUser, Path((ws, id)): Path<(Uuid, Uuid)>, Json(req): Json<AnnouncementReq>) -> ApiResult<Json<Announcement>> {
    let perms = perms_of(&state, &user, ws).await?;
    if !can_manage(&perms) {
        return Err(ApiError::Forbidden);
    }
    let title = req.title.as_deref().map(clean_title).transpose()?;
    let body = req.body.as_deref().map(clean_body).transpose()?;
    let mut tx = state.pool.begin().await?;
    let n = sqlx::query(
        "UPDATE announcements SET title = COALESCE($3, title), body = COALESCE($4, body), pinned = COALESCE($5, pinned),
                expires_at = CASE WHEN $6 THEN $7 ELSE expires_at END, updated_at = NOW()
         WHERE id = $1 AND workspace_id = $2",
    )
    .bind(id)
    .bind(ws)
    .bind(title)
    .bind(body)
    .bind(req.pinned)
    .bind(req.expires_at.is_some())
    .bind(req.expires_at.flatten())
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if n == 0 {
        return Err(ApiError::NotFound);
    }
    match &req.poll {
        Some(Some(p)) => crate::polls::save(&mut tx, id, p).await?,
        Some(None) => crate::polls::delete(&mut tx, id).await?,
        None => {}
    }
    // Nouveaux destinataires : prévenus comme à la publication.
    let before = if req.audience.is_some() { Some(recipients(&state.pool, ws, id).await?) } else { None };
    if let Some(a) = &req.audience {
        save_audience(&mut tx, ws, id, a).await?;
    }
    tx.commit().await?;
    if let Some(before) = before {
        let title: String = sqlx::query_scalar("SELECT title FROM announcements WHERE id = $1").bind(id).fetch_one(&state.pool).await?;
        notify_recipients(&state, ws, id, &title, user.id, &before).await?;
    }
    changed(&state, ws).await;
    Ok(Json(load(&state, id, user.id).await?))
}

async fn remove(State(state): State<AppState>, user: AuthUser, Path((ws, id)): Path<(Uuid, Uuid)>) -> ApiResult<StatusCode> {
    let perms = perms_of(&state, &user, ws).await?;
    if !can_manage(&perms) {
        return Err(ApiError::Forbidden);
    }
    let n = sqlx::query("DELETE FROM announcements WHERE id = $1 AND workspace_id = $2").bind(id).bind(ws).execute(&state.pool).await?.rows_affected();
    if n == 0 {
        return Err(ApiError::NotFound);
    }
    // Les notifications de l'annonce n'ont plus de cible.
    sqlx::query("DELETE FROM notifications WHERE workspace_id = $1 AND target = $2").bind(ws).bind(format!("announcement:{id}")).execute(&state.pool).await?;
    changed(&state, ws).await;
    Ok(StatusCode::NO_CONTENT)
}

/// Clore un sondage avant sa date (ou sans date).
async fn close_poll(State(state): State<AppState>, user: AuthUser, Path((ws, id)): Path<(Uuid, Uuid)>) -> ApiResult<StatusCode> {
    let perms = perms_of(&state, &user, ws).await?;
    if !can_manage(&perms) {
        return Err(ApiError::Forbidden);
    }
    let n = sqlx::query("UPDATE announcement_polls p SET closed = TRUE FROM announcements a WHERE a.id = p.announcement_id AND p.announcement_id = $1 AND a.workspace_id = $2")
        .bind(id)
        .bind(ws)
        .execute(&state.pool)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(ApiError::NotFound);
    }
    changed(&state, ws).await;
    Ok(StatusCode::NO_CONTENT)
}

async fn mark_read(State(state): State<AppState>, user: AuthUser, Path((ws, id)): Path<(Uuid, Uuid)>) -> ApiResult<StatusCode> {
    require_visible(&state, &user, ws, id).await?;
    sqlx::query(
        "INSERT INTO announcement_reads (announcement_id, user_id)
         SELECT id, $3 FROM announcements WHERE id = $1 AND workspace_id = $2 ON CONFLICT DO NOTHING",
    )
    .bind(id)
    .bind(ws)
    .bind(user.id)
    .execute(&state.pool)
    .await?;
    // La notification correspondante est lue aussi.
    sqlx::query("UPDATE notifications SET read_at = NOW() WHERE user_id = $1 AND workspace_id = $2 AND target = $3 AND read_at IS NULL")
        .bind(user.id)
        .bind(ws)
        .bind(format!("announcement:{id}"))
        .execute(&state.pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn read_all(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>) -> ApiResult<StatusCode> {
    perms_of(&state, &user, ws).await?;
    sqlx::query("INSERT INTO announcement_reads (announcement_id, user_id) SELECT id, $2 FROM announcements WHERE workspace_id = $1 ON CONFLICT DO NOTHING")
        .bind(ws)
        .bind(user.id)
        .execute(&state.pool)
        .await?;
    sqlx::query("UPDATE notifications SET read_at = NOW() WHERE user_id = $1 AND workspace_id = $2 AND target LIKE 'announcement:%' AND read_at IS NULL")
        .bind(user.id)
        .bind(ws)
        .execute(&state.pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_are_trimmed_and_bounded() {
        assert_eq!(clean_title("  Changement de salle ").unwrap(), "Changement de salle");
        assert!(clean_title("   ").is_err());
        assert!(clean_title(&"é".repeat(201)).is_err());
        assert!(clean_title(&"é".repeat(200)).is_ok());
    }

    #[test]
    fn body_is_sanitized() {
        let b = clean_body("<p>Salle <b>B12</b></p><script>alert(1)</script>").unwrap();
        assert!(b.contains("B12") && !b.contains("script"));
    }

    #[test]
    fn managers() {
        let mut p = Perms::preset(crate::access::Role::Member);
        assert!(!can_manage(&p));
        p.announcements = true;
        assert!(can_manage(&p));
        assert!(can_manage(&Perms::preset(crate::access::Role::Admin)));
    }
}

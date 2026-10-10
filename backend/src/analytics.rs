//! Statistiques d'usage pour l'administration (instance entière) et pour les administrateurs
//! d'un espace : membres actifs, consultations, actions, heures d'activité, pages les plus
//! consultées, personnes les plus actives ; tableau des liens publics.
//!
//! Sources : `analytics_events` (consultations, sessions) et les journaux existants
//! (activité des tâches et des pages, commentaires).

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get},
    Json, Router,
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::access::{require_perm, AuthUser};
use crate::admin::require_super_admin;
use crate::error::ApiResult;
use crate::state::AppState;

pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/analytics", get(instance_analytics))
        .route("/public-links", get(instance_links))
        .route("/public-links/:page_id", delete(revoke_link))
        .route("/invites/:invite_id", delete(revoke_invite))
}

pub fn workspace_routes() -> Router<AppState> {
    Router::new().route("/analytics", get(workspace_analytics)).route("/public-links", get(workspace_links))
}

/// Enregistre un évènement d'usage (sans jamais faire échouer la requête d'origine).
/// Les sessions sont dédoublonnées : au plus une par personne et par quart d'heure.
pub async fn track(pool: &sqlx::PgPool, workspace_id: Uuid, user_id: Uuid, kind: &str, target: Option<Uuid>) {
    let res = if kind == "session" {
        sqlx::query(
            "INSERT INTO analytics_events (workspace_id, user_id, kind)
             SELECT $1, $2, 'session' WHERE NOT EXISTS (
                SELECT 1 FROM analytics_events WHERE workspace_id = $1 AND user_id = $2 AND kind = 'session' AND created_at > NOW() - interval '15 minutes')",
        )
        .bind(workspace_id)
        .bind(user_id)
        .execute(pool)
        .await
    } else {
        sqlx::query("INSERT INTO analytics_events (workspace_id, user_id, kind, target_id) VALUES ($1, $2, $3, $4)")
            .bind(workspace_id)
            .bind(user_id)
            .bind(kind)
            .bind(target)
            .execute(pool)
            .await
    };
    if let Err(e) = res {
        tracing::debug!("analytics: {e}");
    }
}

/// Élagage : 400 jours d'historique.
pub async fn prune(pool: &sqlx::PgPool) {
    let _ = sqlx::query("DELETE FROM analytics_events WHERE created_at < NOW() - interval '400 days'").execute(pool).await;
}

#[derive(Deserialize)]
struct RangeQuery {
    days: Option<i64>,
    workspace: Option<Uuid>,
}

/// Toutes les traces d'activité de la période, normalisées (personne, espace, date, type).
const EVENTS: &str = "
WITH ev AS (
    SELECT a.user_id, a.workspace_id, a.created_at, a.kind AS k, a.target_id FROM analytics_events a WHERE a.created_at > $1
    UNION ALL
    SELECT ta.user_id, b.workspace_id, ta.created_at, 'task', ta.task_id FROM task_activities ta
        JOIN tasks_all t ON t.id = ta.task_id JOIN board_columns c ON c.id = t.column_id JOIN boards b ON b.id = c.board_id
        WHERE ta.created_at > $1
    UNION ALL
    SELECT pa.user_id, pa.workspace_id, pa.created_at, 'edit', pa.page_id FROM page_activities pa WHERE pa.created_at > $1
    UNION ALL
    SELECT CASE WHEN cm.user_id ~ '^[0-9a-f-]{36}$' THEN cm.user_id::uuid END, b.workspace_id, cm.created_at, 'comment', cm.task_id FROM task_comments cm
        JOIN tasks_all t ON t.id = cm.task_id JOIN board_columns c ON c.id = t.column_id JOIN boards b ON b.id = c.board_id
        WHERE cm.created_at > $1
    UNION ALL
    SELECT ic.user_id, ic.workspace_id, ic.created_at, 'comment', ic.target_id FROM item_comments ic WHERE ic.created_at > $1
), e AS (SELECT * FROM ev WHERE ($2::uuid IS NULL OR workspace_id = $2) AND user_id IS NOT NULL)";

async fn build(state: &AppState, ws: Option<Uuid>, days: i64, instance: bool) -> ApiResult<serde_json::Value> {
    let days = days.clamp(1, 365);
    let since = Utc::now() - chrono::Duration::days(days);
    let pool = &state.pool;

    let totals: (i64, i64, i64, i64, i64, i64) = sqlx::query_as(&format!(
        "{EVENTS} SELECT count(DISTINCT user_id),
            count(*) FILTER (WHERE k = 'view'), count(*) FILTER (WHERE k = 'edit'),
            count(*) FILTER (WHERE k = 'task'), count(*) FILTER (WHERE k = 'comment'), count(*) FILTER (WHERE k = 'session')
         FROM e"
    ))
    .bind(since)
    .bind(ws)
    .fetch_one(pool)
    .await?;

    let daily: Vec<(NaiveDate, i64, i64, i64)> = sqlx::query_as(&format!(
        "{EVENTS} SELECT d::date, count(DISTINCT e.user_id), count(e.*) FILTER (WHERE e.k = 'view'), count(e.*) FILTER (WHERE e.k IN ('edit', 'task', 'comment'))
         FROM generate_series(date_trunc('day', $1::timestamptz), date_trunc('day', NOW()), interval '1 day') d
         LEFT JOIN e ON date_trunc('day', e.created_at) = d
         GROUP BY d ORDER BY d"
    ))
    .bind(since)
    .bind(ws)
    .fetch_all(pool)
    .await?;

    // Heures d'activité : jour de la semaine (1 = lundi) × heure, fuseau du serveur de base.
    let tz = std::env::var("CALENDAR_DEFAULT_TZ").unwrap_or_else(|_| "UTC".into());
    let heat: Vec<(i32, i32, i64)> = sqlx::query_as(&format!(
        "{EVENTS} SELECT extract(isodow FROM created_at AT TIME ZONE $3)::int, extract(hour FROM created_at AT TIME ZONE $3)::int, count(*)
         FROM e GROUP BY 1, 2"
    ))
    .bind(since)
    .bind(ws)
    .bind(&tz)
    .fetch_all(pool)
    .await?;
    let mut heatmap = vec![vec![0i64; 24]; 7];
    for (dow, hour, n) in heat {
        if (1..=7).contains(&dow) && (0..24).contains(&hour) {
            heatmap[(dow - 1) as usize][hour as usize] = n;
        }
    }

    let top_pages: Vec<(Uuid, String, Option<String>, Uuid, String, i64, i64)> = sqlx::query_as(
        "SELECT p.id, p.title, p.icon, w.id, w.name, count(*), count(DISTINCT a.user_id)
         FROM analytics_events a JOIN pages p ON p.id = a.target_id JOIN workspaces w ON w.id = p.workspace_id
         WHERE a.kind = 'view' AND a.created_at > $1 AND ($2::uuid IS NULL OR a.workspace_id = $2)
         GROUP BY p.id, p.title, p.icon, w.id, w.name ORDER BY count(*) DESC LIMIT 15",
    )
    .bind(since)
    .bind(ws)
    .fetch_all(pool)
    .await?;

    let top_users: Vec<(Uuid, String, Option<String>, i64, i64, i64, Option<DateTime<Utc>>)> = sqlx::query_as(&format!(
        "{EVENTS} SELECT u.id, u.name, u.avatar_url,
            count(*) FILTER (WHERE e.k = 'view'), count(*) FILTER (WHERE e.k IN ('edit', 'task', 'comment')),
            count(DISTINCT date_trunc('day', e.created_at)), max(e.created_at)
         FROM e JOIN users u ON u.id = e.user_id
         GROUP BY u.id, u.name, u.avatar_url
         ORDER BY count(*) FILTER (WHERE e.k IN ('edit', 'task', 'comment')) * 3 + count(*) DESC LIMIT 15"
    ))
    .bind(since)
    .bind(ws)
    .fetch_all(pool)
    .await?;

    let workspaces: Vec<(Uuid, String, i64, i64, i64, i64)> = if instance {
        sqlx::query_as(&format!(
            "{EVENTS} SELECT w.id, w.name, (SELECT count(*) FROM workspace_members m WHERE m.workspace_id = w.id),
                count(DISTINCT e.user_id), count(e.*) FILTER (WHERE e.k = 'view'), count(e.*) FILTER (WHERE e.k IN ('edit', 'task', 'comment'))
             FROM workspaces w LEFT JOIN e ON e.workspace_id = w.id
             GROUP BY w.id, w.name ORDER BY count(e.*) DESC"
        ))
        .bind(since)
        .bind(ws)
        .fetch_all(pool)
        .await?
    } else {
        Vec::new()
    };

    let members: i64 = match ws {
        Some(w) => sqlx::query_scalar("SELECT count(*) FROM workspace_members WHERE workspace_id = $1").bind(w).fetch_one(pool).await?,
        None => sqlx::query_scalar("SELECT count(*) FROM users WHERE NOT COALESCE(is_suspended, FALSE)").fetch_one(pool).await?,
    };
    // Invités (pages partagées à des personnes extérieures à l'espace).
    let guest_ids: Vec<Uuid> = match ws {
        Some(w) => sqlx::query_scalar(
            "SELECT DISTINCT pp.user_id FROM page_permissions pp JOIN pages p ON p.id = pp.page_id
             WHERE p.workspace_id = $1 AND pp.role IN ('viewer', 'editor')
               AND NOT EXISTS (SELECT 1 FROM workspace_members m WHERE m.workspace_id = $1 AND m.user_id = pp.user_id::varchar)",
        )
        .bind(w)
        .fetch_all(pool)
        .await?,
        None => Vec::new(),
    };
    let active_guests: i64 = if guest_ids.is_empty() {
        0
    } else {
        sqlx::query_scalar(&format!("{EVENTS} SELECT count(DISTINCT user_id) FROM e WHERE user_id = ANY($3)"))
            .bind(since)
            .bind(ws)
            .bind(&guest_ids)
            .fetch_one(pool)
            .await?
    };
    let new_users: i64 = if instance {
        sqlx::query_scalar("SELECT count(*) FROM users WHERE created_at > $1").bind(since).fetch_one(pool).await?
    } else {
        sqlx::query_scalar("SELECT count(*) FROM workspace_members WHERE workspace_id = $1 AND joined_at > $2").bind(ws).bind(since).fetch_one(pool).await?
    };

    Ok(serde_json::json!({
        "days": days,
        "totals": {
            "members": members, "guests": guest_ids.len(), "active_guests": active_guests, "active_users": totals.0, "views": totals.1, "edits": totals.2,
            "task_actions": totals.3, "comments": totals.4, "sessions": totals.5, "new_users": new_users,
        },
        "daily": daily.into_iter().map(|(d, a, v, x)| serde_json::json!({ "day": d, "active": a, "views": v, "actions": x })).collect::<Vec<_>>(),
        "heatmap": heatmap,
        "timezone": tz,
        "top_pages": top_pages.into_iter().map(|(id, title, icon, wid, wname, views, viewers)| serde_json::json!({
            "id": id, "title": title, "icon": icon, "workspace_id": wid, "workspace_name": wname, "views": views, "viewers": viewers,
        })).collect::<Vec<_>>(),
        "top_users": top_users.into_iter().map(|(id, name, avatar, views, actions, days, last)| serde_json::json!({
            "id": id, "name": name, "avatar_url": avatar, "views": views, "actions": actions, "active_days": days, "last_active": last, "guest": guest_ids.contains(&id),
        })).collect::<Vec<_>>(),
        "workspaces": workspaces.into_iter().map(|(id, name, m, active, views, actions)| serde_json::json!({
            "id": id, "name": name, "members": m, "active": active, "views": views, "actions": actions,
        })).collect::<Vec<_>>(),
    }))
}

async fn instance_analytics(State(state): State<AppState>, user: AuthUser, Query(q): Query<RangeQuery>) -> ApiResult<Json<serde_json::Value>> {
    require_super_admin(&user)?;
    Ok(Json(build(&state, q.workspace, q.days.unwrap_or(30), q.workspace.is_none()).await?))
}

async fn workspace_analytics(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>, Query(q): Query<RangeQuery>) -> ApiResult<Json<serde_json::Value>> {
    require_perm(&state.pool, &user, ws, |p| p.admin).await?;
    Ok(Json(build(&state, Some(ws), q.days.unwrap_or(30), false).await?))
}

/// Liens publics : documents publiés et liens d'invitation actifs.
async fn links(state: &AppState, ws: Option<Uuid>) -> ApiResult<serde_json::Value> {
    let pages: Vec<(Uuid, String, Option<String>, Option<Uuid>, Uuid, String, DateTime<Utc>, Option<String>)> = sqlx::query_as(
        "SELECT p.id, p.title, p.icon, p.share_token, w.id, w.name, p.updated_at, u.name
         FROM pages p JOIN workspaces w ON w.id = p.workspace_id LEFT JOIN users u ON u.id = p.updated_by
         WHERE p.is_public AND p.share_token IS NOT NULL AND ($1::uuid IS NULL OR p.workspace_id = $1)
         ORDER BY p.updated_at DESC LIMIT 500",
    )
    .bind(ws)
    .fetch_all(&state.pool)
    .await?;
    let invites: Vec<(Uuid, Uuid, String, String, Option<String>, Option<DateTime<Utc>>, Option<i32>, i32, DateTime<Utc>, Option<String>)> = sqlx::query_as(
        "SELECT i.id, w.id, w.name, i.role, r.name, i.expires_at, i.max_uses, i.uses, i.created_at, u.name
         FROM workspace_invites i JOIN workspaces w ON w.id = i.workspace_id
         LEFT JOIN workspace_roles r ON r.id = i.role_id LEFT JOIN users u ON u.id = i.created_by
         WHERE i.revoked_at IS NULL AND (i.expires_at IS NULL OR i.expires_at > NOW()) AND (i.max_uses IS NULL OR i.uses < i.max_uses)
           AND ($1::uuid IS NULL OR i.workspace_id = $1)
         ORDER BY i.created_at DESC LIMIT 500",
    )
    .bind(ws)
    .fetch_all(&state.pool)
    .await?;
    Ok(serde_json::json!({
        "pages": pages.into_iter().map(|(id, title, icon, token, wid, wname, updated, by)| serde_json::json!({
            "id": id, "title": title, "icon": icon, "url": format!("{}/public/{}", state.base_url, token.map(|t| t.to_string()).unwrap_or_default()),
            "workspace_id": wid, "workspace_name": wname, "updated_at": updated, "updated_by": by,
        })).collect::<Vec<_>>(),
        "invites": invites.into_iter().map(|(id, wid, wname, role, role_name, expires, max, uses, created, by)| serde_json::json!({
            "id": id, "workspace_id": wid, "workspace_name": wname, "role": role, "role_name": role_name, "expires_at": expires,
            "max_uses": max, "uses": uses, "created_at": created, "created_by": by,
        })).collect::<Vec<_>>(),
    }))
}

async fn instance_links(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<serde_json::Value>> {
    require_super_admin(&user)?;
    Ok(Json(links(&state, None).await?))
}

async fn workspace_links(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>) -> ApiResult<Json<serde_json::Value>> {
    require_perm(&state.pool, &user, ws, |p| p.admin).await?;
    Ok(Json(links(&state, Some(ws)).await?))
}

/// Retrait d'un lien public par l'administrateur de l'instance (même sans être membre).
async fn revoke_link(State(state): State<AppState>, user: AuthUser, Path(page_id): Path<Uuid>) -> ApiResult<StatusCode> {
    require_super_admin(&user)?;
    let ws: Option<Uuid> = sqlx::query_scalar("UPDATE pages_all SET is_public = false, share_token = NULL WHERE id = $1 RETURNING workspace_id")
        .bind(page_id)
        .fetch_optional(&state.pool)
        .await?;
    if let Some(ws) = ws {
        state.emit(ws, serde_json::json!({ "type": "PAGE_PERMISSIONS_UPDATED", "page_id": page_id })).await;
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Révocation d'un lien d'invitation par l'administrateur de l'instance.
async fn revoke_invite(State(state): State<AppState>, user: AuthUser, Path(invite_id): Path<Uuid>) -> ApiResult<StatusCode> {
    require_super_admin(&user)?;
    sqlx::query("UPDATE workspace_invites SET revoked_at = NOW() WHERE id = $1 AND revoked_at IS NULL").bind(invite_id).execute(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT)
}

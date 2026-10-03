//! Acheminement des notifications par e-mail et push, selon les préférences de chacun.
//!
//! Chaque notification de la boîte de réception est rattachée à un type d'événement
//! (assignation, commentaire, mention dans une tâche ou un document, échéance,
//! tâche terminée). Pour chacun, l'utilisateur choisit e-mail et / ou push.
//! Les envois passent par une file (`notification_deliveries`) traitée en tâche de
//! fond : le push part tout de suite, l'e-mail après un court délai et seulement si
//! la notification n'a pas déjà été lue dans l'application.

use axum::{extract::State, routing::get, Json, Router};
use chrono::{DateTime, Timelike, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::sync::Notify;
use uuid::Uuid;

use crate::access::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

/// Types d'événements et valeurs par défaut (e-mail, push).
pub const EVENTS: &[(&str, bool, bool)] = &[
    ("task_assigned", true, true),
    ("task_mention", true, true),
    ("doc_mention", true, true),
    ("task_comment", false, true),
    ("task_due", true, true),
    ("task_completed", false, false),
];

pub fn event_of(kind: &str, target: &str) -> &'static str {
    match kind {
        "assigned" => "task_assigned",
        "comment" => "task_comment",
        "mention" if target.starts_with("task:") => "task_mention",
        "mention" => "doc_mention",
        "due_soon" => "task_due",
        "completed" => "task_completed",
        _ => "other",
    }
}

fn wake() -> &'static Arc<Notify> {
    static W: OnceLock<Arc<Notify>> = OnceLock::new();
    W.get_or_init(|| Arc::new(Notify::new()))
}

// ---------------------------------------------------------------------------
// Préférences
// ---------------------------------------------------------------------------

pub fn routes() -> Router<AppState> {
    Router::new().route("/", get(get_prefs).put(put_prefs))
}

#[derive(Serialize, Deserialize, sqlx::FromRow, Clone)]
struct EventPref {
    event: String,
    email: bool,
    push: bool,
}

#[derive(Serialize)]
struct PrefsDto {
    email_enabled: bool,
    events: Vec<EventPref>,
    /// L'instance peut envoyer des e-mails (SMTP configuré).
    email_available: bool,
    email: String,
}

async fn load_prefs(pool: &PgPool, user_id: Uuid) -> ApiResult<Vec<EventPref>> {
    let rows: Vec<EventPref> = sqlx::query_as("SELECT event, email, push FROM notification_preferences WHERE user_id = $1").bind(user_id).fetch_all(pool).await?;
    Ok(EVENTS
        .iter()
        .map(|(e, email, push)| rows.iter().find(|r| r.event == *e).cloned().unwrap_or(EventPref { event: e.to_string(), email: *email, push: *push }))
        .collect())
}

async fn get_prefs(State(pool): State<PgPool>, user: AuthUser) -> ApiResult<Json<PrefsDto>> {
    let (email_enabled, email): (bool, String) = sqlx::query_as("SELECT email_notifications, email FROM users WHERE id = $1").bind(user.id).fetch_one(&pool).await?;
    Ok(Json(PrefsDto { email_enabled, events: load_prefs(&pool, user.id).await?, email_available: crate::mail::config(&pool).await.is_some(), email }))
}

#[derive(Deserialize)]
struct PrefsReq {
    email_enabled: Option<bool>,
    events: Option<Vec<EventPref>>,
}

async fn put_prefs(State(pool): State<PgPool>, user: AuthUser, Json(p): Json<PrefsReq>) -> ApiResult<Json<PrefsDto>> {
    if let Some(on) = p.email_enabled {
        sqlx::query("UPDATE users SET email_notifications = $1 WHERE id = $2").bind(on).bind(user.id).execute(&pool).await?;
    }
    for e in p.events.unwrap_or_default() {
        if !EVENTS.iter().any(|(k, _, _)| *k == e.event) {
            return Err(ApiError::bad_request("invalid_event"));
        }
        sqlx::query(
            "INSERT INTO notification_preferences (user_id, event, email, push) VALUES ($1, $2, $3, $4)
             ON CONFLICT (user_id, event) DO UPDATE SET email = $3, push = $4",
        )
        .bind(user.id)
        .bind(&e.event)
        .bind(e.email)
        .bind(e.push)
        .execute(&pool)
        .await?;
    }
    get_prefs(State(pool), user).await
}

// ---------------------------------------------------------------------------
// File d'envoi
// ---------------------------------------------------------------------------

/// Programme les envois d'une notification qui vient d'être créée.
pub async fn enqueue(pool: &PgPool, notification_id: Uuid, user_id: Uuid, kind: &str, target: &str) {
    let event = event_of(kind, target);
    let Ok(prefs) = load_prefs(pool, user_id).await else { return };
    let Some(pref) = prefs.iter().find(|p| p.event == event) else { return };
    let email_on: bool = sqlx::query_scalar("SELECT email_notifications FROM users WHERE id = $1").bind(user_id).fetch_one(pool).await.unwrap_or(false);
    if pref.push {
        let has_device: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM push_subscriptions WHERE user_id = $1)").bind(user_id).fetch_one(pool).await.unwrap_or(false);
        if has_device {
            let _ = sqlx::query("INSERT INTO notification_deliveries (notification_id, user_id, channel) VALUES ($1, $2, 'push')").bind(notification_id).bind(user_id).execute(pool).await;
        }
    }
    if pref.email && email_on {
        if let Some(c) = crate::mail::config(pool).await {
            let _ = sqlx::query("INSERT INTO notification_deliveries (notification_id, user_id, channel, send_after) VALUES ($1, $2, 'email', NOW() + make_interval(mins => $3))")
                .bind(notification_id)
                .bind(user_id)
                .bind(c.delay_minutes.max(0))
                .execute(pool)
                .await;
        }
    }
    wake().notify_one();
}

pub fn start_worker(state: AppState) {
    tokio::spawn(async move {
        let mut last_due = std::time::Instant::now() - Duration::from_secs(3600);
        loop {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(20)) => {}
                _ = wake().notified() => {}
            }
            if last_due.elapsed() >= Duration::from_secs(600) {
                last_due = std::time::Instant::now();
                if let Err(e) = due_reminders(&state).await {
                    tracing::warn!("due reminders: {e:?}");
                }
            }
            if let Err(e) = process(&state).await {
                tracing::warn!("notification delivery: {e:?}");
            }
        }
    });
}

#[derive(sqlx::FromRow)]
struct Pending {
    id: Uuid,
    channel: String,
    attempts: i32,
    workspace_id: Uuid,
    kind: String,
    target: String,
    data: serde_json::Value,
    read_at: Option<DateTime<Utc>>,
    actor_name: Option<String>,
    user_id: Uuid,
    email: String,
    name: String,
    locale: Option<String>,
    workspace_name: String,
}

async fn process(state: &AppState) -> ApiResult<()> {
    let pool = &state.pool;
    loop {
        let batch: Vec<Pending> = sqlx::query_as(
            "WITH due AS (
                SELECT id FROM notification_deliveries
                WHERE sent_at IS NULL AND attempts < 5 AND send_after <= NOW()
                ORDER BY send_after LIMIT 25 FOR UPDATE SKIP LOCKED
             ), claimed AS (
                UPDATE notification_deliveries d SET attempts = d.attempts + 1, send_after = NOW() + make_interval(mins => 2 * (d.attempts + 1) * (d.attempts + 1))
                FROM due WHERE d.id = due.id RETURNING d.*
             )
             SELECT c.id, c.channel, c.attempts, n.workspace_id, n.kind, n.target, n.data, n.read_at,
                    a.name AS actor_name, u.id AS user_id, u.email, u.name, u.locale, w.name AS workspace_name
             FROM claimed c
             JOIN notifications n ON n.id = c.notification_id
             JOIN users u ON u.id = c.user_id
             JOIN workspaces w ON w.id = n.workspace_id
             LEFT JOIN users a ON a.id = n.actor_id",
        )
        .fetch_all(pool)
        .await?;
        if batch.is_empty() {
            return Ok(());
        }
        let mail = crate::mail::config(pool).await;
        for p in batch {
            let result: Result<(), String> = async {
                // Lue entre-temps dans l'application : l'e-mail n'a plus d'intérêt.
                if p.channel == "email" && p.read_at.is_some() {
                    return Err("skipped: already read".into());
                }
                let m = message(state, &p).await;
                match p.channel.as_str() {
                    "push" => {
                        let payload = serde_json::json!({ "title": p.workspace_name, "body": m.line, "url": m.path, "tag": p.id });
                        crate::push::send_to_user(state, p.user_id, &payload).await.map(|_| ()).map_err(|e| format!("{e:?}"))
                    }
                    "email" => {
                        let c = mail.as_ref().ok_or("smtp not configured")?;
                        let fr = p.locale.as_deref() == Some("fr");
                        let url = format!("{}{}", state.base_url.trim_end_matches('/'), m.path);
                        let manage = format!("{}/w/{}/settings/notifications", state.base_url.trim_end_matches('/'), p.workspace_id);
                        let (text, html) = render_email(&state.base_url, fr, &p.workspace_name, &m.line, m.excerpt.as_deref(), &url, Some(&manage));
                        let subject = format!("[{}] {}", p.workspace_name, m.line);
                        crate::mail::send(c, &crate::mail::Email { to: p.email.clone(), to_name: p.name.clone(), subject, text, html }).await.map_err(|e| format!("{e:?}"))
                    }
                    other => Err(format!("unknown channel {other}")),
                }
            }
            .await;
            match result {
                Ok(()) => {
                    sqlx::query("UPDATE notification_deliveries SET sent_at = NOW(), error = NULL WHERE id = $1").bind(p.id).execute(pool).await?;
                }
                Err(e) if e.starts_with("skipped") => {
                    sqlx::query("UPDATE notification_deliveries SET sent_at = NOW(), error = $2 WHERE id = $1").bind(p.id).bind(&e).execute(pool).await?;
                }
                Err(e) => {
                    tracing::warn!("{} delivery {} failed (attempt {}): {}", p.channel, p.id, p.attempts + 1, e);
                    sqlx::query("UPDATE notification_deliveries SET error = $2 WHERE id = $1").bind(p.id).bind(e.chars().take(500).collect::<String>()).execute(pool).await?;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Rappels d'échéance
// ---------------------------------------------------------------------------

async fn due_reminders(state: &AppState) -> ApiResult<()> {
    let pool = &state.pool;
    let rows: Vec<(Uuid, Uuid, DateTime<Utc>, String, Uuid, Uuid)> = sqlx::query_as(
        "SELECT t.id, ta.user_id::uuid, t.due_date, t.title, b.id, b.workspace_id
         FROM tasks t
         JOIN task_assignees ta ON ta.task_id = t.id
         JOIN board_columns c ON c.id = t.column_id
         JOIN boards b ON b.id = c.board_id
         JOIN workspace_members wm ON wm.workspace_id = b.workspace_id AND wm.user_id = ta.user_id
         WHERE NOT t.is_completed AND t.due_date IS NOT NULL
           AND t.due_date > NOW() - interval '12 hours' AND t.due_date <= NOW() + interval '24 hours'
           AND ta.user_id ~ '^[0-9a-fA-F-]{36}$'
           AND NOT (t.is_per_person AND EXISTS(SELECT 1 FROM task_validations v WHERE v.task_id = t.id AND v.user_id = ta.user_id))
           AND NOT EXISTS(SELECT 1 FROM task_due_reminders r WHERE r.task_id = t.id AND r.user_id = ta.user_id::uuid AND r.due_date = t.due_date)
         LIMIT 500",
    )
    .fetch_all(pool)
    .await?;
    for (task_id, user_id, due, title, board_id, ws) in rows {
        let inserted = sqlx::query("INSERT INTO task_due_reminders (task_id, user_id, due_date) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING")
            .bind(task_id).bind(user_id).bind(due).execute(pool).await?.rows_affected();
        if inserted > 0 {
            crate::notifications::notify(pool, &state.hub, ws, user_id, None, "due_soon", &format!("task:{task_id}"),
                serde_json::json!({ "title": title, "board_id": board_id, "due_date": due })).await;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Rendu
// ---------------------------------------------------------------------------

struct Rendered {
    line: String,
    excerpt: Option<String>,
    path: String,
}

fn fmt_due(d: &DateTime<Utc>, fr: bool) -> String {
    let all_day = d.hour() == 12 && d.minute() == 0;
    match (fr, all_day) {
        (true, true) => d.format("%d/%m/%Y").to_string(),
        (true, false) => d.format("%d/%m/%Y à %H:%M UTC").to_string(),
        (false, true) => d.format("%b %-d, %Y").to_string(),
        (false, false) => d.format("%b %-d, %Y at %H:%M UTC").to_string(),
    }
}

async fn message(state: &AppState, p: &Pending) -> Rendered {
    let fr = p.locale.as_deref() == Some("fr");
    let actor = p.actor_name.clone().unwrap_or_else(|| if fr { "Quelqu’un".into() } else { "Someone".into() });
    let (kind, id) = p.target.split_once(':').unwrap_or(("", ""));
    let mut title = p.data["title"].as_str().unwrap_or("").to_string();
    let path = match kind {
        "task" => match p.data["board_id"].as_str() {
            Some(b) => format!("/w/{}/board/{}?task={}", p.workspace_id, b, id),
            None => format!("/w/{}/task/{}", p.workspace_id, id),
        },
        "page" => {
            if let Ok(pid) = Uuid::parse_str(id) {
                title = sqlx::query_scalar::<_, String>("SELECT title FROM pages WHERE id = $1").bind(pid).fetch_optional(&state.pool).await.ok().flatten().unwrap_or_default();
            }
            format!("/w/{}/doc/{}", p.workspace_id, id)
        }
        "board" => format!("/w/{}/board/{}", p.workspace_id, id),
        _ => format!("/w/{}/inbox", p.workspace_id),
    };
    if title.trim().is_empty() {
        title = if fr { "Sans titre".into() } else { "Untitled".into() };
    }
    let q = |s: &str| if fr { format!("« {s} »") } else { format!("“{s}”") };
    let line = match (event_of(&p.kind, &p.target), fr) {
        ("task_assigned", false) => format!("{actor} assigned you to {}", q(&title)),
        ("task_assigned", true) => format!("{actor} vous a assigné {}", q(&title)),
        ("task_comment", false) => format!("{actor} commented on {}", q(&title)),
        ("task_comment", true) => format!("{actor} a commenté {}", q(&title)),
        ("task_mention", false) | ("doc_mention", false) => format!("{actor} mentioned you in {}", q(&title)),
        ("task_mention", true) | ("doc_mention", true) => format!("{actor} vous a mentionné dans {}", q(&title)),
        ("task_due", _) => {
            let due = p.data["due_date"].as_str().and_then(|s| DateTime::parse_from_rfc3339(s).ok()).map(|d| fmt_due(&d.with_timezone(&Utc), fr)).unwrap_or_default();
            if fr { format!("{} arrive à échéance le {due}", q(&title)) } else { format!("{} is due {due}", q(&title)) }
        }
        ("task_completed", false) => format!("{actor} completed {}", q(&title)),
        ("task_completed", true) => format!("{actor} a terminé {}", q(&title)),
        (_, false) => format!("New activity from {actor}"),
        (_, true) => format!("Nouvelle activité de {actor}"),
    };
    // Extrait du commentaire, s'il y en a un.
    let mut excerpt = None;
    if let Some(cid) = p.data["comment_id"].as_str().and_then(|s| Uuid::parse_str(s).ok()) {
        if let Ok(Some(html)) = sqlx::query_scalar::<_, String>("SELECT content FROM task_comments WHERE id = $1").bind(cid).fetch_optional(&state.pool).await {
            let text = crate::ical::html_to_text(&html);
            let short: String = text.chars().take(400).collect();
            excerpt = Some(if text.chars().count() > 400 { format!("{short}…") } else { short });
        }
    }
    Rendered { line, excerpt, path }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Corps texte et HTML d'un e-mail de notification.
pub fn render_email(_base: &str, fr: bool, context: &str, line: &str, excerpt: Option<&str>, url: &str, manage_url: Option<&str>) -> (String, String) {
    let open = if fr { "Ouvrir dans Looty" } else { "Open in Looty" };
    let manage = if fr { "Gérer mes notifications" } else { "Manage notifications" };
    let mut text = format!("{line}\n\n");
    if let Some(x) = excerpt {
        text.push_str(&format!("> {}\n\n", x.replace('\n', "\n> ")));
    }
    text.push_str(&format!("{open}: {url}\n"));
    if let Some(m) = manage_url {
        text.push_str(&format!("\n{manage}: {m}\n"));
    }
    let quote = excerpt
        .map(|x| format!(r#"<div style="margin:16px 0 0;padding:10px 14px;border-left:3px solid #d0d0d8;color:#4a4a55;white-space:pre-wrap">{}</div>"#, esc(x)))
        .unwrap_or_default();
    let footer = manage_url
        .map(|m| format!(r#"<p style="margin:24px 0 0;font-size:12px;color:#8a8a96"><a href="{}" style="color:#8a8a96">{}</a></p>"#, esc(m), manage))
        .unwrap_or_default();
    let html = format!(
        r#"<!doctype html><html><body style="margin:0;padding:24px;background:#f5f5f7;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Inter,Roboto,sans-serif;color:#1b1b1f">
<table role="presentation" width="100%" cellspacing="0" cellpadding="0"><tr><td align="center">
<table role="presentation" width="100%" style="max-width:560px;background:#ffffff;border-radius:10px;border:1px solid #e6e6ea" cellspacing="0" cellpadding="0"><tr><td style="padding:28px">
<div style="font-size:12px;color:#8a8a96;text-transform:uppercase;letter-spacing:.04em">{ctx}</div>
<p style="margin:10px 0 0;font-size:16px;line-height:1.5;font-weight:600">{line}</p>{quote}
<p style="margin:22px 0 0"><a href="{url}" style="display:inline-block;padding:9px 16px;border-radius:6px;background:#5e6ad2;color:#ffffff;text-decoration:none;font-weight:500;font-size:14px">{open}</a></p>{footer}
</td></tr></table></td></tr></table></body></html>"#,
        ctx = esc(context),
        line = esc(line),
        url = esc(url),
    );
    (text, html)
}

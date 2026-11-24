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
    ("mail_received", false, true),
    ("moodle_received", false, true),
    ("announcement", true, true),
];

pub fn event_of(kind: &str, target: &str) -> &'static str {
    match kind {
        "assigned" => "task_assigned",
        "comment" => "task_comment",
        "mention" if target.starts_with("task:") => "task_mention",
        "mention" => "doc_mention",
        "due_soon" => "task_due",
        "completed" => "task_completed",
        "mail" => "mail_received",
        "moodle" => "moodle_received",
        "announcement" => "announcement",
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
                _ = tokio::time::sleep(Duration::from_secs(30)) => {}
                _ = wake().notified() => {}
            }
            if last_due.elapsed() >= Duration::from_secs(600) {
                last_due = std::time::Instant::now();
                if let Some(Err(e)) = crate::bg::guard_result("due reminders", due_reminders(&state)).await {
                    tracing::warn!("due reminders: {e:?}");
                }
            }
            crate::bg::tick("notification delivery");
            if let Some(Err(e)) = crate::bg::guard_result("notification delivery", process(&state)).await {
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
    timezone: Option<String>,
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
                    a.name AS actor_name, u.id AS user_id, u.email, u.name, u.locale, u.timezone, w.name AS workspace_name
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
                        // `kind` : type d'événement (canal de notification sur Android : son, vibration…).
                        let payload = serde_json::json!({ "title": p.workspace_name, "body": m.line, "url": m.path, "tag": p.id, "kind": event_of(&p.kind, &p.target) });
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
            if p.channel == "email" && !result.as_ref().is_err_and(|e| e.starts_with("skipped")) {
                crate::bg::record("email delivery", result.clone());
            }
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

/// Une ligne candidate : tâche, personne assignée, échéance, titre, tableau, espace, rappels de la tâche
/// et rappel par défaut de la personne.
type ReminderRow = (Uuid, Uuid, DateTime<Utc>, String, Uuid, Uuid, Option<serde_json::Value>, Option<i32>);

async fn due_reminders(state: &AppState) -> ApiResult<()> {
    let pool = &state.pool;
    let rows: Vec<ReminderRow> = sqlx::query_as(
        "SELECT t.id, ta.user_id::uuid, t.due_date, t.title, b.id, b.workspace_id, t.reminders, u.default_reminder_minutes
         FROM tasks t
         JOIN task_assignees ta ON ta.task_id = t.id
         JOIN users u ON u.id::text = ta.user_id
         JOIN board_columns c ON c.id = t.column_id
         JOIN boards b ON b.id = c.board_id
         JOIN workspace_members wm ON wm.workspace_id = b.workspace_id AND wm.user_id = ta.user_id
         WHERE NOT t.is_completed AND t.due_date IS NOT NULL
           AND t.due_date > NOW() - interval '12 hours'
           AND (t.due_date <= NOW() + make_interval(mins => $1::int) OR jsonb_path_exists(COALESCE(t.reminders, '[]'::jsonb), '$[*].at'))
           AND ta.user_id ~ '^[0-9a-fA-F-]{36}$'
           AND NOT (t.is_per_person AND EXISTS(SELECT 1 FROM task_validations v WHERE v.task_id = t.id AND v.user_id = ta.user_id))
         ORDER BY t.due_date
         LIMIT 5000",
    )
    .bind(crate::reminders::MAX_MINUTES as i32)
    .fetch_all(pool)
    .await?;
    let now = Utc::now();
    // (tâche, personne, échéance, clé du rappel) à envoyer maintenant.
    let mut due_now: Vec<(Uuid, Uuid, DateTime<Utc>, String)> = Vec::new();
    for (task_id, user_id, due, _, _, _, reminders, default_minutes) in &rows {
        for (key, at) in crate::reminders::fire_times(*due, reminders.as_ref(), *default_minutes) {
            if at <= now {
                due_now.push((*task_id, *user_id, *due, key));
            }
        }
    }
    if due_now.is_empty() {
        return Ok(());
    }
    let task_ids: Vec<Uuid> = due_now.iter().map(|r| r.0).collect();
    let user_ids: Vec<Uuid> = due_now.iter().map(|r| r.1).collect();
    let dues: Vec<DateTime<Utc>> = due_now.iter().map(|r| r.2).collect();
    let keys: Vec<String> = due_now.iter().map(|r| r.3.clone()).collect();
    let inserted: Vec<(Uuid, Uuid, DateTime<Utc>)> = sqlx::query_as(
        "INSERT INTO task_due_reminders (task_id, user_id, due_date, reminder_key)
         SELECT * FROM unnest($1::uuid[], $2::uuid[], $3::timestamptz[], $4::text[])
         ON CONFLICT DO NOTHING RETURNING task_id, user_id, due_date",
    )
    .bind(&task_ids).bind(&user_ids).bind(&dues).bind(&keys)
    .fetch_all(pool).await?;
    // Une seule notification par personne et par tâche, même si plusieurs rappels arrivent ensemble.
    let mut seen: std::collections::HashSet<(Uuid, Uuid)> = std::collections::HashSet::new();
    for (task_id, user_id, _) in inserted {
        if !seen.insert((task_id, user_id)) {
            continue;
        }
        if let Some((_, _, due, title, board_id, ws, _, _)) = rows.iter().find(|r| r.0 == task_id && r.1 == user_id) {
            crate::notifications::notify(pool, &state.hub, *ws, user_id, None, "due_soon", &format!("task:{task_id}"),
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

/// Échéance lisible, à l'heure du fuseau du membre (UTC, et dit comme tel, s'il n'en a pas
/// de valide). Une échéance sans heure (12:00 UTC) garde sa date, quel que soit le fuseau.
fn fmt_due(d: &DateTime<Utc>, fr: bool, tz: Option<&str>) -> String {
    if d.hour() == 12 && d.minute() == 0 {
        return if fr { d.format("%d/%m/%Y").to_string() } else { d.format("%b %-d, %Y").to_string() };
    }
    match tz.and_then(|z| z.parse::<chrono_tz::Tz>().ok()) {
        Some(z) => {
            let l = d.with_timezone(&z);
            if fr { l.format("%d/%m/%Y à %H:%M").to_string() } else { l.format("%b %-d, %Y at %-I:%M %p").to_string() }
        }
        None if fr => d.format("%d/%m/%Y à %H:%M UTC").to_string(),
        None => d.format("%b %-d, %Y at %H:%M UTC").to_string(),
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
        "mail" => format!("/w/{}/mail?account={}", p.workspace_id, id),
        "moodle" => format!("/w/{}/moodle", p.workspace_id),
        "announcement" => format!("/w/{}/announcements", p.workspace_id),
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
            let due = p.data["due_date"].as_str().and_then(|s| DateTime::parse_from_rfc3339(s).ok()).map(|d| fmt_due(&d.with_timezone(&Utc), fr, p.timezone.as_deref())).unwrap_or_default();
            if fr { format!("{} arrive à échéance le {due}", q(&title)) } else { format!("{} is due {due}", q(&title)) }
        }
        ("mail_received", false) => match (p.data["from"].as_str(), p.data["count"].as_u64()) {
            (_, Some(n)) => format!("{n} new emails"),
            (Some(from), _) => format!("New email from {from}: {title}"),
            _ => "New email".into(),
        },
        ("mail_received", true) => match (p.data["from"].as_str(), p.data["count"].as_u64()) {
            (_, Some(n)) => format!("{n} nouveaux messages"),
            (Some(from), _) => format!("Nouveau message de {from} : {title}"),
            _ => "Nouveau message".into(),
        },
        ("moodle_received", false) => match p.data["from"].as_str() {
            Some(from) => format!("Moodle — {from}: {title}"),
            None => format!("Moodle: {title}"),
        },
        ("moodle_received", true) => match p.data["from"].as_str() {
            Some(from) => format!("Moodle — {from} : {title}"),
            None => format!("Moodle : {title}"),
        },
        ("announcement", false) => format!("{actor} posted an announcement: {}", q(&title)),
        ("announcement", true) => format!("{actor} a publié une annonce : {}", q(&title)),
        ("task_completed", false) => format!("{actor} completed {}", q(&title)),
        ("task_completed", true) => format!("{actor} a terminé {}", q(&title)),
        (_, false) => format!("New activity from {actor}"),
        (_, true) => format!("Nouvelle activité de {actor}"),
    };
    // Extrait du commentaire ou du texte de l'annonce, s'il y en a un.
    let html = if let Some(cid) = p.data["comment_id"].as_str().and_then(|s| Uuid::parse_str(s).ok()) {
        sqlx::query_scalar::<_, String>("SELECT content FROM task_comments WHERE id = $1").bind(cid).fetch_optional(&state.pool).await.ok().flatten()
    } else if let (Some(aid), "announcement") = (Uuid::parse_str(id).ok(), kind) {
        sqlx::query_scalar::<_, String>("SELECT body FROM announcements WHERE id = $1").bind(aid).fetch_optional(&state.pool).await.ok().flatten()
    } else {
        None
    };
    let excerpt = html.map(|h| crate::ical::html_to_text(&h)).filter(|t| !t.trim().is_empty()).map(|text| {
        let short: String = text.chars().take(400).collect();
        if text.chars().count() > 400 { format!("{short}…") } else { short }
    });
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn due_in_member_timezone() {
        // 13:00 UTC = 14:00 à Paris en hiver, 15:00 en été.
        let winter = Utc.with_ymd_and_hms(2026, 11, 3, 13, 0, 0).unwrap();
        let summer = Utc.with_ymd_and_hms(2026, 7, 3, 13, 0, 0).unwrap();
        assert_eq!(fmt_due(&winter, true, Some("Europe/Paris")), "03/11/2026 à 14:00");
        assert_eq!(fmt_due(&summer, true, Some("Europe/Paris")), "03/07/2026 à 15:00");
        assert_eq!(fmt_due(&winter, false, Some("America/New_York")), "Nov 3, 2026 at 8:00 AM");
        // Passage à l'heure d'hiver (25 octobre 2026, 3 h → 2 h à Paris).
        let before = Utc.with_ymd_and_hms(2026, 10, 25, 0, 30, 0).unwrap();
        let after = Utc.with_ymd_and_hms(2026, 10, 25, 1, 30, 0).unwrap();
        assert_eq!(fmt_due(&before, true, Some("Europe/Paris")), "25/10/2026 à 02:30");
        assert_eq!(fmt_due(&after, true, Some("Europe/Paris")), "25/10/2026 à 02:30");
    }

    #[test]
    fn due_without_valid_timezone_is_utc() {
        let d = Utc.with_ymd_and_hms(2026, 11, 3, 13, 0, 0).unwrap();
        assert_eq!(fmt_due(&d, true, None), "03/11/2026 à 13:00 UTC");
        assert_eq!(fmt_due(&d, false, Some("Mars/Olympus")), "Nov 3, 2026 at 13:00 UTC");
    }

    #[test]
    fn all_day_due_keeps_its_date() {
        let d = Utc.with_ymd_and_hms(2026, 11, 3, 12, 0, 0).unwrap();
        assert_eq!(fmt_due(&d, true, Some("Pacific/Kiritimati")), "03/11/2026");
        assert_eq!(fmt_due(&d, false, Some("Pacific/Pago_Pago")), "Nov 3, 2026");
    }
}

#[cfg(test)]
mod reminder_tests {
    use super::*;
    use sqlx::PgPool;

    /// Une tâche assignée à une personne, échéance dans `due_in` minutes.
    async fn setup(pool: &PgPool, due_in: i64, reminders: Option<serde_json::Value>, default: Option<i32>) -> (Uuid, Uuid) {
        let ws: Uuid = sqlx::query_scalar("INSERT INTO workspaces (name) VALUES ('W') RETURNING id").fetch_one(pool).await.unwrap();
        let uid: Uuid = sqlx::query_scalar("INSERT INTO users (email, name, default_reminder_minutes) VALUES ('a@example.org', 'a', $1) RETURNING id").bind(default).fetch_one(pool).await.unwrap();
        sqlx::query("INSERT INTO workspace_members (workspace_id, user_id, role) VALUES ($1, $2, 'owner')").bind(ws).bind(uid.to_string()).execute(pool).await.unwrap();
        let board: Uuid = sqlx::query_scalar("INSERT INTO boards (workspace_id, name) VALUES ($1, 'B') RETURNING id").bind(ws).fetch_one(pool).await.unwrap();
        let col: Uuid = sqlx::query_scalar("INSERT INTO board_columns (board_id, name, position) VALUES ($1, 'C', 1) RETURNING id").bind(board).fetch_one(pool).await.unwrap();
        let task: Uuid = sqlx::query_scalar("INSERT INTO tasks (column_id, title, position, due_date, reminders) VALUES ($1, 'T', 1, NOW() + make_interval(mins => $2::int), $3) RETURNING id")
            .bind(col).bind(due_in as i32).bind(reminders).fetch_one(pool).await.unwrap();
        sqlx::query("INSERT INTO task_assignees (task_id, user_id) VALUES ($1, $2)").bind(task).bind(uid.to_string()).execute(pool).await.unwrap();
        (task, uid)
    }

    async fn sent(pool: &PgPool, user: Uuid) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM notifications WHERE user_id = $1 AND kind = 'due_soon'").bind(user).fetch_one(pool).await.unwrap()
    }

    fn state(pool: &PgPool) -> AppState {
        AppState::new(pool.clone(), "http://localhost".into())
    }

    #[sqlx::test]
    async fn the_default_reminder_fires_once(pool: PgPool) {
        let (_, uid) = setup(&pool, 600, None, Some(1440)).await;
        due_reminders(&state(&pool)).await.unwrap();
        due_reminders(&state(&pool)).await.unwrap();
        assert_eq!(sent(&pool, uid).await, 1);
    }

    #[sqlx::test]
    async fn no_default_and_no_task_reminder_means_silence(pool: PgPool) {
        let (_, uid) = setup(&pool, 600, None, None).await;
        due_reminders(&state(&pool)).await.unwrap();
        assert_eq!(sent(&pool, uid).await, 0);
    }

    #[sqlx::test]
    async fn a_task_reminder_waits_for_its_time_and_replaces_the_default(pool: PgPool) {
        // Échéance dans 3 h, rappel 1 h avant : rien pour l'instant malgré le défaut de 24 h.
        let (task, uid) = setup(&pool, 180, Some(serde_json::json!([{ "before": 60 }])), Some(1440)).await;
        due_reminders(&state(&pool)).await.unwrap();
        assert_eq!(sent(&pool, uid).await, 0);
        // L'heure du rappel arrive (échéance dans 30 min).
        sqlx::query("UPDATE tasks SET due_date = NOW() + interval '30 minutes' WHERE id = $1").bind(task).execute(&pool).await.unwrap();
        due_reminders(&state(&pool)).await.unwrap();
        due_reminders(&state(&pool)).await.unwrap();
        assert_eq!(sent(&pool, uid).await, 1);
    }

    #[sqlx::test]
    async fn several_reminders_fire_separately_and_an_exact_date_works(pool: PgPool) {
        let at = Utc::now() - chrono::Duration::minutes(5);
        let (task, uid) = setup(&pool, 2 * 1440, Some(serde_json::json!([{ "at": at }])), None).await;
        due_reminders(&state(&pool)).await.unwrap();
        assert_eq!(sent(&pool, uid).await, 1);
        // Un second rappel (1 jour avant) s'ajoute plus tard : un nouvel envoi.
        sqlx::query("UPDATE tasks SET reminders = $2 WHERE id = $1").bind(task).bind(serde_json::json!([{ "at": at }, { "before": 3000 }])).execute(&pool).await.unwrap();
        due_reminders(&state(&pool)).await.unwrap();
        assert_eq!(sent(&pool, uid).await, 2);
        let keys: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM task_due_reminders WHERE task_id = $1").bind(task).fetch_one(&pool).await.unwrap();
        assert_eq!(keys, 2);
    }

    #[sqlx::test]
    async fn completed_tasks_are_left_alone(pool: PgPool) {
        let (task, uid) = setup(&pool, 600, None, Some(1440)).await;
        sqlx::query("UPDATE tasks SET is_completed = true WHERE id = $1").bind(task).execute(&pool).await.unwrap();
        due_reminders(&state(&pool)).await.unwrap();
        assert_eq!(sent(&pool, uid).await, 0);
    }
}

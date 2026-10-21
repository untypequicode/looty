//! Flux d'abonnement ICS en lecture seule : une adresse secrète (`/api/ics/<jeton>.ics`)
//! à coller dans une autre application d'agenda (Infomaniak, Google, Apple, Outlook…).
//!
//! - Le jeton est un `api_tokens` de portée `ics`, lié à un membre et, éventuellement,
//!   à un seul calendrier Looty ; il ne donne accès à rien d'autre.
//! - Le flux contient les événements des espaces du membre au moment de la lecture
//!   (perdre l'accès à un espace en retire les événements) et, pour le flux sans calendrier,
//!   les échéances de ses tâches assignées, en événements d'une journée préfixés « ✓ ».

use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::Response,
};
use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

use crate::access::hash_token;
use crate::calendar::{escape_text, unfolded_lines};
use crate::error::ApiError;
use crate::ical::fold;
use crate::state::AppState;

pub async fn feed(State(state): State<AppState>, Path(file): Path<String>) -> Result<Response, ApiError> {
    let token = file.strip_suffix(".ics").ok_or(ApiError::NotFound)?;
    let row: Option<(Uuid, Option<Uuid>)> = sqlx::query_as(
        "UPDATE api_tokens t SET last_used_at = NOW()
         FROM users u
         WHERE t.token_hash = $1 AND t.scope = 'ics' AND u.id = t.user_id AND NOT u.is_suspended
         RETURNING u.id, t.calendar_id",
    )
    .bind(hash_token(token.trim()))
    .fetch_optional(&state.pool)
    .await?;
    let (user_id, calendar) = row.ok_or(ApiError::NotFound)?;

    let name: String = match calendar {
        Some(c) => sqlx::query_scalar("SELECT 'Looty · ' || w.name || ' · ' || c.name FROM looty_calendars c JOIN workspaces w ON w.id = c.workspace_id WHERE c.id = $1")
            .bind(c)
            .fetch_optional(&state.pool)
            .await?
            .unwrap_or_else(|| "Looty".into()),
        None => "Looty".into(),
    };
    let events: Vec<String> = crate::events::list_for_user(&state.pool, user_id, calendar).await?.into_iter().map(|e| e.dav_ics()).collect();
    let tasks: Vec<(Uuid, String, DateTime<Utc>, DateTime<Utc>)> = if calendar.is_some() {
        vec![]
    } else {
        sqlx::query_as(
            "SELECT t.id, t.title, t.due_date, t.updated_at
             FROM tasks t
             JOIN board_columns c ON c.id = t.column_id
             JOIN boards b ON b.id = c.board_id
             JOIN workspace_members wm ON wm.workspace_id = b.workspace_id AND wm.user_id = $1
             WHERE t.due_date IS NOT NULL AND NOT t.is_completed
               AND EXISTS(SELECT 1 FROM task_assignees ta WHERE ta.task_id = t.id AND ta.user_id = $1)
             ORDER BY t.due_date",
        )
        .bind(user_id.to_string())
        .fetch_all(&state.pool)
        .await?
    };

    let body = build(&name, &events, &tasks);
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/calendar; charset=utf-8")
        .header(header::CONTENT_DISPOSITION, "inline; filename=\"looty.ics\"")
        .header(header::CACHE_CONTROL, "private, no-store")
        .body(axum::body::Body::from(body))
        .unwrap())
}

/// Assemble le flux : les objets iCalendar des événements, puis une journée par échéance de tâche.
fn build(name: &str, events: &[String], tasks: &[(Uuid, String, DateTime<Utc>, DateTime<Utc>)]) -> String {
    let mut lines: Vec<String> = vec![
        "BEGIN:VCALENDAR".into(),
        "VERSION:2.0".into(),
        "PRODID:-//Looty//Subscription feed//EN".into(),
        "CALSCALE:GREGORIAN".into(),
        "METHOD:PUBLISH".into(),
        format!("X-WR-CALNAME:{}", escape_text(name)),
        "REFRESH-INTERVAL;VALUE=DURATION:PT1H".into(),
        "X-PUBLISHED-TTL:PT1H".into(),
    ];
    let mut zones: Vec<String> = Vec::new();
    for ics in events {
        for (zone, block) in components(ics) {
            if let Some(tzid) = zone {
                if zones.contains(&tzid) {
                    continue;
                }
                zones.push(tzid);
            }
            lines.extend(block);
        }
    }
    for (id, title, due, updated) in tasks {
        let day = due.date_naive();
        lines.extend([
            "BEGIN:VEVENT".into(),
            format!("UID:task-{id}@looty"),
            format!("DTSTAMP:{}", updated.format("%Y%m%dT%H%M%SZ")),
            format!("DTSTART;VALUE=DATE:{}", day.format("%Y%m%d")),
            format!("DTEND;VALUE=DATE:{}", (day + Duration::days(1)).format("%Y%m%d")),
            format!("SUMMARY:{}", escape_text(&format!("✓ {title}"))),
            "TRANSP:TRANSPARENT".into(),
            "END:VEVENT".into(),
        ]);
    }
    lines.push("END:VCALENDAR".into());
    let mut out = lines.iter().map(|l| fold(l)).collect::<Vec<_>>().join("\r\n");
    out.push_str("\r\n");
    out
}

/// Composants d'un objet iCalendar (VEVENT, VTIMEZONE…) sans l'enveloppe VCALENDAR ;
/// pour un VTIMEZONE, son TZID (le flux ne le répète pas).
fn components(ics: &str) -> Vec<(Option<String>, Vec<String>)> {
    let mut out = Vec::new();
    let (mut depth, mut cur, mut kind): (usize, Vec<String>, String) = (0, Vec::new(), String::new());
    // Profondeur du VALARM en cours d'ignorance (les rappels n'ont pas leur place dans un flux).
    let mut alarm: Option<usize> = None;
    for line in unfolded_lines(ics) {
        let upper = line.to_ascii_uppercase();
        if depth == 0 {
            if let Some(k) = upper.strip_prefix("BEGIN:") {
                if k.trim() != "VCALENDAR" {
                    kind = k.trim().to_string();
                    cur = vec![line];
                    depth = 1;
                }
            }
            continue;
        }
        let skip = alarm.is_some() || upper.starts_with("ATTENDEE") || upper.starts_with("ORGANIZER");
        if upper.starts_with("BEGIN:") {
            depth += 1;
            if alarm.is_none() && upper.trim() == "BEGIN:VALARM" {
                alarm = Some(depth);
            }
        } else if upper.starts_with("END:") {
            if alarm == Some(depth) {
                alarm = None;
            }
            depth -= 1;
        }
        if !(skip || upper.trim() == "BEGIN:VALARM") {
            cur.push(line);
        }
        if depth == 0 {
            let zone = (kind == "VTIMEZONE").then(|| {
                cur.iter().find(|l| l.to_ascii_uppercase().starts_with("TZID:")).map(|l| l[5..].trim().to_string()).unwrap_or_default()
            });
            out.push((zone, std::mem::take(&mut cur)));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//\r\nBEGIN:VTIMEZONE\r\nTZID:Europe/Paris\r\nEND:VTIMEZONE\r\nBEGIN:VEVENT\r\nUID:a\r\nSUMMARY:A\r\nBEGIN:VALARM\r\nTRIGGER:-PT10M\r\nEND:VALARM\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    const B: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTIMEZONE\r\nTZID:Europe/Paris\r\nEND:VTIMEZONE\r\nBEGIN:VEVENT\r\nUID:b\r\nSUMMARY:B\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

    #[test]
    fn wrapper_dropped_and_timezone_kept_once() {
        let out = build("Looty", &[A.into(), B.into()], &[]);
        assert_eq!(out.matches("BEGIN:VCALENDAR").count(), 1);
        assert_eq!(out.matches("BEGIN:VTIMEZONE").count(), 1);
        assert_eq!(out.matches("BEGIN:VEVENT").count(), 2);
        assert!(!out.contains("VALARM") && out.contains("UID:b"));
        assert!(out.ends_with("END:VCALENDAR\r\n"));
    }

    #[test]
    fn task_due_date_becomes_all_day_event() {
        let due = DateTime::parse_from_rfc3339("2026-10-22T12:00:00Z").unwrap().with_timezone(&Utc);
        let id = Uuid::nil();
        let out = build("Looty", &[], &[(id, "Rendre, le TP".into(), due, due)]);
        assert!(out.contains("DTSTART;VALUE=DATE:20261022\r\n") && out.contains("DTEND;VALUE=DATE:20261023\r\n"));
        assert!(out.contains("SUMMARY:✓ Rendre\\, le TP"));
    }
}

//! Calendrier partagé de l'espace : événements créés dans Looty, visibles par tous les
//! membres et publiés par le serveur CalDAV intégré (lecture et écriture depuis
//! Thunderbird, DAVx⁵, Apple Calendrier…).
//!
//! L'objet iCalendar complet est conservé : une modification faite dans Looty ne
//! retouche que les champs du formulaire (titre, dates, lieu, description, répétition)
//! et garde le reste (alarmes, exceptions, participants ajoutés par un client).

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

use crate::access::{require_member, AuthUser};
use crate::calendar::{escape_text, expand, master_event, series_end, unfolded_lines, MasterEvent};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub fn workspace_routes() -> Router<AppState> {
    Router::new()
        .route("/", axum::routing::post(create))
        .route("/:id", get(get_one).put(update).delete(remove))
}

#[derive(sqlx::FromRow, Clone)]
pub(crate) struct EventRow {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub dav_name: Option<String>,
    pub ics: String,
}

impl EventRow {
    pub fn name(&self) -> String {
        self.dav_name.clone().unwrap_or_else(|| format!("{}.ics", self.id))
    }
}

const COLS: &str = "id, workspace_id, dav_name, ics";

pub(crate) async fn list(pool: &sqlx::PgPool, workspace_id: Uuid) -> ApiResult<Vec<EventRow>> {
    Ok(sqlx::query_as(&format!("SELECT {COLS} FROM workspace_events WHERE workspace_id = $1 ORDER BY created_at")).bind(workspace_id).fetch_all(pool).await?)
}

pub(crate) async fn find_by_name(pool: &sqlx::PgPool, workspace_id: Uuid, name: &str) -> ApiResult<Option<EventRow>> {
    let by_id = name.strip_suffix(".ics").and_then(|s| Uuid::parse_str(s).ok());
    Ok(sqlx::query_as(&format!(
        "SELECT {COLS} FROM workspace_events WHERE workspace_id = $1 AND (dav_name = $2 OR (dav_name IS NULL AND id = $3))"
    ))
    .bind(workspace_id)
    .bind(name)
    .bind(by_id)
    .fetch_optional(pool)
    .await?)
}

/// Colonnes de tri/filtrage déduites de l'objet iCalendar.
fn indexed(ics: &str) -> ApiResult<(MasterEvent, Option<DateTime<Utc>>)> {
    let m = master_event(ics).ok_or_else(|| ApiError::bad_request("event_invalid"))?;
    let end = series_end(ics);
    Ok((m, end))
}

/// Enregistrement depuis un client CalDAV (création ou remplacement complet).
pub(crate) async fn save_from_dav(state: &AppState, workspace_id: Uuid, user: Uuid, name: &str, ics: &str, existing: Option<&EventRow>) -> ApiResult<()> {
    let (m, end) = indexed(ics)?;
    let ics = normalize_calendar(ics);
    match existing {
        Some(e) => {
            sqlx::query(
                "UPDATE workspace_events SET ics = $2, summary = $3, starts_at = $4, series_end = $5, recurring = $6, updated_by = $7, updated_at = NOW() WHERE id = $1",
            )
            .bind(e.id)
            .bind(&ics)
            .bind(&m.summary)
            .bind(m.start)
            .bind(end)
            .bind(m.rrule.is_some())
            .bind(user)
            .execute(&state.pool)
            .await?;
        }
        None => {
            let uid = if m.uid.is_empty() || m.uid.len() > 255 { format!("{}@looty", Uuid::new_v4()) } else { m.uid.clone() };
            sqlx::query(
                "INSERT INTO workspace_events (workspace_id, uid, dav_name, ics, summary, starts_at, series_end, recurring, created_by, updated_by)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9)",
            )
            .bind(workspace_id)
            .bind(&uid)
            .bind(name)
            .bind(&ics)
            .bind(&m.summary)
            .bind(m.start)
            .bind(end)
            .bind(m.rrule.is_some())
            .bind(user)
            .execute(&state.pool)
            .await
            .map_err(|e| match e {
                sqlx::Error::Database(d) if d.is_unique_violation() => ApiError::Conflict("event_exists".into()),
                other => other.into(),
            })?;
        }
    }
    changed(state, workspace_id).await;
    Ok(())
}

pub(crate) async fn delete_row(state: &AppState, e: &EventRow) -> ApiResult<()> {
    sqlx::query("DELETE FROM workspace_events WHERE id = $1").bind(e.id).execute(&state.pool).await?;
    crate::graph::prune_links(&state.pool).await;
    changed(state, e.workspace_id).await;
    Ok(())
}

async fn changed(state: &AppState, workspace_id: Uuid) {
    state.emit(workspace_id, serde_json::json!({ "type": "CALENDAR_EVENTS_UPDATED" })).await;
}

/// Occurrences sur une période, pour la vue Calendrier.
pub(crate) async fn occurrences(pool: &sqlx::PgPool, workspace_id: Uuid, from: DateTime<Utc>, to: DateTime<Utc>, can_edit: bool) -> ApiResult<Vec<serde_json::Value>> {
    let rows: Vec<(Uuid, String, bool)> = sqlx::query_as(
        "SELECT id, ics, recurring FROM workspace_events
         WHERE workspace_id = $1 AND starts_at < $3 AND (series_end IS NULL OR series_end >= $2)
         ORDER BY starts_at LIMIT 2000",
    )
    .bind(workspace_id)
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await?;
    let mut out = Vec::new();
    for (id, ics, recurring) in rows {
        for o in expand(&ics, from, to) {
            out.push(serde_json::json!({
                "id": format!("ws-{id}-{}", o.start.timestamp()),
                "calendar_id": "workspace",
                "event_id": id,
                "recurring": recurring,
                "can_edit": can_edit,
                "summary": o.summary,
                "description": o.description,
                "location": o.location,
                "url": o.url,
                "starts_at": o.start,
                "ends_at": o.end,
                "all_day": o.all_day,
            }));
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Écriture iCalendar
// ---------------------------------------------------------------------------

/// Plie les lignes à 75 octets (RFC 5545) et termine par CRLF.
fn fold(lines: &[String]) -> String {
    let mut out = String::new();
    for line in lines {
        let mut rest = line.as_str();
        let mut first = true;
        while !rest.is_empty() {
            let limit = if first { 75 } else { 74 };
            let mut cut = rest.len().min(limit);
            while !rest.is_char_boundary(cut) {
                cut -= 1;
            }
            if !first {
                out.push(' ');
            }
            out.push_str(&rest[..cut]);
            out.push_str("\r\n");
            rest = &rest[cut..];
            first = false;
        }
    }
    out
}

fn normalize_calendar(ics: &str) -> String {
    fold(&unfolded_lines(ics))
}

fn stamp(t: DateTime<Utc>) -> String {
    t.format("%Y%m%dT%H%M%SZ").to_string()
}

#[derive(Deserialize)]
struct Repeat {
    /// daily | weekly | monthly | yearly
    freq: String,
    interval: Option<u32>,
    /// Dernier jour (inclus), AAAA-MM-JJ.
    until: Option<NaiveDate>,
}

#[derive(Deserialize)]
struct EventReq {
    title: String,
    description: Option<String>,
    location: Option<String>,
    all_day: bool,
    /// Événement horaire : instant RFC 3339 ; journée entière : AAAA-MM-JJ (fin incluse).
    start: String,
    end: String,
    /// Fuseau de l'utilisateur (IANA), pour les séries qui traversent un changement d'heure.
    tz: Option<String>,
    repeat: Option<Repeat>,
    /// Modification : garder la répétition existante (règle non éditable dans le formulaire).
    #[serde(default)]
    keep_recurrence: bool,
}

/// Propriétés produites par le formulaire.
fn form_props(p: &EventReq) -> ApiResult<(Vec<String>, Option<String>)> {
    let title = p.title.trim();
    if title.is_empty() || title.chars().count() > 500 {
        return Err(ApiError::bad_request("event_invalid"));
    }
    let tz = p.tz.as_deref().and_then(|z| chrono_tz::Tz::from_str(z).ok()).unwrap_or(chrono_tz::UTC);
    let mut props = vec![format!("SUMMARY:{}", escape_text(title))];
    let recurring = p.repeat.is_some();
    let (until_value, start_ref);
    if p.all_day {
        let s = NaiveDate::parse_from_str(&p.start, "%Y-%m-%d").map_err(|_| ApiError::bad_request("event_invalid"))?;
        let e = NaiveDate::parse_from_str(&p.end, "%Y-%m-%d").map_err(|_| ApiError::bad_request("event_invalid"))?;
        if e < s || (e - s).num_days() > 366 {
            return Err(ApiError::bad_request("event_invalid"));
        }
        props.push(format!("DTSTART;VALUE=DATE:{}", s.format("%Y%m%d")));
        props.push(format!("DTEND;VALUE=DATE:{}", (e + Duration::days(1)).format("%Y%m%d")));
        until_value = p.repeat.as_ref().and_then(|r| r.until).map(|u| u.format("%Y%m%d").to_string());
        start_ref = s;
    } else {
        let s = DateTime::parse_from_rfc3339(&p.start).map_err(|_| ApiError::bad_request("event_invalid"))?.with_timezone(&Utc);
        let e = DateTime::parse_from_rfc3339(&p.end).map_err(|_| ApiError::bad_request("event_invalid"))?.with_timezone(&Utc);
        if e < s || e - s > Duration::days(366) {
            return Err(ApiError::bad_request("event_invalid"));
        }
        if recurring && tz != chrono_tz::UTC {
            // Série : heure locale + fuseau, pour rester à 9 h après un changement d'heure.
            let fmt = |t: DateTime<Utc>| t.with_timezone(&tz).format("%Y%m%dT%H%M%S").to_string();
            props.push(format!("DTSTART;TZID={}:{}", tz.name(), fmt(s)));
            props.push(format!("DTEND;TZID={}:{}", tz.name(), fmt(e)));
        } else {
            props.push(format!("DTSTART:{}", stamp(s)));
            props.push(format!("DTEND:{}", stamp(e)));
        }
        // UNTIL en UTC : fin du dernier jour, dans le fuseau de l'utilisateur.
        until_value = p.repeat.as_ref().and_then(|r| r.until).and_then(|u| tz.from_local_datetime(&u.and_hms_opt(23, 59, 59)?).earliest()).map(|t| stamp(t.with_timezone(&Utc)));
        start_ref = s.with_timezone(&tz).date_naive();
    }
    let rrule = match &p.repeat {
        None => None,
        Some(r) => {
            let freq = match r.freq.as_str() {
                "daily" => "DAILY",
                "weekly" => "WEEKLY",
                "monthly" => "MONTHLY",
                "yearly" => "YEARLY",
                _ => return Err(ApiError::bad_request("event_invalid")),
            };
            if r.until.is_some_and(|u| u < start_ref) {
                return Err(ApiError::bad_request("event_invalid"));
            }
            let mut rule = format!("RRULE:FREQ={freq}");
            if let Some(i) = r.interval.filter(|i| *i > 1 && *i <= 365) {
                rule.push_str(&format!(";INTERVAL={i}"));
            }
            if let Some(u) = until_value {
                rule.push_str(&format!(";UNTIL={u}"));
            }
            Some(rule)
        }
    };
    if let Some(l) = p.location.as_deref().map(str::trim).filter(|l| !l.is_empty()) {
        props.push(format!("LOCATION:{}", escape_text(&l.chars().take(500).collect::<String>())));
    }
    if let Some(d) = p.description.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
        props.push(format!("DESCRIPTION:{}", escape_text(&d.chars().take(10_000).collect::<String>())));
    }
    Ok((props, rrule))
}

fn prop_name(line: &str) -> String {
    line.split([';', ':']).next().unwrap_or("").trim().to_ascii_uppercase()
}

fn new_ics(uid: &str, props: Vec<String>, rrule: Option<String>) -> String {
    let now = stamp(Utc::now());
    let mut lines = vec![
        "BEGIN:VCALENDAR".to_string(),
        "VERSION:2.0".into(),
        "PRODID:-//Looty//Looty//EN".into(),
        "CALSCALE:GREGORIAN".into(),
        "BEGIN:VEVENT".into(),
        format!("UID:{uid}"),
        format!("DTSTAMP:{now}"),
        format!("CREATED:{now}"),
        format!("LAST-MODIFIED:{now}"),
        "SEQUENCE:0".into(),
    ];
    lines.extend(props);
    lines.extend(rrule);
    lines.push("END:VEVENT".into());
    lines.push("END:VCALENDAR".into());
    fold(&lines)
}

/// Décale les dates exclues (EXDATE) quand l'heure de la série change, pour que les
/// occurrences supprimées restent supprimées.
fn shift_exdate(line: &str, delta: Duration) -> String {
    let Some((head, values)) = line.split_once(':') else { return line.to_string() };
    let shifted: Vec<String> = values
        .split(',')
        .map(|v| {
            let v = v.trim();
            if let Some(z) = v.strip_suffix('Z') {
                if let Ok(n) = chrono::NaiveDateTime::parse_from_str(z, "%Y%m%dT%H%M%S") {
                    return stamp(Utc.from_utc_datetime(&n) + delta);
                }
            } else if v.len() == 8 {
                if let Ok(d) = NaiveDate::parse_from_str(v, "%Y%m%d") {
                    return (d + Duration::days(delta.num_days())).format("%Y%m%d").to_string();
                }
            } else if let Ok(n) = chrono::NaiveDateTime::parse_from_str(v, "%Y%m%dT%H%M%S") {
                // Heure locale (TZID) : même décalage d'horloge.
                return (n + delta).format("%Y%m%dT%H%M%S").to_string();
            }
            v.to_string()
        })
        .collect();
    format!("{head}:{}", shifted.join(","))
}

/// Remplace les champs du formulaire dans l'événement principal et garde le reste.
fn patch_ics(ics: &str, props: Vec<String>, rrule: Option<String>, keep_recurrence: bool) -> String {
    let old_start = master_event(ics).map(|m| m.start);
    let new_start = master_event(&format!("BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\n{}\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n", props.join("\r\n"))).map(|m| m.start);
    let delta = match (old_start, new_start) {
        (Some(a), Some(b)) if keep_recurrence => b - a,
        _ => Duration::zero(),
    };
    let lines = unfolded_lines(ics);
    let mut replaced = vec!["SUMMARY", "DESCRIPTION", "LOCATION", "DTSTART", "DTEND", "DURATION", "DTSTAMP", "LAST-MODIFIED", "SEQUENCE"];
    if !keep_recurrence {
        replaced.extend(["RRULE", "EXDATE", "RDATE"]);
    }
    let mut out = Vec::with_capacity(lines.len() + 4);
    let mut i = 0;
    let mut done = false;
    while i < lines.len() {
        let line = &lines[i];
        if !done && line.eq_ignore_ascii_case("BEGIN:VEVENT") {
            let end = (i..lines.len()).find(|j| lines[*j].eq_ignore_ascii_case("END:VEVENT")).unwrap_or(lines.len() - 1);
            let block = &lines[i + 1..end];
            if !block.iter().any(|l| prop_name(l) == "RECURRENCE-ID") {
                let seq = block.iter().find(|l| prop_name(l) == "SEQUENCE").and_then(|l| l.split(':').nth(1)?.trim().parse::<i64>().ok()).unwrap_or(0);
                let now = stamp(Utc::now());
                out.push(line.clone());
                out.extend(block.iter().filter(|l| !replaced.contains(&prop_name(l).as_str())).map(|l| {
                    if !delta.is_zero() && prop_name(l) == "EXDATE" { shift_exdate(l, delta) } else { l.clone() }
                }));
                out.push(format!("DTSTAMP:{now}"));
                out.push(format!("LAST-MODIFIED:{now}"));
                out.push(format!("SEQUENCE:{}", seq + 1));
                out.extend(props.iter().cloned());
                if !keep_recurrence {
                    out.extend(rrule.iter().cloned());
                }
                out.push(lines[end].clone());
                i = end + 1;
                done = true;
                continue;
            }
        }
        out.push(line.clone());
        i += 1;
    }
    fold(&out)
}

/// Retire une occurrence d'une série (EXDATE dans l'événement principal).
fn exclude_occurrence(ics: &str, occurrence: DateTime<Utc>, all_day: bool) -> String {
    let lines = unfolded_lines(ics);
    let mut out = Vec::with_capacity(lines.len() + 1);
    let mut in_master = false;
    let mut added = false;
    let mut is_override = false;
    for line in lines {
        let name = prop_name(&line);
        if line.eq_ignore_ascii_case("BEGIN:VEVENT") {
            in_master = true;
            is_override = false;
        }
        if in_master && name == "RECURRENCE-ID" {
            is_override = true;
        }
        if in_master && line.eq_ignore_ascii_case("END:VEVENT") {
            if !is_override && !added {
                out.push(if all_day { format!("EXDATE;VALUE=DATE:{}", occurrence.format("%Y%m%d")) } else { format!("EXDATE:{}", stamp(occurrence)) });
                added = true;
            }
            in_master = false;
        }
        out.push(line);
    }
    fold(&out)
}

// ---------------------------------------------------------------------------
// API
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct EventDetail {
    id: Uuid,
    title: String,
    description: Option<String>,
    location: Option<String>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    all_day: bool,
    tzid: Option<String>,
    /// daily | weekly | monthly | yearly, si la règle est simple (éditable dans Looty).
    repeat: Option<serde_json::Value>,
    /// Règle ou exceptions que le formulaire ne sait pas éditer.
    complex_recurrence: bool,
    created_by_name: Option<String>,
    updated_at: DateTime<Utc>,
}

/// Règle simple FREQ[;INTERVAL][;UNTIL] → formulaire ; sinon `None`.
fn simple_repeat(rule: &str, tz: Option<&str>) -> Option<serde_json::Value> {
    let mut freq = None;
    let mut interval = 1;
    let mut until = None;
    for part in rule.split(';') {
        let (k, v) = part.split_once('=')?;
        match k.to_ascii_uppercase().as_str() {
            "FREQ" => freq = Some(v.to_ascii_lowercase()),
            "INTERVAL" => interval = v.parse().ok()?,
            "UNTIL" => {
                let d = if v.len() == 8 {
                    NaiveDate::parse_from_str(v, "%Y%m%d").ok()?
                } else {
                    let t = Utc.from_utc_datetime(&chrono::NaiveDateTime::parse_from_str(v.trim_end_matches('Z'), "%Y%m%dT%H%M%S").ok()?);
                    match tz.and_then(|z| chrono_tz::Tz::from_str(z).ok()) {
                        Some(z) => t.with_timezone(&z).date_naive(),
                        None => t.date_naive(),
                    }
                };
                until = Some(d);
            }
            "WKST" => {}
            _ => return None,
        }
    }
    let freq = freq.filter(|f| ["daily", "weekly", "monthly", "yearly"].contains(&f.as_str()))?;
    Some(serde_json::json!({ "freq": freq, "interval": interval, "until": until }))
}

async fn load(state: &AppState, ws: Uuid, id: Uuid) -> ApiResult<(EventRow, Option<String>, DateTime<Utc>)> {
    let row: Option<(Uuid, Uuid, Option<String>, String, Option<String>, DateTime<Utc>)> = sqlx::query_as(
        "SELECT e.id, e.workspace_id, e.dav_name, e.ics, u.name, e.updated_at FROM workspace_events e
         LEFT JOIN users u ON u.id = e.created_by WHERE e.id = $1 AND e.workspace_id = $2",
    )
    .bind(id)
    .bind(ws)
    .fetch_optional(&state.pool)
    .await?;
    let (id, workspace_id, dav_name, ics, name, updated) = row.ok_or(ApiError::NotFound)?;
    Ok((EventRow { id, workspace_id, dav_name, ics }, name, updated))
}

async fn get_one(State(state): State<AppState>, user: AuthUser, Path((ws, id)): Path<(Uuid, Uuid)>) -> ApiResult<Json<EventDetail>> {
    require_member(&state.pool, &user, ws).await?;
    let (e, created_by_name, updated_at) = load(&state, ws, id).await?;
    let m = master_event(&e.ics).ok_or(ApiError::NotFound)?;
    let repeat = m.rrule.as_deref().and_then(|r| simple_repeat(r, m.tzid.as_deref()));
    Ok(Json(EventDetail {
        id: e.id,
        title: m.summary,
        description: m.description,
        location: m.location,
        start: m.start,
        end: m.end,
        all_day: m.all_day,
        tzid: m.tzid,
        complex_recurrence: (m.rrule.is_some() && repeat.is_none()) || m.has_exceptions,
        repeat,
        created_by_name,
        updated_at,
    }))
}

async fn create(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>, Json(p): Json<EventReq>) -> ApiResult<Json<serde_json::Value>> {
    crate::access::require_perm(&state.pool, &user, ws, |p| p.calendar >= crate::access::level::EDIT).await?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM workspace_events WHERE workspace_id = $1").bind(ws).fetch_one(&state.pool).await?;
    if count >= 20_000 {
        return Err(ApiError::bad_request("event_too_many"));
    }
    let (props, rrule) = form_props(&p)?;
    let uid = format!("{}@looty", Uuid::new_v4());
    let ics = new_ics(&uid, props, rrule);
    let (m, end) = indexed(&ics)?;
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO workspace_events (workspace_id, uid, ics, summary, starts_at, series_end, recurring, created_by, updated_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8) RETURNING id",
    )
    .bind(ws)
    .bind(&uid)
    .bind(&ics)
    .bind(&m.summary)
    .bind(m.start)
    .bind(end)
    .bind(m.rrule.is_some())
    .bind(user.id)
    .fetch_one(&state.pool)
    .await?;
    changed(&state, ws).await;
    Ok(Json(serde_json::json!({ "id": id })))
}

async fn update(State(state): State<AppState>, user: AuthUser, Path((ws, id)): Path<(Uuid, Uuid)>, Json(p): Json<EventReq>) -> ApiResult<StatusCode> {
    crate::access::require_perm(&state.pool, &user, ws, |p| p.calendar >= crate::access::level::EDIT).await?;
    let (e, _, _) = load(&state, ws, id).await?;
    let (props, rrule) = form_props(&p)?;
    let ics = patch_ics(&e.ics, props, rrule, p.keep_recurrence);
    let (m, end) = indexed(&ics)?;
    sqlx::query(
        "UPDATE workspace_events SET ics = $2, summary = $3, starts_at = $4, series_end = $5, recurring = $6, updated_by = $7, updated_at = NOW() WHERE id = $1",
    )
    .bind(id)
    .bind(&ics)
    .bind(&m.summary)
    .bind(m.start)
    .bind(end)
    .bind(m.rrule.is_some())
    .bind(user.id)
    .execute(&state.pool)
    .await?;
    changed(&state, ws).await;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct RemoveQuery {
    /// Début de l'occurrence à retirer d'une série (sinon : tout l'événement).
    occurrence: Option<DateTime<Utc>>,
}

async fn remove(State(state): State<AppState>, user: AuthUser, Path((ws, id)): Path<(Uuid, Uuid)>, Query(q): Query<RemoveQuery>) -> ApiResult<StatusCode> {
    crate::access::require_perm(&state.pool, &user, ws, |p| p.calendar >= crate::access::level::EDIT).await?;
    let (e, _, _) = load(&state, ws, id).await?;
    let m = master_event(&e.ics);
    match (q.occurrence, m) {
        (Some(occ), Some(m)) if m.rrule.is_some() => {
            let ics = exclude_occurrence(&e.ics, occ, m.all_day);
            let end = series_end(&ics);
            sqlx::query("UPDATE workspace_events SET ics = $2, series_end = $3, updated_by = $4, updated_at = NOW() WHERE id = $1")
                .bind(id)
                .bind(&ics)
                .bind(end)
                .bind(user.id)
                .execute(&state.pool)
                .await?;
            changed(&state, ws).await;
        }
        _ => delete_row(&state, &e).await?,
    }
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(json: serde_json::Value) -> EventReq {
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn weekly_meeting_keeps_local_time_and_ends() {
        let p = req(serde_json::json!({
            "title": "Réunion, équipe", "all_day": false, "start": "2026-10-20T07:00:00Z", "end": "2026-10-20T08:00:00Z",
            "tz": "Europe/Paris", "repeat": { "freq": "weekly", "until": "2026-11-10" }
        }));
        let (props, rrule) = form_props(&p).unwrap();
        let ics = new_ics("x@looty", props, rrule);
        assert!(ics.contains("SUMMARY:Réunion\\, équipe"));
        assert!(ics.contains("DTSTART;TZID=Europe/Paris:20261020T090000"));
        let from = Utc.with_ymd_and_hms(2026, 10, 1, 0, 0, 0).unwrap();
        let to = Utc.with_ymd_and_hms(2026, 12, 31, 0, 0, 0).unwrap();
        let occ = expand(&ics, from, to);
        assert_eq!(occ.len(), 4, "{occ:#?}");
        // Après le passage à l'heure d'hiver (25 octobre), toujours 9 h à Paris = 8 h UTC.
        assert_eq!(occ[1].start, Utc.with_ymd_and_hms(2026, 10, 27, 8, 0, 0).unwrap());
        assert_eq!(series_end(&ics), Some(Utc.with_ymd_and_hms(2026, 11, 10, 9, 0, 0).unwrap()));
    }

    #[test]
    fn patch_keeps_unknown_properties_and_exclude_occurrence() {
        let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:a\r\nSUMMARY:Old\r\nDTSTART;VALUE=DATE:20261001\r\nRRULE:FREQ=DAILY;COUNT=3\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nTRIGGER:-PT15M\r\nEND:VALARM\r\nX-CUSTOM:keep\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let p = req(serde_json::json!({ "title": "New", "all_day": true, "start": "2026-10-01", "end": "2026-10-01", "keep_recurrence": true }));
        let (props, rrule) = form_props(&p).unwrap();
        let out = patch_ics(ics, props, rrule, true);
        assert!(out.contains("SUMMARY:New") && !out.contains("SUMMARY:Old"));
        assert!(out.contains("X-CUSTOM:keep") && out.contains("TRIGGER:-PT15M") && out.contains("RRULE:FREQ=DAILY;COUNT=3"));
        assert!(out.contains("SEQUENCE:1"));
        let from = Utc.with_ymd_and_hms(2026, 9, 1, 0, 0, 0).unwrap();
        let to = Utc.with_ymd_and_hms(2026, 12, 1, 0, 0, 0).unwrap();
        assert_eq!(expand(&out, from, to).len(), 3);
        let ex = exclude_occurrence(&out, Utc.with_ymd_and_hms(2026, 10, 2, 0, 0, 0).unwrap(), true);
        assert_eq!(expand(&ex, from, to).len(), 2);
        // Série décalée d'un jour : l'occurrence exclue reste exclue.
        let p = req(serde_json::json!({ "title": "New", "all_day": true, "start": "2026-10-02", "end": "2026-10-02", "keep_recurrence": true }));
        let (props, rrule) = form_props(&p).unwrap();
        let moved = patch_ics(&ex, props, rrule, true);
        let occ = expand(&moved, from, to);
        assert_eq!(occ.len(), 2, "{moved}");
        // 2 et 4 octobre (le 3 reste exclu) : deux jours d'écart.
        assert_eq!(occ[1].start - occ[0].start, Duration::days(2), "{moved}");
    }

    #[test]
    fn simple_repeat_round_trip() {
        assert_eq!(simple_repeat("FREQ=WEEKLY;INTERVAL=2", None).unwrap()["interval"], 2);
        assert!(simple_repeat("FREQ=WEEKLY;BYDAY=MO,WE", None).is_none());
        assert_eq!(simple_repeat("FREQ=MONTHLY;UNTIL=20261231T225959Z", Some("Europe/Paris")).unwrap()["until"], "2026-12-31");
    }
}

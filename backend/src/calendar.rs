//! Calendriers intégrés, en lecture seule.
//!
//! Un calendrier est un abonnement ICS (lien `https://` ou `webcal://`, par ex. l'emploi
//! du temps de l'université) ou un calendrier CalDAV. Il est soit partagé par un espace
//! de travail (géré par ses administrateurs, visible de tous ses membres), soit
//! personnel (visible de son seul propriétaire, dans tous ses espaces).
//!
//! Synchronisation toutes les 15 minutes : les événements sont lus, les récurrences
//! développées sur une fenêtre de -6 mois à +13 mois, et stockés par occurrence.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post, put},
    Json, Router,
};
use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use uuid::Uuid;

use crate::access::{require_member, require_ws_admin, AuthUser};
use crate::caldav::{check_url, parse_multistatus, Dav};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Lecture iCalendar (RFC 5545)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Prop {
    name: String,
    params: Vec<(String, String)>,
    value: String,
}

impl Prop {
    fn param(&self, key: &str) -> Option<&str> {
        self.params.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v.as_str())
    }
}

#[derive(Debug, Default)]
struct Comp {
    name: String,
    props: Vec<Prop>,
    children: Vec<Comp>,
}

impl Comp {
    fn prop(&self, name: &str) -> Option<&Prop> {
        self.props.iter().find(|p| p.name == name)
    }
    fn all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Prop> + 'a {
        self.props.iter().filter(move |p| p.name == name)
    }
}

fn unfold(ics: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for raw in ics.split('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if (line.starts_with(' ') || line.starts_with('\t')) && !out.is_empty() {
            out.last_mut().unwrap().push_str(&line[1..]);
        } else if !line.is_empty() {
            out.push(line.to_string());
        }
    }
    out
}

fn parse_prop(line: &str) -> Option<Prop> {
    // Le premier « : » hors guillemets sépare nom/paramètres et valeur.
    let mut in_quotes = false;
    let colon = line.char_indices().find(|(_, c)| {
        if *c == '"' {
            in_quotes = !in_quotes;
        }
        *c == ':' && !in_quotes
    })?.0;
    let (head, value) = (&line[..colon], &line[colon + 1..]);
    let mut parts = head.split(';');
    let name = parts.next()?.trim().to_ascii_uppercase();
    let params = parts
        .filter_map(|p| p.split_once('='))
        .map(|(k, v)| (k.trim().to_ascii_uppercase(), v.trim().trim_matches('"').to_string()))
        .collect();
    Some(Prop { name, params, value: value.to_string() })
}

fn parse_ics(ics: &str) -> Vec<Comp> {
    let mut stack: Vec<Comp> = Vec::new();
    let mut roots = Vec::new();
    for line in unfold(ics) {
        let Some(p) = parse_prop(&line) else { continue };
        match p.name.as_str() {
            "BEGIN" => stack.push(Comp { name: p.value.trim().to_ascii_uppercase(), ..Default::default() }),
            "END" => {
                if let Some(c) = stack.pop() {
                    match stack.last_mut() {
                        Some(parent) => parent.children.push(c),
                        None => roots.push(c),
                    }
                }
            }
            _ => {
                if let Some(c) = stack.last_mut() {
                    c.props.push(p);
                }
            }
        }
    }
    roots
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') | Some('N') => out.push('\n'),
                Some(o) => out.push(o),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Fuseaux Windows (Outlook / Exchange) les plus courants → noms IANA.
fn windows_zone(name: &str) -> Option<&'static str> {
    Some(match name {
        "Romance Standard Time" => "Europe/Paris",
        "W. Europe Standard Time" => "Europe/Berlin",
        "Central Europe Standard Time" => "Europe/Budapest",
        "Central European Standard Time" => "Europe/Warsaw",
        "GMT Standard Time" => "Europe/London",
        "Greenwich Standard Time" => "Atlantic/Reykjavik",
        "E. Europe Standard Time" => "Europe/Chisinau",
        "FLE Standard Time" => "Europe/Kiev",
        "GTB Standard Time" => "Europe/Bucharest",
        "Russian Standard Time" => "Europe/Moscow",
        "Eastern Standard Time" => "America/New_York",
        "Central Standard Time" => "America/Chicago",
        "Mountain Standard Time" => "America/Denver",
        "Pacific Standard Time" => "America/Los_Angeles",
        "Atlantic Standard Time" => "America/Halifax",
        "Morocco Standard Time" => "Africa/Casablanca",
        "UTC" | "Coordinated Universal Time" => "UTC",
        _ => return None,
    })
}

/// TZID (IANA, Windows, ou préfixé « /mozilla.org/… ») → fuseau.
fn resolve_tz(tzid: &str) -> Option<chrono_tz::Tz> {
    let t = tzid.trim().trim_matches('"');
    if let Ok(tz) = chrono_tz::Tz::from_str(t) {
        return Some(tz);
    }
    if let Some(w) = windows_zone(t) {
        return chrono_tz::Tz::from_str(w).ok();
    }
    // « /freeassociation.sourceforge.net/Europe/Paris », « Europe/Paris_1 »…
    let parts: Vec<&str> = t.split('/').filter(|s| !s.is_empty()).collect();
    for i in 0..parts.len() {
        if let Ok(tz) = chrono_tz::Tz::from_str(&parts[i..].join("/")) {
            return Some(tz);
        }
    }
    None
}

fn default_tz() -> chrono_tz::Tz {
    std::env::var("CALENDAR_DEFAULT_TZ").ok().and_then(|z| chrono_tz::Tz::from_str(&z).ok()).unwrap_or(chrono_tz::UTC)
}

#[derive(Debug, Clone, Copy)]
enum When {
    Date(NaiveDate),
    At(DateTime<Utc>),
}

impl When {
    fn utc(&self) -> DateTime<Utc> {
        match self {
            When::Date(d) => Utc.from_utc_datetime(&d.and_hms_opt(0, 0, 0).unwrap()),
            When::At(t) => *t,
        }
    }
}

fn parse_when(p: &Prop) -> Option<When> {
    let v = p.value.trim();
    if p.param("VALUE").is_some_and(|x| x.eq_ignore_ascii_case("DATE")) || (v.len() == 8 && !v.contains('T')) {
        return NaiveDate::parse_from_str(&v[..8.min(v.len())], "%Y%m%d").ok().map(When::Date);
    }
    if let Some(z) = v.strip_suffix('Z') {
        let n = NaiveDateTime::parse_from_str(z, "%Y%m%dT%H%M%S").ok()?;
        return Some(When::At(Utc.from_utc_datetime(&n)));
    }
    let n = NaiveDateTime::parse_from_str(v, "%Y%m%dT%H%M%S").ok()?;
    let tz = p.param("TZID").and_then(resolve_tz).unwrap_or_else(default_tz);
    let local = tz.from_local_datetime(&n).earliest()?;
    Some(When::At(local.with_timezone(&Utc)))
}

/// DURATION (ex. PT1H30M, P1D, -PT15M).
fn parse_duration(s: &str) -> Option<Duration> {
    let s = s.trim();
    let (neg, s) = match s.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    let s = s.strip_prefix('P')?;
    let (mut total, mut num, mut in_time) = (0i64, String::new(), false);
    for c in s.chars() {
        match c {
            'T' => in_time = true,
            d if d.is_ascii_digit() => num.push(d),
            unit => {
                let n: i64 = num.parse().ok()?;
                num.clear();
                total += n * match (unit, in_time) {
                    ('W', _) => 604800,
                    ('D', _) => 86400,
                    ('H', true) => 3600,
                    ('M', true) => 60,
                    ('S', true) => 1,
                    _ => return None,
                };
            }
        }
    }
    Some(Duration::seconds(if neg { -total } else { total }))
}

#[derive(Debug, Clone, PartialEq)]
pub struct Occurrence {
    pub uid: String,
    pub summary: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub url: Option<String>,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub all_day: bool,
}

/// Ligne iCalendar d'une date pour la bibliothèque rrule (fuseau normalisé en IANA).
fn rrule_line(name: &str, p: &Prop) -> Option<String> {
    let v = p.value.trim();
    if let Some(tzid) = p.param("TZID") {
        let tz = resolve_tz(tzid).unwrap_or_else(default_tz);
        return Some(format!("{name};TZID={}:{v}", tz.name()));
    }
    if p.param("VALUE").is_some_and(|x| x.eq_ignore_ascii_case("DATE")) || (v.len() == 8 && !v.contains('T')) {
        return Some(format!("{name};VALUE=DATE:{v}"));
    }
    if v.ends_with('Z') {
        return Some(format!("{name}:{v}"));
    }
    Some(format!("{name};TZID={}:{v}", default_tz().name()))
}

/// Événement principal d'un objet iCalendar (hors exceptions RECURRENCE-ID).
#[derive(Debug, Clone)]
pub(crate) struct MasterEvent {
    pub uid: String,
    pub summary: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub all_day: bool,
    /// Valeur brute de RRULE (série), le cas échéant.
    pub rrule: Option<String>,
    /// Fuseau de DTSTART (IANA), pour l'affichage dans le formulaire.
    pub tzid: Option<String>,
    /// La série a des exceptions (EXDATE, RDATE, occurrences modifiées).
    pub has_exceptions: bool,
}

pub(crate) fn master_event(ics: &str) -> Option<MasterEvent> {
    let roots = parse_ics(ics);
    let events: Vec<&Comp> = roots.iter().flat_map(|r| r.children.iter().filter(|c| c.name == "VEVENT")).collect();
    let e = events.iter().find(|e| e.prop("RECURRENCE-ID").is_none())?;
    let dtstart = e.prop("DTSTART")?;
    let start = parse_when(dtstart)?;
    let all_day = matches!(start, When::Date(_));
    let dur = e
        .prop("DTEND")
        .and_then(parse_when)
        .map(|end| end.utc() - start.utc())
        .or_else(|| e.prop("DURATION").and_then(|d| parse_duration(&d.value)))
        .unwrap_or_else(|| if all_day { Duration::days(1) } else { Duration::zero() })
        .max(Duration::zero());
    let text = |n: &str| e.prop(n).map(|p| unescape(&p.value)).filter(|s| !s.trim().is_empty());
    Some(MasterEvent {
        uid: e.prop("UID").map(|p| p.value.trim().to_string()).unwrap_or_default(),
        summary: text("SUMMARY").unwrap_or_default(),
        description: text("DESCRIPTION"),
        location: text("LOCATION"),
        start: start.utc(),
        end: start.utc() + dur,
        all_day,
        rrule: e.prop("RRULE").map(|p| p.value.trim().to_string()),
        tzid: dtstart.param("TZID").and_then(resolve_tz).map(|t| t.name().to_string()),
        has_exceptions: e.prop("EXDATE").is_some() || e.prop("RDATE").is_some() || events.len() > 1,
    })
}

/// Fin de la dernière occurrence ; `None` pour une série sans fin.
pub(crate) fn series_end(ics: &str) -> Option<DateTime<Utc>> {
    let m = master_event(ics)?;
    let Some(rule) = m.rrule.as_deref() else { return Some(m.end) };
    let upper = rule.to_ascii_uppercase();
    if !upper.contains("COUNT=") && !upper.contains("UNTIL=") {
        return None;
    }
    let to = m.start + Duration::days(366 * 30);
    let last = expand(ics, m.start - Duration::days(1), to).into_iter().map(|o| o.end).max();
    Some(last.unwrap_or(m.end).max(m.end))
}

/// Échappement d'un texte iCalendar (SUMMARY, DESCRIPTION, LOCATION).
pub(crate) fn escape_text(s: &str) -> String {
    s.replace('\\', "\\\\").replace(';', "\\;").replace(',', "\\,").replace("\r\n", "\\n").replace('\n', "\\n")
}

/// Lignes dépliées (pour retoucher un objet iCalendar en conservant le reste).
pub(crate) fn unfolded_lines(ics: &str) -> Vec<String> {
    unfold(ics)
}

/// Développe les VEVENT d'un ou plusieurs VCALENDAR sur la fenêtre donnée.
pub fn expand(ics: &str, from: DateTime<Utc>, to: DateTime<Utc>) -> Vec<Occurrence> {
    let mut events: Vec<&Comp> = Vec::new();
    let roots = parse_ics(ics);
    for root in &roots {
        events.extend(root.children.iter().filter(|c| c.name == "VEVENT"));
    }
    // Exceptions (RECURRENCE-ID) : remplacent ou annulent une occurrence de la série.
    let mut overrides: HashMap<(String, i64), &Comp> = HashMap::new();
    for e in &events {
        if let (Some(uid), Some(rid)) = (e.prop("UID"), e.prop("RECURRENCE-ID").and_then(parse_when)) {
            overrides.insert((uid.value.clone(), rid.utc().timestamp()), *e);
        }
    }
    let make = |e: &Comp, start: DateTime<Utc>, dur: Duration, all_day: bool| -> Option<Occurrence> {
        if e.prop("STATUS").is_some_and(|s| s.value.eq_ignore_ascii_case("CANCELLED")) {
            return None;
        }
        let text = |n: &str| e.prop(n).map(|p| unescape(&p.value)).filter(|s| !s.trim().is_empty());
        Some(Occurrence {
            uid: e.prop("UID").map(|p| p.value.clone()).unwrap_or_default(),
            summary: text("SUMMARY").unwrap_or_default(),
            description: text("DESCRIPTION").map(|d| d.chars().take(4000).collect()),
            location: text("LOCATION"),
            url: e.prop("URL").map(|p| p.value.trim().to_string()).filter(|u| u.starts_with("https://") || u.starts_with("http://")),
            start,
            end: start + dur,
            all_day,
        })
    };
    let mut out = Vec::new();
    for e in &events {
        if e.prop("RECURRENCE-ID").is_some() {
            continue;
        }
        let Some(dtstart_p) = e.prop("DTSTART") else { continue };
        let Some(start) = parse_when(dtstart_p) else { continue };
        let all_day = matches!(start, When::Date(_));
        let dur = e
            .prop("DTEND")
            .and_then(parse_when)
            .map(|end| end.utc() - start.utc())
            .or_else(|| e.prop("DURATION").and_then(|d| parse_duration(&d.value)))
            .unwrap_or_else(|| if all_day { Duration::days(1) } else { Duration::zero() })
            .max(Duration::zero());
        let uid = e.prop("UID").map(|p| p.value.clone()).unwrap_or_default();

        let Some(rule) = e.prop("RRULE") else {
            let s = start.utc();
            if s < to && s + dur > from {
                if let Some(o) = make(e, s, dur, all_day) {
                    out.push(o);
                }
            }
            continue;
        };
        // Série : DTSTART + RRULE + EXDATE/RDATE, développée par la bibliothèque rrule.
        let mut lines = vec![rrule_line("DTSTART", dtstart_p).unwrap_or_default(), format!("RRULE:{}", rule.value.trim())];
        for ex in e.all("EXDATE") {
            for v in ex.value.split(',') {
                let one = Prop { name: "EXDATE".into(), params: ex.params.clone(), value: v.to_string() };
                if let Some(l) = rrule_line("EXDATE", &one) {
                    lines.push(l);
                }
            }
        }
        for rd in e.all("RDATE").filter(|p| !p.param("VALUE").is_some_and(|v| v.eq_ignore_ascii_case("PERIOD"))) {
            for v in rd.value.split(',') {
                let one = Prop { name: "RDATE".into(), params: rd.params.clone(), value: v.to_string() };
                if let Some(l) = rrule_line("RDATE", &one) {
                    lines.push(l);
                }
            }
        }
        let Ok(set) = rrule::RRuleSet::from_str(&lines.join("\n")) else {
            tracing::debug!("calendar: unsupported recurrence for {uid}");
            continue;
        };
        let after = (from - dur).with_timezone(&rrule::Tz::UTC);
        let before = to.with_timezone(&rrule::Tz::UTC);
        let result = set.after(after).before(before).all(3000);
        for d in result.dates {
            let s = d.with_timezone(&Utc);
            let key = (uid.clone(), s.timestamp());
            if let Some(ov) = overrides.get(&key) {
                // Occurrence modifiée : ses propres dates et textes.
                let Some(os) = ov.prop("DTSTART").and_then(parse_when) else { continue };
                let odur = ov
                    .prop("DTEND")
                    .and_then(parse_when)
                    .map(|end| end.utc() - os.utc())
                    .or_else(|| ov.prop("DURATION").and_then(|d| parse_duration(&d.value)))
                    .unwrap_or(dur);
                if let Some(o) = make(ov, os.utc(), odur, matches!(os, When::Date(_))) {
                    out.push(o);
                }
                continue;
            }
            if let Some(o) = make(e, s, dur, all_day) {
                out.push(o);
            }
        }
    }
    // Exceptions déplacées hors de la série d'origine (nouvelle date dans la fenêtre).
    out.sort_by_key(|o| o.start);
    out.dedup_by(|a, b| a.uid == b.uid && a.start == b.start);
    out
}

// ---------------------------------------------------------------------------
// Synchronisation
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct CalRow {
    id: Uuid,
    kind: String,
    url_enc: String,
    username: Option<String>,
    password_enc: Option<String>,
    etag: Option<String>,
}

fn window() -> (DateTime<Utc>, DateTime<Utc>) {
    let now = Utc::now();
    (now - Duration::days(183), now + Duration::days(400))
}

/// Adresse saisie → URL https (les liens webcal:// sont des liens https).
fn normalize_url(raw: &str) -> String {
    let t = raw.trim();
    match t.strip_prefix("webcal://").or_else(|| t.strip_prefix("webcals://")) {
        Some(rest) => format!("https://{rest}"),
        None => t.to_string(),
    }
}

/// Télécharge le contenu iCalendar ; `None` si inchangé depuis la dernière fois (ETag).
async fn fetch(cal: &CalRow) -> ApiResult<Option<(String, Option<String>)>> {
    let url_s = crate::crypto::decrypt(&cal.url_enc).ok_or_else(|| ApiError::bad_request("calendar_secret_unreadable"))?;
    let url = check_url(&url_s).await?;
    let pass = cal.password_enc.as_deref().and_then(crate::crypto::decrypt).unwrap_or_default();
    let dav = Dav { user: cal.username.clone().unwrap_or_default(), pass };
    if cal.kind == "ics" {
        let mut extra = vec![];
        if let Some(e) = &cal.etag {
            extra.push((reqwest::header::IF_NONE_MATCH, e.clone()));
        }
        let r = dav.send("GET", &url, None, None, &extra).await?;
        if r.status == reqwest::StatusCode::NOT_MODIFIED {
            return Ok(None);
        }
        if r.status == reqwest::StatusCode::UNAUTHORIZED || r.status == reqwest::StatusCode::FORBIDDEN {
            return Err(ApiError::bad_request("calendar_auth_failed"));
        }
        if !r.status.is_success() {
            return Err(ApiError::BadRequest(format!("calendar_http_error: {}", r.status.as_u16())));
        }
        if !r.body.contains("BEGIN:VCALENDAR") {
            return Err(ApiError::bad_request("calendar_not_ics"));
        }
        return Ok(Some((r.body, r.etag)));
    }
    // CalDAV : requête calendar-query limitée à la fenêtre synchronisée.
    let (from, to) = window();
    let body = format!(
        r#"<?xml version="1.0" encoding="utf-8"?><c:calendar-query xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav"><d:prop><c:calendar-data/></d:prop><c:filter><c:comp-filter name="VCALENDAR"><c:comp-filter name="VEVENT"><c:time-range start="{}" end="{}"/></c:comp-filter></c:comp-filter></c:filter></c:calendar-query>"#,
        from.format("%Y%m%dT%H%M%SZ"),
        to.format("%Y%m%dT%H%M%SZ")
    );
    let r = dav.send("REPORT", &url, Some("1"), Some(body), &[(reqwest::header::CONTENT_TYPE, "application/xml; charset=utf-8".into())]).await?;
    if r.status == reqwest::StatusCode::UNAUTHORIZED || r.status == reqwest::StatusCode::FORBIDDEN {
        return Err(ApiError::bad_request("calendar_auth_failed"));
    }
    if r.status.as_u16() != 207 {
        return Err(ApiError::BadRequest(format!("calendar_http_error: {}", r.status.as_u16())));
    }
    let data: Vec<String> = parse_multistatus(&r.body)?.into_iter().filter_map(|m| m.props.get("calendar-data").cloned()).filter(|d| d.contains("BEGIN:VCALENDAR")).collect();
    Ok(Some((data.join("\r\n"), None)))
}

pub async fn sync_calendar(state: &AppState, id: Uuid) -> ApiResult<usize> {
    let pool = &state.pool;
    let cal: CalRow = sqlx::query_as("SELECT id, kind, url_enc, username, password_enc, etag FROM calendars WHERE id = $1").bind(id).fetch_one(pool).await?;
    let result = async {
        let Some((ics, etag)) = fetch(&cal).await? else { return Ok::<Option<usize>, ApiError>(None) };
        let (from, to) = window();
        let occ = expand(&ics, from, to);
        let mut tx = pool.begin().await?;
        sqlx::query("DELETE FROM calendar_events WHERE calendar_id = $1").bind(cal.id).execute(&mut *tx).await?;
        for chunk in occ.chunks(500) {
            let uids: Vec<String> = chunk.iter().map(|o| o.uid.clone()).collect();
            let summaries: Vec<String> = chunk.iter().map(|o| o.summary.chars().take(500).collect()).collect();
            let descs: Vec<Option<String>> = chunk.iter().map(|o| o.description.clone()).collect();
            let locs: Vec<Option<String>> = chunk.iter().map(|o| o.location.as_ref().map(|l| l.chars().take(500).collect())).collect();
            let urls: Vec<Option<String>> = chunk.iter().map(|o| o.url.clone()).collect();
            let starts: Vec<DateTime<Utc>> = chunk.iter().map(|o| o.start).collect();
            let ends: Vec<DateTime<Utc>> = chunk.iter().map(|o| o.end).collect();
            let days: Vec<bool> = chunk.iter().map(|o| o.all_day).collect();
            sqlx::query(
                "INSERT INTO calendar_events (calendar_id, uid, summary, description, location, url, starts_at, ends_at, all_day)
                 SELECT $1, * FROM UNNEST($2::text[], $3::text[], $4::text[], $5::text[], $6::text[], $7::timestamptz[], $8::timestamptz[], $9::bool[])",
            )
            .bind(cal.id)
            .bind(uids)
            .bind(summaries)
            .bind(descs)
            .bind(locs)
            .bind(urls)
            .bind(starts)
            .bind(ends)
            .bind(days)
            .execute(&mut *tx)
            .await?;
        }
        sqlx::query("UPDATE calendars SET etag = $2, event_count = $3 WHERE id = $1").bind(cal.id).bind(etag).bind(occ.len() as i32).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(Some(occ.len()))
    }
    .await;
    match &result {
        Ok(_) => {
            sqlx::query("UPDATE calendars SET last_sync_at = NOW(), last_error = NULL WHERE id = $1").bind(cal.id).execute(pool).await?;
        }
        Err(e) => {
            let msg = match e {
                ApiError::BadRequest(m) | ApiError::Conflict(m) | ApiError::Internal(m) => m.clone(),
                other => format!("{other:?}"),
            };
            sqlx::query("UPDATE calendars SET last_sync_at = NOW(), last_error = $2 WHERE id = $1").bind(cal.id).bind(msg.chars().take(500).collect::<String>()).execute(pool).await?;
        }
    }
    result.map(|n| n.unwrap_or(0))
}

/// Boucle de fond : chaque calendrier est rafraîchi au plus tous les quarts d'heure.
pub fn start_sync_loop(state: AppState) {
    tokio::spawn(async move {
        loop {
            let due: Vec<Uuid> = sqlx::query_scalar(
                "SELECT id FROM calendars WHERE enabled AND (last_sync_at IS NULL OR last_sync_at < NOW() - interval '15 minutes')
                 ORDER BY last_sync_at NULLS FIRST LIMIT 20",
            )
            .fetch_all(&state.pool)
            .await
            .unwrap_or_default();
            for id in due {
                if let Err(e) = sync_calendar(&state, id).await {
                    tracing::debug!("calendar {id}: {e:?}");
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        }
    });
}

// ---------------------------------------------------------------------------
// API
// ---------------------------------------------------------------------------

pub fn workspace_routes() -> Router<AppState> {
    Router::new().route("/", get(list).post(create)).route("/events", get(events))
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/:id", put(update).delete(remove)).route("/:id/sync", post(sync_now))
}

#[derive(Serialize, sqlx::FromRow)]
struct CalendarDto {
    id: Uuid,
    name: String,
    color: String,
    kind: String,
    scope: String,
    url_host: String,
    username: Option<String>,
    enabled: bool,
    last_sync_at: Option<DateTime<Utc>>,
    last_error: Option<String>,
    event_count: i32,
    can_manage: bool,
}

const DTO_SELECT: &str = "SELECT c.id, c.name, c.color, c.kind,
        CASE WHEN c.workspace_id IS NULL THEN 'personal' ELSE 'workspace' END AS scope,
        c.url_host, c.username, c.enabled, c.last_sync_at, c.last_error, c.event_count,
        (COALESCE(c.user_id = $2, FALSE) OR $3) AS can_manage
     FROM calendars c";

async fn list(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>) -> ApiResult<Json<Vec<CalendarDto>>> {
    let role = require_member(&state.pool, &user, ws).await?;
    let mut rows: Vec<CalendarDto> = sqlx::query_as(&format!("{DTO_SELECT} WHERE c.workspace_id = $1 OR c.user_id = $2 ORDER BY c.workspace_id NULLS LAST, c.name"))
        .bind(ws)
        .bind(user.id)
        .bind(role.is_admin())
        .fetch_all(&state.pool)
        .await?;
    // Identifiant du compte CalDAV : réservé à ceux qui gèrent le calendrier.
    for r in rows.iter_mut().filter(|r| !r.can_manage) {
        r.username = None;
    }
    Ok(Json(rows))
}

#[derive(Deserialize)]
struct CreateReq {
    scope: String,
    name: String,
    color: Option<String>,
    kind: String,
    url: String,
    username: Option<String>,
    password: Option<String>,
}

fn clean_color(c: Option<String>) -> String {
    c.filter(|c| c.len() <= 9 && c.starts_with('#') && c[1..].chars().all(|x| x.is_ascii_hexdigit())).unwrap_or_else(|| "#5e6ad2".into())
}

fn clean_name(n: &str) -> ApiResult<String> {
    let n = n.trim();
    if n.is_empty() || n.chars().count() > 120 {
        return Err(ApiError::bad_request("invalid_name"));
    }
    Ok(n.to_string())
}

async fn create(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>, Json(p): Json<CreateReq>) -> ApiResult<Json<CalendarDto>> {
    let pool = &state.pool;
    let role = require_member(pool, &user, ws).await?;
    let (workspace_id, user_id) = match p.scope.as_str() {
        "workspace" => {
            require_ws_admin(pool, &user, ws).await?;
            (Some(ws), None)
        }
        "personal" => (None, Some(user.id)),
        _ => return Err(ApiError::bad_request("invalid_scope")),
    };
    if !matches!(p.kind.as_str(), "ics" | "caldav") {
        return Err(ApiError::bad_request("invalid_kind"));
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM calendars WHERE workspace_id IS NOT DISTINCT FROM $1 AND user_id IS NOT DISTINCT FROM $2")
        .bind(workspace_id)
        .bind(user_id)
        .fetch_one(pool)
        .await?;
    if count >= 30 {
        return Err(ApiError::bad_request("calendar_too_many"));
    }
    let url = check_url(&normalize_url(&p.url)).await?;
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO calendars (workspace_id, user_id, name, color, kind, url_enc, url_host, username, password_enc, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) RETURNING id",
    )
    .bind(workspace_id)
    .bind(user_id)
    .bind(clean_name(&p.name)?)
    .bind(clean_color(p.color))
    .bind(&p.kind)
    .bind(crate::crypto::encrypt(url.as_str()))
    .bind(url.host_str().unwrap_or("").chars().take(255).collect::<String>())
    .bind(p.username.as_deref().map(str::trim).filter(|u| !u.is_empty()))
    .bind(p.password.as_deref().filter(|x| !x.is_empty()).map(crate::crypto::encrypt))
    .bind(user.id)
    .fetch_one(pool)
    .await?;
    // Première synchronisation tout de suite : une erreur (lien invalide…) est visible.
    let _ = sync_calendar(&state, id).await;
    let dto: CalendarDto = sqlx::query_as(&format!("{DTO_SELECT} WHERE c.id = $1")).bind(id).bind(user.id).bind(role.is_admin()).fetch_one(pool).await?;
    Ok(Json(dto))
}

/// Gestion d'un calendrier : son propriétaire, ou un administrateur de son espace.
async fn require_manage(state: &AppState, user: &AuthUser, id: Uuid) -> ApiResult<bool> {
    let row: Option<(Option<Uuid>, Option<Uuid>)> = sqlx::query_as("SELECT workspace_id, user_id FROM calendars WHERE id = $1").bind(id).fetch_optional(&state.pool).await?;
    match row.ok_or(ApiError::NotFound)? {
        (_, Some(owner)) if owner == user.id => Ok(false),
        (Some(ws), _) => {
            require_ws_admin(&state.pool, user, ws).await?;
            Ok(true)
        }
        _ => Err(ApiError::NotFound),
    }
}

#[derive(Deserialize)]
struct UpdateReq {
    name: Option<String>,
    color: Option<String>,
    enabled: Option<bool>,
    url: Option<String>,
    username: Option<String>,
    password: Option<String>,
}

async fn update(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(p): Json<UpdateReq>) -> ApiResult<StatusCode> {
    require_manage(&state, &user, id).await?;
    let pool = &state.pool;
    let name = p.name.as_deref().map(clean_name).transpose()?;
    let url = match &p.url {
        Some(u) if !u.trim().is_empty() => Some(check_url(&normalize_url(u)).await?),
        _ => None,
    };
    sqlx::query(
        "UPDATE calendars SET name = COALESCE($2, name), color = COALESCE($3, color), enabled = COALESCE($4, enabled),
            url_enc = COALESCE($5, url_enc), url_host = COALESCE($6, url_host),
            username = CASE WHEN $7 THEN $8 ELSE username END,
            password_enc = CASE WHEN $9 THEN $10 ELSE password_enc END,
            etag = CASE WHEN $5 IS NOT NULL THEN NULL ELSE etag END
         WHERE id = $1",
    )
    .bind(id)
    .bind(name)
    .bind(p.color.map(|c| clean_color(Some(c))))
    .bind(p.enabled)
    .bind(url.as_ref().map(|u| crate::crypto::encrypt(u.as_str())))
    .bind(url.as_ref().and_then(|u| u.host_str().map(str::to_string)))
    .bind(p.username.is_some())
    .bind(p.username.as_deref().map(str::trim).filter(|u| !u.is_empty()))
    .bind(p.password.is_some())
    .bind(p.password.as_deref().filter(|x| !x.is_empty()).map(crate::crypto::encrypt))
    .execute(pool)
    .await?;
    if url.is_some() || p.password.is_some() {
        let _ = sync_calendar(&state, id).await;
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn remove(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<StatusCode> {
    require_manage(&state, &user, id).await?;
    sqlx::query("DELETE FROM calendars WHERE id = $1").bind(id).execute(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn sync_now(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<Json<serde_json::Value>> {
    require_manage(&state, &user, id).await?;
    let n = sync_calendar(&state, id).await?;
    Ok(Json(serde_json::json!({ "events": n })))
}

#[derive(Deserialize)]
struct RangeQuery {
    from: DateTime<Utc>,
    to: DateTime<Utc>,
}

#[derive(Serialize, sqlx::FromRow)]
struct EventDto {
    id: Uuid,
    calendar_id: Uuid,
    summary: String,
    description: Option<String>,
    location: Option<String>,
    url: Option<String>,
    starts_at: DateTime<Utc>,
    ends_at: DateTime<Utc>,
    all_day: bool,
}

#[derive(Serialize, sqlx::FromRow)]
struct TaskDue {
    id: Uuid,
    title: String,
    due_date: DateTime<Utc>,
    is_completed: bool,
    board_id: Uuid,
    board_name: String,
}

/// Événements des calendriers visibles (espace + personnels) et tâches qui me sont
/// assignées, sur une période d'au plus 400 jours.
async fn events(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>, Query(q): Query<RangeQuery>) -> ApiResult<Json<serde_json::Value>> {
    let (_, perms) = crate::access::member_perms(&state.pool, user.id, ws).await?.ok_or(ApiError::NotFound)?;
    if q.to <= q.from || q.to - q.from > Duration::days(400) {
        return Err(ApiError::bad_request("invalid_range"));
    }
    // Calendrier de l'espace masqué par le rôle : seulement les calendriers personnels.
    let shared = if perms.calendar >= crate::access::level::READ {
        crate::events::occurrences(&state.pool, ws, q.from, q.to, perms.calendar >= crate::access::level::EDIT).await?
    } else {
        Vec::new()
    };
    let ws_cals = perms.calendar >= crate::access::level::READ;
    let events: Vec<EventDto> = sqlx::query_as(
        "SELECT e.id, e.calendar_id, e.summary, e.description, e.location, e.url, e.starts_at, e.ends_at, e.all_day
         FROM calendar_events e JOIN calendars c ON c.id = e.calendar_id
         WHERE c.enabled AND ((c.workspace_id = $1 AND $5) OR c.user_id = $2) AND e.starts_at < $4 AND e.ends_at >= $3
         ORDER BY e.starts_at LIMIT 5000",
    )
    .bind(ws)
    .bind(user.id)
    .bind(q.from)
    .bind(q.to)
    .bind(ws_cals)
    .fetch_all(&state.pool)
    .await?;
    let tasks: Vec<TaskDue> = sqlx::query_as(
        "SELECT t.id, t.title, t.due_date, t.is_completed, b.id AS board_id, b.name AS board_name
         FROM tasks t
         JOIN task_assignees ta ON ta.task_id = t.id AND ta.user_id = $2
         JOIN board_columns c ON c.id = t.column_id
         JOIN boards b ON b.id = c.board_id
         WHERE b.workspace_id = $1 AND t.due_date >= $3 AND t.due_date < $4
         ORDER BY t.due_date LIMIT 1000",
    )
    .bind(ws)
    .bind(user.id.to_string())
    .bind(q.from)
    .bind(q.to)
    .fetch_all(&state.pool)
    .await?;
    // Calendrier de l'espace (événements Looty) en premier, puis les abonnements.
    let mut all = shared;
    all.extend(events.into_iter().filter_map(|e| serde_json::to_value(e).ok()));
    Ok(Json(serde_json::json!({ "events": all, "tasks": tasks })))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win() -> (DateTime<Utc>, DateTime<Utc>) {
        (Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(), Utc.with_ymd_and_hms(2025, 12, 31, 0, 0, 0).unwrap())
    }

    #[test]
    fn weekly_course_with_timezone_exception_and_override() {
        let ics = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:algo\r\nSUMMARY:Algorithmique\r\nLOCATION:Amphi A\r\nDTSTART;TZID=Europe/Paris:20250303T081500\r\nDTEND;TZID=Europe/Paris:20250303T101500\r\nRRULE:FREQ=WEEKLY;COUNT=4\r\nEXDATE;TZID=Europe/Paris:20250310T081500\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:algo\r\nRECURRENCE-ID;TZID=Europe/Paris:20250317T081500\r\nSUMMARY:Algorithmique (salle changée)\r\nLOCATION:Salle 12\r\nDTSTART;TZID=Europe/Paris:20250317T090000\r\nDTEND;TZID=Europe/Paris:20250317T110000\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let (f, t) = win();
        let o = expand(ics, f, t);
        assert_eq!(o.len(), 3, "{o:#?}");
        // Paris = UTC+1 en mars avant le changement d'heure (30 mars).
        assert_eq!(o[0].start, Utc.with_ymd_and_hms(2025, 3, 3, 7, 15, 0).unwrap());
        assert_eq!(o[0].end - o[0].start, Duration::hours(2));
        assert_eq!(o[1].summary, "Algorithmique (salle changée)");
        assert_eq!(o[1].location.as_deref(), Some("Salle 12"));
        assert_eq!(o[1].start, Utc.with_ymd_and_hms(2025, 3, 17, 8, 0, 0).unwrap());
        assert_eq!(o[2].start, Utc.with_ymd_and_hms(2025, 3, 24, 7, 15, 0).unwrap());
    }

    #[test]
    fn utc_all_day_windows_zone_and_folding() {
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:a\nSUMMARY:Partiel\\, maths\nDTSTART:20250610T120000Z\nDURATION:PT1H30M\nEND:VEVENT\nBEGIN:VEVENT\nUID:b\nSUMMARY:Vacances\nDTSTART;VALUE=DATE:20250714\nDTEND;VALUE=DATE:20250716\nEND:VEVENT\nBEGIN:VEVENT\nUID:c\nSUMMARY:Réunion très lon\n gue\nDTSTART;TZID=\"Romance Standard Time\":20250701T140000\nDTEND;TZID=\"Romance Standard Time\":20250701T150000\nEND:VEVENT\nBEGIN:VEVENT\nUID:d\nSUMMARY:Annulé\nSTATUS:CANCELLED\nDTSTART:20250801T100000Z\nEND:VEVENT\nEND:VCALENDAR\n";
        let (f, t) = win();
        let o = expand(ics, f, t);
        assert_eq!(o.len(), 3, "{o:#?}");
        assert_eq!(o[0].summary, "Partiel, maths");
        assert_eq!(o[0].end - o[0].start, Duration::minutes(90));
        assert_eq!(o[1].summary, "Réunion très longue");
        assert_eq!(o[1].start, Utc.with_ymd_and_hms(2025, 7, 1, 12, 0, 0).unwrap()); // Paris = UTC+2 en été
        assert!(o[2].all_day && o[2].end - o[2].start == Duration::days(2));
    }

    #[test]
    fn durations_and_webcal() {
        assert_eq!(parse_duration("PT1H30M"), Some(Duration::minutes(90)));
        assert_eq!(parse_duration("P1D"), Some(Duration::days(1)));
        assert_eq!(parse_duration("-PT15M"), Some(Duration::minutes(-15)));
        assert_eq!(normalize_url("webcal://edt.univ.fr/x.ics"), "https://edt.univ.fr/x.ics");
    }
}

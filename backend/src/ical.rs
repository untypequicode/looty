//! Génération et lecture minimale d'objets iCalendar VTODO (RFC 5545)
//! pour la synchronisation CalDAV des tâches.

use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Timelike, Utc};
use serde::{Deserialize, Serialize};

/// Représentation « métier » d'une tâche telle qu'échangée avec le calendrier.
/// Sert aussi d'instantané pour la détection des changements à trois voies.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TodoFields {
    pub summary: String,
    pub description: String,
    pub due: Option<DateTime<Utc>>,
    pub start: Option<DateTime<Utc>>,
    /// Priorité Looty (0 = aucune, 1 urgente ... 4 basse).
    pub priority: i32,
    pub completed: bool,
}

pub struct TodoMeta<'a> {
    pub uid: &'a str,
    pub url: &'a str,
    pub categories: &'a [String],
    pub last_modified: DateTime<Utc>,
    pub created: DateTime<Utc>,
}

/// Les dates « jour » de Looty sont stockées à midi UTC : on les exporte en VALUE=DATE.
fn is_all_day(d: &DateTime<Utc>) -> bool {
    d.hour() == 12 && d.minute() == 0 && d.second() == 0
}

fn fmt_dt(d: &DateTime<Utc>) -> String {
    d.format("%Y%m%dT%H%M%SZ").to_string()
}

fn date_prop(name: &str, d: &DateTime<Utc>) -> String {
    if is_all_day(d) {
        format!("{name};VALUE=DATE:{}", d.format("%Y%m%d"))
    } else {
        format!("{name}:{}", fmt_dt(d))
    }
}

/// Échappement des valeurs TEXT (RFC 5545 §3.3.11).
pub fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace(';', "\\;").replace(',', "\\,").replace("\r\n", "\\n").replace('\n', "\\n")
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

/// Repli des lignes à 75 octets maximum (sans couper un caractère UTF-8).
fn fold(line: &str) -> String {
    if line.len() <= 75 {
        return line.to_string();
    }
    let mut out = String::new();
    let mut count = 0;
    for ch in line.chars() {
        let w = ch.len_utf8();
        if count + w > 75 {
            out.push_str("\r\n ");
            count = 1;
        }
        out.push(ch);
        count += w;
    }
    out
}

pub fn priority_to_ical(p: i32) -> u8 {
    match p {
        1 => 1,
        2 => 3,
        3 => 5,
        4 => 9,
        _ => 0,
    }
}

pub fn priority_from_ical(p: u8) -> i32 {
    match p {
        0 => 0,
        1..=2 => 1,
        3..=4 => 2,
        5 => 3,
        _ => 4,
    }
}

/// Construit un VCALENDAR contenant un unique VTODO.
pub fn build_vtodo(f: &TodoFields, m: &TodoMeta) -> String {
    let mut lines = vec![
        "BEGIN:VCALENDAR".to_string(),
        "VERSION:2.0".to_string(),
        "PRODID:-//Looty//Tasks//FR".to_string(),
        "BEGIN:VTODO".to_string(),
        format!("UID:{}", m.uid),
        // DTSTAMP déterministe (date de modification) pour un hash stable.
        format!("DTSTAMP:{}", fmt_dt(&m.last_modified)),
        format!("CREATED:{}", fmt_dt(&m.created)),
        format!("LAST-MODIFIED:{}", fmt_dt(&m.last_modified)),
        format!("SUMMARY:{}", escape(&f.summary)),
    ];
    if !f.description.is_empty() {
        lines.push(format!("DESCRIPTION:{}", escape(&f.description)));
    }
    if let Some(s) = &f.start {
        lines.push(date_prop("DTSTART", s));
    }
    if let Some(d) = &f.due {
        // DUE doit être postérieur à DTSTART : on n'émet pas DTSTART s'il est après.
        if f.start.map(|s| s <= *d).unwrap_or(true) {
            lines.push(date_prop("DUE", d));
        } else {
            lines.retain(|l| !l.starts_with("DTSTART"));
            lines.push(date_prop("DUE", d));
        }
    }
    let prio = priority_to_ical(f.priority);
    if prio > 0 {
        lines.push(format!("PRIORITY:{prio}"));
    }
    if f.completed {
        lines.push("STATUS:COMPLETED".into());
        lines.push("PERCENT-COMPLETE:100".into());
        lines.push(format!("COMPLETED:{}", fmt_dt(&m.last_modified)));
    } else {
        lines.push("STATUS:NEEDS-ACTION".into());
    }
    if !m.categories.is_empty() {
        lines.push(format!("CATEGORIES:{}", m.categories.iter().map(|c| escape(c)).collect::<Vec<_>>().join(",")));
    }
    if !m.url.is_empty() {
        lines.push(format!("URL:{}", m.url));
    }
    lines.push("END:VTODO".into());
    lines.push("END:VCALENDAR".into());
    lines.iter().map(|l| fold(l)).collect::<Vec<_>>().join("\r\n") + "\r\n"
}

/// Déplie les lignes repliées.
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

fn parse_date(params: &str, value: &str) -> Option<DateTime<Utc>> {
    let v = value.trim();
    if params.to_ascii_uppercase().contains("VALUE=DATE") && !v.contains('T') || v.len() == 8 {
        let d = NaiveDate::parse_from_str(v, "%Y%m%d").ok()?;
        return Some(Utc.from_utc_datetime(&d.and_hms_opt(12, 0, 0)?));
    }
    if let Some(z) = v.strip_suffix('Z') {
        let n = NaiveDateTime::parse_from_str(z, "%Y%m%dT%H%M%S").ok()?;
        return Some(Utc.from_utc_datetime(&n));
    }
    // Heure locale (TZID) : sans base de fuseaux, on l'interprète comme UTC.
    let n = NaiveDateTime::parse_from_str(v, "%Y%m%dT%H%M%S").ok()?;
    Some(Utc.from_utc_datetime(&n))
}

pub struct ParsedTodo {
    pub uid: Option<String>,
    pub fields: TodoFields,
}

/// Lit le premier VTODO d'un objet iCalendar.
pub fn parse_vtodo(ics: &str) -> Option<ParsedTodo> {
    let mut in_todo = false;
    let mut found = false;
    let mut uid = None;
    let mut f = TodoFields::default();
    let mut status_completed = false;
    let mut percent = 0u32;
    let mut has_completed_prop = false;
    let mut depth_other = 0;
    for line in unfold(ics) {
        let upper = line.to_ascii_uppercase();
        if upper == "BEGIN:VTODO" {
            if found {
                break;
            }
            in_todo = true;
            found = true;
            continue;
        }
        if upper == "END:VTODO" {
            in_todo = false;
            continue;
        }
        if !in_todo {
            continue;
        }
        // Ignore les sous-composants (VALARM...)
        if upper.starts_with("BEGIN:") {
            depth_other += 1;
            continue;
        }
        if upper.starts_with("END:") {
            depth_other -= 1;
            continue;
        }
        if depth_other > 0 {
            continue;
        }
        let Some((left, value)) = line.split_once(':') else { continue };
        let (name, params) = left.split_once(';').unwrap_or((left, ""));
        match name.to_ascii_uppercase().as_str() {
            "UID" => uid = Some(value.trim().to_string()),
            "SUMMARY" => f.summary = unescape(value),
            "DESCRIPTION" => f.description = unescape(value),
            "DUE" => f.due = parse_date(params, value),
            "DTSTART" => f.start = parse_date(params, value),
            "PRIORITY" => f.priority = priority_from_ical(value.trim().parse().unwrap_or(0)),
            "STATUS" => status_completed = value.trim().eq_ignore_ascii_case("COMPLETED"),
            "PERCENT-COMPLETE" => percent = value.trim().parse().unwrap_or(0),
            "COMPLETED" => has_completed_prop = true,
            _ => {}
        }
    }
    if !found {
        return None;
    }
    f.completed = status_completed || percent >= 100 || (has_completed_prop && !status_is_open(ics));
    Some(ParsedTodo { uid, fields: f })
}

fn status_is_open(ics: &str) -> bool {
    let u = ics.to_ascii_uppercase();
    u.contains("STATUS:NEEDS-ACTION") || u.contains("STATUS:IN-PROCESS")
}

/// Texte brut à partir du HTML de l'éditeur (pour DESCRIPTION).
pub fn html_to_text(html: &str) -> String {
    let with_breaks = html
        .replace("</p>", "\n")
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("</li>", "\n")
        .replace("</h1>", "\n")
        .replace("</h2>", "\n")
        .replace("</h3>", "\n");
    let mut out = String::new();
    let mut in_tag = false;
    for c in with_breaks.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    let decoded = out
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&");
    decoded.lines().map(str::trim_end).collect::<Vec<_>>().join("\n").trim().to_string()
}

/// Texte brut -> HTML minimal (paragraphes échappés).
pub fn text_to_html(text: &str) -> String {
    if text.trim().is_empty() {
        return String::new();
    }
    text.split('\n')
        .map(|l| {
            let e = l.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
            format!("<p>{e}</p>")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_vtodo() {
        let due = Utc.with_ymd_and_hms(2026, 10, 5, 12, 0, 0).unwrap();
        let start = Utc.with_ymd_and_hms(2026, 10, 1, 9, 30, 0).unwrap();
        let f = TodoFields {
            summary: "Réunion; budget, Q4 \\ test".into(),
            description: "Ligne 1\nLigne 2 avec un texte assez long pour être replié sur plusieurs lignes du fichier iCalendar".into(),
            due: Some(due),
            start: Some(start),
            priority: 2,
            completed: false,
        };
        let now = Utc::now();
        let ics = build_vtodo(&f, &TodoMeta { uid: "abc@looty", url: "https://x/y", categories: &["Bug".into()], last_modified: now, created: now });
        assert!(ics.contains("DUE;VALUE=DATE:20261005"));
        assert!(ics.lines().all(|l| l.len() <= 76));
        let p = parse_vtodo(&ics).unwrap();
        assert_eq!(p.uid.as_deref(), Some("abc@looty"));
        assert_eq!(p.fields.summary, f.summary);
        assert_eq!(p.fields.description, f.description);
        assert_eq!(p.fields.due, f.due);
        assert_eq!(p.fields.start, f.start);
        assert_eq!(p.fields.priority, 2);
        assert!(!p.fields.completed);
    }

    #[test]
    fn parse_completed_from_client() {
        let ics = "BEGIN:VCALENDAR\r\nBEGIN:VTODO\r\nUID:1\r\nSUMMARY:Done\r\nSTATUS:COMPLETED\r\nPRIORITY:9\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nSUMMARY:alarm\r\nEND:VALARM\r\nEND:VTODO\r\nEND:VCALENDAR\r\n";
        let p = parse_vtodo(ics).unwrap();
        assert!(p.fields.completed);
        assert_eq!(p.fields.priority, 4);
        assert_eq!(p.fields.summary, "Done");
    }

    #[test]
    fn html_text_roundtrip() {
        assert_eq!(html_to_text("<p>Hello &amp; <strong>world</strong></p><p>Line 2</p>"), "Hello & world\nLine 2");
        assert_eq!(text_to_html("a<b\nc"), "<p>a&lt;b</p><p>c</p>");
    }
}

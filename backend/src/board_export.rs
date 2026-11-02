//! Export des tâches d'un tableau en CSV (tableur, bilan de projet).
//!
//! Une ligne par tâche (sous-tâches comprises), en-têtes dans la langue du membre, dates à
//! l'heure de son fuseau. UTF-8 avec BOM pour qu'Excel lise les accents ; les cellules qui
//! commenceraient par une formule (`=`, `+`, `-`, `@`) sont neutralisées.

use axum::{
    extract::{Path, Query, State},
    http::header,
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Timelike, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::access::AuthUser;
use crate::error::{ApiError, ApiResult};

#[derive(sqlx::FromRow)]
struct Row {
    title: String,
    column_name: String,
    is_completed: bool,
    priority: i32,
    labels: Option<String>,
    assignees: Option<String>,
    start_date: Option<DateTime<Utc>>,
    due_date: Option<DateTime<Utc>>,
    parent_title: Option<String>,
    repeat: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    description: Option<String>,
}

/// Cellule CSV : entre guillemets si besoin, formule neutralisée.
fn cell(s: &str) -> String {
    let s = if s.starts_with(['=', '+', '-', '@', '\t', '\r']) { format!("'{s}") } else { s.to_string() };
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s
    }
}

fn line(cells: &[String]) -> String {
    let mut l = cells.iter().map(|c| cell(c)).collect::<Vec<_>>().join(",");
    l.push_str("\r\n");
    l
}

/// Texte brut d'une description HTML de l'éditeur (paragraphes → retours à la ligne).
fn plain(html: &str) -> String {
    let with_breaks = html.replace("</p>", "\n").replace("<br>", "\n").replace("<br/>", "\n").replace("</li>", "\n");
    let mut out = String::with_capacity(with_breaks.len());
    let mut in_tag = false;
    for c in with_breaks.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    let out = out.replace("&nbsp;", " ").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&amp;", "&");
    out.lines().map(str::trim_end).collect::<Vec<_>>().join("\n").trim().to_string()
}

/// Date lisible par un tableur : « AAAA-MM-JJ HH:MM » à l'heure du membre, sans heure pour
/// une échéance d'une journée (12:00 UTC).
fn date(d: Option<DateTime<Utc>>, tz: Option<chrono_tz::Tz>) -> String {
    let Some(d) = d else { return String::new() };
    if d.hour() == 12 && d.minute() == 0 {
        return d.format("%Y-%m-%d").to_string();
    }
    match tz {
        Some(z) => d.with_timezone(&z).format("%Y-%m-%d %H:%M").to_string(),
        None => d.format("%Y-%m-%d %H:%M UTC").to_string(),
    }
}

fn priority(p: i32, fr: bool) -> &'static str {
    match (p, fr) {
        (1, true) => "Urgente",
        (1, false) => "Urgent",
        (2, true) => "Haute",
        (2, false) => "High",
        (3, true) => "Moyenne",
        (3, false) => "Medium",
        (4, true) => "Basse",
        (4, false) => "Low",
        _ => "",
    }
}

fn repeat(r: Option<&str>, fr: bool) -> &'static str {
    match (r, fr) {
        (Some("daily"), true) => "Chaque jour",
        (Some("daily"), false) => "Every day",
        (Some("weekly"), true) => "Chaque semaine",
        (Some("weekly"), false) => "Every week",
        (Some("biweekly"), true) => "Toutes les deux semaines",
        (Some("biweekly"), false) => "Every two weeks",
        (Some("monthly"), true) => "Chaque mois",
        (Some("monthly"), false) => "Every month",
        (Some("yearly"), true) => "Chaque année",
        (Some("yearly"), false) => "Every year",
        _ => "",
    }
}

fn header_row(fr: bool) -> Vec<String> {
    let h: &[&str] = if fr {
        &["Titre", "Colonne", "Terminée", "Priorité", "Étiquettes", "Personnes", "Début", "Échéance", "Tâche parente", "Répétition", "Créée le", "Modifiée le", "Description"]
    } else {
        &["Title", "Column", "Done", "Priority", "Labels", "Assignees", "Start", "Due", "Parent task", "Repeat", "Created", "Updated", "Description"]
    };
    h.iter().map(|s| s.to_string()).collect()
}

fn render(rows: &[Row], fr: bool, tz: Option<chrono_tz::Tz>) -> String {
    let mut out = String::from("\u{feff}");
    out.push_str(&line(&header_row(fr)));
    let yes_no = |b: bool| if fr { if b { "Oui" } else { "Non" } } else if b { "Yes" } else { "No" };
    for r in rows {
        out.push_str(&line(&[
            r.title.clone(),
            r.column_name.clone(),
            yes_no(r.is_completed).to_string(),
            priority(r.priority, fr).to_string(),
            r.labels.clone().unwrap_or_default(),
            r.assignees.clone().unwrap_or_default(),
            date(r.start_date, tz),
            date(r.due_date, tz),
            r.parent_title.clone().unwrap_or_default(),
            repeat(r.repeat.as_deref(), fr).to_string(),
            date(Some(r.created_at), tz),
            date(Some(r.updated_at), tz),
            plain(r.description.as_deref().unwrap_or("")),
        ]));
    }
    out
}

/// Nom de fichier sûr (ASCII) pour `Content-Disposition`.
fn file_name(board: &str) -> String {
    let base: String = crate::classes::fold(board).chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let base = base.split('-').filter(|s| !s.is_empty()).collect::<Vec<_>>().join("-");
    format!("{}.csv", if base.is_empty() { "board".to_string() } else { base })
}

#[derive(serde::Deserialize)]
pub struct ExportQuery {
    /// Langue de l'interface (`fr`, `en`) ; sinon celle du compte.
    lang: Option<String>,
}

pub async fn export_csv(State(pool): State<PgPool>, user: AuthUser, Path(board_id): Path<Uuid>, Query(q): Query<ExportQuery>) -> ApiResult<Response> {
    let (workspace_id, board_name): (Uuid, String) = sqlx::query_as("SELECT workspace_id, name FROM boards WHERE id = $1")
        .bind(board_id)
        .fetch_optional(&pool)
        .await?
        .ok_or(ApiError::NotFound)?;
    crate::access::require_perm(&pool, &user, workspace_id, |p| p.boards >= crate::access::level::READ).await?;
    let (locale, tz): (Option<String>, Option<String>) = sqlx::query_as("SELECT locale, timezone FROM users WHERE id = $1").bind(user.id).fetch_one(&pool).await?;
    let fr = q.lang.or(locale).as_deref() != Some("en");
    let tz = tz.and_then(|z| z.parse::<chrono_tz::Tz>().ok());

    let rows: Vec<Row> = sqlx::query_as(
        "SELECT t.title, c.name AS column_name, t.is_completed, t.priority,
                (SELECT string_agg(l.name, ', ' ORDER BY l.name) FROM task_labels tl JOIN labels l ON l.id = tl.label_id WHERE tl.task_id = t.id) AS labels,
                (SELECT string_agg(u.name, ', ' ORDER BY u.name) FROM task_assignees ta JOIN users u ON u.id::text = ta.user_id WHERE ta.task_id = t.id) AS assignees,
                t.start_date, t.due_date, p.title AS parent_title, t.repeat, t.created_at, t.updated_at, t.description
         FROM tasks t
         JOIN board_columns c ON c.id = t.column_id
         LEFT JOIN tasks p ON p.id = t.parent_id
         WHERE c.board_id = $1
         ORDER BY c.position, t.parent_id NULLS FIRST, t.position",
    )
    .bind(board_id)
    .fetch_all(&pool)
    .await?;

    let name = file_name(&board_name);
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_string()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{name}\"")),
            (header::CACHE_CONTROL, "no-store".to_string()),
        ],
        render(&rows, fr, tz),
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn cells_are_escaped() {
        assert_eq!(cell("simple"), "simple");
        assert_eq!(cell("a, b"), "\"a, b\"");
        assert_eq!(cell("dit \"bonjour\""), "\"dit \"\"bonjour\"\"\"");
        assert_eq!(cell("ligne 1\nligne 2"), "\"ligne 1\nligne 2\"");
        assert_eq!(cell("=SUM(A1)"), "'=SUM(A1)");
        assert_eq!(cell("-1"), "'-1");
    }

    #[test]
    fn description_as_plain_text() {
        assert_eq!(plain("<p>Rendre le <strong>TP</strong> &amp; le rapport</p><p>Avant midi</p>"), "Rendre le TP & le rapport\nAvant midi");
        assert_eq!(plain(""), "");
    }

    #[test]
    fn renders_a_row() {
        let paris: chrono_tz::Tz = "Europe/Paris".parse().unwrap();
        let at = Utc.with_ymd_and_hms(2026, 11, 3, 13, 0, 0).unwrap();
        let row = Row {
            title: "Rapport, version 2".into(),
            column_name: "À faire".into(),
            is_completed: false,
            priority: 2,
            labels: Some("Maths, Projet".into()),
            assignees: Some("Alice".into()),
            start_date: None,
            due_date: Some(Utc.with_ymd_and_hms(2026, 11, 10, 12, 0, 0).unwrap()),
            parent_title: None,
            repeat: Some("weekly".into()),
            created_at: at,
            updated_at: at,
            description: Some("<p>Relire</p>".into()),
        };
        let csv = render(&[row], true, Some(paris));
        let mut lines = csv.lines();
        assert!(lines.next().unwrap().starts_with("\u{feff}Titre,Colonne,Terminée"));
        assert_eq!(
            lines.next().unwrap(),
            "\"Rapport, version 2\",À faire,Non,Haute,\"Maths, Projet\",Alice,,2026-11-10,,Chaque semaine,2026-11-03 14:00,2026-11-03 14:00,Relire"
        );
    }

    #[test]
    fn safe_file_name() {
        assert_eq!(file_name("Projet d'Algèbre / S1"), "projet-d-algebre-s1.csv");
        assert_eq!(file_name("✨"), "board.csv");
    }
}

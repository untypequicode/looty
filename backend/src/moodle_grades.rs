//! Notes Moodle : note globale de chaque cours et carnet de notes d'un cours.
//!
//! `gradereport_overview_get_course_grades` et `gradereport_user_get_grade_items` font
//! partie du service mobile, mais un établissement peut les désactiver ou masquer le
//! carnet : on répond alors `moodle_grades_unavailable` (sans marquer le jeton en erreur,
//! puisque le reste fonctionne). Copie hors ligne comme le reste de Moodle (moodle_cache).

use axum::{
    extract::{Path, State},
    response::Response,
    routing::get,
    Router,
};
use chrono::DateTime;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::access::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::moodle::{conn_of, p};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new().route("/grades", get(overview)).route("/courses/{course_id}/grades", get(course))
}

/// Erreur d'une fonction de notes : un refus d'accès vise les notes, pas le compte.
fn unavailable(e: ApiError) -> ApiError {
    match &e {
        ApiError::BadRequest(m) if m == "moodle_invalid_token" || m == "moodle_ws_disabled" || m.starts_with("moodle_error") => ApiError::bad_request("moodle_grades_unavailable"),
        _ => e,
    }
}

/// Note affichable : Moodle renvoie « - » (ou rien) quand il n'y en a pas.
fn shown(v: &Value) -> Option<String> {
    v.as_str().map(|s| crate::ical::html_to_text(s).trim().to_string()).filter(|s| !s.is_empty() && s != "-")
}

fn num(v: &Value) -> Option<f64> {
    v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok()))
}

/// Note globale de chaque cours (une ligne par cours où le carnet en donne une).
fn overview_rows(v: &Value) -> Vec<Value> {
    v["grades"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|g| json!({ "course_id": g["courseid"], "grade": shown(&g["grade"]), "raw": num(&g["rawgrade"]), "rank": g["rank"] }))
                .collect()
        })
        .unwrap_or_default()
}

/// Éléments du carnet d'un cours, dans l'ordre du site ; le total du cours à part.
fn course_rows(v: &Value, clean: impl Fn(&str) -> String) -> Value {
    let items = v["usergrades"][0]["gradeitems"].as_array().cloned().unwrap_or_default();
    let mut total = Value::Null;
    let mut list = Vec::new();
    for i in &items {
        if i["gradehiddenbyuser"].as_bool() == Some(true) || i["gradeishidden"].as_bool() == Some(true) {
            continue;
        }
        let feedback = i["feedback"].as_str().map(&clean).filter(|f| !crate::ical::html_to_text(f).trim().is_empty());
        let row = json!({
            "id": i["id"],
            "name": i["itemname"].as_str().filter(|s| !s.is_empty()),
            "type": i["itemtype"],
            "module": i["itemmodule"],
            "cmid": i["cmid"],
            "grade": shown(&i["gradeformatted"]),
            "raw": num(&i["graderaw"]),
            "min": num(&i["grademin"]),
            "max": num(&i["grademax"]),
            "percentage": shown(&i["percentageformatted"]),
            "feedback": feedback,
            "graded_at": i["gradedategraded"].as_i64().filter(|t| *t > 0).and_then(|t| DateTime::from_timestamp(t, 0)),
        });
        if i["itemtype"] == "course" {
            total = row;
        } else {
            list.push(row);
        }
    }
    json!({ "items": list, "total": total })
}

async fn overview_live(state: &AppState, user: &AuthUser, ws: Uuid) -> ApiResult<Value> {
    let (_, client, uid) = conn_of(state, user, ws).await?;
    let v = client.call("gradereport_overview_get_course_grades", &[p("userid", uid)]).await.map_err(unavailable)?;
    Ok(Value::Array(overview_rows(&v)))
}

async fn course_live(state: &AppState, user: &AuthUser, ws: Uuid, course_id: i64) -> ApiResult<Value> {
    let (_, client, uid) = conn_of(state, user, ws).await?;
    let v = client.call("gradereport_user_get_grade_items", &[p("courseid", course_id), p("userid", uid)]).await.map_err(unavailable)?;
    let base = client.base.clone();
    Ok(course_rows(&v, |html| crate::moodle_content::clean(html, &base, ws, None)))
}

async fn overview(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>) -> ApiResult<Response> {
    let live = overview_live(&state, &user, ws).await;
    crate::moodle_cache::json_or_cached(&state, ws, user.id, "grades", live).await
}

async fn course(State(state): State<AppState>, user: AuthUser, Path((ws, course_id)): Path<(Uuid, i64)>) -> ApiResult<Response> {
    let live = course_live(&state, &user, ws, course_id).await;
    crate::moodle_cache::json_or_cached(&state, ws, user.id, &format!("grades:{course_id}"), live).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overview_skips_empty_grades() {
        let v = json!({ "grades": [
            { "courseid": 1, "grade": "15,50", "rawgrade": "15.5", "rank": null },
            { "courseid": 2, "grade": "-", "rawgrade": null },
        ], "warnings": [] });
        let rows = overview_rows(&v);
        assert_eq!(rows[0]["grade"], "15,50");
        assert_eq!(rows[0]["raw"], 15.5);
        assert!(rows[1]["grade"].is_null());
    }

    #[test]
    fn course_items_and_total() {
        let v = json!({ "usergrades": [{ "gradeitems": [
            { "id": 1, "itemname": "TP 1", "itemtype": "mod", "itemmodule": "assign", "cmid": 105, "gradeformatted": "14,00", "graderaw": 14.0, "grademin": 0, "grademax": 20, "percentageformatted": "70,00 %", "feedback": "<p>Bien</p>", "gradedategraded": 1_790_000_000 },
            { "id": 2, "itemname": "Caché", "itemtype": "mod", "gradeishidden": true, "gradeformatted": "3,00" },
            { "id": 3, "itemname": "", "itemtype": "course", "gradeformatted": "14,00", "graderaw": 14.0, "grademax": 20, "feedback": "" },
        ] }] });
        let out = course_rows(&v, |h| h.to_string());
        let items = out["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["name"], "TP 1");
        assert_eq!(items[0]["percentage"], "70,00 %");
        assert_eq!(items[0]["feedback"], "<p>Bien</p>");
        assert!(items[0]["graded_at"].is_string());
        assert_eq!(out["total"]["grade"], "14,00");
        assert!(out["total"]["feedback"].is_null());
    }

    #[test]
    fn access_refusal_is_about_grades() {
        assert!(matches!(unavailable(ApiError::bad_request("moodle_invalid_token")), ApiError::BadRequest(m) if m == "moodle_grades_unavailable"));
        assert!(matches!(unavailable(ApiError::bad_request("moodle_unreachable")), ApiError::BadRequest(m) if m == "moodle_unreachable"));
    }
}

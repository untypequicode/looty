//! Rappels avant l'échéance d'une tâche : une liste par tâche (`tasks.reminders`) ou, à défaut,
//! le rappel par défaut de la personne (`users.default_reminder_minutes`).

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{ApiError, ApiResult};

/// 30 jours : au-delà, un rappel n'a plus de sens.
pub const MAX_MINUTES: i64 = 43_200;
const MAX_PER_TASK: usize = 8;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reminder {
    /// Minutes avant l'échéance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<i64>,
    /// Date précise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<DateTime<Utc>>,
}

/// Valide la valeur reçue du client et la normalise (dédoublonnée, triée).
pub fn validate(v: &serde_json::Value) -> ApiResult<serde_json::Value> {
    let mut list: Vec<Reminder> = serde_json::from_value(v.clone()).map_err(|_| ApiError::bad_request("invalid_reminders"))?;
    if list.len() > MAX_PER_TASK {
        return Err(ApiError::bad_request("invalid_reminders"));
    }
    for r in &list {
        match (r.before, r.at) {
            (Some(m), None) if (0..=MAX_MINUTES).contains(&m) => {}
            (None, Some(_)) => {}
            _ => return Err(ApiError::bad_request("invalid_reminders")),
        }
    }
    list.sort_by_key(|r| (r.before.unwrap_or(i64::MAX), r.at));
    list.dedup();
    Ok(serde_json::to_value(list).unwrap_or_default())
}

/// Rappels effectifs d'une personne : (clé de déduplication, heure d'envoi).
/// `reminders` NULL → rappel par défaut de la personne ; une échéance sans heure (12:00 UTC)
/// est traitée comme les autres.
pub fn fire_times(due: DateTime<Utc>, reminders: Option<&serde_json::Value>, default_minutes: Option<i32>) -> Vec<(String, DateTime<Utc>)> {
    let list: Vec<Reminder> = match reminders {
        Some(v) => serde_json::from_value(v.clone()).unwrap_or_default(),
        None => default_minutes.map(|m| vec![Reminder { before: Some(m as i64), at: None }]).unwrap_or_default(),
    };
    list.into_iter()
        .filter_map(|r| match (r.before, r.at) {
            (Some(m), _) if (0..=MAX_MINUTES).contains(&m) => Some((format!("b{m}"), due - Duration::minutes(m))),
            (None, Some(at)) => Some((format!("a{}", at.timestamp()), at)),
            _ => None,
        })
        .collect()
}

/// Heures de rappel à exposer en VALARM (CalDAV, flux d'abonnement), triées.
pub fn alarms(due: Option<DateTime<Utc>>, reminders: Option<&serde_json::Value>, default_minutes: Option<i32>) -> Vec<DateTime<Utc>> {
    let Some(due) = due else { return vec![] };
    let mut v: Vec<DateTime<Utc>> = fire_times(due, reminders, default_minutes).into_iter().map(|(_, t)| t).collect();
    v.sort();
    v.dedup();
    v
}

/// Rappels recopiés sur l'occurrence suivante d'une tâche récurrente : les dates précises
/// sont décalées comme l'échéance.
pub fn shifted(reminders: Option<&serde_json::Value>, old_due: Option<DateTime<Utc>>, new_due: Option<DateTime<Utc>>) -> Option<serde_json::Value> {
    let v = reminders?;
    let list: Vec<Reminder> = serde_json::from_value(v.clone()).ok()?;
    let delta = match (old_due, new_due) {
        (Some(o), Some(n)) => Some(n - o),
        _ => None,
    };
    let out: Vec<Reminder> = list
        .into_iter()
        .filter_map(|r| match (r.before, r.at, delta) {
            (Some(_), _, _) => Some(r),
            (None, Some(at), Some(d)) => Some(Reminder { before: None, at: Some(at + d) }),
            _ => None,
        })
        .collect();
    serde_json::to_value(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn t(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    #[test]
    fn default_is_used_when_task_has_none() {
        let due = t("2026-12-10T09:00:00Z");
        let f = fire_times(due, None, Some(1440));
        assert_eq!(f, vec![("b1440".to_string(), t("2026-12-09T09:00:00Z"))]);
        assert!(fire_times(due, None, None).is_empty());
    }

    #[test]
    fn task_list_overrides_default_and_empty_means_none() {
        let due = t("2026-12-10T09:00:00Z");
        let v = json!([{ "before": 60 }, { "at": "2026-12-01T08:00:00Z" }]);
        let f = fire_times(due, Some(&v), Some(1440));
        assert_eq!(f.len(), 2);
        assert_eq!(f[0], ("b60".to_string(), t("2026-12-10T08:00:00Z")));
        assert_eq!(f[1].1, t("2026-12-01T08:00:00Z"));
        assert!(fire_times(due, Some(&json!([])), Some(1440)).is_empty());
    }

    #[test]
    fn validation_rejects_bad_input_and_normalizes() {
        assert!(validate(&json!([{ "before": -1 }])).is_err());
        assert!(validate(&json!([{ "before": 99999999 }])).is_err());
        assert!(validate(&json!([{}])).is_err());
        assert!(validate(&json!([{ "before": 5, "at": "2026-12-01T08:00:00Z" }])).is_err());
        assert!(validate(&json!("x")).is_err());
        assert!(validate(&json!((0..9).map(|i| json!({ "before": i })).collect::<Vec<_>>())).is_err());
        let ok = validate(&json!([{ "before": 1440 }, { "before": 60 }, { "before": 60 }])).unwrap();
        assert_eq!(ok, json!([{ "before": 60 }, { "before": 1440 }]));
    }

    #[test]
    fn alarms_sorted_and_empty_without_due() {
        let due = t("2026-12-10T09:00:00Z");
        let v = json!([{ "before": 1440 }, { "before": 60 }]);
        assert_eq!(alarms(Some(due), Some(&v), None), vec![t("2026-12-09T09:00:00Z"), t("2026-12-10T08:00:00Z")]);
        assert!(alarms(None, Some(&v), Some(1440)).is_empty());
    }

    #[test]
    fn next_occurrence_shifts_absolute_dates() {
        let v = json!([{ "before": 60 }, { "at": "2026-12-01T08:00:00Z" }]);
        let out = shifted(Some(&v), Some(t("2026-12-10T09:00:00Z")), Some(t("2026-12-17T09:00:00Z"))).unwrap();
        assert_eq!(out, json!([{ "before": 60 }, { "at": "2026-12-08T08:00:00Z" }]));
        assert_eq!(shifted(None, None, None), None);
    }
}

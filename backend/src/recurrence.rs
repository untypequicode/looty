//! Tâches récurrentes : calcul des dates de l'occurrence suivante.
//!
//! Les dates avancent dans le fuseau du membre (même heure locale après un changement
//! d'heure) ; une échéance sans heure (12:00 UTC) reste à 12:00 UTC. Un mois ou une année
//! plus tard garde le jour, ou le dernier jour du mois s'il n'existe pas (31 janv. → 28 févr.).

use chrono::{DateTime, Duration, Months, NaiveDateTime, TimeZone, Timelike, Utc};

pub const REPEATS: &[&str] = &["daily", "weekly", "biweekly", "monthly", "yearly"];

pub fn is_valid(repeat: &str) -> bool {
    REPEATS.contains(&repeat)
}

/// Une période plus tard, en heure « murale ».
fn step(d: NaiveDateTime, repeat: &str) -> Option<NaiveDateTime> {
    match repeat {
        "daily" => Some(d + Duration::days(1)),
        "weekly" => Some(d + Duration::days(7)),
        "biweekly" => Some(d + Duration::days(14)),
        "monthly" => d.checked_add_months(Months::new(1)),
        "yearly" => d.checked_add_months(Months::new(12)),
        _ => None,
    }
}

/// Date `n` périodes après `d` (comptées depuis `d`, pour qu'un 31 reste un 31 quand le
/// mois le permet).
fn nth(d: DateTime<Utc>, repeat: &str, n: u32, tz: Option<chrono_tz::Tz>) -> Option<DateTime<Utc>> {
    let all_day = d.hour() == 12 && d.minute() == 0 && d.second() == 0;
    let months = match repeat {
        "monthly" => Some(n),
        "yearly" => Some(12 * n),
        _ => None,
    };
    let wall = |local: NaiveDateTime| -> Option<NaiveDateTime> {
        match months {
            Some(m) => local.checked_add_months(Months::new(m)),
            None => (0..n).try_fold(local, |acc, _| step(acc, repeat)),
        }
    };
    match tz.filter(|_| !all_day) {
        Some(z) => {
            let local = wall(d.with_timezone(&z).naive_local())?;
            // Heure inexistante (saut de printemps) : la première valide après ; ambiguë : la première.
            let resolved = z.from_local_datetime(&local).earliest().or_else(|| z.from_local_datetime(&(local + Duration::hours(1))).earliest())?;
            Some(resolved.with_timezone(&Utc))
        }
        None => Some(Utc.from_utc_datetime(&wall(d.naive_utc())?)),
    }
}

/// Décalage de l'occurrence suivante : le plus petit nombre de périodes (au moins une) qui
/// place la date de référence (échéance, sinon début) après `now`. Terminer en retard une
/// tâche quotidienne ne crée donc pas une occurrence déjà en retard.
pub fn periods_after(reference: DateTime<Utc>, repeat: &str, tz: Option<chrono_tz::Tz>, now: DateTime<Utc>) -> Option<u32> {
    (1..=1000).find(|&n| nth(reference, repeat, n, tz).is_some_and(|d| d > now))
}

/// `d` décalée de `n` périodes.
pub fn shift(d: DateTime<Utc>, repeat: &str, n: u32, tz: Option<chrono_tz::Tz>) -> Option<DateTime<Utc>> {
    nth(d, repeat, n, tz)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(y: i32, m: u32, d: u32, h: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, h, 0, 0).unwrap()
    }

    #[test]
    fn periods() {
        let d = utc(2026, 11, 2, 9);
        assert_eq!(shift(d, "daily", 1, None), Some(utc(2026, 11, 3, 9)));
        assert_eq!(shift(d, "weekly", 1, None), Some(utc(2026, 11, 9, 9)));
        assert_eq!(shift(d, "biweekly", 1, None), Some(utc(2026, 11, 16, 9)));
        assert_eq!(shift(d, "monthly", 1, None), Some(utc(2026, 12, 2, 9)));
        assert_eq!(shift(d, "yearly", 1, None), Some(utc(2027, 11, 2, 9)));
        assert_eq!(shift(d, "hourly", 1, None), None);
    }

    #[test]
    fn end_of_month_is_kept() {
        let jan31 = utc(2027, 1, 31, 12);
        assert_eq!(shift(jan31, "monthly", 1, None), Some(utc(2027, 2, 28, 12)));
        assert_eq!(shift(jan31, "monthly", 2, None), Some(utc(2027, 3, 31, 12)));
        assert_eq!(shift(utc(2028, 1, 31, 12), "monthly", 1, None), Some(utc(2028, 2, 29, 12)));
        assert_eq!(shift(utc(2028, 2, 29, 12), "yearly", 1, None), Some(utc(2029, 2, 28, 12)));
    }

    #[test]
    fn same_local_time_across_dst() {
        let paris: chrono_tz::Tz = "Europe/Paris".parse().unwrap();
        // Lundi 19 octobre 2026, 15:00 à Paris (UTC+2) ; une semaine plus tard, l'heure d'hiver (UTC+1).
        let before = Utc.with_ymd_and_hms(2026, 10, 19, 13, 0, 0).unwrap();
        let after = shift(before, "weekly", 1, Some(paris)).unwrap();
        assert_eq!(after, Utc.with_ymd_and_hms(2026, 10, 26, 14, 0, 0).unwrap()); // 15:00 à Paris (UTC+1)
        // Sans fuseau : 7 × 24 h.
        assert_eq!(shift(before, "weekly", 1, None).unwrap(), Utc.with_ymd_and_hms(2026, 10, 26, 13, 0, 0).unwrap());
    }

    #[test]
    fn all_day_stays_at_noon_utc() {
        let paris: chrono_tz::Tz = "Europe/Paris".parse().unwrap();
        let d = utc(2026, 10, 20, 12);
        assert_eq!(shift(d, "weekly", 1, Some(paris)), Some(utc(2026, 10, 27, 12)));
    }

    #[test]
    fn late_completion_skips_to_the_future() {
        let due = utc(2026, 11, 1, 9);
        assert_eq!(periods_after(due, "daily", None, utc(2026, 10, 31, 0)), Some(1));
        assert_eq!(periods_after(due, "daily", None, utc(2026, 11, 5, 10)), Some(5));
        assert_eq!(periods_after(due, "weekly", None, utc(2026, 11, 5, 10)), Some(1));
    }
}

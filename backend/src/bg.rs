//! Tâches de fond : une panique pendant le traitement d'un élément (flux ICS mal formé,
//! réponse inattendue d'un serveur…) ne doit pas arrêter la boucle pour tous les autres.
//! Chaque passage est aussi noté (`record`, `tick`) pour la page d'état de l'instance.

use chrono::{DateTime, Utc};
use futures::FutureExt;
use serde::Serialize;
use std::collections::BTreeMap;
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::sync::{Mutex, OnceLock};

/// Ce que l'administration sait d'une tâche de fond depuis le démarrage du serveur.
#[derive(Serialize, Clone, Default, Debug)]
pub struct JobStat {
    pub runs: u64,
    pub errors: u64,
    pub last_run: Option<DateTime<Utc>>,
    pub last_ok: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub last_error_at: Option<DateTime<Utc>>,
}

fn jobs() -> &'static Mutex<BTreeMap<String, JobStat>> {
    static J: OnceLock<Mutex<BTreeMap<String, JobStat>>> = OnceLock::new();
    J.get_or_init(Default::default)
}

/// Un passage de la boucle a eu lieu (même sans rien à faire).
pub fn tick(name: &str) {
    let mut j = jobs().lock().unwrap_or_else(|e| e.into_inner());
    j.entry(name.to_string()).or_default().last_run = Some(Utc::now());
}

/// Résultat d'un élément traité (un compte synchronisé, un envoi) ; l'erreur est tronquée.
pub fn record(name: &str, result: Result<(), String>) {
    let now = Utc::now();
    let mut j = jobs().lock().unwrap_or_else(|e| e.into_inner());
    let s = j.entry(name.to_string()).or_default();
    s.runs += 1;
    s.last_run = Some(now);
    match result {
        Ok(()) => s.last_ok = Some(now),
        Err(e) => {
            s.errors += 1;
            s.last_error = Some(e.chars().take(200).collect());
            s.last_error_at = Some(now);
        }
    }
}

pub fn snapshot() -> Vec<(String, JobStat)> {
    jobs().lock().unwrap_or_else(|e| e.into_inner()).iter().map(|(k, v)| (k.clone(), v.clone())).collect()
}

/// Comme `guard`, et note le résultat (succès, erreur ou panique) pour la page d'état de l'instance.
pub async fn guard_result<T, E: std::fmt::Debug>(what: &str, fut: impl Future<Output = Result<T, E>>) -> Option<Result<T, E>> {
    let out = guard(what, fut).await;
    match &out {
        Some(Ok(_)) => record(what, Ok(())),
        Some(Err(e)) => record(what, Err(format!("{e:?}"))),
        None => record(what, Err("panic".into())),
    }
    out
}

/// Exécute `fut` ; une panique est journalisée et donne `None`.
pub async fn guard<T>(what: &str, fut: impl Future<Output = T>) -> Option<T> {
    match AssertUnwindSafe(fut).catch_unwind().await {
        Ok(v) => Some(v),
        Err(e) => {
            let msg = e.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| e.downcast_ref::<String>().cloned()).unwrap_or_default();
            tracing::error!("background task panicked ({what}): {msg}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn keeps_going_after_a_panic() {
        assert_eq!(guard("ok", async { 1 }).await, Some(1));
        let r: Option<()> = guard("boom", async { panic!("boom") }).await;
        assert!(r.is_none());
    }

    #[tokio::test]
    async fn jobs_remember_successes_errors_and_panics() {
        assert!(guard_result("t-ok", async { Ok::<_, String>(1) }).await.is_some());
        assert!(guard_result("t-err", async { Err::<(), _>("boom".to_string()) }).await.is_some());
        let _ = guard_result::<(), String>("t-panic", async { panic!("x") }).await;
        let all: BTreeMap<_, _> = snapshot().into_iter().collect();
        assert_eq!((all["t-ok"].runs, all["t-ok"].errors), (1, 0));
        assert!(all["t-ok"].last_ok.is_some() && all["t-ok"].last_error.is_none());
        assert_eq!(all["t-err"].errors, 1);
        assert!(all["t-err"].last_error.as_deref().unwrap().contains("boom"));
        assert_eq!(all["t-panic"].last_error.as_deref(), Some("panic"));
        tick("t-tick");
        assert!(snapshot().iter().any(|(k, s)| k == "t-tick" && s.runs == 0 && s.last_run.is_some()));
    }
}

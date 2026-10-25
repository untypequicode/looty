//! Tâches de fond : une panique pendant le traitement d'un élément (flux ICS mal formé,
//! réponse inattendue d'un serveur…) ne doit pas arrêter la boucle pour tous les autres.

use futures::FutureExt;
use std::future::Future;
use std::panic::AssertUnwindSafe;

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
}

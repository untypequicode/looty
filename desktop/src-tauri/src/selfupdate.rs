//! Mises à jour automatiques de l'application Windows (plugin Tauri updater, signature minisign).
//! La vérification interroge `latest.json` (release « updater » de GitHub, mise à jour à chaque publication) :
//! si une version plus récente existe, elle est téléchargée, sa signature est vérifiée, puis l'installateur
//! la pose et relance Looty. Au plus une vérification toutes les 6 heures.

use std::time::Duration;
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

use crate::service::{french, OsNotification};

pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        // Laisse l'application démarrer avant la première vérification.
        tokio::time::sleep(Duration::from_secs(90)).await;
        loop {
            if let Err(e) = check(&app).await {
                tracing::warn!("update check failed: {e}");
            }
            tokio::time::sleep(Duration::from_secs(6 * 3600)).await;
        }
    });
}

async fn check(app: &AppHandle) -> tauri_plugin_updater::Result<()> {
    let Some(update) = app.updater()?.check().await? else { return Ok(()) };
    let v = update.version.clone();
    let body = if french() { format!("Looty {v} est téléchargé : installation puis redémarrage…") } else { format!("Looty {v} is downloaded: installing, then restarting…") };
    crate::tray::notify(app, OsNotification { title: "Looty".into(), body, path: String::new(), done_task: None });
    update.download_and_install(|_, _| {}, || {}).await?;
    Ok(())
}

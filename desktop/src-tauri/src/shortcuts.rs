//! Raccourcis clavier globaux, par le portail XDG « GlobalShortcuts ».
//!
//! Sous Wayland une application ne peut pas saisir de touches elle-même : elle demande au
//! bureau d'enregistrer des raccourcis (GNOME 48+, KDE Plasma, Hyprland avec
//! xdg-desktop-portal-hyprland). Le bureau les montre dans ses réglages, où l'on peut les
//! changer. Sans portail (autre bureau, session minimale), on s'en passe : le raccourci se
//! règle alors à la main sur `looty --quick-add` (voir le README).

use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};
use futures_util::StreamExt;
use tauri::AppHandle;

fn tr(fr: &'static str, en: &'static str) -> &'static str {
    if crate::service::french() { fr } else { en }
}

pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        if let Err(e) = run(app).await {
            tracing::info!("global shortcuts unavailable: {e}");
        }
    });
}

async fn run(app: AppHandle) -> ashpd::Result<()> {
    let portal = GlobalShortcuts::new().await?;
    let session = portal.create_session().await?;
    let shortcuts = [
        NewShortcut::new("quick-add", tr("Ajouter une tâche", "Add a task")).preferred_trigger("CTRL+ALT+n"),
        NewShortcut::new("show", tr("Ouvrir Looty", "Open Looty")).preferred_trigger("CTRL+ALT+l"),
    ];
    portal.bind_shortcuts(&session, &shortcuts, None).await?.response()?;
    tracing::info!("global shortcuts registered");
    let mut activated = portal.receive_activated().await?;
    while let Some(a) = activated.next().await {
        let id = a.shortcut_id().to_string();
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || match id.as_str() {
            "quick-add" => crate::run_action(&handle, crate::Action::QuickAdd),
            "show" => crate::run_action(&handle, crate::Action::Show),
            _ => {}
        });
    }
    Ok(())
}

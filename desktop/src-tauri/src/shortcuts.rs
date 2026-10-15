//! Raccourcis clavier globaux : Ctrl+Alt+N (nouvelle tâche) et Ctrl+Alt+L (ouvrir Looty).
//!
//! - **Linux** : portail XDG « GlobalShortcuts ». Sous Wayland une application ne peut pas
//!   saisir de touches elle-même : elle demande au bureau d'enregistrer des raccourcis
//!   (GNOME 48+, KDE Plasma, Hyprland avec xdg-desktop-portal-hyprland). Le bureau les montre
//!   dans ses réglages, où l'on peut les changer. Sans portail (autre bureau, session
//!   minimale), on s'en passe : le raccourci se règle alors à la main sur
//!   `looty --quick-add` (voir le README).
//! - **Windows** : enregistrement direct auprès du système (plugin global-shortcut).

#[cfg(target_os = "linux")]
mod portal {
    use ashpd::desktop::global_shortcuts::{BindShortcutsOptions, GlobalShortcuts, NewShortcut};
    use ashpd::desktop::CreateSessionOptions;
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
        let session = portal.create_session(CreateSessionOptions::default()).await?;
        let shortcuts = [
            NewShortcut::new("quick-add", tr("Ajouter une tâche", "Add a task")).preferred_trigger("CTRL+ALT+n"),
            NewShortcut::new("show", tr("Ouvrir Looty", "Open Looty")).preferred_trigger("CTRL+ALT+l"),
        ];
        portal.bind_shortcuts(&session, &shortcuts, None, BindShortcutsOptions::default()).await?.response()?;
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
}

#[cfg(target_os = "linux")]
pub use portal::start;

#[cfg(windows)]
pub fn start(app: tauri::AppHandle) {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
    if let Err(e) = app.plugin(tauri_plugin_global_shortcut::Builder::new().build()) {
        tracing::info!("global shortcuts unavailable: {e}");
        return;
    }
    for (keys, quick_add) in [("Ctrl+Alt+N", true), ("Ctrl+Alt+L", false)] {
        let handle = app.clone();
        let registered = app.global_shortcut().on_shortcut(keys, move |_app, _shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            let h = handle.clone();
            let _ = handle.run_on_main_thread(move || crate::run_action(&h, if quick_add { crate::Action::QuickAdd } else { crate::Action::Show }));
        });
        match registered {
            Ok(()) => tracing::info!("global shortcut {keys} registered"),
            Err(e) => tracing::info!("global shortcut {keys} unavailable: {e}"),
        }
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
pub fn start(_app: tauri::AppHandle) {}

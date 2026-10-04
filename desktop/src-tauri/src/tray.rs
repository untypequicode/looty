//! Barre système : menu dynamique (non lues, tâches du jour / en retard), pastille,
//! notifications système et lancement à l'ouverture de session.

use std::path::PathBuf;
use tauri::{
    image::Image,
    menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIcon,
    AppHandle, Manager, Wry,
};

use crate::service::{french, OsNotification, TaskItem, TrayState};

fn tr(fr: &'static str, en: &'static str) -> &'static str {
    if french() { fr } else { en }
}

/// Titre de menu tronqué (les menus très larges sont illisibles).
fn short(s: &str, max: usize) -> String {
    let s = s.replace('_', "__"); // « _ » marque un raccourci clavier dans les menus GTK
    if s.chars().count() <= max { s } else { format!("{}…", s.chars().take(max - 1).collect::<String>()) }
}

pub fn build_menu(app: &AppHandle, state: &TrayState, launch_at_login: bool, notifications: bool) -> tauri::Result<Menu<Wry>> {
    let mut items: Vec<Box<dyn IsMenuItem<Wry>>> = Vec::new();
    let sep = |app: &AppHandle| PredefinedMenuItem::separator(app);

    items.push(Box::new(MenuItem::with_id(app, "open", tr("Ouvrir Looty", "Open Looty"), true, None::<&str>)?));
    if state.logged_in {
        let inbox = if state.unread > 0 {
            format!("{} ({})", tr("Boîte de réception", "Inbox"), state.unread)
        } else {
            tr("Boîte de réception", "Inbox").to_string()
        };
        items.push(Box::new(MenuItem::with_id(app, "inbox", inbox, true, None::<&str>)?));
        items.push(Box::new(MenuItem::with_id(app, "new-task", tr("Nouvelle tâche…", "New task…"), true, None::<&str>)?));
        items.push(Box::new(sep(app)?));

        let section = |label: String, tasks: &[TaskItem], items: &mut Vec<Box<dyn IsMenuItem<Wry>>>| -> tauri::Result<()> {
            if tasks.is_empty() {
                return Ok(());
            }
            items.push(Box::new(MenuItem::with_id(app, format!("hdr-{label}"), &label, false, None::<&str>)?));
            for t in tasks.iter().take(8) {
                let mut name = short(&t.title, 42);
                if state.multi_workspace {
                    name = format!("{name}  ·  {}", short(&t.workspace_name, 16));
                }
                let open = MenuItem::with_id(app, format!("open-task|{}", t.path()), tr("Ouvrir", "Open"), true, None::<&str>)?;
                let done = MenuItem::with_id(app, format!("done-task|{}", t.id), tr("Marquer comme terminée", "Mark as done"), true, None::<&str>)?;
                let board = MenuItem::with_id(app, format!("board-{}", t.id), short(&t.board_name, 30), false, None::<&str>)?;
                items.push(Box::new(Submenu::with_items(app, name, true, &[&board, &PredefinedMenuItem::separator(app)?, &open, &done])?));
            }
            if tasks.len() > 8 {
                items.push(Box::new(MenuItem::with_id(app, format!("more-{label}"), format!("… +{}", tasks.len() - 8), false, None::<&str>)?));
            }
            Ok(())
        };
        section(format!("{} ({})", tr("En retard", "Overdue"), state.overdue.len()), &state.overdue, &mut items)?;
        section(format!("{} ({})", tr("Aujourd’hui", "Today"), state.today.len()), &state.today, &mut items)?;
        if state.overdue.is_empty() && state.today.is_empty() {
            items.push(Box::new(MenuItem::with_id(app, "no-tasks", tr("Rien à rendre aujourd’hui", "Nothing due today"), false, None::<&str>)?));
        }
        items.push(Box::new(MenuItem::with_id(app, "my-tasks", tr("Toutes mes tâches", "All my tasks"), true, None::<&str>)?));
        items.push(Box::new(sep(app)?));
    } else {
        items.push(Box::new(sep(app)?));
    }
    items.push(Box::new(CheckMenuItem::with_id(app, "notifications", tr("Notifications", "Notifications"), true, notifications, None::<&str>)?));
    items.push(Box::new(CheckMenuItem::with_id(app, "autostart", tr("Lancer à l’ouverture de session", "Launch at login"), true, launch_at_login, None::<&str>)?));
    items.push(Box::new(MenuItem::with_id(app, "server", tr("Changer de serveur…", "Change server…"), true, None::<&str>)?));
    items.push(Box::new(sep(app)?));
    items.push(Box::new(MenuItem::with_id(app, "quit", tr("Quitter", "Quit"), true, None::<&str>)?));

    let refs: Vec<&dyn IsMenuItem<Wry>> = items.iter().map(|b| b.as_ref()).collect();
    Menu::with_items(app, &refs)
}

/// Icône avec pastille rouge (non lues) dessinée par-dessus l'icône de l'application.
pub fn badge_icon(base: &Image<'_>, unread: i64) -> Image<'static> {
    let (w, h) = (base.width(), base.height());
    let mut px = base.rgba().to_vec();
    if unread > 0 {
        let r = (w.min(h) as f32) * 0.24;
        let (cx, cy) = (w as f32 - r - 1.0, r + 1.0);
        let ring = r * 0.18;
        for y in 0..h {
            for x in 0..w {
                let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                let i = ((y * w + x) * 4) as usize;
                if d <= r - ring {
                    px[i..i + 4].copy_from_slice(&[229, 72, 77, 255]);
                } else if d <= r {
                    px[i..i + 4].copy_from_slice(&[255, 255, 255, 255]);
                }
            }
        }
    }
    Image::new_owned(px, w, h)
}

pub fn apply(app: &AppHandle, tray: &TrayIcon, state: &TrayState, launch_at_login: bool, notifications: bool) {
    if let Ok(menu) = build_menu(app, state, launch_at_login, notifications) {
        let _ = tray.set_menu(Some(menu));
    }
    if let Some(base) = app.default_window_icon() {
        let _ = tray.set_icon(Some(badge_icon(base, state.unread)));
    }
    let tip = if state.unread > 0 { format!("Looty — {} {}", state.unread, tr("non lue(s)", "unread")) } else { "Looty".to_string() };
    let _ = tray.set_tooltip(Some(tip));
    // Compteur sur l'icône du lanceur (KDE, Dash to Dock… via l'API Unity).
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.set_badge_count(if state.unread > 0 { Some(state.unread) } else { None });
    }
}

/// Notification système ; un clic ouvre la page correspondante.
pub fn notify(app: &AppHandle, n: OsNotification) {
    let app = app.clone();
    std::thread::spawn(move || {
        let mut builder = notify_rust::Notification::new();
        builder.appname("Looty").summary(&n.title).body(&n.body).icon("looty").action("default", tr("Ouvrir", "Open"));
        builder.hint(notify_rust::Hint::DesktopEntry("looty".into()));
        match builder.show() {
            Ok(handle) => handle.wait_for_action(|action| {
                if action == "default" {
                    let path = n.path.clone();
                    let a = app.clone();
                    let _ = app.run_on_main_thread(move || crate::open_path(&a, &path));
                }
            }),
            Err(e) => tracing::warn!("notification: {e}"),
        }
    });
}

// ---------------------------------------------------------------------------
// Lancement à l'ouverture de session (~/.config/autostart/looty.desktop)
// ---------------------------------------------------------------------------

fn autostart_file() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("autostart").join("looty.desktop")
}

pub fn autostart_enabled() -> bool {
    autostart_file().exists()
}

/// Commande à lancer : le lanceur utilisé (enveloppe Nix comprise), sinon `looty`.
fn launcher() -> String {
    std::env::args()
        .next()
        .filter(|a| a.starts_with('/') && std::path::Path::new(a).exists())
        .unwrap_or_else(|| "looty".into())
}

pub fn set_autostart(on: bool) -> std::io::Result<()> {
    let path = autostart_file();
    if on {
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        std::fs::write(
            &path,
            format!(
                "[Desktop Entry]\nType=Application\nName=Looty\nExec={} --hidden\nIcon=looty\nX-GNOME-Autostart-enabled=true\nX-KDE-autostart-after=panel\nNoDisplay=true\n",
                launcher()
            ),
        )
    } else if path.exists() {
        std::fs::remove_file(path)
    } else {
        Ok(())
    }
}

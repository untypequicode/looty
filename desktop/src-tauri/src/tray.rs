//! Barre système : menu dynamique (non lues, tâches du jour / en retard), pastille,
//! notifications système et lancement à l'ouverture de session.

#[cfg(not(windows))]
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
    let ask = app.state::<crate::proxy::Shared>().config.read().map(|c| c.ask_download_location).unwrap_or(true);
    items.push(Box::new(CheckMenuItem::with_id(app, "ask-download", tr("Demander où enregistrer les fichiers", "Ask where to save files"), true, ask, None::<&str>)?));
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
    // Compteur sur l'icône du lanceur (KDE, Dash to Dock… via l'API Unity) ; Windows : pastille
    // rouge sur l'icône de la barre des tâches.
    if let Some(w) = app.get_webview_window("main") {
        #[cfg(windows)]
        {
            let _ = w.set_overlay_icon(if state.unread > 0 { Some(overlay_dot()) } else { None });
        }
        #[cfg(not(windows))]
        {
            let _ = w.set_badge_count(if state.unread > 0 { Some(state.unread) } else { None });
        }
    }
}

/// Pastille rouge (32 px) posée sur l'icône de la barre des tâches de Windows.
#[cfg(windows)]
fn overlay_dot() -> Image<'static> {
    const N: u32 = 32;
    let (c, r) = (N as f32 / 2.0, N as f32 / 2.0 - 1.0);
    let mut px = vec![0u8; (N * N * 4) as usize];
    for y in 0..N {
        for x in 0..N {
            let d = ((x as f32 + 0.5 - c).powi(2) + (y as f32 + 0.5 - c).powi(2)).sqrt();
            let i = ((y * N + x) * 4) as usize;
            if d <= r - 3.0 {
                px[i..i + 4].copy_from_slice(&[229, 72, 77, 255]);
            } else if d <= r {
                px[i..i + 4].copy_from_slice(&[255, 255, 255, 255]);
            }
        }
    }
    Image::new_owned(px, N, N)
}

/// Notification système. `actions` : (identifiant, libellé) ; « default » est le clic sur la
/// notification elle-même. `on_action` reçoit l'identifiant choisi (sur un fil à part).
/// Linux : service de notifications du bureau (D-Bus) ; Windows : notification « toast ».
pub fn toast(title: &str, body: &str, actions: &[(&str, &str)], on_action: impl Fn(&str) + Send + 'static) {
    #[cfg(target_os = "linux")]
    {
        let (title, body) = (title.to_string(), body.to_string());
        let actions: Vec<(String, String)> = actions.iter().map(|(a, l)| (a.to_string(), l.to_string())).collect();
        std::thread::spawn(move || {
            let mut builder = notify_rust::Notification::new();
            builder.appname("Looty").summary(&title).body(&body).icon("looty");
            for (id, label) in &actions {
                builder.action(id, label);
            }
            builder.hint(notify_rust::Hint::DesktopEntry("looty".into()));
            match builder.show() {
                Ok(handle) => handle.wait_for_action(|action| on_action(action)),
                Err(e) => tracing::warn!("notification: {e}"),
            }
        });
    }
    #[cfg(windows)]
    {
        use tauri_winrt_notification::Toast;
        // Installée (NSIS), l'application s'identifie par son identifiant ; en développement,
        // Windows n'affiche que les notifications d'une application connue : celle de PowerShell.
        let app_id = if cfg!(debug_assertions) { Toast::POWERSHELL_APP_ID } else { "app.looty.desktop" };
        let mut t = Toast::new(app_id).title(title).text1(body);
        for (id, label) in actions {
            if *id != "default" {
                t = t.add_button(label, id);
            }
        }
        let shown = t
            .on_activated(move |action| {
                on_action(action.as_deref().unwrap_or("default"));
                Ok(())
            })
            .show();
        if let Err(e) = shown {
            tracing::warn!("notification: {e}");
        }
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = (title, body, actions, on_action);
    }
}

/// Notification de Looty ; un clic ouvre la page correspondante.
pub fn notify(app: &AppHandle, n: OsNotification) {
    let app = app.clone();
    let mut actions = vec![("default", tr("Ouvrir", "Open"))];
    if n.done_task.is_some() {
        actions.push(("done", tr("Terminer", "Mark as done")));
    }
    toast(&n.title, &n.body, &actions, move |action| match action {
        "default" => {
            let path = n.path.clone();
            let a = app.clone();
            let _ = app.run_on_main_thread(move || crate::open_path(&a, &path));
        }
        "done" => {
            if let Some(task) = n.done_task.clone() {
                crate::mark_task_done(&app, task);
            }
        }
        _ => {}
    });
}

// ---------------------------------------------------------------------------
// Lancement à l'ouverture de session : ~/.config/autostart/looty.desktop (Linux),
// clé « Run » du registre de l'utilisateur (Windows)
// ---------------------------------------------------------------------------

#[cfg(not(windows))]
fn autostart_file() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("autostart").join("looty.desktop")
}

#[cfg(not(windows))]
pub fn autostart_enabled() -> bool {
    autostart_file().exists()
}

/// Commande à lancer : le lanceur utilisé (enveloppe Nix comprise), sinon `looty`.
#[cfg(not(windows))]
fn launcher() -> String {
    std::env::args()
        .next()
        .filter(|a| a.starts_with('/') && std::path::Path::new(a).exists())
        .unwrap_or_else(|| "looty".into())
}

#[cfg(not(windows))]
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

#[cfg(windows)]
const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";

/// `reg.exe` sans fenêtre de console.
#[cfg(windows)]
fn reg(args: &[&str]) -> std::io::Result<std::process::ExitStatus> {
    use std::os::windows::process::CommandExt;
    std::process::Command::new("reg").args(args).creation_flags(0x0800_0000).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status()
}

#[cfg(windows)]
pub fn autostart_enabled() -> bool {
    reg(&["query", RUN_KEY, "/v", "Looty"]).map(|s| s.success()).unwrap_or(false)
}

#[cfg(windows)]
pub fn set_autostart(on: bool) -> std::io::Result<()> {
    if on {
        let exe = std::env::current_exe()?;
        let command = format!("\"{}\" --hidden", exe.display());
        reg(&["add", RUN_KEY, "/v", "Looty", "/t", "REG_SZ", "/d", &command, "/f"])?;
    } else if autostart_enabled() {
        reg(&["delete", RUN_KEY, "/v", "Looty", "/f"])?;
    }
    Ok(())
}

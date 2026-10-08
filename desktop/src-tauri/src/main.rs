//! Looty pour le bureau (Linux) : fenêtre, barre système, service de fond et proxy local.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod latex;
mod offline;
mod proxy;
mod service;
mod tray;

use std::sync::{Arc, Mutex};
use tauri::{
    tray::{TrayIcon, TrayIconBuilder},
    webview::DownloadEvent,
    AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_opener::OpenerExt;
use url::Url;

use service::{french, Service, TrayState};

fn tr(fr: &'static str, en: &'static str) -> &'static str {
    if french() { fr } else { en }
}

/// État partagé de l'interface native.
struct Native {
    tray: Mutex<Option<TrayIcon>>,
    last_state: Mutex<TrayState>,
    service: Mutex<Option<Arc<Service>>>,
}

/// Affiche et met au premier plan la fenêtre principale.
pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn navigate_local(app: &AppHandle, path: &str) {
    let proxy = app.state::<proxy::Shared>();
    if let (Some(w), Ok(url)) = (app.get_webview_window("main"), Url::parse(&format!("{}{}", proxy.local_origin(), path))) {
        let _ = w.navigate(url);
    }
}

/// Ouvre une page de Looty dans la fenêtre principale.
pub fn open_path(app: &AppHandle, path: &str) {
    show_main(app);
    navigate_local(app, path);
}

/// Règles de navigation de la fenêtre :
/// - l'application locale : autorisée ;
/// - le serveur Looty lui-même (retour SSO, lien copié depuis le site) : ramené en local ;
/// - pendant une connexion SSO : toutes les pages de la chaîne (fournisseur, fournisseur
///   externe comme GitHub…) restent dans la fenêtre — elles partagent ainsi leurs cookies,
///   sinon le fournisseur refuse le retour (« User Agent does not correspond ») ;
/// - tout autre site : ouvert dans le navigateur par défaut.
fn on_navigation(app: &AppHandle, url: &Url) -> bool {
    let allowed = decide_navigation(app, url);
    // Sans la requête : elle peut contenir le secret d'amorçage ou un code de connexion.
    tracing::debug!("navigation {}{} -> {}", url.origin().ascii_serialization(), url.path(), if allowed { "allow" } else { "redirect/external" });
    allowed
}

fn decide_navigation(app: &AppHandle, url: &Url) -> bool {
    let proxy = app.state::<proxy::Shared>();
    if matches!(url.scheme(), "about" | "blob" | "data") {
        return true;
    }
    if url.origin().ascii_serialization() == proxy.local_origin() {
        // De retour dans Looty : la connexion est terminée (ou abandonnée).
        if !url.path().starts_with("/api/auth/") && !url.path().starts_with("/__looty/sso") {
            proxy.end_sign_in();
        }
        return true;
    }
    if let Some(server) = proxy.server() {
        if url.origin() == server.origin() {
            let mut path = url.path().to_string();
            if let Some(q) = url.query() {
                path = format!("{path}?{q}");
            }
            if let Some(f) = url.fragment() {
                path = format!("{path}#{f}");
            }
            let app = app.clone();
            tauri::async_runtime::spawn(async move { navigate_local(&app, &path) });
            return false;
        }
    }
    if proxy.signing_in() && matches!(url.scheme(), "http" | "https") {
        return true;
    }
    if matches!(url.scheme(), "http" | "https" | "mailto") {
        let _ = app.opener().open_url(url.as_str(), None::<&str>);
    }
    false
}

/// Action demandée en ligne de commande (premier lancement ou instance déjà ouverte).
enum Action {
    Show,
    QuickAdd,
    Open(String),
}

/// Lien `looty://w/<espace>/doc/<page>` ou `--open /w/…` → chemin interne.
/// N'importe quel site peut déclencher un lien looty:// : seules les pages de Looty
/// sont acceptées (jamais l'API, la déconnexion ou les pages internes de l'application).
fn safe_path(raw: &str) -> Option<String> {
    let path = match raw.strip_prefix("looty://") {
        Some(rest) => format!("/{}", rest.trim_start_matches('/')),
        None => raw.to_string(),
    };
    let route = path.split(['?', '#']).next().unwrap_or("");
    let ok = path.starts_with("/w/")
        && !route.contains("..")
        && !route.contains("//")
        && route.chars().all(|c| c.is_ascii_alphanumeric() || "/-_.".contains(c))
        && path.len() < 2048;
    ok.then_some(path)
}

fn action_from(args: &[String]) -> Action {
    let mut it = args.iter().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--quick-add" => return Action::QuickAdd,
            "--open" => {
                if let Some(p) = it.next().and_then(|p| safe_path(p)) {
                    return Action::Open(p);
                }
            }
            s if s.starts_with("looty://") => {
                if let Some(p) = safe_path(s) {
                    return Action::Open(p);
                }
                tracing::warn!("ignored looty:// link (only Looty pages can be opened)");
            }
            _ => {}
        }
    }
    Action::Show
}

fn run_action(app: &AppHandle, action: Action) {
    match action {
        Action::Show => show_main(app),
        Action::QuickAdd => open_quick_add(app),
        Action::Open(path) => open_path(app, &path),
    }
}

fn notifications_on(app: &AppHandle) -> bool {
    app.state::<proxy::Shared>().config.read().map(|c| c.notifications).unwrap_or(true)
}

fn refresh_tray(app: &AppHandle) {
    let native = app.state::<Native>();
    let state = native.last_state.lock().unwrap().clone();
    let tray_icon = native.tray.lock().unwrap().clone();
    if let Some(t) = tray_icon {
        tray::apply(app, &t, &state, tray::autostart_enabled(), notifications_on(app));
    }
}

fn service(app: &AppHandle) -> Option<Arc<Service>> {
    app.state::<Native>().service.lock().unwrap().clone()
}

fn on_tray_menu(app: &AppHandle, id: &str) {
    match id {
        "open" => show_main(app),
        "inbox" | "my-tasks" => {
            let suffix = if id == "inbox" { "inbox" } else { "my-tasks" };
            let ws = app.state::<Native>().last_state.lock().unwrap().workspace.clone();
            match ws {
                Some(w) => open_path(app, &format!("/w/{w}/{suffix}")),
                None => open_path(app, "/"),
            }
        }
        "new-task" => open_quick_add(app),
        "server" => open_path(app, "/__looty/setup"),
        "notifications" => {
            let proxy = app.state::<proxy::Shared>();
            let on = {
                let mut cfg = proxy.config.write().unwrap();
                cfg.notifications = !cfg.notifications;
                let _ = cfg.save();
                cfg.notifications
            };
            if let Some(s) = service(app) {
                *s.notifications_enabled.lock().unwrap() = on;
            }
            refresh_tray(app);
        }
        "ask-download" => {
            if let Ok(mut cfg) = app.state::<proxy::Shared>().config.write() {
                cfg.ask_download_location = !cfg.ask_download_location;
                let _ = cfg.save();
            }
            refresh_tray(app);
        }
        "autostart" => {
            if let Err(e) = tray::set_autostart(!tray::autostart_enabled()) {
                tracing::warn!("autostart: {e}");
            }
            refresh_tray(app);
        }
        "quit" => app.exit(0),
        other => {
            if let Some(path) = other.strip_prefix("open-task|") {
                open_path(app, path);
            } else if let Some(task) = other.strip_prefix("done-task|") {
                if let Some(svc) = service(app) {
                    let task = task.to_string();
                    tauri::async_runtime::spawn(async move {
                        // Même règle que l'interface (validation par personne comprise).
                        svc.put_json(&format!("/api/boards/tasks/{task}/toggle_validation"), serde_json::json!({})).await;
                        svc.refresh_soon();
                    });
                }
            }
        }
    }
}

/// Petite fenêtre d'ajout rapide de tâche.
fn open_quick_add(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("quick-add") {
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let proxy = app.state::<proxy::Shared>();
    let Ok(url) = Url::parse(&format!("{}/__looty/quick-add", proxy.local_origin())) else { return };
    let nav = app.clone();
    let _ = WebviewWindowBuilder::new(app, "quick-add", WebviewUrl::External(url))
        .title(tr("Nouvelle tâche — Looty", "New task — Looty"))
        .inner_size(460.0, 210.0)
        .resizable(false)
        .always_on_top(true)
        .center()
        .on_navigation(move |u| u.origin().ascii_serialization() == nav.state::<proxy::Shared>().local_origin())
        .build();
}

/// Fenêtre du site Moodle officiel (cookies gardés : la session CAS reste ouverte).
/// En connexion (`login`), la redirection finale de Moodle vers `moodlemobile://token=…`
/// est interceptée : la clé part vers la fenêtre principale, qui connecte le compte.
fn open_moodle_window(app: &AppHandle, url: &str, login: bool) {
    let Ok(target) = Url::parse(url) else { return };
    if let Some(w) = app.get_webview_window("moodle") {
        let _ = w.navigate(target);
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let nav = app.clone();
    let built = WebviewWindowBuilder::new(app, "moodle", WebviewUrl::External(target))
        .title(if login { tr("Connexion à Moodle — Looty", "Moodle sign-in — Looty") } else { "Moodle — Looty" })
        .inner_size(1180.0, 820.0)
        .min_inner_size(380.0, 480.0)
        .on_navigation(move |u| moodle_navigation(&nav, u))
        .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
        .build();
    if let Ok(w) = built {
        enable_spellcheck(&w);
    }
}

fn moodle_navigation(app: &AppHandle, url: &Url) -> bool {
    match url.scheme() {
        "http" | "https" | "about" | "blob" | "data" => true,
        // Clé de l'application mobile : jamais affichée, transmise à Looty.
        "moodlemobile" | "web+looty" => {
            tracing::info!("moodle sign-in finished");
            let link = serde_json::to_string(url.as_str()).unwrap_or_default();
            let app = app.clone();
            let _ = app.clone().run_on_main_thread(move || {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.eval(format!("window.dispatchEvent(new CustomEvent('looty:moodle-token', {{ detail: {link} }}))"));
                }
                if let Some(w) = app.get_webview_window("moodle") {
                    let _ = w.close();
                }
                show_main(&app);
            });
            false
        }
        "mailto" => {
            let _ = app.opener().open_url(url.as_str(), None::<&str>);
            false
        }
        _ => false,
    }
}

/// Correcteur orthographique de WebKitGTK (désactivé par défaut) : langue du système,
/// plus le français et l'anglais (dictionnaires hunspell installés via enchant).
fn enable_spellcheck(window: &tauri::WebviewWindow) {
    let _ = window.with_webview(|wv| {
        use webkit2gtk::{WebContextExt, WebViewExt};
        let Some(ctx) = wv.inner().context() else { return };
        let mut langs: Vec<String> = Vec::new();
        for var in ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"] {
            if let Ok(v) = std::env::var(var) {
                for part in v.split(':') {
                    let code = part.split(['.', '@']).next().unwrap_or("").to_string();
                    if code.len() >= 2 && code != "C" && code != "POSIX" && !langs.contains(&code) {
                        langs.push(code);
                    }
                }
            }
        }
        for l in ["fr_FR", "en_US"] {
            if !langs.iter().any(|x| x == l) {
                langs.push(l.into());
            }
        }
        let refs: Vec<&str> = langs.iter().map(String::as_str).collect();
        ctx.set_spell_checking_enabled(true);
        ctx.set_spell_checking_languages(&refs);
    });
}

/// Sans les schémas GSettings de GTK, la fenêtre de fichiers ferait planter le processus :
/// dans ce cas, enregistrement direct dans Téléchargements.
fn file_chooser_available() -> bool {
    let ok = gtk::gio::SettingsSchemaSource::default().and_then(|s| s.lookup("org.gtk.Settings.FileChooser", true)).is_some();
    if !ok {
        tracing::warn!("GTK file chooser schema missing: saving to the Downloads folder");
    }
    ok
}

/// Fenêtre « Enregistrer sous » native (portail XDG si disponible). Appelée sur le fil
/// principal GTK (signal de WebKit) : `run()` tourne une boucle imbriquée.
fn ask_save_path(app: &AppHandle, dir: &std::path::Path, name: &str) -> Option<std::path::PathBuf> {
    use gtk::prelude::*;
    let parent = app.get_webview_window("main").and_then(|w| w.gtk_window().ok());
    let dialog = gtk::FileChooserNative::new(Some(tr("Enregistrer le fichier", "Save file")), parent.as_ref(), gtk::FileChooserAction::Save, Some(tr("Enregistrer", "Save")), Some(tr("Annuler", "Cancel")));
    dialog.set_do_overwrite_confirmation(true);
    let _ = dialog.set_current_folder(dir);
    dialog.set_current_name(name);
    let res = dialog.run();
    let path = if res == gtk::ResponseType::Accept { dialog.filename() } else { None };
    dialog.destroy();
    path
}

/// Téléchargements (exports, pièces jointes) : « Enregistrer sous » ou dossier
/// Téléchargements avec un nom unique.
fn on_download(app: &AppHandle, event: DownloadEvent<'_>) -> bool {
    match event {
        DownloadEvent::Requested { url, destination } => {
            let dir = dirs::download_dir().unwrap_or_else(|| dirs::home_dir().unwrap_or_default());
            let suggested = destination
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .filter(|n| !n.is_empty())
                .or_else(|| url.path_segments().and_then(|mut s| s.next_back().map(str::to_string)).filter(|n| !n.is_empty()))
                .unwrap_or_else(|| "download".into());
            let (stem, ext) = match suggested.rsplit_once('.') {
                Some((s, e)) if !s.is_empty() => (s.to_string(), format!(".{e}")),
                _ => (suggested.clone(), String::new()),
            };
            let ask = app.state::<proxy::Shared>().config.read().map(|c| c.ask_download_location).unwrap_or(true);
            if ask && file_chooser_available() {
                // Annuler la fenêtre annule le téléchargement.
                return match ask_save_path(app, &dir, &suggested) {
                    Some(p) => {
                        *destination = p;
                        true
                    }
                    None => false,
                };
            }
            let mut target = dir.join(&suggested);
            let mut i = 1;
            while target.exists() {
                target = dir.join(format!("{stem} ({i}){ext}"));
                i += 1;
            }
            let _ = std::fs::create_dir_all(&dir);
            *destination = target;
            true
        }
        DownloadEvent::Finished { path, success, .. } => {
            if let Some(path) = path {
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let body = if success { format!("{} {name}", tr("Enregistré :", "Saved:")) } else { format!("{} {name}", tr("Échec du téléchargement :", "Download failed:")) };
                let folder = path.parent().map(|p| p.to_path_buf());
                let app = app.clone();
                std::thread::spawn(move || {
                    let mut n = notify_rust::Notification::new();
                    n.appname("Looty").summary("Looty").body(&body).icon("looty");
                    if success && folder.is_some() {
                        n.action("default", tr("Ouvrir le dossier", "Open folder"));
                    }
                    if let Ok(h) = n.show() {
                        h.wait_for_action(|a| {
                            if a == "default" {
                                if let Some(f) = &folder {
                                    let _ = app.opener().open_path(f.to_string_lossy(), None::<&str>);
                                }
                            }
                        });
                    }
                });
            }
            true
        }
        _ => true,
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "looty=info,warn".into()))
        .init();
    // Pilote NVIDIA propriétaire : le rendu DMA-BUF de WebKitGTK donne une fenêtre vide.
    if std::path::Path::new("/proc/driver/nvidia").exists() && std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
    // Lancement à l'ouverture de session (--hidden) ou ajout rapide : sans la fenêtre.
    let args: Vec<String> = std::env::args().collect();
    let first_action = action_from(&args);
    let start_hidden = args.iter().any(|a| a == "--hidden") || matches!(first_action, Action::QuickAdd);
    // Clé de chiffrement de la session (trousseau système), avant de lire la session.
    if config::init_session_key() {
        tracing::info!("session encrypted with a key from the system keyring");
    }

    tauri::Builder::default()
        // Une seule instance : relancer l'application ramène la fenêtre existante.
        // Les arguments (lien looty://, --quick-add…) sont transmis à l'instance ouverte.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| run_action(app, action_from(&argv))))
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            let cfg = config::Config::load();
            let notifications = cfg.notifications;
            let jar = config::load_jar();
            config::migrate_plain_session(&jar);
            let proxy = tauri::async_runtime::block_on(proxy::start(cfg, jar))?;
            if let Action::Open(path) = &first_action {
                *proxy.initial_path.lock().unwrap() = Some(path.clone());
            }
            let boot: Url = proxy.boot_url().parse()?;
            let handle = app.handle().clone();
            app.manage(Native { tray: Mutex::new(None), last_state: Mutex::new(TrayState::default()), service: Mutex::new(None) });

            let (open_handle, done_handle, qa_handle, moodle_handle) = (handle.clone(), handle.clone(), handle.clone(), handle.clone());
            let _ = proxy.hooks.set(proxy::Hooks {
                open_url: Box::new(move |url| {
                    // Tests automatisés (débogage uniquement) : navigateur imposé.
                    if cfg!(debug_assertions) {
                        if let Ok(cmd) = std::env::var("LOOTY_OPEN_WITH") {
                            let _ = std::process::Command::new(cmd).arg(url).spawn();
                            return;
                        }
                    }
                    let _ = open_handle.opener().open_url(url, None::<&str>);
                }),
                // Connexion terminée dans le navigateur : la fenêtre revient au premier plan.
                signed_in: Box::new(move || {
                    let h = done_handle.clone();
                    let _ = done_handle.run_on_main_thread(move || {
                        show_main(&h);
                        if let Some(s) = service(&h) {
                            s.refresh_soon();
                        }
                    });
                }),
                // Modifications hors ligne synchronisées : notification (refus compris).
                synced: Box::new({
                    let h = handle.clone();
                    move |sent, refused| {
                        if sent == 0 && refused == 0 {
                            return;
                        }
                        let body = match (sent, refused, french()) {
                            (s, 0, true) => format!("{s} modification(s) faite(s) hors ligne synchronisée(s)."),
                            (s, 0, false) => format!("{s} offline change(s) synced."),
                            (0, r, true) => format!("{r} modification(s) hors ligne refusée(s) par le serveur."),
                            (0, r, false) => format!("{r} offline change(s) refused by the server."),
                            (s, r, true) => format!("{s} modification(s) synchronisée(s), {r} refusée(s) par le serveur."),
                            (s, r, false) => format!("{s} change(s) synced, {r} refused by the server."),
                        };
                        tray::notify(&h, service::OsNotification { title: "Looty".into(), body, path: "/".into() });
                    }
                }),
                open_moodle: Box::new(move |url, login| {
                    let h = moodle_handle.clone();
                    let _ = moodle_handle.run_on_main_thread(move || open_moodle_window(&h, &url, login));
                }),
                quick_add_closed: Box::new(move |created| {
                    let h = qa_handle.clone();
                    let _ = qa_handle.run_on_main_thread(move || {
                        if let Some(w) = h.get_webview_window("quick-add") {
                            let _ = w.close();
                        }
                        if let Some(title) = created {
                            if let Some(s) = service(&h) {
                                s.refresh_soon();
                            }
                            if let Some(w) = h.get_webview_window("main") {
                                let _ = w.eval("window.dispatchEvent(new CustomEvent('looty:refresh'))");
                            }
                            let body = format!("{} {title}", tr("Tâche créée :", "Task created:"));
                            tray::notify(&h, service::OsNotification { title: "Looty".into(), body, path: "/".into() });
                        }
                    });
                }),
            });
            let proxy_state = proxy.clone();
            app.manage(proxy);

            let nav_handle = handle.clone();
            let popup_handle = handle.clone();
            let dl_handle = handle.clone();
            let main_window = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(boot))
                .title("Looty")
                .inner_size(1280.0, 820.0)
                .min_inner_size(380.0, 480.0)
                .visible(!start_hidden)
                .on_navigation(move |url| on_navigation(&nav_handle, url))
                // Liens « nouvel onglet » : même règles, jamais de seconde fenêtre.
                .on_new_window(move |url, _features| {
                    if on_navigation(&popup_handle, &url) {
                        let app = popup_handle.clone();
                        let path = format!("{}{}", url.path(), url.query().map(|q| format!("?{q}")).unwrap_or_default());
                        tauri::async_runtime::spawn(async move { navigate_local(&app, &path) });
                    }
                    tauri::webview::NewWindowResponse::Deny
                })
                .on_download(move |_webview, event| on_download(&dl_handle, event))
                .build()?;
            enable_spellcheck(&main_window);

            // Barre système
            let menu_handle = handle.clone();
            match TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().cloned().expect("icon"))
                .tooltip("Looty")
                .menu(&tray::build_menu(&handle, &TrayState::default(), tray::autostart_enabled(), notifications)?)
                .on_menu_event(move |_app, ev| on_tray_menu(&menu_handle, ev.id().as_ref()))
                .build(app)
            {
                Ok(t) => *app.state::<Native>().tray.lock().unwrap() = Some(t),
                Err(e) => tracing::warn!("tray icon unavailable: {e}"),
            }

            // Service de fond (temps réel, notifications, état de la barre système)
            let (tray_handle, notif_handle, focus_handle) = (handle.clone(), handle.clone(), handle.clone());
            let svc = Service::start(
                proxy_state,
                service::Ui {
                    tray: Box::new(move |state| {
                        let h = tray_handle.clone();
                        let _ = tray_handle.run_on_main_thread(move || {
                            *h.state::<Native>().last_state.lock().unwrap() = state;
                            refresh_tray(&h);
                        });
                    }),
                    notify: Box::new(move |n| tray::notify(&notif_handle, n)),
                    window_focused: Box::new(move || {
                        focus_handle.get_webview_window("main").is_some_and(|w| w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false))
                    }),
                },
                notifications,
            );
            *app.state::<Native>().service.lock().unwrap() = Some(svc);
            if matches!(first_action, Action::QuickAdd) {
                open_quick_add(&handle);
            }
            // Tests automatisés (débogage uniquement) : connexion Moodle lancée au démarrage.
            if cfg!(debug_assertions) {
                if let Ok(u) = std::env::var("LOOTY_TEST_MOODLE_LOGIN") {
                    let h = handle.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_secs(12));
                        let h2 = h.clone();
                        let _ = h.run_on_main_thread(move || open_moodle_window(&h2, &u, true));
                    });
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() != "main" {
                    return;
                }
                let close_to_tray = window.state::<proxy::Shared>().config.read().map(|c| c.close_to_tray).unwrap_or(true);
                if close_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Looty");
}

//! Looty pour le bureau (Linux) : fenêtre, barre système et proxy local.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod proxy;

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_opener::OpenerExt;
use url::Url;

fn french() -> bool {
    ["LC_ALL", "LC_MESSAGES", "LANG"].iter().find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty())).is_some_and(|v| v.starts_with("fr"))
}

fn tr(fr: &'static str, en: &'static str) -> &'static str {
    if french() { fr } else { en }
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

/// Règles de navigation de la fenêtre :
/// - l'application locale : autorisée ;
/// - le serveur Looty lui-même (retour SSO, lien copié depuis le site) : ramené en local ;
/// - pendant une connexion SSO : toutes les pages de la chaîne (fournisseur, fournisseur
///   externe comme GitHub…) restent dans la fenêtre — elles partagent ainsi leurs cookies,
///   sinon le fournisseur refuse le retour (« User Agent does not correspond ») ;
/// - tout autre site : ouvert dans le navigateur par défaut.
fn on_navigation(app: &AppHandle, url: &Url) -> bool {
    let allowed = decide_navigation(app, url);
    tracing::debug!("navigation {} -> {}", url, if allowed { "allow" } else { "redirect/external" });
    allowed
}

fn decide_navigation(app: &AppHandle, url: &Url) -> bool {
    let proxy = app.state::<proxy::Shared>();
    if url.scheme() == "about" {
        return true;
    }
    if url.origin().ascii_serialization() == proxy.local_origin() {
        // De retour dans Looty : la connexion est terminée (ou abandonnée).
        if !url.path().starts_with("/api/auth/") {
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

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", tr("Ouvrir Looty", "Open Looty"), true, None::<&str>)?;
    let server = MenuItem::with_id(app, "server", tr("Changer de serveur…", "Change server…"), true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", tr("Quitter", "Quit"), true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &PredefinedMenuItem::separator(app)?, &server, &PredefinedMenuItem::separator(app)?, &quit])?;
    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().cloned().expect("icon"))
        .tooltip("Looty")
        .menu(&menu)
        .on_menu_event(|app, ev| match ev.id().as_ref() {
            "open" => show_main(app),
            "server" => {
                show_main(app);
                navigate_local(app, "/__looty/setup");
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "looty=info,warn".into()))
        .init();
    // Pilote NVIDIA propriétaire : le rendu DMA-BUF de WebKitGTK donne une fenêtre vide.
    if std::path::Path::new("/proc/driver/nvidia").exists() && std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }

    tauri::Builder::default()
        // Une seule instance : relancer l'application ramène la fenêtre existante.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| show_main(app)))
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let cfg = config::Config::load();
            let jar = config::load_jar();
            let proxy = tauri::async_runtime::block_on(proxy::start(cfg, jar))?;
            let boot: Url = proxy.boot_url().parse()?;
            let handle = app.handle().clone();
            let (open_handle, done_handle) = (handle.clone(), handle.clone());
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
                    let _ = done_handle.run_on_main_thread(move || show_main(&h));
                }),
            });
            app.manage(proxy);

            let nav_handle = handle.clone();
            let popup_handle = handle.clone();
            WebviewWindowBuilder::new(app, "main", WebviewUrl::External(boot))
                .title("Looty")
                .inner_size(1280.0, 820.0)
                .min_inner_size(380.0, 480.0)
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
                .build()?;

            if let Err(e) = build_tray(&handle) {
                tracing::warn!("tray icon unavailable: {e}");
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
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

mod access;
mod admin;
mod announcements;
mod archive;
mod auth;
mod board;
mod board_export;
mod caldav;
mod calendar;
mod classes;
mod crypto;
mod ical;
mod ics_feed;
mod dav;
mod db;
mod delivery;
mod demo;
mod demo_mode;
mod error;
mod mail;
mod mailbox;
mod mailclient;
mod moodle;
mod moodle_content;
mod moodle_grades;
mod moodle_cache;
mod projects;
mod public_project;
mod jupyter;
mod latex;
mod drive;
mod trash;
mod storage;
mod text;
mod bg;
mod events;
mod activity;
mod graph;
mod group_links;
mod groups;
mod keywords;
mod releases;
mod notifications;
mod push;
mod recurrence;
mod page;
mod personal;
mod prefs;
mod comments;
mod revisions;
mod roles;
mod planning;
mod polls;
mod analytics;
mod backup;
mod session_store;
mod state;
mod upload;
mod user_settings;
mod workspace;
mod ws;

use axum::{
    extract::DefaultBodyLimit,
    http::{header, HeaderValue},
    routing::{get, post},
    Json, Router,
};
use std::net::SocketAddr;
use time::Duration;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_sessions::{Expiry, SessionManagerLayer};

async fn health_handler() -> Json<serde_json::Value> {
    // `desktop_sso` / `android_sso` : les applications peuvent se connecter via le navigateur système.
    Json(serde_json::json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION"), "desktop_sso": true, "android_sso": true }))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,sqlx=warn".into()),
        )
        .init();

    // `backend seed-demo [fr] [en]` : génère l'espace « Looty Démo » (GLOSSARY : Démo) et s'arrête.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("seed-demo") {
        let pool = db::init_db().await.expect("Failed to initialize database");
        std::process::exit(demo::run(state::AppState::new(pool, base_url()), &args[1..]).await);
    }

    if std::env::var("LOOTY_DEV_LOGIN").as_deref() == Ok("true") {
        tracing::warn!("LOOTY_DEV_LOGIN is enabled: passwordless login at /api/auth/dev-login. NEVER enable this in production.");
    }
    let pool = db::init_db().await.expect("Failed to initialize database");
    archive::start_archive_cron(pool.clone());
    let demo = demo_mode::enabled();
    if !demo {
        backup::start_scheduler(pool.clone());
    }
    if demo {
        tracing::warn!("LOOTY_DEMO is enabled: public demo instance, one throwaway copy per visitor, OIDC and local accounts disabled. NEVER enable this on a real instance.");
    } else {
        seed_default_provider(&pool).await;
    }

    let session_store = session_store::PgSessionStore::new(pool.clone());
    session_store.migrate().await.expect("Failed to migrate session store");
    session_store.start_cleanup();

    let state = state::AppState::new(pool.clone(), base_url());
    if demo {
        // Aucune tâche de fond qui sorte vers l'extérieur ; seul le ménage des copies tourne.
        demo_mode::start_purge_loop(state.clone());
        trash::start_purge_loop(state.clone());
        drive::start_purge_loop(state.clone());
    } else {
        caldav::start_sync_loop(state.clone(), state.caldav_notify.clone());
        delivery::start_worker(state.clone());
        calendar::start_sync_loop(state.clone());
        mailbox::start_poll_loop(state.clone());
        moodle::start_poll_loop(state.clone());
        trash::start_purge_loop(state.clone());
        drive::start_purge_loop(state.clone());
        jupyter::start_reaper();
    }
    let app = app(state);

    tokio::fs::create_dir_all(upload::UPLOAD_DIR).await.ok();

    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8080);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind failed");
    axum::serve(listener, app).await.expect("server error");
}

fn base_url() -> String {
    std::env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:3000".to_string())
}

/// Toutes les routes HTTP (API, fichiers, CalDAV) avec la session et les en-têtes de sécurité.
/// La commande `seed-demo` l'appelle aussi, sans ouvrir de port, pour passer par l'API.
fn app(state: state::AppState) -> Router {
    let session_layer = SessionManagerLayer::new(session_store::PgSessionStore::new(state.pool.clone()))
        .with_name("looty_session")
        .with_secure(state.base_url.starts_with("https://"))
        .with_http_only(true)
        .with_same_site(tower_sessions::cookie::SameSite::Lax)
        .with_expiry(Expiry::OnInactivity(Duration::days(14)));

    let api = Router::new()
        .route("/health", get(health_handler))
        .route("/settings", get(admin::get_public_settings))
        .route("/ws", get(ws::ws_handler))
        .route("/ics/{file}", get(ics_feed::feed))
        .route("/upload", post(upload::upload_file))
        .route("/public/pages/{token}", get(page::get_public_page))
        .route("/public/pages/{token}/notebook", get(public_project::notebook))
        .route("/public/pages/{token}/files/{*path}", get(public_project::image))
        .route("/public/pages/{token}/output.pdf", get(public_project::pdf))
        .nest("/auth", auth::routes().nest("/demo", demo_mode::routes()))
        .nest("/admin", admin::routes())
        .nest("/user", user_settings::routes().merge(prefs::user_routes()))
        .merge(roles::user_routes())
        .nest("/user/releases", releases::user_routes())
        .nest("/releases", releases::routes())
        .nest("/user/caldav", caldav::routes())
        .nest("/user/notifications", delivery::routes())
        .nest("/user/push", push::routes())
        .nest("/admin/mail", mail::admin_routes())
        .nest("/admin/storage", storage::routes())
        .nest("/admin/insights", analytics::admin_routes())
        .nest("/admin/backups", backup::routes())
        .route("/boards/tasks/{task_id}/caldav", get(caldav::task_links))
        .nest("/notifications", notifications::routes())
        .nest("/invites", workspace::invite_routes())
        .nest("/pages", page::access_routes().merge(revisions::routes()))
        .nest("/comments", comments::routes())
        .nest("/workspaces", workspace::routes())
        .nest("/workspaces/{workspace_id}/boards", board::workspace_routes())
        .nest("/workspaces/{workspace_id}/pages", page::routes())
        .nest("/workspaces/{workspace_id}/canvases", page::canvas_routes())
        .nest("/workspaces/{workspace_id}/calendars", calendar::workspace_routes())
        .nest("/calendars", calendar::routes())
        .nest("/mail", mailbox::routes().merge(mailclient::routes()))
        .nest("/workspaces/{workspace_id}/trash", trash::workspace_routes())
        .nest("/workspaces/{workspace_id}/events", events::workspace_routes())
        .nest("/workspaces/{workspace_id}/activity", activity::workspace_routes())
        .nest("/workspaces/{workspace_id}/prefs", prefs::workspace_routes())
        .nest("/workspaces/{workspace_id}/comments", comments::workspace_routes())
        .nest("/workspaces/{workspace_id}/access", roles::workspace_routes())
        .nest("/workspaces/{workspace_id}/groups", groups::workspace_routes())
        .nest("/workspaces/{workspace_id}/keywords", keywords::workspace_routes())
        .nest("/keywords", keywords::routes())
        .nest("/personal", personal::routes())
        .nest("/workspaces/{workspace_id}/announcements", announcements::workspace_routes())
        .nest("/workspaces/{workspace_id}/planning", planning::workspace_routes())
        .nest("/workspaces/{workspace_id}", classes::workspace_routes())
        .nest("/workspaces/{workspace_id}/moodle", moodle::workspace_routes())
        .nest("/workspaces/{workspace_id}/projects", projects::workspace_routes())
        .nest("/workspaces/{workspace_id}/drive", drive::workspace_routes())
        .nest("/projects", projects::routes().merge(jupyter::routes()))
        .nest("/drive/public", drive::public_routes())
        .nest("/workspaces/{workspace_id}/admin", analytics::workspace_routes())
        .nest("/workspaces/{workspace_id}", graph::workspace_routes())
        .nest("/trash", trash::routes())
        .nest("/boards", board::routes());

    Router::new()
        .route("/uploads/{name}", get(upload::serve_upload))
        .merge(projects::public_routes())
        .nest("/api", api)
        .merge(dav::routes())
        .layer(axum::middleware::from_fn_with_state(state.clone(), demo_mode::guard))
        .with_state(state)
        .layer(DefaultBodyLimit::max(50 * 1024 * 1024))
        .layer(session_layer)
        .layer(SetResponseHeaderLayer::if_not_present(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::REFERRER_POLICY,
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::X_FRAME_OPTIONS,
            HeaderValue::from_static("DENY"),
        ))
}

/// Crée le fournisseur OIDC par défaut à partir des variables d'environnement si aucun n'existe.
async fn seed_default_provider(pool: &sqlx::PgPool) {
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM oauth_providers").fetch_one(pool).await.unwrap_or(0);
    if count > 0 {
        return;
    }
    let issuer = std::env::var("OIDC_ISSUER").unwrap_or_default();
    let client_id = std::env::var("OIDC_CLIENT_ID").unwrap_or_default();
    let secret = std::env::var("OIDC_CLIENT_SECRET").unwrap_or_default();
    if issuer.is_empty() || client_id.is_empty() || secret.is_empty() {
        return;
    }
    let provider_id = match parse_provider_id(&std::env::var("OIDC_PROVIDER_ID").unwrap_or_default()) {
        Ok(id) => id,
        Err(value) => {
            tracing::error!(
                "OIDC_PROVIDER_ID={value:?} n'est pas un UUID valide : fournisseur OIDC par défaut non créé"
            );
            return;
        }
    };
    let res = sqlx::query(
        "INSERT INTO oauth_providers (id, name, issuer_url, client_id, client_secret) \
         VALUES (COALESCE($1, gen_random_uuid()), 'SSO', $2, $3, $4)",
    )
    .bind(provider_id)
    .bind(issuer)
    .bind(client_id)
    .bind(secret)
    .execute(pool)
    .await;
    match res {
        Ok(_) => tracing::info!("Seeded default OAuth provider from environment variables"),
        Err(e) => tracing::error!("Échec de la création du fournisseur OIDC par défaut : {e}"),
    }
}

/// Lit `OIDC_PROVIDER_ID` : vide → identifiant aléatoire ; sinon un UUID, qui fixe l'URL de redirection
/// (`<BASE_URL>/api/auth/callback/<id>`) avant le premier démarrage. Rend la valeur fautive en erreur.
fn parse_provider_id(value: &str) -> Result<Option<uuid::Uuid>, String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    uuid::Uuid::parse_str(value).map(Some).map_err(|_| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::parse_provider_id;

    #[test]
    fn provider_id_empty_means_random() {
        assert_eq!(parse_provider_id(""), Ok(None));
        assert_eq!(parse_provider_id("  "), Ok(None));
    }

    #[test]
    fn provider_id_accepts_uuid() {
        let id = parse_provider_id(" 0b6c1f0e-8a4e-4c1e-9d55-3f5a2b7c9e10 ").unwrap().unwrap();
        assert_eq!(id.to_string(), "0b6c1f0e-8a4e-4c1e-9d55-3f5a2b7c9e10");
    }

    #[test]
    fn provider_id_rejects_garbage() {
        assert_eq!(parse_provider_id("looty"), Err("looty".to_string()));
    }
}

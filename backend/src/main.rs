mod access;
mod admin;
mod archive;
mod auth;
mod board;
mod caldav;
mod crypto;
mod ical;
mod dav;
mod db;
mod delivery;
mod error;
mod mail;
mod notifications;
mod push;
mod page;
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
use std::{net::SocketAddr, sync::Arc};
use time::Duration;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_sessions::{Expiry, SessionManagerLayer};
use tower_sessions_sqlx_store::PostgresStore;

async fn health_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") }))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,sqlx=warn".into()),
        )
        .init();

    if std::env::var("LOOTY_DEV_LOGIN").as_deref() == Ok("true") {
        tracing::warn!("LOOTY_DEV_LOGIN is enabled: passwordless login at /api/auth/dev-login. NEVER enable this in production.");
    }
    let pool = db::init_db().await.expect("Failed to initialize database");
    archive::start_archive_cron(pool.clone());
    seed_default_provider(&pool).await;

    let base_url = std::env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());
    let secure_cookies = base_url.starts_with("https://");

    let session_store = PostgresStore::new(pool.clone());
    session_store.migrate().await.expect("Failed to migrate session store");
    let session_layer = SessionManagerLayer::new(session_store)
        .with_name("looty_session")
        .with_secure(secure_cookies)
        .with_http_only(true)
        .with_same_site(tower_sessions::cookie::SameSite::Lax)
        .with_expiry(Expiry::OnInactivity(Duration::days(14)));

    let state = state::AppState {
        pool: pool.clone(),
        oidc_clients: Arc::new(dashmap::DashMap::new()),
        hub: Arc::new(ws::Hub::new()),
        base_url,
        caldav_notify: Arc::new(tokio::sync::Notify::new()),
    };
    caldav::start_sync_loop(state.clone(), state.caldav_notify.clone());
    delivery::start_worker(state.clone());

    let api = Router::new()
        .route("/health", get(health_handler))
        .route("/settings", get(admin::get_public_settings))
        .route("/ws", get(ws::ws_handler))
        .route("/upload", post(upload::upload_file))
        .route("/public/pages/:token", get(page::get_public_page))
        .nest("/auth", auth::routes())
        .nest("/admin", admin::routes())
        .nest("/user", user_settings::routes())
        .nest("/user/caldav", caldav::routes())
        .nest("/user/notifications", delivery::routes())
        .nest("/user/push", push::routes())
        .nest("/admin/mail", mail::admin_routes())
        .route("/boards/tasks/:task_id/caldav", get(caldav::task_links))
        .nest("/notifications", notifications::routes())
        .nest("/invites", workspace::invite_routes())
        .nest("/pages", page::access_routes())
        .nest("/workspaces", workspace::routes())
        .nest("/workspaces/:workspace_id/boards", board::workspace_routes())
        .nest("/workspaces/:workspace_id/pages", page::routes())
        .nest("/boards", board::routes());

    let app = Router::new()
        .route("/uploads/:name", get(upload::serve_upload))
        .nest("/api", api)
        .merge(dav::routes())
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
        ));

    tokio::fs::create_dir_all(upload::UPLOAD_DIR).await.ok();

    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8080);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind failed");
    axum::serve(listener, app).await.expect("server error");
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
    let res = sqlx::query(
        "INSERT INTO oauth_providers (name, issuer_url, client_id, client_secret) VALUES ('SSO', $1, $2, $3)",
    )
    .bind(issuer)
    .bind(client_id)
    .bind(secret)
    .execute(pool)
    .await;
    if res.is_ok() {
        tracing::info!("Seeded default OAuth provider from environment variables");
    }
}

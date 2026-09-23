mod auth;
mod db;
mod ws;
mod workspace;
mod board;
mod archive;
mod page;

use axum::{routing::get, Router, Json, Extension, extract::DefaultBodyLimit};
use serde::Serialize;
use std::{net::SocketAddr, sync::Arc};
use tower_sessions::{SessionManagerLayer, Expiry};
use tower_sessions_sqlx_store::PostgresStore;
use time::Duration;

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    version: &'static str,
}

async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: "0.1.0",
    })
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    
    // Initialisation DB et SQLx
    let pool = db::init_db().await.expect("Failed to initialize database");
    
    archive::start_archive_cron(pool.clone());

    // Client OIDC Zitadel
    let oidc_client = auth::build_oidc_client().await;

    let auth_state = auth::AuthState {
        pool: pool.clone(),
        oidc_client,
    };

    // Initialisation du Session Store dans Postgres
    let session_store = PostgresStore::new(pool.clone());
    session_store.migrate().await.expect("Failed to migrate session store");

    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(false) // Mettre à true en prod (HTTPS)
        .with_same_site(tower_sessions::cookie::SameSite::Lax)
        .with_expiry(Expiry::OnInactivity(Duration::days(7)));

    // État WebSocket
    let ws_state = Arc::new(ws::AppState::new());

    // Assemblage des routes
    let app = Router::new()
        .nest_service("/uploads", tower_http::services::ServeDir::new("uploads"))
        .route("/api/health", get(health_handler))
        .route("/api/ws", get(ws::ws_handler))
        .nest("/api/auth", auth::auth_routes(auth_state.clone()))
        .nest("/api/workspaces", workspace::workspace_routes(auth_state.clone()))
        .nest("/api/workspaces/:workspace_id/boards", board::workspace_board_routes(auth_state.clone()))
        .nest("/api/workspaces/:workspace_id/pages", page::page_routes(auth_state.clone()))
        .nest("/api/boards", board::board_routes(auth_state.clone()))
        .layer(DefaultBodyLimit::max(50 * 1024 * 1024))
        .layer(Extension(ws_state))
        .layer(session_layer);

    tokio::fs::create_dir_all("uploads").await.ok();

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    println!("Listening on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

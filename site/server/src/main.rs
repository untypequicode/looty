use looty_site::{app, Config};
use std::path::PathBuf;

/// Variables : PORT (8080), WEB_DIR (./web), DATA_DIR (./data), UPLOAD_TOKEN (dépôts désactivés si absent), SITE_URL.
#[tokio::main(flavor = "current_thread")]
async fn main() {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "looty_site=info".into())).init();
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    let cfg = Config {
        web: PathBuf::from(env("WEB_DIR").unwrap_or_else(|| "web".into())),
        data: PathBuf::from(env("DATA_DIR").unwrap_or_else(|| "data".into())),
        token: env("UPLOAD_TOKEN"),
        site_url: env("SITE_URL"),
    };
    if cfg.token.is_none() {
        tracing::warn!("UPLOAD_TOKEN absent : les dépôts de fichiers sont désactivés");
    }
    let addr = format!("0.0.0.0:{}", env("PORT").unwrap_or_else(|| "8080".into()));
    let listener = tokio::net::TcpListener::bind(&addr).await.expect("port");
    tracing::info!("site sur {addr}");
    axum::serve(listener, app(cfg)).with_graceful_shutdown(async { let _ = tokio::signal::ctrl_c().await; }).await.expect("serveur");
}

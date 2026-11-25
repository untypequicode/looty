#![allow(clippy::field_reassign_with_default)]
use clap::Parser;
use looty_bridge::latex_service::{router, Config};
use std::net::SocketAddr;

/// LaTeX compile service for the Looty PC tunnel (docs/tunnel-protocol.md, section 13).
/// Listens on loopback by default; the bridge forwards the instance's requests to it.
#[derive(Parser)]
#[command(name = "looty-latex-service", version)]
struct Cli {
    /// Address to listen on. Keep it on loopback (or a private container network): the service runs TeX on whatever it receives.
    #[arg(long, env = "LATEX_LISTEN", default_value = "127.0.0.1:8080")]
    listen: SocketAddr,
    /// Directory holding latexmk, pdflatex... (default: the PATH).
    #[arg(long, env = "LATEX_BIN_DIR")]
    bin_dir: Option<std::path::PathBuf>,
    /// Compilations at the same time.
    #[arg(long, env = "LATEX_SLOTS", default_value_t = 2)]
    slots: usize,
    /// Default time limit of a compilation, in seconds (a request can ask for less or more, up to --max-timeout).
    #[arg(long, env = "LATEX_TIMEOUT", default_value_t = 120)]
    timeout: u64,
    /// Longest time limit a request may ask for, in seconds.
    #[arg(long, env = "LATEX_MAX_TIMEOUT", default_value_t = 300)]
    max_timeout: u64,
    /// Where job directories are created (deleted after each job).
    #[arg(long, env = "LATEX_WORK_DIR")]
    work_dir: Option<std::path::PathBuf>,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let mut cfg = Config::default();
    cfg.bin_dir = cli.bin_dir;
    cfg.slots = cli.slots.max(1);
    cfg.default_timeout = std::time::Duration::from_secs(cli.timeout.max(1));
    cfg.max_timeout = std::time::Duration::from_secs(cli.max_timeout.max(cli.timeout).max(1));
    if let Some(d) = cli.work_dir {
        cfg.work_dir = d;
    }
    let listener = match tokio::net::TcpListener::bind(cli.listen).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("error: cannot listen on {}: {e}", cli.listen);
            std::process::exit(1);
        }
    };
    looty_bridge::log(format!("looty-latex-service listening on {}", cli.listen));
    let app = router(cfg);
    let _ = axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await;
}

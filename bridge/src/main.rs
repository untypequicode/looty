use clap::Parser;
use looty_bridge::config::{agent_url, Services};
use looty_bridge::log;
use looty_bridge::session::{run_forever, Params, AGENT};
use tokio::sync::watch;

/// Connects this PC to a Looty instance (PC tunnel) without opening any port.
#[derive(Parser)]
#[command(name = "looty-bridge", version)]
struct Cli {
    /// Instance address, for example https://looty.example.org
    #[arg(long, env = "LOOTY_INSTANCE")]
    instance: String,
    /// Tunnel token (prefer the environment variable: command lines end up in the shell history).
    #[arg(long, hide_env_values = true)]
    token: Option<String>,
    /// Name of the environment variable holding the token.
    #[arg(long, default_value = "LOOTY_TOKEN")]
    token_env: String,
    /// Local service to expose: name=http://127.0.0.1:8888 or name:kind=URL (kind: jupyter, latex, http). Repeatable; $LOOTY_SERVICES takes a comma separated list.
    #[arg(
        long = "service",
        value_name = "NAME=URL",
        env = "LOOTY_SERVICES",
        value_delimiter = ','
    )]
    services: Vec<String>,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let token = cli
        .token
        .or_else(|| std::env::var(&cli.token_env).ok())
        .unwrap_or_default();
    if token.trim().is_empty() {
        eprintln!("error: no token; set ${} or pass --token", cli.token_env);
        std::process::exit(2);
    }
    let token = token.trim().to_string();
    let url = agent_url(&cli.instance).unwrap_or_else(|e| fail(e));
    let services = Services::parse(&cli.services).unwrap_or_else(|e| fail(e));
    if services.0.is_empty() {
        log("warning: no --service declared, the instance will have nothing to use");
    }
    for s in services.0.values() {
        let host = s.base.trim_start_matches("http://");
        if !(host.starts_with("127.") || host.starts_with("localhost") || host.starts_with("[::1]"))
        {
            log(format!(
                "note: service '{}' is not on the loopback address",
                s.decl.name
            ));
        }
    }
    log(format!("{AGENT} -> {}", cli.instance.trim_end_matches('/')));
    let params = Params {
        url,
        token,
        services,
    };

    let (stx, srx) = watch::channel(false);
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        log("stopping (Ctrl+C)...");
        let _ = stx.send(true);
    });

    std::process::exit(run_forever(&params, srx).await);
}

fn fail(e: String) -> ! {
    eprintln!("error: {e}");
    std::process::exit(2);
}

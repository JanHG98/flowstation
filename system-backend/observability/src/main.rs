mod collector;
mod config;
mod http;
mod protocol;
mod state;

use std::path::PathBuf;

use clap::Parser;
use config::ObservabilityConfig;
use state::SharedObservability;

#[derive(Debug, Parser)]
#[command(name = "netcore-observability")]
#[command(about = "NetCore-Tetra metrics, logs, traces, alerting and NMS management plane")]
struct Args {
    #[arg(long, default_value = "/etc/netcore/observability.toml")]
    config: PathBuf,
    #[arg(long)]
    no_config: bool,
    #[arg(long)]
    bind: Option<std::net::SocketAddr>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "netcore_observability=info".into()),
        )
        .init();

    let args = Args::parse();
    let config_path = if args.no_config { None } else { Some(args.config.as_path()) };
    let mut config = ObservabilityConfig::load(config_path)?;
    config.apply_bind_override(args.bind).map_err(std::io::Error::other)?;

    tracing::warn!("Observability/NMS starts in OPEN LAB mode: no login, no tokens and no TLS");
    tracing::warn!("Place this service and all monitored management endpoints only on an isolated management network");
    tracing::info!("Observability WebUI/API bind={} scrape_interval={}s", config.server.bind, config.collection.scrape_interval_secs);

    let observability = SharedObservability::load(config.clone())?;
    let _collector = collector::spawn_collector(config.clone(), observability.clone());
    let server = http::spawn_http_server(config, observability)?;
    server.join().map_err(|_| -> Box<dyn std::error::Error> { "Observability HTTP server thread panicked".into() })?;
    Ok(())
}

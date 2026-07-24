mod config;
mod http;
mod model;
mod state;
mod worker;

use std::path::PathBuf;

use clap::Parser;
use config::ApplicationGatewayConfig;
use state::SharedGateway;

#[derive(Debug, Parser)]
#[command(name = "netcore-application-gateway")]
#[command(about = "NetCore-Tetra connector, webhook, routing, template and TTS orchestration service")]
struct Args {
    #[arg(long, default_value = "/etc/netcore/application-gateway.toml")]
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
                .unwrap_or_else(|_| "netcore_application_gateway=info".into()),
        )
        .init();

    let args = Args::parse();
    let config_path = if args.no_config { None } else { Some(args.config.as_path()) };
    let mut config = ApplicationGatewayConfig::load(config_path)?;
    config.apply_bind_override(args.bind).map_err(std::io::Error::other)?;

    tracing::warn!("Application Gateway starts in OPEN LAB management mode: no login, no management tokens and no TLS");
    tracing::warn!("External connector credentials are still secrets and are stored separately with redacted management responses");
    tracing::info!(
        "Application Gateway WebUI/API bind={} mode={} worker={}ms",
        config.server.bind,
        config.runtime.operating_mode,
        config.runtime.worker_interval_ms
    );

    let gateway = SharedGateway::load(config.clone())?;
    let _worker = worker::spawn_worker(config.clone(), gateway.clone());
    let server = http::spawn_http_server(config, gateway)?;
    server.join().map_err(|_| -> Box<dyn std::error::Error> {
        "Application Gateway HTTP server thread panicked".into()
    })?;
    Ok(())
}

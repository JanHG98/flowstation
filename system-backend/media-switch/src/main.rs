mod call_control;
mod config;
mod gateway;
mod http;
mod protocol;
mod state;

use std::path::PathBuf;
use std::sync::mpsc;

use clap::Parser;
use config::MediaSwitchConfig;
use state::SharedMedia;

#[derive(Debug, Parser)]
#[command(name = "netcore-media-switch")]
#[command(about = "NetCore central packed-TETRA speech-frame router and jitter buffer")]
struct Args {
    #[arg(long, default_value = "/etc/netcore/media-switch.toml")]
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
                .unwrap_or_else(|_| "netcore_media_switch=info".into()),
        )
        .init();

    let args = Args::parse();
    let config_path = if args.no_config {
        None
    } else {
        Some(args.config.as_path())
    };
    let mut config = MediaSwitchConfig::load(config_path)?;
    config.apply_bind_override(args.bind)?;

    tracing::warn!(
        "Media Switch starts in OPEN LAB mode: no authentication, no tokens and no TLS"
    );

    let media = SharedMedia::new(config.clone());
    let (gateway_tx, gateway_rx) = mpsc::channel();
    let _gateway = gateway::spawn_gateway_worker(config.clone(), media.clone(), gateway_rx);
    let _call_control = call_control::spawn_call_control_worker(config.clone(), media.clone());
    let http = http::spawn_http_server(config, media, gateway_tx)?;
    http.join().map_err(|_| -> Box<dyn std::error::Error> {
        "HTTP server thread panicked".into()
    })?;
    Ok(())
}

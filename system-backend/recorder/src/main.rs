mod config;
mod http;
mod media_switch;
mod protocol;
mod state;
mod tar;

use std::path::PathBuf;

use clap::Parser;
use config::RecorderConfig;
use state::SharedRecorder;

#[derive(Debug, Parser)]
#[command(name = "netcore-recorder")]
#[command(about = "NetCore passive central TETRA media recorder and retention service")]
struct Args {
    #[arg(long, default_value = "/etc/netcore/recorder.toml")]
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
                .unwrap_or_else(|_| "netcore_recorder=info".into()),
        )
        .init();

    let args = Args::parse();
    let config_path = if args.no_config {
        None
    } else {
        Some(args.config.as_path())
    };
    let mut config = RecorderConfig::load(config_path)?;
    config.apply_bind_override(args.bind)?;

    tracing::warn!(
        "Recorder starts in OPEN LAB mode: no authentication, no tokens and no TLS"
    );

    let recorder = SharedRecorder::load(config.clone())?;
    let _media_worker =
        media_switch::spawn_media_switch_worker(config.clone(), recorder.clone());
    let _maintenance = state::spawn_maintenance_worker(config.clone(), recorder.clone());
    let http = http::spawn_http_server(config, recorder)?;
    http.join().map_err(|_| -> Box<dyn std::error::Error> {
        "HTTP server thread panicked".into()
    })?;
    Ok(())
}

mod config;
mod http;
mod media;
mod model;
mod state;
mod worker;

use std::path::PathBuf;

use clap::Parser;
use config::MediaLibraryConfig;
use state::SharedLibrary;

#[derive(Debug, Parser)]
#[command(name = "netcore-media-library")]
#[command(about = "NetCore-Tetra media asset, preview, preparation and controlled playout service")]
struct Args {
    #[arg(long, default_value = "/etc/netcore/media-library.toml")]
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
                .unwrap_or_else(|_| "netcore_media_library=info".into()),
        )
        .init();

    let args = Args::parse();
    let config_path = if args.no_config { None } else { Some(args.config.as_path()) };
    let mut config = MediaLibraryConfig::load(config_path)?;
    config.apply_bind_override(args.bind).map_err(std::io::Error::other)?;

    tracing::warn!("Media Library starts in OPEN LAB management mode: no login, no tokens and no TLS");
    tracing::warn!("Every reachable management client may upload, approve, dispatch, archive or delete media according to configuration");
    tracing::info!(
        "Media Library WebUI/API bind={} mode={} worker={}ms",
        config.server.bind,
        config.runtime.operating_mode,
        config.runtime.worker_interval_ms
    );

    let library = SharedLibrary::load(config.clone())?;
    let _worker = worker::spawn_worker(config.clone(), library.clone());
    let server = http::spawn_http_server(config, library)?;
    server.join().map_err(|_| -> Box<dyn std::error::Error> {
        "Media Library HTTP server thread panicked".into()
    })?;
    Ok(())
}

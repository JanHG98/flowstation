mod config;
mod http;
mod protocol;
mod state;
mod transport;

use std::path::PathBuf;

use clap::Parser;
use config::TransitConfig;
use state::SharedTransit;

#[derive(Debug, Parser)]
#[command(name = "netcore-transit")]
#[command(about = "NetCore inter-region call, SDS, media and mobility transit service")]
struct Args {
    #[arg(long, default_value = "/etc/netcore/transit.toml")]
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
                .unwrap_or_else(|_| "netcore_transit=info".into()),
        )
        .init();

    let args = Args::parse();
    let config_path = if args.no_config {
        None
    } else {
        Some(args.config.as_path())
    };
    let mut config = TransitConfig::load(config_path)?;
    config
        .apply_bind_override(args.bind)
        .map_err(std::io::Error::other)?;

    tracing::warn!(
        "Transit management and peer transport start in OPEN LAB mode: no login, no tokens and no TLS"
    );
    tracing::warn!(
        "The regional transport protocol is NetCore-native and not yet standardized ETSI ISI"
    );
    tracing::info!(
        "Transit region={} swmi={} operating_mode={}",
        config.region.region_id,
        config.region.swmi_id,
        config.region.operating_mode
    );

    let transit = SharedTransit::load(config.clone())?;
    let _transport = transport::spawn_transport_worker(config.clone(), transit.clone());
    let http = http::spawn_http_server(config, transit)?;
    http.join().map_err(|_| -> Box<dyn std::error::Error> {
        "Transit HTTP server thread panicked".into()
    })?;
    Ok(())
}

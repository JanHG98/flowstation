mod config;
mod gateway;
mod http;
mod protocol;
mod state;

use std::path::PathBuf;
use std::sync::mpsc;

use clap::Parser;
use config::PacketCoreConfig;
use state::SharedPacketCore;

#[derive(Debug, Parser)]
#[command(name = "netcore-packet-core")]
#[command(about = "NetCore central SNDCP context, packet-flow and mobility-anchor service")]
struct Args {
    #[arg(long, default_value = "/etc/netcore/packet-core.toml")]
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
                .unwrap_or_else(|_| "netcore_packet_core=info".into()),
        )
        .init();

    let args = Args::parse();
    let config_path = if args.no_config {
        None
    } else {
        Some(args.config.as_path())
    };
    let mut config = PacketCoreConfig::load(config_path)?;
    config.apply_bind_override(args.bind)?;

    tracing::warn!(
        "Packet Core starts in OPEN LAB mode: no authentication, no tokens and no TLS"
    );
    tracing::info!("Packet Core operating mode: {}", config.packet.mode);

    let core = SharedPacketCore::load(config.clone())?;
    let (gateway_tx, gateway_rx) = mpsc::channel();
    let _gateway = gateway::spawn_gateway_worker(config.clone(), core.clone(), gateway_rx);
    let http = http::spawn_http_server(config, core, gateway_tx)?;
    http.join().map_err(|_| -> Box<dyn std::error::Error> {
        "HTTP server thread panicked".into()
    })?;
    Ok(())
}

mod config;
mod dataplane;
mod dns;
mod http;
mod kernel;
mod packet_core;
mod protocol;
mod runtime;
mod state;
mod tun;

use std::path::PathBuf;

use clap::Parser;
use config::IpGatewayConfig;
use state::SharedGateway;

#[derive(Debug, Parser)]
#[command(name = "netcore-ip-gateway")]
#[command(about = "NetCore TETRA packet-data TUN, routing, NAT, DNS and diagnostics gateway")]
struct Args {
    #[arg(long, default_value = "/etc/netcore/ip-gateway.toml")]
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
                .unwrap_or_else(|_| "netcore_ip_gateway=info".into()),
        )
        .init();

    let args = Args::parse();
    let config_path = if args.no_config {
        None
    } else {
        Some(args.config.as_path())
    };
    let mut config = IpGatewayConfig::load(config_path)?;
    config.apply_bind_override(args.bind)?;

    tracing::warn!(
        "IP Gateway starts in OPEN LAB mode: no authentication, no tokens and no TLS"
    );
    tracing::info!("IP Gateway operating mode: {}", config.interface.mode);

    let gateway = SharedGateway::load(config.clone())?;
    let runtime = runtime::spawn_runtime(config.clone(), gateway.clone());
    let _dns = dns::spawn_dns(config.clone(), gateway.clone());
    dataplane::spawn_test_services(config.clone(), gateway.clone());
    let http = http::spawn_http_server(config, gateway, runtime)?;
    http.join().map_err(|_| -> Box<dyn std::error::Error> {
        "HTTP server thread panicked".into()
    })?;
    Ok(())
}

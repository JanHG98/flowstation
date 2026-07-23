mod config;
mod gateway;
mod http;
mod protocol;
mod state;

use std::path::PathBuf;
use std::sync::mpsc;

use clap::Parser;
use config::GroupCoreConfig;
use state::SharedGroups;

#[derive(Debug, Parser)]
#[command(name = "netcore-group-core")]
#[command(about = "NetCore central GSSI, membership, affiliation and DGNA service")]
struct Args {
    #[arg(long, default_value = "/etc/netcore/group-core.toml")]
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
                .unwrap_or_else(|_| "netcore_group_core=info".into()),
        )
        .init();

    let args = Args::parse();
    let config_path = if args.no_config { None } else { Some(args.config.as_path()) };
    let mut config = GroupCoreConfig::load(config_path)?;
    config.apply_bind_override(args.bind)?;

    tracing::warn!(
        "Group Core starts in OPEN LAB mode: no authentication, no tokens and no TLS"
    );

    let groups = SharedGroups::load(config.clone())?;
    let (gateway_tx, gateway_rx) = mpsc::channel();
    let _gateway = gateway::spawn_gateway_worker(config.clone(), groups.clone(), gateway_rx);
    let http = http::spawn_http_server(config, groups, gateway_tx)?;
    http.join().map_err(|_| -> Box<dyn std::error::Error> {
        "HTTP server thread panicked".into()
    })?;
    Ok(())
}

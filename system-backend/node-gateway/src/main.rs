mod config;
mod http;
mod server;
mod state;
mod ws;

use std::net::SocketAddr;
use std::path::PathBuf;

use clap::Parser;
use tracing_subscriber::EnvFilter;

use crate::config::NodeGatewayConfig;
use crate::server::NodeGatewayServer;
use crate::state::SharedGateway;

#[derive(Debug, Parser)]
#[command(name = "netcore-node-gateway")]
#[command(about = "NetCore-Tetra TBS node gateway with an open lab WebUI")]
struct Args {
    /// Optional TOML configuration file.
    #[arg(long)]
    config: Option<PathBuf>,

    /// Override the configured listener address.
    #[arg(long)]
    bind: Option<SocketAddr>,

    /// Print the effective configuration and exit.
    #[arg(long)]
    check_config: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with_target(false)
        .compact()
        .init();

    let mut config = NodeGatewayConfig::load(args.config.as_deref())?;
    config
        .apply_bind_override(args.bind)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;

    if args.check_config {
        println!("{}", toml::to_string_pretty(&config)?);
        return Ok(());
    }

    tracing::warn!(
        "OPEN LAB MODE ACTIVE: every client on the reachable network can view nodes and execute enabled management actions"
    );
    let gateway = SharedGateway::new(config.clone());
    NodeGatewayServer::new(config, gateway).run()?;
    Ok(())
}

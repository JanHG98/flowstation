mod http;
mod server;
mod state;
mod ws;

use std::net::SocketAddr;

use clap::Parser;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Clone, Parser)]
#[command(name = "netcore-control-room")]
#[command(about = "NetCore-Tetra Control-Room Core server for FlowStation nodes")]
struct Args {
    /// Address to bind. Keep 127.0.0.1 for local testing; use 0.0.0.0 behind a reverse proxy/VPN only.
    #[arg(long, default_value = "127.0.0.1:9010")]
    bind: SocketAddr,

    /// WebSocket path used by base-station nodes.
    #[arg(long, default_value = "/node")]
    node_path: String,

    /// WebSocket path used by future Leitstelle/UI clients.
    #[arg(long, default_value = "/ui")]
    ui_path: String,

    /// Number of recent event/audit entries retained in memory.
    #[arg(long, default_value_t = 500)]
    history_limit: usize,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with_target(false)
        .compact()
        .init();

    let state = state::SharedControlRoom::new(args.history_limit);
    let server = server::ControlRoomServer::new(args.bind, args.node_path, args.ui_path, state);
    server.run()?;
    Ok(())
}

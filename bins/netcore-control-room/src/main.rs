mod auth;
mod config;
mod http;
mod persistence;
mod server;
mod state;
mod ws;

use std::net::SocketAddr;
use std::path::PathBuf;

use clap::Parser;
use tracing_subscriber::EnvFilter;

use crate::auth::AuthState;
use crate::config::ControlRoomConfig;
use crate::persistence::PersistenceHandle;

#[derive(Debug, Clone, Parser)]
#[command(name = "netcore-control-room")]
#[command(about = "NetCore-Tetra Control-Room Core server for FlowStation nodes")]
struct Args {
    /// Optional TOML config file. CLI flags override values from this file.
    #[arg(long)]
    config: Option<PathBuf>,

    /// Address to bind. Keep 127.0.0.1 for local testing; use 0.0.0.0 in the LXC/VPN/VLAN.
    #[arg(long)]
    bind: Option<SocketAddr>,

    /// WebSocket path used by base-station nodes.
    #[arg(long)]
    node_path: Option<String>,

    /// WebSocket path used by future Leitstelle/operator clients.
    #[arg(long)]
    ui_path: Option<String>,

    /// Number of recent event/audit entries retained in memory.
    #[arg(long)]
    history_limit: Option<usize>,

    /// Enable SQLite persistence at this database path, regardless of config file.
    #[arg(long)]
    database: Option<PathBuf>,

    /// Force-enable API/WebSocket token auth, regardless of config file.
    #[arg(long)]
    auth_enabled: bool,

    /// Force-disable API/WebSocket token auth, regardless of config file.
    #[arg(long)]
    no_auth: bool,

    /// Node token for BS -> Control Room WebSocket authentication. Prefer env/config in production.
    #[arg(long)]
    node_token: Option<String>,

    /// Operator/API token for HTTP and operator clients. Prefer env/config in production.
    #[arg(long)]
    operator_token: Option<String>,

    /// Force-disable SQLite persistence, regardless of config file.
    #[arg(long)]
    no_persistence: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with_target(false)
        .compact()
        .init();

    let mut config = ControlRoomConfig::load(args.config.as_deref())?;
    config.apply_cli_overrides(
        args.bind,
        args.node_path,
        args.ui_path,
        args.history_limit,
        args.database,
        args.no_persistence,
        args.auth_enabled,
        args.no_auth,
        args.node_token,
        args.operator_token,
    );

    let persistence = if config.persistence.enabled {
        let handle = PersistenceHandle::open(&config.persistence)?;
        tracing::info!(database = %config.persistence.database_path.display(), "SQLite persistence enabled");
        Some(handle)
    } else {
        tracing::info!("SQLite persistence disabled");
        None
    };

    let auth = AuthState::from_config(&config.auth)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    if auth.enabled() {
        tracing::info!(health_public = auth.allow_health_unauthenticated(), "Control Room token authentication enabled");
    } else {
        tracing::warn!("Control Room token authentication disabled");
    }

    let state = state::SharedControlRoom::new_with_persistence(config.server.history_limit, persistence);
    let server = server::ControlRoomServer::new(config.server.bind, config.server.node_path, config.server.ui_path, state, auth);
    server.run()?;
    Ok(())
}

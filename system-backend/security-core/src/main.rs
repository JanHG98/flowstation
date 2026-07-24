mod config;
mod crypto;
mod gateway;
mod http;
mod protocol;
mod state;

use std::path::PathBuf;

use clap::Parser;
use config::SecurityCoreConfig;
use state::SharedSecurityCore;

#[derive(Debug, Parser)]
#[command(name = "netcore-security-core")]
#[command(about = "NetCore central authentication, security-policy and DCK context service")]
struct Args {
    #[arg(long, default_value = "/etc/netcore/security-core.toml")]
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
                .unwrap_or_else(|_| "netcore_security_core=info".into()),
        )
        .init();

    let args = Args::parse();
    let config_path = if args.no_config {
        None
    } else {
        Some(args.config.as_path())
    };
    let mut config = SecurityCoreConfig::load(config_path)?;
    config
        .apply_bind_override(args.bind)
        .map_err(std::io::Error::other)?;

    tracing::warn!(
        "Security Core management starts in OPEN LAB mode: no login, no tokens and no TLS"
    );
    tracing::warn!(
        "The built-in lab_hmac_sha256 provider is a deterministic integration-test provider, not a TETRA TA algorithm or production KMF"
    );
    tracing::info!(
        "Security Core operating mode: {}",
        config.policy.operating_mode
    );

    let core = SharedSecurityCore::load(config.clone())?;
    let _gateway = gateway::spawn_gateway_worker(config.clone(), core.clone());
    let http = http::spawn_http_server(config, core)?;
    http.join().map_err(|_| -> Box<dyn std::error::Error> {
        "Security Core HTTP server thread panicked".into()
    })?;
    Ok(())
}

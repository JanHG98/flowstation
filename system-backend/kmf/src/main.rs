mod config;
mod crypto;
mod http;
mod protocol;
mod state;

use std::path::PathBuf;

use clap::Parser;
use config::KmfConfig;
use state::SharedKmf;

#[derive(Debug, Parser)]
#[command(name = "netcore-kmf")]
#[command(about = "NetCore TETRA key lifecycle, crypto-period and OTAR orchestration service")]
struct Args {
    #[arg(long, default_value = "/etc/netcore/kmf.toml")]
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
                .unwrap_or_else(|_| "netcore_kmf=info".into()),
        )
        .init();

    let args = Args::parse();
    let config_path = if args.no_config {
        None
    } else {
        Some(args.config.as_path())
    };
    let mut config = KmfConfig::load(config_path)?;
    config
        .apply_bind_override(args.bind)
        .map_err(std::io::Error::other)?;

    tracing::warn!("KMF management starts in OPEN LAB mode: no login, no tokens and no TLS");
    tracing::warn!("Raw key material is never returned by the management API or WebUI");
    tracing::warn!("lab_file_vault and lab SHA-256 envelopes are integration mechanisms, not production HSM cryptography or TETRA OTAR air-interface encoding");
    tracing::info!("KMF operating mode: {}", config.policy.operating_mode);

    let kmf = SharedKmf::load(config.clone())?;
    let http = http::spawn_http_server(config, kmf)?;
    http.join().map_err(|_| -> Box<dyn std::error::Error> {
        "KMF HTTP server thread panicked".into()
    })?;
    Ok(())
}

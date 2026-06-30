use std::collections::HashMap;

use serde::Deserialize;
use toml::Value;

/// NetCore Control-Room node endpoint configuration.
///
/// This is the future Leitstelle/Core connection: one bidirectional WebSocket
/// for hello/heartbeat, telemetry and operator commands.
#[derive(Debug, Clone)]
pub struct CfgControlRoom {
    /// Master switch.  A present section defaults to enabled=true.
    pub enabled: bool,
    /// Control-Room Core hostname or IP.
    pub host: String,
    /// Control-Room Core port.
    pub port: u16,
    /// Use TLS (wss://).
    pub use_tls: bool,
    /// HTTP path for the node WebSocket endpoint.  Default: "/node".
    pub endpoint_path: String,
    /// Optional path to a DER-encoded CA certificate for self-signed TLS.
    pub ca_cert: Option<String>,
    /// Optional HTTP Basic Auth credentials.
    pub credentials: Option<(String, String)>,
    /// Stable node id.  When unset, the BS derives one from MCC/MNC/LA/CC/carrier.
    pub node_id: Option<String>,
    /// Human readable station name shown in the Leitstelle.
    pub station_name: Option<String>,
    /// Optional site/location label, e.g. "Hannover Rack" or "xGEAR Event".
    pub site: Option<String>,
}

#[derive(Default, Deserialize)]
pub struct CfgControlRoomDto {
    #[serde(default)]
    pub enabled: Option<bool>,
    pub host: Option<String>,
    pub port: Option<u16>,
    #[serde(default)]
    pub use_tls: bool,
    pub endpoint_path: Option<String>,
    pub ca_cert: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub node_id: Option<String>,
    pub station_name: Option<String>,
    pub site: Option<String>,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

pub fn apply_control_room_patch(src: CfgControlRoomDto) -> Result<CfgControlRoom, String> {
    let enabled = src.enabled.unwrap_or(true);

    if src.ca_cert.is_some() && !src.use_tls {
        return Err("control_room: ca_cert requires use_tls = true".to_string());
    }

    let credentials = match (src.username, src.password) {
        (Some(u), Some(p)) => Some((u, p)),
        (None, None) => None,
        _ => return Err("control_room: both username and password must be set for credentials".to_string()),
    };

    let host = match (enabled, src.host) {
        (true, Some(host)) if !host.trim().is_empty() => host,
        (true, _) => return Err("control_room: host is required when enabled".to_string()),
        (false, Some(host)) => host,
        (false, None) => String::new(),
    };

    let port = match (enabled, src.port) {
        (true, Some(port)) if port > 0 => port,
        (true, _) => return Err("control_room: port is required and must be > 0 when enabled".to_string()),
        (false, Some(port)) => port,
        (false, None) => 0,
    };

    let endpoint_path = src.endpoint_path.unwrap_or_else(|| "/node".to_string());
    if !endpoint_path.starts_with('/') {
        return Err("control_room: endpoint_path must start with '/'".to_string());
    }

    Ok(CfgControlRoom {
        enabled,
        host,
        port,
        use_tls: src.use_tls,
        endpoint_path,
        ca_cert: src.ca_cert,
        credentials,
        node_id: src.node_id,
        station_name: src.station_name,
        site: src.site,
    })
}

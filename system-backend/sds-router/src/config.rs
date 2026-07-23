use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const OPEN_LAB_MODE: &str = "open_lab";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SdsRouterConfig {
    pub server: ServerConfig,
    pub node_gateway: NodeGatewayConfig,
    pub storage: StorageConfig,
    pub routing: RoutingConfig,
    pub security: SecurityConfig,
    pub limits: LimitsConfig,
}

impl Default for SdsRouterConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            node_gateway: NodeGatewayConfig::default(),
            storage: StorageConfig::default(),
            routing: RoutingConfig::default(),
            security: SecurityConfig::default(),
            limits: LimitsConfig::default(),
        }
    }
}

impl SdsRouterConfig {
    pub fn load(path: Option<&Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let mut config = match path {
            Some(path) => toml::from_str::<Self>(&fs::read_to_string(path)?)?,
            None => Self::default(),
        };
        config.normalise().map_err(|error| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, error)
        })?;
        Ok(config)
    }

    pub fn apply_bind_override(&mut self, bind: Option<SocketAddr>) -> Result<(), String> {
        if let Some(bind) = bind {
            self.server.bind = bind;
        }
        self.normalise()
    }

    fn normalise(&mut self) -> Result<(), String> {
        if self.security.mode != OPEN_LAB_MODE {
            return Err(format!(
                "unsupported security.mode={}; this package intentionally implements only open_lab",
                self.security.mode
            ));
        }
        if !self.node_gateway.url.starts_with("ws://") {
            return Err("node_gateway.url must use ws:// in the open lab package".to_string());
        }
        if !self.security.allow_remote_management && !self.server.bind.ip().is_loopback() {
            return Err(
                "server.bind must use a loopback address when allow_remote_management=false"
                    .to_string(),
            );
        }
        self.server.history_limit = self.server.history_limit.max(100);
        self.node_gateway.reconnect_secs = self.node_gateway.reconnect_secs.max(1);
        self.routing.default_ttl_secs = self.routing.default_ttl_secs.max(5);
        self.routing.max_ttl_secs = self
            .routing
            .max_ttl_secs
            .max(self.routing.default_ttl_secs);
        self.routing.max_attempts = self.routing.max_attempts.max(1);
        self.routing.initial_retry_secs = self.routing.initial_retry_secs.max(1);
        self.routing.max_retry_secs = self
            .routing
            .max_retry_secs
            .max(self.routing.initial_retry_secs);
        self.routing.dedupe_window_secs = self.routing.dedupe_window_secs.max(1);
        self.routing.presence_timeout_secs = self.routing.presence_timeout_secs.max(10);
        self.limits.max_body_bytes = self.limits.max_body_bytes.max(1_024);
        self.limits.max_payload_bytes = self.limits.max_payload_bytes.clamp(16, 8_192);
        self.limits.max_messages = self.limits.max_messages.max(100);
        self.limits.max_routes = self.limits.max_routes.max(1);
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub bind: SocketAddr,
    pub history_limit: usize,
}
impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: "0.0.0.0:8150".parse().expect("valid default bind"),
            history_limit: 4_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NodeGatewayConfig {
    pub url: String,
    pub reconnect_secs: u64,
}
impl Default for NodeGatewayConfig {
    fn default() -> Self {
        Self {
            url: "ws://127.0.0.1:8080/ws/backend".to_string(),
            reconnect_secs: 5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StorageConfig {
    pub database_path: PathBuf,
    pub backup_path: PathBuf,
}
impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            database_path: "/var/lib/netcore-sds-router/messages.json".into(),
            backup_path: "/var/lib/netcore-sds-router/messages.json.bak".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RoutingConfig {
    pub default_ttl_secs: u64,
    pub max_ttl_secs: u64,
    pub max_attempts: u32,
    pub initial_retry_secs: u64,
    pub max_retry_secs: u64,
    pub dedupe_window_secs: u64,
    pub presence_timeout_secs: u64,
    pub authoritative_ingress: bool,
}
impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            default_ttl_secs: 300,
            max_ttl_secs: 86_400,
            max_attempts: 5,
            initial_retry_secs: 2,
            max_retry_secs: 60,
            dedupe_window_secs: 30,
            presence_timeout_secs: 90,
            authoritative_ingress: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SecurityConfig {
    pub mode: String,
    pub allow_remote_management: bool,
    pub mask_payload_in_list: bool,
}
impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            mode: OPEN_LAB_MODE.to_string(),
            allow_remote_management: true,
            mask_payload_in_list: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LimitsConfig {
    pub max_body_bytes: usize,
    pub max_payload_bytes: usize,
    pub max_messages: usize,
    pub max_routes: usize,
}
impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            max_body_bytes: 2_097_152,
            max_payload_bytes: 2_048,
            max_messages: 100_000,
            max_routes: 4_096,
        }
    }
}

use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const OPEN_LAB_MODE: &str = "open_lab";
pub const POLICY_ALLOW_LIST: &str = "allow_list";
pub const POLICY_OPEN_NETWORK: &str = "open_network";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SubscriberCoreConfig {
    pub server: ServerConfig,
    pub node_gateway: NodeGatewayConfig,
    pub storage: StorageConfig,
    pub access_policy: AccessPolicyConfig,
    pub security: SecurityConfig,
    pub limits: LimitsConfig,
}

impl Default for SubscriberCoreConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            node_gateway: NodeGatewayConfig::default(),
            storage: StorageConfig::default(),
            access_policy: AccessPolicyConfig::default(),
            security: SecurityConfig::default(),
            limits: LimitsConfig::default(),
        }
    }
}

impl SubscriberCoreConfig {
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
                "unsupported security.mode={}; this lab package intentionally implements only open_lab",
                self.security.mode
            ));
        }
        if !self.node_gateway.url.starts_with("ws://") {
            return Err("node_gateway.url must use ws:// in the open lab package".to_string());
        }
        if !matches!(self.access_policy.mode.as_str(), POLICY_ALLOW_LIST | POLICY_OPEN_NETWORK) {
            return Err(format!(
                "unsupported access_policy.mode={}; expected allow_list or open_network",
                self.access_policy.mode
            ));
        }
        if self.server.history_limit == 0 {
            self.server.history_limit = 2_000;
        }
        if self.node_gateway.reconnect_secs == 0 {
            self.node_gateway.reconnect_secs = 1;
        }
        if self.access_policy.sync_timeout_secs < 5 {
            self.access_policy.sync_timeout_secs = 5;
        }
        if self.limits.max_body_bytes < 1_024 {
            self.limits.max_body_bytes = 1_024;
        }
        if self.limits.max_subscribers == 0 {
            self.limits.max_subscribers = 100_000;
        }
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
            bind: "0.0.0.0:8100".parse().expect("valid default bind"),
            history_limit: 2_000,
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
            database_path: "/var/lib/netcore-subscriber-core/subscribers.json".into(),
            backup_path: "/var/lib/netcore-subscriber-core/subscribers.json.bak".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AccessPolicyConfig {
    pub mode: String,
    pub auto_sync: bool,
    pub disconnect_unauthorized: bool,
    pub sync_timeout_secs: u64,
}
impl Default for AccessPolicyConfig {
    fn default() -> Self {
        Self {
            mode: POLICY_ALLOW_LIST.to_string(),
            auto_sync: true,
            disconnect_unauthorized: true,
            sync_timeout_secs: 30,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SecurityConfig {
    pub mode: String,
    pub allow_remote_management: bool,
}
impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            mode: OPEN_LAB_MODE.to_string(),
            allow_remote_management: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LimitsConfig {
    pub max_body_bytes: usize,
    pub max_subscribers: usize,
    pub max_groups_per_subscriber: usize,
}
impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            max_body_bytes: 2_097_152,
            max_subscribers: 100_000,
            max_groups_per_subscriber: 1_024,
        }
    }
}

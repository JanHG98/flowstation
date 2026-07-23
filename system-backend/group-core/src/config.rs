use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const OPEN_LAB_MODE: &str = "open_lab";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GroupCoreConfig {
    pub server: ServerConfig,
    pub node_gateway: NodeGatewayConfig,
    pub storage: StorageConfig,
    pub policy: PolicyConfig,
    pub security: SecurityConfig,
    pub limits: LimitsConfig,
}

impl Default for GroupCoreConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            node_gateway: NodeGatewayConfig::default(),
            storage: StorageConfig::default(),
            policy: PolicyConfig::default(),
            security: SecurityConfig::default(),
            limits: LimitsConfig::default(),
        }
    }
}

impl GroupCoreConfig {
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
        if !self.security.allow_remote_management && !self.server.bind.ip().is_loopback() {
            return Err(
                "server.bind must use a loopback address when allow_remote_management=false"
                    .to_string(),
            );
        }
        self.server.history_limit = self.server.history_limit.max(100);
        self.node_gateway.reconnect_secs = self.node_gateway.reconnect_secs.max(1);
        self.policy.sync_timeout_secs = self.policy.sync_timeout_secs.max(5);
        self.policy.dgna_timeout_secs = self.policy.dgna_timeout_secs.max(5);
        self.limits.max_body_bytes = self.limits.max_body_bytes.max(1_024);
        self.limits.max_groups = self.limits.max_groups.max(1);
        self.limits.max_memberships = self.limits.max_memberships.max(1);
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
            bind: "0.0.0.0:8110".parse().expect("valid default bind"),
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
            database_path: "/var/lib/netcore-group-core/groups.json".into(),
            backup_path: "/var/lib/netcore-group-core/groups.json.bak".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PolicyConfig {
    pub allow_unlisted_groups: bool,
    pub enforce_memberships: bool,
    pub reconcile_registered: bool,
    pub auto_sync: bool,
    pub sync_timeout_secs: u64,
    pub dgna_timeout_secs: u64,
}
impl Default for PolicyConfig {
    fn default() -> Self {
        Self {
            allow_unlisted_groups: false,
            enforce_memberships: true,
            reconcile_registered: true,
            auto_sync: true,
            sync_timeout_secs: 30,
            dgna_timeout_secs: 30,
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
    pub max_groups: usize,
    pub max_memberships: usize,
}
impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            max_body_bytes: 2_097_152,
            max_groups: 65_536,
            max_memberships: 1_000_000,
        }
    }
}

use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const OPEN_LAB_MODE: &str = "open_lab";
pub const OPERATING_MODE_SHADOW: &str = "shadow";
pub const OPERATING_MODE_AUTHORITATIVE: &str = "authoritative";
pub const LAB_PROVIDER_HMAC_SHA256: &str = "lab_hmac_sha256";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SecurityCoreConfig {
    pub server: ServerConfig,
    pub node_gateway: NodeGatewayConfig,
    pub storage: StorageConfig,
    pub policy: PolicyConfig,
    pub authentication: AuthenticationConfig,
    pub dck: DckConfig,
    pub security: ManagementSecurityConfig,
    pub limits: LimitsConfig,
}

impl Default for SecurityCoreConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            node_gateway: NodeGatewayConfig::default(),
            storage: StorageConfig::default(),
            policy: PolicyConfig::default(),
            authentication: AuthenticationConfig::default(),
            dck: DckConfig::default(),
            security: ManagementSecurityConfig::default(),
            limits: LimitsConfig::default(),
        }
    }
}

impl SecurityCoreConfig {
    pub fn load(path: Option<&Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let mut config = match path {
            Some(path) => toml::from_str::<Self>(&fs::read_to_string(path)?)?,
            None => Self::default(),
        };
        config
            .normalise()
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;
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
                "unsupported security.mode={}; this package intentionally implements only open_lab management access",
                self.security.mode
            ));
        }
        if !matches!(
            self.policy.operating_mode.as_str(),
            OPERATING_MODE_SHADOW | OPERATING_MODE_AUTHORITATIVE
        ) {
            return Err("policy.operating_mode must be shadow or authoritative".to_string());
        }
        if !self.node_gateway.url.starts_with("ws://") {
            return Err("node_gateway.url must use ws:// in the open-lab package".to_string());
        }
        if !self.security.allow_remote_management && !self.server.bind.ip().is_loopback() {
            return Err(
                "server.bind must be loopback when security.allow_remote_management=false"
                    .to_string(),
            );
        }
        if self.policy.default_security_class == 0 || self.policy.default_security_class > 3 {
            return Err("policy.default_security_class must be 1, 2 or 3".to_string());
        }
        if self.policy.minimum_security_class == 0 || self.policy.minimum_security_class > 3 {
            return Err("policy.minimum_security_class must be 1, 2 or 3".to_string());
        }
        if self.policy.minimum_security_class > self.policy.default_security_class {
            return Err(
                "policy.minimum_security_class may not exceed default_security_class".to_string(),
            );
        }
        if self.authentication.provider != LAB_PROVIDER_HMAC_SHA256 {
            return Err(format!(
                "unsupported authentication.provider={}; KMF-backed providers are introduced by the following KMF package",
                self.authentication.provider
            ));
        }
        if self.authentication.response_bytes < 8 || self.authentication.response_bytes > 32 {
            return Err("authentication.response_bytes must be between 8 and 32".to_string());
        }
        if self.dck.key_bytes < 8 || self.dck.key_bytes > 32 {
            return Err("dck.key_bytes must be between 8 and 32".to_string());
        }
        self.server.history_limit = self.server.history_limit.max(100);
        self.node_gateway.reconnect_secs = self.node_gateway.reconnect_secs.max(1);
        self.authentication.challenge_bytes = self.authentication.challenge_bytes.clamp(8, 64);
        self.authentication.challenge_ttl_secs = self.authentication.challenge_ttl_secs.max(5);
        self.authentication.max_attempts = self.authentication.max_attempts.max(1);
        self.authentication.lockout_secs = self.authentication.lockout_secs.max(1);
        self.dck.ttl_secs = self.dck.ttl_secs.max(30);
        self.dck.rotate_before_secs = self.dck.rotate_before_secs.min(self.dck.ttl_secs / 2);
        self.dck.max_active_per_subscriber = self.dck.max_active_per_subscriber.max(1);
        self.limits.max_body_bytes = self.limits.max_body_bytes.max(4_096);
        self.limits.max_profiles = self.limits.max_profiles.max(1);
        self.limits.max_contexts = self.limits.max_contexts.max(32);
        self.limits.max_actions = self.limits.max_actions.max(32);
        self.limits.max_alarms = self.limits.max_alarms.max(32);
        self.limits.max_audit = self.limits.max_audit.max(100);
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
            bind: "0.0.0.0:8180".parse().expect("valid default bind"),
            history_limit: 5_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NodeGatewayConfig {
    pub url: String,
    pub reconnect_secs: u64,
    pub observe_nodes: bool,
}
impl Default for NodeGatewayConfig {
    fn default() -> Self {
        Self {
            url: "ws://127.0.0.1:8080/ws/backend".to_string(),
            reconnect_secs: 5,
            observe_nodes: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StorageConfig {
    pub database_path: PathBuf,
    pub backup_path: PathBuf,
    pub lab_seed_path: PathBuf,
}
impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            database_path: "/var/lib/netcore-security-core/state.json".into(),
            backup_path: "/var/lib/netcore-security-core/state.json.bak".into(),
            lab_seed_path: "/var/lib/netcore-security-core/lab-auth.seed".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PolicyConfig {
    pub operating_mode: String,
    pub default_security_class: u8,
    pub minimum_security_class: u8,
    pub authentication_required: bool,
    pub allow_class1_fallback: bool,
    pub reject_unknown_subscribers: bool,
    pub disable_after_failures: bool,
}
impl Default for PolicyConfig {
    fn default() -> Self {
        Self {
            operating_mode: OPERATING_MODE_SHADOW.to_string(),
            default_security_class: 1,
            minimum_security_class: 1,
            authentication_required: true,
            allow_class1_fallback: true,
            reject_unknown_subscribers: false,
            disable_after_failures: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AuthenticationConfig {
    pub provider: String,
    pub challenge_bytes: usize,
    pub response_bytes: usize,
    pub challenge_ttl_secs: u64,
    pub max_attempts: u32,
    pub lockout_secs: u64,
    pub issue_dck_on_success: bool,
}
impl Default for AuthenticationConfig {
    fn default() -> Self {
        Self {
            provider: LAB_PROVIDER_HMAC_SHA256.to_string(),
            challenge_bytes: 16,
            response_bytes: 16,
            challenge_ttl_secs: 30,
            max_attempts: 3,
            lockout_secs: 300,
            issue_dck_on_success: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DckConfig {
    pub key_bytes: usize,
    pub ttl_secs: u64,
    pub rotate_before_secs: u64,
    pub max_active_per_subscriber: usize,
}
impl Default for DckConfig {
    fn default() -> Self {
        Self {
            key_bytes: 16,
            ttl_secs: 3_600,
            rotate_before_secs: 300,
            max_active_per_subscriber: 2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ManagementSecurityConfig {
    pub mode: String,
    pub allow_remote_management: bool,
    pub expose_ephemeral_edge_material: bool,
}
impl Default for ManagementSecurityConfig {
    fn default() -> Self {
        Self {
            mode: OPEN_LAB_MODE.to_string(),
            allow_remote_management: true,
            expose_ephemeral_edge_material: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LimitsConfig {
    pub max_body_bytes: usize,
    pub max_profiles: usize,
    pub max_contexts: usize,
    pub max_actions: usize,
    pub max_alarms: usize,
    pub max_audit: usize,
}
impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            max_body_bytes: 1_048_576,
            max_profiles: 100_000,
            max_contexts: 20_000,
            max_actions: 20_000,
            max_alarms: 20_000,
            max_audit: 100_000,
        }
    }
}

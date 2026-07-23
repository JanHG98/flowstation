use std::fs;
use std::net::SocketAddr;
use std::path::Path;

use serde::{Deserialize, Serialize};

pub const OPEN_LAB_MODE: &str = "open_lab";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NodeGatewayConfig {
    pub server: ServerConfig,
    pub security: SecurityConfig,
    pub limits: LimitsConfig,
}

impl Default for NodeGatewayConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            security: SecurityConfig::default(),
            limits: LimitsConfig::default(),
        }
    }
}

impl NodeGatewayConfig {
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
        self.server.node_path = normalise_path(&self.server.node_path);
        self.server.backend_path = normalise_path(&self.server.backend_path);
        if self.server.node_path == self.server.backend_path {
            return Err("server.node_path and server.backend_path must differ".to_string());
        }
        if self.server.history_limit == 0 {
            self.server.history_limit = 1_000;
        }
        if self.server.stale_after_secs < 10 {
            self.server.stale_after_secs = 10;
        }
        if self.server.hello_timeout_secs < 2 {
            self.server.hello_timeout_secs = 2;
        }
        if self.server.application_ping_secs < 5 {
            self.server.application_ping_secs = 5;
        }
        if self.limits.max_message_bytes < 4_096 {
            self.limits.max_message_bytes = 4_096;
        }
        if self.limits.max_http_body_bytes < 4_096 {
            self.limits.max_http_body_bytes = 4_096;
        }
        if self.security.mode.trim().to_ascii_lowercase() != OPEN_LAB_MODE {
            return Err(format!(
                "unsupported security.mode={:?}; this package intentionally implements only open_lab and does not pretend to provide token security",
                self.security.mode
            ));
        }
        self.security.mode = OPEN_LAB_MODE.to_string();
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub bind: SocketAddr,
    pub node_path: String,
    pub backend_path: String,
    pub history_limit: usize,
    pub stale_after_secs: u64,
    pub hello_timeout_secs: u64,
    pub application_ping_secs: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: "0.0.0.0:8080".parse().expect("static bind address is valid"),
            node_path: "/ws/node".to_string(),
            backend_path: "/ws/backend".to_string(),
            history_limit: 1_000,
            stale_after_secs: 20,
            hello_timeout_secs: 10,
            application_ping_secs: 15,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SecurityConfig {
    /// Deliberately only `open_lab` in this package. There are no tokens, users or certificates.
    pub mode: String,
    /// Allows write operations from the WebUI/API. Keep true only in an isolated test network.
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
    pub max_message_bytes: usize,
    pub max_http_body_bytes: usize,
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            max_message_bytes: 1_048_576,
            max_http_body_bytes: 1_048_576,
        }
    }
}

fn normalise_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_explicitly_open_lab_without_tokens() {
        let cfg = NodeGatewayConfig::default();
        assert_eq!(cfg.security.mode, OPEN_LAB_MODE);
        assert!(cfg.security.allow_remote_management);
        assert_eq!(cfg.server.bind.port(), 8080);
    }

    #[test]
    fn refuses_fake_secure_modes() {
        let mut cfg = NodeGatewayConfig::default();
        cfg.security.mode = "token".to_string();
        assert!(cfg.normalise().is_err());
    }
}

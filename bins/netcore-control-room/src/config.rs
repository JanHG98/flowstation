use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ControlRoomConfig {
    pub server: ServerConfig,
    pub persistence: PersistenceConfig,
    pub auth: AuthConfig,
    /// Central operator directory served to native UIs via /api/directory.
    /// Keep secrets out of this block; it is visible to every authenticated viewer.
    pub directory: Value,
}

impl Default for ControlRoomConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            persistence: PersistenceConfig::default(),
            auth: AuthConfig::default(),
            directory: json!({
                "subscribers": {},
                "groups": {},
                "status_groups": {},
                "statuses": {},
                "hide_infrastructure": true
            }),
        }
    }
}

impl ControlRoomConfig {
    pub fn load(path: Option<&Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let Some(path) = path else {
            return Ok(Self::default());
        };

        let raw = fs::read_to_string(path)?;
        let mut config: Self = toml::from_str(&raw)?;
        config.normalise();
        Ok(config)
    }

    pub fn apply_cli_overrides(
        &mut self,
        bind: Option<SocketAddr>,
        node_path: Option<String>,
        ui_path: Option<String>,
        history_limit: Option<usize>,
        database: Option<PathBuf>,
        no_persistence: bool,
        auth_enabled: bool,
        no_auth: bool,
        node_token: Option<String>,
        operator_token: Option<String>,
    ) {
        if let Some(bind) = bind {
            self.server.bind = bind;
        }
        if let Some(node_path) = node_path {
            self.server.node_path = node_path;
        }
        if let Some(ui_path) = ui_path {
            self.server.ui_path = ui_path;
        }
        if let Some(history_limit) = history_limit {
            self.server.history_limit = history_limit;
        }
        if let Some(database) = database {
            self.persistence.enabled = true;
            self.persistence.database_path = database;
        }
        if no_persistence {
            self.persistence.enabled = false;
        }
        if auth_enabled {
            self.auth.enabled = true;
        }
        if no_auth {
            self.auth.enabled = false;
        }
        if let Some(node_token) = node_token {
            self.auth.node_token = Some(node_token);
        }
        if let Some(operator_token) = operator_token {
            self.auth.operator_token = Some(operator_token);
        }
        self.normalise();
    }

    fn normalise(&mut self) {
        self.server.node_path = normalise_path(&self.server.node_path);
        self.server.ui_path = normalise_path(&self.server.ui_path);
        if self.server.history_limit == 0 {
            self.server.history_limit = 500;
        }
        if self.persistence.load_recent_limit == 0 {
            self.persistence.load_recent_limit = self.server.history_limit;
        }
        self.auth.normalise();
        if !self.directory.is_object() {
            self.directory = json!({
                "subscribers": {},
                "groups": {},
                "status_groups": {},
                "statuses": {},
                "hide_infrastructure": true
            });
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub bind: SocketAddr,
    pub node_path: String,
    pub ui_path: String,
    pub history_limit: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:9010".parse().expect("static default bind address is valid"),
            node_path: "/node".to_string(),
            ui_path: "/ui".to_string(),
            history_limit: 500,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AuthConfig {
    /// Master switch. Keep disabled until tokens are configured on both the LXC and the TBS/operator.
    pub enabled: bool,
    /// Keep /health public for service checks even when auth is enabled.
    pub allow_health_unauthenticated: bool,
    /// Node token for BS -> Control Room WebSocket authentication. Prefer node_token_env for production.
    pub node_token: Option<String>,
    /// Operator bootstrap token for HTTP API and operator clients. Prefer operator_token_env for production.
    pub operator_token: Option<String>,
    /// Role for the bootstrap operator token. Keep admin until registry tokens are created.
    pub operator_token_role: String,
    /// Environment variable containing the node token.
    pub node_token_env: Option<String>,
    /// Environment variable containing the operator token.
    pub operator_token_env: Option<String>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            allow_health_unauthenticated: true,
            node_token: None,
            operator_token: None,
            operator_token_role: "admin".to_string(),
            node_token_env: Some("NETCORE_CONTROL_ROOM_NODE_TOKEN".to_string()),
            operator_token_env: Some("NETCORE_CONTROL_ROOM_OPERATOR_TOKEN".to_string()),
        }
    }
}

impl AuthConfig {
    fn normalise(&mut self) {
        let role = self.operator_token_role.trim().to_ascii_lowercase();
        self.operator_token_role = match role.as_str() {
            "viewer" | "operator" | "admin" => role,
            "" => "admin".to_string(),
            _ => "admin".to_string(),
        };
        self.node_token = normalise_optional_secret(self.node_token.take());
        self.operator_token = normalise_optional_secret(self.operator_token.take());
        self.node_token_env = normalise_optional_secret(self.node_token_env.take());
        self.operator_token_env = normalise_optional_secret(self.operator_token_env.take());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PersistenceConfig {
    pub enabled: bool,
    pub database_path: PathBuf,
    pub persist_events: bool,
    pub persist_noisy_events: bool,
    pub load_recent_limit: usize,
}

impl Default for PersistenceConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            database_path: PathBuf::from("/var/lib/netcore-control-room/control-room.sqlite3"),
            persist_events: true,
            persist_noisy_events: false,
            load_recent_limit: 500,
        }
    }
}

fn normalise_path(path: &str) -> String {
    if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{}", path)
    }
}

fn normalise_optional_secret(value: Option<String>) -> Option<String> {
    value.and_then(|v| {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

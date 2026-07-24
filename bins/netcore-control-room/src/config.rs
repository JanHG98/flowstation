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
    pub federation: FederationConfig,
    pub operations: OperationsConfig,
    pub services: Vec<CoreServiceConfig>,
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
            federation: FederationConfig::default(),
            operations: OperationsConfig::default(),
            services: default_core_services(),
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
        bootstrap_username: Option<String>,
        bootstrap_password: Option<String>,
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
        if let Some(bootstrap_username) = bootstrap_username {
            self.auth.bootstrap_username = Some(bootstrap_username);
        }
        if let Some(bootstrap_password) = bootstrap_password {
            self.auth.bootstrap_password = Some(bootstrap_password);
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
        self.federation.normalise();
        self.operations.normalise();
        if self.services.is_empty() {
            self.services = default_core_services();
        }
        for service in &mut self.services {
            service.normalise();
        }
        self.services.sort_by(|left, right| left.name.cmp(&right.name));
        self.services.dedup_by(|left, right| left.name == right.name);
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
pub struct FederationConfig {
    pub enabled: bool,
    pub poll_interval_secs: u64,
    pub request_timeout_ms: u64,
    pub failure_threshold: u32,
    pub fetch_summaries: bool,
}

impl Default for FederationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            poll_interval_secs: 5,
            request_timeout_ms: 1200,
            failure_threshold: 3,
            fetch_summaries: true,
        }
    }
}

impl FederationConfig {
    fn normalise(&mut self) {
        self.poll_interval_secs = self.poll_interval_secs.max(1);
        self.request_timeout_ms = self.request_timeout_ms.max(100);
        self.failure_threshold = self.failure_threshold.max(1);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OperationsConfig {
    pub state_path: PathBuf,
    pub backup_path: PathBuf,
    pub auto_service_incidents: bool,
    pub incident_limit: usize,
    pub shift_log_limit: usize,
}

impl Default for OperationsConfig {
    fn default() -> Self {
        Self {
            state_path: PathBuf::from("/var/lib/netcore-control-room/operations.json"),
            backup_path: PathBuf::from("/var/lib/netcore-control-room/operations.json.bak"),
            auto_service_incidents: true,
            incident_limit: 5000,
            shift_log_limit: 10000,
        }
    }
}

impl OperationsConfig {
    fn normalise(&mut self) {
        self.incident_limit = self.incident_limit.max(100);
        self.shift_log_limit = self.shift_log_limit.max(100);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CoreServiceConfig {
    pub name: String,
    pub display_name: String,
    pub kind: String,
    pub base_url: String,
    pub health_live: String,
    pub health_ready: String,
    pub summary_path: String,
    pub webui_path: String,
    pub critical: bool,
    pub enabled: bool,
    pub timeout_ms: Option<u64>,
}

impl Default for CoreServiceConfig {
    fn default() -> Self {
        Self {
            name: "service".to_string(),
            display_name: "Service".to_string(),
            kind: "core".to_string(),
            base_url: "http://127.0.0.1:8080".to_string(),
            health_live: "/health/live".to_string(),
            health_ready: "/health/ready".to_string(),
            summary_path: "/api/v1/status".to_string(),
            webui_path: "/".to_string(),
            critical: false,
            enabled: true,
            timeout_ms: None,
        }
    }
}

impl CoreServiceConfig {
    fn normalise(&mut self) {
        self.name = self.name.trim().to_ascii_lowercase().replace(' ', "-");
        if self.name.is_empty() {
            self.name = "service".to_string();
        }
        if self.display_name.trim().is_empty() {
            self.display_name = self.name.clone();
        }
        self.base_url = self.base_url.trim().trim_end_matches('/').to_string();
        self.health_live = normalise_path(&self.health_live);
        self.health_ready = normalise_path(&self.health_ready);
        self.summary_path = normalise_path(&self.summary_path);
        self.webui_path = normalise_path(&self.webui_path);
        self.timeout_ms = self.timeout_ms.map(|value| value.max(100));
    }
}

fn service(name: &str, display_name: &str, kind: &str, port: u16, critical: bool) -> CoreServiceConfig {
    CoreServiceConfig {
        name: name.to_string(),
        display_name: display_name.to_string(),
        kind: kind.to_string(),
        base_url: format!("http://127.0.0.1:{port}"),
        critical,
        ..CoreServiceConfig::default()
    }
}

fn default_core_services() -> Vec<CoreServiceConfig> {
    vec![
        service("node-gateway", "Node Gateway", "edge", 8080, true),
        service("mobility-core", "Mobility Core", "core", 8090, true),
        service("subscriber-core", "Subscriber Core", "core", 8100, true),
        service("group-core", "Group Core", "core", 8110, true),
        service("call-control", "Call Control", "core", 8120, true),
        service("media-switch", "Media Switch", "media", 8130, true),
        service("recorder", "Recorder", "media", 8140, false),
        service("sds-router", "SDS Router", "data", 8150, false),
        service("packet-core", "Packet Core", "data", 8160, false),
        service("ip-gateway", "IP Gateway", "data", 8170, false),
        service("security-core", "Security Core", "security", 8180, true),
        service("kmf", "KMF", "security", 8190, false),
        service("transit", "Transit", "interworking", 8200, false),
    ]
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
    /// Master switch. Keep disabled only for first lab tests.
    pub enabled: bool,
    /// Keep /health public for service checks even when auth is enabled.
    pub allow_health_unauthenticated: bool,
    /// Node token for BS -> Control Room WebSocket authentication. This remains machine auth for the TBS.
    pub node_token: Option<String>,
    /// Environment variable containing the node token.
    pub node_token_env: Option<String>,
    /// Bootstrap admin username for initial recovery/login. Prefer bootstrap_username_env for production.
    pub bootstrap_username: Option<String>,
    /// Bootstrap admin password for initial recovery/login. Prefer bootstrap_password_env for production.
    pub bootstrap_password: Option<String>,
    /// Role for the bootstrap user. Keep admin unless you intentionally want a weaker recovery account.
    pub bootstrap_role: String,
    /// Environment variable containing the bootstrap username.
    pub bootstrap_username_env: Option<String>,
    /// Environment variable containing the bootstrap password.
    pub bootstrap_password_env: Option<String>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            allow_health_unauthenticated: true,
            node_token: None,
            node_token_env: Some("NETCORE_CONTROL_ROOM_NODE_TOKEN".to_string()),
            bootstrap_username: None,
            bootstrap_password: None,
            bootstrap_role: "admin".to_string(),
            bootstrap_username_env: Some("NETCORE_CONTROL_ROOM_BOOTSTRAP_USER".to_string()),
            bootstrap_password_env: Some("NETCORE_CONTROL_ROOM_BOOTSTRAP_PASSWORD".to_string()),
        }
    }
}

impl AuthConfig {
    fn normalise(&mut self) {
        let role = self.bootstrap_role.trim().to_ascii_lowercase();
        self.bootstrap_role = match role.as_str() {
            "viewer" | "operator" | "admin" => role,
            "" => "admin".to_string(),
            _ => "admin".to_string(),
        };
        self.node_token = normalise_optional_secret(self.node_token.take());
        self.node_token_env = normalise_optional_secret(self.node_token_env.take());
        self.bootstrap_username = normalise_optional_secret(self.bootstrap_username.take());
        self.bootstrap_password = normalise_optional_secret(self.bootstrap_password.take());
        self.bootstrap_username_env = normalise_optional_secret(self.bootstrap_username_env.take());
        self.bootstrap_password_env = normalise_optional_secret(self.bootstrap_password_env.take());
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

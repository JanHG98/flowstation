use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ControlRoomConfig {
    pub server: ServerConfig,
    pub persistence: PersistenceConfig,
}

impl Default for ControlRoomConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            persistence: PersistenceConfig::default(),
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

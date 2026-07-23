use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const OPEN_LAB_MODE: &str = "open_lab";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RecorderConfig {
    pub server: ServerConfig,
    pub media_switch: MediaSwitchConfig,
    pub storage: StorageConfig,
    pub security: SecurityConfig,
    pub limits: LimitsConfig,
}

impl Default for RecorderConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            media_switch: MediaSwitchConfig::default(),
            storage: StorageConfig::default(),
            security: SecurityConfig::default(),
            limits: LimitsConfig::default(),
        }
    }
}

impl RecorderConfig {
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
                "unsupported security.mode={}; this package intentionally implements only open_lab",
                self.security.mode
            ));
        }
        if !self.media_switch.tap_url.starts_with("http://")
            || !self.media_switch.sessions_url.starts_with("http://")
        {
            return Err("media_switch URLs must use http:// in open_lab mode".to_string());
        }
        if !self.security.allow_remote_management && !self.server.bind.ip().is_loopback() {
            return Err(
                "server.bind must use a loopback address when allow_remote_management=false"
                    .to_string(),
            );
        }
        if self.storage.root.as_os_str().is_empty() {
            return Err("storage.root must not be empty".to_string());
        }
        if self.storage.export_root.as_os_str().is_empty() {
            return Err("storage.export_root must not be empty".to_string());
        }

        self.server.history_limit = self.server.history_limit.max(100);
        self.media_switch.poll_interval_ms = self.media_switch.poll_interval_ms.clamp(20, 10_000);
        self.media_switch.session_reconcile_ms =
            self.media_switch.session_reconcile_ms.clamp(250, 60_000);
        self.media_switch.request_timeout_secs = self.media_switch.request_timeout_secs.max(1);
        self.media_switch.batch_limit = self.media_switch.batch_limit.clamp(1, 5_000);
        self.storage.frame_duration_ms = self.storage.frame_duration_ms.clamp(10, 1_000);
        self.storage.session_absent_grace_secs = self.storage.session_absent_grace_secs.max(1);
        self.storage.maximum_idle_secs = self.storage.maximum_idle_secs.max(5);
        self.storage.default_retention_days = self.storage.default_retention_days.clamp(1, 3_650);
        self.storage.retention_scan_secs = self.storage.retention_scan_secs.max(5);
        self.storage.fsync_every_frames = self.storage.fsync_every_frames.max(1);
        self.storage.minimum_free_space_mb = self.storage.minimum_free_space_mb.max(16);
        self.limits.max_body_bytes = self.limits.max_body_bytes.max(1_024);
        self.limits.max_active_recordings = self.limits.max_active_recordings.max(1);
        self.limits.max_recordings = self.limits.max_recordings.max(10);
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
            bind: "0.0.0.0:8140".parse().expect("valid default bind"),
            history_limit: 2_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MediaSwitchConfig {
    pub tap_url: String,
    pub sessions_url: String,
    pub poll_interval_ms: u64,
    pub session_reconcile_ms: u64,
    pub request_timeout_secs: u64,
    pub batch_limit: usize,
}

impl Default for MediaSwitchConfig {
    fn default() -> Self {
        Self {
            tap_url: "http://127.0.0.1:8130/api/v1/recorder/taps".to_string(),
            sessions_url: "http://127.0.0.1:8130/api/v1/sessions".to_string(),
            poll_interval_ms: 100,
            session_reconcile_ms: 1_000,
            request_timeout_secs: 3,
            batch_limit: 500,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StorageConfig {
    pub root: PathBuf,
    pub export_root: PathBuf,
    pub frame_duration_ms: u64,
    pub session_absent_grace_secs: u64,
    pub maximum_idle_secs: u64,
    pub default_retention_days: u32,
    pub retention_scan_secs: u64,
    pub fsync_every_frames: u64,
    pub minimum_free_space_mb: u64,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            root: PathBuf::from("/var/lib/netcore-recorder/recordings"),
            export_root: PathBuf::from("/var/lib/netcore-recorder/exports"),
            frame_duration_ms: 60,
            session_absent_grace_secs: 3,
            maximum_idle_secs: 600,
            default_retention_days: 30,
            retention_scan_secs: 60,
            fsync_every_frames: 50,
            minimum_free_space_mb: 512,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SecurityConfig {
    pub mode: String,
    pub allow_remote_management: bool,
    pub allow_delete: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            mode: OPEN_LAB_MODE.to_string(),
            allow_remote_management: true,
            allow_delete: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LimitsConfig {
    pub max_body_bytes: usize,
    pub max_active_recordings: usize,
    pub max_recordings: usize,
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            max_body_bytes: 1_048_576,
            max_active_recordings: 1_000,
            max_recordings: 100_000,
        }
    }
}

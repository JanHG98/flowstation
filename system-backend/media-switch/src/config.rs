use std::fs;
use std::net::SocketAddr;
use std::path::Path;

use serde::{Deserialize, Serialize};

pub const OPEN_LAB_MODE: &str = "open_lab";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MediaSwitchConfig {
    pub server: ServerConfig,
    pub node_gateway: NodeGatewayConfig,
    pub call_control: CallControlConfig,
    pub media: MediaConfig,
    pub security: SecurityConfig,
    pub limits: LimitsConfig,
}

impl Default for MediaSwitchConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            node_gateway: NodeGatewayConfig::default(),
            call_control: CallControlConfig::default(),
            media: MediaConfig::default(),
            security: SecurityConfig::default(),
            limits: LimitsConfig::default(),
        }
    }
}

impl MediaSwitchConfig {
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
            return Err("node_gateway.url must use ws:// in open_lab mode".to_string());
        }
        if !self.call_control.url.starts_with("http://") {
            return Err("call_control.url must use http:// in open_lab mode".to_string());
        }
        if !self.security.allow_remote_management && !self.server.bind.ip().is_loopback() {
            return Err(
                "server.bind must use a loopback address when allow_remote_management=false"
                    .to_string(),
            );
        }

        self.server.history_limit = self.server.history_limit.max(100);
        self.node_gateway.reconnect_secs = self.node_gateway.reconnect_secs.max(1);
        self.call_control.reconcile_secs = self.call_control.reconcile_secs.max(1);
        self.call_control.request_timeout_secs = self.call_control.request_timeout_secs.max(1);
        self.media.frame_duration_ms = self.media.frame_duration_ms.clamp(10, 1_000);
        self.media.jitter_buffer_frames = self
            .media
            .jitter_buffer_frames
            .min(self.media.max_jitter_buffer_frames.max(1));
        self.media.max_jitter_buffer_frames = self.media.max_jitter_buffer_frames.max(1);
        self.media.session_idle_secs = self.media.session_idle_secs.max(5);
        self.media.max_frames_per_tick = self.media.max_frames_per_tick.max(1);
        self.media.tap_history_frames = self.media.tap_history_frames.max(16);
        self.limits.max_body_bytes = self.limits.max_body_bytes.max(1_024);
        self.limits.max_sessions = self.limits.max_sessions.max(1);
        self.limits.max_streams = self.limits.max_streams.max(2);
        self.limits.max_pending_frames = self.limits.max_pending_frames.max(32);
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
            bind: "0.0.0.0:8130".parse().expect("valid default bind"),
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
            reconnect_secs: 2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CallControlConfig {
    pub url: String,
    pub reconcile_secs: u64,
    pub request_timeout_secs: u64,
}

impl Default for CallControlConfig {
    fn default() -> Self {
        Self {
            url: "http://127.0.0.1:8120/api/v1/calls".to_string(),
            reconcile_secs: 2,
            request_timeout_secs: 2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MediaConfig {
    pub frame_duration_ms: u64,
    pub jitter_buffer_frames: usize,
    pub max_jitter_buffer_frames: usize,
    pub session_idle_secs: u64,
    pub max_frames_per_tick: usize,
    pub allow_same_leg_loopback: bool,
    pub tap_history_frames: usize,
}

impl Default for MediaConfig {
    fn default() -> Self {
        Self {
            frame_duration_ms: 60,
            jitter_buffer_frames: 3,
            max_jitter_buffer_frames: 12,
            session_idle_secs: 30,
            max_frames_per_tick: 256,
            allow_same_leg_loopback: false,
            tap_history_frames: 256,
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
    pub max_sessions: usize,
    pub max_streams: usize,
    pub max_pending_frames: usize,
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            max_body_bytes: 1_048_576,
            max_sessions: 10_000,
            max_streams: 50_000,
            max_pending_frames: 100_000,
        }
    }
}

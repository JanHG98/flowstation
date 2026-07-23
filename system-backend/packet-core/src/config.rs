use std::fs;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const OPEN_LAB_MODE: &str = "open_lab";
pub const MODE_SHADOW: &str = "shadow";
pub const MODE_AUTHORITATIVE: &str = "authoritative";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PacketCoreConfig {
    pub server: ServerConfig,
    pub node_gateway: NodeGatewayConfig,
    pub storage: StorageConfig,
    pub packet: PacketConfig,
    pub address_pool: AddressPoolConfig,
    pub fragmentation: FragmentationConfig,
    pub flow_control: FlowControlConfig,
    pub security: SecurityConfig,
    pub limits: LimitsConfig,
}

impl Default for PacketCoreConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            node_gateway: NodeGatewayConfig::default(),
            storage: StorageConfig::default(),
            packet: PacketConfig::default(),
            address_pool: AddressPoolConfig::default(),
            fragmentation: FragmentationConfig::default(),
            flow_control: FlowControlConfig::default(),
            security: SecurityConfig::default(),
            limits: LimitsConfig::default(),
        }
    }
}

impl PacketCoreConfig {
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
        if !matches!(self.packet.mode.as_str(), MODE_SHADOW | MODE_AUTHORITATIVE) {
            return Err("packet.mode must be shadow or authoritative".to_string());
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
        if self.address_pool.first_host == 0
            || self.address_pool.last_host == 0
            || self.address_pool.first_host > self.address_pool.last_host
            || self.address_pool.last_host == 255
        {
            return Err("address_pool host range must be 1..254 and ordered".to_string());
        }
        self.server.history_limit = self.server.history_limit.max(100);
        self.node_gateway.reconnect_secs = self.node_gateway.reconnect_secs.max(1);
        self.packet.ready_timer_secs = self.packet.ready_timer_secs.max(1);
        self.packet.standby_timer_secs = self
            .packet
            .standby_timer_secs
            .max(self.packet.ready_timer_secs + 1);
        self.packet.response_wait_secs = self.packet.response_wait_secs.max(1);
        self.packet.context_ready_secs = self.packet.context_ready_secs.max(1);
        self.packet.max_contexts_per_subscriber = self.packet.max_contexts_per_subscriber.clamp(1, 14);
        self.packet.max_total_contexts = self.packet.max_total_contexts.max(1);
        self.packet.default_mtu = self.packet.default_mtu.clamp(128, 65_535);
        self.packet.max_n_pdu_bytes = self.packet.max_n_pdu_bytes.clamp(128, 65_535);
        self.fragmentation.timeout_secs = self.fragmentation.timeout_secs.max(1);
        self.fragmentation.max_datagrams = self.fragmentation.max_datagrams.max(1);
        self.fragmentation.max_total_bytes = self.fragmentation.max_total_bytes.max(65_535);
        self.fragmentation.max_fragments_per_datagram = self.fragmentation.max_fragments_per_datagram.max(2);
        self.flow_control.max_queue_packets_per_context = self.flow_control.max_queue_packets_per_context.max(1);
        self.flow_control.max_queue_bytes_per_context = self.flow_control.max_queue_bytes_per_context.max(1_024);
        self.flow_control.queue_ttl_secs = self.flow_control.queue_ttl_secs.max(1);
        self.flow_control.action_retry_secs = self.flow_control.action_retry_secs.max(1);
        self.flow_control.action_max_attempts = self.flow_control.action_max_attempts.max(1);
        self.limits.max_body_bytes = self.limits.max_body_bytes.max(1_024);
        self.limits.max_events = self.limits.max_events.max(100);
        self.limits.max_actions = self.limits.max_actions.max(100);
        self.limits.max_payload_bytes = self.limits.max_payload_bytes.clamp(128, 65_535);
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
            bind: "0.0.0.0:8160".parse().expect("valid default bind"),
            history_limit: 5_000,
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
            database_path: "/var/lib/netcore-packet-core/state.json".into(),
            backup_path: "/var/lib/netcore-packet-core/state.json.bak".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PacketConfig {
    pub mode: String,
    pub ready_timer_secs: u64,
    pub standby_timer_secs: u64,
    pub response_wait_secs: u64,
    pub context_ready_secs: u64,
    pub default_mtu: usize,
    pub max_n_pdu_bytes: usize,
    pub max_contexts_per_subscriber: usize,
    pub max_total_contexts: usize,
    pub strict_source_address: bool,
    pub preserve_context_on_node_loss: bool,
}
impl Default for PacketConfig {
    fn default() -> Self {
        Self {
            mode: MODE_SHADOW.to_string(),
            ready_timer_secs: 5,
            standby_timer_secs: 120,
            response_wait_secs: 10,
            context_ready_secs: 30,
            default_mtu: 480,
            max_n_pdu_bytes: 65_535,
            max_contexts_per_subscriber: 14,
            max_total_contexts: 8_192,
            strict_source_address: true,
            preserve_context_on_node_loss: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AddressPoolConfig {
    pub network_prefix: [u8; 3],
    pub first_host: u8,
    pub last_host: u8,
    pub gateway: Ipv4Addr,
    pub allow_static: bool,
}
impl Default for AddressPoolConfig {
    fn default() -> Self {
        Self {
            network_prefix: [10, 0, 0],
            first_host: 2,
            last_host: 254,
            gateway: Ipv4Addr::new(10, 0, 0, 1),
            allow_static: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FragmentationConfig {
    pub timeout_secs: u64,
    pub max_datagrams: usize,
    pub max_total_bytes: usize,
    pub max_fragments_per_datagram: usize,
    pub reject_overlaps: bool,
}
impl Default for FragmentationConfig {
    fn default() -> Self {
        Self {
            timeout_secs: 30,
            max_datagrams: 256,
            max_total_bytes: 8 * 1024 * 1024,
            max_fragments_per_datagram: 256,
            reject_overlaps: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FlowControlConfig {
    pub max_queue_packets_per_context: usize,
    pub max_queue_bytes_per_context: usize,
    pub queue_ttl_secs: u64,
    pub action_retry_secs: u64,
    pub action_max_attempts: u32,
}
impl Default for FlowControlConfig {
    fn default() -> Self {
        Self {
            max_queue_packets_per_context: 64,
            max_queue_bytes_per_context: 262_144,
            queue_ttl_secs: 30,
            action_retry_secs: 3,
            action_max_attempts: 5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SecurityConfig {
    pub mode: String,
    pub allow_remote_management: bool,
    pub expose_payloads: bool,
}
impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            mode: OPEN_LAB_MODE.to_string(),
            allow_remote_management: true,
            expose_payloads: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LimitsConfig {
    pub max_body_bytes: usize,
    pub max_events: usize,
    pub max_actions: usize,
    pub max_payload_bytes: usize,
}
impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            max_body_bytes: 2_097_152,
            max_events: 100_000,
            max_actions: 100_000,
            max_payload_bytes: 65_535,
        }
    }
}

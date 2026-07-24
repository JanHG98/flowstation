use std::fs;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const OPEN_LAB_MODE: &str = "open_lab";
pub const MODE_SHADOW: &str = "shadow";
pub const MODE_AUTHORITATIVE: &str = "authoritative";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct IpGatewayConfig {
    pub server: ServerConfig,
    pub packet_core: PacketCoreClientConfig,
    pub storage: StorageConfig,
    pub interface: InterfaceConfig,
    pub routing: RoutingConfig,
    pub nat: NatConfig,
    pub firewall: FirewallConfig,
    pub dns: DnsConfig,
    pub test_server: TestServerConfig,
    pub capture: CaptureConfig,
    pub security: SecurityConfig,
    pub limits: LimitsConfig,
}

impl Default for IpGatewayConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            packet_core: PacketCoreClientConfig::default(),
            storage: StorageConfig::default(),
            interface: InterfaceConfig::default(),
            routing: RoutingConfig::default(),
            nat: NatConfig::default(),
            firewall: FirewallConfig::default(),
            dns: DnsConfig::default(),
            test_server: TestServerConfig::default(),
            capture: CaptureConfig::default(),
            security: SecurityConfig::default(),
            limits: LimitsConfig::default(),
        }
    }
}

impl IpGatewayConfig {
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
        if !matches!(self.interface.mode.as_str(), MODE_SHADOW | MODE_AUTHORITATIVE) {
            return Err("interface.mode must be shadow or authoritative".to_string());
        }
        if !self.packet_core.url.starts_with("http://") {
            return Err("packet_core.url must use http:// in the open lab package".to_string());
        }
        if !self.security.allow_remote_management && !self.server.bind.ip().is_loopback() {
            return Err(
                "server.bind must use a loopback address when allow_remote_management=false"
                    .to_string(),
            );
        }
        validate_interface_name(&self.interface.name)?;
        let (network, prefix) = parse_cidr(&self.interface.network)?;
        let (gateway, gateway_prefix) = parse_cidr(&self.interface.address)?;
        if prefix != gateway_prefix {
            return Err("interface.address and interface.network must use the same prefix".to_string());
        }
        if network_address(gateway, prefix) != network_address(network, prefix) {
            return Err("interface.address must belong to interface.network".to_string());
        }
        if self.interface.mtu < 128 || self.interface.mtu > 65_535 {
            return Err("interface.mtu must be between 128 and 65535".to_string());
        }
        if (self.nat.enabled || self.firewall.allow_general_internet)
            && self.nat.egress_interface.trim().is_empty()
        {
            return Err(
                "nat.egress_interface may not be empty when NAT or general forwarding is enabled"
                    .to_string(),
            );
        }
        if !self.nat.egress_interface.trim().is_empty() {
            validate_interface_name(&self.nat.egress_interface)?;
        }
        if !matches!(self.firewall.default_forward_policy.as_str(), "accept" | "drop") {
            return Err("firewall.default_forward_policy must be accept or drop".to_string());
        }
        if (self.dns.enabled || self.test_server.enabled)
            && self.dns.local_domain.trim().is_empty()
        {
            return Err("dns.local_domain may not be empty while DNS or test services are enabled"
                .to_string());
        }
        if self.dns.enabled {
            self.dns
                .upstream
                .parse::<SocketAddr>()
                .map_err(|error| {
                    format!("dns.upstream must be a numeric socket address: {error}")
                })?;
        }
        self.packet_core.poll_interval_ms = self.packet_core.poll_interval_ms.clamp(50, 60_000);
        self.packet_core.context_refresh_ms = self.packet_core.context_refresh_ms.clamp(250, 300_000);
        self.packet_core.request_timeout_ms = self.packet_core.request_timeout_ms.clamp(250, 60_000);
        self.packet_core.outbox_batch = self.packet_core.outbox_batch.clamp(1, 5_000);
        self.routing.reconcile_interval_secs = self.routing.reconcile_interval_secs.max(1);
        self.dns.query_timeout_ms = self.dns.query_timeout_ms.clamp(100, 60_000);
        self.dns.ttl_secs = self.dns.ttl_secs.clamp(1, 86_400);
        self.capture.max_captures = self.capture.max_captures.max(1);
        self.capture.max_file_bytes = self.capture.max_file_bytes.max(65_536);
        self.capture.snaplen = self.capture.snaplen.clamp(64, 65_535);
        self.limits.max_body_bytes = self.limits.max_body_bytes.max(1_024);
        self.limits.max_events = self.limits.max_events.max(100);
        self.limits.max_flows = self.limits.max_flows.max(100);
        self.limits.max_packet_bytes = self.limits.max_packet_bytes.clamp(128, 65_535);
        Ok(())
    }

    pub fn gateway_ipv4(&self) -> Ipv4Addr {
        parse_cidr(&self.interface.address)
            .map(|(address, _)| address)
            .unwrap_or(Ipv4Addr::new(10, 0, 0, 1))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub bind: SocketAddr,
}
impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: "0.0.0.0:8170".parse().expect("valid default bind"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PacketCoreClientConfig {
    pub url: String,
    pub poll_interval_ms: u64,
    pub context_refresh_ms: u64,
    pub request_timeout_ms: u64,
    pub outbox_batch: usize,
}
impl Default for PacketCoreClientConfig {
    fn default() -> Self {
        Self {
            url: "http://127.0.0.1:8160".to_string(),
            poll_interval_ms: 250,
            context_refresh_ms: 1_000,
            request_timeout_ms: 2_000,
            outbox_batch: 250,
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
            database_path: "/var/lib/netcore-ip-gateway/state.json".into(),
            backup_path: "/var/lib/netcore-ip-gateway/state.json.bak".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct InterfaceConfig {
    pub mode: String,
    pub name: String,
    pub address: String,
    pub network: String,
    pub mtu: u16,
    pub owner_user: String,
    pub delete_on_exit: bool,
}
impl Default for InterfaceConfig {
    fn default() -> Self {
        Self {
            mode: MODE_SHADOW.to_string(),
            name: "ntc-tun0".to_string(),
            address: "10.0.0.1/24".to_string(),
            network: "10.0.0.0/24".to_string(),
            mtu: 480,
            owner_user: "netcore".to_string(),
            delete_on_exit: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RoutingConfig {
    pub enable_ipv4_forwarding: bool,
    pub reconcile_interval_secs: u64,
    pub install_connected_route: bool,
}
impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            enable_ipv4_forwarding: true,
            reconcile_interval_secs: 5,
            install_connected_route: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NatConfig {
    pub enabled: bool,
    pub masquerade: bool,
    pub egress_interface: String,
}
impl Default for NatConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            masquerade: true,
            egress_interface: "eth0".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FirewallConfig {
    pub enabled: bool,
    pub default_forward_policy: String,
    pub allow_established: bool,
    pub allow_general_internet: bool,
    pub allow_icmp: bool,
    pub log_drops: bool,
}
impl Default for FirewallConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_forward_policy: "drop".to_string(),
            allow_established: true,
            allow_general_internet: true,
            allow_icmp: true,
            log_drops: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DnsConfig {
    pub enabled: bool,
    pub bind: SocketAddr,
    pub upstream: String,
    pub local_domain: String,
    pub ttl_secs: u32,
    pub query_timeout_ms: u64,
}
impl Default for DnsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            bind: "0.0.0.0:53".parse().expect("valid DNS bind"),
            upstream: "1.1.1.1:53".to_string(),
            local_domain: "netcore.test".to_string(),
            ttl_secs: 30,
            query_timeout_ms: 2_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TestServerConfig {
    pub enabled: bool,
    pub bind: SocketAddr,
    pub udp_echo_bind: SocketAddr,
}
impl Default for TestServerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            bind: "0.0.0.0:8088".parse().expect("valid test server bind"),
            udp_echo_bind: "0.0.0.0:7007".parse().expect("valid UDP echo bind"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CaptureConfig {
    pub directory: PathBuf,
    pub max_captures: usize,
    pub max_file_bytes: u64,
    pub snaplen: usize,
}
impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            directory: "/var/lib/netcore-ip-gateway/captures".into(),
            max_captures: 64,
            max_file_bytes: 256 * 1024 * 1024,
            snaplen: 65_535,
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
    pub max_events: usize,
    pub max_flows: usize,
    pub max_packet_bytes: usize,
}
impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            max_body_bytes: 2_097_152,
            max_events: 100_000,
            max_flows: 100_000,
            max_packet_bytes: 65_535,
        }
    }
}

pub fn parse_cidr(value: &str) -> Result<(Ipv4Addr, u8), String> {
    let (address, prefix) = value
        .split_once('/')
        .ok_or_else(|| format!("invalid IPv4 CIDR {value}"))?;
    let address = address
        .parse::<Ipv4Addr>()
        .map_err(|_| format!("invalid IPv4 address in {value}"))?;
    let prefix = prefix
        .parse::<u8>()
        .map_err(|_| format!("invalid IPv4 prefix in {value}"))?;
    if prefix > 32 {
        return Err(format!("invalid IPv4 prefix in {value}"));
    }
    Ok((address, prefix))
}

pub fn network_address(address: Ipv4Addr, prefix: u8) -> Ipv4Addr {
    let value = u32::from(address);
    let mask = if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    };
    Ipv4Addr::from(value & mask)
}

fn validate_interface_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.len() >= libc::IFNAMSIZ {
        return Err(format!(
            "interface.name must be 1..{} characters",
            libc::IFNAMSIZ - 1
        ));
    }
    if !name
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err("interface.name contains unsupported characters".to_string());
    }
    Ok(())
}

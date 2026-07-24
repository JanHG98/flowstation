use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacketCoreStatus {
    pub mode: String,
}


#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacketCoreContext {
    pub id: String,
    pub issi: u32,
    pub nsapi: u8,
    pub node_id: String,
    pub ipv4: String,
    pub mtu: u16,
    pub priority: u8,
    pub state: String,
    pub available: bool,
    pub usage_active: bool,
    #[serde(default)]
    pub packets_up: u64,
    #[serde(default)]
    pub bytes_up: u64,
    #[serde(default)]
    pub packets_down: u64,
    #[serde(default)]
    pub bytes_down: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacketCoreNpdu {
    pub id: String,
    pub issi: u32,
    pub nsapi: u8,
    pub direction: String,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DownlinkNpduInput {
    pub issi: u32,
    pub nsapi: u8,
    pub payload_hex: String,
    pub acknowledged: bool,
    pub priority: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RouteRuleInput {
    pub name: String,
    pub destination: String,
    pub gateway: Option<String>,
    pub interface: Option<String>,
    pub metric: Option<u32>,
    pub enabled: bool,
}
impl Default for RouteRuleInput {
    fn default() -> Self {
        Self {
            name: String::new(),
            destination: String::new(),
            gateway: None,
            interface: None,
            metric: None,
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NatRuleInput {
    pub name: String,
    pub kind: String,
    pub source_cidr: Option<String>,
    pub destination_cidr: Option<String>,
    pub protocol: Option<String>,
    pub destination_port: Option<u16>,
    pub out_interface: Option<String>,
    pub to_address: Option<String>,
    pub to_port: Option<u16>,
    pub enabled: bool,
}
impl Default for NatRuleInput {
    fn default() -> Self {
        Self {
            name: String::new(),
            kind: "masquerade".to_string(),
            source_cidr: None,
            destination_cidr: None,
            protocol: None,
            destination_port: None,
            out_interface: None,
            to_address: None,
            to_port: None,
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FirewallRuleInput {
    pub name: String,
    pub chain: String,
    pub action: String,
    pub protocol: String,
    pub source_cidr: Option<String>,
    pub destination_cidr: Option<String>,
    pub source_port: Option<u16>,
    pub destination_port: Option<u16>,
    pub in_interface: Option<String>,
    pub out_interface: Option<String>,
    pub priority: i32,
    pub log: bool,
    pub enabled: bool,
}
impl Default for FirewallRuleInput {
    fn default() -> Self {
        Self {
            name: String::new(),
            chain: "forward".to_string(),
            action: "accept".to_string(),
            protocol: "any".to_string(),
            source_cidr: None,
            destination_cidr: None,
            source_port: None,
            destination_port: None,
            in_interface: None,
            out_interface: None,
            priority: 100,
            log: false,
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticDnsInput {
    pub name: String,
    pub address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CaptureStartInput {
    pub name: String,
    pub direction: String,
    pub host: Option<String>,
    pub protocol: Option<String>,
    pub port: Option<u16>,
}
impl Default for CaptureStartInput {
    fn default() -> Self {
        Self {
            name: String::new(),
            direction: "both".to_string(),
            host: None,
            protocol: None,
            port: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockAddressInput {
    pub address: String,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconcileInput {
    #[serde(default)]
    pub force: bool,
}

pub fn bytes_to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

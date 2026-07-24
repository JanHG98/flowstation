use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerCreateInput {
    pub peer_id: String,
    pub region_id: String,
    pub swmi_id: String,
    pub display_name: String,
    pub endpoint: String,
    pub protocol_version: Option<String>,
    pub priority: Option<i32>,
    pub capabilities: Vec<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PeerActionInput {
    pub actor: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerHeartbeatInput {
    pub region_id: String,
    pub swmi_id: String,
    pub display_name: String,
    pub advertised_endpoint: String,
    pub protocol_version: String,
    pub capabilities: Vec<String>,
    pub sent_at: String,
    pub sequence: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteCreateInput {
    pub service: String,
    pub selector_type: String,
    pub selector_value: String,
    pub destination_region: String,
    pub peer_id: String,
    pub preference: Option<i32>,
    pub metric: Option<u32>,
    pub failover_group: Option<String>,
    pub enabled: Option<bool>,
    pub expires_at: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RouteActionInput {
    pub actor: Option<String>,
    pub reason: Option<String>,
    pub preference: Option<i32>,
    pub metric: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriberLocationInput {
    pub issi: u32,
    pub home_region: String,
    pub current_region: String,
    pub serving_node: Option<String>,
    pub sequence: Option<u64>,
    pub source_peer: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupReachabilityInput {
    pub gssi: u32,
    pub regions: Vec<String>,
    pub source_peer: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteResolveInput {
    pub service: String,
    pub destination_kind: String,
    pub destination: String,
    pub target_region: Option<String>,
    pub trace: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitSubmitInput {
    pub service: String,
    pub operation: String,
    pub source_kind: String,
    pub source: String,
    pub destination_kind: String,
    pub destination: String,
    pub target_region: Option<String>,
    pub session_id: Option<String>,
    pub correlation_id: Option<String>,
    pub priority: Option<u8>,
    pub ttl_secs: Option<u64>,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitEnvelopeInput {
    pub protocol_version: String,
    pub envelope_id: String,
    pub dedupe_key: String,
    pub service: String,
    pub operation: String,
    pub origin_region: String,
    pub previous_hop_region: String,
    pub target_region: String,
    pub source_kind: String,
    pub source: String,
    pub destination_kind: String,
    pub destination: String,
    pub session_id: String,
    pub correlation_id: Option<String>,
    pub priority: u8,
    pub trace: Vec<String>,
    pub hop_count: u8,
    pub created_at: String,
    pub expires_at: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeliveryAckInput {
    pub success: bool,
    pub error: Option<String>,
    pub actor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionActionInput {
    pub actor: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MaintenanceInput {
    pub actor: Option<String>,
}

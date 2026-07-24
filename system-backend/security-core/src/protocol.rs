use serde::{Deserialize, Serialize};
use serde_json::Value;
use tetra_entities::net_control_room::{
    ControlRoomNodeCapabilities, ControlRoomNodeIdentity, NodeToControlRoomMessage,
};

pub const BACKEND_PROTOCOL_VERSION: &str = "netcore-node-gateway-backend-v1";
pub const EDGE_PROTOCOL_VERSION: &str = "netcore-security-edge-v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayNodeSnapshot {
    pub node_id: String,
    pub session_id: String,
    pub peer: String,
    pub connected: bool,
    pub stale: bool,
    pub connected_at: String,
    pub last_seen: String,
    pub disconnected_at: Option<String>,
    pub disconnect_reason: Option<String>,
    pub heartbeat_seq: u64,
    pub message_count: u64,
    pub telemetry_count: u64,
    pub control_ack_count: u64,
    pub control_response_count: u64,
    #[serde(default)]
    pub media_frame_count: u64,
    pub error_count: u64,
    pub last_message_kind: String,
    pub last_telemetry: Option<Value>,
    pub identity: ControlRoomNodeIdentity,
    pub capabilities: ControlRoomNodeCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayStatus {
    pub service: String,
    pub started_at: String,
    pub security_mode: String,
    pub warning: String,
    pub remote_management_enabled: bool,
    pub node_path: String,
    pub backend_path: String,
    pub known_nodes: usize,
    pub connected_nodes: usize,
    pub stale_nodes: usize,
    pub backend_clients: usize,
    pub total_node_sessions: u64,
    pub total_node_messages: u64,
    pub total_commands: u64,
    #[serde(default)]
    pub total_media_frames: u64,
    pub total_disconnects: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewaySnapshot {
    pub status: GatewayStatus,
    pub nodes: Vec<GatewayNodeSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayEventRecord {
    pub seq: u64,
    pub timestamp: String,
    pub kind: String,
    pub node_id: Option<String>,
    pub detail: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BackendEvent {
    Snapshot { snapshot: GatewaySnapshot },
    Event { event: GatewayEventRecord },
    NodeMessage { node_id: String, message: NodeToControlRoomMessage },
    ActionResult {
        request_id: Option<String>,
        command_id: Option<String>,
        ok: bool,
        message: String,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BackendRequest {
    Ping { request_id: Option<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileInput {
    pub issi: u32,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub authentication_required: Option<bool>,
    #[serde(default)]
    pub minimum_security_class: Option<u8>,
    #[serde(default)]
    pub preferred_security_class: Option<u8>,
    #[serde(default)]
    pub allow_class1_fallback: Option<bool>,
    #[serde(default)]
    pub allowed_nodes: Vec<String>,
    #[serde(default)]
    pub max_failures: Option<u32>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyInput {
    #[serde(default)]
    pub operating_mode: Option<String>,
    #[serde(default)]
    pub default_security_class: Option<u8>,
    #[serde(default)]
    pub minimum_security_class: Option<u8>,
    #[serde(default)]
    pub authentication_required: Option<bool>,
    #[serde(default)]
    pub allow_class1_fallback: Option<bool>,
    #[serde(default)]
    pub reject_unknown_subscribers: Option<bool>,
    #[serde(default)]
    pub disable_after_failures: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticationStartInput {
    pub node_id: String,
    pub issi: u32,
    #[serde(default)]
    pub requested_security_class: Option<u8>,
    #[serde(default)]
    pub supported_security_classes: Vec<u8>,
    #[serde(default)]
    pub equipment_id: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticationResponseInput {
    pub response_hex: String,
    #[serde(default)]
    pub node_id: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DisableInput {
    #[serde(default)]
    pub equipment: bool,
    #[serde(default)]
    pub equipment_id: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub actor: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RevokeInput {
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub actor: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AlarmAckInput {
    #[serde(default)]
    pub actor: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeClaimInput {
    pub node_id: String,
    #[serde(default = "default_claim_limit")]
    pub limit: usize,
}

fn default_claim_limit() -> usize {
    25
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeActionAckInput {
    #[serde(default = "default_ack_success")]
    pub success: bool,
    #[serde(default)]
    pub message: Option<String>,
}

fn default_ack_success() -> bool {
    true
}

impl Default for EdgeActionAckInput {
    fn default() -> Self {
        Self {
            success: true,
            message: None,
        }
    }
}

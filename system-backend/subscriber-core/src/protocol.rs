use serde::{Deserialize, Serialize};
use serde_json::Value;
use tetra_entities::net_control::ControlCommand;
use tetra_entities::net_control_room::{
    ControlRoomNodeCapabilities, ControlRoomNodeIdentity, NodeToControlRoomMessage,
};

pub const BACKEND_PROTOCOL_VERSION: &str = "netcore-node-gateway-backend-v1";

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

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BackendRequest {
    Ping { request_id: Option<String> },
    Command {
        request_id: Option<String>,
        node_id: String,
        command: ControlCommand,
        operator_id: Option<String>,
    },
}

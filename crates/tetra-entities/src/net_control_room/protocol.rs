use bitcode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use tetra_config::bluestation::StackConfig;
use tetra_core::tetra_entities::TetraEntity;

use crate::{
    net_control::{ControlCommand, ControlResponse},
    net_telemetry::TelemetryEvent,
};

/// Human/machine identity for this physical/logical base-station node.
///
/// `node_id` must be stable across restarts; the control room uses it as the
/// primary key for state, audit logs and command routing.
#[derive(Debug, Clone, Encode, Decode, Serialize, Deserialize)]
pub struct ControlRoomNodeIdentity {
    pub node_id: String,
    pub station_name: String,
    pub site: Option<String>,
    pub stack_version: String,
    pub mcc: u16,
    pub mnc: u16,
    pub location_area: u16,
    pub main_carrier: u16,
    pub secondary_carrier: Option<u16>,
    pub colour_code: u8,
    pub system_code: u8,
}

impl ControlRoomNodeIdentity {
    /// Build a deterministic node identity from config.  A configured node_id wins;
    /// otherwise we derive a readable stable id from the TETRA cell parameters.
    pub fn from_stack_config(
        cfg: &StackConfig,
        node_id: Option<String>,
        station_name: Option<String>,
        site: Option<String>,
    ) -> Self {
        let derived_id = format!(
            "tbs-{}-{}-la{}-cc{}-c{}",
            cfg.net.mcc, cfg.net.mnc, cfg.cell.location_area, cfg.cell.colour_code, cfg.cell.main_carrier
        );
        let node_id = node_id.unwrap_or(derived_id);
        let station_name = station_name.unwrap_or_else(|| node_id.clone());

        Self {
            node_id,
            station_name,
            site,
            stack_version: tetra_core::STACK_VERSION.to_string(),
            mcc: cfg.net.mcc,
            mnc: cfg.net.mnc,
            location_area: cfg.cell.location_area,
            main_carrier: cfg.cell.main_carrier,
            secondary_carrier: cfg.cell.secondary_carrier,
            colour_code: cfg.cell.colour_code,
            system_code: cfg.cell.system_code,
        }
    }
}

/// Capability flags advertised by the base station during hello.  Keep this
/// explicit so the Leitstelle can gray out buttons instead of guessing.
#[derive(Debug, Clone, Encode, Decode, Serialize, Deserialize)]
pub struct ControlRoomNodeCapabilities {
    pub telemetry: bool,
    pub command: bool,
    pub sds: bool,
    pub raw_sds: bool,
    pub dgna: bool,
    pub kick_ms: bool,
    pub emergency_clear: bool,
    pub live_sds: bool,
    pub service_control: bool,
    pub brew_bridge: bool,
    pub dual_carrier: bool,
    /// Node exports SNDCP/PDP/TUN packet-data telemetry and APIs.
    #[serde(default)]
    pub packet_data: bool,
    /// Node accepts WAP/WDP or WAP/SDS-TL payloads through raw SDS Type 4.
    #[serde(default)]
    pub legacy_wap_sds: bool,
    /// Node can maintain more than one independently allocated PDCH bearer.
    #[serde(default)]
    pub multi_pdch: bool,
    /// Node accepts centrally managed subscriber admission policies.
    #[serde(default)]
    pub subscriber_policy: bool,
}

impl ControlRoomNodeCapabilities {
    pub fn from_stack_config(cfg: &StackConfig) -> Self {
        Self {
            telemetry: true,
            command: true,
            sds: true,
            raw_sds: true,
            dgna: true,
            kick_ms: true,
            emergency_clear: true,
            live_sds: cfg.cell.home_mode_display.is_some() || cfg.cell.sds_broadcast.is_some(),
            service_control: cfg.service_name.is_some(),
            brew_bridge: cfg.brew.is_some() || cfg.brew2.is_some(),
            dual_carrier: cfg.cell.secondary_carrier.is_some(),
            packet_data: cfg.cell.wap_ip_sndcp_profile_enabled(),
            legacy_wap_sds: true,
            multi_pdch: cfg.cell.wap_ip_sndcp_profile_enabled(),
            subscriber_policy: true,
        }
    }
}

#[derive(Debug, Clone, Encode, Decode, Serialize, Deserialize)]
pub struct ControlRoomNodeHello {
    pub protocol_version: String,
    pub node: ControlRoomNodeIdentity,
    pub capabilities: ControlRoomNodeCapabilities,
    pub started_at: String,
}

#[derive(Debug, Clone, Encode, Decode, Serialize, Deserialize)]
pub struct ControlRoomNodeHeartbeat {
    pub node_id: String,
    pub seq: u64,
    pub timestamp: String,
    pub connected: bool,
}

#[derive(Debug, Clone, Encode, Decode, Serialize, Deserialize)]
pub struct NodeTelemetryEnvelope {
    pub node_id: String,
    pub seq: u64,
    pub timestamp: String,
    pub event: TelemetryEvent,
}

#[derive(Debug, Clone, Encode, Decode, Serialize, Deserialize)]
pub struct ControlCommandEnvelope {
    pub command_id: String,
    pub target_node_id: String,
    pub operator_id: Option<String>,
    pub issued_at: String,
    pub command: ControlCommand,
}

#[derive(Debug, Clone, Encode, Decode, Serialize, Deserialize)]
pub struct ControlCommandAck {
    pub command_id: String,
    pub node_id: String,
    pub accepted: bool,
    pub target_entity: Option<TetraEntity>,
    pub message: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Encode, Decode, Serialize, Deserialize)]
pub struct ControlResponseEnvelope {
    /// `None` means the legacy entity response could not be correlated back to
    /// a specific command.  The raw response is still valuable for logs/state.
    pub command_id: Option<String>,
    pub node_id: String,
    pub target_entity: Option<TetraEntity>,
    pub timestamp: String,
    pub response: ControlResponse,
}

/// Messages sent from the base station node to the Control-Room Core.
#[derive(Debug, Clone, Encode, Decode, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NodeToControlRoomMessage {
    Hello { hello: ControlRoomNodeHello },
    Heartbeat { heartbeat: ControlRoomNodeHeartbeat },
    Telemetry { envelope: NodeTelemetryEnvelope },
    ControlAck { ack: ControlCommandAck },
    ControlResponse { envelope: ControlResponseEnvelope },
    Error { node_id: String, message: String, timestamp: String },
}

/// Messages sent by the Control-Room Core to the base station node.
#[derive(Debug, Clone, Encode, Decode, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ControlRoomToNodeMessage {
    HelloAck { accepted: bool, message: Option<String> },
    Ping { seq: u64, timestamp: String },
    Command { envelope: ControlCommandEnvelope },
}

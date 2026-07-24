use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::io::Write;
use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tetra_entities::net_control::{ControlCommand, ControlResponse};
use tetra_entities::net_control_room::NodeToControlRoomMessage;
use tetra_entities::net_telemetry::TelemetryEvent;
use uuid::Uuid;

use crate::config::{MODE_AUTHORITATIVE, PacketCoreConfig};
use crate::protocol::{
    ActionAckInput, BackendEvent, BackendRequest, ContextActionInput, DownlinkNpduInput,
    EDGE_PROTOCOL_VERSION, EdgeActionPayload, EdgeEventInput, GatewaySnapshot,
};

const DATABASE_SCHEMA_VERSION: u32 = 1;
const OPEN_LAB_WARNING: &str =
    "OPEN LAB: no authentication, no tokens and no TLS; isolated test network only";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContextState {
    Activating,
    Standby,
    ResponseWaiting,
    Ready,
    Quiescent,
    Suspended,
    Deactivating,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionState {
    Pending,
    InFlight,
    Applied,
    Failed,
    Expired,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdpContextRecord {
    pub id: String,
    pub issi: u32,
    pub nsapi: u8,
    pub node_id: String,
    pub anchor_node_id: String,
    pub ipv4: String,
    pub primary_nsapi: Option<u8>,
    pub snei: Option<u16>,
    pub mtu: u16,
    pub priority: u8,
    pub state: ContextState,
    pub available: bool,
    pub usage_active: bool,
    pub source: String,
    pub created_at: String,
    pub updated_at: String,
    pub last_activity_at: String,
    pub ready_deadline: Option<String>,
    #[serde(default)]
    pub context_ready_deadline: Option<String>,
    pub standby_deadline: Option<String>,
    pub response_wait_deadline: Option<String>,
    pub packets_up: u64,
    pub bytes_up: u64,
    pub packets_down: u64,
    pub bytes_down: u64,
    pub dropped_packets: u64,
    pub queued_packets: u32,
    pub queued_bytes: u64,
    pub carrier_num: Option<u16>,
    pub logical_ts: Option<u8>,
    pub air_ts: Option<u8>,
    pub last_error: Option<String>,
    pub revision: u64,
}

impl PdpContextRecord {
    fn key(issi: u32, nsapi: u8) -> String {
        format!("{issi}:{nsapi}")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacketNodeRecord {
    pub node_id: String,
    pub station_name: String,
    pub connected: bool,
    pub stale: bool,
    pub packet_data_capable: bool,
    pub multi_pdch_capable: bool,
    pub mcc: Option<u16>,
    pub mnc: Option<u16>,
    pub location_area: Option<u16>,
    pub last_seen: String,
    pub last_error: Option<String>,
    pub gateway_running: bool,
    pub interface_name: Option<String>,
    pub gateway_address: Option<String>,
    pub active_contexts: u32,
    pub active_bearers: u32,
    pub bearer_capacity: u8,
    pub traffic_slots_free: u8,
    pub packets_from_mobile: u64,
    pub bytes_from_mobile: u64,
    pub packets_to_mobile: u64,
    pub bytes_to_mobile: u64,
    pub dropped_packets: u64,
    pub io_errors: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdchBearerRecord {
    pub id: String,
    pub node_id: String,
    pub issi: u32,
    pub carrier_num: u16,
    pub logical_ts: u8,
    pub air_ts: u8,
    pub nsapis: Vec<u8>,
    pub active: bool,
    pub first_seen: String,
    pub last_seen: String,
    pub age_secs: u64,
    pub idle_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeActionRecord {
    pub id: String,
    pub sequence: u64,
    pub node_id: String,
    pub context_id: Option<String>,
    pub state: ActionState,
    pub payload: EdgeActionPayload,
    pub created_at: String,
    pub updated_at: String,
    pub expires_at: String,
    pub attempts: u32,
    pub max_attempts: u32,
    pub next_attempt_at: Option<String>,
    pub command_handle: Option<u32>,
    pub command_id: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReassemblyRecord {
    pub id: String,
    pub node_id: String,
    pub issi: u32,
    pub nsapi: u8,
    pub datagram_id: String,
    pub direction: String,
    pub created_at: String,
    pub updated_at: String,
    pub expires_at: String,
    pub total_len: Option<usize>,
    pub received_bytes: usize,
    pub fragment_count: usize,
    pub segments: BTreeMap<usize, Vec<u8>>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpduRecord {
    pub id: String,
    pub node_id: String,
    pub issi: u32,
    pub nsapi: u8,
    pub direction: String,
    pub datagram_id: String,
    pub payload: Vec<u8>,
    pub created_at: String,
    pub delivered: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacketEventRecord {
    pub sequence: u64,
    pub timestamp: String,
    pub kind: String,
    pub node_id: Option<String>,
    pub context_id: Option<String>,
    pub detail: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PacketDatabase {
    schema_version: u32,
    revision: u64,
    next_event_sequence: u64,
    next_action_sequence: u64,
    next_handle: u32,
    contexts: BTreeMap<String, PdpContextRecord>,
    nodes: BTreeMap<String, PacketNodeRecord>,
    bearers: BTreeMap<String, PdchBearerRecord>,
    actions: BTreeMap<String, EdgeActionRecord>,
    reassemblies: BTreeMap<String, ReassemblyRecord>,
    npdu_outbox: VecDeque<NpduRecord>,
    events: VecDeque<PacketEventRecord>,
}

impl Default for PacketDatabase {
    fn default() -> Self {
        Self {
            schema_version: DATABASE_SCHEMA_VERSION,
            revision: 0,
            next_event_sequence: 1,
            next_action_sequence: 1,
            next_handle: 1,
            contexts: BTreeMap::new(),
            nodes: BTreeMap::new(),
            bearers: BTreeMap::new(),
            actions: BTreeMap::new(),
            reassemblies: BTreeMap::new(),
            npdu_outbox: VecDeque::new(),
            events: VecDeque::new(),
        }
    }
}

struct PacketCoreState {
    config: PacketCoreConfig,
    database: PacketDatabase,
    started_at: String,
    node_gateway_connected: bool,
    node_gateway_last_error: Option<String>,
    pending_commands: BTreeMap<u32, String>,
    reassembly_completed: u64,
    reassembly_failed: u64,
}

#[derive(Clone)]
pub struct SharedPacketCore {
    inner: Arc<Mutex<PacketCoreState>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PacketCoreStatus {
    pub service: &'static str,
    pub version: &'static str,
    pub protocol_version: &'static str,
    pub started_at: String,
    pub security_mode: &'static str,
    pub warning: &'static str,
    pub mode: String,
    pub authoritative: bool,
    pub node_gateway_connected: bool,
    pub node_gateway_last_error: Option<String>,
    pub database_revision: u64,
    pub nodes_known: usize,
    pub nodes_connected: usize,
    pub contexts_total: usize,
    pub contexts_ready: usize,
    pub contexts_standby: usize,
    pub contexts_suspended: usize,
    pub bearers_active: usize,
    pub actions_pending: usize,
    pub reassemblies_active: usize,
    pub npdu_outbox: usize,
    pub queued_packets: u64,
    pub queued_bytes: u64,
    pub reassembly_completed: u64,
    pub reassembly_failed: u64,
}

impl SharedPacketCore {
    pub fn load(config: PacketCoreConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let database = match fs::read(&config.storage.database_path) {
            Ok(bytes) => match serde_json::from_slice::<PacketDatabase>(&bytes) {
                Ok(database) if database.schema_version == DATABASE_SCHEMA_VERSION => database,
                Ok(_) => return Err("unsupported Packet Core database schema".into()),
                Err(error) => {
                    tracing::warn!("Packet Core database invalid, trying backup: {error}");
                    read_backup(&config)?
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => PacketDatabase::default(),
            Err(error) => return Err(error.into()),
        };
        Ok(Self {
            inner: Arc::new(Mutex::new(PacketCoreState {
                config,
                database,
                started_at: now_iso(),
                node_gateway_connected: false,
                node_gateway_last_error: None,
                pending_commands: BTreeMap::new(),
                reassembly_completed: 0,
                reassembly_failed: 0,
            })),
        })
    }

    pub fn gateway_connected(&self) {
        let mut state = self.lock();
        state.node_gateway_connected = true;
        state.node_gateway_last_error = None;
        state.event("node_gateway_connected", None, None, json!({}));
    }

    pub fn gateway_disconnected(&self, error: String) {
        let mut state = self.lock();
        state.node_gateway_connected = false;
        state.node_gateway_last_error = Some(error.clone());
        state.event("node_gateway_disconnected", None, None, json!({"error": error}));
    }

    pub fn status(&self) -> PacketCoreStatus {
        let state = self.lock();
        state.status()
    }

    pub fn config(&self) -> PacketCoreConfig {
        self.lock().config.clone()
    }

    pub fn nodes(&self) -> Vec<PacketNodeRecord> {
        self.lock().database.nodes.values().cloned().collect()
    }

    pub fn contexts(&self) -> Vec<PdpContextRecord> {
        self.lock().database.contexts.values().cloned().collect()
    }

    pub fn context(&self, id: &str) -> Option<PdpContextRecord> {
        self.lock().database.contexts.get(id).cloned()
    }

    pub fn bearers(&self) -> Vec<PdchBearerRecord> {
        self.lock().database.bearers.values().cloned().collect()
    }

    pub fn actions(&self, node_id: Option<&str>, after: u64, limit: usize) -> Vec<EdgeActionRecord> {
        self.lock()
            .database
            .actions
            .values()
            .filter(|action| action.sequence > after)
            .filter(|action| node_id.map_or(true, |node_id| action.node_id == node_id))
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn reassemblies(&self) -> Vec<ReassemblyRecord> {
        self.lock().database.reassemblies.values().cloned().collect()
    }

    pub fn npdu_outbox(&self, limit: usize) -> Vec<NpduRecord> {
        self.lock().database.npdu_outbox.iter().take(limit).cloned().collect()
    }

    pub fn recent_events(&self, limit: usize) -> Vec<PacketEventRecord> {
        self.lock().database.events.iter().rev().take(limit).cloned().collect()
    }

    pub fn handle_backend_event(&self, event: BackendEvent) {
        let mut state = self.lock();
        match event {
            BackendEvent::Snapshot { snapshot } => state.apply_gateway_snapshot(snapshot),
            BackendEvent::Event { event } => {
                if event.kind.contains("disconnect") {
                    if let Some(node_id) = event.node_id.as_deref() {
                        state.mark_node_lost(node_id, "node gateway disconnect");
                    }
                }
            }
            BackendEvent::NodeMessage { node_id, message } => {
                state.handle_node_message(&node_id, message);
            }
            BackendEvent::ActionResult { request_id, command_id, ok, message } => {
                state.event(
                    "gateway_action_result",
                    None,
                    None,
                    json!({"request_id":request_id,"command_id":command_id,"ok":ok,"message":message}),
                );
            }
        }
        state.persist_logged();
    }

    pub fn ingest_edge_event(&self, event: EdgeEventInput) -> Result<Vec<EdgeActionRecord>, String> {
        let mut state = self.lock();
        let actions = state.ingest_edge_event(event)?;
        state.persist().map_err(|error| error.to_string())?;
        Ok(actions)
    }

    pub fn acknowledge_action(&self, id: &str, input: ActionAckInput) -> Result<EdgeActionRecord, String> {
        let mut state = self.lock();
        let now = now_iso();
        let (result, should_settle) = {
            let action = state
                .database
                .actions
                .get_mut(id)
                .ok_or_else(|| "action not found".to_string())?;
            let should_settle = matches!(action.state, ActionState::Pending | ActionState::InFlight);
            action.state = if input.success { ActionState::Applied } else { ActionState::Failed };
            action.updated_at = now;
            action.last_error = if input.success { None } else { input.message.clone() };
            (action.clone(), should_settle)
        };
        if should_settle {
            state.settle_npdu_action(&result, input.success);
        }
        state.event(
            if input.success { "edge_action_applied" } else { "edge_action_failed" },
            Some(result.node_id.clone()),
            result.context_id.clone(),
            json!({"action_id":id,"message":input.message}),
        );
        state.persist().map_err(|error| error.to_string())?;
        Ok(result)
    }

    pub fn queue_downlink(&self, input: DownlinkNpduInput) -> Result<Vec<EdgeActionRecord>, String> {
        let mut state = self.lock();
        let actions = state.queue_downlink(input)?;
        state.persist().map_err(|error| error.to_string())?;
        Ok(actions)
    }

    pub fn context_action(
        &self,
        context_id: &str,
        action: &str,
        input: ContextActionInput,
    ) -> Result<(Vec<EdgeActionRecord>, Vec<BackendRequest>), String> {
        let mut state = self.lock();
        let result = state.context_action(context_id, action, input)?;
        state.persist().map_err(|error| error.to_string())?;
        Ok(result)
    }

    pub fn delete_npdu(&self, id: &str) -> Result<(), String> {
        let mut state = self.lock();
        let before = state.database.npdu_outbox.len();
        state.database.npdu_outbox.retain(|npdu| npdu.id != id);
        if state.database.npdu_outbox.len() == before {
            return Err("N-PDU not found".to_string());
        }
        state.touch();
        state.persist().map_err(|error| error.to_string())
    }

    pub fn tick(&self) -> Vec<BackendRequest> {
        let mut state = self.lock();
        let requests = state.tick();
        state.persist_logged();
        requests
    }

    pub fn metrics(&self) -> String {
        let status = self.status();
        format!(
            concat!(
                "# HELP netcore_packet_core_contexts PDP contexts by current service\n",
                "# TYPE netcore_packet_core_contexts gauge\n",
                "netcore_packet_core_contexts{{state=\"total\"}} {}\n",
                "netcore_packet_core_contexts{{state=\"ready\"}} {}\n",
                "netcore_packet_core_contexts{{state=\"standby\"}} {}\n",
                "netcore_packet_core_contexts{{state=\"suspended\"}} {}\n",
                "# TYPE netcore_packet_core_bearers gauge\n",
                "netcore_packet_core_bearers {}\n",
                "# TYPE netcore_packet_core_actions_pending gauge\n",
                "netcore_packet_core_actions_pending {}\n",
                "# TYPE netcore_packet_core_reassemblies gauge\n",
                "netcore_packet_core_reassemblies {}\n",
                "# TYPE netcore_packet_core_npdu_outbox gauge\n",
                "netcore_packet_core_npdu_outbox {}\n",
                "# TYPE netcore_packet_core_gateway_connected gauge\n",
                "netcore_packet_core_gateway_connected {}\n"
            ),
            status.contexts_total,
            status.contexts_ready,
            status.contexts_standby,
            status.contexts_suspended,
            status.bearers_active,
            status.actions_pending,
            status.reassemblies_active,
            status.npdu_outbox,
            u8::from(status.node_gateway_connected),
        )
    }

    pub fn export(&self) -> Value {
        let state = self.lock();
        json!({
            "status": state.status(),
            "nodes": state.database.nodes,
            "contexts": state.database.contexts,
            "bearers": state.database.bearers,
            "actions": state.database.actions,
            "reassemblies": state.database.reassemblies,
            "npdu_outbox": state.database.npdu_outbox,
            "events": state.database.events,
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, PacketCoreState> {
        self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl PacketCoreState {
    fn status(&self) -> PacketCoreStatus {
        let contexts_ready = self
            .database
            .contexts
            .values()
            .filter(|context| context.state == ContextState::Ready)
            .count();
        let contexts_standby = self
            .database
            .contexts
            .values()
            .filter(|context| context.state == ContextState::Standby)
            .count();
        let contexts_suspended = self
            .database
            .contexts
            .values()
            .filter(|context| context.state == ContextState::Suspended)
            .count();
        PacketCoreStatus {
            service: "netcore-packet-core",
            version: env!("CARGO_PKG_VERSION"),
            protocol_version: EDGE_PROTOCOL_VERSION,
            started_at: self.started_at.clone(),
            security_mode: "open_lab",
            warning: OPEN_LAB_WARNING,
            mode: self.config.packet.mode.clone(),
            authoritative: self.config.packet.mode == MODE_AUTHORITATIVE,
            node_gateway_connected: self.node_gateway_connected,
            node_gateway_last_error: self.node_gateway_last_error.clone(),
            database_revision: self.database.revision,
            nodes_known: self.database.nodes.len(),
            nodes_connected: self.database.nodes.values().filter(|node| node.connected).count(),
            contexts_total: self.database.contexts.len(),
            contexts_ready,
            contexts_standby,
            contexts_suspended,
            bearers_active: self.database.bearers.values().filter(|bearer| bearer.active).count(),
            actions_pending: self
                .database
                .actions
                .values()
                .filter(|action| matches!(action.state, ActionState::Pending | ActionState::InFlight))
                .count(),
            reassemblies_active: self.database.reassemblies.len(),
            npdu_outbox: self.database.npdu_outbox.len(),
            queued_packets: self.database.contexts.values().map(|context| u64::from(context.queued_packets)).sum(),
            queued_bytes: self.database.contexts.values().map(|context| context.queued_bytes).sum(),
            reassembly_completed: self.reassembly_completed,
            reassembly_failed: self.reassembly_failed,
        }
    }

    fn apply_gateway_snapshot(&mut self, snapshot: GatewaySnapshot) {
        let now = now_iso();
        let mut seen = BTreeSet::new();
        for node in snapshot.nodes {
            seen.insert(node.node_id.clone());
            let record = self.database.nodes.entry(node.node_id.clone()).or_insert_with(|| PacketNodeRecord {
                node_id: node.node_id.clone(),
                station_name: node.identity.station_name.clone(),
                connected: node.connected,
                stale: node.stale,
                packet_data_capable: node.capabilities.packet_data,
                multi_pdch_capable: node.capabilities.multi_pdch,
                mcc: Some(node.identity.mcc),
                mnc: Some(node.identity.mnc),
                location_area: Some(node.identity.location_area),
                last_seen: node.last_seen.clone(),
                last_error: node.disconnect_reason.clone(),
                gateway_running: false,
                interface_name: None,
                gateway_address: None,
                active_contexts: 0,
                active_bearers: 0,
                bearer_capacity: 0,
                traffic_slots_free: 0,
                packets_from_mobile: 0,
                bytes_from_mobile: 0,
                packets_to_mobile: 0,
                bytes_to_mobile: 0,
                dropped_packets: 0,
                io_errors: 0,
            });
            record.station_name = node.identity.station_name;
            record.connected = node.connected;
            record.stale = node.stale;
            record.packet_data_capable = node.capabilities.packet_data;
            record.multi_pdch_capable = node.capabilities.multi_pdch;
            record.mcc = Some(node.identity.mcc);
            record.mnc = Some(node.identity.mnc);
            record.location_area = Some(node.identity.location_area);
            record.last_seen = node.last_seen;
            record.last_error = node.disconnect_reason;
        }
        for node in self.database.nodes.values_mut() {
            if !seen.contains(&node.node_id) {
                node.connected = false;
                node.last_seen = now.clone();
            }
        }
        self.touch();
    }

    fn handle_node_message(&mut self, node_id: &str, message: NodeToControlRoomMessage) {
        match message {
            NodeToControlRoomMessage::Telemetry { envelope } => match envelope.event {
                TelemetryEvent::PacketDataSnapshot { gateway, contexts, bearers } => {
                    self.import_packet_snapshot(node_id, gateway, contexts, bearers);
                }
                TelemetryEvent::MsRegistration { issi } => {
                    self.event("subscriber_seen", Some(node_id.to_string()), None, json!({"issi":issi}));
                }
                TelemetryEvent::MsDeregistration { issi } | TelemetryEvent::MsTimeoutDrop { issi } => {
                    if !self.config.packet.preserve_context_on_node_loss {
                        self.remove_contexts_for_subscriber(issi, "subscriber deregistered");
                    }
                }
                _ => {}
            },
            NodeToControlRoomMessage::ControlResponse { envelope } => {
                self.handle_control_response(node_id, envelope.response);
            }
            NodeToControlRoomMessage::ControlAck { ack } => {
                self.event(
                    "packet_command_ack",
                    Some(node_id.to_string()),
                    None,
                    json!({"command_id":ack.command_id,"accepted":ack.accepted,"message":ack.message}),
                );
            }
            NodeToControlRoomMessage::Heartbeat { heartbeat } => {
                if let Some(node) = self.database.nodes.get_mut(node_id) {
                    node.connected = heartbeat.connected;
                    node.last_seen = heartbeat.timestamp;
                }
            }
            NodeToControlRoomMessage::Hello { hello } => {
                self.database.nodes.insert(node_id.to_string(), PacketNodeRecord {
                    node_id: node_id.to_string(),
                    station_name: hello.node.station_name,
                    connected: true,
                    stale: false,
                    packet_data_capable: hello.capabilities.packet_data,
                    multi_pdch_capable: hello.capabilities.multi_pdch,
                    mcc: Some(hello.node.mcc),
                    mnc: Some(hello.node.mnc),
                    location_area: Some(hello.node.location_area),
                    last_seen: now_iso(),
                    last_error: None,
                    gateway_running: false,
                    interface_name: None,
                    gateway_address: None,
                    active_contexts: 0,
                    active_bearers: 0,
                    bearer_capacity: 0,
                    traffic_slots_free: 0,
                    packets_from_mobile: 0,
                    bytes_from_mobile: 0,
                    packets_to_mobile: 0,
                    bytes_to_mobile: 0,
                    dropped_packets: 0,
                    io_errors: 0,
                });
            }
            NodeToControlRoomMessage::Error { message, .. } => {
                if let Some(node) = self.database.nodes.get_mut(node_id) {
                    node.last_error = Some(message.clone());
                }
                self.event("node_error", Some(node_id.to_string()), None, json!({"message":message}));
            }
            NodeToControlRoomMessage::MediaFrame { .. } => {}
        }
        self.touch();
    }

    fn import_packet_snapshot(
        &mut self,
        node_id: &str,
        gateway: tetra_entities::net_telemetry::events::PacketDataGatewayTelemetry,
        contexts: Vec<tetra_entities::net_telemetry::events::PacketDataContextTelemetry>,
        bearers: Vec<tetra_entities::net_telemetry::events::PdchBearerTelemetry>,
    ) {
        let now = now_iso();
        let node = self.database.nodes.entry(node_id.to_string()).or_insert_with(|| PacketNodeRecord {
            node_id: node_id.to_string(),
            station_name: node_id.to_string(),
            connected: true,
            stale: false,
            packet_data_capable: true,
            multi_pdch_capable: true,
            mcc: None,
            mnc: None,
            location_area: None,
            last_seen: now.clone(),
            last_error: None,
            gateway_running: gateway.running,
            interface_name: Some(gateway.interface_name.clone()),
            gateway_address: Some(gateway.gateway_address.clone()),
            active_contexts: gateway.active_contexts,
            active_bearers: gateway.active_bearers,
            bearer_capacity: gateway.bearer_capacity,
            traffic_slots_free: gateway.traffic_slots_free,
            packets_from_mobile: gateway.packets_from_mobile,
            bytes_from_mobile: gateway.bytes_from_mobile,
            packets_to_mobile: gateway.packets_to_mobile,
            bytes_to_mobile: gateway.bytes_to_mobile,
            dropped_packets: gateway.dropped_from_mobile.saturating_add(gateway.dropped_to_mobile),
            io_errors: gateway.io_errors,
        });
        node.connected = true;
        node.last_seen = now.clone();
        node.gateway_running = gateway.running;
        node.interface_name = Some(gateway.interface_name);
        node.gateway_address = Some(gateway.gateway_address);
        node.active_contexts = gateway.active_contexts;
        node.active_bearers = gateway.active_bearers;
        node.bearer_capacity = gateway.bearer_capacity;
        node.traffic_slots_free = gateway.traffic_slots_free;
        node.packets_from_mobile = gateway.packets_from_mobile;
        node.bytes_from_mobile = gateway.bytes_from_mobile;
        node.packets_to_mobile = gateway.packets_to_mobile;
        node.bytes_to_mobile = gateway.bytes_to_mobile;
        node.dropped_packets = gateway.dropped_from_mobile.saturating_add(gateway.dropped_to_mobile);
        node.io_errors = gateway.io_errors;

        let mut seen_contexts = BTreeSet::new();
        for item in contexts {
            let key = PdpContextRecord::key(item.issi, item.nsapi);
            seen_contexts.insert(key.clone());
            let mapped_state = parse_context_state(&item.state);
            let context = self.database.contexts.entry(key.clone()).or_insert_with(|| PdpContextRecord {
                id: key.clone(),
                issi: item.issi,
                nsapi: item.nsapi,
                node_id: node_id.to_string(),
                anchor_node_id: node_id.to_string(),
                ipv4: item.ipv4.clone(),
                primary_nsapi: item.primary_nsapi,
                snei: item.snei,
                mtu: item.mtu,
                priority: item.priority,
                state: mapped_state,
                available: mapped_state != ContextState::Suspended,
                usage_active: mapped_state != ContextState::Quiescent,
                source: "node_gateway_shadow".to_string(),
                created_at: now.clone(),
                updated_at: now.clone(),
                last_activity_at: now.clone(),
                ready_deadline: None,
                context_ready_deadline: None,
                standby_deadline: None,
                response_wait_deadline: None,
                packets_up: 0,
                bytes_up: 0,
                packets_down: 0,
                bytes_down: 0,
                dropped_packets: 0,
                queued_packets: item.queued_packets,
                queued_bytes: item.queued_bytes,
                carrier_num: item.carrier_num,
                logical_ts: item.logical_ts,
                air_ts: item.air_ts,
                last_error: None,
                revision: 1,
            });
            context.node_id = node_id.to_string();
            context.anchor_node_id = node_id.to_string();
            context.ipv4 = item.ipv4;
            context.primary_nsapi = item.primary_nsapi;
            context.snei = item.snei;
            context.mtu = item.mtu;
            context.priority = item.priority;
            context.state = mapped_state;
            context.available = mapped_state != ContextState::Suspended;
            context.usage_active = mapped_state != ContextState::Quiescent;
            context.updated_at = now.clone();
            context.last_activity_at = now.clone();
            context.queued_packets = item.queued_packets;
            context.queued_bytes = item.queued_bytes;
            context.carrier_num = item.carrier_num;
            context.logical_ts = item.logical_ts;
            context.air_ts = item.air_ts;
            context.revision = context.revision.saturating_add(1);
        }
        let stale = self
            .database
            .contexts
            .iter()
            .filter(|(_, context)| context.node_id == node_id && context.source == "node_gateway_shadow")
            .filter(|(key, _)| !seen_contexts.contains(*key))
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();
        for key in stale {
            self.database.contexts.remove(&key);
        }

        self.database.bearers.retain(|_, bearer| bearer.node_id != node_id);
        for bearer in bearers {
            let id = format!("{}:{}:{}", node_id, bearer.issi, bearer.logical_ts);
            self.database.bearers.insert(id.clone(), PdchBearerRecord {
                id,
                node_id: node_id.to_string(),
                issi: bearer.issi,
                carrier_num: bearer.carrier_num,
                logical_ts: bearer.logical_ts,
                air_ts: bearer.air_ts,
                nsapis: bearer.nsapis,
                active: true,
                first_seen: now.clone(),
                last_seen: now.clone(),
                age_secs: bearer.age_secs,
                idle_secs: bearer.idle_secs,
            });
        }
        self.touch();
    }

    fn handle_control_response(&mut self, node_id: &str, response: ControlResponse) {
        if let ControlResponse::PacketDataActionResult {
            handle,
            action,
            issi,
            nsapi,
            success,
            message,
        } = response
        {
            let settled = if let Some(action_id) = self.pending_commands.remove(&handle)
                && let Some(record) = self.database.actions.get_mut(&action_id)
            {
                let should_settle = matches!(record.state, ActionState::Pending | ActionState::InFlight);
                record.state = if success { ActionState::Applied } else { ActionState::Failed };
                record.updated_at = now_iso();
                record.last_error = if success { None } else { Some(message.clone()) };
                should_settle.then(|| record.clone())
            } else {
                None
            };
            if let Some(action) = settled.as_ref() {
                self.settle_npdu_action(action, success);
            }
            self.event(
                "packet_command_result",
                Some(node_id.to_string()),
                nsapi.map(|nsapi| PdpContextRecord::key(issi, nsapi)),
                json!({"handle":handle,"action":action,"success":success,"message":message}),
            );
        }
    }

    fn ingest_edge_event(&mut self, event: EdgeEventInput) -> Result<Vec<EdgeActionRecord>, String> {
        let node_id = event.node_id().to_string();
        let now = now_iso();
        self.ensure_node(&node_id);
        if let Some(node) = self.database.nodes.get_mut(&node_id) {
            node.connected = !matches!(&event, EdgeEventInput::NodeLost { .. });
            node.last_seen = now.clone();
        }
        let mut actions = Vec::new();
        match event {
            EdgeEventInput::Hello {
                protocol_version,
                node_id,
                station_name,
                mcc,
                mnc,
                location_area,
            } => {
                if protocol_version != EDGE_PROTOCOL_VERSION {
                    return Err(format!("unsupported edge protocol {protocol_version}"));
                }
                let node = self.database.nodes.get_mut(&node_id).expect("node ensured");
                node.station_name = station_name.unwrap_or_else(|| node_id.clone());
                node.mcc = mcc;
                node.mnc = mnc;
                node.location_area = location_area;
                node.packet_data_capable = true;
                self.event("edge_hello", Some(node_id), None, json!({"protocol":protocol_version}));
            }
            EdgeEventInput::Heartbeat { node_id, sequence } => {
                self.event("edge_heartbeat", Some(node_id), None, json!({"sequence":sequence}));
            }
            EdgeEventInput::SubscriberLocation { node_id, issi } => {
                for context in self.database.contexts.values_mut().filter(|context| context.issi == issi) {
                    context.node_id = node_id.clone();
                    context.updated_at = now.clone();
                    context.revision = context.revision.saturating_add(1);
                }
                self.event("subscriber_location", Some(node_id), None, json!({"issi":issi}));
            }
            EdgeEventInput::ActivateDemand {
                node_id,
                issi,
                nsapi,
                requested_ipv4,
                primary_nsapi,
                snei,
                mtu,
                priority,
            } => {
                validate_nsapi(nsapi)?;
                if self.config.packet.mode != MODE_AUTHORITATIVE {
                    self.event(
                        "activate_demand_shadow",
                        Some(node_id),
                        Some(PdpContextRecord::key(issi, nsapi)),
                        json!({"requested_ipv4":requested_ipv4,"primary_nsapi":primary_nsapi,"snei":snei,"mtu":mtu,"priority":priority}),
                    );
                } else {
                    let context_count = self.database.contexts.values().filter(|context| context.issi == issi).count();
                    if self.database.contexts.len() >= self.config.packet.max_total_contexts
                        || context_count >= self.config.packet.max_contexts_per_subscriber
                    {
                        actions.push(self.queue_action(
                            node_id,
                            Some(PdpContextRecord::key(issi, nsapi)),
                            EdgeActionPayload::ActivateReject {
                                issi,
                                nsapi,
                                cause: "maximum_contexts_reached".to_string(),
                            },
                        ));
                    } else {
                        let ipv4 = self.allocate_ipv4(requested_ipv4.as_deref())?;
                        let snei = snei.unwrap_or_else(|| self.allocate_snei(issi));
                        let mtu = mtu.unwrap_or(self.config.packet.default_mtu as u16).max(128);
                        let priority = priority.unwrap_or(4).min(7);
                        let key = PdpContextRecord::key(issi, nsapi);
                        let context = PdpContextRecord {
                            id: key.clone(),
                            issi,
                            nsapi,
                            node_id: node_id.clone(),
                            anchor_node_id: node_id.clone(),
                            ipv4: ipv4.clone(),
                            primary_nsapi,
                            snei: Some(snei),
                            mtu,
                            priority,
                            state: ContextState::Standby,
                            available: true,
                            usage_active: true,
                            source: "packet_core_authoritative".to_string(),
                            created_at: now.clone(),
                            updated_at: now.clone(),
                            last_activity_at: now.clone(),
                            ready_deadline: None,
                            context_ready_deadline: None,
                            standby_deadline: Some(deadline(self.config.packet.standby_timer_secs)),
                            response_wait_deadline: None,
                            packets_up: 0,
                            bytes_up: 0,
                            packets_down: 0,
                            bytes_down: 0,
                            dropped_packets: 0,
                            queued_packets: 0,
                            queued_bytes: 0,
                            carrier_num: None,
                            logical_ts: None,
                            air_ts: None,
                            last_error: None,
                            revision: 1,
                        };
                        self.database.contexts.insert(key.clone(), context);
                        actions.push(self.queue_action(
                            node_id,
                            Some(key),
                            EdgeActionPayload::ActivateAccept {
                                issi,
                                nsapi,
                                ipv4,
                                primary_nsapi,
                                snei,
                                mtu,
                                priority,
                                ready_timer_secs: self.config.packet.ready_timer_secs,
                                standby_timer_secs: self.config.packet.standby_timer_secs,
                                response_wait_secs: self.config.packet.response_wait_secs,
                            },
                        ));
                    }
                }
            }
            EdgeEventInput::ContextActivated {
                node_id,
                issi,
                nsapi,
                ipv4,
                primary_nsapi,
                snei,
                mtu,
                priority,
            } => {
                validate_nsapi(nsapi)?;
                let key = PdpContextRecord::key(issi, nsapi);
                self.upsert_context_from_edge(
                    &node_id,
                    issi,
                    nsapi,
                    ipv4,
                    primary_nsapi,
                    snei,
                    mtu,
                    priority,
                    ContextState::Standby,
                    "edge_activated",
                );
                self.event("context_activated", Some(node_id), Some(key), json!({}));
            }
            EdgeEventInput::DataTransmitRequest { node_id, issi, nsapis } => {
                let mut accepted = Vec::new();
                for nsapi in nsapis {
                    validate_nsapi(nsapi)?;
                    let key = PdpContextRecord::key(issi, nsapi);
                    if let Some(context) = self.database.contexts.get_mut(&key) {
                        context.state = ContextState::Ready;
                        context.last_activity_at = now.clone();
                        context.updated_at = now.clone();
                        context.ready_deadline = Some(deadline(self.config.packet.ready_timer_secs));
                        context.context_ready_deadline = Some(deadline(self.config.packet.context_ready_secs));
                        context.standby_deadline = None;
                        context.response_wait_deadline = None;
                        context.revision = context.revision.saturating_add(1);
                        accepted.push(nsapi);
                    }
                }
                if self.config.packet.mode == MODE_AUTHORITATIVE {
                    actions.push(self.queue_action(
                        node_id,
                        None,
                        EdgeActionPayload::DataTransmitResponse {
                            issi,
                            nsapis: accepted.clone(),
                            accepted: !accepted.is_empty(),
                            cause: accepted.is_empty().then(|| "unknown_nsapi".to_string()),
                        },
                    ));
                } else {
                    self.event(
                        "data_transmit_request_shadow",
                        Some(node_id),
                        None,
                        json!({"issi":issi,"accepted_nsapis":accepted}),
                    );
                }
            }
            EdgeEventInput::EndOfData { node_id, issi, nsapis } => {
                self.enter_standby(issi, &nsapis);
                self.event("end_of_data", Some(node_id), None, json!({"issi":issi,"nsapis":nsapis}));
            }
            EdgeEventInput::Reconnect { node_id, issi, nsapis, data_to_send } => {
                let found = nsapis.iter().any(|nsapi| self.database.contexts.contains_key(&PdpContextRecord::key(issi, *nsapi)));
                if found && data_to_send {
                    self.enter_ready(issi, &nsapis);
                }
                if self.config.packet.mode == MODE_AUTHORITATIVE {
                    actions.push(self.queue_action(
                        node_id,
                        None,
                        EdgeActionPayload::DataTransmitResponse {
                            issi,
                            nsapis,
                            accepted: found,
                            cause: (!found).then(|| "unknown_nsapi".to_string()),
                        },
                    ));
                } else {
                    self.event(
                        "reconnect_shadow",
                        Some(node_id),
                        None,
                        json!({"issi":issi,"nsapis":nsapis,"data_to_send":data_to_send,"known":found}),
                    );
                }
            }
            EdgeEventInput::Modify {
                node_id,
                issi,
                nsapi,
                availability,
                usage_active,
                priority,
                mtu,
            } => {
                let key = PdpContextRecord::key(issi, nsapi);
                let context = self.database.contexts.get_mut(&key).ok_or_else(|| "context not found".to_string())?;
                if let Some(available) = availability {
                    context.available = available;
                    context.state = if available { ContextState::Standby } else { ContextState::Suspended };
                }
                if let Some(active) = usage_active {
                    context.usage_active = active;
                    if !active {
                        context.state = ContextState::Quiescent;
                    }
                }
                if let Some(priority) = priority {
                    context.priority = priority.min(7);
                }
                if let Some(mtu) = mtu {
                    context.mtu = mtu.max(128);
                }
                context.updated_at = now;
                context.revision = context.revision.saturating_add(1);
                self.event("context_modified", Some(node_id), Some(key), json!({}));
            }
            EdgeEventInput::Deactivate { node_id, issi, nsapi, reason } => {
                self.deactivate_contexts(issi, nsapi, reason.as_deref().unwrap_or("edge deactivation"));
                self.event("context_deactivated", Some(node_id), None, json!({"issi":issi,"nsapi":nsapi}));
            }
            EdgeEventInput::Bearer {
                node_id,
                issi,
                carrier_num,
                logical_ts,
                air_ts,
                nsapis,
                active,
            } => {
                let id = format!("{}:{}:{}", node_id, issi, logical_ts);
                self.database.bearers.insert(id.clone(), PdchBearerRecord {
                    id,
                    node_id: node_id.clone(),
                    issi,
                    carrier_num,
                    logical_ts,
                    air_ts,
                    nsapis: nsapis.clone(),
                    active,
                    first_seen: now.clone(),
                    last_seen: now.clone(),
                    age_secs: 0,
                    idle_secs: 0,
                });
                for nsapi in nsapis {
                    if let Some(context) = self.database.contexts.get_mut(&PdpContextRecord::key(issi, nsapi)) {
                        context.carrier_num = active.then_some(carrier_num);
                        context.logical_ts = active.then_some(logical_ts);
                        context.air_ts = active.then_some(air_ts);
                    }
                }
            }
            EdgeEventInput::Fragment {
                node_id,
                issi,
                nsapi,
                datagram_id,
                direction,
                offset,
                more_fragments,
                total_len,
                payload_hex,
            } => {
                let payload = decode_hex(&payload_hex)?;
                self.ingest_fragment(
                    node_id,
                    issi,
                    nsapi,
                    datagram_id,
                    direction,
                    offset,
                    more_fragments,
                    total_len,
                    payload,
                )?;
            }
            EdgeEventInput::PacketCounters {
                node_id,
                issi,
                nsapi,
                packets_up,
                bytes_up,
                packets_down,
                bytes_down,
                dropped,
            } => {
                let key = PdpContextRecord::key(issi, nsapi);
                let context = self.database.contexts.get_mut(&key).ok_or_else(|| "context not found".to_string())?;
                context.packets_up = packets_up;
                context.bytes_up = bytes_up;
                context.packets_down = packets_down;
                context.bytes_down = bytes_down;
                context.dropped_packets = dropped;
                context.last_activity_at = now;
                context.revision = context.revision.saturating_add(1);
                self.event("packet_counters", Some(node_id), Some(key), json!({}));
            }
            EdgeEventInput::NodeLost { node_id, reason } => {
                self.mark_node_lost(&node_id, reason.as_deref().unwrap_or("edge node lost"));
            }
        }
        self.touch();
        Ok(actions)
    }

    fn queue_downlink(&mut self, input: DownlinkNpduInput) -> Result<Vec<EdgeActionRecord>, String> {
        let key = PdpContextRecord::key(input.issi, input.nsapi);
        let context = self.database.contexts.get(&key).cloned().ok_or_else(|| "context not found".to_string())?;
        if !context.available || !context.usage_active {
            return Err("context is unavailable or inactive".to_string());
        }
        let payload = decode_hex(&input.payload_hex)?;
        if payload.is_empty() {
            return Err("N-PDU payload must not be empty".to_string());
        }
        if payload.len() > self.config.packet.max_n_pdu_bytes || payload.len() > self.config.limits.max_payload_bytes {
            return Err("N-PDU exceeds configured maximum".to_string());
        }
        if usize::try_from(context.queued_packets).unwrap_or(usize::MAX)
            >= self.config.flow_control.max_queue_packets_per_context
            || usize::try_from(context.queued_bytes)
                .unwrap_or(usize::MAX)
                .saturating_add(payload.len())
                > self.config.flow_control.max_queue_bytes_per_context
        {
            return Err("flow-control queue limit reached".to_string());
        }
        let mtu = usize::from(context.mtu).max(128);
        let chunk = ((mtu / 8) * 8).max(8);
        let datagram_id = Uuid::new_v4().to_string();
        let mut actions = Vec::new();
        let wake_required = matches!(context.state, ContextState::Standby | ContextState::Quiescent);
        let wake_pending = self.database.actions.values().any(|action| {
            action.context_id.as_deref() == Some(&key)
                && matches!(action.state, ActionState::Pending | ActionState::InFlight)
                && matches!(&action.payload, EdgeActionPayload::Page { .. })
        });
        if wake_required && !wake_pending {
            actions.push(self.queue_action(
                context.node_id.clone(),
                Some(key.clone()),
                EdgeActionPayload::Page {
                    issi: input.issi,
                    nsapi: input.nsapi,
                },
            ));
        }
        let mut fragment_count = 0usize;
        for (index, part) in payload.chunks(chunk).enumerate() {
            let offset = index * chunk;
            fragment_count = fragment_count.saturating_add(1);
            actions.push(self.queue_action(
                context.node_id.clone(),
                Some(key.clone()),
                EdgeActionPayload::NpduFragment {
                    issi: input.issi,
                    nsapi: input.nsapi,
                    datagram_id: datagram_id.clone(),
                    offset,
                    more_fragments: offset + part.len() < payload.len(),
                    total_len: payload.len(),
                    payload_hex: encode_hex(part),
                    acknowledged: input.acknowledged,
                },
            ));
        }
        if let Some(context) = self.database.contexts.get_mut(&key) {
            context.queued_packets = context.queued_packets.saturating_add(1);
            context.queued_bytes = context.queued_bytes.saturating_add(payload.len() as u64);
            context.priority = input.priority.unwrap_or(context.priority).min(7);
            if wake_required {
                context.state = ContextState::ResponseWaiting;
                context.response_wait_deadline = Some(deadline(self.config.packet.response_wait_secs));
                context.ready_deadline = None;
                context.context_ready_deadline = None;
            }
            context.updated_at = now_iso();
            context.revision = context.revision.saturating_add(1);
        }
        self.event(
            "downlink_npdu_queued",
            Some(context.node_id),
            Some(key),
            json!({"datagram_id":datagram_id,"bytes":payload.len(),"fragments":fragment_count,"wake_queued":wake_required && !wake_pending}),
        );
        self.touch();
        Ok(actions)
    }

    fn context_action(
        &mut self,
        context_id: &str,
        action: &str,
        input: ContextActionInput,
    ) -> Result<(Vec<EdgeActionRecord>, Vec<BackendRequest>), String> {
        let context = self.database.contexts.get(context_id).cloned().ok_or_else(|| "context not found".to_string())?;
        let mut actions = Vec::new();
        let payload = match action {
            "deactivate" => EdgeActionPayload::Deactivate {
                issi: context.issi,
                nsapi: Some(context.nsapi),
                reason: input.reason.clone().unwrap_or_else(|| "operator request".to_string()),
            },
            "suspend" => EdgeActionPayload::Modify {
                issi: context.issi,
                nsapi: context.nsapi,
                availability: Some(false),
                usage_active: None,
                priority: None,
                mtu: None,
            },
            "resume" => EdgeActionPayload::Modify {
                issi: context.issi,
                nsapi: context.nsapi,
                availability: Some(true),
                usage_active: Some(true),
                priority: input.priority,
                mtu: input.mtu,
            },
            "modify" => EdgeActionPayload::Modify {
                issi: context.issi,
                nsapi: context.nsapi,
                availability: input.available,
                usage_active: input.usage_active,
                priority: input.priority,
                mtu: input.mtu,
            },
            "wake" => EdgeActionPayload::Page {
                issi: context.issi,
                nsapi: context.nsapi,
            },
            "end-of-data" => EdgeActionPayload::EndOfData {
                issi: context.issi,
                nsapis: vec![context.nsapi],
            },
            "flush" => {
                for record in self.database.actions.values_mut() {
                    if record.context_id.as_deref() == Some(context_id)
                        && matches!(record.state, ActionState::Pending | ActionState::InFlight)
                        && matches!(&record.payload, EdgeActionPayload::NpduFragment { .. })
                    {
                        record.state = ActionState::Cancelled;
                        record.updated_at = now_iso();
                    }
                }
                if let Some(current) = self.database.contexts.get_mut(context_id) {
                    current.queued_packets = 0;
                    current.queued_bytes = 0;
                }
                self.event("context_queue_flushed", Some(context.node_id), Some(context_id.to_string()), json!({}));
                return Ok((Vec::new(), Vec::new()));
            }
            _ => return Err("unknown context action".to_string()),
        };
        let queued = self.queue_action(context.node_id.clone(), Some(context_id.to_string()), payload);
        let backend = self.backend_command_for_action(&queued);
        if let Some(current) = self.database.contexts.get_mut(context_id) {
            match action {
                "deactivate" => current.state = ContextState::Deactivating,
                "suspend" => {
                    current.state = ContextState::Suspended;
                    current.available = false;
                }
                "resume" => {
                    current.state = ContextState::Standby;
                    current.available = true;
                    current.usage_active = true;
                }
                "wake" => {
                    current.state = ContextState::ResponseWaiting;
                    current.response_wait_deadline = Some(deadline(self.config.packet.response_wait_secs));
                }
                "end-of-data" => current.state = ContextState::Standby,
                "modify" => {
                    if let Some(available) = input.available {
                        current.available = available;
                    }
                    if let Some(active) = input.usage_active {
                        current.usage_active = active;
                    }
                    if let Some(priority) = input.priority {
                        current.priority = priority.min(7);
                    }
                    if let Some(mtu) = input.mtu {
                        current.mtu = mtu.max(128);
                    }
                }
                _ => {}
            }
            current.updated_at = now_iso();
            current.revision = current.revision.saturating_add(1);
        }
        actions.push(self.database.actions.get(&queued.id).cloned().unwrap_or(queued));
        Ok((actions, backend.into_iter().collect()))
    }

    fn backend_command_for_action(
        &mut self,
        action: &EdgeActionRecord,
    ) -> Option<BackendRequest> {
        if !self.node_gateway_connected {
            return None;
        }
        let handle = self.allocate_handle();
        let command = match &action.payload {
            EdgeActionPayload::Deactivate { issi, nsapi, reason } => {
                ControlCommand::PacketDataContextDeactivate {
                    handle,
                    issi: *issi,
                    nsapi: *nsapi,
                    reason: reason.clone(),
                }
            }
            EdgeActionPayload::Modify {
                issi,
                nsapi,
                availability,
                usage_active,
                priority,
                mtu,
            } => ControlCommand::PacketDataContextModify {
                handle,
                issi: *issi,
                nsapi: *nsapi,
                available: *availability,
                usage_active: *usage_active,
                priority: *priority,
                mtu: *mtu,
            },
            EdgeActionPayload::Page { issi, nsapi } => ControlCommand::PacketDataWake {
                handle,
                issi: *issi,
                nsapis: vec![*nsapi],
            },
            EdgeActionPayload::EndOfData { issi, nsapis } => {
                ControlCommand::PacketDataEndOfData {
                    handle,
                    issi: *issi,
                    nsapis: nsapis.clone(),
                }
            }
            _ => return None,
        };
        self.pending_commands.insert(handle, action.id.clone());
        if let Some(record) = self.database.actions.get_mut(&action.id) {
            record.command_handle = Some(handle);
            record.command_id = Some(action.id.clone());
            record.state = ActionState::InFlight;
            record.attempts = record.attempts.saturating_add(1);
            record.updated_at = now_iso();
            record.next_attempt_at = Some(deadline(self.config.flow_control.action_retry_secs));
        }
        Some(BackendRequest::Command {
            request_id: Some(action.id.clone()),
            node_id: action.node_id.clone(),
            command,
            operator_id: Some("packet-core-open-lab".to_string()),
        })
    }

    fn collect_due_backend_requests(&mut self) -> Vec<BackendRequest> {
        if !self.node_gateway_connected {
            return Vec::new();
        }
        let actions = self
            .database
            .actions
            .values()
            .filter(|action| action.state == ActionState::Pending)
            .cloned()
            .collect::<Vec<_>>();
        let mut requests = Vec::new();
        for action in actions {
            if let Some(request) = self.backend_command_for_action(&action) {
                requests.push(request);
            }
        }
        requests
    }

    fn ingest_fragment(
        &mut self,
        node_id: String,
        issi: u32,
        nsapi: u8,
        datagram_id: String,
        direction: String,
        offset: usize,
        more_fragments: bool,
        total_len: Option<usize>,
        payload: Vec<u8>,
    ) -> Result<(), String> {
        validate_nsapi(nsapi)?;
        if !matches!(direction.as_str(), "uplink" | "downlink") {
            return Err("fragment direction must be uplink or downlink".to_string());
        }
        if payload.is_empty() {
            return Err("fragment payload must not be empty".to_string());
        }
        if payload.len() > self.config.limits.max_payload_bytes {
            return Err("fragment payload too large".to_string());
        }
        let id = format!("{}:{}:{}:{}:{}", node_id, issi, nsapi, direction, datagram_id);
        if !self.database.reassemblies.contains_key(&id)
            && self.database.reassemblies.len() >= self.config.fragmentation.max_datagrams
        {
            return Err("reassembly table full".to_string());
        }
        let total_bytes = self
            .database
            .reassemblies
            .values()
            .map(|assembly| assembly.received_bytes)
            .sum::<usize>();
        if total_bytes.saturating_add(payload.len()) > self.config.fragmentation.max_total_bytes {
            return Err("reassembly byte limit reached".to_string());
        }
        let now = now_iso();
        let expires_at = deadline(self.config.fragmentation.timeout_secs);
        let assembly = self.database.reassemblies.entry(id.clone()).or_insert_with(|| ReassemblyRecord {
            id: id.clone(),
            node_id: node_id.clone(),
            issi,
            nsapi,
            datagram_id: datagram_id.clone(),
            direction: direction.clone(),
            created_at: now.clone(),
            updated_at: now.clone(),
            expires_at: expires_at.clone(),
            total_len,
            received_bytes: 0,
            fragment_count: 0,
            segments: BTreeMap::new(),
            last_error: None,
        });
        if assembly.fragment_count >= self.config.fragmentation.max_fragments_per_datagram {
            return Err("too many fragments in datagram".to_string());
        }
        if self.config.fragmentation.reject_overlaps {
            let end = offset.saturating_add(payload.len());
            let overlap = assembly.segments.iter().any(|(existing_offset, existing)| {
                let existing_end = existing_offset.saturating_add(existing.len());
                offset < existing_end && *existing_offset < end
            });
            if overlap {
                assembly.last_error = Some("overlapping fragment rejected".to_string());
                self.reassembly_failed = self.reassembly_failed.saturating_add(1);
                return Err("overlapping fragment rejected".to_string());
            }
        }
        assembly.received_bytes = assembly.received_bytes.saturating_add(payload.len());
        assembly.fragment_count = assembly.fragment_count.saturating_add(1);
        assembly.segments.insert(offset, payload);
        assembly.updated_at = now;
        assembly.expires_at = expires_at;
        if let Some(total_len) = total_len {
            assembly.total_len = Some(total_len);
        }
        if !more_fragments {
            let inferred = assembly
                .segments
                .iter()
                .map(|(offset, segment)| offset.saturating_add(segment.len()))
                .max()
                .unwrap_or(0);
            assembly.total_len = Some(assembly.total_len.unwrap_or(inferred).max(inferred));
        }
        let completed_payload = complete_reassembly(assembly);
        if let Some(payload) = completed_payload {
            if direction == "uplink" && self.config.packet.strict_source_address {
                let expected = self
                    .database
                    .contexts
                    .get(&PdpContextRecord::key(issi, nsapi))
                    .and_then(|context| context.ipv4.parse::<Ipv4Addr>().ok());
                if let Some(expected) = expected
                    && payload.len() >= 20
                    && payload[0] >> 4 == 4
                    && [payload[12], payload[13], payload[14], payload[15]] != expected.octets()
                {
                    self.database.reassemblies.remove(&id);
                    self.reassembly_failed = self.reassembly_failed.saturating_add(1);
                    if let Some(context) = self.database.contexts.get_mut(&PdpContextRecord::key(issi, nsapi)) {
                        context.dropped_packets = context.dropped_packets.saturating_add(1);
                        context.last_error = Some("uplink IPv4 source address rejected".to_string());
                        context.updated_at = now_iso();
                        context.revision = context.revision.saturating_add(1);
                    }
                    self.event(
                        "uplink_source_rejected",
                        Some(node_id),
                        Some(PdpContextRecord::key(issi, nsapi)),
                        json!({"datagram_id":datagram_id,"expected_ipv4":expected.to_string()}),
                    );
                    return Err("uplink IPv4 source address does not match PDP context".to_string());
                }
            }
            let record = NpduRecord {
                id: Uuid::new_v4().to_string(),
                node_id: node_id.clone(),
                issi,
                nsapi,
                direction: direction.clone(),
                datagram_id: datagram_id.clone(),
                payload,
                created_at: now_iso(),
                delivered: false,
            };
            let bytes = record.payload.len();
            self.database.npdu_outbox.push_back(record);
            self.database.reassemblies.remove(&id);
            self.reassembly_completed = self.reassembly_completed.saturating_add(1);
            if let Some(context) = self.database.contexts.get_mut(&PdpContextRecord::key(issi, nsapi)) {
                if direction == "uplink" {
                    context.packets_up = context.packets_up.saturating_add(1);
                    context.bytes_up = context.bytes_up.saturating_add(bytes as u64);
                } else {
                    context.packets_down = context.packets_down.saturating_add(1);
                    context.bytes_down = context.bytes_down.saturating_add(bytes as u64);
                }
                context.last_activity_at = now_iso();
                context.updated_at = now_iso();
                context.revision = context.revision.saturating_add(1);
            }
            self.event(
                "npdu_reassembled",
                Some(node_id),
                Some(PdpContextRecord::key(issi, nsapi)),
                json!({"datagram_id":datagram_id,"direction":direction,"bytes":bytes}),
            );
        }
        Ok(())
    }

    fn settle_npdu_action(&mut self, action: &EdgeActionRecord, delivered: bool) {
        let EdgeActionPayload::NpduFragment {
            payload_hex,
            more_fragments,
            ..
        } = &action.payload
        else {
            return;
        };
        let Some(context_id) = action.context_id.as_deref() else {
            return;
        };
        let Some(context) = self.database.contexts.get_mut(context_id) else {
            return;
        };
        context.queued_bytes = context
            .queued_bytes
            .saturating_sub((payload_hex.len() / 2) as u64);
        if !more_fragments {
            context.queued_packets = context.queued_packets.saturating_sub(1);
        }
        if !delivered {
            context.dropped_packets = context.dropped_packets.saturating_add(1);
        }
        context.updated_at = now_iso();
        context.revision = context.revision.saturating_add(1);
    }

    fn queue_action(
        &mut self,
        node_id: String,
        context_id: Option<String>,
        payload: EdgeActionPayload,
    ) -> EdgeActionRecord {
        if self.database.actions.len() >= self.config.limits.max_actions {
            let removable = self
                .database
                .actions
                .iter()
                .filter(|(_, action)| matches!(action.state, ActionState::Applied | ActionState::Failed | ActionState::Expired | ActionState::Cancelled))
                .min_by_key(|(_, action)| action.sequence)
                .map(|(id, _)| id.clone());
            if let Some(id) = removable {
                self.database.actions.remove(&id);
            }
        }
        let id = Uuid::new_v4().to_string();
        let sequence = self.database.next_action_sequence;
        self.database.next_action_sequence = self.database.next_action_sequence.saturating_add(1);
        let now = now_iso();
        let action = EdgeActionRecord {
            id: id.clone(),
            sequence,
            node_id: node_id.clone(),
            context_id: context_id.clone(),
            state: ActionState::Pending,
            payload,
            created_at: now.clone(),
            updated_at: now,
            expires_at: deadline(self.config.flow_control.queue_ttl_secs),
            attempts: 0,
            max_attempts: self.config.flow_control.action_max_attempts,
            next_attempt_at: None,
            command_handle: None,
            command_id: None,
            last_error: None,
        };
        self.database.actions.insert(id.clone(), action.clone());
        self.event(
            "edge_action_queued",
            Some(node_id),
            context_id,
            json!({"action_id":id,"sequence":sequence}),
        );
        action
    }

    fn tick(&mut self) -> Vec<BackendRequest> {
        let now = Utc::now();
        let mut ready_to_standby = Vec::new();
        let mut context_to_quiescent = Vec::new();
        let mut response_wait_expired = Vec::new();
        let mut expired_contexts = Vec::new();
        for context in self.database.contexts.values() {
            if matches!(context.state, ContextState::Ready | ContextState::Quiescent)
                && is_due(context.ready_deadline.as_deref(), &now)
            {
                ready_to_standby.push((context.id.clone(), context.node_id.clone(), context.issi, context.nsapi));
            } else if context.state == ContextState::Ready
                && is_due(context.context_ready_deadline.as_deref(), &now)
            {
                context_to_quiescent.push((context.id.clone(), context.issi, context.nsapi));
            }
            if context.state == ContextState::ResponseWaiting
                && is_due(context.response_wait_deadline.as_deref(), &now)
            {
                response_wait_expired.push((context.id.clone(), context.issi, context.nsapi));
            }
            if matches!(context.state, ContextState::Standby | ContextState::Suspended | ContextState::Quiescent)
                && is_due(context.standby_deadline.as_deref(), &now)
            {
                expired_contexts.push((context.id.clone(), context.node_id.clone(), context.issi, context.nsapi));
            }
        }
        for (id, issi, nsapi) in context_to_quiescent {
            if let Some(context) = self.database.contexts.get_mut(&id) {
                context.state = ContextState::Quiescent;
                context.context_ready_deadline = None;
                context.updated_at = now_iso();
                context.revision = context.revision.saturating_add(1);
            }
            self.event("context_ready_timer_expired", None, Some(id), json!({"issi":issi,"nsapi":nsapi}));
        }
        for (id, issi, nsapi) in response_wait_expired {
            if let Some(context) = self.database.contexts.get_mut(&id) {
                context.state = ContextState::Standby;
                context.response_wait_deadline = None;
                context.standby_deadline = Some(deadline(self.config.packet.standby_timer_secs));
                context.last_error = Some("response wait timer expired".to_string());
                context.updated_at = now_iso();
                context.revision = context.revision.saturating_add(1);
            }
            self.event("response_wait_timer_expired", None, Some(id), json!({"issi":issi,"nsapi":nsapi}));
        }
        for (id, node_id, issi, nsapi) in ready_to_standby {
            if let Some(context) = self.database.contexts.get_mut(&id) {
                context.state = ContextState::Standby;
                context.ready_deadline = None;
                context.context_ready_deadline = None;
                context.standby_deadline = Some(deadline(self.config.packet.standby_timer_secs));
                context.updated_at = now_iso();
            }
            if self.config.packet.mode == MODE_AUTHORITATIVE {
                self.queue_action(
                    node_id,
                    Some(id.clone()),
                    EdgeActionPayload::EndOfData { issi, nsapis: vec![nsapi] },
                );
            }
            self.event("ready_timer_expired", None, Some(id), json!({"issi":issi,"nsapi":nsapi}));
        }
        for (id, node_id, issi, nsapi) in expired_contexts {
            self.database.contexts.remove(&id);
            if self.config.packet.mode == MODE_AUTHORITATIVE {
                self.queue_action(
                    node_id,
                    Some(id.clone()),
                    EdgeActionPayload::Deactivate {
                        issi,
                        nsapi: Some(nsapi),
                        reason: "standby_timer_expired".to_string(),
                    },
                );
            }
            self.event("standby_timer_expired", None, Some(id), json!({"issi":issi,"nsapi":nsapi}));
        }
        let expired_reassemblies = self
            .database
            .reassemblies
            .iter()
            .filter(|(_, assembly)| is_due(Some(&assembly.expires_at), &now))
            .map(|(id, assembly)| (id.clone(), assembly.node_id.clone(), assembly.issi, assembly.nsapi))
            .collect::<Vec<_>>();
        for (id, node_id, issi, nsapi) in expired_reassemblies {
            self.database.reassemblies.remove(&id);
            self.reassembly_failed = self.reassembly_failed.saturating_add(1);
            self.event(
                "reassembly_timeout",
                Some(node_id),
                Some(PdpContextRecord::key(issi, nsapi)),
                json!({"assembly_id":id}),
            );
        }
        let mut settled_actions = Vec::new();
        for action in self.database.actions.values_mut() {
            if matches!(action.state, ActionState::Applied | ActionState::Failed | ActionState::Expired | ActionState::Cancelled) {
                continue;
            }
            if is_due(Some(&action.expires_at), &now) {
                action.state = ActionState::Expired;
                action.updated_at = now_iso();
                action.last_error = Some("action TTL expired".to_string());
                settled_actions.push(action.clone());
                continue;
            }
            if action.state == ActionState::InFlight
                && is_due(action.next_attempt_at.as_deref(), &now)
            {
                if action.attempts >= action.max_attempts {
                    action.state = ActionState::Failed;
                    action.last_error = Some("maximum delivery attempts reached".to_string());
                    settled_actions.push(action.clone());
                } else {
                    action.state = ActionState::Pending;
                }
                action.updated_at = now_iso();
            }
        }
        for action in &settled_actions {
            self.settle_npdu_action(action, false);
        }
        self.touch();
        self.collect_due_backend_requests()
    }

    fn enter_ready(&mut self, issi: u32, nsapis: &[u8]) {
        let now = now_iso();
        for nsapi in nsapis {
            if let Some(context) = self.database.contexts.get_mut(&PdpContextRecord::key(issi, *nsapi)) {
                context.state = ContextState::Ready;
                context.ready_deadline = Some(deadline(self.config.packet.ready_timer_secs));
                context.context_ready_deadline = Some(deadline(self.config.packet.context_ready_secs));
                context.standby_deadline = None;
                context.response_wait_deadline = None;
                context.last_activity_at = now.clone();
                context.updated_at = now.clone();
                context.revision = context.revision.saturating_add(1);
            }
        }
    }

    fn enter_standby(&mut self, issi: u32, nsapis: &[u8]) {
        let now = now_iso();
        for nsapi in nsapis {
            if let Some(context) = self.database.contexts.get_mut(&PdpContextRecord::key(issi, *nsapi)) {
                context.state = ContextState::Standby;
                context.ready_deadline = None;
                context.context_ready_deadline = None;
                context.standby_deadline = Some(deadline(self.config.packet.standby_timer_secs));
                context.response_wait_deadline = None;
                context.last_activity_at = now.clone();
                context.updated_at = now.clone();
                context.revision = context.revision.saturating_add(1);
            }
        }
    }

    fn deactivate_contexts(&mut self, issi: u32, nsapi: Option<u8>, reason: &str) {
        let keys = self
            .database
            .contexts
            .iter()
            .filter(|(_, context)| context.issi == issi && nsapi.map_or(true, |nsapi| context.nsapi == nsapi))
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();
        for key in keys {
            self.database.contexts.remove(&key);
            self.event("context_removed", None, Some(key), json!({"reason":reason}));
        }
        self.database.bearers.retain(|_, bearer| bearer.issi != issi);
    }

    fn remove_contexts_for_subscriber(&mut self, issi: u32, reason: &str) {
        self.deactivate_contexts(issi, None, reason);
    }

    fn mark_node_lost(&mut self, node_id: &str, reason: &str) {
        if let Some(node) = self.database.nodes.get_mut(node_id) {
            node.connected = false;
            node.last_error = Some(reason.to_string());
            node.last_seen = now_iso();
        }
        if !self.config.packet.preserve_context_on_node_loss {
            let keys = self
                .database
                .contexts
                .iter()
                .filter(|(_, context)| context.node_id == node_id)
                .map(|(key, _)| key.clone())
                .collect::<Vec<_>>();
            for key in keys {
                self.database.contexts.remove(&key);
            }
        } else {
            for context in self.database.contexts.values_mut().filter(|context| context.node_id == node_id) {
                context.state = ContextState::Suspended;
                context.available = false;
                context.last_error = Some(reason.to_string());
                context.updated_at = now_iso();
            }
        }
        self.database.bearers.retain(|_, bearer| bearer.node_id != node_id);
        self.event("node_lost", Some(node_id.to_string()), None, json!({"reason":reason}));
    }

    fn upsert_context_from_edge(
        &mut self,
        node_id: &str,
        issi: u32,
        nsapi: u8,
        ipv4: String,
        primary_nsapi: Option<u8>,
        snei: Option<u16>,
        mtu: u16,
        priority: u8,
        state_value: ContextState,
        source: &str,
    ) {
        let key = PdpContextRecord::key(issi, nsapi);
        let now = now_iso();
        let context = self.database.contexts.entry(key.clone()).or_insert_with(|| PdpContextRecord {
            id: key,
            issi,
            nsapi,
            node_id: node_id.to_string(),
            anchor_node_id: node_id.to_string(),
            ipv4: ipv4.clone(),
            primary_nsapi,
            snei,
            mtu,
            priority: priority.min(7),
            state: state_value,
            available: true,
            usage_active: true,
            source: source.to_string(),
            created_at: now.clone(),
            updated_at: now.clone(),
            last_activity_at: now.clone(),
            ready_deadline: None,
            context_ready_deadline: None,
            standby_deadline: Some(deadline(self.config.packet.standby_timer_secs)),
            response_wait_deadline: None,
            packets_up: 0,
            bytes_up: 0,
            packets_down: 0,
            bytes_down: 0,
            dropped_packets: 0,
            queued_packets: 0,
            queued_bytes: 0,
            carrier_num: None,
            logical_ts: None,
            air_ts: None,
            last_error: None,
            revision: 1,
        });
        context.node_id = node_id.to_string();
        context.anchor_node_id = node_id.to_string();
        context.ipv4 = ipv4;
        context.primary_nsapi = primary_nsapi;
        context.snei = snei;
        context.mtu = mtu;
        context.priority = priority.min(7);
        context.state = state_value;
        context.source = source.to_string();
        context.updated_at = now.clone();
        context.last_activity_at = now;
        context.revision = context.revision.saturating_add(1);
    }

    fn allocate_ipv4(&self, requested: Option<&str>) -> Result<String, String> {
        let used = self
            .database
            .contexts
            .values()
            .map(|context| context.ipv4.clone())
            .collect::<BTreeSet<_>>();
        if let Some(requested) = requested {
            let address = requested.parse::<Ipv4Addr>().map_err(|_| "invalid requested IPv4 address".to_string())?;
            if !self.config.address_pool.allow_static {
                return Err("static IPv4 addresses are disabled".to_string());
            }
            let octets = address.octets();
            if [octets[0], octets[1], octets[2]] != self.config.address_pool.network_prefix {
                return Err("requested IPv4 is outside the configured pool".to_string());
            }
            if used.contains(requested) {
                return Err("requested IPv4 is already in use".to_string());
            }
            return Ok(requested.to_string());
        }
        for host in self.config.address_pool.first_host..=self.config.address_pool.last_host {
            let prefix = self.config.address_pool.network_prefix;
            let candidate = Ipv4Addr::new(prefix[0], prefix[1], prefix[2], host).to_string();
            if !used.contains(&candidate) {
                return Ok(candidate);
            }
        }
        Err("IPv4 pool exhausted".to_string())
    }

    fn allocate_snei(&self, issi: u32) -> u16 {
        let base = ((issi % 65_534) + 1) as u16;
        let used = self
            .database
            .contexts
            .values()
            .filter_map(|context| context.snei)
            .collect::<BTreeSet<_>>();
        (0..65_534u32)
            .map(|offset| ((u32::from(base) + offset - 1) % 65_534 + 1) as u16)
            .find(|candidate| !used.contains(candidate))
            .unwrap_or(base)
    }

    fn allocate_handle(&mut self) -> u32 {
        let handle = self.database.next_handle.max(1);
        self.database.next_handle = handle.wrapping_add(1).max(1);
        handle
    }

    fn ensure_node(&mut self, node_id: &str) {
        let now = now_iso();
        self.database.nodes.entry(node_id.to_string()).or_insert_with(|| PacketNodeRecord {
            node_id: node_id.to_string(),
            station_name: node_id.to_string(),
            connected: true,
            stale: false,
            packet_data_capable: true,
            multi_pdch_capable: false,
            mcc: None,
            mnc: None,
            location_area: None,
            last_seen: now,
            last_error: None,
            gateway_running: false,
            interface_name: None,
            gateway_address: None,
            active_contexts: 0,
            active_bearers: 0,
            bearer_capacity: 0,
            traffic_slots_free: 0,
            packets_from_mobile: 0,
            bytes_from_mobile: 0,
            packets_to_mobile: 0,
            bytes_to_mobile: 0,
            dropped_packets: 0,
            io_errors: 0,
        });
    }

    fn event(&mut self, kind: &str, node_id: Option<String>, context_id: Option<String>, detail: Value) {
        let event = PacketEventRecord {
            sequence: self.database.next_event_sequence,
            timestamp: now_iso(),
            kind: kind.to_string(),
            node_id,
            context_id,
            detail,
        };
        self.database.next_event_sequence = self.database.next_event_sequence.saturating_add(1);
        self.database.events.push_back(event);
        while self.database.events.len() > self.config.server.history_limit
            || self.database.events.len() > self.config.limits.max_events
        {
            self.database.events.pop_front();
        }
    }

    fn touch(&mut self) {
        self.database.revision = self.database.revision.saturating_add(1);
    }

    fn persist_logged(&self) {
        if let Err(error) = self.persist() {
            tracing::error!("Packet Core state persistence failed: {error}");
        }
    }

    fn persist(&self) -> std::io::Result<()> {
        if let Some(parent) = self.config.storage.database_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(&self.database)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
        let temp = self.config.storage.database_path.with_extension("json.tmp");
        let mut file = fs::File::create(&temp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        if self.config.storage.database_path.exists() {
            if let Some(parent) = self.config.storage.backup_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let _ = fs::copy(&self.config.storage.database_path, &self.config.storage.backup_path);
        }
        fs::rename(temp, &self.config.storage.database_path)
    }
}

fn read_backup(config: &PacketCoreConfig) -> Result<PacketDatabase, Box<dyn std::error::Error>> {
    let bytes = fs::read(&config.storage.backup_path)?;
    let database = serde_json::from_slice::<PacketDatabase>(&bytes)?;
    if database.schema_version != DATABASE_SCHEMA_VERSION {
        return Err("unsupported Packet Core backup schema".into());
    }
    Ok(database)
}

fn complete_reassembly(assembly: &ReassemblyRecord) -> Option<Vec<u8>> {
    let total_len = assembly.total_len?;
    let mut cursor = 0usize;
    let mut payload = Vec::with_capacity(total_len);
    for (offset, segment) in &assembly.segments {
        if *offset != cursor {
            return None;
        }
        if cursor.saturating_add(segment.len()) > total_len {
            return None;
        }
        payload.extend_from_slice(segment);
        cursor = cursor.saturating_add(segment.len());
    }
    (cursor == total_len).then_some(payload)
}

fn validate_nsapi(nsapi: u8) -> Result<(), String> {
    if (1..=14).contains(&nsapi) {
        Ok(())
    } else {
        Err("NSAPI must be in range 1..14".to_string())
    }
}

fn parse_context_state(state: &str) -> ContextState {
    match state.to_ascii_uppercase().as_str() {
        "READY" => ContextState::Ready,
        "QUIESCENT" => ContextState::Quiescent,
        "SUSPENDED" => ContextState::Suspended,
        "RESPONSE_WAITING" | "RESPONSE-WAITING" => ContextState::ResponseWaiting,
        "ACTIVATING" => ContextState::Activating,
        "DEACTIVATING" => ContextState::Deactivating,
        "FAILED" => ContextState::Failed,
        _ => ContextState::Standby,
    }
}

pub fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn deadline(seconds: u64) -> String {
    (Utc::now() + ChronoDuration::seconds(seconds.min(i64::MAX as u64) as i64))
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn is_due(value: Option<&str>, now: &DateTime<Utc>) -> bool {
    value
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|deadline| deadline.with_timezone(&Utc) <= *now)
        .unwrap_or(false)
}

pub fn decode_hex(value: &str) -> Result<Vec<u8>, String> {
    let filtered = value
        .chars()
        .filter(|character| !character.is_ascii_whitespace() && *character != ':' && *character != '-')
        .collect::<String>();
    if filtered.len() % 2 != 0 {
        return Err("hex payload must contain an even number of digits".to_string());
    }
    (0..filtered.len())
        .step_by(2)
        .map(|offset| u8::from_str_radix(&filtered[offset..offset + 2], 16).map_err(|_| "invalid hex payload".to_string()))
        .collect()
}

pub fn encode_hex(value: &[u8]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reassembly_requires_gap_free_segments() {
        let mut assembly = ReassemblyRecord {
            id: "a".to_string(),
            node_id: "n".to_string(),
            issi: 1,
            nsapi: 1,
            datagram_id: "d".to_string(),
            direction: "uplink".to_string(),
            created_at: now_iso(),
            updated_at: now_iso(),
            expires_at: deadline(30),
            total_len: Some(4),
            received_bytes: 2,
            fragment_count: 1,
            segments: BTreeMap::from([(2, vec![3, 4])]),
            last_error: None,
        };
        assert!(complete_reassembly(&assembly).is_none());
        assembly.segments.insert(0, vec![1, 2]);
        assert_eq!(complete_reassembly(&assembly), Some(vec![1, 2, 3, 4]));
    }

    #[test]
    fn hex_roundtrip() {
        let value = vec![0x00, 0x12, 0xab, 0xff];
        assert_eq!(decode_hex(&encode_hex(&value)).unwrap(), value);
    }
}

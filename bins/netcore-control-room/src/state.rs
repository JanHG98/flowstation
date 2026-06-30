use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex, mpsc};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tetra_core::tetra_entities::TetraEntity;
use tetra_entities::net_control::ControlCommand;
use tetra_entities::net_control_room::{
    ControlCommandAck, ControlCommandEnvelope, ControlResponseEnvelope, ControlRoomNodeCapabilities, ControlRoomNodeHeartbeat,
    ControlRoomNodeHello, ControlRoomToNodeMessage, NodeTelemetryEnvelope, NodeToControlRoomMessage,
};
use tetra_entities::net_telemetry::TelemetryEvent;
use uuid::Uuid;

pub type NodeCommandSender = mpsc::Sender<ControlRoomToNodeMessage>;
pub type UiSender = mpsc::Sender<UiMessage>;

#[derive(Clone)]
pub struct SharedControlRoom {
    inner: Arc<Mutex<ControlRoomState>>,
    node_senders: Arc<Mutex<HashMap<String, NodeCommandSender>>>,
    ui_senders: Arc<Mutex<HashMap<String, UiSender>>>,
}

impl SharedControlRoom {
    pub fn new(history_limit: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ControlRoomState::new(history_limit))),
            node_senders: Arc::new(Mutex::new(HashMap::new())),
            ui_senders: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn snapshot(&self) -> ControlRoomSnapshot {
        self.inner.lock().expect("control room state poisoned").snapshot()
    }

    pub fn recent_events(&self, limit: usize) -> Vec<EventLogEntry> {
        self.inner.lock().expect("control room state poisoned").recent_events(limit)
    }

    pub fn recent_events_filtered(&self, limit: usize, event_type: Option<&str>, quiet: bool) -> Vec<EventLogEntry> {
        self.inner
            .lock()
            .expect("control room state poisoned")
            .recent_events_filtered(limit, event_type, quiet)
    }

    pub fn recent_commands(&self, limit: usize) -> Vec<CommandAuditEntry> {
        self.inner.lock().expect("control room state poisoned").recent_commands(limit)
    }

    pub fn overview(&self) -> ControlRoomOverview {
        self.inner.lock().expect("control room state poisoned").overview()
    }

    pub fn rf_snapshot(&self) -> ControlRoomRfSnapshot {
        self.inner.lock().expect("control room state poisoned").rf_snapshot()
    }

    pub fn health_snapshot(&self) -> ControlRoomHealthSnapshot {
        self.inner.lock().expect("control room state poisoned").health_snapshot()
    }

    pub fn subscribers_snapshot(&self, node_id: Option<&str>, online_only: bool) -> Option<ControlRoomSubscribersSnapshot> {
        self.inner
            .lock()
            .expect("control room state poisoned")
            .subscribers_snapshot(node_id, online_only)
    }

    pub fn groups_snapshot(&self, node_id: Option<&str>) -> Option<ControlRoomGroupsSnapshot> {
        self.inner
            .lock()
            .expect("control room state poisoned")
            .groups_snapshot(node_id)
    }

    pub fn calls_snapshot(&self, node_id: Option<&str>) -> Option<ControlRoomCallsSnapshot> {
        self.inner
            .lock()
            .expect("control room state poisoned")
            .calls_snapshot(node_id)
    }

    pub fn sds_snapshot(&self, node_id: Option<&str>, limit: usize) -> Option<ControlRoomSdsSnapshot> {
        self.inner
            .lock()
            .expect("control room state poisoned")
            .sds_snapshot(node_id, limit)
    }

    pub fn emergencies_snapshot(&self, node_id: Option<&str>, active_only: bool) -> Option<ControlRoomEmergenciesSnapshot> {
        self.inner
            .lock()
            .expect("control room state poisoned")
            .emergencies_snapshot(node_id, active_only)
    }

    pub fn node_detail(&self, node_id: &str) -> Option<ControlRoomNodeDetail> {
        self.inner
            .lock()
            .expect("control room state poisoned")
            .node_detail(node_id)
    }

    pub fn node_summary(&self, node_id: &str) -> Option<NodeOverview> {
        self.inner.lock().expect("control room state poisoned").node_summary(node_id)
    }

    pub fn node_exists(&self, node_id: &str) -> bool {
        self.inner.lock().expect("control room state poisoned").nodes.contains_key(node_id)
    }

    pub fn handle_node_message(&self, message: NodeToControlRoomMessage) -> Option<String> {
        let mut state = self.inner.lock().expect("control room state poisoned");
        let node_id = state.apply_node_message(&message);
        let snapshot = state.snapshot();
        drop(state);

        self.broadcast(UiMessage::NodeMessage { message });
        self.broadcast(UiMessage::StateSnapshot { snapshot });
        node_id
    }

    pub fn register_node_sender(&self, node_id: String, tx: NodeCommandSender) {
        self.node_senders
            .lock()
            .expect("node sender map poisoned")
            .insert(node_id.clone(), tx);
        self.inner
            .lock()
            .expect("control room state poisoned")
            .mark_node_connected_transport(&node_id);
        self.broadcast_state();
    }

    pub fn unregister_node_sender(&self, node_id: &str) {
        self.node_senders
            .lock()
            .expect("node sender map poisoned")
            .remove(node_id);
        self.inner
            .lock()
            .expect("control room state poisoned")
            .mark_node_disconnected(node_id);
        self.broadcast_state();
    }

    pub fn register_ui(&self) -> (String, mpsc::Receiver<UiMessage>) {
        let (tx, rx) = mpsc::channel();
        let id = Uuid::new_v4().to_string();
        self.ui_senders
            .lock()
            .expect("ui sender map poisoned")
            .insert(id.clone(), tx.clone());
        let _ = tx.send(UiMessage::StateSnapshot { snapshot: self.snapshot() });
        (id, rx)
    }

    pub fn unregister_ui(&self, ui_id: &str) {
        self.ui_senders
            .lock()
            .expect("ui sender map poisoned")
            .remove(ui_id);
    }

    pub fn submit_command(&self, mut envelope: ControlCommandEnvelope) -> Result<QueuedCommand, String> {
        if envelope.command_id.trim().is_empty() {
            envelope.command_id = Uuid::new_v4().to_string();
        }
        if envelope.issued_at.trim().is_empty() {
            envelope.issued_at = now_iso();
        }

        let tx = {
            let senders = self.node_senders.lock().map_err(|_| "node sender map poisoned".to_string())?;
            senders.get(&envelope.target_node_id).cloned()
        }
        .ok_or_else(|| format!("node '{}' is not connected", envelope.target_node_id))?;

        tx.send(ControlRoomToNodeMessage::Command { envelope: envelope.clone() })
            .map_err(|e| format!("failed to queue command for node '{}': {}", envelope.target_node_id, e))?;

        let queued = QueuedCommand {
            command_id: envelope.command_id.clone(),
            target_node_id: envelope.target_node_id.clone(),
            status: "queued".to_string(),
            timestamp: now_iso(),
        };

        let mut state = self.inner.lock().expect("control room state poisoned");
        state.record_command_queued(&envelope);
        let snapshot = state.snapshot();
        drop(state);

        self.broadcast(UiMessage::CommandQueued { queued: queued.clone() });
        self.broadcast(UiMessage::StateSnapshot { snapshot });
        Ok(queued)
    }

    pub fn make_envelope(&self, target_node_id: String, operator_id: Option<String>, command: ControlCommand) -> ControlCommandEnvelope {
        ControlCommandEnvelope {
            command_id: Uuid::new_v4().to_string(),
            target_node_id,
            operator_id,
            issued_at: now_iso(),
            command,
        }
    }

    fn broadcast_state(&self) {
        self.broadcast(UiMessage::StateSnapshot { snapshot: self.snapshot() });
    }

    fn broadcast(&self, msg: UiMessage) {
        let mut dead = Vec::new();
        {
            let senders = self.ui_senders.lock().expect("ui sender map poisoned");
            for (id, tx) in senders.iter() {
                if tx.send(msg.clone()).is_err() {
                    dead.push(id.clone());
                }
            }
        }
        if !dead.is_empty() {
            let mut senders = self.ui_senders.lock().expect("ui sender map poisoned");
            for id in dead {
                senders.remove(&id);
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UiMessage {
    StateSnapshot { snapshot: ControlRoomSnapshot },
    NodeMessage { message: NodeToControlRoomMessage },
    CommandQueued { queued: QueuedCommand },
    Error { message: String, timestamp: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedCommand {
    pub command_id: String,
    pub target_node_id: String,
    pub status: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlRoomSnapshot {
    pub started_at: String,
    pub now: String,
    pub nodes_connected: usize,
    pub nodes: Vec<NodeState>,
    pub recent_events: Vec<EventLogEntry>,
    pub recent_commands: Vec<CommandAuditEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlRoomOverview {
    pub started_at: String,
    pub now: String,
    pub nodes_connected: usize,
    pub node_count: usize,
    pub subscribers_total: usize,
    pub subscribers_online: usize,
    pub groups_total: usize,
    pub active_calls_total: usize,
    pub emergencies_active: usize,
    pub nodes: Vec<NodeOverview>,
    pub recent_commands: Vec<CommandSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeOverview {
    pub node_id: String,
    pub station_name: Option<String>,
    pub site: Option<String>,
    pub connected: bool,
    pub transport_connected: bool,
    pub last_seen: Option<String>,
    pub stack_version: Option<String>,
    pub mcc: Option<u16>,
    pub mnc: Option<u16>,
    pub location_area: Option<u16>,
    pub main_carrier: Option<u16>,
    pub secondary_carrier: Option<u16>,
    pub colour_code: Option<u8>,
    pub system_code: Option<u8>,
    pub dual_carrier: bool,
    pub telemetry_count: u64,
    pub heartbeat_count: u64,
    pub command_ack_count: u64,
    pub control_response_count: u64,
    pub subscribers_total: usize,
    pub subscribers_online: usize,
    pub groups_total: usize,
    pub active_calls_total: usize,
    pub emergencies_active: usize,
    pub sds_log_count: usize,
    pub brew_connected: Option<bool>,
    pub health_overall: Option<String>,
    pub rf_peak_dbfs: Option<f64>,
    pub rf_rms_dbfs: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandSummary {
    pub command_id: String,
    pub target_node_id: String,
    pub operator_id: Option<String>,
    pub issued_at: String,
    pub updated_at: String,
    pub status: String,
    pub message: Option<String>,
}

impl From<&CommandAuditEntry> for CommandSummary {
    fn from(entry: &CommandAuditEntry) -> Self {
        Self {
            command_id: entry.command_id.clone(),
            target_node_id: entry.target_node_id.clone(),
            operator_id: entry.operator_id.clone(),
            issued_at: entry.issued_at.clone(),
            updated_at: entry.updated_at.clone(),
            status: entry.status.clone(),
            message: entry.message.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlRoomRfSnapshot {
    pub now: String,
    pub nodes: Vec<NodeRfSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeRfSnapshot {
    pub node_id: String,
    pub station_name: Option<String>,
    pub connected: bool,
    pub last_seen: Option<String>,
    pub rf_quality: Option<Value>,
    pub sdr_health: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlRoomHealthSnapshot {
    pub now: String,
    pub nodes: Vec<NodeHealthSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeHealthSnapshot {
    pub node_id: String,
    pub station_name: Option<String>,
    pub connected: bool,
    pub transport_connected: bool,
    pub last_seen: Option<String>,
    pub health: Option<Value>,
    pub sdr_health: Option<Value>,
    pub sys_health: Option<Value>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlRoomSubscribersSnapshot {
    pub now: String,
    pub node_filter: Option<String>,
    pub count: usize,
    pub online_count: usize,
    pub subscribers: Vec<SubscriberDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriberDetail {
    pub node_id: String,
    pub station_name: Option<String>,
    pub issi: u32,
    pub online: bool,
    pub groups: Vec<u32>,
    pub rssi_dbfs: Option<f32>,
    pub energy_saving_mode: Option<u8>,
    pub emergency: bool,
    pub remote_source: Option<String>,
    pub last_seen: Option<String>,
    pub last_event: Option<String>,
    pub timeout_drop_count: u64,
    pub active_call_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlRoomGroupsSnapshot {
    pub now: String,
    pub node_filter: Option<String>,
    pub count: usize,
    pub groups: Vec<GroupDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupDetail {
    pub node_id: String,
    pub station_name: Option<String>,
    pub gssi: u32,
    pub members: Vec<u32>,
    pub members_online: usize,
    pub active_call_id: Option<u16>,
    pub active_call_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlRoomCallsSnapshot {
    pub now: String,
    pub node_filter: Option<String>,
    pub count: usize,
    pub calls: Vec<CallDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallDetail {
    pub node_id: String,
    pub station_name: Option<String>,
    pub key: String,
    pub call_kind: String,
    pub call_id: u16,
    pub gssi: Option<u32>,
    pub caller_issi: Option<u32>,
    pub speaker_issi: Option<u32>,
    pub calling_issi: Option<u32>,
    pub called_issi: Option<u32>,
    pub simplex: Option<bool>,
    pub carrier_num: u16,
    pub ts: u8,
    pub priority: u8,
    pub source: String,
    pub started_at: String,
    pub last_activity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlRoomSdsSnapshot {
    pub now: String,
    pub node_filter: Option<String>,
    pub count: usize,
    pub sds: Vec<SdsDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdsDetail {
    pub node_id: String,
    pub station_name: Option<String>,
    pub timestamp: String,
    pub direction: String,
    pub source_issi: u32,
    pub dest_issi: u32,
    pub is_group: bool,
    pub protocol_id: u8,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlRoomEmergenciesSnapshot {
    pub now: String,
    pub node_filter: Option<String>,
    pub count: usize,
    pub active_count: usize,
    pub emergencies: Vec<EmergencyDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergencyDetail {
    pub node_id: String,
    pub station_name: Option<String>,
    pub source_issi: u32,
    pub dest_ssi: u32,
    pub active: bool,
    pub raised_at: String,
    pub cleared_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlRoomNodeDetail {
    pub now: String,
    pub node: NodeOverview,
    pub subscribers: Vec<SubscriberDetail>,
    pub groups: Vec<GroupDetail>,
    pub active_calls: Vec<CallDetail>,
    pub emergencies: Vec<EmergencyDetail>,
    pub sds_log: Vec<SdsDetail>,
    pub brew: HashMap<String, BrewState>,
    pub timeslot_activity: HashMap<String, String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeState {
    pub node_id: String,
    pub station_name: Option<String>,
    pub site: Option<String>,
    pub connected: bool,
    pub transport_connected: bool,
    pub protocol_version: Option<String>,
    pub started_at: Option<String>,
    pub stack_version: Option<String>,
    pub mcc: Option<u16>,
    pub mnc: Option<u16>,
    pub location_area: Option<u16>,
    pub main_carrier: Option<u16>,
    pub secondary_carrier: Option<u16>,
    pub colour_code: Option<u8>,
    pub system_code: Option<u8>,
    pub capabilities: Option<ControlRoomNodeCapabilities>,
    pub last_seen: Option<String>,
    pub last_seq: u64,
    pub telemetry_count: u64,
    pub heartbeat_count: u64,
    pub command_ack_count: u64,
    pub control_response_count: u64,
    pub subscribers: HashMap<u32, SubscriberState>,
    pub groups: HashMap<u32, GroupState>,
    pub active_calls: HashMap<String, CallState>,
    pub emergencies: HashMap<u32, EmergencyState>,
    pub sds_log: VecDeque<SdsLogEntry>,
    pub brew: HashMap<String, BrewState>,
    pub timeslot_activity: HashMap<String, String>,
    pub rf_quality: Option<Value>,
    pub sdr_health: Option<Value>,
    pub sys_health: Option<Value>,
    pub health: Option<Value>,
    pub errors: VecDeque<String>,
}

impl NodeState {
    fn overview(&self) -> NodeOverview {
        NodeOverview {
            node_id: self.node_id.clone(),
            station_name: self.station_name.clone(),
            site: self.site.clone(),
            connected: self.connected,
            transport_connected: self.transport_connected,
            last_seen: self.last_seen.clone(),
            stack_version: self.stack_version.clone(),
            mcc: self.mcc,
            mnc: self.mnc,
            location_area: self.location_area,
            main_carrier: self.main_carrier,
            secondary_carrier: self.secondary_carrier,
            colour_code: self.colour_code,
            system_code: self.system_code,
            dual_carrier: self.secondary_carrier.is_some(),
            telemetry_count: self.telemetry_count,
            heartbeat_count: self.heartbeat_count,
            command_ack_count: self.command_ack_count,
            control_response_count: self.control_response_count,
            subscribers_total: self.subscribers.len(),
            subscribers_online: self.subscribers.values().filter(|s| s.online).count(),
            groups_total: self.groups.len(),
            active_calls_total: self.active_calls.len(),
            emergencies_active: self.active_emergencies_count(),
            sds_log_count: self.sds_log.len(),
            brew_connected: self.brew.get("default").map(|brew| brew.connected),
            health_overall: extract_health_overall(&self.health),
            rf_peak_dbfs: extract_f64_path(&self.rf_quality, &["peak_dbfs"]),
            rf_rms_dbfs: extract_f64_path(&self.rf_quality, &["rms_dbfs"]),
        }
    }

    fn rf_snapshot(&self) -> NodeRfSnapshot {
        NodeRfSnapshot {
            node_id: self.node_id.clone(),
            station_name: self.station_name.clone(),
            connected: self.connected || self.transport_connected,
            last_seen: self.last_seen.clone(),
            rf_quality: self.rf_quality.clone(),
            sdr_health: self.sdr_health.clone(),
        }
    }

    fn health_snapshot(&self) -> NodeHealthSnapshot {
        NodeHealthSnapshot {
            node_id: self.node_id.clone(),
            station_name: self.station_name.clone(),
            connected: self.connected,
            transport_connected: self.transport_connected,
            last_seen: self.last_seen.clone(),
            health: self.health.clone(),
            sdr_health: self.sdr_health.clone(),
            sys_health: self.sys_health.clone(),
            errors: self.errors.iter().cloned().collect(),
        }
    }

    fn active_emergencies_count(&self) -> usize {
        self.emergencies.values().filter(|e| e.active).count()
    }
}

impl NodeState {
    fn new(node_id: String) -> Self {
        Self {
            node_id,
            station_name: None,
            site: None,
            connected: false,
            transport_connected: false,
            protocol_version: None,
            started_at: None,
            stack_version: None,
            mcc: None,
            mnc: None,
            location_area: None,
            main_carrier: None,
            secondary_carrier: None,
            colour_code: None,
            system_code: None,
            capabilities: None,
            last_seen: None,
            last_seq: 0,
            telemetry_count: 0,
            heartbeat_count: 0,
            command_ack_count: 0,
            control_response_count: 0,
            subscribers: HashMap::new(),
            groups: HashMap::new(),
            active_calls: HashMap::new(),
            emergencies: HashMap::new(),
            sds_log: VecDeque::new(),
            brew: HashMap::new(),
            timeslot_activity: HashMap::new(),
            rf_quality: None,
            sdr_health: None,
            sys_health: None,
            health: None,
            errors: VecDeque::new(),
        }
    }

    fn apply_hello(&mut self, hello: &ControlRoomNodeHello) {
        self.connected = true;
        self.transport_connected = true;
        self.protocol_version = Some(hello.protocol_version.clone());
        self.started_at = Some(hello.started_at.clone());
        self.station_name = Some(hello.node.station_name.clone());
        self.site = hello.node.site.clone();
        self.stack_version = Some(hello.node.stack_version.clone());
        self.mcc = Some(hello.node.mcc);
        self.mnc = Some(hello.node.mnc);
        self.location_area = Some(hello.node.location_area);
        self.main_carrier = Some(hello.node.main_carrier);
        self.secondary_carrier = hello.node.secondary_carrier;
        self.colour_code = Some(hello.node.colour_code);
        self.system_code = Some(hello.node.system_code);
        self.capabilities = Some(hello.capabilities.clone());
        self.last_seen = Some(now_iso());
    }

    fn apply_heartbeat(&mut self, heartbeat: &ControlRoomNodeHeartbeat) {
        self.connected = heartbeat.connected;
        self.transport_connected = true;
        self.heartbeat_count = self.heartbeat_count.wrapping_add(1);
        self.last_seq = heartbeat.seq;
        self.last_seen = Some(heartbeat.timestamp.clone());
    }

    fn apply_telemetry(&mut self, envelope: &NodeTelemetryEnvelope) {
        self.connected = true;
        self.transport_connected = true;
        self.telemetry_count = self.telemetry_count.wrapping_add(1);
        self.last_seq = envelope.seq;
        self.last_seen = Some(envelope.timestamp.clone());
        self.apply_event(&envelope.timestamp, &envelope.event);
    }

    fn apply_event(&mut self, timestamp: &str, event: &TelemetryEvent) {
        match event {
            TelemetryEvent::MsRegistration { issi } => {
                let ms = self.subscriber_mut(*issi);
                ms.online = true;
                ms.last_seen = Some(timestamp.to_string());
                ms.last_event = Some("registration".to_string());
            }
            TelemetryEvent::MsDeregistration { issi } => {
                let ms = self.subscriber_mut(*issi);
                ms.online = false;
                ms.last_seen = Some(timestamp.to_string());
                ms.last_event = Some("deregistration".to_string());
                for group in self.groups.values_mut() {
                    group.members.remove(issi);
                }
            }
            TelemetryEvent::MsTimeoutDrop { issi } => {
                let ms = self.subscriber_mut(*issi);
                ms.online = false;
                ms.timeout_drop_count = ms.timeout_drop_count.wrapping_add(1);
                ms.last_seen = Some(timestamp.to_string());
                ms.last_event = Some("timeout_drop".to_string());
                for group in self.groups.values_mut() {
                    group.members.remove(issi);
                }
            }
            TelemetryEvent::MsGroupAttach { issi, gssis } | TelemetryEvent::MsGroupsSnapshot { issi, gssis } => {
                let snapshot = matches!(event, TelemetryEvent::MsGroupsSnapshot { .. });
                if snapshot {
                    let existing: Vec<u32> = self.groups.keys().copied().collect();
                    for gssi in existing {
                        if let Some(group) = self.groups.get_mut(&gssi) {
                            group.members.remove(issi);
                        }
                    }
                    self.subscriber_mut(*issi).groups.clear();
                }

                for gssi in gssis {
                    self.subscriber_mut(*issi).groups.insert(*gssi);
                    self.group_mut(*gssi).members.insert(*issi);
                }
                let ms = self.subscriber_mut(*issi);
                ms.online = true;
                ms.last_seen = Some(timestamp.to_string());
                ms.last_event = Some(if snapshot { "groups_snapshot" } else { "group_attach" }.to_string());
            }
            TelemetryEvent::MsGroupDetach { issi, gssis } => {
                for gssi in gssis {
                    self.subscriber_mut(*issi).groups.remove(gssi);
                    if let Some(group) = self.groups.get_mut(gssi) {
                        group.members.remove(issi);
                    }
                }
                let ms = self.subscriber_mut(*issi);
                ms.last_seen = Some(timestamp.to_string());
                ms.last_event = Some("group_detach".to_string());
            }
            TelemetryEvent::MsRssi { issi, rssi_dbfs } => {
                let ms = self.subscriber_mut(*issi);
                ms.rssi_dbfs = Some(*rssi_dbfs);
                ms.last_seen = Some(timestamp.to_string());
                ms.last_event = Some("rssi".to_string());
            }
            TelemetryEvent::MsEnergySaving { issi, mode } => {
                let ms = self.subscriber_mut(*issi);
                ms.energy_saving_mode = Some(*mode);
                ms.last_seen = Some(timestamp.to_string());
                ms.last_event = Some("energy_saving".to_string());
            }
            TelemetryEvent::GroupCallStarted {
                call_id,
                gssi,
                caller_issi,
                ts,
                carrier_num,
                priority,
                source,
            } => {
                self.group_mut(*gssi).active_call_id = Some(*call_id);
                self.subscriber_mut(*caller_issi).last_seen = Some(timestamp.to_string());
                let key = format!("group:{}", call_id);
                self.active_calls.insert(
                    key,
                    CallState::Group {
                        call_id: *call_id,
                        gssi: *gssi,
                        caller_issi: *caller_issi,
                        speaker_issi: Some(*caller_issi),
                        carrier_num: *carrier_num,
                        ts: *ts,
                        priority: *priority,
                        source: source.clone(),
                        started_at: timestamp.to_string(),
                        last_activity: timestamp.to_string(),
                    },
                );
            }
            TelemetryEvent::GroupCallSpeakerChanged {
                call_id,
                gssi,
                speaker_issi,
                source,
            } => {
                self.subscriber_mut(*speaker_issi).last_seen = Some(timestamp.to_string());
                let key = format!("group:{}", call_id);
                if let Some(CallState::Group {
                    speaker_issi: current_speaker,
                    last_activity,
                    source: current_source,
                    ..
                }) = self.active_calls.get_mut(&key)
                {
                    *current_speaker = Some(*speaker_issi);
                    *last_activity = timestamp.to_string();
                    *current_source = source.clone();
                } else {
                    self.active_calls.insert(
                        key,
                        CallState::Group {
                            call_id: *call_id,
                            gssi: *gssi,
                            caller_issi: *speaker_issi,
                            speaker_issi: Some(*speaker_issi),
                            carrier_num: 0,
                            ts: 0,
                            priority: 0,
                            source: source.clone(),
                            started_at: timestamp.to_string(),
                            last_activity: timestamp.to_string(),
                        },
                    );
                }
            }
            TelemetryEvent::GroupCallEnded { call_id, gssi } => {
                self.active_calls.remove(&format!("group:{}", call_id));
                if let Some(group) = self.groups.get_mut(gssi) {
                    group.active_call_id = None;
                }
            }
            TelemetryEvent::IndividualCallStarted {
                call_id,
                calling_issi,
                called_issi,
                simplex,
                ts,
                carrier_num,
                priority,
                source,
            } => {
                self.subscriber_mut(*calling_issi).last_seen = Some(timestamp.to_string());
                self.subscriber_mut(*called_issi).last_seen = Some(timestamp.to_string());
                self.active_calls.insert(
                    format!("individual:{}", call_id),
                    CallState::Individual {
                        call_id: *call_id,
                        calling_issi: *calling_issi,
                        called_issi: *called_issi,
                        simplex: *simplex,
                        carrier_num: *carrier_num,
                        ts: *ts,
                        priority: *priority,
                        source: source.clone(),
                        started_at: timestamp.to_string(),
                        last_activity: timestamp.to_string(),
                    },
                );
            }
            TelemetryEvent::IndividualCallEnded { call_id } => {
                self.active_calls.remove(&format!("individual:{}", call_id));
            }
            TelemetryEvent::SdsActivity { source_issi, dest_issi, source } => {
                self.subscriber_mut(*source_issi).last_seen = Some(timestamp.to_string());
                self.subscriber_mut(*dest_issi).last_seen = Some(timestamp.to_string());
                let id = format!("sds:{}->{}", source_issi, dest_issi);
                self.timeslot_activity.insert(id, format!("{} via {}", timestamp, source));
            }
            TelemetryEvent::SdsLog {
                direction,
                source_issi,
                dest_issi,
                is_group,
                protocol_id,
                text,
            } => {
                self.push_sds(SdsLogEntry {
                    timestamp: timestamp.to_string(),
                    direction: direction.clone(),
                    source_issi: *source_issi,
                    dest_issi: *dest_issi,
                    is_group: *is_group,
                    protocol_id: *protocol_id,
                    text: text.clone(),
                });
            }
            TelemetryEvent::TsVoiceActivity { carrier_num, ts } => {
                self.timeslot_activity
                    .insert(format!("c{}:ts{}", carrier_num, ts), timestamp.to_string());
            }
            TelemetryEvent::TxQuality { .. } => {
                self.rf_quality = Some(event_for_log(event));
            }
            TelemetryEvent::TxVisual {
                sample_rate,
                center_freq_hz,
                rms_dbfs,
                peak_dbfs,
                spectrum_db_tenths,
                constellation_iq,
            } => {
                self.rf_quality = Some(json!({
                    "type": "tx_visual",
                    "sample_rate": sample_rate,
                    "center_freq_hz": center_freq_hz,
                    "rms_dbfs": rms_dbfs,
                    "peak_dbfs": peak_dbfs,
                    "spectrum_bins": spectrum_db_tenths.len(),
                    "constellation_points": constellation_iq.len() / 2,
                    "timestamp": timestamp,
                }));
            }
            TelemetryEvent::SdrHealth { .. } => {
                self.sdr_health = Some(event_for_log(event));
            }
            TelemetryEvent::SysHealth { .. } => {
                self.sys_health = Some(event_for_log(event));
            }
            TelemetryEvent::HealthSnapshot(_) => {
                self.health = Some(event_for_log(event));
            }
            TelemetryEvent::BrewConnected { connected, server_version } => {
                self.brew.insert(
                    "default".to_string(),
                    BrewState {
                        connected: *connected,
                        server_version: *server_version,
                        last_seen: timestamp.to_string(),
                    },
                );
            }
            TelemetryEvent::EmergencyAlarm { source_issi, dest_ssi } => {
                self.emergencies.insert(
                    *source_issi,
                    EmergencyState {
                        source_issi: *source_issi,
                        dest_ssi: *dest_ssi,
                        active: true,
                        raised_at: timestamp.to_string(),
                        cleared_at: None,
                    },
                );
                let ms = self.subscriber_mut(*source_issi);
                ms.emergency = true;
                ms.last_seen = Some(timestamp.to_string());
            }
            TelemetryEvent::EmergencyCancel { source_issi } => {
                if let Some(emergency) = self.emergencies.get_mut(source_issi) {
                    emergency.active = false;
                    emergency.cleared_at = Some(timestamp.to_string());
                }
                let ms = self.subscriber_mut(*source_issi);
                ms.emergency = false;
                ms.last_seen = Some(timestamp.to_string());
            }
            TelemetryEvent::BrewSubscriberRegistered { issi, source } => {
                let ms = self.subscriber_mut(*issi);
                ms.online = true;
                ms.remote_source = Some(source.clone());
                ms.last_seen = Some(timestamp.to_string());
                ms.last_event = Some("brew_registered".to_string());
            }
            TelemetryEvent::BrewSubscriberDeregistered { issi, source } => {
                let ms = self.subscriber_mut(*issi);
                ms.online = false;
                ms.remote_source = Some(source.clone());
                ms.last_seen = Some(timestamp.to_string());
                ms.last_event = Some("brew_deregistered".to_string());
            }
            TelemetryEvent::DapnetLog { .. } | TelemetryEvent::MeshcomMessageLog { .. } | TelemetryEvent::MeshcomNodeUpdate { .. } => {}
        }
    }

    fn subscriber_mut(&mut self, issi: u32) -> &mut SubscriberState {
        self.subscribers.entry(issi).or_insert_with(|| SubscriberState::new(issi))
    }

    fn group_mut(&mut self, gssi: u32) -> &mut GroupState {
        self.groups.entry(gssi).or_insert_with(|| GroupState::new(gssi))
    }

    fn push_sds(&mut self, entry: SdsLogEntry) {
        self.sds_log.push_back(entry);
        while self.sds_log.len() > 200 {
            self.sds_log.pop_front();
        }
    }

    fn push_error(&mut self, message: String) {
        self.errors.push_back(message);
        while self.errors.len() > 50 {
            self.errors.pop_front();
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriberState {
    pub issi: u32,
    pub online: bool,
    pub groups: HashSet<u32>,
    pub rssi_dbfs: Option<f32>,
    pub energy_saving_mode: Option<u8>,
    pub emergency: bool,
    pub remote_source: Option<String>,
    pub last_seen: Option<String>,
    pub last_event: Option<String>,
    pub timeout_drop_count: u64,
}

impl SubscriberState {
    fn new(issi: u32) -> Self {
        Self {
            issi,
            online: false,
            groups: HashSet::new(),
            rssi_dbfs: None,
            energy_saving_mode: None,
            emergency: false,
            remote_source: None,
            last_seen: None,
            last_event: None,
            timeout_drop_count: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupState {
    pub gssi: u32,
    pub members: HashSet<u32>,
    pub active_call_id: Option<u16>,
}

impl GroupState {
    fn new(gssi: u32) -> Self {
        Self {
            gssi,
            members: HashSet::new(),
            active_call_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CallState {
    Group {
        call_id: u16,
        gssi: u32,
        caller_issi: u32,
        speaker_issi: Option<u32>,
        carrier_num: u16,
        ts: u8,
        priority: u8,
        source: String,
        started_at: String,
        last_activity: String,
    },
    Individual {
        call_id: u16,
        calling_issi: u32,
        called_issi: u32,
        simplex: bool,
        carrier_num: u16,
        ts: u8,
        priority: u8,
        source: String,
        started_at: String,
        last_activity: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergencyState {
    pub source_issi: u32,
    pub dest_ssi: u32,
    pub active: bool,
    pub raised_at: String,
    pub cleared_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdsLogEntry {
    pub timestamp: String,
    pub direction: String,
    pub source_issi: u32,
    pub dest_issi: u32,
    pub is_group: bool,
    pub protocol_id: u8,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrewState {
    pub connected: bool,
    pub server_version: u8,
    pub last_seen: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventLogEntry {
    pub timestamp: String,
    pub node_id: String,
    pub seq: Option<u64>,
    pub event_type: String,
    pub event: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandAuditEntry {
    pub command_id: String,
    pub target_node_id: String,
    pub operator_id: Option<String>,
    pub issued_at: String,
    pub updated_at: String,
    pub status: String,
    pub target_entity: Option<TetraEntity>,
    pub message: Option<String>,
    pub command: Value,
    pub responses: Vec<Value>,
}

struct ControlRoomState {
    started_at: String,
    history_limit: usize,
    nodes: HashMap<String, NodeState>,
    recent_events: VecDeque<EventLogEntry>,
    recent_commands: VecDeque<CommandAuditEntry>,
}

impl ControlRoomState {
    fn new(history_limit: usize) -> Self {
        Self {
            started_at: now_iso(),
            history_limit,
            nodes: HashMap::new(),
            recent_events: VecDeque::new(),
            recent_commands: VecDeque::new(),
        }
    }

    fn snapshot(&self) -> ControlRoomSnapshot {
        let nodes: Vec<NodeState> = self.nodes.values().cloned().collect();
        let nodes_connected = nodes.iter().filter(|n| n.connected || n.transport_connected).count();
        ControlRoomSnapshot {
            started_at: self.started_at.clone(),
            now: now_iso(),
            nodes_connected,
            nodes,
            recent_events: self.recent_events.iter().rev().take(100).cloned().collect(),
            recent_commands: self.recent_commands.iter().rev().take(100).cloned().collect(),
        }
    }

    fn recent_events(&self, limit: usize) -> Vec<EventLogEntry> {
        self.recent_events.iter().rev().take(limit).cloned().collect()
    }

    fn recent_events_filtered(&self, limit: usize, event_type: Option<&str>, quiet: bool) -> Vec<EventLogEntry> {
        self.recent_events
            .iter()
            .rev()
            .filter(|entry| event_type.map(|wanted| entry.event_type == wanted).unwrap_or(true))
            .filter(|entry| !quiet || !is_noisy_event_type(&entry.event_type))
            .take(limit)
            .cloned()
            .collect()
    }

    fn recent_commands(&self, limit: usize) -> Vec<CommandAuditEntry> {
        self.recent_commands.iter().rev().take(limit).cloned().collect()
    }

    fn overview(&self) -> ControlRoomOverview {
        let nodes: Vec<NodeOverview> = self.nodes.values().map(NodeState::overview).collect();
        ControlRoomOverview {
            started_at: self.started_at.clone(),
            now: now_iso(),
            nodes_connected: nodes.iter().filter(|n| n.connected || n.transport_connected).count(),
            node_count: nodes.len(),
            subscribers_total: nodes.iter().map(|n| n.subscribers_total).sum(),
            subscribers_online: nodes.iter().map(|n| n.subscribers_online).sum(),
            groups_total: nodes.iter().map(|n| n.groups_total).sum(),
            active_calls_total: nodes.iter().map(|n| n.active_calls_total).sum(),
            emergencies_active: nodes.iter().map(|n| n.emergencies_active).sum(),
            nodes,
            recent_commands: self.recent_commands.iter().rev().take(20).map(|cmd| CommandSummary::from(cmd)).collect(),
        }
    }

    fn rf_snapshot(&self) -> ControlRoomRfSnapshot {
        ControlRoomRfSnapshot {
            now: now_iso(),
            nodes: self.nodes.values().map(NodeState::rf_snapshot).collect(),
        }
    }

    fn health_snapshot(&self) -> ControlRoomHealthSnapshot {
        ControlRoomHealthSnapshot {
            now: now_iso(),
            nodes: self.nodes.values().map(NodeState::health_snapshot).collect(),
        }
    }

    fn subscribers_snapshot(&self, node_filter: Option<&str>, online_only: bool) -> Option<ControlRoomSubscribersSnapshot> {
        let nodes = self.selected_nodes(node_filter)?;
        let mut subscribers = Vec::new();
        for node in nodes {
            for subscriber in node.subscribers.values() {
                if online_only && !subscriber.online {
                    continue;
                }
                subscribers.push(subscriber_detail(node, subscriber));
            }
        }
        subscribers.sort_by(|a, b| a.node_id.cmp(&b.node_id).then(a.issi.cmp(&b.issi)));
        let online_count = subscribers.iter().filter(|s| s.online).count();
        Some(ControlRoomSubscribersSnapshot {
            now: now_iso(),
            node_filter: node_filter.map(ToString::to_string),
            count: subscribers.len(),
            online_count,
            subscribers,
        })
    }

    fn groups_snapshot(&self, node_filter: Option<&str>) -> Option<ControlRoomGroupsSnapshot> {
        let nodes = self.selected_nodes(node_filter)?;
        let mut groups = Vec::new();
        for node in nodes {
            for group in node.groups.values() {
                groups.push(group_detail(node, group));
            }
        }
        groups.sort_by(|a, b| a.node_id.cmp(&b.node_id).then(a.gssi.cmp(&b.gssi)));
        Some(ControlRoomGroupsSnapshot {
            now: now_iso(),
            node_filter: node_filter.map(ToString::to_string),
            count: groups.len(),
            groups,
        })
    }

    fn calls_snapshot(&self, node_filter: Option<&str>) -> Option<ControlRoomCallsSnapshot> {
        let nodes = self.selected_nodes(node_filter)?;
        let mut calls = Vec::new();
        for node in nodes {
            for (key, call) in &node.active_calls {
                calls.push(call_detail(node, key, call));
            }
        }
        calls.sort_by(|a, b| a.node_id.cmp(&b.node_id).then(a.started_at.cmp(&b.started_at)).then(a.key.cmp(&b.key)));
        Some(ControlRoomCallsSnapshot {
            now: now_iso(),
            node_filter: node_filter.map(ToString::to_string),
            count: calls.len(),
            calls,
        })
    }

    fn sds_snapshot(&self, node_filter: Option<&str>, limit: usize) -> Option<ControlRoomSdsSnapshot> {
        let nodes = self.selected_nodes(node_filter)?;
        let mut sds = Vec::new();
        for node in nodes {
            for entry in node.sds_log.iter().rev().take(limit) {
                sds.push(sds_detail(node, entry));
            }
        }
        sds.sort_by(|a, b| b.timestamp.cmp(&a.timestamp).then(a.node_id.cmp(&b.node_id)));
        sds.truncate(limit);
        Some(ControlRoomSdsSnapshot {
            now: now_iso(),
            node_filter: node_filter.map(ToString::to_string),
            count: sds.len(),
            sds,
        })
    }

    fn emergencies_snapshot(&self, node_filter: Option<&str>, active_only: bool) -> Option<ControlRoomEmergenciesSnapshot> {
        let nodes = self.selected_nodes(node_filter)?;
        let mut emergencies = Vec::new();
        for node in nodes {
            for emergency in node.emergencies.values() {
                if active_only && !emergency.active {
                    continue;
                }
                emergencies.push(emergency_detail(node, emergency));
            }
        }
        emergencies.sort_by(|a, b| {
            b.active
                .cmp(&a.active)
                .then(b.raised_at.cmp(&a.raised_at))
                .then(a.node_id.cmp(&b.node_id))
                .then(a.source_issi.cmp(&b.source_issi))
        });
        let active_count = emergencies.iter().filter(|e| e.active).count();
        Some(ControlRoomEmergenciesSnapshot {
            now: now_iso(),
            node_filter: node_filter.map(ToString::to_string),
            count: emergencies.len(),
            active_count,
            emergencies,
        })
    }

    fn node_detail(&self, node_id: &str) -> Option<ControlRoomNodeDetail> {
        let node = self.nodes.get(node_id)?;
        let mut subscribers: Vec<_> = node.subscribers.values().map(|s| subscriber_detail(node, s)).collect();
        subscribers.sort_by_key(|s| s.issi);

        let mut groups: Vec<_> = node.groups.values().map(|g| group_detail(node, g)).collect();
        groups.sort_by_key(|g| g.gssi);

        let mut active_calls: Vec<_> = node.active_calls.iter().map(|(key, call)| call_detail(node, key, call)).collect();
        active_calls.sort_by(|a, b| a.started_at.cmp(&b.started_at).then(a.key.cmp(&b.key)));

        let mut emergencies: Vec<_> = node.emergencies.values().map(|e| emergency_detail(node, e)).collect();
        emergencies.sort_by(|a, b| b.active.cmp(&a.active).then(b.raised_at.cmp(&a.raised_at)));

        let mut sds_log: Vec<_> = node.sds_log.iter().rev().take(100).map(|entry| sds_detail(node, entry)).collect();
        sds_log.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        Some(ControlRoomNodeDetail {
            now: now_iso(),
            node: node.overview(),
            subscribers,
            groups,
            active_calls,
            emergencies,
            sds_log,
            brew: node.brew.clone(),
            timeslot_activity: node.timeslot_activity.clone(),
            errors: node.errors.iter().cloned().collect(),
        })
    }

    fn selected_nodes<'a>(&'a self, node_filter: Option<&str>) -> Option<Vec<&'a NodeState>> {
        if let Some(node_id) = node_filter {
            return self.nodes.get(node_id).map(|node| vec![node]);
        }
        let mut nodes: Vec<&NodeState> = self.nodes.values().collect();
        nodes.sort_by(|a, b| a.node_id.cmp(&b.node_id));
        Some(nodes)
    }

    fn node_summary(&self, node_id: &str) -> Option<NodeOverview> {
        self.nodes.get(node_id).map(NodeState::overview)
    }

    fn apply_node_message(&mut self, message: &NodeToControlRoomMessage) -> Option<String> {
        match message {
            NodeToControlRoomMessage::Hello { hello } => {
                let node_id = hello.node.node_id.clone();
                self.node_mut(&node_id).apply_hello(hello);
                self.push_event(EventLogEntry {
                    timestamp: now_iso(),
                    node_id: node_id.clone(),
                    seq: None,
                    event_type: "hello".to_string(),
                    event: serde_json::to_value(hello).unwrap_or_else(|_| json!({ "error": "hello serialisation failed" })),
                });
                Some(node_id)
            }
            NodeToControlRoomMessage::Heartbeat { heartbeat } => {
                let node_id = heartbeat.node_id.clone();
                self.node_mut(&node_id).apply_heartbeat(heartbeat);
                Some(node_id)
            }
            NodeToControlRoomMessage::Telemetry { envelope } => {
                let node_id = envelope.node_id.clone();
                self.node_mut(&node_id).apply_telemetry(envelope);
                self.push_event(EventLogEntry {
                    timestamp: envelope.timestamp.clone(),
                    node_id: node_id.clone(),
                    seq: Some(envelope.seq),
                    event_type: telemetry_event_type(&envelope.event).to_string(),
                    event: event_for_log(&envelope.event),
                });
                Some(node_id)
            }
            NodeToControlRoomMessage::ControlAck { ack } => {
                let node_id = ack.node_id.clone();
                {
                    let node = self.node_mut(&node_id);
                    node.command_ack_count = node.command_ack_count.wrapping_add(1);
                    node.last_seen = Some(ack.timestamp.clone());
                }
                self.record_command_ack(ack);
                Some(node_id)
            }
            NodeToControlRoomMessage::ControlResponse { envelope } => {
                let node_id = envelope.node_id.clone();
                {
                    let node = self.node_mut(&node_id);
                    node.control_response_count = node.control_response_count.wrapping_add(1);
                    node.last_seen = Some(envelope.timestamp.clone());
                }
                self.record_control_response(envelope);
                Some(node_id)
            }
            NodeToControlRoomMessage::Error { node_id, message, timestamp } => {
                self.node_mut(node_id).push_error(format!("{}: {}", timestamp, message));
                self.push_event(EventLogEntry {
                    timestamp: timestamp.clone(),
                    node_id: node_id.clone(),
                    seq: None,
                    event_type: "node_error".to_string(),
                    event: json!({ "message": message }),
                });
                Some(node_id.clone())
            }
        }
    }

    fn mark_node_connected_transport(&mut self, node_id: &str) {
        let node = self.node_mut(node_id);
        node.connected = true;
        node.transport_connected = true;
        node.last_seen = Some(now_iso());
    }

    fn mark_node_disconnected(&mut self, node_id: &str) {
        let node = self.node_mut(node_id);
        node.connected = false;
        node.transport_connected = false;
        node.last_seen = Some(now_iso());
    }

    fn record_command_queued(&mut self, envelope: &ControlCommandEnvelope) {
        self.push_command(CommandAuditEntry {
            command_id: envelope.command_id.clone(),
            target_node_id: envelope.target_node_id.clone(),
            operator_id: envelope.operator_id.clone(),
            issued_at: envelope.issued_at.clone(),
            updated_at: now_iso(),
            status: "queued".to_string(),
            target_entity: None,
            message: None,
            command: serde_json::to_value(&envelope.command).unwrap_or_else(|_| json!({ "error": "command serialisation failed" })),
            responses: Vec::new(),
        });
    }

    fn record_command_ack(&mut self, ack: &ControlCommandAck) {
        let idx = self.recent_commands.iter().position(|cmd| cmd.command_id == ack.command_id);
        let status = if ack.accepted { "accepted" } else { "rejected" }.to_string();
        if let Some(idx) = idx {
            if let Some(cmd) = self.recent_commands.get_mut(idx) {
                cmd.updated_at = ack.timestamp.clone();
                cmd.status = status;
                cmd.target_entity = ack.target_entity;
                cmd.message = Some(ack.message.clone());
            }
        } else {
            self.push_command(CommandAuditEntry {
                command_id: ack.command_id.clone(),
                target_node_id: ack.node_id.clone(),
                operator_id: None,
                issued_at: ack.timestamp.clone(),
                updated_at: ack.timestamp.clone(),
                status,
                target_entity: ack.target_entity,
                message: Some(ack.message.clone()),
                command: json!(null),
                responses: Vec::new(),
            });
        }
    }

    fn record_control_response(&mut self, envelope: &ControlResponseEnvelope) {
        let response_value = serde_json::to_value(&envelope.response).unwrap_or_else(|_| json!({ "error": "response serialisation failed" }));
        if let Some(command_id) = &envelope.command_id {
            if let Some(cmd) = self.recent_commands.iter_mut().find(|cmd| cmd.command_id == *command_id) {
                cmd.updated_at = envelope.timestamp.clone();
                cmd.status = "completed".to_string();
                cmd.target_entity = envelope.target_entity;
                cmd.responses.push(response_value);
                return;
            }
        }

        self.push_command(CommandAuditEntry {
            command_id: envelope.command_id.clone().unwrap_or_else(|| format!("uncorrelated-{}", Uuid::new_v4())),
            target_node_id: envelope.node_id.clone(),
            operator_id: None,
            issued_at: envelope.timestamp.clone(),
            updated_at: envelope.timestamp.clone(),
            status: "response".to_string(),
            target_entity: envelope.target_entity,
            message: Some("uncorrelated control response".to_string()),
            command: json!(null),
            responses: vec![response_value],
        });
    }

    fn node_mut(&mut self, node_id: &str) -> &mut NodeState {
        self.nodes
            .entry(node_id.to_string())
            .or_insert_with(|| NodeState::new(node_id.to_string()))
    }

    fn push_event(&mut self, entry: EventLogEntry) {
        self.recent_events.push_back(entry);
        while self.recent_events.len() > self.history_limit {
            self.recent_events.pop_front();
        }
    }

    fn push_command(&mut self, entry: CommandAuditEntry) {
        self.recent_commands.push_back(entry);
        while self.recent_commands.len() > self.history_limit {
            self.recent_commands.pop_front();
        }
    }
}

fn subscriber_detail(node: &NodeState, subscriber: &SubscriberState) -> SubscriberDetail {
    let mut groups: Vec<u32> = subscriber.groups.iter().copied().collect();
    groups.sort_unstable();

    SubscriberDetail {
        node_id: node.node_id.clone(),
        station_name: node.station_name.clone(),
        issi: subscriber.issi,
        online: subscriber.online,
        groups,
        rssi_dbfs: subscriber.rssi_dbfs,
        energy_saving_mode: subscriber.energy_saving_mode,
        emergency: subscriber.emergency,
        remote_source: subscriber.remote_source.clone(),
        last_seen: subscriber.last_seen.clone(),
        last_event: subscriber.last_event.clone(),
        timeout_drop_count: subscriber.timeout_drop_count,
        active_call_keys: subscriber_active_call_keys(node, subscriber.issi),
    }
}

fn group_detail(node: &NodeState, group: &GroupState) -> GroupDetail {
    let mut members: Vec<u32> = group.members.iter().copied().collect();
    members.sort_unstable();

    let members_online = members
        .iter()
        .filter(|issi| node.subscribers.get(issi).map(|subscriber| subscriber.online).unwrap_or(false))
        .count();

    let active_call_key = group.active_call_id.and_then(|call_id| {
        node.active_calls.iter().find_map(|(key, call)| match call {
            CallState::Group { call_id: current_call_id, gssi, .. } if *current_call_id == call_id && *gssi == group.gssi => Some(key.clone()),
            _ => None,
        })
    });

    GroupDetail {
        node_id: node.node_id.clone(),
        station_name: node.station_name.clone(),
        gssi: group.gssi,
        members,
        members_online,
        active_call_id: group.active_call_id,
        active_call_key,
    }
}

fn call_detail(node: &NodeState, key: &str, call: &CallState) -> CallDetail {
    match call {
        CallState::Group {
            call_id,
            gssi,
            caller_issi,
            speaker_issi,
            carrier_num,
            ts,
            priority,
            source,
            started_at,
            last_activity,
        } => CallDetail {
            node_id: node.node_id.clone(),
            station_name: node.station_name.clone(),
            key: key.to_string(),
            call_kind: "group".to_string(),
            call_id: *call_id,
            gssi: Some(*gssi),
            caller_issi: Some(*caller_issi),
            speaker_issi: *speaker_issi,
            calling_issi: None,
            called_issi: None,
            simplex: None,
            carrier_num: *carrier_num,
            ts: *ts,
            priority: *priority,
            source: source.clone(),
            started_at: started_at.clone(),
            last_activity: last_activity.clone(),
        },
        CallState::Individual {
            call_id,
            calling_issi,
            called_issi,
            simplex,
            carrier_num,
            ts,
            priority,
            source,
            started_at,
            last_activity,
        } => CallDetail {
            node_id: node.node_id.clone(),
            station_name: node.station_name.clone(),
            key: key.to_string(),
            call_kind: "individual".to_string(),
            call_id: *call_id,
            gssi: None,
            caller_issi: None,
            speaker_issi: None,
            calling_issi: Some(*calling_issi),
            called_issi: Some(*called_issi),
            simplex: Some(*simplex),
            carrier_num: *carrier_num,
            ts: *ts,
            priority: *priority,
            source: source.clone(),
            started_at: started_at.clone(),
            last_activity: last_activity.clone(),
        },
    }
}

fn sds_detail(node: &NodeState, entry: &SdsLogEntry) -> SdsDetail {
    SdsDetail {
        node_id: node.node_id.clone(),
        station_name: node.station_name.clone(),
        timestamp: entry.timestamp.clone(),
        direction: entry.direction.clone(),
        source_issi: entry.source_issi,
        dest_issi: entry.dest_issi,
        is_group: entry.is_group,
        protocol_id: entry.protocol_id,
        text: entry.text.clone(),
    }
}

fn emergency_detail(node: &NodeState, emergency: &EmergencyState) -> EmergencyDetail {
    EmergencyDetail {
        node_id: node.node_id.clone(),
        station_name: node.station_name.clone(),
        source_issi: emergency.source_issi,
        dest_ssi: emergency.dest_ssi,
        active: emergency.active,
        raised_at: emergency.raised_at.clone(),
        cleared_at: emergency.cleared_at.clone(),
    }
}

fn subscriber_active_call_keys(node: &NodeState, issi: u32) -> Vec<String> {
    let mut keys: Vec<String> = node
        .active_calls
        .iter()
        .filter(|(_, call)| call_involves_subscriber(call, issi))
        .map(|(key, _)| key.clone())
        .collect();
    keys.sort();
    keys
}

fn call_involves_subscriber(call: &CallState, issi: u32) -> bool {
    match call {
        CallState::Group { caller_issi, speaker_issi, .. } => *caller_issi == issi || speaker_issi.map(|speaker| speaker == issi).unwrap_or(false),
        CallState::Individual { calling_issi, called_issi, .. } => *calling_issi == issi || *called_issi == issi,
    }
}

pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub fn telemetry_event_type(event: &TelemetryEvent) -> &'static str {
    match event {
        TelemetryEvent::MsRegistration { .. } => "ms_registration",
        TelemetryEvent::MsDeregistration { .. } => "ms_deregistration",
        TelemetryEvent::MsTimeoutDrop { .. } => "ms_timeout_drop",
        TelemetryEvent::MsGroupAttach { .. } => "ms_group_attach",
        TelemetryEvent::MsGroupsSnapshot { .. } => "ms_groups_snapshot",
        TelemetryEvent::MsGroupDetach { .. } => "ms_group_detach",
        TelemetryEvent::MsRssi { .. } => "ms_rssi",
        TelemetryEvent::GroupCallStarted { .. } => "group_call_started",
        TelemetryEvent::GroupCallEnded { .. } => "group_call_ended",
        TelemetryEvent::GroupCallSpeakerChanged { .. } => "group_call_speaker_changed",
        TelemetryEvent::IndividualCallStarted { .. } => "individual_call_started",
        TelemetryEvent::IndividualCallEnded { .. } => "individual_call_ended",
        TelemetryEvent::MsEnergySaving { .. } => "ms_energy_saving",
        TelemetryEvent::BrewConnected { .. } => "brew_connected",
        TelemetryEvent::SdsActivity { .. } => "sds_activity",
        TelemetryEvent::SdsLog { .. } => "sds_log",
        TelemetryEvent::TsVoiceActivity { .. } => "ts_voice_activity",
        TelemetryEvent::TxVisual { .. } => "tx_visual",
        TelemetryEvent::TxQuality { .. } => "tx_quality",
        TelemetryEvent::SdrHealth { .. } => "sdr_health",
        TelemetryEvent::SysHealth { .. } => "sys_health",
        TelemetryEvent::HealthSnapshot(_) => "health_snapshot",
        TelemetryEvent::EmergencyAlarm { .. } => "emergency_alarm",
        TelemetryEvent::EmergencyCancel { .. } => "emergency_cancel",
        TelemetryEvent::DapnetLog { .. } => "dapnet_log",
        TelemetryEvent::MeshcomMessageLog { .. } => "meshcom_message_log",
        TelemetryEvent::MeshcomNodeUpdate { .. } => "meshcom_node_update",
        TelemetryEvent::BrewSubscriberRegistered { .. } => "brew_subscriber_registered",
        TelemetryEvent::BrewSubscriberDeregistered { .. } => "brew_subscriber_deregistered",
    }
}

fn is_noisy_event_type(event_type: &str) -> bool {
    matches!(
        event_type,
        "tx_visual" | "tx_quality" | "sdr_health" | "sys_health" | "health_snapshot" | "ms_rssi" | "ts_voice_activity"
    )
}

fn extract_health_overall(value: &Option<Value>) -> Option<String> {
    let value = value.as_ref()?;
    value
        .get("HealthSnapshot")
        .and_then(|v| v.get("overall"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn extract_f64_path(value: &Option<Value>, path: &[&str]) -> Option<f64> {
    let mut current = value.as_ref()?;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_f64()
}

fn event_for_log(event: &TelemetryEvent) -> Value {
    match event {
        TelemetryEvent::TxVisual {
            sample_rate,
            center_freq_hz,
            rms_dbfs,
            peak_dbfs,
            spectrum_db_tenths,
            constellation_iq,
        } => json!({
            "TxVisual": {
                "sample_rate": sample_rate,
                "center_freq_hz": center_freq_hz,
                "rms_dbfs": rms_dbfs,
                "peak_dbfs": peak_dbfs,
                "spectrum_bins": spectrum_db_tenths.len(),
                "constellation_points": constellation_iq.len() / 2
            }
        }),
        _ => serde_json::to_value(event).unwrap_or_else(|_| json!({ "error": "event serialisation failed" })),
    }
}

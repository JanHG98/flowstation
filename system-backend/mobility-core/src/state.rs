use std::collections::{BTreeSet, HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tetra_entities::net_control::{
    ControlCommand, ControlResponse, MobilityContextPayload,
};
use tetra_entities::net_control_room::NodeToControlRoomMessage;
use tetra_entities::net_telemetry::TelemetryEvent;
use uuid::Uuid;

use crate::config::MobilityCoreConfig;
use crate::protocol::{
    BackendEvent, BackendRequest, GatewayNodeSnapshot, GatewaySnapshot,
};

#[derive(Debug, Clone, Serialize)]
pub struct MobilityStatus {
    pub service: &'static str,
    pub started_at: String,
    pub security_mode: &'static str,
    pub warning: &'static str,
    pub node_gateway_connected: bool,
    pub node_gateway_last_error: Option<String>,
    pub nodes_known: usize,
    pub nodes_connected: usize,
    pub subscribers_known: usize,
    pub transfers_active: usize,
    pub transfers_completed: u64,
    pub transfers_failed: u64,
    pub total_gateway_events: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeRecord {
    pub node_id: String,
    pub station_name: String,
    pub site: Option<String>,
    pub connected: bool,
    pub stale: bool,
    pub last_seen: String,
    pub mcc: u16,
    pub mnc: u16,
    pub location_area: u16,
    pub colour_code: u8,
    pub main_carrier: u16,
    pub secondary_carrier: Option<u16>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubscriberRecord {
    pub issi: u32,
    pub serving_node: Option<String>,
    pub registered: bool,
    pub groups: BTreeSet<u32>,
    pub energy_saving_mode: Option<u8>,
    pub last_rssi_dbfs: Option<f32>,
    pub first_seen: String,
    pub last_seen: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TransferPhase {
    ExportQueued,
    ExportRequested,
    ImportQueued,
    ImportRequested,
    SourceCleanupQueued,
    SourceCleanupRequested,
    Completed,
    Cancelled,
    Failed,
    TimedOut,
}

impl TransferPhase {
    pub fn terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Cancelled | Self::Failed | Self::TimedOut
        )
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TransferRecord {
    pub transfer_id: String,
    pub handle: u32,
    pub issi: u32,
    pub source_node: String,
    pub target_node: String,
    pub target_local_issi: u32,
    pub phase: TransferPhase,
    pub created_at: String,
    pub updated_at: String,
    pub export_command_id: Option<String>,
    pub import_command_id: Option<String>,
    pub cleanup_command_id: Option<String>,
    pub context: Option<MobilityContextPayload>,
    pub error: Option<String>,
    #[serde(skip)]
    deadline: Instant,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateTransferRequest {
    pub issi: u32,
    pub source_node: String,
    pub target_node: String,
    pub target_local_issi: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CoreEventRecord {
    pub seq: u64,
    pub timestamp: String,
    pub kind: String,
    pub transfer_id: Option<String>,
    pub node_id: Option<String>,
    pub issi: Option<u32>,
    pub detail: Value,
}

struct MobilityState {
    config: MobilityCoreConfig,
    started_at: String,
    gateway_connected: bool,
    gateway_last_error: Option<String>,
    nodes: HashMap<String, NodeRecord>,
    subscribers: HashMap<u32, SubscriberRecord>,
    transfers: HashMap<String, TransferRecord>,
    events: VecDeque<CoreEventRecord>,
    next_event_seq: u64,
    next_handle: u32,
    transfers_completed: u64,
    transfers_failed: u64,
    total_gateway_events: u64,
}

#[derive(Clone)]
pub struct SharedMobility(Arc<Mutex<MobilityState>>);

impl SharedMobility {
    pub fn new(config: MobilityCoreConfig) -> Self {
        Self(Arc::new(Mutex::new(MobilityState {
            config,
            started_at: now_iso(),
            gateway_connected: false,
            gateway_last_error: None,
            nodes: HashMap::new(),
            subscribers: HashMap::new(),
            transfers: HashMap::new(),
            events: VecDeque::new(),
            next_event_seq: 1,
            next_handle: 1,
            transfers_completed: 0,
            transfers_failed: 0,
            total_gateway_events: 0,
        })))
    }

    pub fn status(&self) -> MobilityStatus {
        let state = self.0.lock().expect("mobility state poisoned");
        MobilityStatus {
            service: "netcore-mobility-core",
            started_at: state.started_at.clone(),
            security_mode: "open_lab",
            warning: "No authentication, no tokens and no TLS; isolated test network only",
            node_gateway_connected: state.gateway_connected,
            node_gateway_last_error: state.gateway_last_error.clone(),
            nodes_known: state.nodes.len(),
            nodes_connected: state.nodes.values().filter(|node| node.connected && !node.stale).count(),
            subscribers_known: state.subscribers.len(),
            transfers_active: state.transfers.values().filter(|transfer| !transfer.phase.terminal()).count(),
            transfers_completed: state.transfers_completed,
            transfers_failed: state.transfers_failed,
            total_gateway_events: state.total_gateway_events,
        }
    }

    pub fn nodes(&self) -> Vec<NodeRecord> {
        let state = self.0.lock().expect("mobility state poisoned");
        let mut nodes: Vec<_> = state.nodes.values().cloned().collect();
        nodes.sort_by(|a, b| a.node_id.cmp(&b.node_id));
        nodes
    }

    pub fn subscribers(&self) -> Vec<SubscriberRecord> {
        let state = self.0.lock().expect("mobility state poisoned");
        let mut subscribers: Vec<_> = state.subscribers.values().cloned().collect();
        subscribers.sort_by_key(|subscriber| subscriber.issi);
        subscribers
    }

    pub fn transfers(&self) -> Vec<TransferRecord> {
        let state = self.0.lock().expect("mobility state poisoned");
        let mut transfers: Vec<_> = state.transfers.values().cloned().collect();
        transfers.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        transfers
    }

    pub fn recent_events(&self, limit: usize) -> Vec<CoreEventRecord> {
        let state = self.0.lock().expect("mobility state poisoned");
        state.events.iter().rev().take(limit.min(state.events.len())).cloned().collect()
    }

    pub fn gateway_connected(&self) {
        let mut state = self.0.lock().expect("mobility state poisoned");
        state.gateway_connected = true;
        state.gateway_last_error = None;
        push_event(&mut state, "gateway_connected", None, None, None, json!({}));
    }

    pub fn gateway_disconnected(&self, error: impl Into<String>) {
        let mut state = self.0.lock().expect("mobility state poisoned");
        state.gateway_connected = false;
        let error = error.into();
        state.gateway_last_error = Some(error.clone());
        push_event(
            &mut state,
            "gateway_disconnected",
            None,
            None,
            None,
            json!({ "error": error }),
        );
    }

    pub fn create_transfer(
        &self,
        request: CreateTransferRequest,
    ) -> Result<(TransferRecord, BackendRequest), String> {
        let mut state = self.0.lock().expect("mobility state poisoned");
        if !state.config.security.allow_remote_management {
            return Err("remote management is disabled".to_string());
        }
        if request.source_node == request.target_node {
            return Err("source_node and target_node must differ".to_string());
        }
        for node_id in [&request.source_node, &request.target_node] {
            let node = state.nodes.get(node_id)
                .ok_or_else(|| format!("unknown node {node_id}"))?;
            if !node.connected || node.stale {
                return Err(format!("node {node_id} is not currently usable"));
            }
        }
        let subscriber = state.subscribers.get(&request.issi)
            .ok_or_else(|| format!("subscriber {} is unknown", request.issi))?;
        if subscriber.serving_node.as_deref() != Some(request.source_node.as_str()) || !subscriber.registered {
            return Err(format!(
                "subscriber {} is not registered on source node {}",
                request.issi, request.source_node
            ));
        }
        if state.transfers.values().any(|transfer| {
            transfer.issi == request.issi && !transfer.phase.terminal()
        }) {
            return Err(format!("subscriber {} already has an active transfer", request.issi));
        }
        if state.transfers.len() >= state.config.limits.max_transfers {
            return Err("transfer limit reached".to_string());
        }

        let transfer_id = Uuid::new_v4().to_string();
        let handle = state.next_handle;
        state.next_handle = state.next_handle.wrapping_add(1).max(1);
        let now = now_iso();
        let transfer = TransferRecord {
            transfer_id: transfer_id.clone(),
            handle,
            issi: request.issi,
            source_node: request.source_node.clone(),
            target_node: request.target_node.clone(),
            target_local_issi: request.target_local_issi.unwrap_or(request.issi),
            phase: TransferPhase::ExportQueued,
            created_at: now.clone(),
            updated_at: now,
            export_command_id: None,
            import_command_id: None,
            cleanup_command_id: None,
            context: None,
            error: None,
            deadline: Instant::now() + Duration::from_secs(state.config.server.transfer_timeout_secs),
        };
        state.transfers.insert(transfer_id.clone(), transfer.clone());
        push_event(
            &mut state,
            "transfer_created",
            Some(transfer_id.clone()),
            Some(request.source_node),
            Some(request.issi),
            json!({ "target_node": request.target_node, "target_local_issi": transfer.target_local_issi }),
        );

        let backend = command_request(
            &transfer_id,
            "export",
            &transfer.source_node,
            ControlCommand::MobilityExportContext {
                handle,
                issi: transfer.issi,
            },
        );
        Ok((transfer, backend))
    }

    pub fn cancel_transfer(&self, transfer_id: &str) -> Result<TransferRecord, String> {
        let mut state = self.0.lock().expect("mobility state poisoned");
        let transfer = state.transfers.get_mut(transfer_id)
            .ok_or_else(|| format!("unknown transfer {transfer_id}"))?;
        if matches!(
            transfer.phase,
            TransferPhase::SourceCleanupQueued
                | TransferPhase::SourceCleanupRequested
                | TransferPhase::Completed
        ) {
            return Err("transfer cannot be cancelled after target import completed".to_string());
        }
        if transfer.phase.terminal() {
            return Err(format!("transfer is already {:?}", transfer.phase));
        }
        transfer.phase = TransferPhase::Cancelled;
        transfer.updated_at = now_iso();
        transfer.error = Some("cancelled by operator".to_string());
        let snapshot = transfer.clone();
        push_event(
            &mut state,
            "transfer_cancelled",
            Some(transfer_id.to_string()),
            None,
            Some(snapshot.issi),
            json!({}),
        );
        Ok(snapshot)
    }

    pub fn handle_backend_event(&self, event: BackendEvent) -> Vec<BackendRequest> {
        let mut state = self.0.lock().expect("mobility state poisoned");
        state.total_gateway_events = state.total_gateway_events.wrapping_add(1);
        let mut requests = Vec::new();

        match event {
            BackendEvent::Snapshot { snapshot } => apply_gateway_snapshot(&mut state, snapshot),
            BackendEvent::Event { event } => {
                if let Some(node_id) = &event.node_id {
                    if event.kind == "node_disconnected" {
                        if let Some(node) = state.nodes.get_mut(node_id) {
                            node.connected = false;
                        }
                    }
                }
            }
            BackendEvent::ActionResult {
                request_id,
                command_id,
                ok,
                message,
            } => {
                if let Some(request_id) = request_id {
                    apply_action_result(
                        &mut state,
                        &request_id,
                        command_id,
                        ok,
                        message,
                    );
                }
            }
            BackendEvent::NodeMessage { node_id, message } => {
                requests.extend(apply_node_message(&mut state, &node_id, message));
            }
        }
        requests
    }

    pub fn expire_transfers(&self) {
        let mut state = self.0.lock().expect("mobility state poisoned");
        let now = Instant::now();
        let expired: Vec<String> = state.transfers.iter()
            .filter(|(_, transfer)| !transfer.phase.terminal() && transfer.deadline <= now)
            .map(|(id, _)| id.clone())
            .collect();
        for id in expired {
            let issi = if let Some(transfer) = state.transfers.get_mut(&id) {
                transfer.phase = TransferPhase::TimedOut;
                transfer.updated_at = now_iso();
                transfer.error = Some("transfer timed out".to_string());
                transfer.issi
            } else {
                continue;
            };
            state.transfers_failed = state.transfers_failed.wrapping_add(1);
            push_event(
                &mut state,
                "transfer_timed_out",
                Some(id),
                None,
                Some(issi),
                json!({}),
            );
        }
    }

    pub fn metrics(&self) -> String {
        let status = self.status();
        format!(
            concat!(
                "# HELP netcore_mobility_core_up Service liveness.\n",
                "# TYPE netcore_mobility_core_up gauge\n",
                "netcore_mobility_core_up 1\n",
                "# TYPE netcore_mobility_core_gateway_connected gauge\n",
                "netcore_mobility_core_gateway_connected {}\n",
                "# TYPE netcore_mobility_core_nodes_connected gauge\n",
                "netcore_mobility_core_nodes_connected {}\n",
                "# TYPE netcore_mobility_core_subscribers gauge\n",
                "netcore_mobility_core_subscribers {}\n",
                "# TYPE netcore_mobility_core_transfers_active gauge\n",
                "netcore_mobility_core_transfers_active {}\n",
                "# TYPE netcore_mobility_core_transfers_completed_total counter\n",
                "netcore_mobility_core_transfers_completed_total {}\n",
                "# TYPE netcore_mobility_core_transfers_failed_total counter\n",
                "netcore_mobility_core_transfers_failed_total {}\n"
            ),
            if status.node_gateway_connected { 1 } else { 0 },
            status.nodes_connected,
            status.subscribers_known,
            status.transfers_active,
            status.transfers_completed,
            status.transfers_failed,
        )
    }
}

fn command_request(
    transfer_id: &str,
    step: &str,
    node_id: &str,
    command: ControlCommand,
) -> BackendRequest {
    BackendRequest::Command {
        request_id: Some(format!("transfer/{transfer_id}/{step}")),
        node_id: node_id.to_string(),
        command,
        operator_id: Some("mobility-core-open-lab".to_string()),
    }
}

fn apply_gateway_snapshot(state: &mut MobilityState, snapshot: GatewaySnapshot) {
    state.gateway_connected = true;
    state.gateway_last_error = None;
    for node in snapshot.nodes {
        upsert_node(state, node);
    }
}

fn upsert_node(state: &mut MobilityState, node: GatewayNodeSnapshot) {
    state.nodes.insert(node.node_id.clone(), NodeRecord {
        node_id: node.node_id,
        station_name: node.identity.station_name,
        site: node.identity.site,
        connected: node.connected,
        stale: node.stale,
        last_seen: node.last_seen,
        mcc: node.identity.mcc,
        mnc: node.identity.mnc,
        location_area: node.identity.location_area,
        colour_code: node.identity.colour_code,
        main_carrier: node.identity.main_carrier,
        secondary_carrier: node.identity.secondary_carrier,
    });
}

fn apply_action_result(
    state: &mut MobilityState,
    request_id: &str,
    command_id: Option<String>,
    ok: bool,
    message: String,
) {
    let parts: Vec<&str> = request_id.split('/').collect();
    if parts.len() != 3 || parts[0] != "transfer" {
        return;
    }
    let transfer_id = parts[1];
    let step = parts[2];
    let mut failure: Option<(u32, String)> = None;
    if let Some(transfer) = state.transfers.get_mut(transfer_id) {
        if transfer.phase.terminal() {
            return;
        }
        transfer.updated_at = now_iso();
        if !ok {
            let still_waiting_for_this_step = match step {
                "export" => matches!(
                    &transfer.phase,
                    TransferPhase::ExportQueued | TransferPhase::ExportRequested
                ),
                "import" => matches!(
                    &transfer.phase,
                    TransferPhase::ImportQueued | TransferPhase::ImportRequested
                ),
                "cleanup" => matches!(
                    &transfer.phase,
                    TransferPhase::SourceCleanupQueued
                        | TransferPhase::SourceCleanupRequested
                ),
                _ => false,
            };
            if still_waiting_for_this_step {
                transfer.phase = TransferPhase::Failed;
                transfer.error = Some(message.clone());
                failure = Some((transfer.issi, message));
            }
        } else {
            match step {
                "export" => {
                    transfer.export_command_id = command_id;
                    if transfer.phase == TransferPhase::ExportQueued {
                        transfer.phase = TransferPhase::ExportRequested;
                    }
                }
                "import" => {
                    transfer.import_command_id = command_id;
                    if transfer.phase == TransferPhase::ImportQueued {
                        transfer.phase = TransferPhase::ImportRequested;
                    }
                }
                "cleanup" => {
                    transfer.cleanup_command_id = command_id;
                    if transfer.phase == TransferPhase::SourceCleanupQueued {
                        transfer.phase = TransferPhase::SourceCleanupRequested;
                    }
                }
                _ => {}
            }
        }
    }
    if let Some((issi, error)) = failure {
        state.transfers_failed = state.transfers_failed.wrapping_add(1);
        push_event(
            state,
            "transfer_command_rejected",
            Some(transfer_id.to_string()),
            None,
            Some(issi),
            json!({ "error": error }),
        );
    }
}

fn apply_node_message(
    state: &mut MobilityState,
    node_id: &str,
    message: NodeToControlRoomMessage,
) -> Vec<BackendRequest> {
    match message {
        NodeToControlRoomMessage::Telemetry { envelope } => {
            apply_telemetry(state, node_id, envelope.event);
            Vec::new()
        }
        NodeToControlRoomMessage::ControlResponse { envelope } => {
            apply_control_response(state, node_id, envelope.command_id.as_deref(), envelope.response)
        }
        NodeToControlRoomMessage::Heartbeat { heartbeat } => {
            if let Some(node) = state.nodes.get_mut(node_id) {
                node.connected = heartbeat.connected;
                node.stale = false;
                node.last_seen = heartbeat.timestamp;
            }
            Vec::new()
        }
        _ => Vec::new(),
    }
}

fn apply_telemetry(state: &mut MobilityState, node_id: &str, event: TelemetryEvent) {
    let now = now_iso();
    match event {
        TelemetryEvent::MsRegistration { issi } => {
            if state.subscribers.len() >= state.config.limits.max_subscribers
                && !state.subscribers.contains_key(&issi)
            {
                return;
            }
            let subscriber = state.subscribers.entry(issi).or_insert_with(|| SubscriberRecord {
                issi,
                serving_node: None,
                registered: false,
                groups: BTreeSet::new(),
                energy_saving_mode: None,
                last_rssi_dbfs: None,
                first_seen: now.clone(),
                last_seen: now.clone(),
            });
            subscriber.serving_node = Some(node_id.to_string());
            subscriber.registered = true;
            subscriber.last_seen = now;
        }
        TelemetryEvent::MsDeregistration { issi } | TelemetryEvent::MsTimeoutDrop { issi } => {
            if let Some(subscriber) = state.subscribers.get_mut(&issi) {
                if subscriber.serving_node.as_deref() == Some(node_id) {
                    subscriber.registered = false;
                    subscriber.last_seen = now;
                }
            }
        }
        TelemetryEvent::MsGroupAttach { issi, gssis } => {
            let subscriber = ensure_subscriber(state, node_id, issi, &now);
            subscriber.groups.extend(gssis);
        }
        TelemetryEvent::MsGroupsSnapshot { issi, gssis } => {
            let subscriber = ensure_subscriber(state, node_id, issi, &now);
            subscriber.groups = gssis.into_iter().collect();
        }
        TelemetryEvent::MsGroupDetach { issi, gssis } => {
            let subscriber = ensure_subscriber(state, node_id, issi, &now);
            for gssi in gssis {
                subscriber.groups.remove(&gssi);
            }
        }
        TelemetryEvent::MsEnergySaving { issi, mode } => {
            let subscriber = ensure_subscriber(state, node_id, issi, &now);
            subscriber.energy_saving_mode = Some(mode);
        }
        TelemetryEvent::MsRssi { issi, rssi_dbfs } => {
            let subscriber = ensure_subscriber(state, node_id, issi, &now);
            subscriber.last_rssi_dbfs = Some(rssi_dbfs);
        }
        _ => {}
    }
}

fn ensure_subscriber<'a>(
    state: &'a mut MobilityState,
    node_id: &str,
    issi: u32,
    now: &str,
) -> &'a mut SubscriberRecord {
    let subscriber = state.subscribers.entry(issi).or_insert_with(|| SubscriberRecord {
        issi,
        serving_node: Some(node_id.to_string()),
        registered: true,
        groups: BTreeSet::new(),
        energy_saving_mode: None,
        last_rssi_dbfs: None,
        first_seen: now.to_string(),
        last_seen: now.to_string(),
    });
    subscriber.last_seen = now.to_string();
    subscriber
}

fn apply_control_response(
    state: &mut MobilityState,
    node_id: &str,
    command_id: Option<&str>,
    response: ControlResponse,
) -> Vec<BackendRequest> {
    let response_handle = match &response {
        ControlResponse::MobilityContextExported { handle, .. }
        | ControlResponse::MobilityContextImported { handle, .. }
        | ControlResponse::MobilityContextRemoved { handle, .. } => Some(*handle),
        _ => None,
    };
    let transfer_id = state.transfers.iter().find_map(|(id, transfer)| {
        let command_matches = command_id.is_some_and(|command_id| {
            transfer.export_command_id.as_deref() == Some(command_id)
                || transfer.import_command_id.as_deref() == Some(command_id)
                || transfer.cleanup_command_id.as_deref() == Some(command_id)
        });
        let handle_matches = response_handle == Some(transfer.handle)
            && !transfer.phase.terminal()
            && (node_id == transfer.source_node || node_id == transfer.target_node);
        if command_matches || handle_matches {
            Some(id.clone())
        } else {
            None
        }
    });
    let Some(transfer_id) = transfer_id else {
        return Vec::new();
    };

    let mut next_request = None;
    let mut completion: Option<(u32, String, u32)> = None;
    let mut failure: Option<(u32, String)> = None;

    if let Some(transfer) = state.transfers.get_mut(&transfer_id) {
        if transfer.phase.terminal() {
            return Vec::new();
        }
        transfer.updated_at = now_iso();
        match response {
            ControlResponse::MobilityContextExported {
                found,
                context,
                message,
                ..
            } if node_id == transfer.source_node => {
                if !found {
                    transfer.phase = TransferPhase::Failed;
                    transfer.error = Some(message.clone());
                    failure = Some((transfer.issi, message));
                } else if let Some(context) = context {
                    transfer.context = Some(context.clone());
                    transfer.phase = TransferPhase::ImportQueued;
                    next_request = Some(command_request(
                        &transfer.transfer_id,
                        "import",
                        &transfer.target_node,
                        ControlCommand::MobilityImportContext {
                            handle: transfer.handle,
                            local_issi: transfer.target_local_issi,
                            context,
                        },
                    ));
                } else {
                    transfer.phase = TransferPhase::Failed;
                    transfer.error = Some("export response did not include context".to_string());
                    failure = Some((transfer.issi, "export response did not include context".to_string()));
                }
            }
            ControlResponse::MobilityContextImported {
                success,
                message,
                ..
            } if node_id == transfer.target_node => {
                if success {
                    transfer.phase = TransferPhase::SourceCleanupQueued;
                    next_request = Some(command_request(
                        &transfer.transfer_id,
                        "cleanup",
                        &transfer.source_node,
                        ControlCommand::MobilityRemoveContext {
                            handle: transfer.handle,
                            issi: transfer.issi,
                            reason: format!("transferred to {}", transfer.target_node),
                        },
                    ));
                } else {
                    transfer.phase = TransferPhase::Failed;
                    transfer.error = Some(message.clone());
                    failure = Some((transfer.issi, message));
                }
            }
            ControlResponse::MobilityContextRemoved {
                success,
                message,
                ..
            } if node_id == transfer.source_node => {
                if success {
                    transfer.phase = TransferPhase::Completed;
                    completion = Some((
                        transfer.issi,
                        transfer.target_node.clone(),
                        transfer.target_local_issi,
                    ));
                } else {
                    transfer.phase = TransferPhase::Failed;
                    transfer.error = Some(format!(
                        "target import succeeded, but source cleanup failed: {message}"
                    ));
                    failure = Some((transfer.issi, transfer.error.clone().unwrap_or_default()));
                }
            }
            _ => {}
        }
    }

    if let Some((issi, target_node, target_local_issi)) = completion {
        state.transfers_completed = state.transfers_completed.wrapping_add(1);
        let now = now_iso();
        if target_local_issi == issi {
            let subscriber = ensure_subscriber(state, &target_node, issi, &now);
            subscriber.serving_node = Some(target_node.clone());
            subscriber.registered = true;
        } else {
            if let Some(source) = state.subscribers.get_mut(&issi) {
                source.registered = false;
                source.last_seen = now.clone();
            }
            let mut target = state.subscribers.get(&issi).cloned().unwrap_or(SubscriberRecord {
                issi: target_local_issi,
                serving_node: None,
                registered: false,
                groups: BTreeSet::new(),
                energy_saving_mode: None,
                last_rssi_dbfs: None,
                first_seen: now.clone(),
                last_seen: now.clone(),
            });
            target.issi = target_local_issi;
            target.serving_node = Some(target_node.clone());
            target.registered = true;
            target.last_seen = now;
            state.subscribers.insert(target_local_issi, target);
        }
        push_event(
            state,
            "transfer_completed",
            Some(transfer_id.clone()),
            Some(target_node),
            Some(target_local_issi),
            json!({}),
        );
    }
    if let Some((issi, error)) = failure {
        state.transfers_failed = state.transfers_failed.wrapping_add(1);
        push_event(
            state,
            "transfer_failed",
            Some(transfer_id),
            None,
            Some(issi),
            json!({ "error": error }),
        );
    }

    next_request.into_iter().collect()
}

fn push_event(
    state: &mut MobilityState,
    kind: &str,
    transfer_id: Option<String>,
    node_id: Option<String>,
    issi: Option<u32>,
    detail: Value,
) {
    let event = CoreEventRecord {
        seq: state.next_event_seq,
        timestamp: now_iso(),
        kind: kind.to_string(),
        transfer_id,
        node_id,
        issi,
        detail,
    };
    state.next_event_seq = state.next_event_seq.wrapping_add(1);
    state.events.push_back(event);
    while state.events.len() > state.config.server.history_limit {
        state.events.pop_front();
    }
}

pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{GatewayStatus, GatewaySnapshot};
    use tetra_entities::net_control_room::{
        ControlRoomNodeCapabilities, ControlRoomNodeIdentity,
    };

    fn node(id: &str, la: u16) -> GatewayNodeSnapshot {
        GatewayNodeSnapshot {
            node_id: id.to_string(),
            session_id: "session".to_string(),
            peer: "127.0.0.1".to_string(),
            connected: true,
            stale: false,
            connected_at: now_iso(),
            last_seen: now_iso(),
            disconnected_at: None,
            disconnect_reason: None,
            heartbeat_seq: 1,
            message_count: 1,
            telemetry_count: 0,
            control_ack_count: 0,
            control_response_count: 0,
            error_count: 0,
            last_message_kind: "hello".to_string(),
            last_telemetry: None,
            identity: ControlRoomNodeIdentity {
                node_id: id.to_string(),
                station_name: id.to_string(),
                site: None,
                stack_version: "test".to_string(),
                mcc: 262,
                mnc: 1,
                location_area: la,
                main_carrier: 1521,
                secondary_carrier: None,
                colour_code: 1,
                system_code: 1,
            },
            capabilities: ControlRoomNodeCapabilities {
                telemetry: true,
                command: true,
                sds: true,
                raw_sds: true,
                dgna: true,
                kick_ms: true,
                emergency_clear: true,
                live_sds: false,
                service_control: false,
                brew_bridge: false,
                dual_carrier: false,
                packet_data: false,
                legacy_wap_sds: false,
                multi_pdch: false,
                subscriber_policy: true,
                group_policy: true,
                call_control: true,
                call_restore_context: true,
            },
        }
    }

    #[test]
    fn creates_export_request_for_known_subscriber() {
        let core = SharedMobility::new(MobilityCoreConfig::default());
        core.handle_backend_event(BackendEvent::Snapshot {
            snapshot: GatewaySnapshot {
                status: GatewayStatus {
                    service: "gateway".to_string(),
                    started_at: now_iso(),
                    security_mode: "open_lab".to_string(),
                    warning: String::new(),
                    remote_management_enabled: true,
                    node_path: "/ws/node".to_string(),
                    backend_path: "/ws/backend".to_string(),
                    known_nodes: 2,
                    connected_nodes: 2,
                    stale_nodes: 0,
                    backend_clients: 1,
                    total_node_sessions: 2,
                    total_node_messages: 0,
                    total_commands: 0,
                    total_disconnects: 0,
                },
                nodes: vec![node("a", 10), node("b", 11)],
            },
        });
        core.handle_backend_event(BackendEvent::NodeMessage {
            node_id: "a".to_string(),
            message: NodeToControlRoomMessage::Telemetry {
                envelope: tetra_entities::net_control_room::NodeTelemetryEnvelope {
                    node_id: "a".to_string(),
                    seq: 1,
                    timestamp: now_iso(),
                    event: TelemetryEvent::MsRegistration { issi: 1234 },
                },
            },
        });
        let (_, request) = core.create_transfer(CreateTransferRequest {
            issi: 1234,
            source_node: "a".to_string(),
            target_node: "b".to_string(),
            target_local_issi: None,
        }).expect("transfer");
        assert!(matches!(
            request,
            BackendRequest::Command {
                command: ControlCommand::MobilityExportContext { issi: 1234, .. },
                ..
            }
        ));
    }
    #[test]
    fn completes_three_step_context_transfer() {
        use tetra_entities::net_control::{
            MobilityClientState, MobilityContextPayload,
        };
        use tetra_entities::net_control_room::ControlResponseEnvelope;

        let core = SharedMobility::new(MobilityCoreConfig::default());
        core.handle_backend_event(BackendEvent::Snapshot {
            snapshot: GatewaySnapshot {
                status: GatewayStatus {
                    service: "gateway".to_string(),
                    started_at: now_iso(),
                    security_mode: "open_lab".to_string(),
                    warning: String::new(),
                    remote_management_enabled: true,
                    node_path: "/ws/node".to_string(),
                    backend_path: "/ws/backend".to_string(),
                    known_nodes: 2,
                    connected_nodes: 2,
                    stale_nodes: 0,
                    backend_clients: 1,
                    total_node_sessions: 2,
                    total_node_messages: 0,
                    total_commands: 0,
                    total_disconnects: 0,
                },
                nodes: vec![node("a", 10), node("b", 11)],
            },
        });
        core.handle_backend_event(BackendEvent::NodeMessage {
            node_id: "a".to_string(),
            message: NodeToControlRoomMessage::Telemetry {
                envelope: tetra_entities::net_control_room::NodeTelemetryEnvelope {
                    node_id: "a".to_string(),
                    seq: 1,
                    timestamp: now_iso(),
                    event: TelemetryEvent::MsRegistration { issi: 1234 },
                },
            },
        });

        let (transfer, _) = core.create_transfer(CreateTransferRequest {
            issi: 1234,
            source_node: "a".to_string(),
            target_node: "b".to_string(),
            target_local_issi: None,
        }).expect("transfer");

        core.handle_backend_event(BackendEvent::ActionResult {
            request_id: Some(format!("transfer/{}/export", transfer.transfer_id)),
            command_id: Some("export-command".to_string()),
            ok: true,
            message: "queued".to_string(),
        });

        let context = MobilityContextPayload {
            home_issi: 1234,
            state: MobilityClientState::Attached,
            groups: vec![100, 200],
            energy_saving_mode: 1,
            monitoring_frame: Some(1),
            monitoring_multiframe: Some(1),
            class_of_ms: None,
            last_handle: 0,
            tei: Some(42),
        };
        let import_requests = core.handle_backend_event(BackendEvent::NodeMessage {
            node_id: "a".to_string(),
            message: NodeToControlRoomMessage::ControlResponse {
                envelope: ControlResponseEnvelope {
                    command_id: Some("export-command".to_string()),
                    node_id: "a".to_string(),
                    target_entity: None,
                    timestamp: now_iso(),
                    response: ControlResponse::MobilityContextExported {
                        handle: transfer.handle,
                        issi: 1234,
                        found: true,
                        context: Some(context),
                        message: "ok".to_string(),
                    },
                },
            },
        });
        assert_eq!(import_requests.len(), 1);

        core.handle_backend_event(BackendEvent::ActionResult {
            request_id: Some(format!("transfer/{}/import", transfer.transfer_id)),
            command_id: Some("import-command".to_string()),
            ok: true,
            message: "queued".to_string(),
        });
        let cleanup_requests = core.handle_backend_event(BackendEvent::NodeMessage {
            node_id: "b".to_string(),
            message: NodeToControlRoomMessage::ControlResponse {
                envelope: ControlResponseEnvelope {
                    command_id: Some("import-command".to_string()),
                    node_id: "b".to_string(),
                    target_entity: None,
                    timestamp: now_iso(),
                    response: ControlResponse::MobilityContextImported {
                        handle: transfer.handle,
                        local_issi: 1234,
                        success: true,
                        message: "ok".to_string(),
                    },
                },
            },
        });
        assert_eq!(cleanup_requests.len(), 1);

        core.handle_backend_event(BackendEvent::ActionResult {
            request_id: Some(format!("transfer/{}/cleanup", transfer.transfer_id)),
            command_id: Some("cleanup-command".to_string()),
            ok: true,
            message: "queued".to_string(),
        });
        core.handle_backend_event(BackendEvent::NodeMessage {
            node_id: "a".to_string(),
            message: NodeToControlRoomMessage::ControlResponse {
                envelope: ControlResponseEnvelope {
                    command_id: Some("cleanup-command".to_string()),
                    node_id: "a".to_string(),
                    target_entity: None,
                    timestamp: now_iso(),
                    response: ControlResponse::MobilityContextRemoved {
                        handle: transfer.handle,
                        issi: 1234,
                        success: true,
                        message: "ok".to_string(),
                    },
                },
            },
        });

        let completed = core.transfers().into_iter()
            .find(|item| item.transfer_id == transfer.transfer_id)
            .expect("completed transfer");
        assert_eq!(completed.phase, TransferPhase::Completed);
        let subscriber = core.subscribers().into_iter()
            .find(|item| item.issi == 1234)
            .expect("subscriber");
        assert_eq!(subscriber.serving_node.as_deref(), Some("b"));
        assert!(subscriber.registered);
    }

}

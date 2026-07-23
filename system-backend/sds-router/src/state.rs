use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tetra_entities::net_control::{ControlCommand, ControlResponse};
use tetra_entities::net_control_room::NodeToControlRoomMessage;
use tetra_entities::net_telemetry::TelemetryEvent;
use uuid::Uuid;

use crate::config::SdsRouterConfig;
use crate::protocol::{BackendEvent, BackendRequest, GatewaySnapshot};

const DATABASE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageState {
    Received,
    Queued,
    Offline,
    InFlight,
    Delivered,
    Partial,
    Failed,
    Expired,
    Cancelled,
    DeadLetter,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LegState {
    Pending,
    InFlight,
    Delivered,
    RetryWaiting,
    Failed,
    Expired,
    Cancelled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RouteKind {
    Protocol,
    Individual,
    Group,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RouteTargetKind {
    Node,
    Application,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RouteMode {
    Tap,
    Intercept,
    Route,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteRule {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub kind: RouteKind,
    pub match_value: u32,
    pub target_kind: RouteTargetKind,
    pub target: String,
    pub mode: RouteMode,
    pub notes: String,
    pub created_at: String,
    pub updated_at: String,
    pub revision: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RouteInput {
    #[serde(default)]
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub kind: RouteKind,
    pub match_value: u32,
    pub target_kind: RouteTargetKind,
    pub target: String,
    #[serde(default = "default_route_mode")]
    pub mode: RouteMode,
    #[serde(default)]
    pub notes: String,
}

fn default_true() -> bool {
    true
}

fn default_route_mode() -> RouteMode {
    RouteMode::Route
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryLeg {
    pub node_id: String,
    pub state: LegState,
    pub attempts: u32,
    pub max_attempts: u32,
    pub handle: Option<u32>,
    pub command_id: Option<String>,
    pub queued_at: String,
    pub last_attempt_at: Option<String>,
    pub next_attempt_at: Option<String>,
    pub completed_at: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationLeg {
    pub application: String,
    pub route_id: String,
    pub mode: RouteMode,
    pub state: LegState,
    pub queued_at: String,
    pub completed_at: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalReport {
    pub received_at: String,
    pub source_issi: u32,
    pub status: u16,
    pub message_reference: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdsMessageRecord {
    pub id: String,
    pub ingress_node: Option<String>,
    pub ingress: String,
    pub source_issi: u32,
    pub dest_issi: u32,
    pub is_group: bool,
    /// 0 = status, 1..=4 = SDS data type.
    pub sds_type: u8,
    pub protocol_id: u8,
    pub status_code: Option<u16>,
    pub len_bits: u16,
    pub payload: Vec<u8>,
    pub text_preview: String,
    pub priority: u8,
    pub state: MessageState,
    pub created_at: String,
    pub updated_at: String,
    pub expires_at: String,
    pub ttl_secs: u64,
    pub duplicate_count: u32,
    pub message_reference: Option<u8>,
    pub terminal_report: Option<TerminalReport>,
    pub delivery_legs: Vec<DeliveryLeg>,
    pub application_legs: Vec<ApplicationLeg>,
    pub last_error: Option<String>,
    pub trace: Vec<TraceEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEntry {
    pub timestamp: String,
    pub kind: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MessageSummary {
    pub id: String,
    pub created_at: String,
    pub source_issi: u32,
    pub dest_issi: u32,
    pub is_group: bool,
    pub sds_type: u8,
    pub protocol_id: u8,
    pub priority: u8,
    pub state: MessageState,
    pub text_preview: String,
    pub payload_hex: String,
    pub expires_at: String,
    pub delivered_legs: usize,
    pub total_legs: usize,
    pub application_pending: usize,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeRecord {
    pub node_id: String,
    pub station_name: String,
    pub site: Option<String>,
    pub connected: bool,
    pub stale: bool,
    pub last_seen: String,
    pub sds_capable: bool,
    pub raw_sds_capable: bool,
    pub mcc: u16,
    pub mnc: u16,
    pub location_area: u16,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubscriberLocation {
    pub issi: u32,
    pub node_id: String,
    pub last_seen: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GroupLocation {
    pub gssi: u32,
    pub nodes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SdsEventRecord {
    pub seq: u64,
    pub timestamp: String,
    pub kind: String,
    pub message_id: Option<String>,
    pub node_id: Option<String>,
    pub detail: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct SdsRouterStatus {
    pub service: &'static str,
    pub started_at: String,
    pub security_mode: &'static str,
    pub warning: &'static str,
    pub node_gateway_connected: bool,
    pub node_gateway_last_error: Option<String>,
    pub database_revision: u64,
    pub messages_total: usize,
    pub queued: usize,
    pub offline: usize,
    pub in_flight: usize,
    pub delivered: usize,
    pub failed: usize,
    pub dead_letter: usize,
    pub routes_total: usize,
    pub nodes_connected: usize,
    pub subscribers_known: usize,
    pub groups_known: usize,
    pub application_outbox: usize,
    pub duplicate_messages: u64,
    pub authoritative_ingress: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MessageInput {
    pub source_issi: u32,
    pub dest_issi: u32,
    #[serde(default)]
    pub is_group: bool,
    #[serde(default = "default_sds_type")]
    pub sds_type: u8,
    #[serde(default = "default_protocol_id")]
    pub protocol_id: u8,
    pub status_code: Option<u16>,
    pub len_bits: Option<u16>,
    #[serde(default)]
    pub payload_hex: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub priority: u8,
    pub ttl_secs: Option<u64>,
    #[serde(default)]
    pub ingress: String,
    #[serde(default)]
    pub force_nodes: Vec<String>,
}

fn default_sds_type() -> u8 {
    4
}

fn default_protocol_id() -> u8 {
    0x82
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApplicationAckInput {
    #[serde(default = "default_true")]
    pub success: bool,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SdsDatabase {
    schema_version: u32,
    revision: u64,
    messages: BTreeMap<String, SdsMessageRecord>,
    routes: BTreeMap<String, RouteRule>,
}

struct PendingRequest {
    message_id: String,
    node_id: String,
    handle: u32,
}

struct RouterState {
    config: SdsRouterConfig,
    started_at: String,
    gateway_connected: bool,
    gateway_last_error: Option<String>,
    nodes: BTreeMap<String, NodeRecord>,
    subscribers: BTreeMap<u32, SubscriberLocation>,
    node_groups: BTreeMap<String, BTreeMap<u32, BTreeSet<u32>>>,
    group_nodes: BTreeMap<u32, BTreeSet<String>>,
    messages: BTreeMap<String, SdsMessageRecord>,
    routes: BTreeMap<String, RouteRule>,
    revision: u64,
    events: VecDeque<SdsEventRecord>,
    next_event_seq: u64,
    next_handle: u32,
    request_map: BTreeMap<String, PendingRequest>,
    command_map: BTreeMap<String, PendingRequest>,
    handle_map: BTreeMap<u32, PendingRequest>,
    dedupe: BTreeMap<u64, (String, String)>,
    duplicate_messages: u64,
}

#[derive(Clone)]
pub struct SharedSdsRouter(Arc<Mutex<RouterState>>);

impl SharedSdsRouter {
    pub fn load(config: SdsRouterConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let database = load_database(&config)?;
        let router = Self(Arc::new(Mutex::new(RouterState {
            config,
            started_at: now_iso(),
            gateway_connected: false,
            gateway_last_error: None,
            nodes: BTreeMap::new(),
            subscribers: BTreeMap::new(),
            node_groups: BTreeMap::new(),
            group_nodes: BTreeMap::new(),
            messages: database.messages,
            routes: database.routes,
            revision: database.revision,
            events: VecDeque::new(),
            next_event_seq: 1,
            next_handle: 1,
            request_map: BTreeMap::new(),
            command_map: BTreeMap::new(),
            handle_map: BTreeMap::new(),
            dedupe: BTreeMap::new(),
            duplicate_messages: 0,
        })));
        {
            let mut state = router.0.lock().expect("SDS router state poisoned");
            recover_incomplete_locked(&mut state);
            let _ = persist_locked(&state);
        }
        Ok(router)
    }

    pub fn status(&self) -> SdsRouterStatus {
        let state = self.0.lock().expect("SDS router state poisoned");
        status_locked(&state)
    }

    pub fn nodes(&self) -> Vec<NodeRecord> {
        let state = self.0.lock().expect("SDS router state poisoned");
        state.nodes.values().cloned().collect()
    }

    pub fn subscribers(&self) -> Vec<SubscriberLocation> {
        let state = self.0.lock().expect("SDS router state poisoned");
        state.subscribers.values().cloned().collect()
    }

    pub fn groups(&self) -> Vec<GroupLocation> {
        let state = self.0.lock().expect("SDS router state poisoned");
        state
            .group_nodes
            .iter()
            .map(|(gssi, nodes)| GroupLocation {
                gssi: *gssi,
                nodes: nodes.iter().cloned().collect(),
            })
            .collect()
    }

    pub fn messages(&self, limit: usize, state_filter: Option<MessageState>) -> Vec<MessageSummary> {
        let state = self.0.lock().expect("SDS router state poisoned");
        let mut values: Vec<_> = state
            .messages
            .values()
            .filter(|message| state_filter.is_none_or(|filter| message.state == filter))
            .map(|message| summary(message, state.config.security.mask_payload_in_list))
            .collect();
        values.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        values.truncate(limit.min(5_000));
        values
    }

    pub fn message(&self, id: &str) -> Option<SdsMessageRecord> {
        self.0
            .lock()
            .expect("SDS router state poisoned")
            .messages
            .get(id)
            .cloned()
    }

    pub fn routes(&self) -> Vec<RouteRule> {
        self.0
            .lock()
            .expect("SDS router state poisoned")
            .routes
            .values()
            .cloned()
            .collect()
    }

    pub fn recent_events(&self, limit: usize) -> Vec<SdsEventRecord> {
        self.0
            .lock()
            .expect("SDS router state poisoned")
            .events
            .iter()
            .rev()
            .take(limit.min(2_000))
            .cloned()
            .collect()
    }

    pub fn create_message(
        &self,
        input: MessageInput,
    ) -> Result<(SdsMessageRecord, Vec<BackendRequest>), String> {
        let mut state = self.0.lock().expect("SDS router state poisoned");
        let force_nodes = input.force_nodes.clone();
        let mut message = message_from_input_locked(&state, input)?;
        if state.messages.len() >= state.config.limits.max_messages {
            prune_terminal_messages_locked(&mut state);
        }
        if state.messages.len() >= state.config.limits.max_messages {
            return Err("message limit reached; delete or archive old records".to_string());
        }
        push_trace(&mut message, "accepted", "manual/API message accepted");
        let id = message.id.clone();
        state.messages.insert(id.clone(), message);
        state.revision = state.revision.saturating_add(1);
        plan_message_locked(&mut state, &id, &force_nodes);
        let requests = collect_due_requests_locked(&mut state);
        persist_locked(&state)?;
        push_event_locked(
            &mut state,
            "message_created",
            Some(id.clone()),
            None,
            json!({"force_nodes": force_nodes}),
        );
        let message = state.messages.get(&id).cloned().expect("inserted message");
        Ok((message, requests))
    }

    pub fn retry_message(&self, id: &str) -> Result<Vec<BackendRequest>, String> {
        let mut state = self.0.lock().expect("SDS router state poisoned");
        let now = now_iso();
        let message = state.messages.get_mut(id).ok_or_else(|| "message not found".to_string())?;
        if matches!(message.state, MessageState::Expired | MessageState::Cancelled) {
            return Err("expired or cancelled messages cannot be retried; requeue them instead".to_string());
        }
        for leg in &mut message.delivery_legs {
            if !matches!(leg.state, LegState::Delivered) {
                leg.state = LegState::Pending;
                leg.next_attempt_at = Some(now.clone());
                leg.last_error = None;
            }
        }
        message.state = MessageState::Queued;
        message.updated_at = now.clone();
        message.last_error = None;
        push_trace(message, "manual_retry", "operator requested a retry");
        let requests = collect_due_requests_locked(&mut state);
        persist_locked(&state)?;
        push_event_locked(&mut state, "message_retry", Some(id.to_string()), None, json!({}));
        Ok(requests)
    }

    pub fn cancel_message(&self, id: &str) -> Result<(), String> {
        let mut state = self.0.lock().expect("SDS router state poisoned");
        let message = state.messages.get_mut(id).ok_or_else(|| "message not found".to_string())?;
        if matches!(message.state, MessageState::Delivered | MessageState::Expired) {
            return Err("completed message cannot be cancelled".to_string());
        }
        message.state = MessageState::Cancelled;
        message.updated_at = now_iso();
        for leg in &mut message.delivery_legs {
            if !matches!(leg.state, LegState::Delivered) {
                leg.state = LegState::Cancelled;
            }
        }
        for leg in &mut message.application_legs {
            if !matches!(leg.state, LegState::Delivered) {
                leg.state = LegState::Cancelled;
            }
        }
        push_trace(message, "cancelled", "operator cancelled message");
        persist_locked(&state)?;
        push_event_locked(&mut state, "message_cancelled", Some(id.to_string()), None, json!({}));
        Ok(())
    }

    pub fn requeue_message(&self, id: &str) -> Result<(SdsMessageRecord, Vec<BackendRequest>), String> {
        let mut state = self.0.lock().expect("SDS router state poisoned");
        let original = state.messages.get(id).cloned().ok_or_else(|| "message not found".to_string())?;
        let now = Utc::now();
        let ttl = original.ttl_secs.min(state.config.routing.max_ttl_secs);
        let mut clone = original.clone();
        clone.id = Uuid::new_v4().to_string();
        clone.created_at = now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        clone.updated_at = clone.created_at.clone();
        clone.expires_at = (now + ChronoDuration::seconds(ttl as i64))
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        clone.state = MessageState::Received;
        clone.delivery_legs.clear();
        clone.application_legs.clear();
        clone.terminal_report = None;
        clone.duplicate_count = 0;
        clone.last_error = None;
        clone.trace.clear();
        push_trace(&mut clone, "requeued", &format!("requeued from {id}"));
        let new_id = clone.id.clone();
        state.messages.insert(new_id.clone(), clone);
        state.revision = state.revision.saturating_add(1);
        plan_message_locked(&mut state, &new_id, &[]);
        let requests = collect_due_requests_locked(&mut state);
        persist_locked(&state)?;
        push_event_locked(
            &mut state,
            "message_requeued",
            Some(new_id.clone()),
            None,
            json!({"source_message_id": id}),
        );
        Ok((state.messages.get(&new_id).cloned().expect("requeued message"), requests))
    }

    pub fn delete_message(&self, id: &str) -> Result<(), String> {
        let mut state = self.0.lock().expect("SDS router state poisoned");
        let Some(message) = state.messages.get(id) else {
            return Err("message not found".to_string());
        };
        if matches!(message.state, MessageState::InFlight | MessageState::Queued) {
            return Err("cancel an active message before deleting it".to_string());
        }
        state.messages.remove(id);
        state.revision = state.revision.saturating_add(1);
        persist_locked(&state)?;
        push_event_locked(&mut state, "message_deleted", Some(id.to_string()), None, json!({}));
        Ok(())
    }

    pub fn create_route(&self, input: RouteInput) -> Result<RouteRule, String> {
        let mut state = self.0.lock().expect("SDS router state poisoned");
        validate_route_input(&state, &input)?;
        if state.routes.len() >= state.config.limits.max_routes {
            return Err("route limit reached".to_string());
        }
        let now = now_iso();
        state.revision = state.revision.saturating_add(1);
        let route = RouteRule {
            id: Uuid::new_v4().to_string(),
            name: input.name.trim().to_string(),
            enabled: input.enabled,
            kind: input.kind,
            match_value: input.match_value,
            target_kind: input.target_kind,
            target: input.target.trim().to_string(),
            mode: input.mode,
            notes: input.notes.trim().to_string(),
            created_at: now.clone(),
            updated_at: now,
            revision: state.revision,
        };
        state.routes.insert(route.id.clone(), route.clone());
        persist_locked(&state)?;
        push_event_locked(
            &mut state,
            "route_created",
            None,
            None,
            json!({"route_id": route.id, "name": route.name}),
        );
        Ok(route)
    }

    pub fn update_route(&self, id: &str, input: RouteInput) -> Result<RouteRule, String> {
        let mut state = self.0.lock().expect("SDS router state poisoned");
        validate_route_input(&state, &input)?;
        let created_at = state.routes.get(id).map(|route| route.created_at.clone()).ok_or_else(|| "route not found".to_string())?;
        state.revision = state.revision.saturating_add(1);
        let route = RouteRule {
            id: id.to_string(),
            name: input.name.trim().to_string(),
            enabled: input.enabled,
            kind: input.kind,
            match_value: input.match_value,
            target_kind: input.target_kind,
            target: input.target.trim().to_string(),
            mode: input.mode,
            notes: input.notes.trim().to_string(),
            created_at,
            updated_at: now_iso(),
            revision: state.revision,
        };
        state.routes.insert(id.to_string(), route.clone());
        persist_locked(&state)?;
        push_event_locked(
            &mut state,
            "route_updated",
            None,
            None,
            json!({"route_id": id, "name": route.name}),
        );
        Ok(route)
    }

    pub fn delete_route(&self, id: &str) -> Result<(), String> {
        let mut state = self.0.lock().expect("SDS router state poisoned");
        if state.routes.remove(id).is_none() {
            return Err("route not found".to_string());
        }
        state.revision = state.revision.saturating_add(1);
        persist_locked(&state)?;
        push_event_locked(
            &mut state,
            "route_deleted",
            None,
            None,
            json!({"route_id": id}),
        );
        Ok(())
    }

    pub fn application_outbox(&self, application: Option<&str>, limit: usize) -> Vec<SdsMessageRecord> {
        let state = self.0.lock().expect("SDS router state poisoned");
        let mut messages: Vec<_> = state
            .messages
            .values()
            .filter(|message| {
                message.application_legs.iter().any(|leg| {
                    matches!(leg.state, LegState::Pending | LegState::RetryWaiting)
                        && application.is_none_or(|name| leg.application == name)
                })
            })
            .cloned()
            .collect();
        messages.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.created_at.cmp(&b.created_at)));
        messages.truncate(limit.min(2_000));
        messages
    }

    pub fn acknowledge_application(
        &self,
        message_id: &str,
        application: &str,
        input: ApplicationAckInput,
    ) -> Result<SdsMessageRecord, String> {
        let mut state = self.0.lock().expect("SDS router state poisoned");
        let message = state.messages.get_mut(message_id).ok_or_else(|| "message not found".to_string())?;
        let leg = message
            .application_legs
            .iter_mut()
            .find(|leg| leg.application == application)
            .ok_or_else(|| "application leg not found".to_string())?;
        leg.state = if input.success { LegState::Delivered } else { LegState::Failed };
        leg.completed_at = Some(now_iso());
        leg.last_error = (!input.success).then_some(input.message.clone());
        push_trace(
            message,
            if input.success { "application_ack" } else { "application_fail" },
            &format!("{application}: {}", input.message),
        );
        update_message_state(message);
        persist_locked(&state)?;
        push_event_locked(
            &mut state,
            "application_acknowledged",
            Some(message_id.to_string()),
            None,
            json!({"application": application, "success": input.success}),
        );
        Ok(state.messages.get(message_id).cloned().expect("message still present"))
    }

    pub fn gateway_connected(&self) {
        let mut state = self.0.lock().expect("SDS router state poisoned");
        state.gateway_connected = true;
        state.gateway_last_error = None;
        push_event_locked(&mut state, "gateway_connected", None, None, json!({}));
    }

    pub fn gateway_disconnected(&self, error: String) {
        let mut state = self.0.lock().expect("SDS router state poisoned");
        state.gateway_connected = false;
        state.gateway_last_error = Some(error.clone());
        push_event_locked(
            &mut state,
            "gateway_disconnected",
            None,
            None,
            json!({"error": error}),
        );
    }

    pub fn handle_backend_event(&self, event: BackendEvent) -> Vec<BackendRequest> {
        let mut state = self.0.lock().expect("SDS router state poisoned");
        match event {
            BackendEvent::Snapshot { snapshot } => apply_snapshot_locked(&mut state, snapshot),
            BackendEvent::Event { event } => {
                if event.kind == "node_disconnected" {
                    if let Some(node_id) = event.node_id {
                        if let Some(node) = state.nodes.get_mut(&node_id) {
                            node.connected = false;
                        }
                        move_node_legs_offline_locked(&mut state, &node_id, "node disconnected");
                    }
                }
            }
            BackendEvent::NodeMessage { node_id, message } => {
                handle_node_message_locked(&mut state, &node_id, message)
            }
            BackendEvent::ActionResult {
                request_id,
                command_id,
                ok,
                message,
            } => handle_action_result_locked(&mut state, request_id, command_id, ok, message),
        }
        expire_locked(&mut state);
        refresh_offline_locked(&mut state);
        let requests = collect_due_requests_locked(&mut state);
        let _ = persist_locked(&state);
        requests
    }

    pub fn tick(&self) -> Vec<BackendRequest> {
        let mut state = self.0.lock().expect("SDS router state poisoned");
        prune_dedupe_locked(&mut state);
        expire_locked(&mut state);
        refresh_offline_locked(&mut state);
        let requests = collect_due_requests_locked(&mut state);
        let _ = persist_locked(&state);
        requests
    }

    pub fn metrics(&self) -> String {
        let status = self.status();
        format!(
            concat!(
                "# HELP netcore_sds_router_up Service liveness.\n",
                "# TYPE netcore_sds_router_up gauge\n",
                "netcore_sds_router_up 1\n",
                "# TYPE netcore_sds_router_messages gauge\n",
                "netcore_sds_router_messages {}\n",
                "# TYPE netcore_sds_router_queued gauge\n",
                "netcore_sds_router_queued {}\n",
                "# TYPE netcore_sds_router_offline gauge\n",
                "netcore_sds_router_offline {}\n",
                "# TYPE netcore_sds_router_in_flight gauge\n",
                "netcore_sds_router_in_flight {}\n",
                "# TYPE netcore_sds_router_delivered gauge\n",
                "netcore_sds_router_delivered {}\n",
                "# TYPE netcore_sds_router_dead_letter gauge\n",
                "netcore_sds_router_dead_letter {}\n",
                "# TYPE netcore_sds_router_duplicates_total counter\n",
                "netcore_sds_router_duplicates_total {}\n",
                "# TYPE netcore_sds_router_application_outbox gauge\n",
                "netcore_sds_router_application_outbox {}\n"
            ),
            status.messages_total,
            status.queued,
            status.offline,
            status.in_flight,
            status.delivered,
            status.dead_letter,
            status.duplicate_messages,
            status.application_outbox,
        )
    }
}

fn load_database(config: &SdsRouterConfig) -> Result<SdsDatabase, Box<dyn std::error::Error>> {
    if !config.storage.database_path.exists() {
        return Ok(SdsDatabase {
            schema_version: DATABASE_SCHEMA_VERSION,
            revision: 0,
            messages: BTreeMap::new(),
            routes: BTreeMap::new(),
        });
    }
    let bytes = fs::read(&config.storage.database_path)?;
    let database: SdsDatabase = serde_json::from_slice(&bytes)?;
    if database.schema_version != DATABASE_SCHEMA_VERSION {
        return Err(format!(
            "unsupported SDS database schema {}; expected {}",
            database.schema_version, DATABASE_SCHEMA_VERSION
        )
        .into());
    }
    Ok(database)
}

fn persist_locked(state: &RouterState) -> Result<(), String> {
    let database = SdsDatabase {
        schema_version: DATABASE_SCHEMA_VERSION,
        revision: state.revision,
        messages: state.messages.clone(),
        routes: state.routes.clone(),
    };
    let path = &state.config.storage.database_path;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    if path.exists() {
        if let Some(parent) = state.config.storage.backup_path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let _ = fs::copy(path, &state.config.storage.backup_path);
    }
    let temp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(&database).map_err(|error| error.to_string())?;
    let mut file = fs::File::create(&temp).map_err(|error| error.to_string())?;
    file.write_all(&bytes).map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    fs::rename(&temp, path).map_err(|error| error.to_string())?;
    Ok(())
}

fn recover_incomplete_locked(state: &mut RouterState) {
    let now = now_iso();
    for message in state.messages.values_mut() {
        if matches!(message.state, MessageState::InFlight | MessageState::Queued) {
            message.state = MessageState::Queued;
            message.updated_at = now.clone();
            for leg in &mut message.delivery_legs {
                if matches!(leg.state, LegState::InFlight) {
                    leg.state = LegState::RetryWaiting;
                    leg.next_attempt_at = Some(now.clone());
                    leg.last_error = Some("recovered after SDS Router restart".to_string());
                    leg.command_id = None;
                    leg.handle = None;
                }
            }
            push_trace(message, "recovered", "incomplete delivery recovered after restart");
        }
    }
}

fn status_locked(state: &RouterState) -> SdsRouterStatus {
    let count = |needle| state.messages.values().filter(|message| message.state == needle).count();
    SdsRouterStatus {
        service: "netcore-sds-router",
        started_at: state.started_at.clone(),
        security_mode: "open_lab",
        warning: "NO AUTHENTICATION, NO TOKENS, NO TLS - ISOLATED TEST NETWORK ONLY",
        node_gateway_connected: state.gateway_connected,
        node_gateway_last_error: state.gateway_last_error.clone(),
        database_revision: state.revision,
        messages_total: state.messages.len(),
        queued: count(MessageState::Queued),
        offline: count(MessageState::Offline),
        in_flight: count(MessageState::InFlight),
        delivered: count(MessageState::Delivered),
        failed: count(MessageState::Failed),
        dead_letter: count(MessageState::DeadLetter),
        routes_total: state.routes.len(),
        nodes_connected: state.nodes.values().filter(|node| node.connected && !node.stale).count(),
        subscribers_known: state.subscribers.len(),
        groups_known: state.group_nodes.len(),
        application_outbox: state
            .messages
            .values()
            .flat_map(|message| &message.application_legs)
            .filter(|leg| matches!(leg.state, LegState::Pending | LegState::RetryWaiting))
            .count(),
        duplicate_messages: state.duplicate_messages,
        authoritative_ingress: state.config.routing.authoritative_ingress,
    }
}

fn message_from_input_locked(state: &RouterState, input: MessageInput) -> Result<SdsMessageRecord, String> {
    validate_ssi(input.source_issi, "source_issi")?;
    validate_ssi(input.dest_issi, "dest_issi")?;
    if input.priority > 15 {
        return Err("priority must be 0..=15".to_string());
    }
    if input.sds_type > 4 {
        return Err("sds_type must be 0..=4".to_string());
    }
    let ttl = input
        .ttl_secs
        .unwrap_or(state.config.routing.default_ttl_secs)
        .clamp(5, state.config.routing.max_ttl_secs);
    let now = Utc::now();
    let created_at = now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let expires_at = (now + ChronoDuration::seconds(ttl as i64))
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

    let (sds_type, protocol_id, status_code, len_bits, payload, text_preview) = if input.sds_type == 0 || input.status_code.is_some() {
        let status = input.status_code.ok_or_else(|| "status_code is required for sds_type=0".to_string())?;
        (0, 0, Some(status), 16, status.to_be_bytes().to_vec(), format!("Status {status}"))
    } else if !input.payload_hex.trim().is_empty() {
        let payload = decode_hex(&input.payload_hex)?;
        if payload.len() > state.config.limits.max_payload_bytes {
            return Err(format!("payload exceeds {} bytes", state.config.limits.max_payload_bytes));
        }
        let required = match input.sds_type {
            1 => 16,
            2 => 32,
            3 => 64,
            4 => input.len_bits.unwrap_or((payload.len() * 8) as u16),
            _ => return Err("invalid sds_type".to_string()),
        };
        let required_bytes = (required as usize).div_ceil(8);
        if required == 0 {
            return Err("len_bits must be greater than zero".to_string());
        }
        if payload.len() != required_bytes {
            return Err(format!(
                "payload length does not match SDS type/len_bits: expected {required_bytes} byte(s), got {}",
                payload.len()
            ));
        }
        let protocol = payload.first().copied().unwrap_or(input.protocol_id);
        let preview = decode_text_preview(protocol, &payload);
        (input.sds_type, protocol, None, required, payload, preview)
    } else {
        if input.sds_type != 4 {
            return Err("text composition is only supported for SDS Type 4".to_string());
        }
        let mut payload = vec![input.protocol_id, 0x04, next_message_reference(), 0x01];
        payload.extend_from_slice(input.text.as_bytes());
        if payload.len() > state.config.limits.max_payload_bytes {
            return Err(format!("payload exceeds {} bytes", state.config.limits.max_payload_bytes));
        }
        let len_bits = (payload.len() * 8) as u16;
        (4, input.protocol_id, None, len_bits, payload, input.text.trim().to_string())
    };

    let message_reference = extract_message_reference(sds_type, protocol_id, &payload);
    Ok(SdsMessageRecord {
        id: Uuid::new_v4().to_string(),
        ingress_node: None,
        ingress: if input.ingress.trim().is_empty() { "manual".to_string() } else { input.ingress.trim().to_string() },
        source_issi: input.source_issi,
        dest_issi: input.dest_issi,
        is_group: input.is_group,
        sds_type,
        protocol_id,
        status_code,
        len_bits,
        payload,
        text_preview,
        priority: input.priority,
        state: MessageState::Received,
        created_at: created_at.clone(),
        updated_at: created_at,
        expires_at,
        ttl_secs: ttl,
        duplicate_count: 0,
        message_reference,
        terminal_report: None,
        delivery_legs: Vec::new(),
        application_legs: Vec::new(),
        last_error: None,
        trace: Vec::new(),
    })
}

fn validate_route_input(state: &RouterState, input: &RouteInput) -> Result<(), String> {
    if input.target.trim().is_empty() {
        return Err("target must not be empty".to_string());
    }
    match input.kind {
        RouteKind::Protocol if input.match_value > 255 => {
            return Err("protocol route match_value must be 0..=255".to_string());
        }
        RouteKind::Individual | RouteKind::Group => validate_ssi(input.match_value, "match_value")?,
        RouteKind::Protocol => {}
    }
    if input.target_kind == RouteTargetKind::Node
        && !state.nodes.is_empty()
        && !state.nodes.contains_key(input.target.trim())
    {
        return Err("target node is not known to the Node Gateway".to_string());
    }
    if input.kind == RouteKind::Protocol && input.target_kind == RouteTargetKind::Node {
        return Err("protocol routes target applications; use individual/group routes for nodes".to_string());
    }
    Ok(())
}

fn apply_snapshot_locked(state: &mut RouterState, snapshot: GatewaySnapshot) {
    let mut seen = BTreeSet::new();
    for node in snapshot.nodes {
        seen.insert(node.node_id.clone());
        state.nodes.insert(
            node.node_id.clone(),
            NodeRecord {
                node_id: node.node_id,
                station_name: node.identity.station_name,
                site: node.identity.site,
                connected: node.connected,
                stale: node.stale,
                last_seen: node.last_seen,
                sds_capable: node.capabilities.sds,
                raw_sds_capable: node.capabilities.raw_sds,
                mcc: node.identity.mcc,
                mnc: node.identity.mnc,
                location_area: node.identity.location_area,
            },
        );
    }
    for (node_id, node) in &mut state.nodes {
        if !seen.contains(node_id) {
            node.connected = false;
        }
    }
}

fn handle_node_message_locked(state: &mut RouterState, node_id: &str, message: NodeToControlRoomMessage) {
    match message {
        NodeToControlRoomMessage::Telemetry { envelope } => {
            handle_telemetry_locked(state, node_id, envelope.timestamp, envelope.event)
        }
        NodeToControlRoomMessage::ControlAck { ack } => {
            if let Some(pending) = state.command_map.get(&ack.command_id).map(clone_pending)
                && !ack.accepted
            {
                state.command_map.remove(&ack.command_id);
                state.handle_map.remove(&pending.handle);
                state
                    .request_map
                    .retain(|_, request| request.handle != pending.handle);
                fail_leg_locked(
                    state,
                    &pending.message_id,
                    &pending.node_id,
                    format!("TBS rejected command: {}", ack.message),
                );
            }
        }
        NodeToControlRoomMessage::ControlResponse { envelope } => match envelope.response {
            ControlResponse::SdsDeliveryResponse { handle, success, message } => {
                complete_handle_locked(state, handle, success, message)
            }
            ControlResponse::SendSdsResponse { handle, success } => complete_handle_locked(
                state,
                handle,
                success,
                if success { "legacy SDS accepted".to_string() } else { "legacy SDS rejected".to_string() },
            ),
            _ => {}
        },
        _ => {}
    }
}

fn handle_telemetry_locked(state: &mut RouterState, node_id: &str, timestamp: String, event: TelemetryEvent) {
    match event {
        TelemetryEvent::MsRegistration { issi } => {
            state.subscribers.insert(
                issi,
                SubscriberLocation { issi, node_id: node_id.to_string(), last_seen: timestamp },
            );
            push_event_locked(state, "subscriber_registered", None, Some(node_id.to_string()), json!({"issi": issi}));
        }
        TelemetryEvent::MsDeregistration { issi } | TelemetryEvent::MsTimeoutDrop { issi } => {
            if state.subscribers.get(&issi).is_some_and(|location| location.node_id == node_id) {
                state.subscribers.remove(&issi);
            }
            remove_subscriber_groups_locked(state, node_id, issi);
            push_event_locked(state, "subscriber_deregistered", None, Some(node_id.to_string()), json!({"issi": issi}));
        }
        TelemetryEvent::MsGroupAttach { issi, gssis } => {
            let entry = state.node_groups.entry(node_id.to_string()).or_default().entry(issi).or_default();
            for gssi in gssis {
                entry.insert(gssi);
                state.group_nodes.entry(gssi).or_default().insert(node_id.to_string());
            }
        }
        TelemetryEvent::MsGroupsSnapshot { issi, gssis } => {
            remove_subscriber_groups_locked(state, node_id, issi);
            let entry = state.node_groups.entry(node_id.to_string()).or_default().entry(issi).or_default();
            for gssi in gssis {
                entry.insert(gssi);
                state.group_nodes.entry(gssi).or_default().insert(node_id.to_string());
            }
        }
        TelemetryEvent::MsGroupDetach { issi, gssis } => {
            if let Some(groups) = state.node_groups.get_mut(node_id).and_then(|members| members.get_mut(&issi)) {
                for gssi in gssis {
                    groups.remove(&gssi);
                }
            }
            rebuild_group_nodes_locked(state);
        }
        TelemetryEvent::SdsEdgeIngress {
            message_id,
            ingress,
            source_issi,
            dest_issi,
            is_group,
            sds_type,
            protocol_id,
            len_bits,
            payload,
            priority,
        } => ingest_edge_message_locked(
            state,
            node_id,
            message_id,
            ingress,
            source_issi,
            dest_issi,
            is_group,
            sds_type,
            protocol_id,
            len_bits,
            payload,
            priority,
            timestamp,
        ),
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn ingest_edge_message_locked(
    state: &mut RouterState,
    node_id: &str,
    message_id: String,
    ingress: String,
    source_issi: u32,
    dest_issi: u32,
    is_group: bool,
    sds_type: u8,
    protocol_id: u8,
    len_bits: u16,
    payload: Vec<u8>,
    priority: u8,
    timestamp: String,
) {
    if state.messages.contains_key(&message_id) {
        if let Some(existing) = state.messages.get_mut(&message_id) {
            existing.duplicate_count = existing.duplicate_count.saturating_add(1);
        }
        state.duplicate_messages = state.duplicate_messages.saturating_add(1);
        return;
    }
    let fingerprint = fingerprint_message(source_issi, dest_issi, is_group, sds_type, len_bits, &payload);
    if let Some((existing_id, seen_at)) = state.dedupe.get(&fingerprint).cloned() {
        if seconds_between(&seen_at, &timestamp).is_some_and(|seconds| seconds <= state.config.routing.dedupe_window_secs) {
            if let Some(existing) = state.messages.get_mut(&existing_id) {
                existing.duplicate_count = existing.duplicate_count.saturating_add(1);
                push_trace(existing, "duplicate", &format!("duplicate observed at node {node_id}"));
            }
            state.duplicate_messages = state.duplicate_messages.saturating_add(1);
            push_event_locked(
                state,
                "message_duplicate",
                Some(existing_id),
                Some(node_id.to_string()),
                json!({"fingerprint": fingerprint}),
            );
            return;
        }
    }
    state.dedupe.insert(fingerprint, (message_id.clone(), timestamp.clone()));

    let ttl = state.config.routing.default_ttl_secs;
    let created = DateTime::parse_from_rfc3339(&timestamp)
        .map(|value| value.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());
    let status_code = (sds_type == 0 && payload.len() >= 2).then(|| u16::from_be_bytes([payload[0], payload[1]]));
    let mut record = SdsMessageRecord {
        id: message_id.clone(),
        ingress_node: Some(node_id.to_string()),
        ingress,
        source_issi,
        dest_issi,
        is_group,
        sds_type,
        protocol_id,
        status_code,
        len_bits,
        text_preview: if sds_type == 0 {
            status_code.map(|value| format!("Status {value}")).unwrap_or_else(|| "Status".to_string())
        } else {
            decode_text_preview(protocol_id, &payload)
        },
        priority: priority.min(15),
        state: MessageState::Received,
        created_at: created.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        updated_at: now_iso(),
        expires_at: (created + ChronoDuration::seconds(ttl as i64))
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        ttl_secs: ttl,
        duplicate_count: 0,
        message_reference: extract_message_reference(sds_type, protocol_id, &payload),
        terminal_report: None,
        delivery_legs: Vec::new(),
        application_legs: Vec::new(),
        last_error: None,
        trace: Vec::new(),
        payload,
    };
    push_trace(&mut record, "ingress", &format!("received from TBS {node_id}"));
    correlate_terminal_report_locked(state, &record);
    state.messages.insert(message_id.clone(), record);
    state.revision = state.revision.saturating_add(1);
    plan_message_locked(state, &message_id, &[]);
    push_event_locked(
        state,
        "message_ingress",
        Some(message_id),
        Some(node_id.to_string()),
        json!({"source_issi": source_issi, "dest_issi": dest_issi, "group": is_group}),
    );
}

fn correlate_terminal_report_locked(state: &mut RouterState, report: &SdsMessageRecord) {
    let report_data = if report.sds_type == 4
        && matches!(report.protocol_id, 0x82 | 0x89)
        && report.payload.len() >= 4
        && report.payload[1] == 0x10
    {
        Some((report.payload[3], report.payload[2] as u16))
    } else if report.sds_type == 0 {
        report.status_code.and_then(|raw| {
            (31_744..=32_767).contains(&raw).then_some(((raw & 0xFF) as u8, ((raw >> 8) & 0x03) as u16))
        })
    } else {
        None
    };
    let Some((message_reference, status)) = report_data else {
        return;
    };
    let candidate = state
        .messages
        .values()
        .filter(|message| {
            message.source_issi == report.dest_issi
                && message.dest_issi == report.source_issi
                && message.message_reference == Some(message_reference)
        })
        .max_by(|a, b| a.created_at.cmp(&b.created_at))
        .map(|message| message.id.clone());
    if let Some(id) = candidate
        && let Some(original) = state.messages.get_mut(&id)
    {
        original.terminal_report = Some(TerminalReport {
            received_at: report.created_at.clone(),
            source_issi: report.source_issi,
            status,
            message_reference,
        });
        push_trace(
            original,
            "terminal_report",
            &format!("MR={message_reference} status={status}"),
        );
    }
}

fn plan_message_locked(state: &mut RouterState, id: &str, force_nodes: &[String]) {
    let Some(snapshot) = state.messages.get(id).cloned() else {
        return;
    };
    if is_expired(&snapshot.expires_at) {
        if let Some(message) = state.messages.get_mut(id) {
            message.state = MessageState::Expired;
            message.last_error = Some("TTL expired before routing".to_string());
        }
        return;
    }

    let matching_routes: Vec<_> = state
        .routes
        .values()
        .filter(|route| route.enabled)
        .filter(|route| match route.kind {
            RouteKind::Protocol => route.match_value == snapshot.protocol_id as u32,
            RouteKind::Individual => !snapshot.is_group && route.match_value == snapshot.dest_issi,
            RouteKind::Group => snapshot.is_group && route.match_value == snapshot.dest_issi,
        })
        .cloned()
        .collect();

    let mut application_legs = Vec::new();
    let intercept = matching_routes.iter().any(|route| {
        route.target_kind == RouteTargetKind::Application && route.mode == RouteMode::Intercept
    });
    for route in matching_routes.iter().filter(|route| route.target_kind == RouteTargetKind::Application) {
        application_legs.push(ApplicationLeg {
            application: route.target.clone(),
            route_id: route.id.clone(),
            mode: route.mode,
            state: LegState::Pending,
            queued_at: now_iso(),
            completed_at: None,
            last_error: None,
        });
    }

    let mut target_nodes = BTreeSet::new();
    if !intercept {
        for node_id in force_nodes {
            target_nodes.insert(node_id.clone());
        }
        for route in matching_routes.iter().filter(|route| route.target_kind == RouteTargetKind::Node) {
            target_nodes.insert(route.target.clone());
        }
        if target_nodes.is_empty() {
            if snapshot.is_group {
                if let Some(nodes) = state.group_nodes.get(&snapshot.dest_issi) {
                    target_nodes.extend(nodes.iter().cloned());
                }
            } else if let Some(location) = state.subscribers.get(&snapshot.dest_issi) {
                target_nodes.insert(location.node_id.clone());
            }
        }
    }

    let now = now_iso();
    let max_attempts = state.config.routing.max_attempts;
    let delivery_legs: Vec<_> = target_nodes
        .into_iter()
        .map(|node_id| DeliveryLeg {
            node_id,
            state: LegState::Pending,
            attempts: 0,
            max_attempts,
            handle: None,
            command_id: None,
            queued_at: now.clone(),
            last_attempt_at: None,
            next_attempt_at: Some(now.clone()),
            completed_at: None,
            last_error: None,
        })
        .collect();

    if let Some(message) = state.messages.get_mut(id) {
        if message.delivery_legs.is_empty() {
            message.delivery_legs = delivery_legs;
        }
        if message.application_legs.is_empty() {
            message.application_legs = application_legs;
        }
        if message.delivery_legs.is_empty() && message.application_legs.is_empty() {
            message.state = MessageState::Offline;
            message.last_error = Some("no serving TBS or matching application route".to_string());
            push_trace(message, "offline", "no route resolved; stored for later delivery");
        } else {
            message.state = MessageState::Queued;
            message.last_error = None;
            push_trace(
                message,
                "planned",
                &format!(
                    "{} TBS leg(s), {} application leg(s)",
                    message.delivery_legs.len(),
                    message.application_legs.len()
                ),
            );
        }
        message.updated_at = now;
    }
}

fn collect_due_requests_locked(state: &mut RouterState) -> Vec<BackendRequest> {
    let now = Utc::now();
    let mut due = Vec::new();
    let mut ids: Vec<(u8, String, String)> = state
        .messages
        .values()
        .map(|message| (message.priority, message.created_at.clone(), message.id.clone()))
        .collect();
    ids.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    for (_, _, id) in ids {
        let Some(message_snapshot) = state.messages.get(&id).cloned() else {
            continue;
        };
        if !matches!(message_snapshot.state, MessageState::Queued | MessageState::InFlight | MessageState::Partial) {
            continue;
        }
        if is_expired(&message_snapshot.expires_at) {
            continue;
        }
        let leg_nodes: Vec<String> = message_snapshot
            .delivery_legs
            .iter()
            .filter(|leg| matches!(leg.state, LegState::Pending | LegState::RetryWaiting))
            .filter(|leg| leg.next_attempt_at.as_deref().is_none_or(|at| parse_time(at).is_none_or(|at| at <= now)))
            .map(|leg| leg.node_id.clone())
            .collect();
        for node_id in leg_nodes {
            if !node_available(state, &node_id) {
                continue;
            }
            let handle = allocate_handle_locked(state);
            let request_id = format!("sds:{}:{}:{}", id, node_id, handle);
            let command = build_delivery_command(&message_snapshot, handle);
            let pending = PendingRequest { message_id: id.clone(), node_id: node_id.clone(), handle };
            state.request_map.insert(request_id.clone(), pending);
            state.handle_map.insert(handle, PendingRequest { message_id: id.clone(), node_id: node_id.clone(), handle });
            if let Some(message) = state.messages.get_mut(&id) {
                let attempt = if let Some(leg) = message
                    .delivery_legs
                    .iter_mut()
                    .find(|leg| leg.node_id == node_id)
                {
                    leg.state = LegState::InFlight;
                    leg.attempts = leg.attempts.saturating_add(1);
                    leg.handle = Some(handle);
                    leg.last_attempt_at = Some(now_iso());
                    leg.next_attempt_at = None;
                    leg.last_error = None;
                    Some(leg.attempts)
                } else {
                    None
                };
                if let Some(attempt) = attempt {
                    message.state = MessageState::InFlight;
                    message.updated_at = now_iso();
                    push_trace(
                        message,
                        "dispatch",
                        &format!("queued for TBS {node_id}, attempt {attempt}"),
                    );
                }
            }
            due.push(BackendRequest::Command {
                request_id: Some(request_id),
                node_id,
                command,
                operator_id: Some("sds-router-open-lab".to_string()),
            });
        }
    }
    due
}

fn build_delivery_command(message: &SdsMessageRecord, handle: u32) -> ControlCommand {
    if message.sds_type == 0 {
        ControlCommand::SendStatus {
            handle,
            source_ssi: message.source_issi,
            dest_ssi: message.dest_issi,
            pre_coded_status: message.status_code.unwrap_or_else(|| {
                if message.payload.len() >= 2 {
                    u16::from_be_bytes([message.payload[0], message.payload[1]])
                } else {
                    0
                }
            }),
        }
    } else {
        ControlCommand::DeliverSds {
            handle,
            source_ssi: message.source_issi,
            dest_ssi: message.dest_issi,
            dest_is_group: message.is_group,
            sds_type: message.sds_type,
            len_bits: message.len_bits,
            payload: message.payload.clone(),
        }
    }
}

fn handle_action_result_locked(
    state: &mut RouterState,
    request_id: Option<String>,
    command_id: Option<String>,
    ok: bool,
    message: String,
) {
    let Some(request_id) = request_id else {
        return;
    };
    let Some(pending) = state.request_map.remove(&request_id) else {
        return;
    };
    if ok {
        if let Some(command_id) = command_id {
            state.command_map.insert(command_id.clone(), clone_pending(&pending));
            if let Some(record) = state.messages.get_mut(&pending.message_id)
                && let Some(leg) = record.delivery_legs.iter_mut().find(|leg| leg.node_id == pending.node_id)
            {
                leg.command_id = Some(command_id);
            }
        }
    } else {
        state.handle_map.remove(&pending.handle);
        state
            .command_map
            .retain(|_, command| command.handle != pending.handle);
        fail_leg_locked(state, &pending.message_id, &pending.node_id, message);
    }
}

fn complete_handle_locked(state: &mut RouterState, handle: u32, success: bool, detail: String) {
    let Some(pending) = state.handle_map.remove(&handle) else {
        return;
    };
    state.command_map.retain(|_, value| value.handle != handle);
    let retry_config = state.config.clone();
    let Some(message) = state.messages.get_mut(&pending.message_id) else {
        return;
    };
    let trace_kind = {
        let Some(leg) = message
            .delivery_legs
            .iter_mut()
            .find(|leg| leg.node_id == pending.node_id)
        else {
            return;
        };
        leg.completed_at = success.then(now_iso);
        leg.last_error = (!success).then_some(detail.clone());
        leg.handle = None;
        leg.command_id = None;
        leg.state = if success {
            LegState::Delivered
        } else {
            LegState::RetryWaiting
        };
        if success {
            leg.next_attempt_at = None;
            "edge_accepted"
        } else if leg.attempts >= leg.max_attempts {
            leg.state = LegState::Failed;
            leg.next_attempt_at = None;
            "delivery_failed"
        } else {
            let delay = retry_delay_secs(&retry_config, leg.attempts);
            leg.next_attempt_at = Some(
                (Utc::now() + ChronoDuration::seconds(delay as i64))
                    .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            );
            "retry_wait"
        }
    };
    let trace_detail = if success {
        format!("TBS {} accepted delivery", pending.node_id)
    } else {
        format!("TBS {}: {detail}", pending.node_id)
    };
    push_trace(message, trace_kind, &trace_detail);
    message.updated_at = now_iso();
    update_message_state(message);
    push_event_locked(
        state,
        if success { "delivery_accepted" } else { "delivery_retry" },
        Some(pending.message_id),
        Some(pending.node_id),
        json!({"handle": handle, "success": success, "detail": detail}),
    );
}

fn fail_leg_locked(state: &mut RouterState, message_id: &str, node_id: &str, error: String) {
    let delay_config = state.config.clone();
    let Some(message) = state.messages.get_mut(message_id) else {
        return;
    };
    {
        let Some(leg) = message
            .delivery_legs
            .iter_mut()
            .find(|leg| leg.node_id == node_id)
        else {
            return;
        };
        leg.last_error = Some(error.clone());
        leg.command_id = None;
        leg.handle = None;
        if leg.attempts >= leg.max_attempts {
            leg.state = LegState::Failed;
            leg.next_attempt_at = None;
        } else {
            leg.state = LegState::RetryWaiting;
            let delay = retry_delay_secs(&delay_config, leg.attempts.max(1));
            leg.next_attempt_at = Some(
                (Utc::now() + ChronoDuration::seconds(delay as i64))
                    .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            );
        }
    }
    message.updated_at = now_iso();
    message.last_error = Some(error.clone());
    push_trace(message, "transport_error", &format!("TBS {node_id}: {error}"));
    update_message_state(message);
}

fn update_message_state(message: &mut SdsMessageRecord) {
    if matches!(message.state, MessageState::Cancelled | MessageState::Expired) {
        return;
    }
    let radio_total = message.delivery_legs.len();
    let radio_delivered = message.delivery_legs.iter().filter(|leg| leg.state == LegState::Delivered).count();
    let radio_failed = message.delivery_legs.iter().filter(|leg| leg.state == LegState::Failed).count();
    let radio_active = message
        .delivery_legs
        .iter()
        .any(|leg| matches!(leg.state, LegState::Pending | LegState::InFlight | LegState::RetryWaiting));
    let app_total = message.application_legs.len();
    let app_delivered = message.application_legs.iter().filter(|leg| leg.state == LegState::Delivered).count();
    let app_failed = message.application_legs.iter().filter(|leg| leg.state == LegState::Failed).count();
    let app_active = message
        .application_legs
        .iter()
        .any(|leg| matches!(leg.state, LegState::Pending | LegState::InFlight | LegState::RetryWaiting));

    if radio_total + app_total == 0 {
        message.state = MessageState::Offline;
    } else if radio_active || app_active {
        message.state = if message.delivery_legs.iter().any(|leg| leg.state == LegState::InFlight) {
            MessageState::InFlight
        } else {
            MessageState::Queued
        };
    } else if radio_delivered + app_delivered == radio_total + app_total {
        message.state = MessageState::Delivered;
        message.last_error = None;
    } else if radio_delivered + app_delivered > 0 {
        message.state = MessageState::Partial;
    } else if radio_failed + app_failed == radio_total + app_total {
        message.state = MessageState::DeadLetter;
    } else {
        message.state = MessageState::Failed;
    }
}

fn refresh_offline_locked(state: &mut RouterState) {
    let ids: Vec<String> = state
        .messages
        .values()
        .filter(|message| message.state == MessageState::Offline)
        .map(|message| message.id.clone())
        .collect();
    for id in ids {
        if is_expired(state.messages.get(&id).map(|message| message.expires_at.as_str()).unwrap_or("")) {
            continue;
        }
        plan_message_locked(state, &id, &[]);
    }
}

fn expire_locked(state: &mut RouterState) {
    let ids: Vec<String> = state
        .messages
        .values()
        .filter(|message| {
            !matches!(message.state, MessageState::Delivered | MessageState::Cancelled | MessageState::Expired)
                && is_expired(&message.expires_at)
        })
        .map(|message| message.id.clone())
        .collect();
    for id in ids {
        if let Some(message) = state.messages.get_mut(&id) {
            message.state = MessageState::Expired;
            message.updated_at = now_iso();
            message.last_error = Some("message TTL expired".to_string());
            for leg in &mut message.delivery_legs {
                if !matches!(leg.state, LegState::Delivered) {
                    leg.state = LegState::Expired;
                }
            }
            for leg in &mut message.application_legs {
                if !matches!(leg.state, LegState::Delivered) {
                    leg.state = LegState::Expired;
                }
            }
            push_trace(message, "expired", "TTL expired");
        }
        push_event_locked(state, "message_expired", Some(id), None, json!({}));
    }
}

fn move_node_legs_offline_locked(state: &mut RouterState, node_id: &str, reason: &str) {
    for message in state.messages.values_mut() {
        for leg in &mut message.delivery_legs {
            if leg.node_id == node_id && matches!(leg.state, LegState::Pending | LegState::InFlight) {
                leg.state = LegState::RetryWaiting;
                leg.next_attempt_at = Some(now_iso());
                leg.last_error = Some(reason.to_string());
                leg.command_id = None;
                leg.handle = None;
            }
        }
        update_message_state(message);
    }
    state.command_map.retain(|_, pending| pending.node_id != node_id);
    state.handle_map.retain(|_, pending| pending.node_id != node_id);
    state.request_map.retain(|_, pending| pending.node_id != node_id);
}

fn remove_subscriber_groups_locked(state: &mut RouterState, node_id: &str, issi: u32) {
    if let Some(members) = state.node_groups.get_mut(node_id) {
        members.remove(&issi);
    }
    rebuild_group_nodes_locked(state);
}

fn rebuild_group_nodes_locked(state: &mut RouterState) {
    state.group_nodes.clear();
    for (node_id, subscribers) in &state.node_groups {
        for groups in subscribers.values() {
            for gssi in groups {
                state.group_nodes.entry(*gssi).or_default().insert(node_id.clone());
            }
        }
    }
}

fn node_available(state: &RouterState, node_id: &str) -> bool {
    state.nodes.get(node_id).is_some_and(|node| node.connected && !node.stale && node.sds_capable)
}

fn retry_delay_secs(config: &SdsRouterConfig, attempts: u32) -> u64 {
    let factor = 1u64.checked_shl(attempts.saturating_sub(1).min(20)).unwrap_or(u64::MAX);
    config
        .routing
        .initial_retry_secs
        .saturating_mul(factor)
        .min(config.routing.max_retry_secs)
}

fn allocate_handle_locked(state: &mut RouterState) -> u32 {
    loop {
        let handle = state.next_handle.max(1);
        state.next_handle = state.next_handle.wrapping_add(1).max(1);
        if !state.handle_map.contains_key(&handle) {
            return handle;
        }
    }
}

fn prune_terminal_messages_locked(state: &mut RouterState) {
    let mut candidates: Vec<_> = state
        .messages
        .values()
        .filter(|message| {
            matches!(
                message.state,
                MessageState::Delivered
                    | MessageState::Expired
                    | MessageState::Cancelled
                    | MessageState::DeadLetter
            )
        })
        .map(|message| (message.created_at.clone(), message.id.clone()))
        .collect();
    candidates.sort();
    let remove_count = candidates.len().min((state.config.limits.max_messages / 10).max(1));
    for (_, id) in candidates.into_iter().take(remove_count) {
        state.messages.remove(&id);
    }
}

fn prune_dedupe_locked(state: &mut RouterState) {
    let window = state.config.routing.dedupe_window_secs.saturating_mul(2);
    state.dedupe.retain(|_, (_, timestamp)| seconds_since(timestamp).is_none_or(|age| age <= window));
}

fn summary(message: &SdsMessageRecord, mask_payload: bool) -> MessageSummary {
    MessageSummary {
        id: message.id.clone(),
        created_at: message.created_at.clone(),
        source_issi: message.source_issi,
        dest_issi: message.dest_issi,
        is_group: message.is_group,
        sds_type: message.sds_type,
        protocol_id: message.protocol_id,
        priority: message.priority,
        state: message.state,
        text_preview: if mask_payload && !message.text_preview.is_empty() {
            "••••••".to_string()
        } else {
            message.text_preview.clone()
        },
        payload_hex: if mask_payload {
            "masked".to_string()
        } else {
            hex(&message.payload)
        },
        expires_at: message.expires_at.clone(),
        delivered_legs: message.delivery_legs.iter().filter(|leg| leg.state == LegState::Delivered).count(),
        total_legs: message.delivery_legs.len(),
        application_pending: message
            .application_legs
            .iter()
            .filter(|leg| matches!(leg.state, LegState::Pending | LegState::RetryWaiting))
            .count(),
        last_error: message.last_error.clone(),
    }
}

fn push_trace(message: &mut SdsMessageRecord, kind: &str, detail: &str) {
    message.trace.push(TraceEntry {
        timestamp: now_iso(),
        kind: kind.to_string(),
        detail: detail.to_string(),
    });
    if message.trace.len() > 200 {
        let drain = message.trace.len() - 200;
        message.trace.drain(0..drain);
    }
}

fn push_event_locked(
    state: &mut RouterState,
    kind: &str,
    message_id: Option<String>,
    node_id: Option<String>,
    detail: Value,
) {
    state.events.push_back(SdsEventRecord {
        seq: state.next_event_seq,
        timestamp: now_iso(),
        kind: kind.to_string(),
        message_id,
        node_id,
        detail,
    });
    state.next_event_seq = state.next_event_seq.wrapping_add(1);
    while state.events.len() > state.config.server.history_limit {
        state.events.pop_front();
    }
}

fn validate_ssi(value: u32, name: &str) -> Result<(), String> {
    if value == 0 || value > 0xFF_FFFF {
        Err(format!("{name} must be 1..=16777215"))
    } else {
        Ok(())
    }
}

fn decode_hex(value: &str) -> Result<Vec<u8>, String> {
    let clean: String = value.chars().filter(|character| !character.is_whitespace() && *character != ':' && *character != '-').collect();
    if clean.len() % 2 != 0 {
        return Err("payload_hex must contain an even number of hex digits".to_string());
    }
    (0..clean.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&clean[index..index + 2], 16).map_err(|_| "payload_hex contains invalid hex".to_string()))
        .collect()
}

fn decode_text_preview(protocol_id: u8, payload: &[u8]) -> String {
    let body = match protocol_id {
        0x82 | 0x89 | 0x80 | 0x8A if payload.len() > 4 => &payload[4..],
        0x02 | 0x09 if payload.len() > 1 => &payload[1..],
        _ => return String::new(),
    };
    String::from_utf8_lossy(body)
        .chars()
        .filter(|character| !character.is_control() || *character == '\t')
        .collect::<String>()
        .trim()
        .to_string()
}

fn extract_message_reference(sds_type: u8, protocol_id: u8, payload: &[u8]) -> Option<u8> {
    (sds_type == 4
        && matches!(protocol_id, 0x82 | 0x89)
        && payload.len() >= 4
        && payload[1] != 0x10)
        .then_some(payload[2])
}

fn next_message_reference() -> u8 {
    use std::sync::atomic::{AtomicU8, Ordering};
    static NEXT: AtomicU8 = AtomicU8::new(1);
    let value = NEXT.fetch_add(1, Ordering::Relaxed);
    if value == 0 {
        NEXT.store(2, Ordering::Relaxed);
        1
    } else {
        value
    }
}

fn fingerprint_message(
    source_issi: u32,
    dest_issi: u32,
    is_group: bool,
    sds_type: u8,
    len_bits: u16,
    payload: &[u8],
) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    source_issi.hash(&mut hasher);
    dest_issi.hash(&mut hasher);
    is_group.hash(&mut hasher);
    sds_type.hash(&mut hasher);
    len_bits.hash(&mut hasher);
    payload.hash(&mut hasher);
    hasher.finish()
}

fn clone_pending(pending: &PendingRequest) -> PendingRequest {
    PendingRequest {
        message_id: pending.message_id.clone(),
        node_id: pending.node_id.clone(),
        handle: pending.handle,
    }
}

fn parse_time(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value).ok().map(|value| value.with_timezone(&Utc))
}

fn is_expired(value: &str) -> bool {
    parse_time(value).is_some_and(|time| time <= Utc::now())
}

fn seconds_since(value: &str) -> Option<u64> {
    let time = parse_time(value)?;
    Some(Utc::now().signed_duration_since(time).num_seconds().max(0) as u64)
}

fn seconds_between(older: &str, newer: &str) -> Option<u64> {
    let older = parse_time(older)?;
    let newer = parse_time(newer)?;
    Some(newer.signed_duration_since(older).num_seconds().unsigned_abs())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02X}")).collect::<Vec<_>>().join(" ")
}

pub fn parse_message_state(value: &str) -> Option<MessageState> {
    match value {
        "received" => Some(MessageState::Received),
        "queued" => Some(MessageState::Queued),
        "offline" => Some(MessageState::Offline),
        "in_flight" => Some(MessageState::InFlight),
        "delivered" => Some(MessageState::Delivered),
        "partial" => Some(MessageState::Partial),
        "failed" => Some(MessageState::Failed),
        "expired" => Some(MessageState::Expired),
        "cancelled" => Some(MessageState::Cancelled),
        "dead_letter" => Some(MessageState::DeadLetter),
        _ => None,
    }
}

pub fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_message_is_wrapped_as_sds_tl_type4() {
        let state = RouterState {
            config: SdsRouterConfig::default(),
            started_at: now_iso(),
            gateway_connected: false,
            gateway_last_error: None,
            nodes: BTreeMap::new(),
            subscribers: BTreeMap::new(),
            node_groups: BTreeMap::new(),
            group_nodes: BTreeMap::new(),
            messages: BTreeMap::new(),
            routes: BTreeMap::new(),
            revision: 0,
            events: VecDeque::new(),
            next_event_seq: 1,
            next_handle: 1,
            request_map: BTreeMap::new(),
            command_map: BTreeMap::new(),
            handle_map: BTreeMap::new(),
            dedupe: BTreeMap::new(),
            duplicate_messages: 0,
        };
        let message = message_from_input_locked(
            &state,
            MessageInput {
                source_issi: 4_010_001,
                dest_issi: 4_010_002,
                is_group: false,
                sds_type: 4,
                protocol_id: 0x82,
                status_code: None,
                len_bits: None,
                payload_hex: String::new(),
                text: "Hallo".to_string(),
                priority: 0,
                ttl_secs: None,
                ingress: String::new(),
                force_nodes: Vec::new(),
            },
        )
        .unwrap();
        assert_eq!(message.payload[0], 0x82);
        assert_eq!(message.payload[1], 0x04);
        assert_eq!(message.sds_type, 4);
        assert_eq!(message.text_preview, "Hallo");
    }

    #[test]
    fn fixed_size_sds_rejects_non_exact_payload_length() {
        let state = RouterState {
            config: SdsRouterConfig::default(),
            started_at: now_iso(),
            gateway_connected: false,
            gateway_last_error: None,
            nodes: BTreeMap::new(),
            subscribers: BTreeMap::new(),
            node_groups: BTreeMap::new(),
            group_nodes: BTreeMap::new(),
            messages: BTreeMap::new(),
            routes: BTreeMap::new(),
            revision: 0,
            events: VecDeque::new(),
            next_event_seq: 1,
            next_handle: 1,
            request_map: BTreeMap::new(),
            command_map: BTreeMap::new(),
            handle_map: BTreeMap::new(),
            dedupe: BTreeMap::new(),
            duplicate_messages: 0,
        };
        let error = message_from_input_locked(
            &state,
            MessageInput {
                source_issi: 4_010_001,
                dest_issi: 4_010_002,
                is_group: false,
                sds_type: 1,
                protocol_id: 0,
                status_code: None,
                len_bits: Some(16),
                payload_hex: "00 01 02".to_string(),
                text: String::new(),
                priority: 0,
                ttl_secs: None,
                ingress: String::new(),
                force_nodes: Vec::new(),
            },
        )
        .unwrap_err();
        assert!(error.contains("expected 2 byte(s)"));
    }

    #[test]
    fn retry_delay_is_bounded() {
        let config = SdsRouterConfig::default();
        assert_eq!(retry_delay_secs(&config, 1), 2);
        assert_eq!(retry_delay_secs(&config, 50), 60);
    }
}

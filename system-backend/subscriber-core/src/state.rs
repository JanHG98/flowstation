use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::fs;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tetra_entities::net_control::{ControlCommand, ControlResponse};
use tetra_entities::net_control_room::NodeToControlRoomMessage;
use tetra_entities::net_telemetry::TelemetryEvent;
use uuid::Uuid;

use crate::config::{POLICY_OPEN_NETWORK, SubscriberCoreConfig};
use crate::protocol::{BackendEvent, BackendRequest, GatewaySnapshot};

const DATABASE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize)]
pub struct SubscriberStatus {
    pub service: &'static str,
    pub started_at: String,
    pub security_mode: &'static str,
    pub warning: &'static str,
    pub node_gateway_connected: bool,
    pub node_gateway_last_error: Option<String>,
    pub database_path: String,
    pub database_revision: u64,
    pub subscribers_total: usize,
    pub subscribers_authorized: usize,
    pub subscribers_blocked: usize,
    pub observed_registered: usize,
    pub nodes_known: usize,
    pub nodes_connected: usize,
    pub nodes_synced: usize,
    pub nodes_out_of_sync: usize,
    pub access_mode: String,
    pub auto_sync: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeRecord {
    pub node_id: String,
    pub station_name: String,
    pub site: Option<String>,
    pub connected: bool,
    pub stale: bool,
    pub last_seen: String,
    pub subscriber_policy_capable: bool,
    pub mcc: u16,
    pub mnc: u16,
    pub location_area: u16,
    pub colour_code: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriberProfile {
    pub issi: u32,
    pub home_mcc: u16,
    pub home_mnc: u16,
    pub display_name: String,
    pub organization: String,
    pub device_label: String,
    pub device_tei: Option<u64>,
    pub enabled: bool,
    pub registration_allowed: bool,
    pub call_priority: u8,
    pub emergency_allowed: bool,
    pub sds_allowed: bool,
    pub packet_data_allowed: bool,
    pub default_groups: BTreeSet<u32>,
    pub notes: String,
    pub created_at: String,
    pub updated_at: String,
    pub revision: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubscriberInput {
    pub issi: u32,
    #[serde(default)]
    pub home_mcc: u16,
    #[serde(default)]
    pub home_mnc: u16,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub organization: String,
    #[serde(default)]
    pub device_label: String,
    #[serde(default)]
    pub device_tei: Option<u64>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub registration_allowed: bool,
    #[serde(default)]
    pub call_priority: u8,
    #[serde(default)]
    pub emergency_allowed: bool,
    #[serde(default = "default_true")]
    pub sds_allowed: bool,
    #[serde(default)]
    pub packet_data_allowed: bool,
    #[serde(default)]
    pub default_groups: BTreeSet<u32>,
    #[serde(default)]
    pub notes: String,
}

fn default_true() -> bool { true }

#[derive(Debug, Clone, Serialize)]
pub struct ObservedSubscriber {
    pub issi: u32,
    pub registered: bool,
    pub serving_node: Option<String>,
    pub groups: BTreeSet<u32>,
    pub energy_saving_mode: Option<u8>,
    pub last_rssi_dbfs: Option<f32>,
    pub first_seen: String,
    pub last_seen: String,
    pub known_profile: bool,
    pub authorized: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyncPhase {
    Pending,
    Requested,
    Applied,
    Failed,
    TimedOut,
    Offline,
    Unsupported,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncRecord {
    pub node_id: String,
    pub desired_revision: u64,
    pub applied_revision: Option<u64>,
    pub phase: SyncPhase,
    pub allow_all: bool,
    pub allowed_count: usize,
    pub disconnect_unauthorized: bool,
    pub requested_at: Option<String>,
    pub updated_at: String,
    pub request_id: Option<String>,
    pub command_id: Option<String>,
    pub message: Option<String>,
    #[serde(skip)]
    deadline: Option<Instant>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubscriberEventRecord {
    pub seq: u64,
    pub timestamp: String,
    pub kind: String,
    pub issi: Option<u32>,
    pub node_id: Option<String>,
    pub detail: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SubscriberDatabase {
    schema_version: u32,
    revision: u64,
    subscribers: Vec<SubscriberProfile>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImportRequest {
    #[serde(default)]
    pub replace: bool,
    pub subscribers: Vec<SubscriberInput>,
}

struct SubscriberState {
    config: SubscriberCoreConfig,
    started_at: String,
    gateway_connected: bool,
    gateway_last_error: Option<String>,
    database_revision: u64,
    subscribers: BTreeMap<u32, SubscriberProfile>,
    observed: HashMap<u32, ObservedSubscriber>,
    nodes: HashMap<String, NodeRecord>,
    syncs: HashMap<String, SyncRecord>,
    events: VecDeque<SubscriberEventRecord>,
    next_event_seq: u64,
    next_handle: u32,
}

#[derive(Clone)]
pub struct SharedSubscribers(Arc<Mutex<SubscriberState>>);

impl SharedSubscribers {
    pub fn load(config: SubscriberCoreConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let (database_revision, subscribers) = load_database(&config)?;
        let this = Self(Arc::new(Mutex::new(SubscriberState {
            config,
            started_at: now_iso(),
            gateway_connected: false,
            gateway_last_error: None,
            database_revision,
            subscribers,
            observed: HashMap::new(),
            nodes: HashMap::new(),
            syncs: HashMap::new(),
            events: VecDeque::new(),
            next_event_seq: 1,
            next_handle: 1,
        })));
        this.push_event("database_loaded", None, None, json!({"revision": database_revision}));
        Ok(this)
    }

    pub fn status(&self) -> SubscriberStatus {
        let state = self.0.lock().expect("subscriber state poisoned");
        let authorized = state.subscribers.values().filter(|p| profile_authorized(p)).count();
        let connected = state.nodes.values().filter(|n| n.connected && !n.stale).count();
        let synced = state.syncs.values().filter(|sync| {
            sync.phase == SyncPhase::Applied
                && sync.applied_revision == Some(state.database_revision)
                && state.nodes.get(&sync.node_id).is_some_and(|node| node.connected && !node.stale)
        }).count();
        SubscriberStatus {
            service: "netcore-subscriber-core",
            started_at: state.started_at.clone(),
            security_mode: "open_lab",
            warning: "OPEN LAB: no authentication, no token and no TLS",
            node_gateway_connected: state.gateway_connected,
            node_gateway_last_error: state.gateway_last_error.clone(),
            database_path: state.config.storage.database_path.display().to_string(),
            database_revision: state.database_revision,
            subscribers_total: state.subscribers.len(),
            subscribers_authorized: authorized,
            subscribers_blocked: state.subscribers.len().saturating_sub(authorized),
            observed_registered: state.observed.values().filter(|o| o.registered).count(),
            nodes_known: state.nodes.len(),
            nodes_connected: connected,
            nodes_synced: synced,
            nodes_out_of_sync: connected.saturating_sub(synced),
            access_mode: state.config.access_policy.mode.clone(),
            auto_sync: state.config.access_policy.auto_sync,
        }
    }

    pub fn nodes(&self) -> Vec<NodeRecord> {
        let state = self.0.lock().expect("subscriber state poisoned");
        let mut values: Vec<_> = state.nodes.values().cloned().collect();
        values.sort_by(|a,b| a.node_id.cmp(&b.node_id));
        values
    }

    pub fn subscribers(&self) -> Vec<SubscriberProfile> {
        self.0.lock().expect("subscriber state poisoned")
            .subscribers.values().cloned().collect()
    }

    pub fn subscriber(&self, issi: u32) -> Option<SubscriberProfile> {
        self.0.lock().expect("subscriber state poisoned")
            .subscribers.get(&issi).cloned()
    }

    pub fn observed(&self) -> Vec<ObservedSubscriber> {
        let state = self.0.lock().expect("subscriber state poisoned");
        let mut values: Vec<_> = state.observed.values().cloned().collect();
        values.sort_by_key(|item| item.issi);
        values
    }

    pub fn syncs(&self) -> Vec<SyncRecord> {
        let state = self.0.lock().expect("subscriber state poisoned");
        let mut values: Vec<_> = state.syncs.values().cloned().collect();
        values.sort_by(|a,b| a.node_id.cmp(&b.node_id));
        values
    }

    pub fn recent_events(&self, limit: usize) -> Vec<SubscriberEventRecord> {
        self.0.lock().expect("subscriber state poisoned")
            .events.iter().rev().take(limit).cloned().collect()
    }

    pub fn create_subscriber(&self, input: SubscriberInput) -> Result<(SubscriberProfile, Vec<BackendRequest>), String> {
        validate_input(&input, &self.0.lock().expect("subscriber state poisoned").config)?;
        let mut state = self.0.lock().expect("subscriber state poisoned");
        if state.subscribers.contains_key(&input.issi) {
            return Err(format!("ISSI {} already exists", input.issi));
        }
        if state.subscribers.len() >= state.config.limits.max_subscribers {
            return Err("subscriber limit reached".to_string());
        }
        state.database_revision = state.database_revision.saturating_add(1);
        let now = now_iso();
        let profile = profile_from_input(input, now.clone(), now, state.database_revision);
        state.subscribers.insert(profile.issi, profile.clone());
        persist_locked(&state)?;
        refresh_observed_authorization(&mut state, profile.issi);
        let revision = state.database_revision;
        push_event_locked(&mut state, "subscriber_created", Some(profile.issi), None, json!({"revision": revision}));
        let requests = maybe_sync_all_locked(&mut state);
        Ok((profile, requests))
    }

    pub fn update_subscriber(&self, issi: u32, mut input: SubscriberInput) -> Result<(SubscriberProfile, Vec<BackendRequest>), String> {
        input.issi = issi;
        validate_input(&input, &self.0.lock().expect("subscriber state poisoned").config)?;
        let mut state = self.0.lock().expect("subscriber state poisoned");
        let created_at = state.subscribers.get(&issi)
            .map(|profile| profile.created_at.clone())
            .ok_or_else(|| format!("ISSI {} not found", issi))?;
        state.database_revision = state.database_revision.saturating_add(1);
        let profile = profile_from_input(input, created_at, now_iso(), state.database_revision);
        state.subscribers.insert(issi, profile.clone());
        persist_locked(&state)?;
        refresh_observed_authorization(&mut state, issi);
        let revision = state.database_revision;
        push_event_locked(&mut state, "subscriber_updated", Some(issi), None, json!({"revision": revision}));
        let requests = maybe_sync_all_locked(&mut state);
        Ok((profile, requests))
    }

    pub fn delete_subscriber(&self, issi: u32) -> Result<Vec<BackendRequest>, String> {
        let mut state = self.0.lock().expect("subscriber state poisoned");
        if state.subscribers.remove(&issi).is_none() {
            return Err(format!("ISSI {} not found", issi));
        }
        state.database_revision = state.database_revision.saturating_add(1);
        persist_locked(&state)?;
        refresh_observed_authorization(&mut state, issi);
        let revision = state.database_revision;
        push_event_locked(&mut state, "subscriber_deleted", Some(issi), None, json!({"revision": revision}));
        Ok(maybe_sync_all_locked(&mut state))
    }

    pub fn import_subscribers(&self, request: ImportRequest) -> Result<(usize, Vec<BackendRequest>), String> {
        let config = self.0.lock().expect("subscriber state poisoned").config.clone();
        if request.subscribers.len() > config.limits.max_subscribers {
            return Err("import exceeds subscriber limit".to_string());
        }
        for input in &request.subscribers {
            validate_input(input, &config)?;
        }

        let mut state = self.0.lock().expect("subscriber state poisoned");
        let mut next = if request.replace {
            BTreeMap::new()
        } else {
            state.subscribers.clone()
        };
        if next.len().saturating_add(request.subscribers.len()) > state.config.limits.max_subscribers
            && !request.replace
        {
            return Err("resulting database exceeds subscriber limit".to_string());
        }

        let now = now_iso();
        let revision = state.database_revision.saturating_add(1);
        let mut imported = 0usize;
        for input in request.subscribers {
            let created = next.get(&input.issi)
                .map(|profile| profile.created_at.clone())
                .unwrap_or_else(|| now.clone());
            let profile = profile_from_input(input, created, now.clone(), revision);
            next.insert(profile.issi, profile);
            imported += 1;
        }
        if next.len() > state.config.limits.max_subscribers {
            return Err("resulting database exceeds subscriber limit".to_string());
        }

        state.subscribers = next;
        state.database_revision = revision;
        persist_locked(&state)?;
        let keys: Vec<u32> = state.observed.keys().copied().collect();
        for issi in keys {
            refresh_observed_authorization(&mut state, issi);
        }
        push_event_locked(
            &mut state,
            "subscribers_imported",
            None,
            None,
            json!({"count": imported, "replace": request.replace, "revision": revision}),
        );
        let requests = maybe_sync_all_locked(&mut state);
        Ok((imported, requests))
    }

    pub fn sync_all(&self) -> Vec<BackendRequest> {
        let mut state = self.0.lock().expect("subscriber state poisoned");
        schedule_sync_all_locked(&mut state)
    }

    pub fn export_database(&self) -> Value {
        let state = self.0.lock().expect("subscriber state poisoned");
        json!({
            "schema_version": DATABASE_SCHEMA_VERSION,
            "revision": state.database_revision,
            "subscribers": state.subscribers.values().collect::<Vec<_>>()
        })
    }

    pub fn export_csv(&self) -> String {
        let state = self.0.lock().expect("subscriber state poisoned");
        let mut out = String::from("issi,home_mcc,home_mnc,display_name,organization,device_label,enabled,registration_allowed,call_priority,emergency_allowed,sds_allowed,packet_data_allowed,default_groups,notes\n");
        for p in state.subscribers.values() {
            let groups = p.default_groups.iter().map(u32::to_string).collect::<Vec<_>>().join(";");
            let row = [
                p.issi.to_string(), p.home_mcc.to_string(), p.home_mnc.to_string(),
                csv(&p.display_name), csv(&p.organization), csv(&p.device_label),
                p.enabled.to_string(), p.registration_allowed.to_string(), p.call_priority.to_string(),
                p.emergency_allowed.to_string(), p.sds_allowed.to_string(), p.packet_data_allowed.to_string(),
                csv(&groups), csv(&p.notes),
            ];
            out.push_str(&row.join(",")); out.push('\n');
        }
        out
    }

    pub fn metrics(&self) -> String {
        let status = self.status();
        format!(
            "# HELP netcore_subscriber_profiles Subscriber profiles\n# TYPE netcore_subscriber_profiles gauge\nnetcore_subscriber_profiles {}\n# HELP netcore_subscriber_authorized Authorized subscriber profiles\n# TYPE netcore_subscriber_authorized gauge\nnetcore_subscriber_authorized {}\n# HELP netcore_subscriber_observed_registered Registered subscribers observed through TBS telemetry\n# TYPE netcore_subscriber_observed_registered gauge\nnetcore_subscriber_observed_registered {}\n# HELP netcore_subscriber_nodes_connected Connected TBS nodes\n# TYPE netcore_subscriber_nodes_connected gauge\nnetcore_subscriber_nodes_connected {}\n# HELP netcore_subscriber_nodes_synced TBS nodes on current policy revision\n# TYPE netcore_subscriber_nodes_synced gauge\nnetcore_subscriber_nodes_synced {}\n# HELP netcore_subscriber_database_revision Current database revision\n# TYPE netcore_subscriber_database_revision gauge\nnetcore_subscriber_database_revision {}\n",
            status.subscribers_total, status.subscribers_authorized, status.observed_registered,
            status.nodes_connected, status.nodes_synced, status.database_revision,
        )
    }

    pub fn gateway_connected(&self) {
        let mut state = self.0.lock().expect("subscriber state poisoned");
        state.gateway_connected = true;
        state.gateway_last_error = None;
        push_event_locked(&mut state, "gateway_connected", None, None, json!({}));
    }

    pub fn gateway_disconnected(&self, error: String) {
        let mut state = self.0.lock().expect("subscriber state poisoned");
        state.gateway_connected = false;
        state.gateway_last_error = Some(error.clone());
        for sync in state.syncs.values_mut() {
            if matches!(sync.phase, SyncPhase::Pending | SyncPhase::Requested) {
                sync.phase = SyncPhase::Offline;
                sync.message = Some("node gateway disconnected".to_string());
                sync.deadline = None;
                sync.updated_at = now_iso();
            }
        }
        push_event_locked(&mut state, "gateway_disconnected", None, None, json!({"error": error}));
    }

    pub fn handle_backend_event(&self, event: BackendEvent) -> Vec<BackendRequest> {
        let mut state = self.0.lock().expect("subscriber state poisoned");
        match event {
            BackendEvent::Snapshot { snapshot } => handle_snapshot_locked(&mut state, snapshot),
            BackendEvent::Event { event } => {
                push_event_locked(&mut state, "gateway_event", None, event.node_id, event.detail);
                Vec::new()
            }
            BackendEvent::NodeMessage { node_id, message } => {
                handle_node_message_locked(&mut state, &node_id, message);
                Vec::new()
            }
            BackendEvent::ActionResult { request_id, command_id, ok, message } => {
                handle_action_result_locked(&mut state, request_id, command_id, ok, message);
                Vec::new()
            }
        }
    }

    pub fn expire_syncs(&self) -> Vec<BackendRequest> {
        let mut state = self.0.lock().expect("subscriber state poisoned");
        let now = Instant::now();
        let mut timed_out = Vec::new();
        for sync in state.syncs.values_mut() {
            if matches!(sync.phase, SyncPhase::Pending | SyncPhase::Requested)
                && sync.deadline.is_some_and(|deadline| deadline <= now)
            {
                sync.phase = SyncPhase::TimedOut;
                sync.message = Some("policy synchronization timed out".to_string());
                sync.deadline = None;
                sync.updated_at = now_iso();
                timed_out.push(sync.node_id.clone());
            }
        }
        for node in timed_out {
            push_event_locked(&mut state, "policy_sync_timeout", None, Some(node), json!({}));
        }
        Vec::new()
    }

    fn push_event(&self, kind: &str, issi: Option<u32>, node: Option<String>, detail: Value) {
        let mut state = self.0.lock().expect("subscriber state poisoned");
        push_event_locked(&mut state, kind, issi, node, detail);
    }
}

fn load_database(config: &SubscriberCoreConfig) -> Result<(u64, BTreeMap<u32, SubscriberProfile>), Box<dyn std::error::Error>> {
    let path = &config.storage.database_path;
    if !path.exists() {
        if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
        return Ok((0, BTreeMap::new()));
    }
    let database: SubscriberDatabase = serde_json::from_str(&fs::read_to_string(path)?)?;
    if database.schema_version != DATABASE_SCHEMA_VERSION {
        return Err(format!("unsupported subscriber database schema {}", database.schema_version).into());
    }
    let mut profiles = BTreeMap::new();
    for profile in database.subscribers {
        if profile.issi == 0 || profile.issi > 0xFF_FFFF {
            return Err(format!("invalid ISSI {} in database", profile.issi).into());
        }
        profiles.insert(profile.issi, profile);
    }
    Ok((database.revision, profiles))
}

fn persist_locked(state: &SubscriberState) -> Result<(), String> {
    let database = SubscriberDatabase {
        schema_version: DATABASE_SCHEMA_VERSION,
        revision: state.database_revision,
        subscribers: state.subscribers.values().cloned().collect(),
    };
    let bytes = serde_json::to_vec_pretty(&database)
        .map_err(|error| format!("database serialization failed: {error}"))?;
    let path = &state.config.storage.database_path;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("database directory failed: {error}"))?;
    }
    if path.exists() {
        let _ = fs::copy(path, &state.config.storage.backup_path);
    }
    let tmp = path.with_extension(format!("tmp-{}", Uuid::new_v4()));
    {
        let mut file = fs::File::create(&tmp).map_err(|error| format!("temporary database create failed: {error}"))?;
        file.write_all(&bytes).map_err(|error| format!("temporary database write failed: {error}"))?;
        file.sync_all().map_err(|error| format!("temporary database sync failed: {error}"))?;
    }
    fs::rename(&tmp, path).map_err(|error| format!("database replace failed: {error}"))
}

fn validate_input(input: &SubscriberInput, config: &SubscriberCoreConfig) -> Result<(), String> {
    if input.issi == 0 || input.issi > 0xFF_FFFF { return Err("ISSI must be in 1..=16777215".to_string()); }
    if input.call_priority > 15 { return Err("call_priority must be in 0..=15".to_string()); }
    if input.default_groups.len() > config.limits.max_groups_per_subscriber { return Err("too many default groups".to_string()); }
    if input.default_groups.iter().any(|gssi| *gssi == 0 || *gssi > 0xFF_FFFF) { return Err("GSSI must be in 1..=16777215".to_string()); }
    if input.display_name.len() > 160 || input.organization.len() > 160 || input.device_label.len() > 160 || input.notes.len() > 4_096 { return Err("one or more text fields are too long".to_string()); }
    Ok(())
}

fn profile_from_input(input: SubscriberInput, created_at: String, updated_at: String, revision: u64) -> SubscriberProfile {
    SubscriberProfile {
        issi: input.issi,
        home_mcc: input.home_mcc,
        home_mnc: input.home_mnc,
        display_name: input.display_name.trim().to_string(),
        organization: input.organization.trim().to_string(),
        device_label: input.device_label.trim().to_string(),
        device_tei: input.device_tei,
        enabled: input.enabled,
        registration_allowed: input.registration_allowed,
        call_priority: input.call_priority,
        emergency_allowed: input.emergency_allowed,
        sds_allowed: input.sds_allowed,
        packet_data_allowed: input.packet_data_allowed,
        default_groups: input.default_groups,
        notes: input.notes.trim().to_string(),
        created_at,
        updated_at,
        revision,
    }
}

fn profile_authorized(profile: &SubscriberProfile) -> bool {
    profile.enabled && profile.registration_allowed
}

fn policy_values(state: &SubscriberState) -> (bool, Vec<u32>) {
    if state.config.access_policy.mode == POLICY_OPEN_NETWORK {
        (true, Vec::new())
    } else {
        (false, state.subscribers.values().filter(|p| profile_authorized(p)).map(|p| p.issi).collect())
    }
}

fn maybe_sync_all_locked(state: &mut SubscriberState) -> Vec<BackendRequest> {
    if state.config.access_policy.auto_sync { schedule_sync_all_locked(state) } else { Vec::new() }
}

fn schedule_sync_all_locked(state: &mut SubscriberState) -> Vec<BackendRequest> {
    let nodes: Vec<String> = state.nodes.values()
        .filter(|node| node.connected && !node.stale)
        .map(|node| node.node_id.clone()).collect();
    nodes.into_iter().filter_map(|node| schedule_sync_locked(state, &node)).collect()
}

fn schedule_sync_locked(state: &mut SubscriberState, node_id: &str) -> Option<BackendRequest> {
    let node = state.nodes.get(node_id)?;
    if !node.connected || node.stale { return None; }
    if !node.subscriber_policy_capable {
        state.syncs.insert(node_id.to_string(), SyncRecord {
            node_id: node_id.to_string(), desired_revision: state.database_revision, applied_revision: None,
            phase: SyncPhase::Unsupported, allow_all: false, allowed_count: 0,
            disconnect_unauthorized: false, requested_at: None, updated_at: now_iso(),
            request_id: None, command_id: None, message: Some("TBS does not advertise subscriber_policy capability".to_string()), deadline: None,
        });
        return None;
    }
    let (allow_all, allowed_issis) = policy_values(state);
    state.next_handle = state.next_handle.wrapping_add(1).max(1);
    let handle = state.next_handle;
    let request_id = format!("subscriber-policy:{}:{}:{}", node_id, state.database_revision, Uuid::new_v4());
    let now = now_iso();
    state.syncs.insert(node_id.to_string(), SyncRecord {
        node_id: node_id.to_string(), desired_revision: state.database_revision, applied_revision: None,
        phase: SyncPhase::Pending, allow_all, allowed_count: allowed_issis.len(),
        disconnect_unauthorized: state.config.access_policy.disconnect_unauthorized,
        requested_at: Some(now.clone()), updated_at: now, request_id: Some(request_id.clone()),
        command_id: None, message: Some("policy queued".to_string()),
        deadline: Some(Instant::now() + Duration::from_secs(state.config.access_policy.sync_timeout_secs)),
    });
    let revision = state.database_revision;
    push_event_locked(state, "policy_sync_queued", None, Some(node_id.to_string()), json!({"revision": revision, "allow_all": allow_all, "allowed_count": allowed_issis.len()}));
    Some(BackendRequest::Command {
        request_id: Some(request_id), node_id: node_id.to_string(),
        command: ControlCommand::SubscriberAccessPolicyApply {
            handle, revision: state.database_revision, allow_all, allowed_issis,
            disconnect_unauthorized: state.config.access_policy.disconnect_unauthorized,
        },
        operator_id: Some("subscriber-core/open-lab".to_string()),
    })
}

fn handle_snapshot_locked(state: &mut SubscriberState, snapshot: GatewaySnapshot) -> Vec<BackendRequest> {
    let mut newly_connected = Vec::new();
    for node in snapshot.nodes {
        let was_connected = state.nodes.get(&node.node_id).is_some_and(|old| old.connected && !old.stale);
        let connected = node.connected && !node.stale;
        state.nodes.insert(node.node_id.clone(), NodeRecord {
            node_id: node.node_id.clone(), station_name: node.identity.station_name,
            site: node.identity.site, connected: node.connected, stale: node.stale,
            last_seen: node.last_seen, subscriber_policy_capable: node.capabilities.subscriber_policy,
            mcc: node.identity.mcc, mnc: node.identity.mnc, location_area: node.identity.location_area,
            colour_code: node.identity.colour_code,
        });
        if connected && !was_connected { newly_connected.push(node.node_id); }
    }
    for sync in state.syncs.values_mut() {
        if !state.nodes.get(&sync.node_id).is_some_and(|n| n.connected && !n.stale) {
            sync.phase = SyncPhase::Offline;
            sync.updated_at = now_iso();
            sync.deadline = None;
        }
    }
    if state.config.access_policy.auto_sync {
        newly_connected.into_iter().filter_map(|node| schedule_sync_locked(state, &node)).collect()
    } else { Vec::new() }
}

fn handle_node_message_locked(state: &mut SubscriberState, node_id: &str, message: NodeToControlRoomMessage) {
    match message {
        NodeToControlRoomMessage::Telemetry { envelope } => handle_telemetry_locked(state, node_id, envelope.event),
        NodeToControlRoomMessage::ControlAck { ack } => {
            if let Some(sync) = state.syncs.values_mut().find(|s| s.command_id.as_deref() == Some(&ack.command_id)) {
                if !ack.accepted {
                    sync.phase = SyncPhase::Failed;
                    sync.message = Some(ack.message.clone());
                    sync.deadline = None;
                }
                sync.updated_at = now_iso();
            }
        }
        NodeToControlRoomMessage::ControlResponse { envelope } => {
            if let ControlResponse::SubscriberAccessPolicyApplied { revision, success, allow_all, allowed_count, disconnected_count, message, .. } = envelope.response {
                let sync_node_id = match envelope.command_id.as_deref() {
                    Some(command_id) => state.syncs.values()
                        .find(|sync| sync.command_id.as_deref() == Some(command_id))
                        .map(|sync| sync.node_id.clone()),
                    None => Some(node_id.to_string()),
                };
                if let Some(sync) = sync_node_id.as_deref().and_then(|id| state.syncs.get_mut(id)) {
                    sync.phase = if success { SyncPhase::Applied } else { SyncPhase::Failed };
                    if success { sync.applied_revision = Some(revision); }
                    sync.message = Some(message.clone());
                    sync.updated_at = now_iso();
                    sync.deadline = None;
                }
                push_event_locked(state, if success {"policy_sync_applied"} else {"policy_sync_failed"}, None, Some(node_id.to_string()), json!({"revision": revision, "allow_all": allow_all, "allowed_count": allowed_count, "disconnected_count": disconnected_count, "message": message}));
            }
        }
        NodeToControlRoomMessage::Error { message, .. } => {
            push_event_locked(state, "node_error", None, Some(node_id.to_string()), json!({"message": message}));
        }
        _ => {}
    }
}

fn handle_action_result_locked(state: &mut SubscriberState, request_id: Option<String>, command_id: Option<String>, ok: bool, message: String) {
    let Some(request_id) = request_id else { return; };
    if let Some(sync) = state.syncs.values_mut().find(|s| s.request_id.as_deref() == Some(&request_id)) {
        sync.updated_at = now_iso();
        sync.command_id = command_id;
        sync.message = Some(message.clone());
        if sync.phase != SyncPhase::Applied {
            sync.phase = if ok { SyncPhase::Requested } else { SyncPhase::Failed };
        }
        if !ok { sync.deadline = None; }
    }
}

fn handle_telemetry_locked(state: &mut SubscriberState, node_id: &str, event: TelemetryEvent) {
    let now = now_iso();
    match event {
        TelemetryEvent::MsRegistration { issi } => {
            ensure_observed(state, issi, &now);
            if let Some(item) = state.observed.get_mut(&issi) {
                item.registered = true;
                item.serving_node = Some(node_id.to_string());
                item.last_seen = now;
            }
            refresh_observed_authorization(state, issi);
        }
        TelemetryEvent::MsDeregistration { issi } | TelemetryEvent::MsTimeoutDrop { issi } => {
            ensure_observed(state, issi, &now);
            if let Some(item) = state.observed.get_mut(&issi) {
                item.registered = false;
                item.serving_node = None;
                item.last_seen = now;
            }
        }
        TelemetryEvent::MsGroupAttach { issi, gssis } => {
            ensure_observed(state, issi, &now);
            if let Some(item) = state.observed.get_mut(&issi) {
                item.groups.extend(gssis);
                item.last_seen = now;
            }
        }
        TelemetryEvent::MsGroupsSnapshot { issi, gssis } => {
            ensure_observed(state, issi, &now);
            if let Some(item) = state.observed.get_mut(&issi) {
                item.groups = gssis.into_iter().collect();
                item.last_seen = now;
            }
        }
        TelemetryEvent::MsGroupDetach { issi, gssis } => {
            ensure_observed(state, issi, &now);
            if let Some(item) = state.observed.get_mut(&issi) {
                for gssi in gssis {
                    item.groups.remove(&gssi);
                }
                item.last_seen = now;
            }
        }
        TelemetryEvent::MsEnergySaving { issi, mode } => {
            ensure_observed(state, issi, &now);
            if let Some(item) = state.observed.get_mut(&issi) {
                item.energy_saving_mode = Some(mode);
                item.last_seen = now;
            }
        }
        TelemetryEvent::MsRssi { issi, rssi_dbfs } => {
            ensure_observed(state, issi, &now);
            if let Some(item) = state.observed.get_mut(&issi) {
                item.last_rssi_dbfs = Some(rssi_dbfs);
                item.last_seen = now;
            }
        }
        _ => return,
    }
}

fn ensure_observed(state: &mut SubscriberState, issi: u32, now: &str) {
    if state.observed.contains_key(&issi) {
        return;
    }
    let item = new_observed(state, issi, now);
    state.observed.insert(issi, item);
}

fn new_observed(state: &SubscriberState, issi: u32, now: &str) -> ObservedSubscriber {
    let known = state.subscribers.contains_key(&issi);
    let authorized = state.config.access_policy.mode == POLICY_OPEN_NETWORK
        || state.subscribers.get(&issi).is_some_and(profile_authorized);
    ObservedSubscriber {
        issi, registered: false, serving_node: None, groups: BTreeSet::new(),
        energy_saving_mode: None, last_rssi_dbfs: None,
        first_seen: now.to_string(), last_seen: now.to_string(), known_profile: known, authorized,
    }
}

fn refresh_observed_authorization(state: &mut SubscriberState, issi: u32) {
    let known = state.subscribers.contains_key(&issi);
    let authorized = state.config.access_policy.mode == POLICY_OPEN_NETWORK
        || state.subscribers.get(&issi).is_some_and(profile_authorized);
    if let Some(item) = state.observed.get_mut(&issi) {
        item.known_profile = known; item.authorized = authorized;
    }
}

fn push_event_locked(state: &mut SubscriberState, kind: &str, issi: Option<u32>, node_id: Option<String>, detail: Value) {
    let event = SubscriberEventRecord {
        seq: state.next_event_seq, timestamp: now_iso(), kind: kind.to_string(), issi, node_id, detail,
    };
    state.next_event_seq = state.next_event_seq.saturating_add(1);
    state.events.push_back(event);
    while state.events.len() > state.config.server.history_limit { state.events.pop_front(); }
}

fn csv(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config(name: &str) -> SubscriberCoreConfig {
        let mut config = SubscriberCoreConfig::default();
        let base = std::env::temp_dir().join(format!("netcore-subscriber-test-{name}-{}", Uuid::new_v4()));
        config.storage.database_path = base.with_extension("json");
        config.storage.backup_path = base.with_extension("bak");
        config
    }

    #[test]
    fn create_and_reload_profile() {
        let config = config("reload");
        let state = SharedSubscribers::load(config.clone()).unwrap();
        state.create_subscriber(SubscriberInput {
            issi: 1234, home_mcc: 262, home_mnc: 1, display_name: "Test".into(), organization: String::new(),
            device_label: String::new(), device_tei: None, enabled: true, registration_allowed: true,
            call_priority: 4, emergency_allowed: true, sds_allowed: true, packet_data_allowed: false,
            default_groups: [100].into_iter().collect(), notes: String::new(),
        }).unwrap();
        let reloaded = SharedSubscribers::load(config.clone()).unwrap();
        assert_eq!(reloaded.subscriber(1234).unwrap().display_name, "Test");
        let _ = fs::remove_file(config.storage.database_path);
        let _ = fs::remove_file(config.storage.backup_path);
    }

    #[test]
    fn closed_empty_policy_is_not_open_network() {
        let config = config("closed-empty");
        let state = SharedSubscribers::load(config.clone()).unwrap();
        let (allow_all, allowed) = {
            let guard = state.0.lock().unwrap(); policy_values(&guard)
        };
        assert!(!allow_all);
        assert!(allowed.is_empty());
        let _ = fs::remove_file(config.storage.database_path);
    }
}

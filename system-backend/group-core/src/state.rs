use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tetra_entities::net_control::{
    ControlCommand, ControlResponse, GroupMembershipPolicy, GroupPolicyDefinition,
};
use tetra_entities::net_control_room::NodeToControlRoomMessage;
use tetra_entities::net_telemetry::TelemetryEvent;
use uuid::Uuid;

use crate::config::GroupCoreConfig;
use crate::protocol::{BackendEvent, BackendRequest, GatewaySnapshot};

const DATABASE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize)]
pub struct GroupCoreStatus {
    pub service: &'static str,
    pub started_at: String,
    pub security_mode: &'static str,
    pub warning: &'static str,
    pub node_gateway_connected: bool,
    pub node_gateway_last_error: Option<String>,
    pub database_revision: u64,
    pub groups_total: usize,
    pub groups_enabled: usize,
    pub memberships_total: usize,
    pub observed_affiliations: usize,
    pub nodes_connected: usize,
    pub nodes_synced: usize,
    pub syncs_pending: usize,
    pub dgna_pending: usize,
    pub allow_unlisted_groups: bool,
    pub enforce_memberships: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeRecord {
    pub node_id: String,
    pub station_name: String,
    pub site: Option<String>,
    pub connected: bool,
    pub stale: bool,
    pub last_seen: String,
    pub group_policy_capable: bool,
    pub dgna_capable: bool,
    pub mcc: u16,
    pub mnc: u16,
    pub location_area: u16,
    pub colour_code: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupProfile {
    pub gssi: u32,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub attach_allowed: bool,
    pub dgna_allowed: bool,
    pub call_allowed: bool,
    pub sds_allowed: bool,
    pub emergency_allowed: bool,
    pub call_priority: u8,
    pub class_of_usage: u8,
    pub area_nodes: BTreeSet<String>,
    pub notes: String,
    pub created_at: String,
    pub updated_at: String,
    pub revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupInput {
    pub gssi: u32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub attach_allowed: bool,
    #[serde(default = "default_true")]
    pub dgna_allowed: bool,
    #[serde(default = "default_true")]
    pub call_allowed: bool,
    #[serde(default = "default_true")]
    pub sds_allowed: bool,
    #[serde(default)]
    pub emergency_allowed: bool,
    #[serde(default)]
    pub call_priority: u8,
    #[serde(default = "default_class_of_usage")]
    pub class_of_usage: u8,
    #[serde(default)]
    pub area_nodes: BTreeSet<String>,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MembershipRecord {
    pub issi: u32,
    pub gssi: u32,
    pub allowed: bool,
    pub auto_attach: bool,
    pub locked: bool,
    pub notes: String,
    pub created_at: String,
    pub updated_at: String,
    pub revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MembershipInput {
    pub issi: u32,
    pub gssi: u32,
    #[serde(default = "default_true")]
    pub allowed: bool,
    #[serde(default)]
    pub auto_attach: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ObservedAffiliation {
    pub node_id: String,
    pub issi: u32,
    pub registered: bool,
    pub groups: BTreeSet<u32>,
    pub last_seen: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
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
    pub group_count: usize,
    pub membership_count: usize,
    pub requested_at: Option<String>,
    pub updated_at: String,
    pub request_id: Option<String>,
    pub command_id: Option<String>,
    pub message: Option<String>,
    #[serde(skip)]
    deadline: Option<Instant>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DgnaPhase {
    Pending,
    Requested,
    Applied,
    Failed,
    TimedOut,
    Cancelled,
}

#[derive(Debug, Clone, Serialize)]
pub struct DgnaRecord {
    pub id: String,
    pub node_id: String,
    pub issi: u32,
    pub gssi: u32,
    pub attach: bool,
    pub force: bool,
    pub phase: DgnaPhase,
    pub requested_at: String,
    pub updated_at: String,
    pub request_id: String,
    pub command_id: Option<String>,
    pub message: Option<String>,
    #[serde(skip)]
    deadline: Option<Instant>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DgnaInput {
    pub node_id: String,
    pub issi: u32,
    pub gssi: u32,
    pub attach: bool,
    #[serde(default)]
    pub force: bool,
    #[serde(default)]
    pub update_membership: bool,
    #[serde(default)]
    pub auto_attach: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct GroupEventRecord {
    pub seq: u64,
    pub timestamp: String,
    pub kind: String,
    pub gssi: Option<u32>,
    pub issi: Option<u32>,
    pub node_id: Option<String>,
    pub detail: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupDatabase {
    pub schema_version: u32,
    pub revision: u64,
    pub groups: Vec<GroupProfile>,
    pub memberships: Vec<MembershipRecord>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImportRequest {
    #[serde(default)]
    pub replace: bool,
    #[serde(default)]
    pub groups: Vec<GroupInput>,
    #[serde(default)]
    pub memberships: Vec<MembershipInput>,
}

#[derive(Clone)]
pub struct SharedGroups(Arc<Mutex<GroupState>>);

struct GroupState {
    config: GroupCoreConfig,
    started_at: String,
    gateway_connected: bool,
    gateway_last_error: Option<String>,
    database_revision: u64,
    groups: BTreeMap<u32, GroupProfile>,
    memberships: BTreeMap<(u32, u32), MembershipRecord>,
    nodes: BTreeMap<String, NodeRecord>,
    affiliations: BTreeMap<String, ObservedAffiliation>,
    syncs: BTreeMap<String, SyncRecord>,
    dgna: BTreeMap<String, DgnaRecord>,
    events: VecDeque<GroupEventRecord>,
    next_event_seq: u64,
    next_handle: u32,
}

impl SharedGroups {
    pub fn load(config: GroupCoreConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let (revision, groups, memberships) = load_database(&config)?;
        Ok(Self(Arc::new(Mutex::new(GroupState {
            config,
            started_at: now_iso(),
            gateway_connected: false,
            gateway_last_error: None,
            database_revision: revision,
            groups,
            memberships,
            nodes: BTreeMap::new(),
            affiliations: BTreeMap::new(),
            syncs: BTreeMap::new(),
            dgna: BTreeMap::new(),
            events: VecDeque::new(),
            next_event_seq: 0,
            next_handle: 0,
        }))))
    }

    pub fn status(&self) -> GroupCoreStatus {
        let state = self.0.lock().expect("group state poisoned");
        GroupCoreStatus {
            service: "netcore-group-core",
            started_at: state.started_at.clone(),
            security_mode: "open_lab",
            warning: "No authentication, no tokens and no TLS. Isolated test network only.",
            node_gateway_connected: state.gateway_connected,
            node_gateway_last_error: state.gateway_last_error.clone(),
            database_revision: state.database_revision,
            groups_total: state.groups.len(),
            groups_enabled: state.groups.values().filter(|group| group.enabled).count(),
            memberships_total: state.memberships.len(),
            observed_affiliations: state.affiliations.values().map(|item| item.groups.len()).sum(),
            nodes_connected: state.nodes.values().filter(|node| node.connected && !node.stale).count(),
            nodes_synced: state.syncs.values().filter(|sync| sync.phase == SyncPhase::Applied).count(),
            syncs_pending: state.syncs.values().filter(|sync| matches!(sync.phase, SyncPhase::Pending | SyncPhase::Requested)).count(),
            dgna_pending: state.dgna.values().filter(|item| matches!(item.phase, DgnaPhase::Pending | DgnaPhase::Requested)).count(),
            allow_unlisted_groups: state.config.policy.allow_unlisted_groups,
            enforce_memberships: state.config.policy.enforce_memberships,
        }
    }

    pub fn nodes(&self) -> Vec<NodeRecord> {
        self.0.lock().expect("group state poisoned").nodes.values().cloned().collect()
    }
    pub fn groups(&self) -> Vec<GroupProfile> {
        self.0.lock().expect("group state poisoned").groups.values().cloned().collect()
    }
    pub fn group(&self, gssi: u32) -> Option<GroupProfile> {
        self.0.lock().expect("group state poisoned").groups.get(&gssi).cloned()
    }
    pub fn memberships(&self) -> Vec<MembershipRecord> {
        self.0.lock().expect("group state poisoned").memberships.values().cloned().collect()
    }
    pub fn affiliations(&self) -> Vec<ObservedAffiliation> {
        self.0.lock().expect("group state poisoned").affiliations.values().cloned().collect()
    }
    pub fn syncs(&self) -> Vec<SyncRecord> {
        self.0.lock().expect("group state poisoned").syncs.values().cloned().collect()
    }
    pub fn dgna_operations(&self) -> Vec<DgnaRecord> {
        let mut values: Vec<_> = self.0.lock().expect("group state poisoned").dgna.values().cloned().collect();
        values.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        values
    }
    pub fn recent_events(&self, limit: usize) -> Vec<GroupEventRecord> {
        self.0.lock().expect("group state poisoned").events.iter().rev().take(limit).cloned().collect()
    }
    pub fn export_database(&self) -> GroupDatabase {
        let state = self.0.lock().expect("group state poisoned");
        database_snapshot(&state)
    }

    pub fn create_group(&self, input: GroupInput) -> Result<(GroupProfile, Vec<BackendRequest>), String> {
        let mut state = self.0.lock().expect("group state poisoned");
        validate_group_input(&state, &input, false)?;
        if state.groups.contains_key(&input.gssi) {
            return Err(format!("GSSI {} already exists", input.gssi));
        }
        let now = now_iso();
        state.database_revision = state.database_revision.saturating_add(1);
        let profile = profile_from_input(input, now.clone(), now, state.database_revision);
        state.groups.insert(profile.gssi, profile.clone());
        persist_locked(&state)?;
        push_event_locked(&mut state, "group_created", Some(profile.gssi), None, None, json!({"name": profile.name}));
        let commands = maybe_sync_all_locked(&mut state);
        Ok((profile, commands))
    }

    pub fn update_group(&self, gssi: u32, mut input: GroupInput) -> Result<(GroupProfile, Vec<BackendRequest>), String> {
        let mut state = self.0.lock().expect("group state poisoned");
        let old = state.groups.get(&gssi).cloned().ok_or_else(|| "group not found".to_string())?;
        input.gssi = gssi;
        validate_group_input(&state, &input, true)?;
        state.database_revision = state.database_revision.saturating_add(1);
        let profile = profile_from_input(input, old.created_at, now_iso(), state.database_revision);
        state.groups.insert(gssi, profile.clone());
        persist_locked(&state)?;
        push_event_locked(&mut state, "group_updated", Some(gssi), None, None, json!({"name": profile.name}));
        let commands = maybe_sync_all_locked(&mut state);
        Ok((profile, commands))
    }

    pub fn delete_group(&self, gssi: u32) -> Result<Vec<BackendRequest>, String> {
        let mut state = self.0.lock().expect("group state poisoned");
        if state.groups.remove(&gssi).is_none() {
            return Err("group not found".to_string());
        }
        state.memberships.retain(|(_, member_gssi), _| *member_gssi != gssi);
        state.database_revision = state.database_revision.saturating_add(1);
        persist_locked(&state)?;
        push_event_locked(&mut state, "group_deleted", Some(gssi), None, None, json!({}));
        Ok(maybe_sync_all_locked(&mut state))
    }

    pub fn upsert_membership(&self, input: MembershipInput) -> Result<(MembershipRecord, Vec<BackendRequest>), String> {
        let mut state = self.0.lock().expect("group state poisoned");
        validate_membership_input(&state, &input)?;
        let key = (input.issi, input.gssi);
        let created_at = state.memberships.get(&key).map(|item| item.created_at.clone()).unwrap_or_else(now_iso);
        state.database_revision = state.database_revision.saturating_add(1);
        let record = MembershipRecord {
            issi: input.issi,
            gssi: input.gssi,
            allowed: input.allowed,
            auto_attach: input.auto_attach,
            locked: input.locked,
            notes: input.notes.trim().to_string(),
            created_at,
            updated_at: now_iso(),
            revision: state.database_revision,
        };
        state.memberships.insert(key, record.clone());
        persist_locked(&state)?;
        push_event_locked(&mut state, "membership_upserted", Some(record.gssi), Some(record.issi), None, json!({"auto_attach": record.auto_attach, "locked": record.locked}));
        let commands = maybe_sync_all_locked(&mut state);
        Ok((record, commands))
    }

    pub fn delete_membership(&self, issi: u32, gssi: u32) -> Result<Vec<BackendRequest>, String> {
        let mut state = self.0.lock().expect("group state poisoned");
        if state.memberships.remove(&(issi, gssi)).is_none() {
            return Err("membership not found".to_string());
        }
        state.database_revision = state.database_revision.saturating_add(1);
        persist_locked(&state)?;
        push_event_locked(&mut state, "membership_deleted", Some(gssi), Some(issi), None, json!({}));
        Ok(maybe_sync_all_locked(&mut state))
    }

    pub fn import_database(&self, request: ImportRequest) -> Result<(usize, Vec<BackendRequest>), String> {
        let mut state = self.0.lock().expect("group state poisoned");
        if request.groups.len() > state.config.limits.max_groups || request.memberships.len() > state.config.limits.max_memberships {
            return Err("import exceeds configured limits".to_string());
        }
        if request.replace {
            state.groups.clear();
            state.memberships.clear();
        }
        for input in request.groups {
            validate_group_input(&state, &input, state.groups.contains_key(&input.gssi))?;
            state.database_revision = state.database_revision.saturating_add(1);
            let now = now_iso();
            let created = state.groups.get(&input.gssi).map(|item| item.created_at.clone()).unwrap_or_else(|| now.clone());
            let profile = profile_from_input(input, created, now, state.database_revision);
            state.groups.insert(profile.gssi, profile);
        }
        for input in request.memberships {
            validate_membership_input(&state, &input)?;
            state.database_revision = state.database_revision.saturating_add(1);
            let key = (input.issi, input.gssi);
            let now = now_iso();
            let created = state.memberships.get(&key).map(|item| item.created_at.clone()).unwrap_or_else(|| now.clone());
            state.memberships.insert(key, MembershipRecord {
                issi: input.issi,
                gssi: input.gssi,
                allowed: input.allowed,
                auto_attach: input.auto_attach,
                locked: input.locked,
                notes: input.notes.trim().to_string(),
                created_at: created,
                updated_at: now,
                revision: state.database_revision,
            });
        }
        persist_locked(&state)?;
        let count = state.groups.len() + state.memberships.len();
        push_event_locked(&mut state, "database_imported", None, None, None, json!({"replace": request.replace, "records": count}));
        Ok((count, maybe_sync_all_locked(&mut state)))
    }

    pub fn sync_all(&self) -> Vec<BackendRequest> {
        schedule_sync_all_locked(&mut self.0.lock().expect("group state poisoned"))
    }

    pub fn request_dgna(&self, input: DgnaInput) -> Result<(DgnaRecord, Vec<BackendRequest>), String> {
        let mut state = self.0.lock().expect("group state poisoned");
        validate_identity(input.issi, "ISSI")?;
        validate_identity(input.gssi, "GSSI")?;
        let node = state.nodes.get(&input.node_id).ok_or_else(|| "unknown TBS".to_string())?;
        if !node.connected || node.stale { return Err("target TBS is offline".to_string()); }
        if !node.dgna_capable { return Err("target TBS does not advertise DGNA capability".to_string()); }
        if !input.force {
            let group = state.groups.get(&input.gssi).ok_or_else(|| "group is not defined".to_string())?;
            if !group.enabled || !group.dgna_allowed { return Err("group does not allow DGNA".to_string()); }
            if input.attach && state.config.policy.enforce_memberships {
                let allowed = state.memberships.get(&(input.issi, input.gssi)).is_some_and(|item| item.allowed);
                if !allowed { return Err("subscriber has no allowed membership for this group".to_string()); }
            }
        }

        let mut commands = Vec::new();
        if input.update_membership {
            if input.attach {
                let membership = MembershipInput { issi: input.issi, gssi: input.gssi, allowed: true, auto_attach: input.auto_attach, locked: false, notes: "created by DGNA operation".to_string() };
                validate_membership_input(&state, &membership)?;
                state.database_revision = state.database_revision.saturating_add(1);
                let now = now_iso();
                state.memberships.insert((input.issi, input.gssi), MembershipRecord {
                    issi: input.issi, gssi: input.gssi, allowed: true, auto_attach: input.auto_attach, locked: false,
                    notes: membership.notes, created_at: now.clone(), updated_at: now, revision: state.database_revision,
                });
            } else {
                state.memberships.remove(&(input.issi, input.gssi));
                state.database_revision = state.database_revision.saturating_add(1);
            }
            persist_locked(&state)?;
            commands.extend(schedule_sync_all_locked(&mut state));
        }

        state.next_handle = state.next_handle.wrapping_add(1).max(1);
        let handle = state.next_handle;
        let id = Uuid::new_v4().to_string();
        let request_id = format!("group-dgna:{}:{}", id, Uuid::new_v4());
        let now = now_iso();
        let record = DgnaRecord {
            id: id.clone(), node_id: input.node_id.clone(), issi: input.issi, gssi: input.gssi,
            attach: input.attach, force: input.force, phase: DgnaPhase::Pending,
            requested_at: now.clone(), updated_at: now, request_id: request_id.clone(), command_id: None,
            message: Some("DGNA queued".to_string()),
            deadline: Some(Instant::now() + Duration::from_secs(state.config.policy.dgna_timeout_secs)),
        };
        state.dgna.insert(id.clone(), record.clone());
        push_event_locked(&mut state, "dgna_queued", Some(input.gssi), Some(input.issi), Some(input.node_id.clone()), json!({"attach": input.attach, "force": input.force, "id": id}));
        commands.push(BackendRequest::Command {
            request_id: Some(request_id),
            node_id: input.node_id,
            command: ControlCommand::GroupDgnaApply { handle, issi: input.issi, gssi: input.gssi, attach: input.attach, force: input.force },
            operator_id: Some("group-core/open-lab".to_string()),
        });
        Ok((record, commands))
    }

    pub fn cancel_dgna(&self, id: &str) -> Result<(), String> {
        let mut state = self.0.lock().expect("group state poisoned");
        let item = state.dgna.get_mut(id).ok_or_else(|| "DGNA operation not found".to_string())?;
        if !matches!(item.phase, DgnaPhase::Pending | DgnaPhase::Requested) {
            return Err("DGNA operation can no longer be cancelled".to_string());
        }
        item.phase = DgnaPhase::Cancelled;
        item.updated_at = now_iso();
        item.deadline = None;
        item.message = Some("cancelled locally; already transmitted commands cannot be recalled".to_string());
        Ok(())
    }

    pub fn metrics(&self) -> String {
        let status = self.status();
        format!(
            "# TYPE netcore_group_core_groups gauge\nnetcore_group_core_groups {}\n# TYPE netcore_group_core_memberships gauge\nnetcore_group_core_memberships {}\n# TYPE netcore_group_core_affiliations gauge\nnetcore_group_core_affiliations {}\n# TYPE netcore_group_core_nodes_connected gauge\nnetcore_group_core_nodes_connected {}\n# TYPE netcore_group_core_sync_pending gauge\nnetcore_group_core_sync_pending {}\n# TYPE netcore_group_core_dgna_pending gauge\nnetcore_group_core_dgna_pending {}\n",
            status.groups_total, status.memberships_total, status.observed_affiliations,
            status.nodes_connected, status.syncs_pending, status.dgna_pending,
        )
    }

    pub fn gateway_connected(&self) {
        let mut state = self.0.lock().expect("group state poisoned");
        state.gateway_connected = true;
        state.gateway_last_error = None;
        push_event_locked(&mut state, "gateway_connected", None, None, None, json!({}));
    }

    pub fn gateway_disconnected(&self, error: String) {
        let mut state = self.0.lock().expect("group state poisoned");
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
        push_event_locked(&mut state, "gateway_disconnected", None, None, None, json!({"error": error}));
    }

    pub fn handle_backend_event(&self, event: BackendEvent) -> Vec<BackendRequest> {
        let mut state = self.0.lock().expect("group state poisoned");
        match event {
            BackendEvent::Snapshot { snapshot } => handle_snapshot_locked(&mut state, snapshot),
            BackendEvent::Event { event } => {
                push_event_locked(&mut state, "gateway_event", None, None, event.node_id, event.detail);
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

    pub fn expire_operations(&self) -> Vec<BackendRequest> {
        let mut state = self.0.lock().expect("group state poisoned");
        let now = Instant::now();
        let mut sync_timeouts = Vec::new();
        for sync in state.syncs.values_mut() {
            if matches!(sync.phase, SyncPhase::Pending | SyncPhase::Requested)
                && sync.deadline.is_some_and(|deadline| deadline <= now)
            {
                sync.phase = SyncPhase::TimedOut;
                sync.updated_at = now_iso();
                sync.message = Some("group policy synchronization timed out".to_string());
                sync.deadline = None;
                sync_timeouts.push(sync.node_id.clone());
            }
        }
        let mut dgna_timeouts = Vec::new();
        for item in state.dgna.values_mut() {
            if matches!(item.phase, DgnaPhase::Pending | DgnaPhase::Requested)
                && item.deadline.is_some_and(|deadline| deadline <= now)
            {
                item.phase = DgnaPhase::TimedOut;
                item.updated_at = now_iso();
                item.message = Some("DGNA operation timed out".to_string());
                item.deadline = None;
                dgna_timeouts.push((item.node_id.clone(), item.issi, item.gssi));
            }
        }
        for node in sync_timeouts { push_event_locked(&mut state, "policy_sync_timeout", None, None, Some(node), json!({})); }
        for (node, issi, gssi) in dgna_timeouts { push_event_locked(&mut state, "dgna_timeout", Some(gssi), Some(issi), Some(node), json!({})); }
        Vec::new()
    }
}

fn load_database(config: &GroupCoreConfig) -> Result<(u64, BTreeMap<u32, GroupProfile>, BTreeMap<(u32, u32), MembershipRecord>), Box<dyn std::error::Error>> {
    let path = &config.storage.database_path;
    if !path.exists() {
        if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
        return Ok((0, BTreeMap::new(), BTreeMap::new()));
    }
    let database: GroupDatabase = serde_json::from_str(&fs::read_to_string(path)?)?;
    if database.schema_version != DATABASE_SCHEMA_VERSION {
        return Err(format!("unsupported group database schema {}", database.schema_version).into());
    }
    let groups = database.groups.into_iter().map(|item| (item.gssi, item)).collect();
    let memberships = database.memberships.into_iter().map(|item| ((item.issi, item.gssi), item)).collect();
    Ok((database.revision, groups, memberships))
}

fn database_snapshot(state: &GroupState) -> GroupDatabase {
    GroupDatabase {
        schema_version: DATABASE_SCHEMA_VERSION,
        revision: state.database_revision,
        groups: state.groups.values().cloned().collect(),
        memberships: state.memberships.values().cloned().collect(),
    }
}

fn persist_locked(state: &GroupState) -> Result<(), String> {
    let database = database_snapshot(state);
    let payload = serde_json::to_vec_pretty(&database).map_err(|error| error.to_string())?;
    if let Some(parent) = state.config.storage.database_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    if state.config.storage.database_path.exists() {
        let _ = fs::copy(&state.config.storage.database_path, &state.config.storage.backup_path);
    }
    let temp = state.config.storage.database_path.with_extension("json.tmp");
    let mut file = fs::File::create(&temp).map_err(|error| error.to_string())?;
    file.write_all(&payload).map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    fs::rename(temp, &state.config.storage.database_path).map_err(|error| error.to_string())
}

fn validate_group_input(state: &GroupState, input: &GroupInput, updating: bool) -> Result<(), String> {
    validate_identity(input.gssi, "GSSI")?;
    if !updating && state.groups.len() >= state.config.limits.max_groups { return Err("group limit reached".to_string()); }
    if input.call_priority > 15 || input.class_of_usage > 15 { return Err("priority and class_of_usage must be in 0..=15".to_string()); }
    if input.name.len() > 160 || input.description.len() > 1_024 || input.notes.len() > 4_096 { return Err("one or more text fields are too long".to_string()); }
    if input.area_nodes.iter().any(|node| node.len() > 160) { return Err("area node identifier is too long".to_string()); }
    Ok(())
}

fn validate_membership_input(state: &GroupState, input: &MembershipInput) -> Result<(), String> {
    validate_identity(input.issi, "ISSI")?;
    validate_identity(input.gssi, "GSSI")?;
    if !state.groups.contains_key(&input.gssi) { return Err("referenced group does not exist".to_string()); }
    if !state.memberships.contains_key(&(input.issi, input.gssi)) && state.memberships.len() >= state.config.limits.max_memberships { return Err("membership limit reached".to_string()); }
    if input.notes.len() > 4_096 { return Err("membership notes are too long".to_string()); }
    Ok(())
}

fn validate_identity(value: u32, name: &str) -> Result<(), String> {
    if value == 0 || value > 0xFF_FFFF { Err(format!("{name} must be in 1..=16777215")) } else { Ok(()) }
}

fn profile_from_input(input: GroupInput, created_at: String, updated_at: String, revision: u64) -> GroupProfile {
    GroupProfile {
        gssi: input.gssi,
        name: input.name.trim().to_string(),
        description: input.description.trim().to_string(),
        enabled: input.enabled,
        attach_allowed: input.attach_allowed,
        dgna_allowed: input.dgna_allowed,
        call_allowed: input.call_allowed,
        sds_allowed: input.sds_allowed,
        emergency_allowed: input.emergency_allowed,
        call_priority: input.call_priority.min(15),
        class_of_usage: input.class_of_usage.min(15),
        area_nodes: input.area_nodes,
        notes: input.notes.trim().to_string(),
        created_at,
        updated_at,
        revision,
    }
}

fn maybe_sync_all_locked(state: &mut GroupState) -> Vec<BackendRequest> {
    if state.config.policy.auto_sync { schedule_sync_all_locked(state) } else { Vec::new() }
}

fn schedule_sync_all_locked(state: &mut GroupState) -> Vec<BackendRequest> {
    let nodes: Vec<String> = state.nodes.values().filter(|node| node.connected && !node.stale).map(|node| node.node_id.clone()).collect();
    nodes.into_iter().filter_map(|node| schedule_sync_locked(state, &node)).collect()
}

fn schedule_sync_locked(state: &mut GroupState, node_id: &str) -> Option<BackendRequest> {
    let revision = state.database_revision;
    let node = state.nodes.get(node_id)?.clone();
    if !node.connected || node.stale { return None; }
    if !node.group_policy_capable {
        state.syncs.insert(node_id.to_string(), SyncRecord {
            node_id: node_id.to_string(), desired_revision: revision, applied_revision: None,
            phase: SyncPhase::Unsupported, group_count: 0, membership_count: 0, requested_at: None,
            updated_at: now_iso(), request_id: None, command_id: None,
            message: Some("TBS does not advertise group_policy capability".to_string()), deadline: None,
        });
        return None;
    }

    let groups: Vec<GroupPolicyDefinition> = state.groups.values()
        .filter(|group| group.area_nodes.is_empty() || group.area_nodes.contains(node_id))
        .map(|group| GroupPolicyDefinition {
            gssi: group.gssi, enabled: group.enabled, attach_allowed: group.attach_allowed,
            dgna_allowed: group.dgna_allowed, call_allowed: group.call_allowed, sds_allowed: group.sds_allowed,
            emergency_allowed: group.emergency_allowed, call_priority: group.call_priority,
            class_of_usage: group.class_of_usage,
        }).collect();
    let visible: BTreeSet<u32> = groups.iter().map(|group| group.gssi).collect();
    let memberships: Vec<GroupMembershipPolicy> = state.memberships.values()
        .filter(|membership| visible.contains(&membership.gssi))
        .map(|membership| GroupMembershipPolicy {
            issi: membership.issi, gssi: membership.gssi, allowed: membership.allowed,
            auto_attach: membership.auto_attach, locked: membership.locked,
        }).collect();

    state.next_handle = state.next_handle.wrapping_add(1).max(1);
    let handle = state.next_handle;
    let request_id = format!("group-policy:{}:{}:{}", node_id, revision, Uuid::new_v4());
    let now = now_iso();
    state.syncs.insert(node_id.to_string(), SyncRecord {
        node_id: node_id.to_string(), desired_revision: revision, applied_revision: None,
        phase: SyncPhase::Pending, group_count: groups.len(), membership_count: memberships.len(),
        requested_at: Some(now.clone()), updated_at: now, request_id: Some(request_id.clone()),
        command_id: None, message: Some("group policy queued".to_string()),
        deadline: Some(Instant::now() + Duration::from_secs(state.config.policy.sync_timeout_secs)),
    });
    push_event_locked(state, "policy_sync_queued", None, None, Some(node_id.to_string()), json!({"revision": revision, "groups": groups.len(), "memberships": memberships.len()}));
    Some(BackendRequest::Command {
        request_id: Some(request_id), node_id: node_id.to_string(),
        command: ControlCommand::GroupAccessPolicyApply {
            handle, revision,
            allow_unlisted_groups: state.config.policy.allow_unlisted_groups,
            enforce_memberships: state.config.policy.enforce_memberships,
            reconcile_registered: state.config.policy.reconcile_registered,
            groups, memberships,
        },
        operator_id: Some("group-core/open-lab".to_string()),
    })
}

fn handle_snapshot_locked(state: &mut GroupState, snapshot: GatewaySnapshot) -> Vec<BackendRequest> {
    let mut newly_connected = Vec::new();
    for node in snapshot.nodes {
        let was_connected = state.nodes.get(&node.node_id).is_some_and(|old| old.connected && !old.stale);
        let connected = node.connected && !node.stale;
        state.nodes.insert(node.node_id.clone(), NodeRecord {
            node_id: node.node_id.clone(), station_name: node.identity.station_name, site: node.identity.site,
            connected: node.connected, stale: node.stale, last_seen: node.last_seen,
            group_policy_capable: node.capabilities.group_policy, dgna_capable: node.capabilities.dgna,
            mcc: node.identity.mcc, mnc: node.identity.mnc, location_area: node.identity.location_area,
            colour_code: node.identity.colour_code,
        });
        if connected && !was_connected { newly_connected.push(node.node_id); }
    }
    for sync in state.syncs.values_mut() {
        if !state.nodes.get(&sync.node_id).is_some_and(|node| node.connected && !node.stale) {
            sync.phase = SyncPhase::Offline;
            sync.updated_at = now_iso();
            sync.deadline = None;
        }
    }
    if state.config.policy.auto_sync {
        newly_connected.into_iter().filter_map(|node| schedule_sync_locked(state, &node)).collect()
    } else { Vec::new() }
}

fn handle_node_message_locked(state: &mut GroupState, node_id: &str, message: NodeToControlRoomMessage) {
    match message {
        NodeToControlRoomMessage::Telemetry { envelope } => handle_telemetry_locked(state, node_id, envelope.event),
        NodeToControlRoomMessage::ControlAck { ack } => {
            if let Some(sync) = state.syncs.values_mut().find(|sync| sync.command_id.as_deref() == Some(&ack.command_id)) {
                if !ack.accepted { sync.phase = SyncPhase::Failed; sync.message = Some(ack.message.clone()); sync.deadline = None; }
                sync.updated_at = now_iso();
            }
            if let Some(item) = state.dgna.values_mut().find(|item| item.command_id.as_deref() == Some(&ack.command_id)) {
                if !ack.accepted { item.phase = DgnaPhase::Failed; item.message = Some(ack.message); item.deadline = None; }
                item.updated_at = now_iso();
            }
        }
        NodeToControlRoomMessage::ControlResponse { envelope } => match envelope.response {
            ControlResponse::GroupAccessPolicyApplied { revision, success, group_count, membership_count, attached_count, detached_count, message, .. } => {
                let sync_id = match envelope.command_id.as_deref() {
                    Some(command_id) => state
                        .syncs
                        .values()
                        .find(|sync| sync.command_id.as_deref() == Some(command_id))
                        .map(|sync| sync.node_id.clone()),
                    None => state
                        .syncs
                        .get(node_id)
                        .filter(|sync| sync.command_id.is_none())
                        .map(|sync| sync.node_id.clone()),
                };
                let Some(sync_id) = sync_id else {
                    push_event_locked(
                        state,
                        "policy_sync_orphan_response",
                        None,
                        None,
                        Some(node_id.to_string()),
                        json!({"revision": revision, "command_id": envelope.command_id}),
                    );
                    return;
                };
                let revision_matches = state
                    .syncs
                    .get(&sync_id)
                    .is_some_and(|sync| sync.desired_revision == revision);
                let applied = success && revision_matches;
                if let Some(sync) = state.syncs.get_mut(&sync_id) {
                    sync.phase = if applied { SyncPhase::Applied } else { SyncPhase::Failed };
                    if applied { sync.applied_revision = Some(revision); }
                    sync.updated_at = now_iso();
                    sync.deadline = None;
                    sync.message = Some(if success && !revision_matches {
                        format!("revision mismatch: expected {}, received {revision}", sync.desired_revision)
                    } else {
                        message.clone()
                    });
                }
                push_event_locked(state, if applied {"policy_sync_applied"} else {"policy_sync_failed"}, None, None, Some(node_id.to_string()), json!({"revision": revision, "revision_matches": revision_matches, "groups": group_count, "memberships": membership_count, "attached": attached_count, "detached": detached_count, "message": message}));
            }
            ControlResponse::GroupDgnaApplied { issi, gssi, attach, success, message, .. } => {
                let id = envelope.command_id.as_deref().and_then(|command_id| state.dgna.values().find(|item| item.command_id.as_deref() == Some(command_id)).map(|item| item.id.clone()));
                if let Some(id) = id {
                    if let Some(item) = state.dgna.get_mut(&id) { item.phase = if success { DgnaPhase::Applied } else { DgnaPhase::Failed }; item.updated_at = now_iso(); item.deadline = None; item.message = Some(message.clone()); }
                }
                push_event_locked(state, if success {"dgna_applied"} else {"dgna_failed"}, Some(gssi), Some(issi), Some(node_id.to_string()), json!({"attach": attach, "message": message}));
            }
            _ => {}
        },
        NodeToControlRoomMessage::Error { message, .. } => push_event_locked(state, "node_error", None, None, Some(node_id.to_string()), json!({"message": message})),
        _ => {}
    }
}

fn handle_action_result_locked(state: &mut GroupState, request_id: Option<String>, command_id: Option<String>, ok: bool, message: String) {
    let Some(request_id) = request_id else { return; };
    if let Some(sync) = state.syncs.values_mut().find(|sync| sync.request_id.as_deref() == Some(&request_id)) {
        sync.updated_at = now_iso(); sync.command_id = command_id; sync.message = Some(message);
        if sync.phase != SyncPhase::Applied { sync.phase = if ok { SyncPhase::Requested } else { SyncPhase::Failed }; }
        if !ok { sync.deadline = None; }
        return;
    }
    if let Some(item) = state.dgna.values_mut().find(|item| item.request_id == request_id) {
        item.updated_at = now_iso(); item.command_id = command_id; item.message = Some(message);
        if item.phase != DgnaPhase::Applied { item.phase = if ok { DgnaPhase::Requested } else { DgnaPhase::Failed }; }
        if !ok { item.deadline = None; }
    }
}

fn handle_telemetry_locked(state: &mut GroupState, node_id: &str, event: TelemetryEvent) {
    let now = now_iso();
    match event {
        TelemetryEvent::MsRegistration { issi } => {
            let key = affiliation_key(node_id, issi);
            let item = state.affiliations.entry(key).or_insert_with(|| ObservedAffiliation { node_id: node_id.to_string(), issi, registered: true, groups: BTreeSet::new(), last_seen: now.clone() });
            item.registered = true; item.last_seen = now;
        }
        TelemetryEvent::MsDeregistration { issi } | TelemetryEvent::MsTimeoutDrop { issi } => {
            let key = affiliation_key(node_id, issi);
            let item = state.affiliations.entry(key).or_insert_with(|| ObservedAffiliation { node_id: node_id.to_string(), issi, registered: false, groups: BTreeSet::new(), last_seen: now.clone() });
            item.registered = false; item.groups.clear(); item.last_seen = now;
        }
        TelemetryEvent::MsGroupAttach { issi, gssis } => {
            let key = affiliation_key(node_id, issi);
            let item = state.affiliations.entry(key).or_insert_with(|| ObservedAffiliation { node_id: node_id.to_string(), issi, registered: true, groups: BTreeSet::new(), last_seen: now.clone() });
            item.groups.extend(gssis.iter().copied()); item.last_seen = now;
            push_event_locked(state, "group_attach_observed", None, Some(issi), Some(node_id.to_string()), json!({"gssis": gssis}));
        }
        TelemetryEvent::MsGroupDetach { issi, gssis } => {
            let key = affiliation_key(node_id, issi);
            let item = state.affiliations.entry(key).or_insert_with(|| ObservedAffiliation { node_id: node_id.to_string(), issi, registered: true, groups: BTreeSet::new(), last_seen: now.clone() });
            for gssi in &gssis { item.groups.remove(gssi); } item.last_seen = now;
            push_event_locked(state, "group_detach_observed", None, Some(issi), Some(node_id.to_string()), json!({"gssis": gssis}));
        }
        TelemetryEvent::MsGroupsSnapshot { issi, gssis } => {
            let key = affiliation_key(node_id, issi);
            state.affiliations.insert(key, ObservedAffiliation { node_id: node_id.to_string(), issi, registered: true, groups: gssis.into_iter().collect(), last_seen: now });
        }
        TelemetryEvent::GroupCallStarted { gssi, caller_issi, priority, .. } => push_event_locked(state, "group_call_started", Some(gssi), Some(caller_issi), Some(node_id.to_string()), json!({"priority": priority})),
        TelemetryEvent::GroupCallEnded { gssi, .. } => push_event_locked(state, "group_call_ended", Some(gssi), None, Some(node_id.to_string()), json!({})),
        _ => {}
    }
}

fn push_event_locked(state: &mut GroupState, kind: &str, gssi: Option<u32>, issi: Option<u32>, node_id: Option<String>, detail: Value) {
    state.next_event_seq = state.next_event_seq.saturating_add(1);
    state.events.push_back(GroupEventRecord { seq: state.next_event_seq, timestamp: now_iso(), kind: kind.to_string(), gssi, issi, node_id, detail });
    while state.events.len() > state.config.server.history_limit { state.events.pop_front(); }
}

fn affiliation_key(node_id: &str, issi: u32) -> String { format!("{node_id}:{issi}") }
fn now_iso() -> String { chrono::Utc::now().to_rfc3339() }
fn default_true() -> bool { true }
fn default_class_of_usage() -> u8 { 4 }

#[cfg(test)]
mod tests {
    use super::*;

    fn test_state() -> GroupState {
        GroupState {
            config: GroupCoreConfig::default(),
            started_at: now_iso(),
            gateway_connected: false,
            gateway_last_error: None,
            database_revision: 3,
            groups: BTreeMap::new(),
            memberships: BTreeMap::new(),
            nodes: BTreeMap::new(),
            affiliations: BTreeMap::new(),
            syncs: BTreeMap::new(),
            dgna: BTreeMap::new(),
            events: VecDeque::new(),
            next_event_seq: 0,
            next_handle: 0,
        }
    }

    #[test]
    fn schedule_sync_filters_groups_by_tbs_area() {
        let mut state = test_state();
        state.nodes.insert(
            "tbs-a".to_string(),
            NodeRecord {
                node_id: "tbs-a".to_string(),
                station_name: "A".to_string(),
                site: None,
                connected: true,
                stale: false,
                last_seen: now_iso(),
                group_policy_capable: true,
                dgna_capable: true,
                mcc: 262,
                mnc: 1,
                location_area: 10,
                colour_code: 1,
            },
        );
        state.groups.insert(
            15501,
            GroupProfile {
                gssi: 15501,
                name: "Global".to_string(),
                description: String::new(),
                enabled: true,
                attach_allowed: true,
                dgna_allowed: true,
                call_allowed: true,
                sds_allowed: true,
                emergency_allowed: false,
                call_priority: 5,
                class_of_usage: 4,
                area_nodes: BTreeSet::new(),
                notes: String::new(),
                created_at: now_iso(),
                updated_at: now_iso(),
                revision: 1,
            },
        );
        state.groups.insert(
            15502,
            GroupProfile {
                gssi: 15502,
                name: "Other site".to_string(),
                description: String::new(),
                enabled: true,
                attach_allowed: true,
                dgna_allowed: true,
                call_allowed: true,
                sds_allowed: true,
                emergency_allowed: false,
                call_priority: 0,
                class_of_usage: 4,
                area_nodes: BTreeSet::from(["tbs-b".to_string()]),
                notes: String::new(),
                created_at: now_iso(),
                updated_at: now_iso(),
                revision: 2,
            },
        );
        state.memberships.insert(
            (1001, 15501),
            MembershipRecord {
                issi: 1001,
                gssi: 15501,
                allowed: true,
                auto_attach: true,
                locked: false,
                notes: String::new(),
                created_at: now_iso(),
                updated_at: now_iso(),
                revision: 3,
            },
        );

        let request = schedule_sync_locked(&mut state, "tbs-a").expect("sync command");
        let BackendRequest::Command { command, .. } = request else {
            panic!("expected command request");
        };
        let ControlCommand::GroupAccessPolicyApply {
            revision,
            groups,
            memberships,
            ..
        } = command else {
            panic!("expected group policy command");
        };
        assert_eq!(revision, 3);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].gssi, 15501);
        assert_eq!(groups[0].call_priority, 5);
        assert_eq!(memberships.len(), 1);
        assert!(memberships[0].auto_attach);
    }

    #[test]
    fn profile_validation_rejects_invalid_gssi() {
        let state = test_state();
        let input = GroupInput { gssi: 0, name: String::new(), description: String::new(), enabled: true, attach_allowed: true, dgna_allowed: true, call_allowed: true, sds_allowed: true, emergency_allowed: false, call_priority: 0, class_of_usage: 4, area_nodes: BTreeSet::new(), notes: String::new() };
        assert!(validate_group_input(&state, &input, false).is_err());
    }
}

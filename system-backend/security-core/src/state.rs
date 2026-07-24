use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::io::Write;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Duration as ChronoDuration, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::config::{
    OPERATING_MODE_AUTHORITATIVE, OPERATING_MODE_SHADOW, SecurityCoreConfig,
};
use crate::crypto::{
    constant_time_eq, decode_hex, derive_dck, derive_subscriber_key, encode_hex,
    expected_response, fingerprint, load_or_create_seed, random_bytes,
};
use crate::protocol::{
    AlarmAckInput, AuthenticationResponseInput, AuthenticationStartInput, BackendEvent,
    DisableInput, EdgeActionAckInput, EdgeClaimInput, PolicyInput, ProfileInput, RevokeInput,
};

const DATABASE_SCHEMA_VERSION: u32 = 1;
const OPEN_LAB_WARNING: &str =
    "OPEN LAB: management has no authentication, no tokens and no TLS; isolated test network only";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthenticationState {
    ChallengePending,
    AwaitingResponse,
    Authenticated,
    Rejected,
    Expired,
    Revoked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DckState {
    PendingInstall,
    Active,
    Expired,
    Revoked,
    InstallFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeActionState {
    Pending,
    InFlight,
    Applied,
    Failed,
    Expired,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlarmState {
    Open,
    Acknowledged,
    Cleared,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityPolicyRecord {
    pub revision: u64,
    pub operating_mode: String,
    pub default_security_class: u8,
    pub minimum_security_class: u8,
    pub authentication_required: bool,
    pub allow_class1_fallback: bool,
    pub reject_unknown_subscribers: bool,
    pub disable_after_failures: bool,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityProfileRecord {
    pub issi: u32,
    pub display_name: String,
    pub authentication_required: bool,
    pub minimum_security_class: u8,
    pub preferred_security_class: u8,
    pub allow_class1_fallback: bool,
    pub allowed_nodes: Vec<String>,
    pub max_failures: u32,
    pub disabled: bool,
    pub equipment_disabled: bool,
    pub equipment_id: Option<String>,
    pub notes: String,
    pub created_at: String,
    pub updated_at: String,
    pub revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriberSecurityRecord {
    pub issi: u32,
    pub current_node_id: Option<String>,
    pub equipment_id: Option<String>,
    pub negotiated_security_class: Option<u8>,
    pub authenticated: bool,
    pub active_auth_context_id: Option<String>,
    pub active_dck_id: Option<String>,
    pub authentication_failures: u32,
    pub lockout_until: Option<String>,
    pub disabled: bool,
    pub equipment_disabled: bool,
    pub last_authentication_at: Option<String>,
    pub last_failure_at: Option<String>,
    pub last_seen_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticationContextRecord {
    pub id: String,
    pub issi: u32,
    pub node_id: String,
    pub equipment_id: Option<String>,
    pub requested_security_class: u8,
    pub supported_security_classes: Vec<u8>,
    pub negotiated_security_class: u8,
    pub state: AuthenticationState,
    pub provider: String,
    pub challenge_fingerprint: String,
    pub response_fingerprint: Option<String>,
    pub challenge_action_id: Option<String>,
    pub dck_id: Option<String>,
    pub created_at: String,
    pub expires_at: String,
    pub completed_at: Option<String>,
    pub attempts: u32,
    pub max_attempts: u32,
    pub source: String,
    pub failure_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DckContextRecord {
    pub id: String,
    pub key_reference: String,
    pub key_fingerprint: String,
    pub issi: u32,
    pub node_id: String,
    pub auth_context_id: String,
    pub security_class: u8,
    pub state: DckState,
    pub issued_at: String,
    pub expires_at: String,
    pub installed_at: Option<String>,
    pub revoked_at: Option<String>,
    pub revoke_reason: Option<String>,
    pub install_action_id: Option<String>,
    pub usage_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeActionRecord {
    pub id: String,
    pub sequence: u64,
    pub node_id: String,
    pub issi: Option<u32>,
    pub context_id: Option<String>,
    pub dck_id: Option<String>,
    pub kind: String,
    pub state: EdgeActionState,
    pub secret_bearing: bool,
    pub created_at: String,
    pub updated_at: String,
    pub expires_at: String,
    pub attempts: u32,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EdgeClaimedAction {
    pub id: String,
    pub sequence: u64,
    pub node_id: String,
    pub kind: String,
    pub issi: Option<u32>,
    pub context_id: Option<String>,
    pub dck_id: Option<String>,
    pub expires_at: String,
    pub protocol_version: &'static str,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityAlarmRecord {
    pub id: String,
    pub severity: String,
    pub kind: String,
    pub state: AlarmState,
    pub issi: Option<u32>,
    pub node_id: Option<String>,
    pub context_id: Option<String>,
    pub message: String,
    pub created_at: String,
    pub acknowledged_at: Option<String>,
    pub acknowledged_by: Option<String>,
    pub acknowledgement_note: Option<String>,
    pub cleared_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityAuditRecord {
    pub sequence: u64,
    pub timestamp: String,
    pub actor: String,
    pub action: String,
    pub target: String,
    pub outcome: String,
    pub detail: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityNodeRecord {
    pub node_id: String,
    pub station_name: String,
    pub connected: bool,
    pub stale: bool,
    pub mcc: Option<u16>,
    pub mnc: Option<u16>,
    pub location_area: Option<u16>,
    pub last_seen: String,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SecurityDatabase {
    schema_version: u32,
    revision: u64,
    next_action_sequence: u64,
    next_audit_sequence: u64,
    policy: SecurityPolicyRecord,
    profiles: BTreeMap<u32, SecurityProfileRecord>,
    subscribers: BTreeMap<u32, SubscriberSecurityRecord>,
    auth_contexts: BTreeMap<String, AuthenticationContextRecord>,
    dck_contexts: BTreeMap<String, DckContextRecord>,
    actions: BTreeMap<String, EdgeActionRecord>,
    alarms: BTreeMap<String, SecurityAlarmRecord>,
    audit: VecDeque<SecurityAuditRecord>,
    nodes: BTreeMap<String, SecurityNodeRecord>,
}

impl SecurityDatabase {
    fn new(config: &SecurityCoreConfig) -> Self {
        let now = now_iso();
        Self {
            schema_version: DATABASE_SCHEMA_VERSION,
            revision: 0,
            next_action_sequence: 1,
            next_audit_sequence: 1,
            policy: SecurityPolicyRecord {
                revision: 1,
                operating_mode: config.policy.operating_mode.clone(),
                default_security_class: config.policy.default_security_class,
                minimum_security_class: config.policy.minimum_security_class,
                authentication_required: config.policy.authentication_required,
                allow_class1_fallback: config.policy.allow_class1_fallback,
                reject_unknown_subscribers: config.policy.reject_unknown_subscribers,
                disable_after_failures: config.policy.disable_after_failures,
                updated_at: now,
            },
            profiles: BTreeMap::new(),
            subscribers: BTreeMap::new(),
            auth_contexts: BTreeMap::new(),
            dck_contexts: BTreeMap::new(),
            actions: BTreeMap::new(),
            alarms: BTreeMap::new(),
            audit: VecDeque::new(),
            nodes: BTreeMap::new(),
        }
    }
}

struct RuntimeSecrets {
    lab_seed: Vec<u8>,
    challenges: BTreeMap<String, Vec<u8>>,
    expected_responses: BTreeMap<String, Vec<u8>>,
    dck_material: BTreeMap<String, Vec<u8>>,
    action_payloads: BTreeMap<String, Value>,
}

struct SecurityCoreState {
    config: SecurityCoreConfig,
    database: SecurityDatabase,
    secrets: RuntimeSecrets,
    started_at: String,
    node_gateway_connected: bool,
    node_gateway_last_error: Option<String>,
    node_gateway_last_seen: Option<String>,
}

#[derive(Clone)]
pub struct SharedSecurityCore {
    inner: Arc<Mutex<SecurityCoreState>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SecurityCoreStatus {
    pub service: &'static str,
    pub version: &'static str,
    pub edge_protocol_version: &'static str,
    pub started_at: String,
    pub management_security_mode: &'static str,
    pub warning: &'static str,
    pub operating_mode: String,
    pub authoritative: bool,
    pub node_gateway_connected: bool,
    pub node_gateway_last_error: Option<String>,
    pub node_gateway_last_seen: Option<String>,
    pub database_revision: u64,
    pub policy_revision: u64,
    pub profiles: usize,
    pub subscribers: usize,
    pub active_auth_contexts: usize,
    pub active_dck_contexts: usize,
    pub pending_actions: usize,
    pub open_alarms: usize,
    pub known_nodes: usize,
    pub lab_provider: String,
    pub lab_seed_fingerprint: String,
    pub raw_secrets_exposed_by_management_api: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SecurityExport {
    pub generated_at: String,
    pub status: SecurityCoreStatus,
    pub policy: SecurityPolicyRecord,
    pub profiles: Vec<SecurityProfileRecord>,
    pub subscribers: Vec<SubscriberSecurityRecord>,
    pub auth_contexts: Vec<AuthenticationContextRecord>,
    pub dck_contexts: Vec<DckContextRecord>,
    pub actions: Vec<EdgeActionRecord>,
    pub alarms: Vec<SecurityAlarmRecord>,
    pub nodes: Vec<SecurityNodeRecord>,
    pub audit: Vec<SecurityAuditRecord>,
}

impl SharedSecurityCore {
    pub fn load(config: SecurityCoreConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let lab_seed = load_or_create_seed(&config.storage.lab_seed_path)
            .map_err(std::io::Error::other)?;
        let database = load_database(&config)?;
        let core = Self {
            inner: Arc::new(Mutex::new(SecurityCoreState {
                config,
                database,
                secrets: RuntimeSecrets {
                    lab_seed,
                    challenges: BTreeMap::new(),
                    expected_responses: BTreeMap::new(),
                    dck_material: BTreeMap::new(),
                    action_payloads: BTreeMap::new(),
                },
                started_at: now_iso(),
                node_gateway_connected: false,
                node_gateway_last_error: None,
                node_gateway_last_seen: None,
            })),
        };
        core.recover_after_restart()?;
        Ok(core)
    }

    fn recover_after_restart(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut state = self.inner.lock().expect("security state poisoned");
        let now = now_iso();
        let mut changed = false;
        for context in state.database.auth_contexts.values_mut() {
            if matches!(
                context.state,
                AuthenticationState::ChallengePending | AuthenticationState::AwaitingResponse
            ) {
                context.state = AuthenticationState::Expired;
                context.completed_at = Some(now.clone());
                context.failure_reason = Some(
                    "ephemeral challenge material was intentionally not persisted across restart"
                        .to_string(),
                );
                changed = true;
            }
        }
        for action in state.database.actions.values_mut() {
            if matches!(action.state, EdgeActionState::Pending | EdgeActionState::InFlight) {
                action.state = EdgeActionState::Failed;
                action.updated_at = now.clone();
                action.last_error = Some(
                    "ephemeral edge payload was intentionally not persisted across restart"
                        .to_string(),
                );
                changed = true;
            }
        }
        for dck in state.database.dck_contexts.values_mut() {
            if matches!(dck.state, DckState::PendingInstall | DckState::Active) {
                dck.state = DckState::Revoked;
                dck.revoked_at = Some(now.clone());
                dck.revoke_reason = Some(
                    "DCK material was intentionally not persisted across restart".to_string(),
                );
                changed = true;
            }
        }
        if changed {
            for subscriber in state.database.subscribers.values_mut() {
                subscriber.authenticated = false;
                subscriber.active_auth_context_id = None;
                subscriber.active_dck_id = None;
            }
        }
        if changed {
            state.database.revision = state.database.revision.saturating_add(1);
            add_audit_locked(
                &mut state,
                "system",
                "restart_recovery",
                "ephemeral-security-state",
                "applied",
                json!({"secret_material_restored":false}),
            );
            persist_locked(&state)?;
        }
        Ok(())
    }

    pub fn status(&self) -> SecurityCoreStatus {
        let state = self.inner.lock().expect("security state poisoned");
        status_locked(&state)
    }

    pub fn policy(&self) -> SecurityPolicyRecord {
        self.inner
            .lock()
            .expect("security state poisoned")
            .database
            .policy
            .clone()
    }

    pub fn profiles(&self) -> Vec<SecurityProfileRecord> {
        self.inner
            .lock()
            .expect("security state poisoned")
            .database
            .profiles
            .values()
            .cloned()
            .collect()
    }

    pub fn profile(&self, issi: u32) -> Option<SecurityProfileRecord> {
        self.inner
            .lock()
            .expect("security state poisoned")
            .database
            .profiles
            .get(&issi)
            .cloned()
    }

    pub fn subscribers(&self) -> Vec<SubscriberSecurityRecord> {
        self.inner
            .lock()
            .expect("security state poisoned")
            .database
            .subscribers
            .values()
            .cloned()
            .collect()
    }

    pub fn auth_contexts(&self) -> Vec<AuthenticationContextRecord> {
        self.inner
            .lock()
            .expect("security state poisoned")
            .database
            .auth_contexts
            .values()
            .cloned()
            .collect()
    }

    pub fn auth_context(&self, id: &str) -> Option<AuthenticationContextRecord> {
        self.inner
            .lock()
            .expect("security state poisoned")
            .database
            .auth_contexts
            .get(id)
            .cloned()
    }

    pub fn dck_contexts(&self) -> Vec<DckContextRecord> {
        self.inner
            .lock()
            .expect("security state poisoned")
            .database
            .dck_contexts
            .values()
            .cloned()
            .collect()
    }

    pub fn actions(&self) -> Vec<EdgeActionRecord> {
        self.inner
            .lock()
            .expect("security state poisoned")
            .database
            .actions
            .values()
            .cloned()
            .collect()
    }

    pub fn alarms(&self) -> Vec<SecurityAlarmRecord> {
        self.inner
            .lock()
            .expect("security state poisoned")
            .database
            .alarms
            .values()
            .cloned()
            .collect()
    }

    pub fn audit(&self, limit: usize) -> Vec<SecurityAuditRecord> {
        let state = self.inner.lock().expect("security state poisoned");
        state
            .database
            .audit
            .iter()
            .rev()
            .take(limit.min(state.config.limits.max_audit))
            .cloned()
            .collect()
    }

    pub fn nodes(&self) -> Vec<SecurityNodeRecord> {
        self.inner
            .lock()
            .expect("security state poisoned")
            .database
            .nodes
            .values()
            .cloned()
            .collect()
    }

    pub fn export(&self) -> SecurityExport {
        let state = self.inner.lock().expect("security state poisoned");
        SecurityExport {
            generated_at: now_iso(),
            status: status_locked(&state),
            policy: state.database.policy.clone(),
            profiles: state.database.profiles.values().cloned().collect(),
            subscribers: state.database.subscribers.values().cloned().collect(),
            auth_contexts: state.database.auth_contexts.values().cloned().collect(),
            dck_contexts: state.database.dck_contexts.values().cloned().collect(),
            actions: state.database.actions.values().cloned().collect(),
            alarms: state.database.alarms.values().cloned().collect(),
            nodes: state.database.nodes.values().cloned().collect(),
            audit: state.database.audit.iter().cloned().collect(),
        }
    }

    pub fn redacted_config(&self) -> Value {
        let state = self.inner.lock().expect("security state poisoned");
        json!({
            "server": state.config.server.clone(),
            "node_gateway": state.config.node_gateway.clone(),
            "storage": {
                "database_path": state.config.storage.database_path.clone(),
                "backup_path": state.config.storage.backup_path.clone(),
                "lab_seed_path": state.config.storage.lab_seed_path.clone(),
                "lab_seed_fingerprint": fingerprint(&state.secrets.lab_seed),
                "raw_seed": "never exposed"
            },
            "policy": state.database.policy.clone(),
            "authentication": state.config.authentication.clone(),
            "dck": state.config.dck.clone(),
            "security": state.config.security.clone(),
            "limits": state.config.limits.clone()
        })
    }

    pub fn upsert_profile(&self, input: ProfileInput, actor: &str) -> Result<SecurityProfileRecord, String> {
        validate_issi(input.issi)?;
        let mut state = self.inner.lock().expect("security state poisoned");
        if !state.database.profiles.contains_key(&input.issi)
            && state.database.profiles.len() >= state.config.limits.max_profiles
        {
            return Err("profile limit reached".to_string());
        }
        let now = now_iso();
        let existing = state.database.profiles.get(&input.issi).cloned();
        let minimum = input
            .minimum_security_class
            .or_else(|| existing.as_ref().map(|entry| entry.minimum_security_class))
            .unwrap_or(state.database.policy.minimum_security_class);
        let preferred = input
            .preferred_security_class
            .or_else(|| existing.as_ref().map(|entry| entry.preferred_security_class))
            .unwrap_or(state.database.policy.default_security_class);
        validate_security_class(minimum)?;
        validate_security_class(preferred)?;
        if minimum > preferred {
            return Err("minimum_security_class may not exceed preferred_security_class".to_string());
        }
        let profile = SecurityProfileRecord {
            issi: input.issi,
            display_name: input
                .display_name
                .or_else(|| existing.as_ref().map(|entry| entry.display_name.clone()))
                .unwrap_or_else(|| format!("ISSI {}", input.issi)),
            authentication_required: input
                .authentication_required
                .or_else(|| existing.as_ref().map(|entry| entry.authentication_required))
                .unwrap_or(state.database.policy.authentication_required),
            minimum_security_class: minimum,
            preferred_security_class: preferred,
            allow_class1_fallback: input
                .allow_class1_fallback
                .or_else(|| existing.as_ref().map(|entry| entry.allow_class1_fallback))
                .unwrap_or(state.database.policy.allow_class1_fallback),
            allowed_nodes: if input.allowed_nodes.is_empty() {
                existing
                    .as_ref()
                    .map(|entry| entry.allowed_nodes.clone())
                    .unwrap_or_default()
            } else {
                unique_strings(input.allowed_nodes)
            },
            max_failures: input
                .max_failures
                .or_else(|| existing.as_ref().map(|entry| entry.max_failures))
                .unwrap_or(state.config.authentication.max_attempts)
                .max(1),
            disabled: existing.as_ref().is_some_and(|entry| entry.disabled),
            equipment_disabled: existing
                .as_ref()
                .is_some_and(|entry| entry.equipment_disabled),
            equipment_id: existing.as_ref().and_then(|entry| entry.equipment_id.clone()),
            notes: input
                .notes
                .or_else(|| existing.as_ref().map(|entry| entry.notes.clone()))
                .unwrap_or_default(),
            created_at: existing
                .as_ref()
                .map(|entry| entry.created_at.clone())
                .unwrap_or_else(|| now.clone()),
            updated_at: now,
            revision: existing
                .as_ref()
                .map(|entry| entry.revision.saturating_add(1))
                .unwrap_or(1),
        };
        state.database.profiles.insert(input.issi, profile.clone());
        state.database.revision = state.database.revision.saturating_add(1);
        add_audit_locked(
            &mut state,
            actor,
            "profile_upsert",
            &format!("issi:{}", input.issi),
            "success",
            json!({
                "minimum_security_class": profile.minimum_security_class,
                "preferred_security_class": profile.preferred_security_class,
                "authentication_required": profile.authentication_required,
                "allowed_nodes": profile.allowed_nodes.clone()
            }),
        );
        persist_locked(&state).map_err(|error| error.to_string())?;
        Ok(profile)
    }

    pub fn delete_profile(&self, issi: u32, actor: &str) -> Result<(), String> {
        let mut state = self.inner.lock().expect("security state poisoned");
        if state.database.profiles.remove(&issi).is_none() {
            return Err("profile not found".to_string());
        }
        state.database.revision = state.database.revision.saturating_add(1);
        add_audit_locked(
            &mut state,
            actor,
            "profile_delete",
            &format!("issi:{issi}"),
            "success",
            json!({}),
        );
        persist_locked(&state).map_err(|error| error.to_string())
    }

    pub fn update_policy(&self, input: PolicyInput, actor: &str) -> Result<SecurityPolicyRecord, String> {
        let mut state = self.inner.lock().expect("security state poisoned");
        let mut policy = state.database.policy.clone();
        if let Some(mode) = input.operating_mode {
            if !matches!(mode.as_str(), OPERATING_MODE_SHADOW | OPERATING_MODE_AUTHORITATIVE) {
                return Err("operating_mode must be shadow or authoritative".to_string());
            }
            policy.operating_mode = mode;
        }
        if let Some(value) = input.default_security_class {
            validate_security_class(value)?;
            policy.default_security_class = value;
        }
        if let Some(value) = input.minimum_security_class {
            validate_security_class(value)?;
            policy.minimum_security_class = value;
        }
        if policy.minimum_security_class > policy.default_security_class {
            return Err("minimum_security_class may not exceed default_security_class".to_string());
        }
        if let Some(value) = input.authentication_required {
            policy.authentication_required = value;
        }
        if let Some(value) = input.allow_class1_fallback {
            policy.allow_class1_fallback = value;
        }
        if let Some(value) = input.reject_unknown_subscribers {
            policy.reject_unknown_subscribers = value;
        }
        if let Some(value) = input.disable_after_failures {
            policy.disable_after_failures = value;
        }
        policy.revision = policy.revision.saturating_add(1);
        policy.updated_at = now_iso();
        state.database.policy = policy.clone();
        state.database.revision = state.database.revision.saturating_add(1);
        add_audit_locked(
            &mut state,
            actor,
            "policy_update",
            "global-policy",
            "success",
            json!({
                "revision": policy.revision,
                "operating_mode": policy.operating_mode.clone(),
                "default_security_class": policy.default_security_class,
                "minimum_security_class": policy.minimum_security_class,
                "authentication_required": policy.authentication_required,
                "allow_class1_fallback": policy.allow_class1_fallback,
                "reject_unknown_subscribers": policy.reject_unknown_subscribers,
                "disable_after_failures": policy.disable_after_failures
            }),
        );
        persist_locked(&state).map_err(|error| error.to_string())?;
        Ok(policy)
    }

    pub fn start_authentication(
        &self,
        input: AuthenticationStartInput,
    ) -> Result<AuthenticationContextRecord, String> {
        validate_issi(input.issi)?;
        if input.node_id.trim().is_empty() {
            return Err("node_id may not be empty".to_string());
        }
        let mut state = self.inner.lock().expect("security state poisoned");
        expire_locked(&mut state);
        if state.database.auth_contexts.len() >= state.config.limits.max_contexts {
            return Err("authentication context limit reached".to_string());
        }
        let profile = state.database.profiles.get(&input.issi).cloned();
        if profile.is_none() && state.database.policy.reject_unknown_subscribers {
            raise_alarm_locked(
                &mut state,
                "warning",
                "unknown_subscriber",
                Some(input.issi),
                Some(input.node_id.clone()),
                None,
                "authentication rejected because the subscriber has no security profile",
            );
            return Err("unknown subscriber rejected by policy".to_string());
        }
        let effective = effective_profile(&state, input.issi, profile.as_ref());
        if effective.disabled || effective.equipment_disabled {
            return Err("subscriber or equipment is disabled".to_string());
        }
        if !effective.allowed_nodes.is_empty()
            && !effective.allowed_nodes.iter().any(|node| node == &input.node_id)
        {
            raise_alarm_locked(
                &mut state,
                "warning",
                "node_policy_violation",
                Some(input.issi),
                Some(input.node_id.clone()),
                None,
                "subscriber attempted authentication on a node outside the allowed-node profile",
            );
            return Err("subscriber is not allowed on this node".to_string());
        }
        if let Some(record) = state.database.subscribers.get(&input.issi) {
            if record.disabled || record.equipment_disabled {
                return Err("subscriber or equipment is disabled".to_string());
            }
            if lockout_active(record) {
                return Err(format!(
                    "subscriber is locked out until {}",
                    record.lockout_until.clone().unwrap_or_default()
                ));
            }
        }

        let requested = input
            .requested_security_class
            .unwrap_or(effective.preferred_security_class);
        validate_security_class(requested)?;
        let supported = normalise_supported_classes(&input.supported_security_classes, requested)?;
        let negotiated = negotiate_security_class(
            requested,
            effective.minimum_security_class,
            effective.preferred_security_class,
            effective.allow_class1_fallback,
            &supported,
        )?;
        let context_id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let challenge_ttl_secs = state.config.authentication.challenge_ttl_secs;
        let expires = now.clone() + ChronoDuration::seconds(challenge_ttl_secs as i64);
        let source = input.source.unwrap_or_else(|| "edge-api".to_string());
        // TETRA security class 3 depends on successful authentication because the DCK
        // is derived from that exchange. A profile may relax authentication for class 1/2,
        // but it may never bypass it for class 3.
        let auth_required = effective.authentication_required || negotiated == 3;

        let mut context = AuthenticationContextRecord {
            id: context_id.clone(),
            issi: input.issi,
            node_id: input.node_id.clone(),
            equipment_id: input.equipment_id.clone(),
            requested_security_class: requested,
            supported_security_classes: supported,
            negotiated_security_class: negotiated,
            state: if auth_required {
                AuthenticationState::ChallengePending
            } else {
                AuthenticationState::Authenticated
            },
            provider: state.config.authentication.provider.clone(),
            challenge_fingerprint: "not-issued".to_string(),
            response_fingerprint: None,
            challenge_action_id: None,
            dck_id: None,
            created_at: format_time(now.clone()),
            expires_at: format_time(expires),
            completed_at: if auth_required { None } else { Some(now_iso()) },
            attempts: 0,
            max_attempts: effective.max_failures,
            source,
            failure_reason: None,
        };

        let subscriber = state.database.subscribers.entry(input.issi).or_insert_with(|| {
            SubscriberSecurityRecord {
                issi: input.issi,
                current_node_id: None,
                equipment_id: None,
                negotiated_security_class: None,
                authenticated: false,
                active_auth_context_id: None,
                active_dck_id: None,
                authentication_failures: 0,
                lockout_until: None,
                disabled: false,
                equipment_disabled: false,
                last_authentication_at: None,
                last_failure_at: None,
                last_seen_at: now_iso(),
            }
        });
        subscriber.current_node_id = Some(input.node_id.clone());
        subscriber.equipment_id = input.equipment_id;
        subscriber.negotiated_security_class = Some(negotiated);
        subscriber.active_auth_context_id = Some(context_id.clone());
        subscriber.disabled = effective.disabled;
        subscriber.equipment_disabled = effective.equipment_disabled;
        subscriber.last_seen_at = now_iso();

        if auth_required {
            let challenge = random_bytes(state.config.authentication.challenge_bytes)?;
            let subscriber_key = derive_subscriber_key(&state.secrets.lab_seed, input.issi)?;
            let expected = expected_response(
                &subscriber_key,
                input.issi,
                &input.node_id,
                &context_id,
                &challenge,
                state.config.authentication.response_bytes,
            )?;
            context.challenge_fingerprint = fingerprint(&challenge);
            context.state = AuthenticationState::ChallengePending;
            state.secrets.challenges.insert(context_id.clone(), challenge.clone());
            state
                .secrets
                .expected_responses
                .insert(context_id.clone(), expected);
            let action = create_action_locked(
                &mut state,
                input.node_id.clone(),
                Some(input.issi),
                Some(context_id.clone()),
                None,
                "authentication_challenge",
                true,
                json!({
                    "kind":"authentication_challenge",
                    "context_id":context_id.clone(),
                    "issi":input.issi,
                    "security_class":negotiated,
                    "challenge_hex":encode_hex(&challenge),
                    "expires_at":context.expires_at.clone(),
                    "provider":"lab_hmac_sha256"
                }),
                challenge_ttl_secs,
            );
            context.challenge_action_id = Some(action.id);
        } else {
            let subscriber = state.database.subscribers.get_mut(&input.issi).expect("subscriber exists");
            subscriber.authenticated = true;
            subscriber.last_authentication_at = Some(now_iso());
            subscriber.authentication_failures = 0;
            subscriber.lockout_until = None;
        }

        state
            .database
            .auth_contexts
            .insert(context_id.clone(), context.clone());
        state.database.revision = state.database.revision.saturating_add(1);
        add_audit_locked(
            &mut state,
            "edge",
            "authentication_start",
            &format!("auth-context:{context_id}"),
            "accepted",
            json!({
                "issi":input.issi,
                "node_id":input.node_id,
                "requested_security_class":requested,
                "negotiated_security_class":negotiated,
                "authentication_required":auth_required,
                "challenge_fingerprint":context.challenge_fingerprint.clone()
            }),
        );
        persist_locked(&state).map_err(|error| error.to_string())?;
        Ok(context)
    }

    pub fn submit_authentication_response(
        &self,
        id: &str,
        input: AuthenticationResponseInput,
    ) -> Result<AuthenticationContextRecord, String> {
        let response = decode_hex(&input.response_hex)?;
        let mut state = self.inner.lock().expect("security state poisoned");
        expire_locked(&mut state);
        let context = state
            .database
            .auth_contexts
            .get(id)
            .cloned()
            .ok_or_else(|| "authentication context not found".to_string())?;
        if context.state != AuthenticationState::AwaitingResponse {
            return Err(format!(
                "authentication context is in state {:?}",
                context.state
            ));
        }
        if let Some(node_id) = input.node_id.as_deref()
            && node_id != context.node_id
        {
            return Err("response node_id does not match authentication context".to_string());
        }
        let expected = state
            .secrets
            .expected_responses
            .get(id)
            .cloned()
            .ok_or_else(|| "ephemeral verifier is unavailable; restart the authentication".to_string())?;
        let source = input.source.unwrap_or_else(|| "edge-api".to_string());
        let success = constant_time_eq(&response, &expected);
        if !success {
            let updated = fail_authentication_locked(
                &mut state,
                id,
                &source,
                "challenge response did not match the expected verifier",
            )?;
            persist_locked(&state).map_err(|error| error.to_string())?;
            return Ok(updated);
        }

        let challenge = state
            .secrets
            .challenges
            .get(id)
            .cloned()
            .ok_or_else(|| "ephemeral challenge is unavailable; restart the authentication".to_string())?;
        let subscriber_key = derive_subscriber_key(&state.secrets.lab_seed, context.issi)?;
        let response_fingerprint = fingerprint(&response);
        let now = now_iso();
        let mut dck_id = None;

        if context.negotiated_security_class == 3 && state.config.authentication.issue_dck_on_success {
            let dck = derive_dck(
                &subscriber_key,
                context.issi,
                &context.node_id,
                id,
                &challenge,
                &response,
                state.config.dck.key_bytes,
            )?;
            revoke_excess_dcks_locked(&mut state, context.issi);
            let id_value = Uuid::new_v4().to_string();
            let key_reference = format!("dck:{}:{}", context.issi, &id_value[..8]);
            let dck_ttl_secs = state.config.dck.ttl_secs;
            let expires = Utc::now() + ChronoDuration::seconds(dck_ttl_secs as i64);
            let action = create_action_locked(
                &mut state,
                context.node_id.clone(),
                Some(context.issi),
                Some(context.id.clone()),
                Some(id_value.clone()),
                "install_dck",
                true,
                json!({
                    "kind":"install_dck",
                    "context_id":context.id.clone(),
                    "dck_id":id_value.clone(),
                    "key_reference":key_reference.clone(),
                    "issi":context.issi,
                    "security_class":3,
                    "dck_hex":encode_hex(&dck),
                    "expires_at":format_time(expires.clone())
                }),
                dck_ttl_secs,
            );
            let record = DckContextRecord {
                id: id_value.clone(),
                key_reference,
                key_fingerprint: fingerprint(&dck),
                issi: context.issi,
                node_id: context.node_id.clone(),
                auth_context_id: context.id.clone(),
                security_class: 3,
                state: DckState::PendingInstall,
                issued_at: now.clone(),
                expires_at: format_time(expires),
                installed_at: None,
                revoked_at: None,
                revoke_reason: None,
                install_action_id: Some(action.id),
                usage_count: 0,
            };
            state.secrets.dck_material.insert(id_value.clone(), dck);
            state.database.dck_contexts.insert(id_value.clone(), record);
            dck_id = Some(id_value);
        }

        let updated = state
            .database
            .auth_contexts
            .get_mut(id)
            .expect("authentication context still exists");
        updated.state = AuthenticationState::Authenticated;
        updated.response_fingerprint = Some(response_fingerprint.clone());
        updated.completed_at = Some(now.clone());
        updated.dck_id = dck_id.clone();
        updated.failure_reason = None;
        let result = updated.clone();

        let subscriber = state
            .database
            .subscribers
            .get_mut(&context.issi)
            .expect("subscriber state exists");
        subscriber.authenticated = true;
        subscriber.active_dck_id = dck_id;
        subscriber.authentication_failures = 0;
        subscriber.lockout_until = None;
        subscriber.last_authentication_at = Some(now);
        subscriber.last_seen_at = now_iso();

        state.secrets.expected_responses.remove(id);
        state.secrets.challenges.remove(id);
        state.database.revision = state.database.revision.saturating_add(1);
        add_audit_locked(
            &mut state,
            &source,
            "authentication_success",
            &format!("auth-context:{id}"),
            "success",
            json!({
                "issi":context.issi,
                "node_id":context.node_id.clone(),
                "security_class":context.negotiated_security_class,
                "response_fingerprint":response_fingerprint,
                "dck_issued":result.dck_id.is_some()
            }),
        );
        persist_locked(&state).map_err(|error| error.to_string())?;
        Ok(result)
    }

    pub fn revoke_authentication(
        &self,
        id: &str,
        input: RevokeInput,
    ) -> Result<AuthenticationContextRecord, String> {
        let mut state = self.inner.lock().expect("security state poisoned");
        let context = state
            .database
            .auth_contexts
            .get(id)
            .cloned()
            .ok_or_else(|| "authentication context not found".to_string())?;
        let reason = input.reason.unwrap_or_else(|| "operator revoke".to_string());
        let actor = input.actor.unwrap_or_else(|| "open-lab-operator".to_string());
        revoke_auth_context_locked(&mut state, id, &reason)?;
        let action = create_action_locked(
            &mut state,
            context.node_id.clone(),
            Some(context.issi),
            Some(context.id.clone()),
            context.dck_id.clone(),
            "revoke_security_context",
            false,
            json!({
                "kind":"revoke_security_context",
                "context_id":context.id.clone(),
                "issi":context.issi,
                "reason":reason.clone()
            }),
            300,
        );
        add_audit_locked(
            &mut state,
            &actor,
            "authentication_revoke",
            &format!("auth-context:{id}"),
            "success",
            json!({"reason":reason.clone(),"action_id":action.id}),
        );
        persist_locked(&state).map_err(|error| error.to_string())?;
        Ok(state
            .database
            .auth_contexts
            .get(id)
            .expect("context remains")
            .clone())
    }

    pub fn set_disabled(
        &self,
        issi: u32,
        disabled: bool,
        input: DisableInput,
    ) -> Result<SecurityProfileRecord, String> {
        validate_issi(issi)?;
        let mut state = self.inner.lock().expect("security state poisoned");
        let now = now_iso();
        let actor = input.actor.unwrap_or_else(|| "open-lab-operator".to_string());
        let reason = input.reason.unwrap_or_else(|| {
            if disabled {
                "operator disable".to_string()
            } else {
                "operator enable".to_string()
            }
        });
        let default_policy = state.database.policy.clone();
        let default_max_failures = state.config.authentication.max_attempts;
        let profile = state.database.profiles.entry(issi).or_insert_with(|| {
            SecurityProfileRecord {
                issi,
                display_name: format!("ISSI {issi}"),
                authentication_required: default_policy.authentication_required,
                minimum_security_class: default_policy.minimum_security_class,
                preferred_security_class: default_policy.default_security_class,
                allow_class1_fallback: default_policy.allow_class1_fallback,
                allowed_nodes: Vec::new(),
                max_failures: default_max_failures,
                disabled: false,
                equipment_disabled: false,
                equipment_id: None,
                notes: String::new(),
                created_at: now.clone(),
                updated_at: now.clone(),
                revision: 0,
            }
        });
        if input.equipment {
            profile.equipment_disabled = disabled;
            if input.equipment_id.is_some() {
                profile.equipment_id = input.equipment_id.clone();
            }
        } else {
            profile.disabled = disabled;
        }
        profile.updated_at = now.clone();
        profile.revision = profile.revision.saturating_add(1);
        let profile_result = profile.clone();

        let subscriber = state.database.subscribers.entry(issi).or_insert_with(|| {
            SubscriberSecurityRecord {
                issi,
                current_node_id: None,
                equipment_id: input.equipment_id.clone(),
                negotiated_security_class: None,
                authenticated: false,
                active_auth_context_id: None,
                active_dck_id: None,
                authentication_failures: 0,
                lockout_until: None,
                disabled: false,
                equipment_disabled: false,
                last_authentication_at: None,
                last_failure_at: None,
                last_seen_at: now.clone(),
            }
        });
        if input.equipment {
            subscriber.equipment_disabled = disabled;
        } else {
            subscriber.disabled = disabled;
        }
        if disabled {
            subscriber.authenticated = false;
        }
        let node_id = subscriber.current_node_id.clone().unwrap_or_else(|| "*".to_string());

        if disabled {
            revoke_subscriber_contexts_locked(&mut state, issi, &reason);
        }
        let kind = if disabled { "disable" } else { "enable" };
        let action = create_action_locked(
            &mut state,
            node_id,
            Some(issi),
            None,
            None,
            kind,
            false,
            json!({
                "kind":kind,
                "issi":issi,
                "equipment":input.equipment,
                "equipment_id":input.equipment_id.clone(),
                "reason":reason.clone()
            }),
            600,
        );
        state.database.revision = state.database.revision.saturating_add(1);
        add_audit_locked(
            &mut state,
            &actor,
            kind,
            &format!("issi:{issi}"),
            "success",
            json!({
                "equipment":input.equipment,
                "reason":reason,
                "edge_action_id":action.id
            }),
        );
        persist_locked(&state).map_err(|error| error.to_string())?;
        Ok(profile_result)
    }

    pub fn revoke_dck(&self, id: &str, input: RevokeInput) -> Result<DckContextRecord, String> {
        let mut state = self.inner.lock().expect("security state poisoned");
        let reason = input.reason.unwrap_or_else(|| "operator revoke".to_string());
        let actor = input.actor.unwrap_or_else(|| "open-lab-operator".to_string());
        let record = state
            .database
            .dck_contexts
            .get(id)
            .cloned()
            .ok_or_else(|| "DCK context not found".to_string())?;
        revoke_dck_locked(&mut state, id, &reason);
        let action = create_action_locked(
            &mut state,
            record.node_id.clone(),
            Some(record.issi),
            Some(record.auth_context_id.clone()),
            Some(id.to_string()),
            "revoke_dck",
            false,
            json!({
                "kind":"revoke_dck",
                "dck_id":id,
                "key_reference":record.key_reference.clone(),
                "issi":record.issi,
                "reason":reason.clone()
            }),
            300,
        );
        state.database.revision = state.database.revision.saturating_add(1);
        add_audit_locked(
            &mut state,
            &actor,
            "dck_revoke",
            &format!("dck:{id}"),
            "success",
            json!({"reason":reason.clone(),"action_id":action.id}),
        );
        persist_locked(&state).map_err(|error| error.to_string())?;
        Ok(state
            .database
            .dck_contexts
            .get(id)
            .expect("DCK remains")
            .clone())
    }

    pub fn claim_edge_actions(&self, input: EdgeClaimInput) -> Result<Vec<EdgeClaimedAction>, String> {
        if input.node_id.trim().is_empty() {
            return Err("node_id may not be empty".to_string());
        }
        let mut state = self.inner.lock().expect("security state poisoned");
        expire_locked(&mut state);
        if !state.config.security.expose_ephemeral_edge_material {
            return Err("ephemeral edge material export is disabled".to_string());
        }
        if state.database.policy.operating_mode != OPERATING_MODE_AUTHORITATIVE {
            return Ok(Vec::new());
        }
        let limit = input.limit.clamp(1, 250);
        let ids: Vec<String> = state
            .database
            .actions
            .values()
            .filter(|action| {
                action.state == EdgeActionState::Pending
                    && (action.node_id == input.node_id || action.node_id == "*")
            })
            .take(limit)
            .map(|action| action.id.clone())
            .collect();
        let mut claimed = Vec::new();
        for id in ids {
            let payload = match state.secrets.action_payloads.get(&id).cloned() {
                Some(payload) => payload,
                None => {
                    if let Some(action) = state.database.actions.get_mut(&id) {
                        action.state = EdgeActionState::Failed;
                        action.updated_at = now_iso();
                        action.last_error = Some("ephemeral action payload unavailable".to_string());
                    }
                    continue;
                }
            };
            let action = state.database.actions.get_mut(&id).expect("action exists");
            action.state = EdgeActionState::InFlight;
            action.attempts = action.attempts.saturating_add(1);
            action.updated_at = now_iso();
            claimed.push(EdgeClaimedAction {
                id: action.id.clone(),
                sequence: action.sequence,
                node_id: input.node_id.clone(),
                kind: action.kind.clone(),
                issi: action.issi,
                context_id: action.context_id.clone(),
                dck_id: action.dck_id.clone(),
                expires_at: action.expires_at.clone(),
                protocol_version: crate::protocol::EDGE_PROTOCOL_VERSION,
                payload,
            });
        }
        if !claimed.is_empty() {
            state.database.revision = state.database.revision.saturating_add(1);
            add_audit_locked(
                &mut state,
                "edge",
                "edge_actions_claim",
                &format!("node:{}", input.node_id),
                "success",
                json!({"count":claimed.len(),"action_ids":claimed.iter().map(|entry| entry.id.clone()).collect::<Vec<_>>()})
            );
            persist_locked(&state).map_err(|error| error.to_string())?;
        }
        Ok(claimed)
    }

    pub fn acknowledge_edge_action(
        &self,
        id: &str,
        input: EdgeActionAckInput,
    ) -> Result<EdgeActionRecord, String> {
        let mut state = self.inner.lock().expect("security state poisoned");
        let action = state
            .database
            .actions
            .get(id)
            .cloned()
            .ok_or_else(|| "edge action not found".to_string())?;
        if !matches!(action.state, EdgeActionState::InFlight | EdgeActionState::Pending) {
            return Err(format!("edge action is in state {:?}", action.state));
        }

        let result = {
            let updated = state.database.actions.get_mut(id).expect("action exists");
            updated.state = if input.success {
                EdgeActionState::Applied
            } else {
                EdgeActionState::Failed
            };
            updated.updated_at = now_iso();
            updated.last_error = if input.success {
                None
            } else {
                input.message.clone()
            };
            updated.clone()
        };

        let mut deferred_alarm: Option<(String, u32, String, String, String)> = None;

        if action.kind == "authentication_challenge" {
            if let Some(context_id) = action.context_id.as_deref() {
                let challenge_result = state.database.auth_contexts.get_mut(context_id).and_then(|context| {
                    if input.success {
                        if context.state == AuthenticationState::ChallengePending {
                            context.state = AuthenticationState::AwaitingResponse;
                        }
                        None
                    } else {
                        let message = input
                            .message
                            .clone()
                            .unwrap_or_else(|| "edge rejected authentication challenge".to_string());
                        context.state = AuthenticationState::Rejected;
                        context.completed_at = Some(now_iso());
                        context.failure_reason = Some(message.clone());
                        Some((context.issi, context.node_id.clone(), context.id.clone(), message))
                    }
                });

                if let Some((issi, node_id, context_id, message)) = challenge_result {
                    state.secrets.challenges.remove(&context_id);
                    state.secrets.expected_responses.remove(&context_id);
                    if let Some(subscriber) = state.database.subscribers.get_mut(&issi) {
                        subscriber.authenticated = false;
                        if subscriber.active_auth_context_id.as_deref() == Some(context_id.as_str()) {
                            subscriber.active_auth_context_id = None;
                        }
                        subscriber.last_failure_at = Some(now_iso());
                    }
                    deferred_alarm = Some((
                        "authentication_challenge_failed".to_string(),
                        issi,
                        node_id,
                        context_id,
                        message,
                    ));
                }
            }
        }

        if action.kind == "install_dck" {
            if let Some(dck_id) = action.dck_id.as_deref() {
                let install_result = state.database.dck_contexts.get_mut(dck_id).and_then(|dck| {
                    if input.success {
                        dck.state = DckState::Active;
                        dck.installed_at = Some(now_iso());
                        None
                    } else {
                        let message = input
                            .message
                            .clone()
                            .unwrap_or_else(|| "edge rejected DCK installation".to_string());
                        dck.state = DckState::InstallFailed;
                        dck.revoked_at = Some(now_iso());
                        dck.revoke_reason = Some(message.clone());
                        Some((dck.issi, dck.node_id.clone(), dck.auth_context_id.clone(), message))
                    }
                });

                if let Some((issi, node_id, context_id, message)) = install_result {
                    if let Some(context) = state.database.auth_contexts.get_mut(&context_id) {
                        context.state = AuthenticationState::Revoked;
                        context.completed_at = Some(now_iso());
                        context.failure_reason = Some("DCK installation failed at the edge".to_string());
                    }
                    if let Some(subscriber) = state.database.subscribers.get_mut(&issi) {
                        subscriber.authenticated = false;
                        if subscriber.active_dck_id.as_deref() == Some(dck_id) {
                            subscriber.active_dck_id = None;
                        }
                        if subscriber.active_auth_context_id.as_deref() == Some(context_id.as_str()) {
                            subscriber.active_auth_context_id = None;
                        }
                        subscriber.last_failure_at = Some(now_iso());
                    }
                    deferred_alarm = Some((
                        "dck_install_failed".to_string(),
                        issi,
                        node_id,
                        context_id,
                        message,
                    ));
                }
            }
        }

        if let Some((kind, issi, node_id, context_id, message)) = deferred_alarm {
            raise_alarm_locked(
                &mut state,
                "critical",
                &kind,
                Some(issi),
                Some(node_id),
                Some(context_id),
                &message,
            );
        }

        state.secrets.action_payloads.remove(id);
        if action.kind == "install_dck" && !input.success {
            if let Some(dck_id) = action.dck_id.as_deref() {
                state.secrets.dck_material.remove(dck_id);
            }
        }
        state.database.revision = state.database.revision.saturating_add(1);
        add_audit_locked(
            &mut state,
            "edge",
            "edge_action_ack",
            &format!("edge-action:{id}"),
            if input.success { "success" } else { "failed" },
            json!({"kind":action.kind,"message":input.message}),
        );
        persist_locked(&state).map_err(|error| error.to_string())?;
        Ok(result)
    }

    pub fn acknowledge_alarm(
        &self,
        id: &str,
        input: AlarmAckInput,
    ) -> Result<SecurityAlarmRecord, String> {
        let mut state = self.inner.lock().expect("security state poisoned");
        let actor = input.actor.unwrap_or_else(|| "open-lab-operator".to_string());
        let alarm = state
            .database
            .alarms
            .get_mut(id)
            .ok_or_else(|| "alarm not found".to_string())?;
        alarm.state = AlarmState::Acknowledged;
        alarm.acknowledged_at = Some(now_iso());
        alarm.acknowledged_by = Some(actor.clone());
        alarm.acknowledgement_note = input.note.clone();
        let result = alarm.clone();
        state.database.revision = state.database.revision.saturating_add(1);
        add_audit_locked(
            &mut state,
            &actor,
            "alarm_acknowledge",
            &format!("alarm:{id}"),
            "success",
            json!({"note":input.note}),
        );
        persist_locked(&state).map_err(|error| error.to_string())?;
        Ok(result)
    }

    pub fn backup(&self) -> Result<String, String> {
        let state = self.inner.lock().expect("security state poisoned");
        let source = &state.config.storage.database_path;
        let target = &state.config.storage.backup_path;
        if !source.exists() {
            persist_locked(&state).map_err(|error| error.to_string())?;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::copy(source, target).map_err(|error| error.to_string())?;
        Ok(target.display().to_string())
    }

    pub fn maintenance_tick(&self) -> Result<SecurityCoreStatus, String> {
        let mut state = self.inner.lock().expect("security state poisoned");
        let changed = expire_locked(&mut state);
        if changed {
            state.database.revision = state.database.revision.saturating_add(1);
            persist_locked(&state).map_err(|error| error.to_string())?;
        }
        Ok(status_locked(&state))
    }

    pub fn gateway_connected(&self) {
        let mut state = self.inner.lock().expect("security state poisoned");
        state.node_gateway_connected = true;
        state.node_gateway_last_error = None;
        state.node_gateway_last_seen = Some(now_iso());
    }

    pub fn gateway_disconnected(&self, error: String) {
        let mut state = self.inner.lock().expect("security state poisoned");
        state.node_gateway_connected = false;
        state.node_gateway_last_error = Some(error);
        state.node_gateway_last_seen = Some(now_iso());
    }

    pub fn handle_backend_event(&self, event: BackendEvent) {
        let mut state = self.inner.lock().expect("security state poisoned");
        state.node_gateway_last_seen = Some(now_iso());
        match event {
            BackendEvent::Snapshot { snapshot } => {
                for node in snapshot.nodes {
                    state.database.nodes.insert(
                        node.node_id.clone(),
                        SecurityNodeRecord {
                            node_id: node.node_id,
                            station_name: node.identity.station_name,
                            connected: node.connected,
                            stale: node.stale,
                            mcc: Some(node.identity.mcc),
                            mnc: Some(node.identity.mnc),
                            location_area: Some(node.identity.location_area),
                            last_seen: node.last_seen,
                            last_error: node.disconnect_reason,
                        },
                    );
                }
            }
            BackendEvent::NodeMessage { node_id, message } => {
                let now = now_iso();
                let entry = state.database.nodes.entry(node_id.clone()).or_insert_with(|| {
                    SecurityNodeRecord {
                        node_id: node_id.clone(),
                        station_name: node_id.clone(),
                        connected: true,
                        stale: false,
                        mcc: None,
                        mnc: None,
                        location_area: None,
                        last_seen: now.clone(),
                        last_error: None,
                    }
                });
                entry.connected = true;
                entry.stale = false;
                entry.last_seen = now;
                if let tetra_entities::net_control_room::NodeToControlRoomMessage::Hello { hello } = message {
                    entry.station_name = hello.node.station_name;
                    entry.mcc = Some(hello.node.mcc);
                    entry.mnc = Some(hello.node.mnc);
                    entry.location_area = Some(hello.node.location_area);
                }
            }
            BackendEvent::Event { event } => {
                if let Some(node_id) = event.node_id {
                    let entry = state.database.nodes.entry(node_id.clone()).or_insert_with(|| {
                        SecurityNodeRecord {
                            node_id: node_id.clone(),
                            station_name: node_id.clone(),
                            connected: false,
                            stale: false,
                            mcc: None,
                            mnc: None,
                            location_area: None,
                            last_seen: event.timestamp.clone(),
                            last_error: None,
                        }
                    });
                    entry.last_seen = event.timestamp;
                    if event.kind.contains("disconnect") {
                        entry.connected = false;
                        entry.last_error = Some(event.detail.to_string());
                    }
                }
            }
            BackendEvent::ActionResult { ok, message, .. } => {
                if !ok {
                    state.node_gateway_last_error = Some(message);
                }
            }
        }
    }

    pub fn metrics(&self) -> String {
        let status = self.status();
        format!(
            concat!(
                "# HELP netcore_security_core_profiles Configured security profiles\n",
                "# TYPE netcore_security_core_profiles gauge\n",
                "netcore_security_core_profiles {}\n",
                "# HELP netcore_security_core_auth_contexts_active Active authentication contexts\n",
                "# TYPE netcore_security_core_auth_contexts_active gauge\n",
                "netcore_security_core_auth_contexts_active {}\n",
                "# HELP netcore_security_core_dck_contexts_active Active or pending DCK contexts\n",
                "# TYPE netcore_security_core_dck_contexts_active gauge\n",
                "netcore_security_core_dck_contexts_active {}\n",
                "# HELP netcore_security_core_actions_pending Pending edge actions\n",
                "# TYPE netcore_security_core_actions_pending gauge\n",
                "netcore_security_core_actions_pending {}\n",
                "# HELP netcore_security_core_alarms_open Open security alarms\n",
                "# TYPE netcore_security_core_alarms_open gauge\n",
                "netcore_security_core_alarms_open {}\n",
                "# HELP netcore_security_core_node_gateway_connected Node Gateway connection state\n",
                "# TYPE netcore_security_core_node_gateway_connected gauge\n",
                "netcore_security_core_node_gateway_connected {}\n"
            ),
            status.profiles,
            status.active_auth_contexts,
            status.active_dck_contexts,
            status.pending_actions,
            status.open_alarms,
            if status.node_gateway_connected { 1 } else { 0 }
        )
    }
}

#[derive(Debug, Clone)]
struct EffectiveProfile {
    authentication_required: bool,
    minimum_security_class: u8,
    preferred_security_class: u8,
    allow_class1_fallback: bool,
    allowed_nodes: Vec<String>,
    max_failures: u32,
    disabled: bool,
    equipment_disabled: bool,
}

fn effective_profile(
    state: &SecurityCoreState,
    _issi: u32,
    profile: Option<&SecurityProfileRecord>,
) -> EffectiveProfile {
    match profile {
        Some(profile) => EffectiveProfile {
            authentication_required: profile.authentication_required,
            minimum_security_class: profile.minimum_security_class,
            preferred_security_class: profile.preferred_security_class,
            allow_class1_fallback: profile.allow_class1_fallback,
            allowed_nodes: profile.allowed_nodes.clone(),
            max_failures: profile.max_failures,
            disabled: profile.disabled,
            equipment_disabled: profile.equipment_disabled,
        },
        None => EffectiveProfile {
            authentication_required: state.database.policy.authentication_required,
            minimum_security_class: state.database.policy.minimum_security_class,
            preferred_security_class: state.database.policy.default_security_class,
            allow_class1_fallback: state.database.policy.allow_class1_fallback,
            allowed_nodes: Vec::new(),
            max_failures: state.config.authentication.max_attempts,
            disabled: false,
            equipment_disabled: false,
        },
    }
}

fn status_locked(state: &SecurityCoreState) -> SecurityCoreStatus {
    SecurityCoreStatus {
        service: "netcore-security-core",
        version: env!("CARGO_PKG_VERSION"),
        edge_protocol_version: crate::protocol::EDGE_PROTOCOL_VERSION,
        started_at: state.started_at.clone(),
        management_security_mode: "open_lab",
        warning: OPEN_LAB_WARNING,
        operating_mode: state.database.policy.operating_mode.clone(),
        authoritative: state.database.policy.operating_mode == OPERATING_MODE_AUTHORITATIVE,
        node_gateway_connected: state.node_gateway_connected,
        node_gateway_last_error: state.node_gateway_last_error.clone(),
        node_gateway_last_seen: state.node_gateway_last_seen.clone(),
        database_revision: state.database.revision,
        policy_revision: state.database.policy.revision,
        profiles: state.database.profiles.len(),
        subscribers: state.database.subscribers.len(),
        active_auth_contexts: state
            .database
            .auth_contexts
            .values()
            .filter(|context| {
                matches!(
                    context.state,
                    AuthenticationState::ChallengePending
                        | AuthenticationState::AwaitingResponse
                        | AuthenticationState::Authenticated
                )
            })
            .count(),
        active_dck_contexts: state
            .database
            .dck_contexts
            .values()
            .filter(|context| matches!(context.state, DckState::PendingInstall | DckState::Active))
            .count(),
        pending_actions: state
            .database
            .actions
            .values()
            .filter(|action| matches!(action.state, EdgeActionState::Pending | EdgeActionState::InFlight))
            .count(),
        open_alarms: state
            .database
            .alarms
            .values()
            .filter(|alarm| alarm.state == AlarmState::Open)
            .count(),
        known_nodes: state.database.nodes.len(),
        lab_provider: state.config.authentication.provider.clone(),
        lab_seed_fingerprint: fingerprint(&state.secrets.lab_seed),
        raw_secrets_exposed_by_management_api: false,
    }
}

fn fail_authentication_locked(
    state: &mut SecurityCoreState,
    id: &str,
    source: &str,
    reason: &str,
) -> Result<AuthenticationContextRecord, String> {
    let (result, issi, node_id, attempts, max_attempts, context_id, rejected) = {
        let context = state
            .database
            .auth_contexts
            .get_mut(id)
            .ok_or_else(|| "authentication context not found".to_string())?;
        context.attempts = context.attempts.saturating_add(1);
        context.response_fingerprint = None;
        context.failure_reason = Some(reason.to_string());
        let rejected = context.attempts >= context.max_attempts;
        if rejected {
            context.state = AuthenticationState::Rejected;
            context.completed_at = Some(now_iso());
        }
        (
            context.clone(),
            context.issi,
            context.node_id.clone(),
            context.attempts,
            context.max_attempts,
            context.id.clone(),
            rejected,
        )
    };

    let subscriber = state
        .database
        .subscribers
        .get_mut(&issi)
        .ok_or_else(|| "subscriber state missing".to_string())?;
    subscriber.authenticated = false;
    subscriber.authentication_failures = subscriber.authentication_failures.saturating_add(1);
    subscriber.last_failure_at = Some(now_iso());
    if rejected {
        let lockout = Utc::now()
            + ChronoDuration::seconds(state.config.authentication.lockout_secs as i64);
        subscriber.lockout_until = Some(format_time(lockout));
        state.secrets.challenges.remove(id);
        state.secrets.expected_responses.remove(id);
    }

    add_audit_locked(
        state,
        source,
        "authentication_failure",
        &format!("auth-context:{id}"),
        if rejected { "rejected" } else { "retry_allowed" },
        json!({
            "issi":issi,
            "node_id":node_id.clone(),
            "attempts":attempts,
            "max_attempts":max_attempts,
            "reason":reason
        }),
    );
    raise_alarm_locked(
        state,
        if rejected { "critical" } else { "warning" },
        "authentication_failure",
        Some(issi),
        Some(node_id.clone()),
        Some(context_id.clone()),
        reason,
    );
    if rejected && state.database.policy.disable_after_failures {
        let now = now_iso();
        let policy = state.database.policy.clone();
        let max_failures = state.config.authentication.max_attempts;
        let profile = state.database.profiles.entry(issi).or_insert_with(|| {
            SecurityProfileRecord {
                issi,
                display_name: format!("ISSI {issi}"),
                authentication_required: policy.authentication_required,
                minimum_security_class: policy.minimum_security_class,
                preferred_security_class: policy.default_security_class,
                allow_class1_fallback: policy.allow_class1_fallback,
                allowed_nodes: Vec::new(),
                max_failures,
                disabled: false,
                equipment_disabled: false,
                equipment_id: None,
                notes: "Created automatically after repeated authentication failures".to_string(),
                created_at: now.clone(),
                updated_at: now.clone(),
                revision: 0,
            }
        });
        profile.disabled = true;
        profile.updated_at = now;
        profile.revision = profile.revision.saturating_add(1);
        if let Some(subscriber) = state.database.subscribers.get_mut(&issi) {
            subscriber.disabled = true;
            subscriber.active_auth_context_id = None;
            subscriber.active_dck_id = None;
        }
        let action = create_action_locked(
            state,
            node_id,
            Some(issi),
            Some(context_id),
            None,
            "disable",
            false,
            json!({
                "kind":"disable",
                "issi":issi,
                "equipment":false,
                "reason":"automatic disable after repeated authentication failures"
            }),
            600,
        );
        add_audit_locked(
            state,
            "security-core",
            "automatic_disable",
            &format!("issi:{issi}"),
            "success",
            json!({"edge_action_id":action.id,"reason":reason}),
        );
    }
    state.database.revision = state.database.revision.saturating_add(1);
    Ok(result)
}

fn create_action_locked(
    state: &mut SecurityCoreState,
    node_id: String,
    issi: Option<u32>,
    context_id: Option<String>,
    dck_id: Option<String>,
    kind: &str,
    secret_bearing: bool,
    payload: Value,
    ttl_secs: u64,
) -> EdgeActionRecord {
    while state.database.actions.len() >= state.config.limits.max_actions {
        let removable = state
            .database
            .actions
            .values()
            .filter(|action| {
                matches!(
                    action.state,
                    EdgeActionState::Applied
                        | EdgeActionState::Failed
                        | EdgeActionState::Expired
                        | EdgeActionState::Cancelled
                )
            })
            .min_by_key(|action| action.sequence)
            .map(|action| action.id.clone());
        match removable {
            Some(id) => {
                state.database.actions.remove(&id);
                state.secrets.action_payloads.remove(&id);
            }
            None => break,
        }
    }
    let now = Utc::now();
    let id = Uuid::new_v4().to_string();
    let action = EdgeActionRecord {
        id: id.clone(),
        sequence: state.database.next_action_sequence,
        node_id,
        issi,
        context_id,
        dck_id,
        kind: kind.to_string(),
        state: EdgeActionState::Pending,
        secret_bearing,
        created_at: format_time(now.clone()),
        updated_at: format_time(now.clone()),
        expires_at: format_time(now + ChronoDuration::seconds(ttl_secs.max(1) as i64)),
        attempts: 0,
        last_error: None,
    };
    state.database.next_action_sequence = state.database.next_action_sequence.saturating_add(1);
    state.database.actions.insert(id.clone(), action.clone());
    state.secrets.action_payloads.insert(id, payload);
    action
}

fn revoke_auth_context_locked(
    state: &mut SecurityCoreState,
    id: &str,
    reason: &str,
) -> Result<(), String> {
    let (issi, dck_id) = {
        let context = state
            .database
            .auth_contexts
            .get_mut(id)
            .ok_or_else(|| "authentication context not found".to_string())?;
        context.state = AuthenticationState::Revoked;
        context.completed_at = Some(now_iso());
        context.failure_reason = Some(reason.to_string());
        (context.issi, context.dck_id.clone())
    };

    state.secrets.challenges.remove(id);
    state.secrets.expected_responses.remove(id);
    if let Some(dck_id) = dck_id {
        revoke_dck_locked(state, &dck_id, reason);
    }
    if let Some(subscriber) = state.database.subscribers.get_mut(&issi) {
        subscriber.authenticated = false;
        subscriber.active_auth_context_id = None;
        subscriber.active_dck_id = None;
    }
    state.database.revision = state.database.revision.saturating_add(1);
    Ok(())
}

fn revoke_dck_locked(state: &mut SecurityCoreState, id: &str, reason: &str) {
    let issi = if let Some(dck) = state.database.dck_contexts.get_mut(id) {
        dck.state = DckState::Revoked;
        dck.revoked_at = Some(now_iso());
        dck.revoke_reason = Some(reason.to_string());
        Some(dck.issi)
    } else {
        None
    };

    if let Some(issi) = issi {
        if let Some(subscriber) = state.database.subscribers.get_mut(&issi) {
            if subscriber.active_dck_id.as_deref() == Some(id) {
                subscriber.active_dck_id = None;
            }
        }
    }
    state.secrets.dck_material.remove(id);
}

fn revoke_subscriber_contexts_locked(state: &mut SecurityCoreState, issi: u32, reason: &str) {
    let auth_ids: Vec<String> = state
        .database
        .auth_contexts
        .values()
        .filter(|context| context.issi == issi)
        .map(|context| context.id.clone())
        .collect();
    for id in auth_ids {
        let _ = revoke_auth_context_locked(state, &id, reason);
    }
    let dck_ids: Vec<String> = state
        .database
        .dck_contexts
        .values()
        .filter(|context| context.issi == issi)
        .map(|context| context.id.clone())
        .collect();
    for id in dck_ids {
        revoke_dck_locked(state, &id, reason);
    }
}

fn revoke_excess_dcks_locked(state: &mut SecurityCoreState, issi: u32) {
    let mut active: Vec<DckContextRecord> = state
        .database
        .dck_contexts
        .values()
        .filter(|context| {
            context.issi == issi
                && matches!(context.state, DckState::PendingInstall | DckState::Active)
        })
        .cloned()
        .collect();
    active.sort_by(|left, right| left.issued_at.cmp(&right.issued_at));
    let keep = state.config.dck.max_active_per_subscriber.saturating_sub(1);
    while active.len() > keep {
        let oldest = active.remove(0);
        revoke_dck_locked(state, &oldest.id, "superseded by a newer DCK");
    }
}

fn raise_alarm_locked(
    state: &mut SecurityCoreState,
    severity: &str,
    kind: &str,
    issi: Option<u32>,
    node_id: Option<String>,
    context_id: Option<String>,
    message: &str,
) {
    while state.database.alarms.len() >= state.config.limits.max_alarms {
        let removable = state
            .database
            .alarms
            .values()
            .filter(|alarm| alarm.state != AlarmState::Open)
            .min_by_key(|alarm| alarm.created_at.clone())
            .map(|alarm| alarm.id.clone());
        match removable {
            Some(id) => {
                state.database.alarms.remove(&id);
            }
            None => break,
        }
    }
    let id = Uuid::new_v4().to_string();
    state.database.alarms.insert(
        id.clone(),
        SecurityAlarmRecord {
            id,
            severity: severity.to_string(),
            kind: kind.to_string(),
            state: AlarmState::Open,
            issi,
            node_id,
            context_id,
            message: message.to_string(),
            created_at: now_iso(),
            acknowledged_at: None,
            acknowledged_by: None,
            acknowledgement_note: None,
            cleared_at: None,
        },
    );
}

fn add_audit_locked(
    state: &mut SecurityCoreState,
    actor: &str,
    action: &str,
    target: &str,
    outcome: &str,
    detail: Value,
) {
    state.database.audit.push_back(SecurityAuditRecord {
        sequence: state.database.next_audit_sequence,
        timestamp: now_iso(),
        actor: actor.to_string(),
        action: action.to_string(),
        target: target.to_string(),
        outcome: outcome.to_string(),
        detail,
    });
    state.database.next_audit_sequence = state.database.next_audit_sequence.saturating_add(1);
    while state.database.audit.len() > state.config.limits.max_audit {
        state.database.audit.pop_front();
    }
}

fn expire_locked(state: &mut SecurityCoreState) -> bool {
    let now = Utc::now();
    let mut changed = false;
    for context in state.database.auth_contexts.values_mut() {
        if matches!(
            context.state,
            AuthenticationState::ChallengePending | AuthenticationState::AwaitingResponse
        ) && parse_time(&context.expires_at).is_some_and(|expires| expires <= now)
        {
            context.state = AuthenticationState::Expired;
            context.completed_at = Some(format_time(now.clone()));
            context.failure_reason = Some("challenge expired".to_string());
            state.secrets.challenges.remove(&context.id);
            state.secrets.expected_responses.remove(&context.id);
            changed = true;
        }
    }
    for dck in state.database.dck_contexts.values_mut() {
        if matches!(dck.state, DckState::PendingInstall | DckState::Active)
            && parse_time(&dck.expires_at).is_some_and(|expires| expires <= now)
        {
            dck.state = DckState::Expired;
            dck.revoked_at = Some(format_time(now.clone()));
            dck.revoke_reason = Some("DCK lifetime expired".to_string());
            state.secrets.dck_material.remove(&dck.id);
            changed = true;
        }
    }
    for action in state.database.actions.values_mut() {
        if matches!(action.state, EdgeActionState::Pending | EdgeActionState::InFlight)
            && parse_time(&action.expires_at).is_some_and(|expires| expires <= now)
        {
            action.state = EdgeActionState::Expired;
            action.updated_at = format_time(now.clone());
            action.last_error = Some("edge action expired".to_string());
            state.secrets.action_payloads.remove(&action.id);
            changed = true;
        }
    }
    let active_auth_ids: BTreeSet<String> = state
        .database
        .auth_contexts
        .values()
        .filter(|context| {
            matches!(
                context.state,
                AuthenticationState::ChallengePending
                    | AuthenticationState::AwaitingResponse
                    | AuthenticationState::Authenticated
            )
        })
        .map(|context| context.id.clone())
        .collect();
    let active_dck_ids: BTreeSet<String> = state
        .database
        .dck_contexts
        .values()
        .filter(|dck| matches!(dck.state, DckState::PendingInstall | DckState::Active))
        .map(|dck| dck.id.clone())
        .collect();

    for subscriber in state.database.subscribers.values_mut() {
        if subscriber
            .lockout_until
            .as_deref()
            .and_then(parse_time)
            .is_some_and(|expires| expires <= now)
        {
            subscriber.lockout_until = None;
            changed = true;
        }
        if subscriber
            .active_auth_context_id
            .as_ref()
            .is_some_and(|id| !active_auth_ids.contains(id))
        {
            subscriber.active_auth_context_id = None;
            subscriber.authenticated = false;
            changed = true;
        }
        if subscriber
            .active_dck_id
            .as_ref()
            .is_some_and(|id| !active_dck_ids.contains(id))
        {
            subscriber.active_dck_id = None;
            changed = true;
        }
    }
    changed
}

fn lockout_active(record: &SubscriberSecurityRecord) -> bool {
    record
        .lockout_until
        .as_deref()
        .and_then(parse_time)
        .is_some_and(|expires| expires > Utc::now())
}

fn negotiate_security_class(
    requested: u8,
    minimum: u8,
    preferred: u8,
    allow_class1_fallback: bool,
    supported: &[u8],
) -> Result<u8, String> {
    let mut candidates = vec![requested, preferred, 3, 2, 1];
    candidates.dedup();
    for candidate in candidates {
        if candidate < minimum || !supported.contains(&candidate) {
            continue;
        }
        if candidate == 1 && !allow_class1_fallback && requested != 1 {
            continue;
        }
        return Ok(candidate);
    }
    Err(format!(
        "no mutually supported security class satisfies minimum class {minimum}; supported={supported:?}"
    ))
}

fn normalise_supported_classes(values: &[u8], requested: u8) -> Result<Vec<u8>, String> {
    let source = if values.is_empty() {
        vec![requested]
    } else {
        values.to_vec()
    };
    let mut result = Vec::new();
    for value in source {
        validate_security_class(value)?;
        if !result.contains(&value) {
            result.push(value);
        }
    }
    result.sort_unstable();
    Ok(result)
}

fn validate_security_class(value: u8) -> Result<(), String> {
    if (1..=3).contains(&value) {
        Ok(())
    } else {
        Err("security class must be 1, 2 or 3".to_string())
    }
}

fn validate_issi(issi: u32) -> Result<(), String> {
    if issi <= 0x00ff_ffff {
        Ok(())
    } else {
        Err("ISSI must fit into 24 bits".to_string())
    }
}

fn unique_strings(values: Vec<String>) -> Vec<String> {
    let mut result = Vec::new();
    for value in values {
        let trimmed = value.trim();
        if !trimmed.is_empty() && !result.iter().any(|entry| entry == trimmed) {
            result.push(trimmed.to_string());
        }
    }
    result
}

fn load_database(config: &SecurityCoreConfig) -> Result<SecurityDatabase, Box<dyn std::error::Error>> {
    if !config.storage.database_path.exists() {
        return Ok(SecurityDatabase::new(config));
    }
    let bytes = fs::read(&config.storage.database_path)?;
    let database: SecurityDatabase = serde_json::from_slice(&bytes)?;
    if database.schema_version != DATABASE_SCHEMA_VERSION {
        return Err(format!(
            "unsupported Security Core database schema {}; expected {}",
            database.schema_version, DATABASE_SCHEMA_VERSION
        )
        .into());
    }
    Ok(database)
}

fn persist_locked(state: &SecurityCoreState) -> Result<(), Box<dyn std::error::Error>> {
    let path = &state.config.storage.database_path;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("json.tmp");
    let payload = serde_json::to_vec_pretty(&state.database)?;
    let mut file = fs::File::create(&temporary)?;
    file.write_all(&payload)?;
    file.sync_all()?;
    fs::rename(&temporary, path)?;
    Ok(())
}

fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn format_time(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn parse_time(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|value| value.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_negotiation_prefers_requested() {
        assert_eq!(negotiate_security_class(3, 1, 1, true, &[1, 3]).unwrap(), 3);
    }

    #[test]
    fn class_negotiation_can_fall_back_to_class_one() {
        assert_eq!(negotiate_security_class(3, 1, 3, true, &[1]).unwrap(), 1);
    }

    #[test]
    fn class_negotiation_rejects_forbidden_fallback() {
        assert!(negotiate_security_class(3, 1, 3, false, &[1]).is_err());
    }
}

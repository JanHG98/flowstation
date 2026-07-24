use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::config::{
    KmfConfig, OPERATING_MODE_AUTHORITATIVE, OPERATING_MODE_SHADOW,
};
use crate::crypto::{
    SealedBlob, derive_node_transport_key, fingerprint, hex_encode,
    load_or_create_secret, open, random_bytes, seal, sha256_hex, write_private_file,
};
use crate::protocol::{
    BackupInput, EdgeActionAckInput, EdgeClaimInput, KeyCreateInput, KeyRotateInput,
    LifecycleInput, NodeCreateInput, NodeStateInput, OtarApprovalInput, OtarJobCreateInput,
    OtarQueueInput, PolicyInput, OTAR_EDGE_PROTOCOL_VERSION,
};

const DATABASE_SCHEMA_VERSION: u32 = 1;
const VAULT_SCHEMA_VERSION: u32 = 1;
const SERVICE_WARNING: &str =
    "OPEN LAB: no authentication, no tokens and no TLS. Restrict this service to an isolated management network.";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyState {
    Draft,
    Staged,
    Active,
    Retiring,
    Retired,
    Revoked,
    Destroyed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OtarJobState {
    AwaitingApproval,
    Approved,
    Staged,
    Queued,
    InProgress,
    Completed,
    PartialFailure,
    Failed,
    Cancelled,
    Expired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryState {
    Staged,
    Pending,
    InFlight,
    Applied,
    Failed,
    Expired,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionState {
    Staged,
    Pending,
    InFlight,
    Applied,
    Failed,
    Expired,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRecord {
    pub revision: u64,
    pub operating_mode: String,
    pub default_key_bytes: usize,
    pub default_crypto_period_secs: u64,
    pub rotation_lead_secs: u64,
    pub require_dual_approval: bool,
    pub allow_overlapping_crypto_periods: bool,
    pub auto_retire_predecessor: bool,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyRecord {
    pub id: String,
    pub kind: String,
    pub scope: String,
    pub scope_value: Option<String>,
    pub label: String,
    pub algorithm_profile: String,
    pub key_bytes: usize,
    pub version: u32,
    pub state: KeyState,
    pub fingerprint: String,
    pub material_reference: Option<String>,
    pub crypto_period_start: String,
    pub crypto_period_end: String,
    pub predecessor_id: Option<String>,
    pub successor_id: Option<String>,
    pub created_at: String,
    pub created_by: String,
    pub activated_at: Option<String>,
    pub retired_at: Option<String>,
    pub revoked_at: Option<String>,
    pub destroyed_at: Option<String>,
    pub lifecycle_reason: Option<String>,
    pub notes: String,
    pub revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeTransportRecord {
    pub node_id: String,
    pub display_name: String,
    pub enabled: bool,
    pub transport_key_fingerprint: String,
    pub secret_reference: Option<String>,
    pub bootstrap_path: String,
    pub created_at: String,
    pub created_by: String,
    pub updated_at: String,
    pub disabled_reason: Option<String>,
    pub last_claim_at: Option<String>,
    pub last_ack_at: Option<String>,
    pub notes: String,
    pub revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtarApprovalRecord {
    pub actor: String,
    pub timestamp: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtarDeliveryRecord {
    pub id: String,
    pub node_id: String,
    pub state: DeliveryState,
    pub action_id: Option<String>,
    pub attempts: u32,
    pub created_at: String,
    pub updated_at: String,
    pub applied_at: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtarJobRecord {
    pub id: String,
    pub key_id: String,
    pub key_fingerprint: String,
    pub key_kind: String,
    pub key_version: u32,
    pub state: OtarJobState,
    pub target_nodes: Vec<String>,
    pub target_issis: Vec<u32>,
    pub target_gssis: Vec<u32>,
    pub approvals: Vec<OtarApprovalRecord>,
    pub required_approvals: usize,
    pub deliveries: Vec<OtarDeliveryRecord>,
    pub created_at: String,
    pub created_by: String,
    pub not_before: String,
    pub expires_at: String,
    pub queued_at: Option<String>,
    pub completed_at: Option<String>,
    pub notes: String,
    pub revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtarActionRecord {
    pub id: String,
    pub sequence: u64,
    pub job_id: String,
    pub delivery_id: String,
    pub node_id: String,
    pub key_id: String,
    pub state: ActionState,
    pub created_at: String,
    pub updated_at: String,
    pub expires_at: String,
    pub attempts: u32,
    pub next_attempt_at: String,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClaimedOtarAction {
    pub protocol_version: &'static str,
    pub id: String,
    pub sequence: u64,
    pub job_id: String,
    pub delivery_id: String,
    pub node_id: String,
    pub key_id: String,
    pub key_kind: String,
    pub key_version: u32,
    pub key_fingerprint: String,
    pub algorithm_profile: String,
    pub crypto_period_start: String,
    pub crypto_period_end: String,
    pub target_issis: Vec<u32>,
    pub target_gssis: Vec<u32>,
    pub envelope: SealedBlob,
    pub envelope_context: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecord {
    pub sequence: u64,
    pub timestamp: String,
    pub actor: String,
    pub action: String,
    pub target: String,
    pub outcome: String,
    pub detail: Value,
    pub previous_hash: String,
    pub record_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupRecord {
    pub id: String,
    pub created_at: String,
    pub created_by: String,
    pub directory: String,
    pub metadata_sha256: String,
    pub vault_sha256: String,
    pub audit_head_hash: String,
    pub note: String,
    pub verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct KmfDatabase {
    schema_version: u32,
    revision: u64,
    next_action_sequence: u64,
    next_audit_sequence: u64,
    policy: PolicyRecord,
    keys: BTreeMap<String, KeyRecord>,
    nodes: BTreeMap<String, NodeTransportRecord>,
    jobs: BTreeMap<String, OtarJobRecord>,
    actions: BTreeMap<String, OtarActionRecord>,
    audit: VecDeque<AuditRecord>,
    backups: VecDeque<BackupRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VaultDatabase {
    schema_version: u32,
    revision: u64,
    entries: BTreeMap<String, SealedBlob>,
}

struct KmfState {
    config: KmfConfig,
    database: KmfDatabase,
    vault: VaultDatabase,
    master_key: Vec<u8>,
    started_at: String,
    last_error: Option<String>,
}

#[derive(Clone)]
pub struct SharedKmf {
    inner: Arc<Mutex<KmfState>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KmfStatus {
    pub service: &'static str,
    pub version: &'static str,
    pub started_at: String,
    pub management_security_mode: &'static str,
    pub warning: &'static str,
    pub operating_mode: String,
    pub authoritative: bool,
    pub database_revision: u64,
    pub vault_revision: u64,
    pub vault_provider: String,
    pub vault_ready: bool,
    pub master_key_fingerprint: String,
    pub raw_keys_exposed_by_management_api: bool,
    pub total_keys: usize,
    pub active_keys: usize,
    pub staged_keys: usize,
    pub revoked_keys: usize,
    pub node_transport_profiles: usize,
    pub enabled_nodes: usize,
    pub otar_jobs: usize,
    pub pending_actions: usize,
    pub audit_records: usize,
    pub audit_head_hash: String,
    pub backups: usize,
    pub last_error: Option<String>,
    pub hsm_configured: bool,
    pub hsm_connected: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RedactedExport {
    pub generated_at: String,
    pub status: KmfStatus,
    pub policy: PolicyRecord,
    pub keys: Vec<KeyRecord>,
    pub nodes: Vec<NodeTransportRecord>,
    pub jobs: Vec<OtarJobRecord>,
    pub actions: Vec<OtarActionRecord>,
    pub audit: Vec<AuditRecord>,
    pub backups: Vec<BackupRecord>,
    pub note: &'static str,
}

impl KmfDatabase {
    fn new(config: &KmfConfig) -> Self {
        let now = now_iso();
        Self {
            schema_version: DATABASE_SCHEMA_VERSION,
            revision: 0,
            next_action_sequence: 1,
            next_audit_sequence: 1,
            policy: PolicyRecord {
                revision: 1,
                operating_mode: config.policy.operating_mode.clone(),
                default_key_bytes: config.policy.default_key_bytes,
                default_crypto_period_secs: config.policy.default_crypto_period_secs,
                rotation_lead_secs: config.policy.rotation_lead_secs,
                require_dual_approval: config.policy.require_dual_approval,
                allow_overlapping_crypto_periods: config.policy.allow_overlapping_crypto_periods,
                auto_retire_predecessor: config.policy.auto_retire_predecessor,
                updated_at: now,
            },
            keys: BTreeMap::new(),
            nodes: BTreeMap::new(),
            jobs: BTreeMap::new(),
            actions: BTreeMap::new(),
            audit: VecDeque::new(),
            backups: VecDeque::new(),
        }
    }
}

impl VaultDatabase {
    fn new() -> Self {
        Self {
            schema_version: VAULT_SCHEMA_VERSION,
            revision: 0,
            entries: BTreeMap::new(),
        }
    }
}

impl SharedKmf {
    pub fn load(config: KmfConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let master_key = load_or_create_secret(
            &config.storage.master_key_path,
            config.vault.master_key_bytes,
        )
        .map_err(std::io::Error::other)?;
        let database = load_database(&config)?;
        let vault = load_vault(&config)?;
        let kmf = Self {
            inner: Arc::new(Mutex::new(KmfState {
                config,
                database,
                vault,
                master_key,
                started_at: now_iso(),
                last_error: None,
            })),
        };
        kmf.recover_runtime_state()?;
        Ok(kmf)
    }

    pub fn status(&self) -> KmfStatus {
        let state = self.inner.lock().expect("KMF state mutex poisoned");
        status_locked(&state)
    }

    pub fn redacted_config(&self) -> Value {
        let state = self.inner.lock().expect("KMF state mutex poisoned");
        json!({
            "server": {
                "bind": state.config.server.bind.to_string(),
                "history_limit": state.config.server.history_limit,
            },
            "storage": {
                "database_path": state.config.storage.database_path,
                "vault_path": state.config.storage.vault_path,
                "master_key_path": state.config.storage.master_key_path,
                "backup_dir": state.config.storage.backup_dir,
                "bootstrap_dir": state.config.storage.bootstrap_dir,
            },
            "policy": state.database.policy,
            "vault": {
                "provider": state.config.vault.provider,
                "master_key_bytes": state.config.vault.master_key_bytes,
                "fsync": state.config.vault.fsync,
                "hsm_library": state.config.vault.hsm_library,
                "hsm_slot": state.config.vault.hsm_slot,
                "raw_key_material": "redacted",
            },
            "otar": state.config.otar,
            "security": {
                "mode": state.config.security.mode,
                "allow_remote_management": state.config.security.allow_remote_management,
                "expose_raw_keys": false,
            },
            "limits": state.config.limits,
        })
    }

    pub fn policy(&self) -> PolicyRecord {
        self.inner
            .lock()
            .expect("KMF state mutex poisoned")
            .database
            .policy
            .clone()
    }

    pub fn update_policy(&self, input: PolicyInput, actor: &str) -> Result<PolicyRecord, String> {
        if !matches!(
            input.operating_mode.as_str(),
            OPERATING_MODE_SHADOW | OPERATING_MODE_AUTHORITATIVE
        ) {
            return Err("operating_mode must be shadow or authoritative".to_string());
        }
        if input.default_key_bytes < 8 || input.default_key_bytes > 32 {
            return Err("default_key_bytes must be between 8 and 32".to_string());
        }
        if input.default_crypto_period_secs < 60 {
            return Err("default_crypto_period_secs must be at least 60".to_string());
        }
        let mut state = self.inner.lock().map_err(|error| error.to_string())?;
        let old_mode = state.database.policy.operating_mode.clone();
        state.database.policy.revision = state.database.policy.revision.saturating_add(1);
        state.database.policy.operating_mode = input.operating_mode;
        state.database.policy.default_key_bytes = input.default_key_bytes;
        state.database.policy.default_crypto_period_secs = input.default_crypto_period_secs;
        state.database.policy.rotation_lead_secs = input.rotation_lead_secs;
        state.database.policy.require_dual_approval = input.require_dual_approval;
        state.database.policy.allow_overlapping_crypto_periods =
            input.allow_overlapping_crypto_periods;
        state.database.policy.auto_retire_predecessor = input.auto_retire_predecessor;
        state.database.policy.updated_at = now_iso();
        if old_mode != OPERATING_MODE_AUTHORITATIVE
            && state.database.policy.operating_mode == OPERATING_MODE_AUTHORITATIVE
        {
            for action in state.database.actions.values_mut() {
                if action.state == ActionState::Staged {
                    action.state = ActionState::Pending;
                    action.updated_at = now_iso();
                }
            }
            for job in state.database.jobs.values_mut() {
                if job.state == OtarJobState::Staged {
                    job.state = OtarJobState::Queued;
                    for delivery in &mut job.deliveries {
                        if delivery.state == DeliveryState::Staged {
                            delivery.state = DeliveryState::Pending;
                            delivery.updated_at = now_iso();
                        }
                    }
                }
            }
        }
        let policy = state.database.policy.clone();
        append_audit_locked(
            &mut state,
            actor,
            "policy.update",
            "global",
            "success",
            json!({"old_mode":old_mode,"new_mode":policy.operating_mode}),
        );
        persist_locked(&mut state)?;
        Ok(policy)
    }

    pub fn keys(&self) -> Vec<KeyRecord> {
        let state = self.inner.lock().expect("KMF state mutex poisoned");
        let mut values = state.database.keys.values().cloned().collect::<Vec<_>>();
        values.sort_by(|left, right| {
            right
                .created_at
                .cmp(&left.created_at)
                .then_with(|| right.version.cmp(&left.version))
        });
        values
    }

    pub fn key(&self, id: &str) -> Option<KeyRecord> {
        self.inner
            .lock()
            .ok()?
            .database
            .keys
            .get(id)
            .cloned()
    }

    pub fn create_key(&self, input: KeyCreateInput, actor: &str) -> Result<KeyRecord, String> {
        let kind = input.kind.trim().to_ascii_uppercase();
        let scope = input.scope.trim().to_ascii_lowercase();
        let scope_value = input
            .scope_value
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        validate_key_kind_scope(&kind, &scope, scope_value.as_deref())?;
        let mut state = self.inner.lock().map_err(|error| error.to_string())?;
        if state.database.keys.len() >= state.config.limits.max_keys {
            return Err("key limit reached".to_string());
        }
        let key_bytes = input
            .key_bytes
            .unwrap_or(state.database.policy.default_key_bytes);
        if !(8..=32).contains(&key_bytes) {
            return Err("key_bytes must be between 8 and 32".to_string());
        }
        let now = Utc::now();
        let start = parse_time_or(input.crypto_period_start.as_deref(), now.clone())?;
        let end = parse_time_or(
            input.crypto_period_end.as_deref(),
            start.clone() + Duration::seconds(state.database.policy.default_crypto_period_secs as i64),
        )?;
        if end <= start {
            return Err("crypto_period_end must be later than crypto_period_start".to_string());
        }
        let version = next_key_version(
            &state.database,
            &kind,
            &scope,
            scope_value.as_deref(),
        );
        let id = Uuid::new_v4().to_string();
        let material = random_bytes(key_bytes)?;
        let material_reference = format!("key:{id}");
        let blob = seal(
            &state.master_key,
            &material,
            material_reference.as_bytes(),
        )?;
        state
            .vault
            .entries
            .insert(material_reference.clone(), blob);
        state.vault.revision = state.vault.revision.saturating_add(1);
        let record = KeyRecord {
            id: id.clone(),
            kind,
            scope,
            scope_value,
            label: clean_text(input.label, 128),
            algorithm_profile: input
                .algorithm_profile
                .unwrap_or_else(|| "tetra-key-material-lab-v1".to_string()),
            key_bytes,
            version,
            state: KeyState::Draft,
            fingerprint: fingerprint(&material),
            material_reference: Some(material_reference),
            crypto_period_start: start.to_rfc3339(),
            crypto_period_end: end.to_rfc3339(),
            predecessor_id: None,
            successor_id: None,
            created_at: now.to_rfc3339(),
            created_by: actor.to_string(),
            activated_at: None,
            retired_at: None,
            revoked_at: None,
            destroyed_at: None,
            lifecycle_reason: None,
            notes: clean_text(input.notes.unwrap_or_default(), 2_000),
            revision: 1,
        };
        state.database.keys.insert(id.clone(), record.clone());
        append_audit_locked(
            &mut state,
            actor,
            "key.generate",
            &id,
            "success",
            json!({
                "kind":record.kind,
                "scope":record.scope,
                "scope_value":record.scope_value,
                "version":record.version,
                "fingerprint":record.fingerprint,
                "key_bytes":record.key_bytes,
            }),
        );
        persist_locked(&mut state)?;
        Ok(record)
    }

    pub fn rotate_key(&self, id: &str, input: KeyRotateInput) -> Result<KeyRecord, String> {
        let actor = input.actor.unwrap_or_else(|| "open-lab-api".to_string());
        let mut state = self.inner.lock().map_err(|error| error.to_string())?;
        let predecessor = state
            .database
            .keys
            .get(id)
            .cloned()
            .ok_or_else(|| "key not found".to_string())?;
        if matches!(predecessor.state, KeyState::Revoked | KeyState::Destroyed) {
            return Err("revoked or destroyed keys cannot be rotated".to_string());
        }
        let now = Utc::now();
        let predecessor_end = parse_time(&predecessor.crypto_period_end)?;
        let default_start = if predecessor_end > now {
            predecessor_end - Duration::seconds(state.database.policy.rotation_lead_secs as i64)
        } else {
            now.clone()
        };
        let start = parse_time_or(input.activate_at.as_deref(), default_start.clone())?;
        let end = parse_time_or(
            input.crypto_period_end.as_deref(),
            start.clone() + Duration::seconds(state.database.policy.default_crypto_period_secs as i64),
        )?;
        if end <= start {
            return Err("successor crypto period must have a positive duration".to_string());
        }
        let new_id = Uuid::new_v4().to_string();
        let material = random_bytes(predecessor.key_bytes)?;
        let material_reference = format!("key:{new_id}");
        let blob = seal(
            &state.master_key,
            &material,
            material_reference.as_bytes(),
        )?;
        state
            .vault
            .entries
            .insert(material_reference.clone(), blob);
        state.vault.revision = state.vault.revision.saturating_add(1);
        let successor = KeyRecord {
            id: new_id.clone(),
            kind: predecessor.kind.clone(),
            scope: predecessor.scope.clone(),
            scope_value: predecessor.scope_value.clone(),
            label: predecessor.label.clone(),
            algorithm_profile: predecessor.algorithm_profile.clone(),
            key_bytes: predecessor.key_bytes,
            version: predecessor.version.saturating_add(1),
            state: KeyState::Staged,
            fingerprint: fingerprint(&material),
            material_reference: Some(material_reference),
            crypto_period_start: start.to_rfc3339(),
            crypto_period_end: end.to_rfc3339(),
            predecessor_id: Some(id.to_string()),
            successor_id: None,
            created_at: now.to_rfc3339(),
            created_by: actor.clone(),
            activated_at: None,
            retired_at: None,
            revoked_at: None,
            destroyed_at: None,
            lifecycle_reason: Some("rotation successor".to_string()),
            notes: clean_text(input.notes.unwrap_or_default(), 2_000),
            revision: 1,
        };
        if let Some(entry) = state.database.keys.get_mut(id) {
            entry.successor_id = Some(new_id.clone());
            if entry.state == KeyState::Active {
                entry.state = KeyState::Retiring;
            }
            entry.revision = entry.revision.saturating_add(1);
        }
        state.database.keys.insert(new_id.clone(), successor.clone());
        append_audit_locked(
            &mut state,
            &actor,
            "key.rotate",
            id,
            "success",
            json!({"successor_id":new_id,"successor_version":successor.version}),
        );
        persist_locked(&mut state)?;
        Ok(successor)
    }

    pub fn stage_key(&self, id: &str, input: LifecycleInput) -> Result<KeyRecord, String> {
        self.change_key_state(id, KeyState::Staged, input, "key.stage")
    }

    pub fn activate_key(&self, id: &str, input: LifecycleInput) -> Result<KeyRecord, String> {
        let actor = input.actor.unwrap_or_else(|| "open-lab-api".to_string());
        let reason = clean_text(input.reason.unwrap_or_default(), 500);
        let mut state = self.inner.lock().map_err(|error| error.to_string())?;
        let candidate = state
            .database
            .keys
            .get(id)
            .cloned()
            .ok_or_else(|| "key not found".to_string())?;
        if matches!(candidate.state, KeyState::Revoked | KeyState::Destroyed) {
            return Err("revoked or destroyed key cannot be activated".to_string());
        }
        if candidate.material_reference.is_none() {
            return Err("key material is unavailable".to_string());
        }
        if !state.database.policy.allow_overlapping_crypto_periods {
            for other in state.database.keys.values() {
                if other.id != candidate.id
                    && other.state == KeyState::Active
                    && same_scope(other, &candidate)
                    && periods_overlap(other, &candidate)?
                {
                    return Err(format!(
                        "crypto period overlaps active key {} and overlap policy is disabled",
                        other.id
                    ));
                }
            }
        }
        let now = now_iso();
        if state.database.policy.auto_retire_predecessor {
            for other in state.database.keys.values_mut() {
                if other.id != candidate.id
                    && other.state == KeyState::Active
                    && same_scope(other, &candidate)
                {
                    other.state = KeyState::Retired;
                    other.retired_at = Some(now.clone());
                    other.lifecycle_reason = Some(format!("superseded by {}", candidate.id));
                    other.revision = other.revision.saturating_add(1);
                }
            }
        }
        let record = state
            .database
            .keys
            .get_mut(id)
            .ok_or_else(|| "key not found".to_string())?;
        record.state = KeyState::Active;
        record.activated_at = Some(now);
        record.lifecycle_reason = non_empty(reason);
        record.revision = record.revision.saturating_add(1);
        let out = record.clone();
        append_audit_locked(
            &mut state,
            &actor,
            "key.activate",
            id,
            "success",
            json!({"version":out.version,"fingerprint":out.fingerprint}),
        );
        persist_locked(&mut state)?;
        Ok(out)
    }

    pub fn retire_key(&self, id: &str, input: LifecycleInput) -> Result<KeyRecord, String> {
        self.change_key_state(id, KeyState::Retired, input, "key.retire")
    }

    pub fn revoke_key(&self, id: &str, input: LifecycleInput) -> Result<KeyRecord, String> {
        self.change_key_state(id, KeyState::Revoked, input, "key.revoke")
    }

    pub fn destroy_key(&self, id: &str, input: LifecycleInput) -> Result<KeyRecord, String> {
        let actor = input.actor.unwrap_or_else(|| "open-lab-api".to_string());
        let reason = clean_text(input.reason.unwrap_or_default(), 500);
        let mut state = self.inner.lock().map_err(|error| error.to_string())?;
        let material_reference = state
            .database
            .keys
            .get(id)
            .ok_or_else(|| "key not found".to_string())?
            .material_reference
            .clone();
        if state
            .database
            .jobs
            .values()
            .any(|job| job.key_id == id && !is_terminal_job(job.state))
        {
            return Err("key has non-terminal OTAR jobs and cannot be destroyed".to_string());
        }
        if let Some(reference) = material_reference {
            state.vault.entries.remove(&reference);
            state.vault.revision = state.vault.revision.saturating_add(1);
        }
        let now = now_iso();
        let record = state
            .database
            .keys
            .get_mut(id)
            .ok_or_else(|| "key not found".to_string())?;
        record.state = KeyState::Destroyed;
        record.material_reference = None;
        record.destroyed_at = Some(now);
        record.lifecycle_reason = non_empty(reason);
        record.revision = record.revision.saturating_add(1);
        let out = record.clone();
        append_audit_locked(
            &mut state,
            &actor,
            "key.destroy",
            id,
            "success",
            json!({"fingerprint":out.fingerprint,"material_removed":true}),
        );
        persist_locked(&mut state)?;
        Ok(out)
    }

    fn change_key_state(
        &self,
        id: &str,
        target: KeyState,
        input: LifecycleInput,
        action: &str,
    ) -> Result<KeyRecord, String> {
        let actor = input.actor.unwrap_or_else(|| "open-lab-api".to_string());
        let reason = clean_text(input.reason.unwrap_or_default(), 500);
        let mut state = self.inner.lock().map_err(|error| error.to_string())?;
        let now = now_iso();
        let out = {
            let record = state
                .database
                .keys
                .get_mut(id)
                .ok_or_else(|| "key not found".to_string())?;
            if record.state == KeyState::Destroyed {
                return Err("destroyed key lifecycle is final".to_string());
            }
            record.state = target;
            record.lifecycle_reason = non_empty(reason.clone());
            record.revision = record.revision.saturating_add(1);
            match target {
                KeyState::Retired => record.retired_at = Some(now.clone()),
                KeyState::Revoked => record.revoked_at = Some(now.clone()),
                _ => {}
            }
            record.clone()
        };
        if target == KeyState::Revoked {
            cancel_actions_for_key_locked(
                &mut state,
                id,
                if reason.is_empty() {
                    "key revoked"
                } else {
                    &reason
                },
            );
        }
        append_audit_locked(
            &mut state,
            &actor,
            action,
            id,
            "success",
            json!({"state":out.state,"pending_otar_cancelled":target == KeyState::Revoked}),
        );
        persist_locked(&mut state)?;
        Ok(out)
    }

    pub fn nodes(&self) -> Vec<NodeTransportRecord> {
        let state = self.inner.lock().expect("KMF state mutex poisoned");
        state.database.nodes.values().cloned().collect()
    }

    pub fn create_node(
        &self,
        input: NodeCreateInput,
    ) -> Result<NodeTransportRecord, String> {
        validate_node_id(&input.node_id)?;
        let actor = input.actor.unwrap_or_else(|| "open-lab-api".to_string());
        let mut state = self.inner.lock().map_err(|error| error.to_string())?;
        if state.database.nodes.len() >= state.config.limits.max_nodes {
            return Err("node transport profile limit reached".to_string());
        }
        if state.database.nodes.contains_key(&input.node_id) {
            return Err("node transport profile already exists".to_string());
        }
        let node_secret = random_bytes(32)?;
        let transport_key = derive_node_transport_key(&node_secret, &input.node_id);
        let secret_reference = format!("node:{}", input.node_id);
        let sealed = seal(
            &state.master_key,
            &node_secret,
            secret_reference.as_bytes(),
        )?;
        state.vault.entries.insert(secret_reference.clone(), sealed);
        state.vault.revision = state.vault.revision.saturating_add(1);

        fs::create_dir_all(&state.config.storage.bootstrap_dir)
            .map_err(|error| error.to_string())?;
        let bootstrap_path = state
            .config
            .storage
            .bootstrap_dir
            .join(format!("{}-kmf-bootstrap.json", input.node_id));
        let bootstrap = serde_json::to_vec_pretty(&json!({
            "schema":"netcore-kmf-node-bootstrap-v1",
            "node_id":input.node_id,
            "transport_secret_hex":hex_encode(&node_secret),
            "transport_key_fingerprint":fingerprint(&transport_key),
            "otar_edge_protocol":OTAR_EDGE_PROTOCOL_VERSION,
            "warning":"Contains secret bootstrap material. Copy offline to the matching TBS edge and delete this file after import.",
        }))
        .map_err(|error| error.to_string())?;
        write_private_file(&bootstrap_path, &bootstrap)?;
        let now = now_iso();
        let record = NodeTransportRecord {
            node_id: input.node_id.clone(),
            display_name: clean_text(input.display_name, 128),
            enabled: true,
            transport_key_fingerprint: fingerprint(&transport_key),
            secret_reference: Some(secret_reference),
            bootstrap_path: bootstrap_path.display().to_string(),
            created_at: now.clone(),
            created_by: actor.clone(),
            updated_at: now,
            disabled_reason: None,
            last_claim_at: None,
            last_ack_at: None,
            notes: clean_text(input.notes.unwrap_or_default(), 2_000),
            revision: 1,
        };
        state
            .database
            .nodes
            .insert(input.node_id.clone(), record.clone());
        append_audit_locked(
            &mut state,
            &actor,
            "node_transport.create",
            &input.node_id,
            "success",
            json!({
                "transport_key_fingerprint":record.transport_key_fingerprint,
                "bootstrap_path":record.bootstrap_path,
                "secret_returned_by_api":false,
            }),
        );
        persist_locked(&mut state)?;
        Ok(record)
    }

    pub fn set_node_enabled(
        &self,
        node_id: &str,
        enabled: bool,
        input: NodeStateInput,
    ) -> Result<NodeTransportRecord, String> {
        let actor = input.actor.unwrap_or_else(|| "open-lab-api".to_string());
        let reason = clean_text(input.reason.unwrap_or_default(), 500);
        let mut state = self.inner.lock().map_err(|error| error.to_string())?;
        let out = {
            let record = state
                .database
                .nodes
                .get_mut(node_id)
                .ok_or_else(|| "node transport profile not found".to_string())?;
            record.enabled = enabled;
            record.updated_at = now_iso();
            record.disabled_reason = if enabled {
                None
            } else {
                non_empty(reason.clone())
            };
            record.revision = record.revision.saturating_add(1);
            record.clone()
        };
        if !enabled {
            cancel_actions_for_node_locked(
                &mut state,
                node_id,
                if reason.is_empty() {
                    "node transport profile disabled"
                } else {
                    &reason
                },
            );
        }
        append_audit_locked(
            &mut state,
            &actor,
            if enabled {
                "node_transport.enable"
            } else {
                "node_transport.disable"
            },
            node_id,
            "success",
            json!({"enabled":enabled}),
        );
        persist_locked(&mut state)?;
        Ok(out)
    }

    pub fn jobs(&self) -> Vec<OtarJobRecord> {
        let state = self.inner.lock().expect("KMF state mutex poisoned");
        let mut jobs = state.database.jobs.values().cloned().collect::<Vec<_>>();
        jobs.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        jobs
    }

    pub fn job(&self, id: &str) -> Option<OtarJobRecord> {
        self.inner.lock().ok()?.database.jobs.get(id).cloned()
    }

    pub fn create_job(&self, input: OtarJobCreateInput) -> Result<OtarJobRecord, String> {
        let actor = input.actor.unwrap_or_else(|| "open-lab-api".to_string());
        let mut state = self.inner.lock().map_err(|error| error.to_string())?;
        if state.database.jobs.len() >= state.config.limits.max_jobs {
            return Err("OTAR job limit reached".to_string());
        }
        let key = state
            .database
            .keys
            .get(&input.key_id)
            .cloned()
            .ok_or_else(|| "key not found".to_string())?;
        if matches!(key.state, KeyState::Draft | KeyState::Revoked | KeyState::Destroyed) {
            return Err("key must be staged, active, retiring or retired before distribution".to_string());
        }
        if key.material_reference.is_none() {
            return Err("key material unavailable".to_string());
        }
        if input.target_nodes.is_empty() {
            return Err("at least one target node is required".to_string());
        }
        let mut target_nodes = input.target_nodes;
        target_nodes.sort();
        target_nodes.dedup();
        for node_id in &target_nodes {
            let node = state
                .database
                .nodes
                .get(node_id)
                .ok_or_else(|| format!("node transport profile {node_id} not found"))?;
            if !node.enabled {
                return Err(format!("node transport profile {node_id} is disabled"));
            }
        }
        validate_24bit_values(&input.target_issis, "ISSI")?;
        validate_24bit_values(&input.target_gssis, "GSSI")?;
        let now = Utc::now();
        let not_before = parse_time_or(input.not_before.as_deref(), now.clone())?;
        let expires_at = parse_time_or(
            input.expires_at.as_deref(),
            parse_time(&key.crypto_period_end)?,
        )?;
        if expires_at <= not_before {
            return Err("expires_at must be later than not_before".to_string());
        }
        let required_approvals = if state.database.policy.require_dual_approval {
            2
        } else {
            1
        };
        let id = Uuid::new_v4().to_string();
        let record = OtarJobRecord {
            id: id.clone(),
            key_id: key.id.clone(),
            key_fingerprint: key.fingerprint.clone(),
            key_kind: key.kind.clone(),
            key_version: key.version,
            state: OtarJobState::AwaitingApproval,
            target_nodes,
            target_issis: input.target_issis,
            target_gssis: input.target_gssis,
            approvals: Vec::new(),
            required_approvals,
            deliveries: Vec::new(),
            created_at: now.to_rfc3339(),
            created_by: actor.clone(),
            not_before: not_before.to_rfc3339(),
            expires_at: expires_at.to_rfc3339(),
            queued_at: None,
            completed_at: None,
            notes: clean_text(input.notes.unwrap_or_default(), 2_000),
            revision: 1,
        };
        state.database.jobs.insert(id.clone(), record.clone());
        append_audit_locked(
            &mut state,
            &actor,
            "otar_job.create",
            &id,
            "success",
            json!({
                "key_id":record.key_id,
                "key_fingerprint":record.key_fingerprint,
                "nodes":record.target_nodes,
                "required_approvals":record.required_approvals,
            }),
        );
        persist_locked(&mut state)?;
        Ok(record)
    }

    pub fn approve_job(
        &self,
        id: &str,
        input: OtarApprovalInput,
    ) -> Result<OtarJobRecord, String> {
        let actor = clean_text(input.actor, 128);
        if actor.trim().is_empty() {
            return Err("approval actor is required".to_string());
        }
        let mut state = self.inner.lock().map_err(|error| error.to_string())?;
        let job = state
            .database
            .jobs
            .get_mut(id)
            .ok_or_else(|| "OTAR job not found".to_string())?;
        if !matches!(
            job.state,
            OtarJobState::AwaitingApproval | OtarJobState::Approved
        ) {
            return Err("job is not awaiting approval".to_string());
        }
        if job.approvals.iter().any(|approval| approval.actor == actor) {
            return Err("the same actor cannot approve the job twice".to_string());
        }
        job.approvals.push(OtarApprovalRecord {
            actor: actor.clone(),
            timestamp: now_iso(),
            note: clean_text(input.note.unwrap_or_default(), 500),
        });
        if job.approvals.len() >= job.required_approvals {
            job.state = OtarJobState::Approved;
        }
        job.revision = job.revision.saturating_add(1);
        let out = job.clone();
        append_audit_locked(
            &mut state,
            &actor,
            "otar_job.approve",
            id,
            "success",
            json!({
                "approval_count":out.approvals.len(),
                "required_approvals":out.required_approvals,
                "advisory_identity_only":true,
            }),
        );
        persist_locked(&mut state)?;
        Ok(out)
    }

    pub fn queue_job(&self, id: &str, input: OtarQueueInput) -> Result<OtarJobRecord, String> {
        let actor = input.actor.unwrap_or_else(|| "open-lab-api".to_string());
        let mut state = self.inner.lock().map_err(|error| error.to_string())?;
        let job_snapshot = state
            .database
            .jobs
            .get(id)
            .cloned()
            .ok_or_else(|| "OTAR job not found".to_string())?;
        if job_snapshot.state != OtarJobState::Approved {
            return Err("OTAR job must be fully approved before queueing".to_string());
        }
        if state
            .database
            .actions
            .len()
            .saturating_add(job_snapshot.target_nodes.len())
            > state.config.limits.max_actions
        {
            return Err("OTAR action limit would be exceeded".to_string());
        }
        let now_dt = Utc::now();
        if parse_time(&job_snapshot.expires_at)? <= now_dt {
            return Err("OTAR job has already expired".to_string());
        }
        for node_id in &job_snapshot.target_nodes {
            let node = state
                .database
                .nodes
                .get(node_id)
                .ok_or_else(|| format!("node transport profile {node_id} not found"))?;
            if !node.enabled {
                return Err(format!("node transport profile {node_id} is disabled"));
            }
        }
        let authoritative =
            state.database.policy.operating_mode == OPERATING_MODE_AUTHORITATIVE;
        let now = now_iso();
        let mut deliveries = Vec::new();
        for node_id in &job_snapshot.target_nodes {
            let delivery_id = Uuid::new_v4().to_string();
            let action_id = Uuid::new_v4().to_string();
            let action_state = if authoritative {
                ActionState::Pending
            } else {
                ActionState::Staged
            };
            let delivery_state = if authoritative {
                DeliveryState::Pending
            } else {
                DeliveryState::Staged
            };
            let sequence = state.database.next_action_sequence;
            state.database.next_action_sequence =
                state.database.next_action_sequence.saturating_add(1);
            let action = OtarActionRecord {
                id: action_id.clone(),
                sequence,
                job_id: id.to_string(),
                delivery_id: delivery_id.clone(),
                node_id: node_id.clone(),
                key_id: job_snapshot.key_id.clone(),
                state: action_state,
                created_at: now.clone(),
                updated_at: now.clone(),
                expires_at: job_snapshot.expires_at.clone(),
                attempts: 0,
                next_attempt_at: job_snapshot.not_before.clone(),
                last_error: None,
            };
            state.database.actions.insert(action_id.clone(), action);
            deliveries.push(OtarDeliveryRecord {
                id: delivery_id,
                node_id: node_id.clone(),
                state: delivery_state,
                action_id: Some(action_id),
                attempts: 0,
                created_at: now.clone(),
                updated_at: now.clone(),
                applied_at: None,
                last_error: None,
            });
        }
        let job = state
            .database
            .jobs
            .get_mut(id)
            .ok_or_else(|| "OTAR job not found".to_string())?;
        job.deliveries = deliveries;
        job.state = if authoritative {
            OtarJobState::Queued
        } else {
            OtarJobState::Staged
        };
        job.queued_at = Some(now);
        job.revision = job.revision.saturating_add(1);
        let out = job.clone();
        let operating_mode = state.database.policy.operating_mode.clone();
        append_audit_locked(
            &mut state,
            &actor,
            "otar_job.queue",
            id,
            "success",
            json!({
                "mode":operating_mode,
                "deliveries":out.deliveries.len(),
                "released_to_edge":authoritative,
            }),
        );
        persist_locked(&mut state)?;
        Ok(out)
    }

    pub fn cancel_job(&self, id: &str, input: LifecycleInput) -> Result<OtarJobRecord, String> {
        let actor = input.actor.unwrap_or_else(|| "open-lab-api".to_string());
        let reason = clean_text(input.reason.unwrap_or_default(), 500);
        let mut state = self.inner.lock().map_err(|error| error.to_string())?;
        let action_ids = state
            .database
            .actions
            .values()
            .filter(|action| action.job_id == id)
            .map(|action| action.id.clone())
            .collect::<Vec<_>>();
        for action_id in action_ids {
            if let Some(action) = state.database.actions.get_mut(&action_id) {
                if !matches!(action.state, ActionState::Applied | ActionState::Expired) {
                    action.state = ActionState::Cancelled;
                    action.updated_at = now_iso();
                    action.last_error = non_empty(reason.clone());
                }
            }
        }
        let job = state
            .database
            .jobs
            .get_mut(id)
            .ok_or_else(|| "OTAR job not found".to_string())?;
        job.state = OtarJobState::Cancelled;
        for delivery in &mut job.deliveries {
            if delivery.state != DeliveryState::Applied {
                delivery.state = DeliveryState::Cancelled;
                delivery.updated_at = now_iso();
                delivery.last_error = non_empty(reason.clone());
            }
        }
        job.revision = job.revision.saturating_add(1);
        let out = job.clone();
        append_audit_locked(
            &mut state,
            &actor,
            "otar_job.cancel",
            id,
            "success",
            json!({"reason":reason}),
        );
        persist_locked(&mut state)?;
        Ok(out)
    }

    pub fn actions(&self) -> Vec<OtarActionRecord> {
        let state = self.inner.lock().expect("KMF state mutex poisoned");
        let mut actions = state
            .database
            .actions
            .values()
            .cloned()
            .collect::<Vec<_>>();
        actions.sort_by_key(|action| action.sequence);
        actions
    }

    pub fn claim_actions(&self, input: EdgeClaimInput) -> Result<Vec<ClaimedOtarAction>, String> {
        validate_node_id(&input.node_id)?;
        let mut state = self.inner.lock().map_err(|error| error.to_string())?;
        if state.database.policy.operating_mode != OPERATING_MODE_AUTHORITATIVE {
            return Ok(Vec::new());
        }
        let node = state
            .database
            .nodes
            .get(&input.node_id)
            .cloned()
            .ok_or_else(|| "node transport profile not found".to_string())?;
        if !node.enabled {
            return Err("node transport profile is disabled".to_string());
        }
        let node_reference = node
            .secret_reference
            .as_deref()
            .ok_or_else(|| "node transport secret unavailable".to_string())?;
        let node_secret = open_vault_entry(&state, node_reference)?;
        let transport_key =
            derive_node_transport_key(&node_secret, &input.node_id);
        let max_actions = input
            .max_actions
            .unwrap_or(state.config.otar.max_claim_batch)
            .min(state.config.otar.max_claim_batch);
        let now = Utc::now();
        let candidate_ids = state
            .database
            .actions
            .values()
            .filter(|action| {
                action.node_id == input.node_id
                    && action.state == ActionState::Pending
                    && parse_time(&action.next_attempt_at)
                        .map(|time| time <= now)
                        .unwrap_or(false)
                    && parse_time(&action.expires_at)
                        .map(|time| time > now)
                        .unwrap_or(false)
            })
            .take(max_actions)
            .map(|action| action.id.clone())
            .collect::<Vec<_>>();
        let mut claimed = Vec::new();
        for action_id in candidate_ids {
            let action = state
                .database
                .actions
                .get(&action_id)
                .cloned()
                .ok_or_else(|| "action disappeared".to_string())?;
            let job = state
                .database
                .jobs
                .get(&action.job_id)
                .cloned()
                .ok_or_else(|| "OTAR job missing".to_string())?;
            let key = state
                .database
                .keys
                .get(&action.key_id)
                .cloned()
                .ok_or_else(|| "key metadata missing".to_string())?;
            let key_reference = key
                .material_reference
                .as_deref()
                .ok_or_else(|| "key material destroyed".to_string())?;
            let key_material = open_vault_entry(&state, key_reference)?;
            let envelope_context = format!(
                "{}:{}:{}:{}",
                OTAR_EDGE_PROTOCOL_VERSION, action.id, action.node_id, action.key_id
            );
            let envelope = seal(
                &transport_key,
                &key_material,
                envelope_context.as_bytes(),
            )?;
            claimed.push(ClaimedOtarAction {
                protocol_version: OTAR_EDGE_PROTOCOL_VERSION,
                id: action.id.clone(),
                sequence: action.sequence,
                job_id: action.job_id.clone(),
                delivery_id: action.delivery_id.clone(),
                node_id: action.node_id.clone(),
                key_id: key.id,
                key_kind: key.kind,
                key_version: key.version,
                key_fingerprint: key.fingerprint,
                algorithm_profile: key.algorithm_profile,
                crypto_period_start: key.crypto_period_start,
                crypto_period_end: key.crypto_period_end,
                target_issis: job.target_issis,
                target_gssis: job.target_gssis,
                envelope,
                envelope_context,
                expires_at: action.expires_at.clone(),
            });
            if let Some(entry) = state.database.actions.get_mut(&action_id) {
                entry.state = ActionState::InFlight;
                entry.attempts = entry.attempts.saturating_add(1);
                entry.updated_at = now_iso();
            }
            update_delivery_locked(
                &mut state,
                &action.job_id,
                &action.delivery_id,
                DeliveryState::InFlight,
                None,
                None,
            );
            recompute_job_state_locked(&mut state, &action.job_id);
        }
        if let Some(node) = state.database.nodes.get_mut(&input.node_id) {
            node.last_claim_at = Some(now_iso());
            node.updated_at = now_iso();
            node.revision = node.revision.saturating_add(1);
        }
        if !claimed.is_empty() {
            append_audit_locked(
                &mut state,
                &format!("edge:{}", input.node_id),
                "otar_action.claim",
                &input.node_id,
                "success",
                json!({
                    "actions":claimed.iter().map(|action| action.id.clone()).collect::<Vec<_>>(),
                    "raw_key_returned":false,
                    "sealed_envelopes":true,
                }),
            );
            persist_locked(&mut state)?;
        }
        Ok(claimed)
    }

    pub fn acknowledge_action(
        &self,
        id: &str,
        input: EdgeActionAckInput,
    ) -> Result<OtarActionRecord, String> {
        let mut state = self.inner.lock().map_err(|error| error.to_string())?;
        let snapshot = state
            .database
            .actions
            .get(id)
            .cloned()
            .ok_or_else(|| "OTAR action not found".to_string())?;
        if snapshot.state != ActionState::InFlight {
            return Err("only in-flight actions may be acknowledged".to_string());
        }
        let success = input.success;
        let error = clean_text(input.error.unwrap_or_default(), 1_000);
        let applied_at = input.applied_at.unwrap_or_else(now_iso);
        let max_attempts = state.config.otar.max_attempts;
        let retry_backoff = state.config.otar.retry_backoff_secs;
        let action = state
            .database
            .actions
            .get_mut(id)
            .ok_or_else(|| "OTAR action not found".to_string())?;
        if success {
            action.state = ActionState::Applied;
            action.last_error = None;
        } else if action.attempts >= max_attempts {
            action.state = ActionState::Failed;
            action.last_error = non_empty(error.clone());
        } else {
            action.state = ActionState::Pending;
            action.last_error = non_empty(error.clone());
            action.next_attempt_at =
                (Utc::now() + Duration::seconds(retry_backoff as i64)).to_rfc3339();
        }
        action.updated_at = now_iso();
        let out = action.clone();
        let delivery_state = match out.state {
            ActionState::Applied => DeliveryState::Applied,
            ActionState::Failed => DeliveryState::Failed,
            ActionState::Pending => DeliveryState::Pending,
            _ => DeliveryState::InFlight,
        };
        update_delivery_locked(
            &mut state,
            &out.job_id,
            &out.delivery_id,
            delivery_state,
            if success { Some(applied_at) } else { None },
            non_empty(error),
        );
        recompute_job_state_locked(&mut state, &out.job_id);
        if let Some(node) = state.database.nodes.get_mut(&out.node_id) {
            node.last_ack_at = Some(now_iso());
            node.updated_at = now_iso();
            node.revision = node.revision.saturating_add(1);
        }
        append_audit_locked(
            &mut state,
            &format!("edge:{}", out.node_id),
            "otar_action.ack",
            id,
            if success { "success" } else { "failure" },
            json!({"state":out.state,"attempts":out.attempts,"error":out.last_error}),
        );
        persist_locked(&mut state)?;
        Ok(out)
    }

    pub fn audit(&self, limit: usize) -> Vec<AuditRecord> {
        let state = self.inner.lock().expect("KMF state mutex poisoned");
        state
            .database
            .audit
            .iter()
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn backups(&self) -> Vec<BackupRecord> {
        self.inner
            .lock()
            .expect("KMF state mutex poisoned")
            .database
            .backups
            .iter()
            .rev()
            .cloned()
            .collect()
    }

    pub fn create_backup(&self, input: BackupInput) -> Result<BackupRecord, String> {
        let actor = input.actor.unwrap_or_else(|| "open-lab-api".to_string());
        let note = clean_text(input.note.unwrap_or_default(), 1_000);
        let mut state = self.inner.lock().map_err(|error| error.to_string())?;
        persist_locked(&mut state)?;
        let id = format!(
            "{}-{}",
            Utc::now().format("%Y%m%dT%H%M%SZ"),
            Uuid::new_v4().simple()
        );
        let directory = state.config.storage.backup_dir.join(&id);
        fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        let metadata_bytes = fs::read(&state.config.storage.database_path)
            .map_err(|error| error.to_string())?;
        let vault_bytes =
            fs::read(&state.config.storage.vault_path).map_err(|error| error.to_string())?;
        let backup_state_path = directory.join("state.json");
        let backup_vault_path = directory.join("vault.json");
        write_private_file(&backup_state_path, &metadata_bytes)?;
        write_private_file(&backup_vault_path, &vault_bytes)?;
        let verified = fs::read(&backup_state_path).map_err(|error| error.to_string())?
            == metadata_bytes
            && fs::read(&backup_vault_path).map_err(|error| error.to_string())?
                == vault_bytes;
        if !verified {
            return Err("backup read-back verification failed".to_string());
        }
        let record = BackupRecord {
            id: id.clone(),
            created_at: now_iso(),
            created_by: actor.clone(),
            directory: directory.display().to_string(),
            metadata_sha256: sha256_hex(&metadata_bytes),
            vault_sha256: sha256_hex(&vault_bytes),
            audit_head_hash: audit_head_hash(&state.database),
            note,
            verified,
        };
        let manifest = serde_json::to_vec_pretty(&record).map_err(|error| error.to_string())?;
        write_private_file(&directory.join("manifest.json"), &manifest)?;
        state.database.backups.push_back(record.clone());
        while state.database.backups.len() > state.config.server.history_limit {
            state.database.backups.pop_front();
        }
        append_audit_locked(
            &mut state,
            &actor,
            "backup.create",
            &id,
            "success",
            json!({
                "directory":record.directory,
                "metadata_sha256":record.metadata_sha256,
                "vault_sha256":record.vault_sha256,
                "contains_plaintext_keys":false,
            }),
        );
        persist_locked(&mut state)?;
        Ok(record)
    }

    pub fn maintenance_tick(&self) -> Result<KmfStatus, String> {
        let mut state = self.inner.lock().map_err(|error| error.to_string())?;
        let now = Utc::now();
        let mut changed = false;
        let action_ids = state.database.actions.keys().cloned().collect::<Vec<_>>();
        for action_id in action_ids {
            let snapshot = match state.database.actions.get(&action_id).cloned() {
                Some(action) => action,
                None => continue,
            };
            let expires_at = parse_time(&snapshot.expires_at)?;
            if expires_at <= now
                && matches!(
                    snapshot.state,
                    ActionState::Staged | ActionState::Pending | ActionState::InFlight
                )
            {
                if let Some(action) = state.database.actions.get_mut(&action_id) {
                    action.state = ActionState::Expired;
                    action.updated_at = now_iso();
                    action.last_error = Some("OTAR action expired".to_string());
                }
                update_delivery_locked(
                    &mut state,
                    &snapshot.job_id,
                    &snapshot.delivery_id,
                    DeliveryState::Expired,
                    None,
                    Some("OTAR action expired".to_string()),
                );
                recompute_job_state_locked(&mut state, &snapshot.job_id);
                changed = true;
            } else if snapshot.state == ActionState::InFlight {
                let updated_at = parse_time(&snapshot.updated_at)?;
                if updated_at
                    + Duration::seconds(state.config.otar.action_ttl_secs as i64)
                    <= now
                {
                    let max_attempts = state.config.otar.max_attempts;
                    let retry_backoff = state.config.otar.retry_backoff_secs;
                    if let Some(action) = state.database.actions.get_mut(&action_id) {
                        if action.attempts >= max_attempts {
                            action.state = ActionState::Failed;
                            action.last_error = Some("edge acknowledgement timeout".to_string());
                        } else {
                            action.state = ActionState::Pending;
                            action.next_attempt_at =
                                (now + Duration::seconds(retry_backoff as i64)).to_rfc3339();
                            action.last_error = Some("edge acknowledgement timeout; retry queued".to_string());
                        }
                        action.updated_at = now_iso();
                    }
                    let delivery_state = state
                        .database
                        .actions
                        .get(&action_id)
                        .map(|action| {
                            if action.state == ActionState::Failed {
                                DeliveryState::Failed
                            } else {
                                DeliveryState::Pending
                            }
                        })
                        .unwrap_or(DeliveryState::Failed);
                    update_delivery_locked(
                        &mut state,
                        &snapshot.job_id,
                        &snapshot.delivery_id,
                        delivery_state,
                        None,
                        Some("edge acknowledgement timeout".to_string()),
                    );
                    recompute_job_state_locked(&mut state, &snapshot.job_id);
                    changed = true;
                }
            }
        }
        let job_ids = state.database.jobs.keys().cloned().collect::<Vec<_>>();
        for job_id in job_ids {
            let snapshot = match state.database.jobs.get(&job_id).cloned() {
                Some(job) => job,
                None => continue,
            };
            if !is_terminal_job(snapshot.state) && parse_time(&snapshot.expires_at)? <= now {
                for action in state.database.actions.values_mut() {
                    if action.job_id == job_id
                        && matches!(
                            action.state,
                            ActionState::Staged | ActionState::Pending | ActionState::InFlight
                        )
                    {
                        action.state = ActionState::Expired;
                        action.updated_at = now_iso();
                        action.last_error = Some("OTAR job expired".to_string());
                    }
                }
                if let Some(job) = state.database.jobs.get_mut(&job_id) {
                    job.state = OtarJobState::Expired;
                    job.completed_at = Some(now_iso());
                    job.revision = job.revision.saturating_add(1);
                    for delivery in &mut job.deliveries {
                        if !matches!(delivery.state, DeliveryState::Applied) {
                            delivery.state = DeliveryState::Expired;
                            delivery.updated_at = now_iso();
                            delivery.last_error = Some("OTAR job expired".to_string());
                        }
                    }
                }
                changed = true;
            }
        }
        let key_ids = state.database.keys.keys().cloned().collect::<Vec<_>>();
        for key_id in key_ids {
            let end = state
                .database
                .keys
                .get(&key_id)
                .map(|key| parse_time(&key.crypto_period_end))
                .transpose()?
                .unwrap_or_else(Utc::now);
            if end <= now {
                if let Some(key) = state.database.keys.get_mut(&key_id) {
                    if matches!(key.state, KeyState::Active | KeyState::Retiring) {
                        key.state = KeyState::Retired;
                        key.retired_at = Some(now_iso());
                        key.lifecycle_reason = Some("crypto period ended".to_string());
                        key.revision = key.revision.saturating_add(1);
                        changed = true;
                    }
                }
            }
        }
        if changed {
            append_audit_locked(
                &mut state,
                "maintenance",
                "maintenance.tick",
                "global",
                "success",
                json!({"changes_applied":true}),
            );
            persist_locked(&mut state)?;
        }
        Ok(status_locked(&state))
    }

    pub fn export(&self) -> RedactedExport {
        let state = self.inner.lock().expect("KMF state mutex poisoned");
        RedactedExport {
            generated_at: now_iso(),
            status: status_locked(&state),
            policy: state.database.policy.clone(),
            keys: state.database.keys.values().cloned().collect(),
            nodes: state.database.nodes.values().cloned().collect(),
            jobs: state.database.jobs.values().cloned().collect(),
            actions: state.database.actions.values().cloned().collect(),
            audit: state.database.audit.iter().cloned().collect(),
            backups: state.database.backups.iter().cloned().collect(),
            note: "Redacted export: no master key, node transport secret, raw CCK/GCK/SCK material or encrypted vault blobs are included.",
        }
    }

    pub fn metrics(&self) -> String {
        let status = self.status();
        format!(
            concat!(
                "# HELP netcore_kmf_keys_total Number of KMF key metadata records\n",
                "# TYPE netcore_kmf_keys_total gauge\n",
                "netcore_kmf_keys_total {}\n",
                "# HELP netcore_kmf_active_keys Number of active keys\n",
                "# TYPE netcore_kmf_active_keys gauge\n",
                "netcore_kmf_active_keys {}\n",
                "# HELP netcore_kmf_otar_jobs_total Number of OTAR jobs\n",
                "# TYPE netcore_kmf_otar_jobs_total gauge\n",
                "netcore_kmf_otar_jobs_total {}\n",
                "# HELP netcore_kmf_pending_actions Number of staged, pending or in-flight OTAR actions\n",
                "# TYPE netcore_kmf_pending_actions gauge\n",
                "netcore_kmf_pending_actions {}\n",
                "# HELP netcore_kmf_enabled_nodes Number of enabled node transport profiles\n",
                "# TYPE netcore_kmf_enabled_nodes gauge\n",
                "netcore_kmf_enabled_nodes {}\n",
                "# HELP netcore_kmf_authoritative Operating mode (1 authoritative, 0 shadow)\n",
                "# TYPE netcore_kmf_authoritative gauge\n",
                "netcore_kmf_authoritative {}\n",
            ),
            status.total_keys,
            status.active_keys,
            status.otar_jobs,
            status.pending_actions,
            status.enabled_nodes,
            if status.authoritative { 1 } else { 0 },
        )
    }

    fn recover_runtime_state(&self) -> Result<(), String> {
        let mut state = self.inner.lock().map_err(|error| error.to_string())?;
        let mut changed = false;
        let authoritative =
            state.database.policy.operating_mode == OPERATING_MODE_AUTHORITATIVE;
        for action in state.database.actions.values_mut() {
            if action.state == ActionState::InFlight {
                action.state = if authoritative {
                    ActionState::Pending
                } else {
                    ActionState::Staged
                };
                action.last_error = Some("recovered after KMF restart".to_string());
                action.updated_at = now_iso();
                changed = true;
            }
        }
        if changed {
            append_audit_locked(
                &mut state,
                "system",
                "runtime.recover",
                "global",
                "success",
                json!({"in_flight_actions_requeued":true}),
            );
            persist_locked(&mut state)?;
        }
        Ok(())
    }
}

fn status_locked(state: &KmfState) -> KmfStatus {
    let total_keys = state.database.keys.len();
    let active_keys = state
        .database
        .keys
        .values()
        .filter(|key| key.state == KeyState::Active)
        .count();
    let staged_keys = state
        .database
        .keys
        .values()
        .filter(|key| matches!(key.state, KeyState::Draft | KeyState::Staged))
        .count();
    let revoked_keys = state
        .database
        .keys
        .values()
        .filter(|key| matches!(key.state, KeyState::Revoked | KeyState::Destroyed))
        .count();
    let pending_actions = state
        .database
        .actions
        .values()
        .filter(|action| {
            matches!(
                action.state,
                ActionState::Staged | ActionState::Pending | ActionState::InFlight
            )
        })
        .count();
    KmfStatus {
        service: "netcore-kmf",
        version: env!("CARGO_PKG_VERSION"),
        started_at: state.started_at.clone(),
        management_security_mode: "open_lab",
        warning: SERVICE_WARNING,
        operating_mode: state.database.policy.operating_mode.clone(),
        authoritative: state.database.policy.operating_mode == OPERATING_MODE_AUTHORITATIVE,
        database_revision: state.database.revision,
        vault_revision: state.vault.revision,
        vault_provider: state.config.vault.provider.clone(),
        vault_ready: state.master_key.len() == 32,
        master_key_fingerprint: fingerprint(&state.master_key),
        raw_keys_exposed_by_management_api: false,
        total_keys,
        active_keys,
        staged_keys,
        revoked_keys,
        node_transport_profiles: state.database.nodes.len(),
        enabled_nodes: state.database.nodes.values().filter(|node| node.enabled).count(),
        otar_jobs: state.database.jobs.len(),
        pending_actions,
        audit_records: state.database.audit.len(),
        audit_head_hash: audit_head_hash(&state.database),
        backups: state.database.backups.len(),
        last_error: state.last_error.clone(),
        hsm_configured: state.config.vault.hsm_library.is_some(),
        hsm_connected: false,
    }
}

fn validate_key_kind_scope(kind: &str, scope: &str, value: Option<&str>) -> Result<(), String> {
    let kind = kind.to_ascii_uppercase();
    let scope = scope.to_ascii_lowercase();
    if !matches!(kind.as_str(), "CCK" | "GCK" | "SCK") {
        return Err("kind must be CCK, GCK or SCK".to_string());
    }
    match kind.as_str() {
        "CCK" if !matches!(scope.as_str(), "network" | "location_area") => {
            return Err("CCK scope must be network or location_area".to_string());
        }
        "GCK" if scope != "group" => {
            return Err("GCK scope must be group".to_string());
        }
        "SCK" if !matches!(scope.as_str(), "subscriber" | "group" | "network") => {
            return Err("SCK scope must be subscriber, group or network".to_string());
        }
        _ => {}
    }
    if scope != "network" && value.map(str::trim).unwrap_or_default().is_empty() {
        return Err(format!("scope_value is required for scope {scope}"));
    }
    if let Some(value) = value {
        if value.len() > 128 {
            return Err("scope_value is too long".to_string());
        }
        if matches!(scope.as_str(), "group" | "subscriber") {
            let ssi = value
                .parse::<u32>()
                .map_err(|_| format!("scope_value for {scope} must be a decimal 24-bit SSI"))?;
            if ssi > 0x00ff_ffff {
                return Err(format!("scope_value for {scope} must fit into 24 bits"));
            }
        }
    }
    Ok(())
}

fn validate_node_id(node_id: &str) -> Result<(), String> {
    if node_id.is_empty() || node_id.len() > 96 {
        return Err("node_id must contain between 1 and 96 characters".to_string());
    }
    if !node_id
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || "-_.:".contains(character))
    {
        return Err("node_id contains unsupported characters".to_string());
    }
    Ok(())
}

fn validate_24bit_values(values: &[u32], label: &str) -> Result<(), String> {
    if values.iter().any(|value| *value > 0x00ff_ffff) {
        return Err(format!("{label} values must fit into 24 bits"));
    }
    Ok(())
}

fn next_key_version(
    database: &KmfDatabase,
    kind: &str,
    scope: &str,
    scope_value: Option<&str>,
) -> u32 {
    database
        .keys
        .values()
        .filter(|key| {
            key.kind.eq_ignore_ascii_case(kind)
                && key.scope.eq_ignore_ascii_case(scope)
                && key.scope_value.as_deref() == scope_value
        })
        .map(|key| key.version)
        .max()
        .unwrap_or(0)
        .saturating_add(1)
}

fn same_scope(left: &KeyRecord, right: &KeyRecord) -> bool {
    left.kind == right.kind
        && left.scope == right.scope
        && left.scope_value == right.scope_value
}

fn periods_overlap(left: &KeyRecord, right: &KeyRecord) -> Result<bool, String> {
    let left_start = parse_time(&left.crypto_period_start)?;
    let left_end = parse_time(&left.crypto_period_end)?;
    let right_start = parse_time(&right.crypto_period_start)?;
    let right_end = parse_time(&right.crypto_period_end)?;
    Ok(left_start < right_end && right_start < left_end)
}

fn open_vault_entry(state: &KmfState, reference: &str) -> Result<Vec<u8>, String> {
    let blob = state
        .vault
        .entries
        .get(reference)
        .ok_or_else(|| format!("vault entry {reference} not found"))?;
    open(&state.master_key, blob, reference.as_bytes())
}

fn cancel_actions_for_node_locked(state: &mut KmfState, node_id: &str, reason: &str) {
    let snapshots = state
        .database
        .actions
        .values()
        .filter(|action| {
            action.node_id == node_id
                && matches!(
                    action.state,
                    ActionState::Staged | ActionState::Pending | ActionState::InFlight
                )
        })
        .map(|action| {
            (
                action.id.clone(),
                action.job_id.clone(),
                action.delivery_id.clone(),
            )
        })
        .collect::<Vec<_>>();
    let mut affected_jobs = Vec::new();
    for (action_id, job_id, delivery_id) in snapshots {
        if let Some(action) = state.database.actions.get_mut(&action_id) {
            action.state = ActionState::Cancelled;
            action.updated_at = now_iso();
            action.last_error = Some(reason.to_string());
        }
        update_delivery_locked(
            state,
            &job_id,
            &delivery_id,
            DeliveryState::Cancelled,
            None,
            Some(reason.to_string()),
        );
        affected_jobs.push(job_id);
    }
    affected_jobs.sort();
    affected_jobs.dedup();
    for job_id in affected_jobs {
        recompute_job_state_locked(state, &job_id);
    }
}

fn cancel_actions_for_key_locked(state: &mut KmfState, key_id: &str, reason: &str) {
    let snapshots = state
        .database
        .actions
        .values()
        .filter(|action| {
            action.key_id == key_id
                && matches!(
                    action.state,
                    ActionState::Staged | ActionState::Pending | ActionState::InFlight
                )
        })
        .map(|action| {
            (
                action.id.clone(),
                action.job_id.clone(),
                action.delivery_id.clone(),
            )
        })
        .collect::<Vec<_>>();
    for (action_id, job_id, delivery_id) in snapshots {
        if let Some(action) = state.database.actions.get_mut(&action_id) {
            action.state = ActionState::Cancelled;
            action.updated_at = now_iso();
            action.last_error = Some(reason.to_string());
        }
        update_delivery_locked(
            state,
            &job_id,
            &delivery_id,
            DeliveryState::Cancelled,
            None,
            Some(reason.to_string()),
        );
    }
    for job in state.database.jobs.values_mut() {
        if job.key_id == key_id && !is_terminal_job(job.state) {
            job.state = OtarJobState::Cancelled;
            job.completed_at = Some(now_iso());
            job.revision = job.revision.saturating_add(1);
        }
    }
}

fn update_delivery_locked(
    state: &mut KmfState,
    job_id: &str,
    delivery_id: &str,
    delivery_state: DeliveryState,
    applied_at: Option<String>,
    error: Option<String>,
) {
    if let Some(job) = state.database.jobs.get_mut(job_id) {
        if let Some(delivery) = job
            .deliveries
            .iter_mut()
            .find(|delivery| delivery.id == delivery_id)
        {
            delivery.state = delivery_state;
            delivery.updated_at = now_iso();
            delivery.attempts = delivery.attempts.saturating_add(1);
            if applied_at.is_some() {
                delivery.applied_at = applied_at;
            }
            delivery.last_error = error;
        }
        job.revision = job.revision.saturating_add(1);
    }
}

fn recompute_job_state_locked(state: &mut KmfState, job_id: &str) {
    if let Some(job) = state.database.jobs.get_mut(job_id) {
        let applied = job
            .deliveries
            .iter()
            .filter(|delivery| delivery.state == DeliveryState::Applied)
            .count();
        let failed = job
            .deliveries
            .iter()
            .filter(|delivery| {
                matches!(
                    delivery.state,
                    DeliveryState::Failed | DeliveryState::Expired | DeliveryState::Cancelled
                )
            })
            .count();
        let active = job.deliveries.len().saturating_sub(applied + failed);
        job.state = if !job.deliveries.is_empty() && applied == job.deliveries.len() {
            job.completed_at = Some(now_iso());
            OtarJobState::Completed
        } else if active == 0 && applied > 0 && failed > 0 {
            job.completed_at = Some(now_iso());
            OtarJobState::PartialFailure
        } else if active == 0 && failed == job.deliveries.len() {
            job.completed_at = Some(now_iso());
            OtarJobState::Failed
        } else if active > 0 {
            OtarJobState::InProgress
        } else {
            job.state
        };
        job.revision = job.revision.saturating_add(1);
    }
}

fn is_terminal_job(state: OtarJobState) -> bool {
    matches!(
        state,
        OtarJobState::Completed
            | OtarJobState::PartialFailure
            | OtarJobState::Failed
            | OtarJobState::Cancelled
            | OtarJobState::Expired
    )
}

fn append_audit_locked(
    state: &mut KmfState,
    actor: &str,
    action: &str,
    target: &str,
    outcome: &str,
    detail: Value,
) {
    let sequence = state.database.next_audit_sequence;
    state.database.next_audit_sequence = state.database.next_audit_sequence.saturating_add(1);
    let previous_hash = audit_head_hash(&state.database);
    let timestamp = now_iso();
    let content = json!({
        "sequence":sequence,
        "timestamp":timestamp.clone(),
        "actor":actor,
        "action":action,
        "target":target,
        "outcome":outcome,
        "detail":detail.clone(),
        "previous_hash":previous_hash.clone(),
    });
    let record_hash = sha256_hex(&serde_json::to_vec(&content).unwrap_or_default());
    state.database.audit.push_back(AuditRecord {
        sequence,
        timestamp,
        actor: actor.to_string(),
        action: action.to_string(),
        target: target.to_string(),
        outcome: outcome.to_string(),
        detail,
        previous_hash,
        record_hash,
    });
    while state.database.audit.len() > state.config.limits.max_audit {
        state.database.audit.pop_front();
    }
}

fn audit_head_hash(database: &KmfDatabase) -> String {
    database
        .audit
        .back()
        .map(|record| record.record_hash.clone())
        .unwrap_or_else(|| "0".repeat(64))
}

fn load_database(config: &KmfConfig) -> Result<KmfDatabase, Box<dyn std::error::Error>> {
    if !config.storage.database_path.exists() {
        return Ok(KmfDatabase::new(config));
    }
    let bytes = fs::read(&config.storage.database_path)?;
    let database = serde_json::from_slice::<KmfDatabase>(&bytes)?;
    if database.schema_version != DATABASE_SCHEMA_VERSION {
        return Err(format!(
            "unsupported KMF database schema {}",
            database.schema_version
        )
        .into());
    }
    Ok(database)
}

fn load_vault(config: &KmfConfig) -> Result<VaultDatabase, Box<dyn std::error::Error>> {
    if !config.storage.vault_path.exists() {
        return Ok(VaultDatabase::new());
    }
    let bytes = fs::read(&config.storage.vault_path)?;
    let vault = serde_json::from_slice::<VaultDatabase>(&bytes)?;
    if vault.schema_version != VAULT_SCHEMA_VERSION {
        return Err(format!("unsupported KMF vault schema {}", vault.schema_version).into());
    }
    Ok(vault)
}

fn persist_locked(state: &mut KmfState) -> Result<(), String> {
    state.database.revision = state.database.revision.saturating_add(1);
    let database_bytes =
        serde_json::to_vec_pretty(&state.database).map_err(|error| error.to_string())?;
    let vault_bytes =
        serde_json::to_vec_pretty(&state.vault).map_err(|error| error.to_string())?;
    atomic_write_private(&state.config.storage.database_path, &database_bytes)?;
    atomic_write_private(&state.config.storage.vault_path, &vault_bytes)?;
    Ok(())
}

fn atomic_write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temporary = temporary_path(path);
    write_private_file(&temporary, bytes)?;
    fs::rename(&temporary, path).map_err(|error| error.to_string())?;
    Ok(())
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut temporary = path.as_os_str().to_os_string();
    temporary.push(format!(".{}.tmp", Uuid::new_v4()));
    PathBuf::from(temporary)
}

fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

fn parse_time(value: &str) -> Result<DateTime<Utc>, String> {
    DateTime::parse_from_rfc3339(value)
        .map(|time| time.with_timezone(&Utc))
        .map_err(|error| format!("invalid RFC3339 timestamp {value}: {error}"))
}

fn parse_time_or(value: Option<&str>, fallback: DateTime<Utc>) -> Result<DateTime<Utc>, String> {
    match value {
        Some(value) if !value.trim().is_empty() => parse_time(value),
        _ => Ok(fallback),
    }
}

fn clean_text(value: String, max: usize) -> String {
    value
        .chars()
        .filter(|character| !character.is_control() || *character == '\n' || *character == '\t')
        .take(max)
        .collect()
}

fn non_empty(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_key_scopes() {
        assert!(validate_key_kind_scope("CCK", "network", None).is_ok());
        assert!(validate_key_kind_scope("GCK", "group", Some("15501")).is_ok());
        assert!(validate_key_kind_scope("GCK", "group", Some("16777216")).is_err());
        assert!(validate_key_kind_scope("SCK", "subscriber", Some("not-an-issi")).is_err());
        assert!(validate_key_kind_scope("GCK", "network", None).is_err());
    }

    #[test]
    fn audit_hash_chain_has_head() {
        let config = KmfConfig::default();
        let mut state = KmfState {
            database: KmfDatabase::new(&config),
            vault: VaultDatabase::new(),
            master_key: vec![1; 32],
            started_at: now_iso(),
            last_error: None,
            config,
        };
        append_audit_locked(
            &mut state,
            "test",
            "test.action",
            "target",
            "success",
            json!({}),
        );
        assert_ne!(audit_head_hash(&state.database), "0".repeat(64));
    }

    #[test]
    fn lab_envelope_algorithm_is_explicit() {
        assert_eq!(crate::crypto::LAB_ENVELOPE_ALGORITHM, "lab_sha256_stream_mac_v1");
    }
}

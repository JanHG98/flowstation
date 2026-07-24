use std::collections::BTreeMap;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorRecord {
    pub connector_id: String,
    pub display_name: String,
    pub kind: String,
    pub direction: String,
    pub endpoint: String,
    pub health_endpoint: Option<String>,
    pub enabled: bool,
    pub timeout_ms: u64,
    pub rate_limit_per_minute: u32,
    pub circuit_failure_threshold: u32,
    pub circuit_open_secs: u64,
    pub required_secrets: Vec<String>,
    pub settings: BTreeMap<String, String>,
    pub health: String,
    pub circuit_state: String,
    pub circuit_open_until: Option<DateTime<Utc>>,
    pub consecutive_failures: u32,
    pub sent_total: u64,
    pub failed_total: u64,
    pub received_total: u64,
    pub rate_window_started_at: DateTime<Utc>,
    pub rate_window_count: u32,
    pub last_probe_at: Option<DateTime<Utc>>,
    pub last_success_at: Option<DateTime<Utc>>,
    pub last_failure_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteRuleRecord {
    pub rule_id: String,
    pub name: String,
    pub enabled: bool,
    pub priority: i32,
    pub source_connector: String,
    pub event_type: String,
    pub text_contains: Option<String>,
    pub target_connector: String,
    pub template_id: Option<String>,
    pub destination: Option<String>,
    pub stop_processing: bool,
    pub matched_total: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateRecord {
    pub template_id: String,
    pub name: String,
    pub kind: String,
    pub body: String,
    pub content_type: String,
    pub enabled: bool,
    pub target_connector: Option<String>,
    pub default_destination: Option<String>,
    pub description: String,
    pub render_total: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRecord {
    pub event_id: String,
    pub source_connector: String,
    pub event_type: String,
    pub destination: Option<String>,
    pub text: Option<String>,
    pub payload: Value,
    pub idempotency_key: Option<String>,
    pub correlation_id: String,
    pub priority: i32,
    pub state: String,
    pub matched_rules: Vec<String>,
    pub delivery_ids: Vec<String>,
    pub received_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryRecord {
    pub delivery_id: String,
    pub event_id: String,
    pub connector_id: String,
    pub template_id: Option<String>,
    pub event_type: String,
    pub destination: Option<String>,
    pub text: Option<String>,
    pub payload: Value,
    pub content_type: String,
    pub correlation_id: String,
    pub priority: i32,
    pub state: String,
    pub attempts: u32,
    pub max_attempts: u32,
    pub next_attempt_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub last_attempt_at: Option<DateTime<Utc>>,
    pub delivered_at: Option<DateTime<Utc>>,
    pub response_status: Option<u16>,
    pub response_excerpt: Option<String>,
    pub last_error: Option<String>,
    pub artifact_path: Option<PathBuf>,
    pub artifact_sha256: Option<String>,
    pub artifact_size_bytes: Option<u64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsJobRecord {
    pub job_id: String,
    pub name: String,
    pub template_id: Option<String>,
    pub text: String,
    pub rendered_text: String,
    pub voice: String,
    pub speed: f32,
    pub speaker_id: Option<u32>,
    pub state: String,
    pub synthesis_delivery_id: String,
    pub publish_delivery_id: Option<String>,
    pub destination_kind: Option<String>,
    pub destination_id: Option<u32>,
    pub priority: u8,
    pub artifact_path: Option<PathBuf>,
    pub artifact_url: Option<String>,
    pub artifact_sha256: Option<String>,
    pub artifact_size_bytes: Option<u64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecord {
    pub sequence: u64,
    pub timestamp: DateTime<Utc>,
    pub actor: String,
    pub category: String,
    pub action: String,
    pub object_type: String,
    pub object_id: String,
    pub result: String,
    pub detail: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretEntry {
    pub value: String,
    pub fingerprint: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SecretVault {
    pub connectors: BTreeMap<String, BTreeMap<String, SecretEntry>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretStatus {
    pub connector_id: String,
    pub name: String,
    pub present: bool,
    pub fingerprint: Option<String>,
    pub updated_at: Option<DateTime<Utc>>,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupRecord {
    pub backup_id: String,
    pub path: PathBuf,
    pub state_sha256: String,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
    pub note: Option<String>,
    pub includes_secrets: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedGateway {
    pub schema_version: u32,
    pub connectors: BTreeMap<String, ConnectorRecord>,
    pub rules: BTreeMap<String, RouteRuleRecord>,
    pub templates: BTreeMap<String, TemplateRecord>,
    pub events: Vec<EventRecord>,
    pub deliveries: Vec<DeliveryRecord>,
    pub tts_jobs: Vec<TtsJobRecord>,
    pub audit: Vec<AuditRecord>,
    pub backups: Vec<BackupRecord>,
    pub audit_sequence: u64,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayStatus {
    pub ready: bool,
    pub security_mode: String,
    pub operating_mode: String,
    pub management_token_auth: bool,
    pub management_tls: bool,
    pub connectors_total: usize,
    pub connectors_enabled: usize,
    pub connectors_healthy: usize,
    pub connectors_degraded: usize,
    pub circuits_open: usize,
    pub events_total: usize,
    pub events_unrouted: usize,
    pub deliveries_queued: usize,
    pub deliveries_retry: usize,
    pub deliveries_delivered: usize,
    pub deliveries_shadowed: usize,
    pub deliveries_dead_letter: usize,
    pub tts_jobs_total: usize,
    pub tts_jobs_ready: usize,
    pub missing_required_secrets: usize,
    pub state_path: String,
    pub secrets_path: String,
    pub spool_dir: String,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchInput {
    pub source_connector: Option<String>,
    pub event_type: String,
    pub destination: Option<String>,
    pub text: Option<String>,
    #[serde(default)]
    pub payload: Value,
    pub idempotency_key: Option<String>,
    pub correlation_id: Option<String>,
    pub priority: Option<i32>,
    pub ttl_secs: Option<u64>,
    #[serde(default)]
    pub target_connectors: Vec<String>,
    pub template_id: Option<String>,
    pub actor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorInput {
    pub connector_id: String,
    pub display_name: String,
    pub kind: String,
    pub direction: String,
    pub endpoint: String,
    pub health_endpoint: Option<String>,
    pub enabled: Option<bool>,
    pub timeout_ms: Option<u64>,
    pub rate_limit_per_minute: Option<u32>,
    pub circuit_failure_threshold: Option<u32>,
    pub circuit_open_secs: Option<u64>,
    #[serde(default)]
    pub required_secrets: Vec<String>,
    #[serde(default)]
    pub settings: BTreeMap<String, String>,
    pub actor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteRuleInput {
    pub rule_id: String,
    pub name: String,
    pub enabled: Option<bool>,
    pub priority: Option<i32>,
    pub source_connector: String,
    pub event_type: String,
    pub text_contains: Option<String>,
    pub target_connector: String,
    pub template_id: Option<String>,
    pub destination: Option<String>,
    pub stop_processing: Option<bool>,
    pub actor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateInput {
    pub template_id: String,
    pub name: String,
    pub kind: String,
    pub body: String,
    pub content_type: Option<String>,
    pub enabled: Option<bool>,
    pub target_connector: Option<String>,
    pub default_destination: Option<String>,
    pub description: Option<String>,
    pub actor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretSetInput {
    pub name: String,
    pub value: String,
    pub actor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ActionInput {
    pub actor: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsJobInput {
    pub name: String,
    pub text: String,
    pub template_id: Option<String>,
    pub voice: Option<String>,
    pub speed: Option<f32>,
    pub speaker_id: Option<u32>,
    pub destination_kind: Option<String>,
    pub destination_id: Option<u32>,
    pub priority: Option<u8>,
    pub actor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TtsPublishInput {
    pub actor: Option<String>,
    pub destination_kind: Option<String>,
    pub destination_id: Option<u32>,
    pub priority: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateRenderInput {
    pub text: Option<String>,
    pub destination: Option<String>,
    pub source: Option<String>,
    pub event_type: Option<String>,
    #[serde(default)]
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BackupInput {
    pub actor: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ClaimedDelivery {
    pub delivery: DeliveryRecord,
    pub connector: ConnectorRecord,
    pub secrets: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct DeliveryOutcome {
    pub success: bool,
    pub status: Option<u16>,
    pub response_excerpt: Option<String>,
    pub error: Option<String>,
    pub artifact_path: Option<PathBuf>,
    pub artifact_sha256: Option<String>,
    pub artifact_size_bytes: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct ConnectorProbeOutcome {
    pub connector_id: String,
    pub success: bool,
    pub status: Option<u16>,
    pub response_ms: f64,
    pub error: Option<String>,
}

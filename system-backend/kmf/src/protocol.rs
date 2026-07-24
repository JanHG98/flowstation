use serde::{Deserialize, Serialize};

pub const OTAR_EDGE_PROTOCOL_VERSION: &str = "netcore-kmf-otar-edge-v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyInput {
    pub operating_mode: String,
    pub default_key_bytes: usize,
    pub default_crypto_period_secs: u64,
    pub rotation_lead_secs: u64,
    pub require_dual_approval: bool,
    pub allow_overlapping_crypto_periods: bool,
    pub auto_retire_predecessor: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyCreateInput {
    pub kind: String,
    pub scope: String,
    pub scope_value: Option<String>,
    pub label: String,
    pub algorithm_profile: Option<String>,
    pub key_bytes: Option<usize>,
    pub crypto_period_start: Option<String>,
    pub crypto_period_end: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KeyRotateInput {
    pub actor: Option<String>,
    pub activate_at: Option<String>,
    pub crypto_period_end: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LifecycleInput {
    pub actor: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeCreateInput {
    pub node_id: String,
    pub display_name: String,
    pub actor: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NodeStateInput {
    pub actor: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtarJobCreateInput {
    pub key_id: String,
    pub target_nodes: Vec<String>,
    pub target_issis: Vec<u32>,
    pub target_gssis: Vec<u32>,
    pub not_before: Option<String>,
    pub expires_at: Option<String>,
    pub actor: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtarApprovalInput {
    pub actor: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OtarQueueInput {
    pub actor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeClaimInput {
    pub node_id: String,
    pub max_actions: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeActionAckInput {
    pub success: bool,
    pub error: Option<String>,
    pub applied_at: Option<String>,
}

impl Default for EdgeActionAckInput {
    fn default() -> Self {
        Self {
            success: true,
            error: None,
            applied_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BackupInput {
    pub actor: Option<String>,
    pub note: Option<String>,
}

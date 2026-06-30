use std::{collections::HashMap, time::Duration};

use serde::Deserialize;
use toml::Value;

/// NetCore Directory / SwMI-Light runtime export configuration.
///
/// This is deliberately HTTP/JSON and fire-and-forget: it publishes the BS runtime state
/// to the local NetCore Directory server without making the RF stack depend on it.
#[derive(Debug, Clone)]
pub struct CfgDirectory {
    /// Master switch for publishing live BS/runtime data to NetCore Directory.
    pub enabled: bool,
    /// Base URL of the Directory server, e.g. http://127.0.0.1:8095.
    pub base_url: String,
    /// Source label written into Directory rows (usually the station name).
    pub source: String,
    /// ISSI used to identify this base station/control endpoint in Directory rows.
    pub bs_issi: u32,
    /// HTTP request timeout.
    pub timeout: Duration,
    /// Publish registration / deregistration / group / RSSI information.
    pub publish_presence: bool,
    /// Publish U-STATUS / Directory status labels into the live status board.
    pub publish_status: bool,
    /// Publish group and individual call start/end rows into CDR.
    pub publish_cdr: bool,
    /// Publish emergency enter/cancel events.
    pub publish_emergencies: bool,
    /// Publish health snapshots / host and SDR health as health-events.
    pub publish_health: bool,
    /// Publish SDS activity as lightweight CDR/activity rows.
    pub publish_sds_activity: bool,
    /// Publish decoded LIP/APRS positions when SDS PID 10 contains a decoded coordinate string.
    pub publish_positions: bool,
    /// If true, local-originated SDS and call setup consult `/api/policy-check` before proceeding.
    pub enforce_policies: bool,
    /// If true, a failed/unreachable policy check denies traffic. Default is fail-open so the RF
    /// cell keeps working when the Directory server is down.
    pub policy_fail_closed: bool,
}

#[derive(Deserialize)]
pub struct CfgDirectoryDto {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_directory_base_url")]
    pub base_url: String,
    #[serde(default = "default_directory_source")]
    pub source: String,
    #[serde(default = "default_directory_bs_issi")]
    pub bs_issi: u32,
    #[serde(default = "default_directory_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_true")]
    pub publish_presence: bool,
    #[serde(default = "default_true")]
    pub publish_status: bool,
    #[serde(default = "default_true")]
    pub publish_cdr: bool,
    #[serde(default = "default_true")]
    pub publish_emergencies: bool,
    #[serde(default = "default_true")]
    pub publish_health: bool,
    #[serde(default = "default_true")]
    pub publish_sds_activity: bool,
    #[serde(default = "default_true")]
    pub publish_positions: bool,
    #[serde(default)]
    pub enforce_policies: bool,
    #[serde(default)]
    pub policy_fail_closed: bool,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl Default for CfgDirectoryDto {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: default_directory_base_url(),
            source: default_directory_source(),
            bs_issi: default_directory_bs_issi(),
            timeout_ms: default_directory_timeout_ms(),
            publish_presence: true,
            publish_status: true,
            publish_cdr: true,
            publish_emergencies: true,
            publish_health: true,
            publish_sds_activity: true,
            publish_positions: true,
            enforce_policies: false,
            policy_fail_closed: false,
            extra: HashMap::new(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_directory_base_url() -> String {
    "http://127.0.0.1:8095".to_string()
}

fn default_directory_source() -> String {
    "bluestation-bs".to_string()
}

fn default_directory_bs_issi() -> u32 {
    // NetCore's current local control / dashboard ISSI convention.
    4010001
}

fn default_directory_timeout_ms() -> u64 {
    1500
}

pub fn apply_directory_patch(src: CfgDirectoryDto) -> Result<CfgDirectory, String> {
    let base_url = if src.base_url.trim().is_empty() {
        default_directory_base_url()
    } else {
        src.base_url.trim().trim_end_matches('/').to_string()
    };
    if src.enabled && !(base_url.starts_with("http://") || base_url.starts_with("https://")) {
        return Err("netcore_directory: base_url must start with http:// or https://".to_string());
    }

    Ok(CfgDirectory {
        enabled: src.enabled,
        base_url,
        source: if src.source.trim().is_empty() {
            default_directory_source()
        } else {
            src.source.trim().to_string()
        },
        bs_issi: if src.bs_issi == 0 { default_directory_bs_issi() } else { src.bs_issi },
        timeout: Duration::from_millis(src.timeout_ms.clamp(250, 10_000)),
        publish_presence: src.publish_presence,
        publish_status: src.publish_status,
        publish_cdr: src.publish_cdr,
        publish_emergencies: src.publish_emergencies,
        publish_health: src.publish_health,
        publish_sds_activity: src.publish_sds_activity,
        publish_positions: src.publish_positions,
        enforce_policies: src.enforce_policies,
        policy_fail_closed: src.policy_fail_closed,
    })
}

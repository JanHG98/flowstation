use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const OPEN_LAB_MODE: &str = "open_lab";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ObservabilityConfig {
    pub server: ServerConfig,
    pub storage: StorageConfig,
    pub security: SecurityConfig,
    pub collection: CollectionConfig,
    pub retention: RetentionConfig,
    pub stack: StackConfig,
    pub targets: Vec<TargetConfig>,
    pub alert_rules: Vec<AlertRuleConfig>,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            storage: StorageConfig::default(),
            security: SecurityConfig::default(),
            collection: CollectionConfig::default(),
            retention: RetentionConfig::default(),
            stack: StackConfig::default(),
            targets: default_targets(),
            alert_rules: default_rules(),
        }
    }
}

impl ObservabilityConfig {
    pub fn load(path: Option<&Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let mut config = match path {
            Some(path) => toml::from_str::<Self>(&fs::read_to_string(path)?)?,
            None => Self::default(),
        };
        config
            .normalise()
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;
        Ok(config)
    }

    pub fn apply_bind_override(&mut self, bind: Option<SocketAddr>) -> Result<(), String> {
        if let Some(bind) = bind {
            self.server.bind = bind;
        }
        self.normalise()
    }

    fn normalise(&mut self) -> Result<(), String> {
        if self.security.mode != OPEN_LAB_MODE {
            return Err(format!(
                "unsupported security.mode={}; this package intentionally supports open_lab only",
                self.security.mode
            ));
        }
        if self.security.token_auth || self.security.tls {
            return Err("security.token_auth and security.tls must remain false in open_lab".into());
        }
        if !self.security.allow_remote_management && !self.server.bind.ip().is_loopback() {
            return Err("server.bind must be loopback when allow_remote_management=false".into());
        }
        if self.collection.scrape_interval_secs == 0 {
            return Err("collection.scrape_interval_secs must be at least 1".into());
        }
        if self.collection.request_timeout_ms < 100 {
            return Err("collection.request_timeout_ms must be at least 100".into());
        }
        self.server.max_body_bytes = self.server.max_body_bytes.max(65_536);
        self.server.history_limit = self.server.history_limit.max(100);
        self.collection.request_timeout_ms = self.collection.request_timeout_ms.max(100);
        self.collection.max_response_bytes = self.collection.max_response_bytes.max(65_536);
        self.collection.scrape_interval_secs = self.collection.scrape_interval_secs.max(1);
        self.retention.metric_retention_secs = self.retention.metric_retention_secs.max(60);
        self.retention.log_retention_secs = self.retention.log_retention_secs.max(60);
        self.retention.trace_retention_secs = self.retention.trace_retention_secs.max(60);
        self.retention.audit_retention_secs = self.retention.audit_retention_secs.max(300);
        self.retention.max_series = self.retention.max_series.max(100);
        self.retention.max_samples_per_series = self.retention.max_samples_per_series.max(10);
        self.retention.max_logs = self.retention.max_logs.max(100);
        self.retention.max_spans = self.retention.max_spans.max(100);
        self.retention.max_alerts = self.retention.max_alerts.max(100);
        self.retention.max_audit_records = self.retention.max_audit_records.max(100);

        let mut ids = std::collections::HashSet::new();
        for target in &mut self.targets {
            target.target_id = target.target_id.trim().to_ascii_lowercase();
            if target.target_id.is_empty() || !ids.insert(target.target_id.clone()) {
                return Err(format!("target_id must be non-empty and unique: {}", target.target_id));
            }
            if !target.base_url.starts_with("http://") {
                return Err(format!("target {} must use http:// in open_lab", target.target_id));
            }
            if !target.metrics_path.starts_with('/') || !target.live_path.starts_with('/') || !target.ready_path.starts_with('/') {
                return Err(format!("target {} paths must start with /", target.target_id));
            }
        }
        let mut rule_ids = std::collections::HashSet::new();
        for rule in &mut self.alert_rules {
            rule.rule_id = rule.rule_id.trim().to_ascii_lowercase();
            if rule.rule_id.is_empty() || !rule_ids.insert(rule.rule_id.clone()) {
                return Err(format!("rule_id must be non-empty and unique: {}", rule.rule_id));
            }
            if !matches!(rule.comparator.as_str(), ">" | ">=" | "<" | "<=" | "==" | "!=") {
                return Err(format!("rule {} has invalid comparator", rule.rule_id));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub bind: SocketAddr,
    pub history_limit: usize,
    pub max_body_bytes: usize,
}
impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: "0.0.0.0:8210".parse().expect("valid default bind"),
            history_limit: 5_000,
            max_body_bytes: 4 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StorageConfig {
    pub state_path: PathBuf,
    pub backup_path: PathBuf,
    pub diagnostic_dir: PathBuf,
}
impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            state_path: "/var/lib/netcore-observability/state.json".into(),
            backup_path: "/var/lib/netcore-observability/state.json.bak".into(),
            diagnostic_dir: "/var/lib/netcore-observability/diagnostics".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SecurityConfig {
    pub mode: String,
    pub token_auth: bool,
    pub tls: bool,
    pub allow_remote_management: bool,
    pub warning_banner: String,
}
impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            mode: OPEN_LAB_MODE.to_string(),
            token_auth: false,
            tls: false,
            allow_remote_management: true,
            warning_banner: "OPEN LAB: no login, no tokens and no TLS. Isolated management network only.".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CollectionConfig {
    pub scrape_interval_secs: u64,
    pub request_timeout_ms: u64,
    pub max_response_bytes: usize,
    pub scrape_on_start: bool,
    pub ingest_logs: bool,
    pub ingest_traces: bool,
}
impl Default for CollectionConfig {
    fn default() -> Self {
        Self {
            scrape_interval_secs: 15,
            request_timeout_ms: 2_000,
            max_response_bytes: 2 * 1024 * 1024,
            scrape_on_start: true,
            ingest_logs: true,
            ingest_traces: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RetentionConfig {
    pub metric_retention_secs: u64,
    pub log_retention_secs: u64,
    pub trace_retention_secs: u64,
    pub audit_retention_secs: u64,
    pub max_series: usize,
    pub max_samples_per_series: usize,
    pub max_logs: usize,
    pub max_spans: usize,
    pub max_alerts: usize,
    pub max_audit_records: usize,
}
impl Default for RetentionConfig {
    fn default() -> Self {
        Self {
            metric_retention_secs: 24 * 3600,
            log_retention_secs: 7 * 24 * 3600,
            trace_retention_secs: 24 * 3600,
            audit_retention_secs: 30 * 24 * 3600,
            max_series: 10_000,
            max_samples_per_series: 5_760,
            max_logs: 100_000,
            max_spans: 50_000,
            max_alerts: 10_000,
            max_audit_records: 50_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StackConfig {
    pub prometheus_url: String,
    pub grafana_url: String,
    pub loki_url: String,
    pub alertmanager_url: String,
    pub prometheus_ready_path: String,
    pub grafana_ready_path: String,
    pub loki_ready_path: String,
    pub alertmanager_ready_path: String,
}
impl Default for StackConfig {
    fn default() -> Self {
        Self {
            prometheus_url: "http://127.0.0.1:9090".to_string(),
            grafana_url: "http://127.0.0.1:3000".to_string(),
            loki_url: "http://127.0.0.1:3100".to_string(),
            alertmanager_url: "http://127.0.0.1:9093".to_string(),
            prometheus_ready_path: "/-/ready".to_string(),
            grafana_ready_path: "/api/health".to_string(),
            loki_ready_path: "/ready".to_string(),
            alertmanager_ready_path: "/-/ready".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TargetConfig {
    pub target_id: String,
    pub display_name: String,
    pub service: String,
    pub base_url: String,
    pub metrics_path: String,
    pub live_path: String,
    pub ready_path: String,
    pub events_path: Option<String>,
    pub enabled: bool,
    pub labels: std::collections::BTreeMap<String, String>,
}
impl Default for TargetConfig {
    fn default() -> Self {
        Self {
            target_id: "service".to_string(),
            display_name: "Service".to_string(),
            service: "service".to_string(),
            base_url: "http://127.0.0.1:8080".to_string(),
            metrics_path: "/metrics".to_string(),
            live_path: "/health/live".to_string(),
            ready_path: "/health/ready".to_string(),
            events_path: None,
            enabled: true,
            labels: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AlertRuleConfig {
    pub rule_id: String,
    pub name: String,
    pub description: String,
    pub metric: String,
    pub comparator: String,
    pub threshold: f64,
    pub for_secs: u64,
    pub severity: String,
    pub service: Option<String>,
    pub target_id: Option<String>,
    pub enabled: bool,
    pub labels: std::collections::BTreeMap<String, String>,
    pub annotations: std::collections::BTreeMap<String, String>,
}
impl Default for AlertRuleConfig {
    fn default() -> Self {
        Self {
            rule_id: "target-down".to_string(),
            name: "Target down".to_string(),
            description: "A monitored target does not answer its liveness endpoint".to_string(),
            metric: "netcore_observability_target_up".to_string(),
            comparator: "<".to_string(),
            threshold: 1.0,
            for_secs: 30,
            severity: "critical".to_string(),
            service: None,
            target_id: None,
            enabled: true,
            labels: Default::default(),
            annotations: Default::default(),
        }
    }
}

fn target(id: &str, name: &str, service: &str, port: u16) -> TargetConfig {
    TargetConfig {
        target_id: id.to_string(),
        display_name: name.to_string(),
        service: service.to_string(),
        base_url: format!("http://127.0.0.1:{port}"),
        labels: std::collections::BTreeMap::from([
            ("environment".to_string(), "open-lab".to_string()),
            ("component".to_string(), service.to_string()),
        ]),
        ..TargetConfig::default()
    }
}

fn default_targets() -> Vec<TargetConfig> {
    vec![
        target("node-gateway", "Node Gateway", "node-gateway", 8080),
        target("mobility-core", "Mobility Core", "mobility-core", 8090),
        target("subscriber-core", "Subscriber Core", "subscriber-core", 8100),
        target("group-core", "Group Core", "group-core", 8110),
        target("call-control", "Call Control", "call-control", 8120),
        target("media-switch", "Media Switch", "media-switch", 8130),
        target("recorder", "Recorder", "recorder", 8140),
        target("sds-router", "SDS Router", "sds-router", 8150),
        target("packet-core", "Packet Core", "packet-core", 8160),
        target("ip-gateway", "IP Gateway", "ip-gateway", 8170),
        target("security-core", "Security Core", "security-core", 8180),
        target("kmf", "KMF", "kmf", 8190),
        target("transit", "Transit", "transit", 8200),
        target("control-room", "Control Room", "control-room", 9010),
    ]
}

fn default_rules() -> Vec<AlertRuleConfig> {
    vec![
        AlertRuleConfig::default(),
        AlertRuleConfig {
            rule_id: "target-not-ready".to_string(),
            name: "Target not ready".to_string(),
            description: "A service is alive but reports not ready".to_string(),
            metric: "netcore_observability_target_ready".to_string(),
            comparator: "<".to_string(),
            threshold: 1.0,
            for_secs: 60,
            severity: "warning".to_string(),
            ..AlertRuleConfig::default()
        },
        AlertRuleConfig {
            rule_id: "scrape-errors".to_string(),
            name: "Repeated scrape errors".to_string(),
            description: "The NMS cannot collect metrics from a target".to_string(),
            metric: "netcore_observability_target_consecutive_failures".to_string(),
            comparator: ">=".to_string(),
            threshold: 3.0,
            for_secs: 0,
            severity: "warning".to_string(),
            ..AlertRuleConfig::default()
        },
    ]
}

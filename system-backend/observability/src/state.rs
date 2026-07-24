use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::collector::ScrapeResult;
use crate::config::{AlertRuleConfig, ObservabilityConfig, TargetConfig};
use crate::protocol::{
    ActionInput, DiagnosticInput, LogIngestInput, RuleInput, SilenceInput, TargetCreateInput,
    TraceIngestInput,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetRecord {
    pub target_id: String,
    pub display_name: String,
    pub service: String,
    pub base_url: String,
    pub metrics_path: String,
    pub live_path: String,
    pub ready_path: String,
    pub events_path: Option<String>,
    pub enabled: bool,
    pub labels: BTreeMap<String, String>,
    pub live: bool,
    pub ready: bool,
    pub metrics_ok: bool,
    pub response_ms: Option<f64>,
    pub consecutive_failures: u64,
    pub scrape_failures_total: u64,
    pub sample_count: u64,
    pub series_count: usize,
    pub last_scrape_at: Option<DateTime<Utc>>,
    pub last_success_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TargetRecord {
    fn from_config(value: &TargetConfig, now: DateTime<Utc>) -> Self {
        Self {
            target_id: value.target_id.clone(),
            display_name: value.display_name.clone(),
            service: value.service.clone(),
            base_url: value.base_url.clone(),
            metrics_path: value.metrics_path.clone(),
            live_path: value.live_path.clone(),
            ready_path: value.ready_path.clone(),
            events_path: value.events_path.clone(),
            enabled: value.enabled,
            labels: value.labels.clone(),
            live: false,
            ready: false,
            metrics_ok: false,
            response_ms: None,
            consecutive_failures: 0,
            scrape_failures_total: 0,
            sample_count: 0,
            series_count: 0,
            last_scrape_at: None,
            last_success_at: None,
            last_error: None,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MetricPointInput {
    pub name: String,
    pub labels: BTreeMap<String, String>,
    pub value: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricSample {
    pub timestamp: DateTime<Utc>,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricSeries {
    pub series_id: String,
    pub target_id: String,
    pub service: String,
    pub name: String,
    pub labels: BTreeMap<String, String>,
    pub samples: Vec<MetricSample>,
    pub last_value: f64,
    pub last_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRuleRecord {
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
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
impl AlertRuleRecord {
    fn from_config(value: &AlertRuleConfig, now: DateTime<Utc>) -> Self {
        Self {
            rule_id: value.rule_id.clone(),
            name: value.name.clone(),
            description: value.description.clone(),
            metric: value.metric.clone(),
            comparator: value.comparator.clone(),
            threshold: value.threshold,
            for_secs: value.for_secs,
            severity: value.severity.clone(),
            service: value.service.clone(),
            target_id: value.target_id.clone(),
            enabled: value.enabled,
            labels: value.labels.clone(),
            annotations: value.annotations.clone(),
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRecord {
    pub alert_id: String,
    pub fingerprint: String,
    pub rule_id: String,
    pub rule_name: String,
    pub severity: String,
    pub state: String,
    pub service: String,
    pub target_id: String,
    pub metric: String,
    pub series_id: String,
    pub value: f64,
    pub comparator: String,
    pub threshold: f64,
    pub summary: String,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub first_seen_at: DateTime<Utc>,
    pub pending_since: DateTime<Utc>,
    pub fired_at: Option<DateTime<Utc>>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub acknowledged: bool,
    pub acknowledged_at: Option<DateTime<Utc>>,
    pub acknowledged_by: Option<String>,
    pub acknowledgement_note: Option<String>,
    pub silenced: bool,
    pub silence_ids: Vec<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SilenceRecord {
    pub silence_id: String,
    pub comment: String,
    pub created_by: String,
    pub rule_id: Option<String>,
    pub service: Option<String>,
    pub target_id: Option<String>,
    pub severity: Option<String>,
    pub match_labels: BTreeMap<String, String>,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub expired_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogRecord {
    pub log_id: String,
    pub timestamp: DateTime<Utc>,
    pub received_at: DateTime<Utc>,
    pub service: String,
    pub node: Option<String>,
    pub level: String,
    pub message: String,
    pub correlation_id: Option<String>,
    pub trace_id: Option<String>,
    pub fields: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceSpan {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub service: String,
    pub operation: String,
    pub started_at: DateTime<Utc>,
    pub received_at: DateTime<Utc>,
    pub duration_ms: f64,
    pub status: String,
    pub attributes: BTreeMap<String, Value>,
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
pub struct DiagnosticRecord {
    pub diagnostic_id: String,
    pub state: String,
    pub reason: Option<String>,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub archive_path: Option<PathBuf>,
    pub sha256: Option<String>,
    pub size_bytes: Option<u64>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackComponent {
    pub component: String,
    pub endpoint: String,
    pub ready: bool,
    pub response_ms: Option<f64>,
    pub checked_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StackProbe {
    pub component: String,
    pub endpoint: String,
    pub ready: bool,
    pub response_ms: f64,
    pub checked_at: DateTime<Utc>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct PersistentState {
    targets: HashMap<String, TargetRecord>,
    series: HashMap<String, MetricSeries>,
    rules: HashMap<String, AlertRuleRecord>,
    alerts: HashMap<String, AlertRecord>,
    silences: HashMap<String, SilenceRecord>,
    logs: Vec<LogRecord>,
    traces: Vec<TraceSpan>,
    audit: Vec<AuditRecord>,
    diagnostics: HashMap<String, DiagnosticRecord>,
    stack: HashMap<String, StackComponent>,
    sequence: u64,
    started_at: DateTime<Utc>,
    last_maintenance_at: Option<DateTime<Utc>>,
}
impl Default for PersistentState {
    fn default() -> Self {
        Self {
            targets: HashMap::new(),
            series: HashMap::new(),
            rules: HashMap::new(),
            alerts: HashMap::new(),
            silences: HashMap::new(),
            logs: Vec::new(),
            traces: Vec::new(),
            audit: Vec::new(),
            diagnostics: HashMap::new(),
            stack: HashMap::new(),
            sequence: 0,
            started_at: Utc::now(),
            last_maintenance_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ObservabilityStatus {
    pub ready: bool,
    pub security_mode: String,
    pub warning: String,
    pub started_at: DateTime<Utc>,
    pub targets_total: usize,
    pub targets_up: usize,
    pub targets_ready: usize,
    pub series: usize,
    pub samples: usize,
    pub logs: usize,
    pub traces: usize,
    pub alerts_firing: usize,
    pub alerts_pending: usize,
    pub alerts_unacknowledged: usize,
    pub active_silences: usize,
    pub stack_ready: usize,
    pub stack_total: usize,
    pub last_maintenance_at: Option<DateTime<Utc>>,
}

#[derive(Clone)]
pub struct SharedObservability {
    config: ObservabilityConfig,
    inner: Arc<Mutex<PersistentState>>,
}

impl SharedObservability {
    pub fn load(config: ObservabilityConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let mut state = if config.storage.state_path.is_file() {
            serde_json::from_slice::<PersistentState>(&fs::read(&config.storage.state_path)?)?
        } else {
            PersistentState::default()
        };
        let now = Utc::now();
        for target in &config.targets {
            state.targets.entry(target.target_id.clone()).or_insert_with(|| TargetRecord::from_config(target, now));
        }
        for rule in &config.alert_rules {
            state.rules.entry(rule.rule_id.clone()).or_insert_with(|| AlertRuleRecord::from_config(rule, now));
        }
        for (component, endpoint) in [
            ("prometheus", config.stack.prometheus_url.clone()),
            ("grafana", config.stack.grafana_url.clone()),
            ("loki", config.stack.loki_url.clone()),
            ("alertmanager", config.stack.alertmanager_url.clone()),
        ] {
            state.stack.entry(component.to_string()).or_insert(StackComponent {
                component: component.to_string(), endpoint, ready: false, response_ms: None,
                checked_at: None, last_error: None,
            });
        }
        let shared = Self { config, inner: Arc::new(Mutex::new(state)) };
        shared.persist()?;
        Ok(shared)
    }

    pub fn status(&self) -> ObservabilityStatus {
        let state = self.lock();
        let enabled: Vec<&TargetRecord> = state.targets.values().filter(|target| target.enabled).collect();
        let firing: Vec<&AlertRecord> = state.alerts.values().filter(|alert| alert.state == "firing").collect();
        ObservabilityStatus {
            ready: true,
            security_mode: self.config.security.mode.clone(),
            warning: self.config.security.warning_banner.clone(),
            started_at: state.started_at,
            targets_total: enabled.len(),
            targets_up: enabled.iter().filter(|target| target.live).count(),
            targets_ready: enabled.iter().filter(|target| target.ready).count(),
            series: state.series.len(),
            samples: state.series.values().map(|series| series.samples.len()).sum(),
            logs: state.logs.len(),
            traces: state.traces.len(),
            alerts_firing: firing.len(),
            alerts_pending: state.alerts.values().filter(|alert| alert.state == "pending").count(),
            alerts_unacknowledged: firing.iter().filter(|alert| !alert.acknowledged && !alert.silenced).count(),
            active_silences: state.silences.values().filter(|silence| silence.active).count(),
            stack_ready: state.stack.values().filter(|component| component.ready).count(),
            stack_total: state.stack.len(),
            last_maintenance_at: state.last_maintenance_at,
        }
    }

    pub fn targets_for_scrape(&self) -> Vec<TargetRecord> {
        let state = self.lock();
        let mut values: Vec<_> = state.targets.values().filter(|target| target.enabled).cloned().collect();
        values.sort_by(|a, b| a.target_id.cmp(&b.target_id));
        values
    }

    pub fn targets(&self) -> Vec<TargetRecord> {
        let state = self.lock();
        let mut values: Vec<_> = state.targets.values().cloned().collect();
        values.sort_by(|a, b| a.target_id.cmp(&b.target_id));
        values
    }

    pub fn target(&self, target_id: &str) -> Option<TargetRecord> {
        self.lock().targets.get(target_id).cloned()
    }

    pub fn create_target(&self, input: TargetCreateInput) -> Result<TargetRecord, String> {
        validate_target_input(&input)?;
        let mut state = self.lock();
        let id = input.target_id.trim().to_ascii_lowercase();
        if state.targets.contains_key(&id) { return Err(format!("target {id} already exists")); }
        let now = Utc::now();
        let record = TargetRecord {
            target_id: id.clone(), display_name: input.display_name, service: input.service,
            base_url: input.base_url.trim_end_matches('/').to_string(),
            metrics_path: input.metrics_path.unwrap_or_else(|| "/metrics".to_string()),
            live_path: input.live_path.unwrap_or_else(|| "/health/live".to_string()),
            ready_path: input.ready_path.unwrap_or_else(|| "/health/ready".to_string()),
            events_path: input.events_path, enabled: input.enabled.unwrap_or(true), labels: input.labels,
            live: false, ready: false, metrics_ok: false, response_ms: None, consecutive_failures: 0,
            scrape_failures_total: 0, sample_count: 0, series_count: 0, last_scrape_at: None,
            last_success_at: None, last_error: None, created_at: now, updated_at: now,
        };
        state.targets.insert(id.clone(), record.clone());
        audit(&mut state, actor(None), "configuration", "target.create", "target", &id, "success", json!({"base_url":record.base_url}));
        drop(state); self.persist().map_err(|error| error.to_string())?;
        Ok(record)
    }

    pub fn target_action(&self, target_id: &str, action: &str, input: ActionInput) -> Result<TargetRecord, String> {
        let mut state = self.lock();
        let now = Utc::now();
        let record = state.targets.get_mut(target_id).ok_or_else(|| format!("target {target_id} not found"))?;
        match action {
            "enable" => record.enabled = true,
            "disable" => record.enabled = false,
            "reset-failures" => { record.consecutive_failures = 0; record.last_error = None; }
            _ => return Err(format!("unsupported target action {action}")),
        }
        record.updated_at = now;
        let result = record.clone();
        audit(&mut state, actor(input.actor), "configuration", &format!("target.{action}"), "target", target_id, "success", json!({"reason":input.reason}));
        drop(state); self.persist().map_err(|error| error.to_string())?;
        Ok(result)
    }

    pub fn delete_target(&self, target_id: &str, input: ActionInput) -> Result<(), String> {
        let mut state = self.lock();
        if state.targets.remove(target_id).is_none() { return Err(format!("target {target_id} not found")); }
        state.series.retain(|_, series| series.target_id != target_id);
        audit(&mut state, actor(input.actor), "configuration", "target.delete", "target", target_id, "success", json!({"reason":input.reason}));
        drop(state); self.persist().map_err(|error| error.to_string())
    }

    pub fn record_scrape(&self, result: ScrapeResult) -> Result<(), Box<dyn std::error::Error>> {
        let mut state = self.lock();
        let service;
        {
            let Some(target) = state.targets.get_mut(&result.target_id) else { return Ok(()); };
            service = target.service.clone();
            target.live = result.live;
            target.ready = result.ready;
            target.metrics_ok = result.metrics_ok;
            target.response_ms = Some(result.response_ms);
            target.last_scrape_at = Some(result.timestamp);
            target.updated_at = result.timestamp;
            if result.live && result.metrics_ok {
                target.consecutive_failures = 0;
                target.last_success_at = Some(result.timestamp);
                target.last_error = None;
            } else {
                target.consecutive_failures += 1;
                target.scrape_failures_total += 1;
                target.last_error = result.error.clone().or_else(|| Some("liveness or metrics scrape failed".to_string()));
            }
            target.sample_count += result.metrics.len() as u64;
        }
        let mut labels = BTreeMap::from([
            ("target_id".to_string(), result.target_id.clone()),
            ("service".to_string(), service.clone()),
        ]);
        insert_point(&mut state, &self.config, &result.target_id, &service, MetricPointInput {
            name: "netcore_observability_target_up".to_string(), labels: labels.clone(), value: if result.live { 1.0 } else { 0.0 }, timestamp: result.timestamp,
        });
        insert_point(&mut state, &self.config, &result.target_id, &service, MetricPointInput {
            name: "netcore_observability_target_ready".to_string(), labels: labels.clone(), value: if result.ready { 1.0 } else { 0.0 }, timestamp: result.timestamp,
        });
        let failures = state.targets.get(&result.target_id).map(|target| target.consecutive_failures).unwrap_or(0);
        let failures_total = state.targets.get(&result.target_id).map(|target| target.scrape_failures_total).unwrap_or(0);
        insert_point(&mut state, &self.config, &result.target_id, &service, MetricPointInput {
            name: "netcore_observability_target_consecutive_failures".to_string(), labels: labels.clone(), value: failures as f64, timestamp: result.timestamp,
        });
        insert_point(&mut state, &self.config, &result.target_id, &service, MetricPointInput {
            name: "netcore_observability_target_scrape_failures_total".to_string(), labels: labels.clone(), value: failures_total as f64, timestamp: result.timestamp,
        });
        labels.insert("unit".to_string(), "milliseconds".to_string());
        insert_point(&mut state, &self.config, &result.target_id, &service, MetricPointInput {
            name: "netcore_observability_target_response_ms".to_string(), labels, value: result.response_ms, timestamp: result.timestamp,
        });
        for point in result.metrics { insert_point(&mut state, &self.config, &result.target_id, &service, point); }
        let count = state.series.values().filter(|series| series.target_id == result.target_id).count();
        if let Some(target) = state.targets.get_mut(&result.target_id) { target.series_count = count; }
        evaluate_rules(&mut state);
        trim_limits(&mut state, &self.config);
        drop(state);
        self.persist()
    }

    pub fn record_stack_probe(&self, probe: StackProbe) {
        let mut state = self.lock();
        let component = state.stack.entry(probe.component.clone()).or_insert(StackComponent {
            component: probe.component.clone(), endpoint: probe.endpoint.clone(), ready: false,
            response_ms: None, checked_at: None, last_error: None,
        });
        component.endpoint = probe.endpoint;
        component.ready = probe.ready;
        component.response_ms = Some(probe.response_ms);
        component.checked_at = Some(probe.checked_at);
        component.last_error = probe.last_error;
    }

    pub fn stack(&self) -> Vec<StackComponent> {
        let state = self.lock();
        let mut values: Vec<_> = state.stack.values().cloned().collect();
        values.sort_by(|a, b| a.component.cmp(&b.component));
        values
    }

    pub fn metric_catalog(&self) -> Vec<Value> {
        let state = self.lock();
        let mut by_name: BTreeMap<String, (usize, DateTime<Utc>)> = BTreeMap::new();
        for series in state.series.values() {
            let entry = by_name.entry(series.name.clone()).or_insert((0, series.last_at));
            entry.0 += 1;
            if series.last_at > entry.1 { entry.1 = series.last_at; }
        }
        by_name.into_iter().map(|(name, (series, last_at))| json!({"name":name,"series":series,"last_at":last_at})).collect()
    }

    pub fn series(&self, metric: Option<&str>, target_id: Option<&str>, service: Option<&str>, limit: usize) -> Vec<MetricSeries> {
        let state = self.lock();
        let mut values: Vec<_> = state.series.values().filter(|series| {
            metric.map(|value| series.name == value).unwrap_or(true)
                && target_id.map(|value| series.target_id == value).unwrap_or(true)
                && service.map(|value| series.service == value).unwrap_or(true)
        }).cloned().collect();
        values.sort_by(|a, b| b.last_at.cmp(&a.last_at));
        values.truncate(limit);
        values
    }

    pub fn rules(&self) -> Vec<AlertRuleRecord> {
        let state = self.lock();
        let mut values: Vec<_> = state.rules.values().cloned().collect();
        values.sort_by(|a, b| a.rule_id.cmp(&b.rule_id)); values
    }

    pub fn create_rule(&self, input: RuleInput) -> Result<AlertRuleRecord, String> {
        validate_rule_input(&input)?;
        let mut state = self.lock();
        let id = input.rule_id.trim().to_ascii_lowercase();
        if state.rules.contains_key(&id) { return Err(format!("rule {id} already exists")); }
        let now = Utc::now();
        let record = rule_from_input(id.clone(), input, now);
        state.rules.insert(id.clone(), record.clone());
        audit(&mut state, "open-lab".to_string(), "alerting", "rule.create", "rule", &id, "success", json!({"metric":record.metric}));
        evaluate_rules(&mut state);
        drop(state); self.persist().map_err(|error| error.to_string())?; Ok(record)
    }

    pub fn update_rule(&self, rule_id: &str, input: RuleInput) -> Result<AlertRuleRecord, String> {
        validate_rule_input(&input)?;
        let mut state = self.lock();
        let created_at = state.rules.get(rule_id).ok_or_else(|| format!("rule {rule_id} not found"))?.created_at;
        let mut record = rule_from_input(rule_id.to_string(), input, Utc::now());
        record.created_at = created_at;
        state.rules.insert(rule_id.to_string(), record.clone());
        audit(&mut state, "open-lab".to_string(), "alerting", "rule.update", "rule", rule_id, "success", json!({"metric":record.metric}));
        evaluate_rules(&mut state);
        drop(state); self.persist().map_err(|error| error.to_string())?; Ok(record)
    }

    pub fn rule_action(&self, rule_id: &str, action: &str, input: ActionInput) -> Result<AlertRuleRecord, String> {
        let mut state = self.lock();
        let record = state.rules.get_mut(rule_id).ok_or_else(|| format!("rule {rule_id} not found"))?;
        match action { "enable" => record.enabled = true, "disable" => record.enabled = false, _ => return Err(format!("unsupported rule action {action}")) }
        record.updated_at = Utc::now();
        let output = record.clone();
        audit(&mut state, actor(input.actor), "alerting", &format!("rule.{action}"), "rule", rule_id, "success", json!({"reason":input.reason}));
        evaluate_rules(&mut state);
        drop(state); self.persist().map_err(|error| error.to_string())?; Ok(output)
    }

    pub fn delete_rule(&self, rule_id: &str, input: ActionInput) -> Result<(), String> {
        let mut state = self.lock();
        if state.rules.remove(rule_id).is_none() { return Err(format!("rule {rule_id} not found")); }
        let now = Utc::now();
        for alert in state.alerts.values_mut().filter(|alert| alert.rule_id == rule_id && alert.state != "resolved") {
            alert.state = "resolved".to_string(); alert.resolved_at = Some(now); alert.updated_at = now;
        }
        audit(&mut state, actor(input.actor), "alerting", "rule.delete", "rule", rule_id, "success", json!({"reason":input.reason}));
        drop(state); self.persist().map_err(|error| error.to_string())
    }

    pub fn alerts(&self, state_filter: Option<&str>, severity: Option<&str>, service: Option<&str>, limit: usize) -> Vec<AlertRecord> {
        let state = self.lock();
        let mut values: Vec<_> = state.alerts.values().filter(|alert| {
            state_filter.map(|value| alert.state == value).unwrap_or(true)
                && severity.map(|value| alert.severity == value).unwrap_or(true)
                && service.map(|value| alert.service == value).unwrap_or(true)
        }).cloned().collect();
        values.sort_by(|a, b| b.updated_at.cmp(&a.updated_at)); values.truncate(limit); values
    }

    pub fn alert_action(&self, alert_id: &str, action: &str, input: ActionInput) -> Result<AlertRecord, String> {
        let mut state = self.lock();
        let now = Utc::now();
        let alert = state.alerts.get_mut(alert_id).ok_or_else(|| format!("alert {alert_id} not found"))?;
        match action {
            "acknowledge" => { alert.acknowledged = true; alert.acknowledged_at = Some(now); alert.acknowledged_by = Some(actor(input.actor.clone())); alert.acknowledgement_note = input.reason.clone(); }
            "unacknowledge" => { alert.acknowledged = false; alert.acknowledged_at = None; alert.acknowledged_by = None; alert.acknowledgement_note = None; }
            "resolve" => { alert.state = "resolved".to_string(); alert.resolved_at = Some(now); }
            _ => return Err(format!("unsupported alert action {action}")),
        }
        alert.updated_at = now;
        let output = alert.clone();
        audit(&mut state, actor(input.actor), "alerting", &format!("alert.{action}"), "alert", alert_id, "success", json!({"reason":input.reason}));
        drop(state); self.persist().map_err(|error| error.to_string())?; Ok(output)
    }

    pub fn silences(&self) -> Vec<SilenceRecord> {
        let state = self.lock(); let mut values: Vec<_> = state.silences.values().cloned().collect();
        values.sort_by(|a, b| b.created_at.cmp(&a.created_at)); values
    }

    pub fn create_silence(&self, input: SilenceInput) -> Result<SilenceRecord, String> {
        if input.duration_secs == 0 || input.duration_secs > 365 * 24 * 3600 { return Err("duration_secs must be between 1 and 31536000".to_string()); }
        if input.comment.trim().is_empty() { return Err("silence comment must not be empty".to_string()); }
        let mut state = self.lock(); let now = Utc::now(); let id = Uuid::new_v4().to_string();
        let record = SilenceRecord { silence_id: id.clone(), comment: input.comment, created_by: actor(input.created_by), rule_id: input.rule_id, service: input.service, target_id: input.target_id, severity: input.severity, match_labels: input.match_labels, starts_at: now, ends_at: now + Duration::seconds(input.duration_secs as i64), active: true, created_at: now, expired_at: None };
        state.silences.insert(id.clone(), record.clone());
        audit(&mut state, record.created_by.clone(), "alerting", "silence.create", "silence", &id, "success", json!({"ends_at":record.ends_at,"comment":record.comment}));
        evaluate_rules(&mut state);
        drop(state); self.persist().map_err(|error| error.to_string())?; Ok(record)
    }

    pub fn expire_silence(&self, silence_id: &str, input: ActionInput) -> Result<SilenceRecord, String> {
        let mut state = self.lock(); let now = Utc::now();
        let silence = state.silences.get_mut(silence_id).ok_or_else(|| format!("silence {silence_id} not found"))?;
        silence.active = false; silence.expired_at = Some(now); silence.ends_at = now; let output = silence.clone();
        audit(&mut state, actor(input.actor), "alerting", "silence.expire", "silence", silence_id, "success", json!({"reason":input.reason}));
        evaluate_rules(&mut state);
        drop(state); self.persist().map_err(|error| error.to_string())?; Ok(output)
    }

    pub fn ingest_logs(&self, input: LogIngestInput) -> Result<usize, String> {
        if !self.config.collection.ingest_logs { return Err("log ingestion is disabled".to_string()); }
        let mut state = self.lock(); let now = Utc::now(); let mut accepted = 0usize;
        for item in input.records {
            if item.service.trim().is_empty() || item.message.trim().is_empty() { continue; }
            let timestamp = item.timestamp.as_deref().and_then(parse_datetime).unwrap_or(now);
            state.logs.push(LogRecord { log_id: Uuid::new_v4().to_string(), timestamp, received_at: now, service: item.service, node: item.node, level: item.level.unwrap_or_else(|| "info".to_string()).to_ascii_lowercase(), message: item.message, correlation_id: item.correlation_id, trace_id: item.trace_id, fields: item.fields }); accepted += 1;
        }
        trim_limits(&mut state, &self.config);
        if accepted > 0 { audit(&mut state, "ingest".to_string(), "logs", "logs.ingest", "batch", &Uuid::new_v4().to_string(), "success", json!({"accepted":accepted})); }
        drop(state); self.persist().map_err(|error| error.to_string())?; Ok(accepted)
    }

    pub fn logs(&self, service: Option<&str>, level: Option<&str>, contains: Option<&str>, trace_id: Option<&str>, limit: usize) -> Vec<LogRecord> {
        let state = self.lock(); let needle = contains.map(|value| value.to_ascii_lowercase());
        let mut values: Vec<_> = state.logs.iter().filter(|record| {
            service.map(|value| record.service == value).unwrap_or(true)
                && level.map(|value| record.level == value).unwrap_or(true)
                && trace_id.map(|value| record.trace_id.as_deref() == Some(value)).unwrap_or(true)
                && needle.as_ref().map(|value| record.message.to_ascii_lowercase().contains(value)).unwrap_or(true)
        }).cloned().collect();
        values.sort_by(|a, b| b.timestamp.cmp(&a.timestamp)); values.truncate(limit); values
    }

    pub fn ingest_traces(&self, input: TraceIngestInput) -> Result<usize, String> {
        if !self.config.collection.ingest_traces { return Err("trace ingestion is disabled".to_string()); }
        let mut state = self.lock(); let now = Utc::now(); let mut accepted = 0usize;
        for item in input.spans {
            if item.trace_id.trim().is_empty() || item.span_id.trim().is_empty() || item.service.trim().is_empty() { continue; }
            let started_at = item.started_at.as_deref().and_then(parse_datetime).unwrap_or(now);
            state.traces.push(TraceSpan { trace_id: item.trace_id, span_id: item.span_id, parent_span_id: item.parent_span_id, service: item.service, operation: item.operation, started_at, received_at: now, duration_ms: item.duration_ms.max(0.0), status: item.status.unwrap_or_else(|| "ok".to_string()), attributes: item.attributes }); accepted += 1;
        }
        trim_limits(&mut state, &self.config);
        if accepted > 0 { audit(&mut state, "ingest".to_string(), "traces", "traces.ingest", "batch", &Uuid::new_v4().to_string(), "success", json!({"accepted":accepted})); }
        drop(state); self.persist().map_err(|error| error.to_string())?; Ok(accepted)
    }

    pub fn traces(&self, service: Option<&str>, trace_id: Option<&str>, status: Option<&str>, limit: usize) -> Vec<TraceSpan> {
        let state = self.lock(); let mut values: Vec<_> = state.traces.iter().filter(|span| {
            service.map(|value| span.service == value).unwrap_or(true)
                && trace_id.map(|value| span.trace_id == value).unwrap_or(true)
                && status.map(|value| span.status == value).unwrap_or(true)
        }).cloned().collect();
        values.sort_by(|a, b| b.started_at.cmp(&a.started_at)); values.truncate(limit); values
    }

    pub fn audit(&self, limit: usize) -> Vec<AuditRecord> {
        let state = self.lock(); let mut values = state.audit.clone(); values.sort_by(|a,b| b.sequence.cmp(&a.sequence)); values.truncate(limit); values
    }

    pub fn diagnostics(&self) -> Vec<DiagnosticRecord> {
        let state = self.lock(); let mut values: Vec<_> = state.diagnostics.values().cloned().collect(); values.sort_by(|a,b| b.created_at.cmp(&a.created_at)); values
    }

    pub fn create_diagnostic(&self, input: DiagnosticInput) -> Result<DiagnosticRecord, String> {
        let id = Uuid::new_v4().to_string(); let now = Utc::now(); let actor_name = actor(input.actor.clone());
        {
            let mut state = self.lock();
            state.diagnostics.insert(id.clone(), DiagnosticRecord { diagnostic_id: id.clone(), state: "building".to_string(), reason: input.reason.clone(), created_by: actor_name.clone(), created_at: now, completed_at: None, archive_path: None, sha256: None, size_bytes: None, last_error: None });
            audit(&mut state, actor_name.clone(), "maintenance", "diagnostic.create", "diagnostic", &id, "started", json!({"reason":input.reason}));
        }
        let result = self.build_diagnostic(&id, &input);
        let mut state = self.lock();
        let record = state.diagnostics.get_mut(&id).ok_or_else(|| "diagnostic record disappeared".to_string())?;
        match result {
            Ok((path, sha256, size)) => { record.state = "ready".to_string(); record.archive_path = Some(path); record.sha256 = Some(sha256); record.size_bytes = Some(size); record.completed_at = Some(Utc::now()); }
            Err(error) => { record.state = "failed".to_string(); record.last_error = Some(error.clone()); record.completed_at = Some(Utc::now()); }
        }
        let output = record.clone();
        let final_state = output.state.clone();
        audit(&mut state, actor_name, "maintenance", "diagnostic.complete", "diagnostic", &id, &final_state, json!({"sha256":output.sha256,"error":output.last_error}));
        drop(state); self.persist().map_err(|error| error.to_string())?; Ok(output)
    }

    fn build_diagnostic(&self, id: &str, input: &DiagnosticInput) -> Result<(PathBuf, String, u64), String> {
        fs::create_dir_all(&self.config.storage.diagnostic_dir).map_err(|error| error.to_string())?;
        let max_records = input.max_records.unwrap_or(1_000).clamp(10, 10_000);
        let state = self.lock();
        let payload = json!({
            "format":"netcore-observability-diagnostic-v1",
            "created_at":Utc::now(),
            "status":self.status_from_state(&state),
            "targets":state.targets,
            "stack":state.stack,
            "rules":state.rules,
            "alerts":state.alerts,
            "silences":state.silences,
            "metric_catalog":metric_catalog_from_state(&state),
            "logs":if input.include_logs.unwrap_or(true) { state.logs.iter().rev().take(max_records).cloned().collect::<Vec<_>>() } else { Vec::new() },
            "traces":if input.include_traces.unwrap_or(true) { state.traces.iter().rev().take(max_records).cloned().collect::<Vec<_>>() } else { Vec::new() },
            "audit":state.audit.iter().rev().take(max_records).cloned().collect::<Vec<_>>(),
            "configuration":self.config,
        });
        drop(state);
        let json_name = format!("diagnostic-{id}.json"); let sha_name = format!("diagnostic-{id}.json.sha256"); let archive_name = format!("diagnostic-{id}.tar.gz");
        let json_path = self.config.storage.diagnostic_dir.join(&json_name); let sha_path = self.config.storage.diagnostic_dir.join(&sha_name); let archive_path = self.config.storage.diagnostic_dir.join(&archive_name);
        fs::write(&json_path, serde_json::to_vec_pretty(&payload).map_err(|error| error.to_string())?).map_err(|error| error.to_string())?;
        let output = Command::new("sha256sum").arg(&json_path).output().map_err(|error| format!("sha256sum failed: {error}"))?;
        if !output.status.success() { return Err("sha256sum returned failure".to_string()); }
        let checksum_line = String::from_utf8(output.stdout).map_err(|error| error.to_string())?;
        let sha256 = checksum_line.split_whitespace().next().ok_or_else(|| "sha256sum output missing checksum".to_string())?.to_string();
        fs::write(&sha_path, format!("{sha256}  {json_name}\n")).map_err(|error| error.to_string())?;
        let status = Command::new("tar").arg("-czf").arg(&archive_path).arg("-C").arg(&self.config.storage.diagnostic_dir).arg(&json_name).arg(&sha_name).status().map_err(|error| format!("tar failed: {error}"))?;
        if !status.success() { return Err("tar returned failure".to_string()); }
        let size = fs::metadata(&archive_path).map_err(|error| error.to_string())?.len();
        Ok((archive_path, sha256, size))
    }

    pub fn diagnostic_file(&self, diagnostic_id: &str) -> Result<(String, Vec<u8>), String> {
        let state = self.lock(); let record = state.diagnostics.get(diagnostic_id).ok_or_else(|| format!("diagnostic {diagnostic_id} not found"))?;
        let path = record.archive_path.as_ref().ok_or_else(|| "diagnostic is not ready".to_string())?;
        let name = path.file_name().and_then(|value| value.to_str()).unwrap_or("diagnostic.tar.gz").to_string();
        fs::read(path).map(|bytes| (name, bytes)).map_err(|error| error.to_string())
    }

    pub fn maintenance(&self, actor_name: Option<String>) -> Result<ObservabilityStatus, Box<dyn std::error::Error>> {
        let mut state = self.lock(); let now = Utc::now();
        let metric_cutoff = now - Duration::seconds(self.config.retention.metric_retention_secs as i64);
        let log_cutoff = now - Duration::seconds(self.config.retention.log_retention_secs as i64);
        let trace_cutoff = now - Duration::seconds(self.config.retention.trace_retention_secs as i64);
        let audit_cutoff = now - Duration::seconds(self.config.retention.audit_retention_secs as i64);
        for series in state.series.values_mut() { series.samples.retain(|sample| sample.timestamp >= metric_cutoff); }
        state.series.retain(|_, series| !series.samples.is_empty());
        state.logs.retain(|record| record.timestamp >= log_cutoff);
        state.traces.retain(|span| span.started_at >= trace_cutoff);
        state.audit.retain(|record| record.timestamp >= audit_cutoff);
        for silence in state.silences.values_mut() {
            if silence.active && silence.ends_at <= now { silence.active = false; silence.expired_at = Some(now); }
        }
        evaluate_rules(&mut state); trim_limits(&mut state, &self.config); state.last_maintenance_at = Some(now);
        if let Some(actor_name) = actor_name { audit(&mut state, actor_name, "maintenance", "retention.run", "service", "observability", "success", json!({"at":now})); }
        let status = self.status_from_state(&state); drop(state); self.persist()?; Ok(status)
    }

    pub fn backup(&self, actor_name: Option<String>) -> Result<Value, String> {
        self.persist().map_err(|error| error.to_string())?;
        if let Some(parent) = self.config.storage.backup_path.parent() { fs::create_dir_all(parent).map_err(|error| error.to_string())?; }
        fs::copy(&self.config.storage.state_path, &self.config.storage.backup_path).map_err(|error| error.to_string())?;
        let size = fs::metadata(&self.config.storage.backup_path).map_err(|error| error.to_string())?.len();
        let mut state = self.lock(); audit(&mut state, actor(actor_name), "maintenance", "backup.create", "backup", &self.config.storage.backup_path.display().to_string(), "success", json!({"size_bytes":size})); drop(state);
        self.persist().map_err(|error| error.to_string())?;
        Ok(json!({"path":self.config.storage.backup_path,"size_bytes":size,"created_at":Utc::now()}))
    }

    pub fn export(&self) -> Value {
        let state = self.lock();
        json!({"format":"netcore-observability-export-v1","exported_at":Utc::now(),"status":self.status_from_state(&state),"targets":state.targets,"stack":state.stack,"rules":state.rules,"alerts":state.alerts,"silences":state.silences,"metric_catalog":metric_catalog_from_state(&state),"diagnostics":state.diagnostics,"audit":state.audit})
    }

    pub fn metrics(&self) -> String {
        let state = self.lock(); let status = self.status_from_state(&state); let mut output = String::new();
        output.push_str("# HELP netcore_observability_ready Whether the NMS management plane is ready\n# TYPE netcore_observability_ready gauge\n");
        output.push_str(&format!("netcore_observability_ready {}\n", if status.ready {1} else {0}));
        for (name, value) in [
            ("netcore_observability_targets_total", status.targets_total as f64),
            ("netcore_observability_targets_up", status.targets_up as f64),
            ("netcore_observability_targets_ready", status.targets_ready as f64),
            ("netcore_observability_series", status.series as f64),
            ("netcore_observability_samples", status.samples as f64),
            ("netcore_observability_logs", status.logs as f64),
            ("netcore_observability_traces", status.traces as f64),
            ("netcore_observability_alerts_firing", status.alerts_firing as f64),
            ("netcore_observability_alerts_unacknowledged", status.alerts_unacknowledged as f64),
            ("netcore_observability_active_silences", status.active_silences as f64),
        ] { output.push_str(&format!("# TYPE {name} gauge\n{name} {value}\n")); }
        for target in state.targets.values() {
            output.push_str(&format!("netcore_observability_target_up{{target_id=\"{}\",service=\"{}\"}} {}\n", escape_label(&target.target_id), escape_label(&target.service), if target.live {1} else {0}));
            output.push_str(&format!("netcore_observability_target_ready{{target_id=\"{}\",service=\"{}\"}} {}\n", escape_label(&target.target_id), escape_label(&target.service), if target.ready {1} else {0}));
            output.push_str(&format!("netcore_observability_target_consecutive_failures{{target_id=\"{}\",service=\"{}\"}} {}\n", escape_label(&target.target_id), escape_label(&target.service), target.consecutive_failures));
        }
        output
    }

    fn status_from_state(&self, state: &PersistentState) -> ObservabilityStatus {
        let enabled: Vec<&TargetRecord> = state.targets.values().filter(|target| target.enabled).collect();
        let firing: Vec<&AlertRecord> = state.alerts.values().filter(|alert| alert.state == "firing").collect();
        ObservabilityStatus { ready: true, security_mode: self.config.security.mode.clone(), warning: self.config.security.warning_banner.clone(), started_at: state.started_at, targets_total: enabled.len(), targets_up: enabled.iter().filter(|target| target.live).count(), targets_ready: enabled.iter().filter(|target| target.ready).count(), series: state.series.len(), samples: state.series.values().map(|series| series.samples.len()).sum(), logs: state.logs.len(), traces: state.traces.len(), alerts_firing: firing.len(), alerts_pending: state.alerts.values().filter(|alert| alert.state == "pending").count(), alerts_unacknowledged: firing.iter().filter(|alert| !alert.acknowledged && !alert.silenced).count(), active_silences: state.silences.values().filter(|silence| silence.active).count(), stack_ready: state.stack.values().filter(|component| component.ready).count(), stack_total: state.stack.len(), last_maintenance_at: state.last_maintenance_at }
    }

    fn persist(&self) -> Result<(), Box<dyn std::error::Error>> {
        let state = self.lock(); let bytes = serde_json::to_vec_pretty(&*state)?; drop(state);
        write_atomic(&self.config.storage.state_path, &bytes)?; Ok(())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, PersistentState> {
        self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn validate_target_input(input: &TargetCreateInput) -> Result<(), String> {
    if input.target_id.trim().is_empty() || input.display_name.trim().is_empty() || input.service.trim().is_empty() { return Err("target_id, display_name and service are required".to_string()); }
    if !input.base_url.starts_with("http://") { return Err("base_url must use http:// in open_lab".to_string()); }
    for path in [input.metrics_path.as_deref(), input.live_path.as_deref(), input.ready_path.as_deref()].into_iter().flatten() { if !path.starts_with('/') { return Err("target paths must start with /".to_string()); } }
    Ok(())
}

fn validate_rule_input(input: &RuleInput) -> Result<(), String> {
    if input.rule_id.trim().is_empty() || input.name.trim().is_empty() || input.metric.trim().is_empty() { return Err("rule_id, name and metric are required".to_string()); }
    if !matches!(input.comparator.as_str(), ">" | ">=" | "<" | "<=" | "==" | "!=") { return Err("comparator must be one of > >= < <= == !=".to_string()); }
    if !input.threshold.is_finite() { return Err("threshold must be finite".to_string()); }
    Ok(())
}

fn rule_from_input(rule_id: String, input: RuleInput, now: DateTime<Utc>) -> AlertRuleRecord {
    AlertRuleRecord { rule_id, name: input.name, description: input.description.unwrap_or_default(), metric: input.metric, comparator: input.comparator, threshold: input.threshold, for_secs: input.for_secs.unwrap_or(0), severity: input.severity.to_ascii_lowercase(), service: input.service, target_id: input.target_id, enabled: input.enabled.unwrap_or(true), labels: input.labels, annotations: input.annotations, created_at: now, updated_at: now }
}

fn insert_point(state: &mut PersistentState, config: &ObservabilityConfig, target_id: &str, service: &str, point: MetricPointInput) {
    let series_id = series_id(target_id, &point.name, &point.labels);
    if !state.series.contains_key(&series_id) && state.series.len() >= config.retention.max_series { return; }
    let series = state.series.entry(series_id.clone()).or_insert(MetricSeries { series_id, target_id: target_id.to_string(), service: service.to_string(), name: point.name, labels: point.labels, samples: Vec::new(), last_value: point.value, last_at: point.timestamp });
    series.last_value = point.value; series.last_at = point.timestamp; series.samples.push(MetricSample { timestamp: point.timestamp, value: point.value });
    if series.samples.len() > config.retention.max_samples_per_series { let remove = series.samples.len() - config.retention.max_samples_per_series; series.samples.drain(0..remove); }
}

fn series_id(target_id: &str, name: &str, labels: &BTreeMap<String, String>) -> String {
    let labels = labels.iter().map(|(key,value)| format!("{key}={value}")).collect::<Vec<_>>().join(",");
    format!("{target_id}|{name}|{labels}")
}

fn evaluate_rules(state: &mut PersistentState) {
    let now = Utc::now(); let rules: Vec<_> = state.rules.values().filter(|rule| rule.enabled).cloned().collect(); let series: Vec<_> = state.series.values().cloned().collect();
    for rule in rules {
        let matching: Vec<_> = series.iter().filter(|item| item.name == rule.metric && rule.service.as_deref().map(|value| item.service == value).unwrap_or(true) && rule.target_id.as_deref().map(|value| item.target_id == value).unwrap_or(true)).cloned().collect();
        for item in matching {
            let fingerprint = format!("{}|{}", rule.rule_id, item.series_id);
            let condition = compare(item.last_value, &rule.comparator, rule.threshold);
            let active_id = state.alerts.values().find(|alert| alert.fingerprint == fingerprint && alert.state != "resolved").map(|alert| alert.alert_id.clone());
            if condition {
                if let Some(alert_id) = active_id {
                    if let Some(alert) = state.alerts.get_mut(&alert_id) {
                        alert.value = item.last_value; alert.updated_at = now;
                        if alert.state == "pending" && (now - alert.pending_since).num_seconds() >= rule.for_secs as i64 { alert.state = "firing".to_string(); alert.fired_at = Some(now); }
                    }
                    refresh_silence(state, &alert_id);
                } else {
                    let id = Uuid::new_v4().to_string(); let firing = rule.for_secs == 0;
                    let mut labels = item.labels.clone(); for (key,value) in &rule.labels { labels.insert(key.clone(), value.clone()); }
                    let service = item.labels.get("service").cloned().unwrap_or_else(|| item.service.clone());
                    let target_id = item.labels.get("target_id").cloned().unwrap_or_else(|| item.target_id.clone());
                    let record = AlertRecord { alert_id: id.clone(), fingerprint, rule_id: rule.rule_id.clone(), rule_name: rule.name.clone(), severity: rule.severity.clone(), state: if firing {"firing".to_string()} else {"pending".to_string()}, service, target_id, metric: rule.metric.clone(), series_id: item.series_id.clone(), value: item.last_value, comparator: rule.comparator.clone(), threshold: rule.threshold, summary: format!("{}: {} {} {}", rule.name, item.last_value, rule.comparator, rule.threshold), labels, annotations: rule.annotations.clone(), first_seen_at: now, pending_since: now, fired_at: if firing {Some(now)} else {None}, resolved_at: None, acknowledged: false, acknowledged_at: None, acknowledged_by: None, acknowledgement_note: None, silenced: false, silence_ids: Vec::new(), updated_at: now };
                    state.alerts.insert(id.clone(), record); refresh_silence(state, &id);
                }
            } else if let Some(alert_id) = active_id {
                if let Some(alert) = state.alerts.get_mut(&alert_id) { alert.state = "resolved".to_string(); alert.resolved_at = Some(now); alert.updated_at = now; alert.silenced = false; alert.silence_ids.clear(); }
            }
        }
    }
}

fn refresh_silence(state: &mut PersistentState, alert_id: &str) {
    let Some(snapshot) = state.alerts.get(alert_id).cloned() else { return; };
    let ids: Vec<String> = state.silences.values().filter(|silence| silence_matches(silence, &snapshot)).map(|silence| silence.silence_id.clone()).collect();
    if let Some(alert) = state.alerts.get_mut(alert_id) { alert.silenced = !ids.is_empty(); alert.silence_ids = ids; }
}

fn silence_matches(silence: &SilenceRecord, alert: &AlertRecord) -> bool {
    if !silence.active || silence.starts_at > Utc::now() || silence.ends_at <= Utc::now() { return false; }
    if silence.rule_id.as_deref().map(|value| alert.rule_id == value).unwrap_or(true)
        && silence.service.as_deref().map(|value| alert.service == value).unwrap_or(true)
        && silence.target_id.as_deref().map(|value| alert.target_id == value).unwrap_or(true)
        && silence.severity.as_deref().map(|value| alert.severity == value).unwrap_or(true) {
        return silence.match_labels.iter().all(|(key,value)| alert.labels.get(key) == Some(value));
    }
    false
}

fn compare(value: f64, comparator: &str, threshold: f64) -> bool {
    match comparator { ">" => value > threshold, ">=" => value >= threshold, "<" => value < threshold, "<=" => value <= threshold, "==" => (value-threshold).abs() < f64::EPSILON, "!=" => (value-threshold).abs() >= f64::EPSILON, _ => false }
}

fn trim_limits(state: &mut PersistentState, config: &ObservabilityConfig) {
    trim_front(&mut state.logs, config.retention.max_logs); trim_front(&mut state.traces, config.retention.max_spans); trim_front(&mut state.audit, config.retention.max_audit_records);
    if state.alerts.len() > config.retention.max_alerts {
        let mut ids: Vec<_> = state.alerts.values().filter(|alert| alert.state == "resolved").map(|alert| (alert.updated_at, alert.alert_id.clone())).collect(); ids.sort_by_key(|value| value.0);
        for (_, id) in ids.into_iter().take(state.alerts.len() - config.retention.max_alerts) { state.alerts.remove(&id); }
    }
}

fn trim_front<T>(values: &mut Vec<T>, max: usize) { if values.len() > max { let remove = values.len() - max; values.drain(0..remove); } }

fn audit(state: &mut PersistentState, actor: String, category: &str, action: &str, object_type: &str, object_id: &str, result: &str, detail: Value) {
    state.sequence += 1; state.audit.push(AuditRecord { sequence: state.sequence, timestamp: Utc::now(), actor, category: category.to_string(), action: action.to_string(), object_type: object_type.to_string(), object_id: object_id.to_string(), result: result.to_string(), detail });
}

fn actor(value: Option<String>) -> String { value.filter(|value| !value.trim().is_empty()).unwrap_or_else(|| "open-lab".to_string()) }
fn parse_datetime(value: &str) -> Option<DateTime<Utc>> { DateTime::parse_from_rfc3339(value).ok().map(|value| value.with_timezone(&Utc)) }
fn escape_label(value: &str) -> String { value.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n") }

fn metric_catalog_from_state(state: &PersistentState) -> Vec<Value> {
    let mut by_name: BTreeMap<String, (usize, DateTime<Utc>)> = BTreeMap::new(); for series in state.series.values() { let entry = by_name.entry(series.name.clone()).or_insert((0,series.last_at)); entry.0 += 1; if series.last_at > entry.1 { entry.1 = series.last_at; } }
    by_name.into_iter().map(|(name,(series,last_at))| json!({"name":name,"series":series,"last_at":last_at})).collect()
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
    let temporary = path.with_extension(format!("tmp-{}", Uuid::new_v4())); fs::write(&temporary, bytes)?; fs::rename(temporary, path)?; Ok(())
}

use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream, ToSocketAddrs};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::config::{CoreServiceConfig, FederationConfig, OperationsConfig};
use crate::state::now_iso;

const STATE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServiceSnapshot {
    pub name: String,
    pub display_name: String,
    pub kind: String,
    pub base_url: String,
    pub webui_url: String,
    pub critical: bool,
    pub enabled: bool,
    pub status: String,
    pub live: Option<bool>,
    pub ready: Option<bool>,
    pub checked_at: Option<String>,
    pub last_success_at: Option<String>,
    pub latency_ms: Option<u64>,
    pub consecutive_failures: u32,
    pub http_status: Option<u16>,
    pub message: Option<String>,
    pub health: Option<Value>,
    pub readiness: Option<Value>,
    pub summary: Option<Value>,
}

impl Default for ServiceSnapshot {
    fn default() -> Self {
        Self {
            name: String::new(),
            display_name: String::new(),
            kind: "core".to_string(),
            base_url: String::new(),
            webui_url: String::new(),
            critical: false,
            enabled: true,
            status: "unknown".to_string(),
            live: None,
            ready: None,
            checked_at: None,
            last_success_at: None,
            latency_ms: None,
            consecutive_failures: 0,
            http_status: None,
            message: None,
            health: None,
            readiness: None,
            summary: None,
        }
    }
}

impl ServiceSnapshot {
    fn from_config(config: &CoreServiceConfig) -> Self {
        Self {
            name: config.name.clone(),
            display_name: config.display_name.clone(),
            kind: config.kind.clone(),
            base_url: config.base_url.clone(),
            webui_url: join_url(&config.base_url, &config.webui_path),
            critical: config.critical,
            enabled: config.enabled,
            status: if config.enabled { "unknown" } else { "disabled" }.to_string(),
            ..Self::default()
        }
    }

    fn apply_config(&mut self, config: &CoreServiceConfig) {
        self.display_name = config.display_name.clone();
        self.kind = config.kind.clone();
        self.base_url = config.base_url.clone();
        self.webui_url = join_url(&config.base_url, &config.webui_path);
        self.critical = config.critical;
        if !self.enabled {
            self.status = "disabled".to_string();
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentNote {
    pub id: String,
    pub timestamp: String,
    pub operator_id: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentRecord {
    pub id: String,
    pub source_key: Option<String>,
    pub severity: String,
    pub status: String,
    pub title: String,
    pub description: String,
    pub service: Option<String>,
    pub node_id: Option<String>,
    pub issi: Option<u32>,
    pub gssi: Option<u32>,
    pub created_at: String,
    pub created_by: String,
    pub acknowledged_at: Option<String>,
    pub acknowledged_by: Option<String>,
    pub resolved_at: Option<String>,
    pub resolved_by: Option<String>,
    pub notes: Vec<IncidentNote>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShiftLogEntry {
    pub id: String,
    pub timestamp: String,
    pub operator_id: String,
    pub category: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct OperationsState {
    schema_version: u32,
    started_at: String,
    updated_at: String,
    services: BTreeMap<String, ServiceSnapshot>,
    incidents: Vec<IncidentRecord>,
    shift_log: Vec<ShiftLogEntry>,
    last_poll_started_at: Option<String>,
    last_poll_finished_at: Option<String>,
    poll_in_progress: bool,
    poll_count: u64,
}

impl Default for OperationsState {
    fn default() -> Self {
        let now = now_iso();
        Self {
            schema_version: STATE_SCHEMA_VERSION,
            started_at: now.clone(),
            updated_at: now,
            services: BTreeMap::new(),
            incidents: Vec::new(),
            shift_log: Vec::new(),
            last_poll_started_at: None,
            last_poll_finished_at: None,
            poll_in_progress: false,
            poll_count: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationsOverview {
    pub started_at: String,
    pub updated_at: String,
    pub last_poll_started_at: Option<String>,
    pub last_poll_finished_at: Option<String>,
    pub poll_in_progress: bool,
    pub poll_count: u64,
    pub services_total: usize,
    pub services_healthy: usize,
    pub services_degraded: usize,
    pub services_offline: usize,
    pub services_disabled: usize,
    pub critical_services_offline: usize,
    pub incidents_open: usize,
    pub incidents_acknowledged: usize,
    pub shift_log_entries: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateIncidentRequest {
    pub severity: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub operator_id: Option<String>,
    pub service: Option<String>,
    pub node_id: Option<String>,
    pub issi: Option<u32>,
    pub gssi: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct IncidentActionRequest {
    pub operator_id: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IncidentNoteRequest {
    pub operator_id: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ShiftLogRequest {
    pub operator_id: Option<String>,
    pub category: Option<String>,
    pub text: String,
}

#[derive(Clone)]
pub struct SharedOperations {
    inner: Arc<Mutex<OperationsState>>,
    federation: FederationConfig,
    operations: OperationsConfig,
    service_configs: Arc<Vec<CoreServiceConfig>>,
}

impl SharedOperations {
    pub fn load(
        federation: FederationConfig,
        operations: OperationsConfig,
        services: Vec<CoreServiceConfig>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut state = if operations.state_path.is_file() {
            let raw = fs::read_to_string(&operations.state_path)?;
            match serde_json::from_str::<OperationsState>(&raw) {
                Ok(mut state) => {
                    state.poll_in_progress = false;
                    state
                }
                Err(error) => {
                    tracing::warn!(path = %operations.state_path.display(), "Control Room operations state could not be parsed: {error}; starting clean");
                    OperationsState::default()
                }
            }
        } else {
            OperationsState::default()
        };

        let mut merged = BTreeMap::new();
        for service in &services {
            let mut snapshot = state
                .services
                .remove(&service.name)
                .unwrap_or_else(|| ServiceSnapshot::from_config(service));
            snapshot.apply_config(service);
            if !service.enabled && snapshot.enabled {
                snapshot.enabled = false;
                snapshot.status = "disabled".to_string();
            }
            merged.insert(service.name.clone(), snapshot);
        }
        state.services = merged;
        state.schema_version = STATE_SCHEMA_VERSION;
        state.updated_at = now_iso();

        let shared = Self {
            inner: Arc::new(Mutex::new(state)),
            federation,
            operations,
            service_configs: Arc::new(services),
        };
        shared.persist()?;
        Ok(shared)
    }

    pub fn start_poller(&self) {
        if !self.federation.enabled {
            tracing::warn!("Core-service federation polling disabled");
            return;
        }
        let shared = self.clone();
        let interval = self.federation.poll_interval_secs.max(1);
        let _ = thread::Builder::new()
            .name("control-room-core-poller".to_string())
            .spawn(move || {
                thread::sleep(Duration::from_millis(250));
                loop {
                    shared.poll_cycle();
                    thread::sleep(Duration::from_secs(interval));
                }
            });
    }

    pub fn trigger_poll(&self) -> bool {
        {
            let mut state = self.inner.lock().expect("operations state poisoned");
            if state.poll_in_progress {
                return false;
            }
            state.poll_in_progress = true;
            state.last_poll_started_at = Some(now_iso());
            state.updated_at = now_iso();
        }
        let shared = self.clone();
        let _ = thread::Builder::new()
            .name("control-room-manual-poll".to_string())
            .spawn(move || shared.poll_cycle_claimed());
        true
    }

    fn poll_cycle(&self) {
        {
            let mut state = self.inner.lock().expect("operations state poisoned");
            if state.poll_in_progress {
                return;
            }
            state.poll_in_progress = true;
            state.last_poll_started_at = Some(now_iso());
            state.updated_at = now_iso();
        }
        self.poll_cycle_claimed();
    }

    fn poll_cycle_claimed(&self) {
        let configs = self.service_configs.as_ref().clone();
        for config in configs {
            let enabled = {
                let state = self.inner.lock().expect("operations state poisoned");
                state
                    .services
                    .get(&config.name)
                    .map(|service| service.enabled)
                    .unwrap_or(config.enabled)
            };
            if !enabled {
                let mut state = self.inner.lock().expect("operations state poisoned");
                if let Some(service) = state.services.get_mut(&config.name) {
                    service.status = "disabled".to_string();
                    service.live = None;
                    service.ready = None;
                    service.checked_at = Some(now_iso());
                    service.message = Some("administratively disabled in Control Room".to_string());
                }
                continue;
            }
            let result = poll_service(&config, &self.federation);
            let mut state = self.inner.lock().expect("operations state poisoned");
            let snapshot_for_incident = {
                let snapshot = state
                    .services
                    .entry(config.name.clone())
                    .or_insert_with(|| ServiceSnapshot::from_config(&config));
                apply_poll_result(snapshot, result);
                snapshot.clone()
            };
            if self.operations.auto_service_incidents {
                reconcile_service_incident(
                    &mut state.incidents,
                    &snapshot_for_incident,
                    self.federation.failure_threshold,
                );
            }
            trim_incidents(&mut state.incidents, self.operations.incident_limit);
            state.updated_at = now_iso();
        }

        {
            let mut state = self.inner.lock().expect("operations state poisoned");
            state.poll_in_progress = false;
            state.poll_count = state.poll_count.saturating_add(1);
            state.last_poll_finished_at = Some(now_iso());
            state.updated_at = now_iso();
        }
        if let Err(error) = self.persist() {
            tracing::warn!("failed to persist Control Room operations state: {error}");
        }
    }

    pub fn overview(&self) -> OperationsOverview {
        let state = self.inner.lock().expect("operations state poisoned");
        let mut healthy = 0;
        let mut degraded = 0;
        let mut offline = 0;
        let mut disabled = 0;
        let mut critical_offline = 0;
        for service in state.services.values() {
            match service.status.as_str() {
                "healthy" => healthy += 1,
                "degraded" | "unknown" => degraded += 1,
                "offline" => {
                    offline += 1;
                    if service.critical {
                        critical_offline += 1;
                    }
                }
                "disabled" => disabled += 1,
                _ => degraded += 1,
            }
        }
        OperationsOverview {
            started_at: state.started_at.clone(),
            updated_at: state.updated_at.clone(),
            last_poll_started_at: state.last_poll_started_at.clone(),
            last_poll_finished_at: state.last_poll_finished_at.clone(),
            poll_in_progress: state.poll_in_progress,
            poll_count: state.poll_count,
            services_total: state.services.len(),
            services_healthy: healthy,
            services_degraded: degraded,
            services_offline: offline,
            services_disabled: disabled,
            critical_services_offline: critical_offline,
            incidents_open: state.incidents.iter().filter(|item| item.status == "open").count(),
            incidents_acknowledged: state
                .incidents
                .iter()
                .filter(|item| item.status == "acknowledged")
                .count(),
            shift_log_entries: state.shift_log.len(),
        }
    }

    pub fn services(&self) -> Vec<ServiceSnapshot> {
        self.inner
            .lock()
            .expect("operations state poisoned")
            .services
            .values()
            .cloned()
            .collect()
    }

    pub fn service(&self, name: &str) -> Option<ServiceSnapshot> {
        self.inner
            .lock()
            .expect("operations state poisoned")
            .services
            .get(name)
            .cloned()
    }

    pub fn set_service_enabled(
        &self,
        name: &str,
        enabled: bool,
        operator_id: &str,
    ) -> Result<ServiceSnapshot, String> {
        let snapshot = {
            let mut state = self.inner.lock().map_err(|_| "operations state poisoned".to_string())?;
            let service = state
                .services
                .get_mut(name)
                .ok_or_else(|| format!("unknown service '{name}'"))?;
            service.enabled = enabled;
            service.status = if enabled { "unknown" } else { "disabled" }.to_string();
            service.message = Some(format!(
                "administratively {} by {}",
                if enabled { "enabled" } else { "disabled" },
                normalise_operator(operator_id)
            ));
            service.checked_at = Some(now_iso());
            let snapshot = service.clone();
            state.shift_log.push(ShiftLogEntry {
                id: format!("log_{}", Uuid::new_v4().as_simple()),
                timestamp: now_iso(),
                operator_id: normalise_operator(operator_id),
                category: "service".to_string(),
                text: format!(
                    "Service {} administratively {} in Control Room monitoring",
                    name,
                    if enabled { "enabled" } else { "disabled" }
                ),
            });
            trim_shift_log(&mut state.shift_log, self.operations.shift_log_limit);
            state.updated_at = now_iso();
            snapshot
        };
        self.persist().map_err(|error| error.to_string())?;
        Ok(snapshot)
    }

    pub fn incidents(&self, status: Option<&str>, limit: usize) -> Vec<IncidentRecord> {
        let state = self.inner.lock().expect("operations state poisoned");
        state
            .incidents
            .iter()
            .rev()
            .filter(|incident| status.map(|value| incident.status == value).unwrap_or(true))
            .take(limit.max(1))
            .cloned()
            .collect()
    }

    pub fn create_incident(&self, request: CreateIncidentRequest) -> Result<IncidentRecord, String> {
        let title = request.title.trim();
        if title.is_empty() {
            return Err("title is required".to_string());
        }
        let severity = normalise_severity(request.severity.as_deref().unwrap_or("warning"));
        let incident = IncidentRecord {
            id: format!("inc_{}", Uuid::new_v4().as_simple()),
            source_key: None,
            severity,
            status: "open".to_string(),
            title: title.to_string(),
            description: request.description.unwrap_or_default().trim().to_string(),
            service: clean_optional(request.service),
            node_id: clean_optional(request.node_id),
            issi: request.issi,
            gssi: request.gssi,
            created_at: now_iso(),
            created_by: normalise_operator(request.operator_id.as_deref().unwrap_or("operator")),
            acknowledged_at: None,
            acknowledged_by: None,
            resolved_at: None,
            resolved_by: None,
            notes: Vec::new(),
        };
        {
            let mut state = self.inner.lock().map_err(|_| "operations state poisoned".to_string())?;
            state.incidents.push(incident.clone());
            trim_incidents(&mut state.incidents, self.operations.incident_limit);
            state.updated_at = now_iso();
        }
        self.persist().map_err(|error| error.to_string())?;
        Ok(incident)
    }

    pub fn acknowledge_incident(
        &self,
        id: &str,
        request: IncidentActionRequest,
    ) -> Result<IncidentRecord, String> {
        let operator = normalise_operator(request.operator_id.as_deref().unwrap_or("operator"));
        let updated = {
            let mut state = self.inner.lock().map_err(|_| "operations state poisoned".to_string())?;
            let incident = state
                .incidents
                .iter_mut()
                .find(|incident| incident.id == id)
                .ok_or_else(|| format!("unknown incident '{id}'"))?;
            if incident.status == "resolved" {
                return Err("resolved incident cannot be acknowledged".to_string());
            }
            incident.status = "acknowledged".to_string();
            incident.acknowledged_at = Some(now_iso());
            incident.acknowledged_by = Some(operator.clone());
            if let Some(note) = request.note.and_then(non_empty_string) {
                incident.notes.push(IncidentNote {
                    id: format!("note_{}", Uuid::new_v4().as_simple()),
                    timestamp: now_iso(),
                    operator_id: operator,
                    text: note,
                });
            }
            let updated = incident.clone();
            state.updated_at = now_iso();
            updated
        };
        self.persist().map_err(|error| error.to_string())?;
        Ok(updated)
    }

    pub fn resolve_incident(
        &self,
        id: &str,
        request: IncidentActionRequest,
    ) -> Result<IncidentRecord, String> {
        let operator = normalise_operator(request.operator_id.as_deref().unwrap_or("operator"));
        let updated = {
            let mut state = self.inner.lock().map_err(|_| "operations state poisoned".to_string())?;
            let incident = state
                .incidents
                .iter_mut()
                .find(|incident| incident.id == id)
                .ok_or_else(|| format!("unknown incident '{id}'"))?;
            incident.status = "resolved".to_string();
            incident.resolved_at = Some(now_iso());
            incident.resolved_by = Some(operator.clone());
            if let Some(note) = request.note.and_then(non_empty_string) {
                incident.notes.push(IncidentNote {
                    id: format!("note_{}", Uuid::new_v4().as_simple()),
                    timestamp: now_iso(),
                    operator_id: operator,
                    text: note,
                });
            }
            let updated = incident.clone();
            state.updated_at = now_iso();
            updated
        };
        self.persist().map_err(|error| error.to_string())?;
        Ok(updated)
    }

    pub fn add_incident_note(
        &self,
        id: &str,
        request: IncidentNoteRequest,
    ) -> Result<IncidentRecord, String> {
        let text = request.text.trim();
        if text.is_empty() {
            return Err("note text is required".to_string());
        }
        let updated = {
            let mut state = self.inner.lock().map_err(|_| "operations state poisoned".to_string())?;
            let incident = state
                .incidents
                .iter_mut()
                .find(|incident| incident.id == id)
                .ok_or_else(|| format!("unknown incident '{id}'"))?;
            incident.notes.push(IncidentNote {
                id: format!("note_{}", Uuid::new_v4().as_simple()),
                timestamp: now_iso(),
                operator_id: normalise_operator(request.operator_id.as_deref().unwrap_or("operator")),
                text: text.to_string(),
            });
            let updated = incident.clone();
            state.updated_at = now_iso();
            updated
        };
        self.persist().map_err(|error| error.to_string())?;
        Ok(updated)
    }

    pub fn shift_log(&self, limit: usize) -> Vec<ShiftLogEntry> {
        self.inner
            .lock()
            .expect("operations state poisoned")
            .shift_log
            .iter()
            .rev()
            .take(limit.max(1))
            .cloned()
            .collect()
    }

    pub fn add_shift_log(&self, request: ShiftLogRequest) -> Result<ShiftLogEntry, String> {
        let text = request.text.trim();
        if text.is_empty() {
            return Err("text is required".to_string());
        }
        let entry = ShiftLogEntry {
            id: format!("log_{}", Uuid::new_v4().as_simple()),
            timestamp: now_iso(),
            operator_id: normalise_operator(request.operator_id.as_deref().unwrap_or("operator")),
            category: request
                .category
                .and_then(non_empty_string)
                .unwrap_or_else(|| "general".to_string()),
            text: text.to_string(),
        };
        {
            let mut state = self.inner.lock().map_err(|_| "operations state poisoned".to_string())?;
            state.shift_log.push(entry.clone());
            trim_shift_log(&mut state.shift_log, self.operations.shift_log_limit);
            state.updated_at = now_iso();
        }
        self.persist().map_err(|error| error.to_string())?;
        Ok(entry)
    }

    pub fn federated_domain_overview(&self) -> Value {
        let state = self.inner.lock().expect("operations state poisoned");
        let mut domains = BTreeMap::<String, Value>::new();

        for (name, service) in &state.services {
            let metrics = curated_metrics(name, service.summary.as_ref());
            domains.insert(
                name.clone(),
                json!({
                    "display_name": service.display_name,
                    "status": service.status,
                    "critical": service.critical,
                    "checked_at": service.checked_at,
                    "last_success_at": service.last_success_at,
                    "metrics": metrics,
                }),
            );
        }

        let preferred_counts = json!({
            "connected_nodes": first_metric(&state.services, &[
                ("node-gateway", "connected_nodes"),
                ("mobility-core", "nodes_connected"),
                ("subscriber-core", "nodes_connected"),
            ]),
            "subscribers_registered": first_metric(&state.services, &[
                ("subscriber-core", "observed_registered"),
                ("mobility-core", "subscribers_known"),
            ]),
            "subscribers_total": first_metric(&state.services, &[
                ("subscriber-core", "subscribers_total"),
            ]),
            "groups_total": first_metric(&state.services, &[
                ("group-core", "groups_total"),
            ]),
            "group_affiliations": first_metric(&state.services, &[
                ("group-core", "observed_affiliations"),
            ]),
            "active_calls": first_metric(&state.services, &[
                ("call-control", "calls_active"),
            ]),
            "active_media_sessions": first_metric(&state.services, &[
                ("media-switch", "sessions_active"),
            ]),
            "active_recordings": first_metric(&state.services, &[
                ("recorder", "active_recordings"),
            ]),
            "sds_queued": sum_metrics(&state.services, &[
                ("sds-router", "queued"),
                ("sds-router", "offline"),
                ("sds-router", "in_flight"),
            ]),
            "packet_contexts_ready": first_metric(&state.services, &[
                ("packet-core", "contexts_ready"),
            ]),
            "ip_flows": first_metric(&state.services, &[
                ("ip-gateway", "flows"),
            ]),
            "security_alarms": first_metric(&state.services, &[
                ("security-core", "open_alarms"),
            ]),
            "active_keys": first_metric(&state.services, &[
                ("kmf", "active_keys"),
            ]),
            "transit_sessions": first_metric(&state.services, &[
                ("transit", "sessions_active"),
            ]),
            "transit_peers_up": first_metric(&state.services, &[
                ("transit", "peers_up"),
            ]),
            "application_deliveries_pending": sum_metrics(&state.services, &[
                ("application-gateway", "deliveries_queued"),
                ("application-gateway", "deliveries_retry"),
            ]),
            "application_dead_letters": first_metric(&state.services, &[
                ("application-gateway", "deliveries_dead_letter"),
            ]),
        });

        json!({
            "source": "polled authoritative core-service summaries",
            "authoritative_state_stored_locally": false,
            "last_poll_finished_at": state.last_poll_finished_at,
            "preferred_counts": preferred_counts,
            "domains": domains,
        })
    }

    pub fn dependencies(&self) -> Value {
        let services = self.services();
        json!({
            "architecture": "Control Room is an operator and presentation layer. Core services remain authoritative.",
            "write_proxy": false,
            "direct_tbs_compatibility": true,
            "services": services,
            "timestamp": now_iso(),
        })
    }

    pub fn export(&self) -> Value {
        let state = self.inner.lock().expect("operations state poisoned");
        json!({
            "schema_version": STATE_SCHEMA_VERSION,
            "exported_at": now_iso(),
            "security_mode": "open_lab",
            "authoritative_data_owner": "individual core services",
            "operations": &*state,
        })
    }

    pub fn metrics(&self) -> String {
        let overview = self.overview();
        format!(
            concat!(
                "# HELP netcore_control_room_services_total Configured backend services\n",
                "# TYPE netcore_control_room_services_total gauge\n",
                "netcore_control_room_services_total {}\n",
                "# HELP netcore_control_room_services_healthy Healthy backend services\n",
                "# TYPE netcore_control_room_services_healthy gauge\n",
                "netcore_control_room_services_healthy {}\n",
                "# HELP netcore_control_room_services_offline Offline backend services\n",
                "# TYPE netcore_control_room_services_offline gauge\n",
                "netcore_control_room_services_offline {}\n",
                "# HELP netcore_control_room_critical_services_offline Offline critical backend services\n",
                "# TYPE netcore_control_room_critical_services_offline gauge\n",
                "netcore_control_room_critical_services_offline {}\n",
                "# HELP netcore_control_room_incidents_open Open or acknowledged incidents\n",
                "# TYPE netcore_control_room_incidents_open gauge\n",
                "netcore_control_room_incidents_open {}\n",
                "# HELP netcore_control_room_poll_count Completed service polls\n",
                "# TYPE netcore_control_room_poll_count counter\n",
                "netcore_control_room_poll_count {}\n"
            ),
            overview.services_total,
            overview.services_healthy,
            overview.services_offline,
            overview.critical_services_offline,
            overview.incidents_open + overview.incidents_acknowledged,
            overview.poll_count,
        )
    }

    pub fn config_snapshot(&self) -> Value {
        json!({
            "security": {
                "mode": "open_lab",
                "auth_enabled": false,
                "token_auth": false,
                "tls": false,
            },
            "federation": &self.federation,
            "operations": {
                "state_path": self.operations.state_path.display().to_string(),
                "backup_path": self.operations.backup_path.display().to_string(),
                "auto_service_incidents": self.operations.auto_service_incidents,
                "incident_limit": self.operations.incident_limit,
                "shift_log_limit": self.operations.shift_log_limit,
            },
            "services": self.service_configs.as_ref(),
            "architecture_boundary": "No authoritative subscriber, mobility, group, call, SDS, packet or key state is created here.",
        })
    }

    fn persist(&self) -> Result<(), Box<dyn std::error::Error>> {
        let state = self.inner.lock().expect("operations state poisoned").clone();
        write_json_atomic(&self.operations.state_path, &self.operations.backup_path, &state)
    }
}

#[derive(Debug)]
struct PollResult {
    status: String,
    live: Option<bool>,
    ready: Option<bool>,
    checked_at: String,
    latency_ms: Option<u64>,
    http_status: Option<u16>,
    message: Option<String>,
    health: Option<Value>,
    readiness: Option<Value>,
    summary: Option<Value>,
}

fn poll_service(config: &CoreServiceConfig, federation: &FederationConfig) -> PollResult {
    let timeout = Duration::from_millis(
        config
            .timeout_ms
            .unwrap_or(federation.request_timeout_ms)
            .max(100),
    );
    let started = Instant::now();
    let live_result = http_get_json(&config.base_url, &config.health_live, timeout);
    let ready_result = http_get_json(&config.base_url, &config.health_ready, timeout);
    let summary_result = if federation.fetch_summaries && !config.summary_path.trim().is_empty() {
        http_get_json(&config.base_url, &config.summary_path, timeout).ok()
    } else {
        None
    };

    let live = live_result.as_ref().ok().map(|response| response.status < 400);
    let ready = ready_result.as_ref().ok().map(|response| response.status < 400);
    let status = match (live, ready) {
        (Some(true), Some(true)) => "healthy",
        (Some(true), _) => "degraded",
        _ => "offline",
    }
    .to_string();

    let message = match (&live_result, &ready_result) {
        (Ok(_), Ok(_)) if status == "healthy" => None,
        (Ok(_), Ok(_)) => Some("service is live but not ready".to_string()),
        (Err(live_error), Err(ready_error)) => Some(format!(
            "live check failed: {live_error}; ready check failed: {ready_error}"
        )),
        (Err(error), _) => Some(format!("live check failed: {error}")),
        (_, Err(error)) => Some(format!("ready check failed: {error}")),
    };

    PollResult {
        status,
        live,
        ready,
        checked_at: now_iso(),
        latency_ms: Some(started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64),
        http_status: live_result
            .as_ref()
            .ok()
            .map(|response| response.status)
            .or_else(|| ready_result.as_ref().ok().map(|response| response.status)),
        message,
        health: live_result.ok().map(|response| response.body),
        readiness: ready_result.ok().map(|response| response.body),
        summary: summary_result.map(|response| response.body),
    }
}

fn apply_poll_result(snapshot: &mut ServiceSnapshot, result: PollResult) {
    snapshot.status = result.status;
    snapshot.live = result.live;
    snapshot.ready = result.ready;
    snapshot.checked_at = Some(result.checked_at);
    snapshot.latency_ms = result.latency_ms;
    snapshot.http_status = result.http_status;
    snapshot.message = result.message;
    snapshot.health = result.health;
    snapshot.readiness = result.readiness;
    snapshot.summary = result.summary;
    if snapshot.status == "healthy" {
        snapshot.last_success_at = snapshot.checked_at.clone();
        snapshot.consecutive_failures = 0;
    } else {
        snapshot.consecutive_failures = snapshot.consecutive_failures.saturating_add(1);
    }
}

fn reconcile_service_incident(
    incidents: &mut Vec<IncidentRecord>,
    snapshot: &ServiceSnapshot,
    failure_threshold: u32,
) {
    let source_key = format!("service:{}", snapshot.name);
    let open_index = incidents.iter().position(|incident| {
        incident.source_key.as_deref() == Some(source_key.as_str()) && incident.status != "resolved"
    });

    if snapshot.status == "offline" && snapshot.consecutive_failures >= failure_threshold.max(1) {
        if open_index.is_none() {
            incidents.push(IncidentRecord {
                id: format!("inc_{}", Uuid::new_v4().as_simple()),
                source_key: Some(source_key),
                severity: if snapshot.critical { "critical" } else { "warning" }.to_string(),
                status: "open".to_string(),
                title: format!("{} ist nicht erreichbar", snapshot.display_name),
                description: snapshot
                    .message
                    .clone()
                    .unwrap_or_else(|| "health/readiness checks failed".to_string()),
                service: Some(snapshot.name.clone()),
                node_id: None,
                issi: None,
                gssi: None,
                created_at: now_iso(),
                created_by: "control-room-auto".to_string(),
                acknowledged_at: None,
                acknowledged_by: None,
                resolved_at: None,
                resolved_by: None,
                notes: Vec::new(),
            });
        }
    } else if snapshot.status == "healthy" {
        if let Some(index) = open_index {
            let incident = &mut incidents[index];
            incident.status = "resolved".to_string();
            incident.resolved_at = Some(now_iso());
            incident.resolved_by = Some("control-room-auto".to_string());
            incident.notes.push(IncidentNote {
                id: format!("note_{}", Uuid::new_v4().as_simple()),
                timestamp: now_iso(),
                operator_id: "control-room-auto".to_string(),
                text: "Service health and readiness recovered".to_string(),
            });
        }
    }
}

#[derive(Debug)]
struct HttpJsonResponse {
    status: u16,
    body: Value,
}

fn http_get_json(
    base_url: &str,
    path: &str,
    timeout: Duration,
) -> Result<HttpJsonResponse, String> {
    let url = parse_http_url(&join_url(base_url, path))?;
    let address = format!("{}:{}", url.host, url.port);
    let mut addresses = address
        .to_socket_addrs()
        .map_err(|error| format!("resolve {address}: {error}"))?;
    let socket = addresses
        .next()
        .ok_or_else(|| format!("no address for {address}"))?;
    let mut stream = TcpStream::connect_timeout(&socket, timeout)
        .map_err(|error| format!("connect {address}: {error}"))?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|error| error.to_string())?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|error| error.to_string())?;

    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nAccept: application/json\r\nUser-Agent: netcore-control-room/1\r\nConnection: close\r\n\r\n",
        url.path, url.host_header
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|error| format!("write request: {error}"))?;
    let _ = stream.shutdown(Shutdown::Write);
    let mut bytes = Vec::new();
    stream
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read response: {error}"))?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err("response exceeds 4 MiB".to_string());
    }
    let split = find_subslice(&bytes, b"\r\n\r\n")
        .ok_or_else(|| "invalid HTTP response".to_string())?;
    let header = String::from_utf8_lossy(&bytes[..split]);
    let status = header
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| "invalid HTTP status".to_string())?;
    let body_bytes = &bytes[split + 4..];
    let body = if body_bytes.is_empty() {
        json!({})
    } else {
        serde_json::from_slice(body_bytes).unwrap_or_else(|_| {
            json!({ "raw": String::from_utf8_lossy(body_bytes).chars().take(2048).collect::<String>() })
        })
    };
    Ok(HttpJsonResponse { status, body })
}

#[derive(Debug)]
struct ParsedHttpUrl {
    host: String,
    host_header: String,
    port: u16,
    path: String,
}

fn parse_http_url(url: &str) -> Result<ParsedHttpUrl, String> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| "only http:// URLs are supported in open_lab federation".to_string())?;
    let (authority, path) = match rest.split_once('/') {
        Some((authority, path)) => (authority, format!("/{path}")),
        None => (rest, "/".to_string()),
    };
    if authority.trim().is_empty() {
        return Err("URL host is empty".to_string());
    }
    let (host, port) = if authority.starts_with('[') {
        let end = authority
            .find(']')
            .ok_or_else(|| "invalid IPv6 authority".to_string())?;
        let host = authority[1..end].to_string();
        let port = authority[end + 1..]
            .strip_prefix(':')
            .map(|value| value.parse::<u16>())
            .transpose()
            .map_err(|error| format!("invalid port: {error}"))?
            .unwrap_or(80);
        (host, port)
    } else if let Some((host, port)) = authority.rsplit_once(':') {
        if port.chars().all(|character| character.is_ascii_digit()) {
            (
                host.to_string(),
                port.parse::<u16>()
                    .map_err(|error| format!("invalid port: {error}"))?,
            )
        } else {
            (authority.to_string(), 80)
        }
    } else {
        (authority.to_string(), 80)
    };
    Ok(ParsedHttpUrl {
        host,
        host_header: authority.to_string(),
        port,
        path,
    })
}

fn join_url(base_url: &str, path: &str) -> String {
    let base = base_url.trim_end_matches('/');
    if path.trim().is_empty() || path == "/" {
        format!("{base}/")
    } else if path.starts_with('/') {
        format!("{base}{path}")
    } else {
        format!("{base}/{path}")
    }
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
}

fn write_json_atomic<T: Serialize>(
    path: &Path,
    backup_path: &Path,
    value: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    if let Some(parent) = backup_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, &bytes)?;
    if path.exists() {
        let _ = fs::copy(path, backup_path);
    }
    fs::rename(&temporary, path)?;
    Ok(())
}

fn normalise_operator(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "operator".to_string()
    } else {
        trimmed.chars().take(64).collect()
    }
}

fn normalise_severity(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "info" => "info",
        "warning" | "warn" => "warning",
        "critical" | "emergency" => "critical",
        _ => "warning",
    }
    .to_string()
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value.and_then(non_empty_string)
}

fn non_empty_string(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn curated_metrics(service_name: &str, summary: Option<&Value>) -> BTreeMap<String, Value> {
    let fields: &[&str] = match service_name {
        "node-gateway" => &[
            "connected_nodes", "known_nodes", "stale_nodes", "backend_clients",
            "total_node_messages", "total_commands", "total_media_frames",
        ],
        "subscriber-core" => &[
            "subscribers_total", "subscribers_authorized", "subscribers_blocked",
            "observed_registered", "nodes_connected", "nodes_out_of_sync",
        ],
        "group-core" => &[
            "groups_total", "groups_enabled", "memberships_total", "observed_affiliations",
            "nodes_connected", "syncs_pending", "dgna_pending",
        ],
        "mobility-core" => &[
            "subscribers_known", "transfers_active", "transfers_completed", "transfers_failed",
            "nodes_connected",
        ],
        "call-control" => &[
            "calls_total", "calls_active", "calls_managed", "call_legs_active",
            "participants_registered", "pending_commands", "restores_pending",
        ],
        "media-switch" => &[
            "sessions_active", "streams_active", "pending_frames", "frames_received",
            "frames_routed", "frames_dropped", "recorder_taps_buffered",
        ],
        "recorder" => &[
            "active_recordings", "completed_recordings", "frames_ingested",
            "frames_lost_before_recorder", "storage_used_bytes", "storage_free_bytes",
        ],
        "sds-router" => &[
            "messages_total", "queued", "offline", "in_flight", "delivered", "failed",
            "dead_letter", "duplicate_messages",
        ],
        "packet-core" => &[
            "contexts_total", "contexts_ready", "contexts_standby", "contexts_suspended",
            "bearers_active", "actions_pending", "queued_packets", "queued_bytes",
        ],
        "ip-gateway" => &[
            "contexts", "flows", "captures_active", "packets_uplink", "packets_downlink",
            "packets_dropped", "dns_queries", "test_requests",
        ],
        "security-core" => &[
            "profiles", "subscribers", "active_auth_contexts", "active_dck_contexts",
            "pending_actions", "open_alarms", "known_nodes",
        ],
        "kmf" => &[
            "total_keys", "active_keys", "staged_keys", "revoked_keys", "otar_jobs",
            "pending_actions", "enabled_nodes", "backups",
        ],
        "transit" => &[
            "peers_total", "peers_up", "peers_degraded", "routes_total", "sessions_active",
            "outbound_pending", "local_deliveries_pending", "loop_rejections",
            "duplicate_rejections",
        ],
        "application-gateway" => &[
            "connectors_total", "connectors_enabled", "connectors_healthy", "connectors_degraded",
            "circuits_open", "events_total", "events_unrouted", "deliveries_queued",
            "deliveries_retry", "deliveries_delivered", "deliveries_shadowed",
            "deliveries_dead_letter", "tts_jobs_total", "tts_jobs_ready",
            "missing_required_secrets",
        ],
        _ => &[],
    };

    let mut metrics = BTreeMap::new();
    let Some(summary) = summary.and_then(Value::as_object) else {
        return metrics;
    };
    for field in fields {
        if let Some(value) = summary.get(*field) {
            metrics.insert((*field).to_string(), value.clone());
        }
    }
    metrics
}

fn summary_metric(services: &BTreeMap<String, ServiceSnapshot>, service: &str, field: &str) -> Option<u64> {
    services
        .get(service)
        .and_then(|snapshot| snapshot.summary.as_ref())
        .and_then(|summary| summary.get(field))
        .and_then(value_as_u64)
}

fn first_metric(services: &BTreeMap<String, ServiceSnapshot>, candidates: &[(&str, &str)]) -> Option<u64> {
    candidates
        .iter()
        .find_map(|(service, field)| summary_metric(services, service, field))
}

fn sum_metrics(services: &BTreeMap<String, ServiceSnapshot>, candidates: &[(&str, &str)]) -> Option<u64> {
    let values = candidates
        .iter()
        .filter_map(|(service, field)| summary_metric(services, service, field))
        .collect::<Vec<_>>();
    if values.is_empty() {
        None
    } else {
        Some(values.into_iter().fold(0_u64, u64::saturating_add))
    }
}

fn value_as_u64(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_i64().and_then(|number| u64::try_from(number).ok()))
        .or_else(|| value.as_str().and_then(|number| number.parse::<u64>().ok()))
}

fn trim_incidents(incidents: &mut Vec<IncidentRecord>, limit: usize) {
    let keep = limit.max(100);
    if incidents.len() > keep {
        let remove = incidents.len() - keep;
        incidents.drain(0..remove);
    }
}

fn trim_shift_log(entries: &mut Vec<ShiftLogEntry>, limit: usize) {
    let keep = limit.max(100);
    if entries.len() > keep {
        let remove = entries.len() - keep;
        entries.drain(0..remove);
    }
}

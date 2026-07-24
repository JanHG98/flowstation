use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::config::{
    TransitConfig, MODE_AUTHORITATIVE, TRANSIT_PROTOCOL_VERSION,
};
use crate::protocol::{
    DeliveryAckInput, GroupReachabilityInput, MaintenanceInput, PeerActionInput, PeerCreateInput,
    PeerHeartbeatInput, RouteActionInput, RouteCreateInput, RouteResolveInput, SessionActionInput,
    SubscriberLocationInput, TransitEnvelopeInput, TransitSubmitInput,
};

const MAX_SSI: u32 = 16_777_215;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalRegionRecord {
    pub region_id: String,
    pub swmi_id: String,
    pub display_name: String,
    pub advertised_endpoint: String,
    pub protocol_version: String,
    pub capabilities: Vec<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerRecord {
    pub peer_id: String,
    pub region_id: String,
    pub swmi_id: String,
    pub display_name: String,
    pub endpoint: String,
    pub protocol_version: String,
    pub priority: i32,
    pub capabilities: Vec<String>,
    pub admin_state: String,
    pub oper_state: String,
    pub dynamic: bool,
    pub latency_ms: Option<f64>,
    pub failure_count: u32,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub last_heartbeat_sent_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteRecord {
    pub route_id: String,
    pub service: String,
    pub selector_type: String,
    pub selector_value: String,
    pub destination_region: String,
    pub peer_id: String,
    pub preference: i32,
    pub metric: u32,
    pub failover_group: Option<String>,
    pub enabled: bool,
    pub learned: bool,
    pub path_vector: Vec<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriberLocation {
    pub issi: u32,
    pub home_region: String,
    pub current_region: String,
    pub serving_node: Option<String>,
    pub sequence: u64,
    pub source_peer: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupReachability {
    pub gssi: u32,
    pub regions: Vec<String>,
    pub source_peer: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionLeg {
    pub target_region: String,
    pub selected_peer: Option<String>,
    pub backup_peers: Vec<String>,
    pub state: String,
    pub failover_count: u32,
    pub last_error: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub session_id: String,
    pub service: String,
    pub source_kind: String,
    pub source: String,
    pub destination_kind: String,
    pub destination: String,
    pub origin_region: String,
    pub correlation_id: Option<String>,
    pub state: String,
    pub legs: Vec<SessionLeg>,
    pub envelope_count: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundEnvelope {
    pub protocol_version: String,
    pub envelope_id: String,
    pub dedupe_key: String,
    pub service: String,
    pub operation: String,
    pub origin_region: String,
    pub previous_hop_region: String,
    pub target_region: String,
    pub source_kind: String,
    pub source: String,
    pub destination_kind: String,
    pub destination: String,
    pub session_id: String,
    pub correlation_id: Option<String>,
    pub priority: u8,
    pub trace: Vec<String>,
    pub hop_count: u8,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub payload: Value,
    pub selected_peer: Option<String>,
    pub backup_peers: Vec<String>,
    pub route_id: Option<String>,
    pub state: String,
    pub attempts: u32,
    pub next_attempt_at: DateTime<Utc>,
    pub last_attempt_at: Option<DateTime<Utc>>,
    pub delivered_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

impl OutboundEnvelope {
    pub fn to_wire(&self, local_region: &str) -> TransitEnvelopeInput {
        TransitEnvelopeInput {
            protocol_version: self.protocol_version.clone(),
            envelope_id: self.envelope_id.clone(),
            dedupe_key: self.dedupe_key.clone(),
            service: self.service.clone(),
            operation: self.operation.clone(),
            origin_region: self.origin_region.clone(),
            previous_hop_region: local_region.to_string(),
            target_region: self.target_region.clone(),
            source_kind: self.source_kind.clone(),
            source: self.source.clone(),
            destination_kind: self.destination_kind.clone(),
            destination: self.destination.clone(),
            session_id: self.session_id.clone(),
            correlation_id: self.correlation_id.clone(),
            priority: self.priority,
            trace: self.trace.clone(),
            hop_count: self.hop_count,
            created_at: self.created_at.to_rfc3339(),
            expires_at: self.expires_at.to_rfc3339(),
            payload: self.payload.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalDelivery {
    pub delivery_id: String,
    pub envelope_id: String,
    pub service: String,
    pub operation: String,
    pub source_region: String,
    pub source_kind: String,
    pub source: String,
    pub destination_kind: String,
    pub destination: String,
    pub session_id: String,
    pub correlation_id: Option<String>,
    pub priority: u8,
    pub payload: Value,
    pub trace: Vec<String>,
    pub state: String,
    pub created_at: DateTime<Utc>,
    pub acknowledged_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DedupeEntry {
    pub dedupe_key: String,
    pub envelope_id: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitEvent {
    pub sequence: u64,
    pub timestamp: DateTime<Utc>,
    pub severity: String,
    pub category: String,
    pub action: String,
    pub actor: String,
    pub target: String,
    pub detail: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitDatabase {
    pub schema_version: u32,
    pub local_region: LocalRegionRecord,
    pub peers: HashMap<String, PeerRecord>,
    pub routes: HashMap<String, RouteRecord>,
    pub subscriber_locations: HashMap<String, SubscriberLocation>,
    pub group_reachability: HashMap<String, GroupReachability>,
    pub sessions: HashMap<String, SessionRecord>,
    pub outbound: HashMap<String, OutboundEnvelope>,
    pub local_deliveries: HashMap<String, LocalDelivery>,
    pub dedupe: HashMap<String, DedupeEntry>,
    pub events: Vec<TransitEvent>,
    pub next_event_sequence: u64,
    pub next_heartbeat_sequence: u64,
    pub started_at: DateTime<Utc>,
    pub last_maintenance_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransitStatus {
    pub service: &'static str,
    pub version: &'static str,
    pub protocol_version: &'static str,
    pub security_mode: &'static str,
    pub token_auth: bool,
    pub tls: bool,
    pub operating_mode: String,
    pub authoritative: bool,
    pub region_id: String,
    pub swmi_id: String,
    pub peers_total: usize,
    pub peers_up: usize,
    pub peers_degraded: usize,
    pub peers_blocked: usize,
    pub routes_total: usize,
    pub subscriber_locations: usize,
    pub group_locations: usize,
    pub sessions_active: usize,
    pub outbound_pending: usize,
    pub local_deliveries_pending: usize,
    pub loop_rejections: usize,
    pub duplicate_rejections: usize,
    pub ready: bool,
    pub uptime_secs: i64,
    pub last_maintenance_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RouteDecision {
    pub accepted: bool,
    pub target_region: Option<String>,
    pub selected_peer: Option<String>,
    pub backup_peers: Vec<String>,
    pub route_id: Option<String>,
    pub reason: String,
    pub candidate_count: usize,
    pub trace: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubmitResult {
    pub accepted: bool,
    pub session_id: String,
    pub envelope_ids: Vec<String>,
    pub decisions: Vec<RouteDecision>,
    pub operating_mode: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct IngestResult {
    pub accepted: bool,
    pub duplicate: bool,
    pub local_delivery_id: Option<String>,
    pub forwarded_envelope_id: Option<String>,
    pub route: Option<RouteDecision>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BackupResult {
    pub path: String,
    pub bytes: u64,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct SharedTransit {
    inner: Arc<Mutex<TransitDatabase>>,
    config: TransitConfig,
}

impl SharedTransit {
    pub fn load(config: TransitConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let now = Utc::now();
        let local_region = local_region_from_config(&config, now);
        let mut database = if config.storage.database_path.is_file() {
            serde_json::from_slice::<TransitDatabase>(&fs::read(&config.storage.database_path)?)?
        } else {
            TransitDatabase {
                schema_version: 1,
                local_region: local_region.clone(),
                peers: HashMap::new(),
                routes: HashMap::new(),
                subscriber_locations: HashMap::new(),
                group_reachability: HashMap::new(),
                sessions: HashMap::new(),
                outbound: HashMap::new(),
                local_deliveries: HashMap::new(),
                dedupe: HashMap::new(),
                events: Vec::new(),
                next_event_sequence: 1,
                next_heartbeat_sequence: 1,
                started_at: now,
                last_maintenance_at: None,
            }
        };
        database.local_region = local_region;
        database.schema_version = 1;
        prune_database(&mut database, &config, now);
        let transit = Self {
            inner: Arc::new(Mutex::new(database)),
            config,
        };
        transit.persist()?;
        Ok(transit)
    }

    pub fn status(&self) -> TransitStatus {
        let database = self.inner.lock().expect("transit state poisoned");
        let now = Utc::now();
        let peers_up = database
            .peers
            .values()
            .filter(|peer| peer.oper_state == "up" && peer.admin_state == "enabled")
            .count();
        let peers_degraded = database
            .peers
            .values()
            .filter(|peer| peer.oper_state == "degraded")
            .count();
        let peers_blocked = database
            .peers
            .values()
            .filter(|peer| peer.admin_state != "enabled")
            .count();
        let sessions_active = database
            .sessions
            .values()
            .filter(|session| !matches!(session.state.as_str(), "closed" | "failed" | "expired"))
            .count();
        let outbound_pending = database
            .outbound
            .values()
            .filter(|envelope| matches!(envelope.state.as_str(), "queued" | "retry" | "in_flight" | "shadow"))
            .count();
        let local_deliveries_pending = database
            .local_deliveries
            .values()
            .filter(|delivery| delivery.state == "pending")
            .count();
        let loop_rejections = database
            .events
            .iter()
            .filter(|event| event.action == "loop_rejected")
            .count();
        let duplicate_rejections = database
            .events
            .iter()
            .filter(|event| event.action == "duplicate_suppressed")
            .count();
        let authoritative = self.config.region.operating_mode == MODE_AUTHORITATIVE;
        TransitStatus {
            service: "netcore-transit",
            version: env!("CARGO_PKG_VERSION"),
            protocol_version: TRANSIT_PROTOCOL_VERSION,
            security_mode: "open_lab",
            token_auth: false,
            tls: false,
            operating_mode: self.config.region.operating_mode.clone(),
            authoritative,
            region_id: database.local_region.region_id.clone(),
            swmi_id: database.local_region.swmi_id.clone(),
            peers_total: database.peers.len(),
            peers_up,
            peers_degraded,
            peers_blocked,
            routes_total: database.routes.len(),
            subscriber_locations: database.subscriber_locations.len(),
            group_locations: database.group_reachability.len(),
            sessions_active,
            outbound_pending,
            local_deliveries_pending,
            loop_rejections,
            duplicate_rejections,
            ready: !authoritative || database.peers.is_empty() || peers_up > 0,
            uptime_secs: now.signed_duration_since(database.started_at.clone()).num_seconds().max(0),
            last_maintenance_at: database.last_maintenance_at.clone(),
        }
    }

    pub fn peers(&self) -> Vec<PeerRecord> {
        let database = self.inner.lock().expect("transit state poisoned");
        let mut peers: Vec<_> = database.peers.values().cloned().collect();
        peers.sort_by(|left, right| left.region_id.cmp(&right.region_id).then(left.peer_id.cmp(&right.peer_id)));
        peers
    }

    pub fn routes(&self) -> Vec<RouteRecord> {
        let database = self.inner.lock().expect("transit state poisoned");
        let mut routes: Vec<_> = database.routes.values().cloned().collect();
        routes.sort_by(|left, right| {
            left.service
                .cmp(&right.service)
                .then(right.preference.cmp(&left.preference))
                .then(left.metric.cmp(&right.metric))
        });
        routes
    }

    pub fn subscriber_locations(&self) -> Vec<SubscriberLocation> {
        let database = self.inner.lock().expect("transit state poisoned");
        let mut values: Vec<_> = database.subscriber_locations.values().cloned().collect();
        values.sort_by_key(|entry| entry.issi);
        values
    }

    pub fn group_reachability(&self) -> Vec<GroupReachability> {
        let database = self.inner.lock().expect("transit state poisoned");
        let mut values: Vec<_> = database.group_reachability.values().cloned().collect();
        values.sort_by_key(|entry| entry.gssi);
        values
    }

    pub fn sessions(&self) -> Vec<SessionRecord> {
        let database = self.inner.lock().expect("transit state poisoned");
        let mut values: Vec<_> = database.sessions.values().cloned().collect();
        values.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        values
    }

    pub fn outbound(&self, peer_id: Option<&str>, limit: usize) -> Vec<OutboundEnvelope> {
        let database = self.inner.lock().expect("transit state poisoned");
        let mut values: Vec<_> = database
            .outbound
            .values()
            .filter(|entry| peer_id.is_none_or(|peer| entry.selected_peer.as_deref() == Some(peer)))
            .cloned()
            .collect();
        values.sort_by(|left, right| right.priority.cmp(&left.priority).then(left.created_at.cmp(&right.created_at)));
        values.truncate(limit);
        values
    }

    pub fn local_deliveries(&self, service: Option<&str>, limit: usize) -> Vec<LocalDelivery> {
        let database = self.inner.lock().expect("transit state poisoned");
        let mut values: Vec<_> = database
            .local_deliveries
            .values()
            .filter(|entry| service.is_none_or(|value| entry.service == value))
            .cloned()
            .collect();
        values.sort_by(|left, right| right.priority.cmp(&left.priority).then(left.created_at.cmp(&right.created_at)));
        values.truncate(limit);
        values
    }

    pub fn recent_events(&self, limit: usize) -> Vec<TransitEvent> {
        let database = self.inner.lock().expect("transit state poisoned");
        database.events.iter().rev().take(limit).cloned().collect()
    }

    pub fn create_peer(&self, input: PeerCreateInput) -> Result<PeerRecord, String> {
        validate_identifier(&input.peer_id, "peer_id")?;
        validate_identifier(&input.region_id, "region_id")?;
        if !input.endpoint.starts_with("http://") {
            return Err("peer endpoint must use http:// in open_lab mode".to_string());
        }
        let now = Utc::now();
        let mut database = self.inner.lock().map_err(|_| "state lock poisoned".to_string())?;
        if database.peers.len() >= self.config.limits.max_peers && !database.peers.contains_key(&input.peer_id) {
            return Err("peer limit reached".to_string());
        }
        if database.peers.values().any(|peer| peer.region_id == input.region_id && peer.peer_id != input.peer_id) {
            return Err("a peer for this region_id already exists".to_string());
        }
        let record = PeerRecord {
            peer_id: input.peer_id.clone(),
            region_id: input.region_id,
            swmi_id: input.swmi_id,
            display_name: input.display_name,
            endpoint: input.endpoint.trim_end_matches('/').to_string(),
            protocol_version: input.protocol_version.unwrap_or_else(|| TRANSIT_PROTOCOL_VERSION.to_string()),
            priority: input.priority.unwrap_or(100),
            capabilities: sorted_unique(input.capabilities),
            admin_state: "enabled".to_string(),
            oper_state: "unknown".to_string(),
            dynamic: false,
            latency_ms: None,
            failure_count: 0,
            last_seen_at: None,
            last_heartbeat_sent_at: None,
            last_error: None,
            created_at: database.peers.get(&input.peer_id).map(|peer| peer.created_at.clone()).unwrap_or(now),
            updated_at: now,
            notes: input.notes,
        };
        database.peers.insert(input.peer_id.clone(), record.clone());
        record_event(&mut database, &self.config, "info", "peer", "peer_saved", "operator", &input.peer_id, json!({"region_id":record.region_id.clone(),"endpoint":record.endpoint.clone()}));
        self.persist_locked(&database)?;
        Ok(record)
    }

    pub fn peer_action(&self, peer_id: &str, action: &str, input: PeerActionInput) -> Result<PeerRecord, String> {
        let now = Utc::now();
        let actor = input.actor.unwrap_or_else(|| "operator".to_string());
        let mut database = self.inner.lock().map_err(|_| "state lock poisoned".to_string())?;
        let record = database.peers.get_mut(peer_id).ok_or_else(|| "peer not found".to_string())?;
        match action {
            "enable" | "unblock" => record.admin_state = "enabled".to_string(),
            "maintenance" => record.admin_state = "maintenance".to_string(),
            "block" => record.admin_state = "blocked".to_string(),
            "mark-down" => record.oper_state = "down".to_string(),
            _ => return Err("unsupported peer action".to_string()),
        }
        record.updated_at = now;
        if let Some(reason) = input.reason.clone() {
            record.last_error = Some(reason);
        }
        let output = record.clone();
        record_event(&mut database, &self.config, "warning", "peer", action, &actor, peer_id, json!({"reason":input.reason}));
        if matches!(action, "block" | "maintenance" | "mark-down") {
            failover_from_peer(&mut database, &self.config, peer_id, now, &actor);
        }
        self.persist_locked(&database)?;
        Ok(output)
    }

    pub fn ingest_heartbeat(&self, input: PeerHeartbeatInput) -> Result<PeerRecord, String> {
        if input.protocol_version != TRANSIT_PROTOCOL_VERSION {
            return Err(format!("incompatible protocol_version={}", input.protocol_version));
        }
        if input.region_id == self.config.region.region_id {
            return Err("heartbeat claims the local region_id".to_string());
        }
        let now = Utc::now();
        let sent_at = parse_time(&input.sent_at, "sent_at")?;
        let latency_ms = now.signed_duration_since(sent_at).num_milliseconds().max(0) as f64;
        let mut database = self.inner.lock().map_err(|_| "state lock poisoned".to_string())?;
        let existing_id = database.peers.values().find(|peer| peer.region_id == input.region_id).map(|peer| peer.peer_id.clone());
        let peer_id = match existing_id {
            Some(peer_id) => peer_id,
            None if self.config.routing.allow_dynamic_peers => format!("dynamic-{}", sanitise_id(&input.region_id)),
            None => return Err("unknown peer region and dynamic peers are disabled".to_string()),
        };
        if database.peers.len() >= self.config.limits.max_peers && !database.peers.contains_key(&peer_id) {
            return Err("peer limit reached".to_string());
        }
        let record = database.peers.entry(peer_id.clone()).or_insert_with(|| PeerRecord {
            peer_id: peer_id.clone(),
            region_id: input.region_id.clone(),
            swmi_id: input.swmi_id.clone(),
            display_name: input.display_name.clone(),
            endpoint: input.advertised_endpoint.trim_end_matches('/').to_string(),
            protocol_version: input.protocol_version.clone(),
            priority: 10,
            capabilities: input.capabilities.clone(),
            admin_state: "enabled".to_string(),
            oper_state: "unknown".to_string(),
            dynamic: true,
            latency_ms: None,
            failure_count: 0,
            last_seen_at: None,
            last_heartbeat_sent_at: None,
            last_error: None,
            created_at: now,
            updated_at: now,
            notes: Some("Dynamically discovered through OPEN LAB heartbeat".to_string()),
        });
        record.swmi_id = input.swmi_id;
        record.display_name = input.display_name;
        record.endpoint = input.advertised_endpoint.trim_end_matches('/').to_string();
        record.protocol_version = input.protocol_version;
        record.capabilities = sorted_unique(input.capabilities);
        record.oper_state = if record.admin_state == "enabled" { "up".to_string() } else { record.oper_state.clone() };
        record.latency_ms = Some(latency_ms);
        record.failure_count = 0;
        record.last_seen_at = Some(now);
        record.last_error = None;
        record.updated_at = now;
        let output = record.clone();
        record_event(&mut database, &self.config, "debug", "peer", "heartbeat_received", &input.region_id, &peer_id, json!({"sequence":input.sequence,"latency_ms":latency_ms}));
        self.persist_locked(&database)?;
        Ok(output)
    }

    pub fn create_route(&self, input: RouteCreateInput) -> Result<RouteRecord, String> {
        validate_route_input(&input)?;
        let expires_at = input.expires_at.as_deref().map(|value| parse_time(value, "expires_at")).transpose()?;
        let now = Utc::now();
        let mut database = self.inner.lock().map_err(|_| "state lock poisoned".to_string())?;
        if !database.peers.contains_key(&input.peer_id) {
            return Err("peer_id does not exist".to_string());
        }
        if database.routes.len() >= self.config.limits.max_routes {
            return Err("route limit reached".to_string());
        }
        let route_id = Uuid::new_v4().to_string();
        let record = RouteRecord {
            route_id: route_id.clone(),
            service: normalise_service(&input.service)?,
            selector_type: input.selector_type.to_ascii_lowercase(),
            selector_value: input.selector_value,
            destination_region: input.destination_region,
            peer_id: input.peer_id,
            preference: input.preference.unwrap_or(100),
            metric: input.metric.unwrap_or(100),
            failover_group: input.failover_group,
            enabled: input.enabled.unwrap_or(true),
            learned: false,
            path_vector: vec![self.config.region.region_id.clone()],
            expires_at,
            created_at: now,
            updated_at: now,
            notes: input.notes,
        };
        database.routes.insert(route_id.clone(), record.clone());
        record_event(&mut database, &self.config, "info", "route", "route_created", "operator", &route_id, json!({"service":record.service.clone(),"destination_region":record.destination_region.clone(),"peer_id":record.peer_id.clone()}));
        self.persist_locked(&database)?;
        Ok(record)
    }

    pub fn route_action(&self, route_id: &str, action: &str, input: RouteActionInput) -> Result<RouteRecord, String> {
        let actor = input.actor.unwrap_or_else(|| "operator".to_string());
        let mut database = self.inner.lock().map_err(|_| "state lock poisoned".to_string())?;
        let route = database.routes.get_mut(route_id).ok_or_else(|| "route not found".to_string())?;
        match action {
            "enable" => route.enabled = true,
            "disable" => route.enabled = false,
            "prefer" => {
                route.preference = input.preference.unwrap_or(route.preference + 10);
                if let Some(metric) = input.metric {
                    route.metric = metric;
                }
            }
            _ => return Err("unsupported route action".to_string()),
        }
        route.updated_at = Utc::now();
        let output = route.clone();
        record_event(&mut database, &self.config, "info", "route", action, &actor, route_id, json!({"reason":input.reason,"preference":output.preference,"metric":output.metric}));
        self.persist_locked(&database)?;
        Ok(output)
    }

    pub fn delete_route(&self, route_id: &str) -> Result<(), String> {
        let mut database = self.inner.lock().map_err(|_| "state lock poisoned".to_string())?;
        if database.routes.remove(route_id).is_none() {
            return Err("route not found".to_string());
        }
        record_event(&mut database, &self.config, "warning", "route", "route_deleted", "operator", route_id, json!({}));
        self.persist_locked(&database)
    }

    pub fn update_subscriber_location(&self, input: SubscriberLocationInput) -> Result<SubscriberLocation, String> {
        validate_ssi(input.issi, "ISSI")?;
        let now = Utc::now();
        let key = input.issi.to_string();
        let mut database = self.inner.lock().map_err(|_| "state lock poisoned".to_string())?;
        let old_sequence = database.subscriber_locations.get(&key).map(|entry| entry.sequence).unwrap_or(0);
        let sequence = input.sequence.unwrap_or(old_sequence + 1);
        if sequence < old_sequence {
            return Err("stale subscriber location sequence".to_string());
        }
        let record = SubscriberLocation {
            issi: input.issi,
            home_region: input.home_region,
            current_region: input.current_region,
            serving_node: input.serving_node,
            sequence,
            source_peer: input.source_peer,
            updated_at: now,
        };
        database.subscriber_locations.insert(key.clone(), record.clone());
        record_event(&mut database, &self.config, "info", "mobility", "subscriber_location_updated", "core", &key, json!({"home_region":record.home_region.clone(),"current_region":record.current_region.clone(),"sequence":sequence}));
        self.persist_locked(&database)?;
        Ok(record)
    }

    pub fn update_group_reachability(&self, input: GroupReachabilityInput) -> Result<GroupReachability, String> {
        validate_ssi(input.gssi, "GSSI")?;
        let key = input.gssi.to_string();
        let mut regions = sorted_unique(input.regions);
        regions.retain(|region| !region.trim().is_empty());
        let record = GroupReachability {
            gssi: input.gssi,
            regions,
            source_peer: input.source_peer,
            updated_at: Utc::now(),
        };
        let mut database = self.inner.lock().map_err(|_| "state lock poisoned".to_string())?;
        database.group_reachability.insert(key.clone(), record.clone());
        record_event(&mut database, &self.config, "info", "group", "group_reachability_updated", "core", &key, json!({"regions":record.regions.clone()}));
        self.persist_locked(&database)?;
        Ok(record)
    }

    pub fn resolve(&self, input: RouteResolveInput) -> Result<RouteDecision, String> {
        let database = self.inner.lock().map_err(|_| "state lock poisoned".to_string())?;
        let target = determine_single_target(&database, &input.service, &input.destination_kind, &input.destination, input.target_region.as_deref())?;
        Ok(resolve_route(&database, &self.config, &input.service, &input.destination_kind, &input.destination, &target, &input.trace))
    }

    pub fn submit(&self, input: TransitSubmitInput) -> Result<SubmitResult, String> {
        let service = normalise_service(&input.service)?;
        validate_priority(input.priority.unwrap_or(5))?;
        validate_address(&input.destination_kind, &input.destination)?;
        let now = Utc::now();
        let ttl_secs = input.ttl_secs.unwrap_or(default_ttl(&service)).clamp(5, 86_400);
        let expires_at = now + Duration::seconds(ttl_secs as i64);
        let mut database = self.inner.lock().map_err(|_| "state lock poisoned".to_string())?;
        if database.outbound.len() >= self.config.limits.max_envelopes {
            return Err("outbound envelope limit reached".to_string());
        }
        let targets = determine_targets(&database, &service, &input.destination_kind, &input.destination, input.target_region.as_deref(), &self.config.region.region_id)?;
        if targets.is_empty() {
            return Err("no target region can be determined".to_string());
        }
        let session_id = input.session_id.clone().unwrap_or_else(|| Uuid::new_v4().to_string());
        let trace = vec![self.config.region.region_id.clone()];
        let mut decisions = Vec::new();
        let mut envelope_ids = Vec::new();
        let mut legs = Vec::new();
        for target_region in targets {
            if target_region == self.config.region.region_id {
                let envelope_id = Uuid::new_v4().to_string();
                let delivery = make_local_delivery_from_submit(&input, &service, &session_id, &envelope_id, &trace, now);
                let delivery_id = delivery.delivery_id.clone();
                database.local_deliveries.insert(delivery_id, delivery);
                envelope_ids.push(envelope_id);
                decisions.push(RouteDecision {
                    accepted: true,
                    target_region: Some(target_region.clone()),
                    selected_peer: None,
                    backup_peers: Vec::new(),
                    route_id: None,
                    reason: "destination is local region".to_string(),
                    candidate_count: 0,
                    trace: trace.clone(),
                });
                legs.push(SessionLeg {
                    target_region,
                    selected_peer: None,
                    backup_peers: Vec::new(),
                    state: "local_delivery".to_string(),
                    failover_count: 0,
                    last_error: None,
                    updated_at: now,
                });
                continue;
            }
            let decision = resolve_route(&database, &self.config, &service, &input.destination_kind, &input.destination, &target_region, &trace);
            if !decision.accepted {
                decisions.push(decision.clone());
                legs.push(SessionLeg {
                    target_region,
                    selected_peer: None,
                    backup_peers: Vec::new(),
                    state: "unroutable".to_string(),
                    failover_count: 0,
                    last_error: Some(decision.reason.clone()),
                    updated_at: now,
                });
                continue;
            }
            let envelope_id = Uuid::new_v4().to_string();
            let dedupe_key = input.correlation_id.clone().unwrap_or_else(|| envelope_id.clone());
            let state = if self.config.region.operating_mode == MODE_AUTHORITATIVE { "queued" } else { "shadow" };
            let envelope = OutboundEnvelope {
                protocol_version: TRANSIT_PROTOCOL_VERSION.to_string(),
                envelope_id: envelope_id.clone(),
                dedupe_key,
                service: service.clone(),
                operation: input.operation.clone(),
                origin_region: self.config.region.region_id.clone(),
                previous_hop_region: self.config.region.region_id.clone(),
                target_region: target_region.clone(),
                source_kind: input.source_kind.clone(),
                source: input.source.clone(),
                destination_kind: input.destination_kind.clone(),
                destination: input.destination.clone(),
                session_id: session_id.clone(),
                correlation_id: input.correlation_id.clone(),
                priority: input.priority.unwrap_or(5),
                trace: trace.clone(),
                hop_count: 0,
                created_at: now,
                expires_at,
                payload: input.payload.clone(),
                selected_peer: decision.selected_peer.clone(),
                backup_peers: decision.backup_peers.clone(),
                route_id: decision.route_id.clone(),
                state: state.to_string(),
                attempts: 0,
                next_attempt_at: now,
                last_attempt_at: None,
                delivered_at: None,
                last_error: None,
            };
            database.outbound.insert(envelope_id.clone(), envelope);
            envelope_ids.push(envelope_id);
            legs.push(SessionLeg {
                target_region,
                selected_peer: decision.selected_peer.clone(),
                backup_peers: decision.backup_peers.clone(),
                state: state.to_string(),
                failover_count: 0,
                last_error: None,
                updated_at: now,
            });
            decisions.push(decision);
        }
        if envelope_ids.is_empty() {
            return Err("all target regions are unroutable".to_string());
        }
        let session = database.sessions.entry(session_id.clone()).or_insert_with(|| SessionRecord {
            session_id: session_id.clone(),
            service: service.clone(),
            source_kind: input.source_kind.clone(),
            source: input.source.clone(),
            destination_kind: input.destination_kind.clone(),
            destination: input.destination.clone(),
            origin_region: self.config.region.region_id.clone(),
            correlation_id: input.correlation_id.clone(),
            state: if self.config.region.operating_mode == MODE_AUTHORITATIVE { "routing".to_string() } else { "shadow".to_string() },
            legs: Vec::new(),
            envelope_count: 0,
            created_at: now,
            updated_at: now,
            closed_at: None,
            last_error: None,
        });
        merge_legs(&mut session.legs, legs);
        session.envelope_count += envelope_ids.len() as u64;
        session.updated_at = now;
        record_event(&mut database, &self.config, "info", "transit", "transit_submitted", "local-core", &session_id, json!({"service":service.clone(),"destination":input.destination.clone(),"envelopes":envelope_ids.clone(),"mode":self.config.region.operating_mode.clone()}));
        self.persist_locked(&database)?;
        Ok(SubmitResult {
            accepted: true,
            session_id,
            envelope_ids,
            decisions,
            operating_mode: self.config.region.operating_mode.clone(),
        })
    }

    pub fn ingest_envelope(&self, input: TransitEnvelopeInput) -> Result<IngestResult, String> {
        if input.protocol_version != TRANSIT_PROTOCOL_VERSION {
            return Err(format!("unsupported transit protocol {}", input.protocol_version));
        }
        let now = Utc::now();
        let created_at = parse_time(&input.created_at, "created_at")?;
        let expires_at = parse_time(&input.expires_at, "expires_at")?;
        if expires_at <= now {
            return Err("transit envelope expired".to_string());
        }
        if input.hop_count >= self.config.routing.max_hops {
            return Err("maximum transit hop count reached".to_string());
        }
        let mut database = self.inner.lock().map_err(|_| "state lock poisoned".to_string())?;
        let previous_peer = database.peers.values().find(|peer| peer.region_id == input.previous_hop_region).cloned();
        if previous_peer.is_none() && !self.config.routing.allow_dynamic_peers {
            return Err("previous hop is not a configured peer".to_string());
        }
        if input.trace.iter().any(|region| region == &self.config.region.region_id) {
            record_event(&mut database, &self.config, "error", "routing", "loop_rejected", &input.previous_hop_region, &input.envelope_id, json!({"trace":input.trace}));
            self.persist_locked(&database)?;
            return Err("regional loop detected".to_string());
        }
        let duplicate_of = database
            .dedupe
            .get(&input.dedupe_key)
            .filter(|entry| entry.expires_at > now)
            .map(|entry| entry.envelope_id.clone());
        if let Some(original_envelope) = duplicate_of {
            record_event(&mut database, &self.config, "debug", "routing", "duplicate_suppressed", &input.previous_hop_region, &input.envelope_id, json!({"dedupe_key":input.dedupe_key.clone(),"original_envelope":original_envelope}));
            self.persist_locked(&database)?;
            return Ok(IngestResult {
                accepted: true,
                duplicate: true,
                local_delivery_id: None,
                forwarded_envelope_id: None,
                route: None,
            });
        }
        database.dedupe.insert(input.dedupe_key.clone(), DedupeEntry {
            dedupe_key: input.dedupe_key.clone(),
            envelope_id: input.envelope_id.clone(),
            expires_at: now + Duration::seconds(self.config.routing.dedupe_ttl_secs as i64),
        });
        let mut trace = input.trace.clone();
        trace.push(self.config.region.region_id.clone());
        upsert_session_from_inbound(&mut database, &input, now);
        if input.target_region == self.config.region.region_id {
            if database.local_deliveries.len() >= self.config.limits.max_local_deliveries {
                return Err("local delivery queue limit reached".to_string());
            }
            let delivery_id = Uuid::new_v4().to_string();
            let delivery = LocalDelivery {
                delivery_id: delivery_id.clone(),
                envelope_id: input.envelope_id.clone(),
                service: input.service.clone(),
                operation: input.operation.clone(),
                source_region: input.origin_region.clone(),
                source_kind: input.source_kind.clone(),
                source: input.source.clone(),
                destination_kind: input.destination_kind.clone(),
                destination: input.destination.clone(),
                session_id: input.session_id.clone(),
                correlation_id: input.correlation_id.clone(),
                priority: input.priority,
                payload: input.payload.clone(),
                trace,
                state: "pending".to_string(),
                created_at: now,
                acknowledged_at: None,
                last_error: None,
            };
            database.local_deliveries.insert(delivery_id.clone(), delivery);
            record_event(&mut database, &self.config, "info", "transit", "local_delivery_queued", &input.previous_hop_region, &delivery_id, json!({"service":input.service,"session_id":input.session_id}));
            self.persist_locked(&database)?;
            return Ok(IngestResult {
                accepted: true,
                duplicate: false,
                local_delivery_id: Some(delivery_id),
                forwarded_envelope_id: None,
                route: None,
            });
        }
        if !self.config.routing.allow_transitive_routing {
            return Err("envelope is not for the local region and transitive routing is disabled".to_string());
        }
        let decision = resolve_route(&database, &self.config, &input.service, &input.destination_kind, &input.destination, &input.target_region, &trace);
        if !decision.accepted {
            record_event(&mut database, &self.config, "error", "routing", "forward_unroutable", &input.previous_hop_region, &input.envelope_id, json!({"reason":decision.reason.clone(),"target_region":input.target_region.clone()}));
            self.persist_locked(&database)?;
            return Err(decision.reason);
        }
        let state = if self.config.region.operating_mode == MODE_AUTHORITATIVE { "queued" } else { "shadow" };
        let envelope = OutboundEnvelope {
            protocol_version: input.protocol_version,
            envelope_id: input.envelope_id.clone(),
            dedupe_key: input.dedupe_key,
            service: input.service,
            operation: input.operation,
            origin_region: input.origin_region,
            previous_hop_region: self.config.region.region_id.clone(),
            target_region: input.target_region,
            source_kind: input.source_kind,
            source: input.source,
            destination_kind: input.destination_kind,
            destination: input.destination,
            session_id: input.session_id,
            correlation_id: input.correlation_id,
            priority: input.priority,
            trace,
            hop_count: input.hop_count + 1,
            created_at,
            expires_at,
            payload: input.payload,
            selected_peer: decision.selected_peer.clone(),
            backup_peers: decision.backup_peers.clone(),
            route_id: decision.route_id.clone(),
            state: state.to_string(),
            attempts: 0,
            next_attempt_at: now,
            last_attempt_at: None,
            delivered_at: None,
            last_error: None,
        };
        database.outbound.insert(input.envelope_id.clone(), envelope);
        record_event(&mut database, &self.config, "info", "transit", "envelope_forwarded", &input.previous_hop_region, &input.envelope_id, json!({"selected_peer":decision.selected_peer.clone(),"target_region":decision.target_region.clone(),"mode":self.config.region.operating_mode.clone()}));
        self.persist_locked(&database)?;
        Ok(IngestResult {
            accepted: true,
            duplicate: false,
            local_delivery_id: None,
            forwarded_envelope_id: Some(input.envelope_id),
            route: Some(decision),
        })
    }

    pub fn acknowledge_local_delivery(&self, delivery_id: &str, input: DeliveryAckInput) -> Result<LocalDelivery, String> {
        let now = Utc::now();
        let actor = input.actor.unwrap_or_else(|| "local-core".to_string());
        let mut database = self.inner.lock().map_err(|_| "state lock poisoned".to_string())?;
        let delivery = database.local_deliveries.get_mut(delivery_id).ok_or_else(|| "delivery not found".to_string())?;
        delivery.state = if input.success { "acknowledged" } else { "failed" }.to_string();
        delivery.acknowledged_at = Some(now);
        delivery.last_error = input.error.clone();
        let output = delivery.clone();
        if let Some(session) = database.sessions.get_mut(&output.session_id) {
            session.updated_at = now;
            if !input.success {
                session.last_error = input.error.clone();
            }
        }
        record_event(&mut database, &self.config, if input.success { "info" } else { "error" }, "delivery", "local_delivery_ack", &actor, delivery_id, json!({"success":input.success,"error":input.error}));
        self.persist_locked(&database)?;
        Ok(output)
    }

    pub fn session_action(&self, session_id: &str, action: &str, input: SessionActionInput) -> Result<SessionRecord, String> {
        let now = Utc::now();
        let actor = input.actor.unwrap_or_else(|| "operator".to_string());
        let mut database = self.inner.lock().map_err(|_| "state lock poisoned".to_string())?;
        match action {
            "close" | "cleanup" => {
                let session = database.sessions.get_mut(session_id).ok_or_else(|| "session not found".to_string())?;
                session.state = "closed".to_string();
                session.closed_at = Some(now);
                session.updated_at = now;
                if let Some(reason) = input.reason.clone() {
                    session.last_error = Some(reason);
                }
                for envelope in database.outbound.values_mut().filter(|envelope| envelope.session_id == session_id && !matches!(envelope.state.as_str(), "delivered" | "failed" | "expired")) {
                    envelope.state = "cancelled".to_string();
                    envelope.last_error = Some("session closed by operator".to_string());
                }
            }
            "failover" => {
                force_session_failover(&mut database, &self.config, session_id, now, &actor)?;
            }
            _ => return Err("unsupported session action".to_string()),
        }
        let output = database.sessions.get(session_id).cloned().ok_or_else(|| "session not found".to_string())?;
        record_event(&mut database, &self.config, "warning", "session", action, &actor, session_id, json!({"reason":input.reason}));
        self.persist_locked(&database)?;
        Ok(output)
    }

    pub fn due_outbound(&self, limit: usize) -> Vec<OutboundEnvelope> {
        if self.config.region.operating_mode != MODE_AUTHORITATIVE {
            return Vec::new();
        }
        let database = self.inner.lock().expect("transit state poisoned");
        let now = Utc::now();
        let mut values: Vec<_> = database
            .outbound
            .values()
            .filter(|entry| matches!(entry.state.as_str(), "queued" | "retry") && entry.next_attempt_at <= now && entry.expires_at > now && entry.selected_peer.is_some())
            .cloned()
            .collect();
        values.sort_by(|left, right| right.priority.cmp(&left.priority).then(left.next_attempt_at.cmp(&right.next_attempt_at)));
        values.truncate(limit);
        values
    }

    pub fn mark_outbound_attempt(&self, envelope_id: &str) -> Result<OutboundEnvelope, String> {
        let now = Utc::now();
        let mut database = self.inner.lock().map_err(|_| "state lock poisoned".to_string())?;
        let envelope = database.outbound.get_mut(envelope_id).ok_or_else(|| "envelope not found".to_string())?;
        if !matches!(envelope.state.as_str(), "queued" | "retry") {
            return Err("envelope is not dispatchable".to_string());
        }
        envelope.state = "in_flight".to_string();
        envelope.attempts += 1;
        envelope.last_attempt_at = Some(now);
        let output = envelope.clone();
        self.persist_locked(&database)?;
        Ok(output)
    }

    pub fn complete_outbound(&self, envelope_id: &str, success: bool, error: Option<String>, latency_ms: Option<f64>) -> Result<OutboundEnvelope, String> {
        let now = Utc::now();
        let mut database = self.inner.lock().map_err(|_| "state lock poisoned".to_string())?;
        let current = database.outbound.get(envelope_id).cloned().ok_or_else(|| "envelope not found".to_string())?;
        let peer_id = current.selected_peer.clone();
        if success {
            if let Some(envelope) = database.outbound.get_mut(envelope_id) {
                envelope.state = "delivered".to_string();
                envelope.delivered_at = Some(now);
                envelope.last_error = None;
            }
            if let Some(peer_id) = peer_id.as_deref() {
                mark_peer_success(&mut database, peer_id, latency_ms, now);
            }
            update_session_leg_state(&mut database, &current.session_id, &current.target_region, "active", None, now);
            record_event(&mut database, &self.config, "info", "transport", "envelope_delivered", peer_id.as_deref().unwrap_or("unknown"), envelope_id, json!({"attempts":current.attempts,"latency_ms":latency_ms}));
        } else {
            if let Some(peer_id) = peer_id.as_deref() {
                mark_peer_failure(&mut database, peer_id, error.clone(), now);
            }
            let backup = choose_backup_peer(&database, &self.config, &current);
            enum FailureOutcome {
                Failover(String),
                Failed,
                Retry,
            }
            let outcome = {
                let envelope = database.outbound.get_mut(envelope_id).ok_or_else(|| "envelope not found".to_string())?;
                envelope.last_error = error.clone();
                if let Some(backup_peer) = backup.clone() {
                    let failed_peer = envelope.selected_peer.take();
                    envelope.selected_peer = Some(backup_peer.clone());
                    envelope.backup_peers.retain(|peer| peer != &backup_peer);
                    if let Some(failed_peer) = failed_peer {
                        envelope.backup_peers.push(failed_peer);
                    }
                    envelope.state = "retry".to_string();
                    envelope.next_attempt_at = now;
                    FailureOutcome::Failover(backup_peer)
                } else if envelope.attempts >= self.config.transport.max_attempts || envelope.expires_at <= now {
                    envelope.state = "failed".to_string();
                    FailureOutcome::Failed
                } else {
                    envelope.state = "retry".to_string();
                    envelope.next_attempt_at = now + Duration::seconds(self.config.transport.retry_backoff_secs as i64 * envelope.attempts.max(1) as i64);
                    FailureOutcome::Retry
                }
            };
            match outcome {
                FailureOutcome::Failover(backup_peer) => {
                    increment_session_failover(&mut database, &current.session_id, &current.target_region, &backup_peer, error.clone(), now);
                    record_event(&mut database, &self.config, "warning", "routing", "automatic_failover", "transport", envelope_id, json!({"new_peer":backup_peer,"error":error.clone()}));
                }
                FailureOutcome::Failed => {
                    update_session_leg_state(&mut database, &current.session_id, &current.target_region, "failed", error.clone(), now);
                }
                FailureOutcome::Retry => {
                    update_session_leg_state(&mut database, &current.session_id, &current.target_region, "degraded", error.clone(), now);
                }
            }
            record_event(&mut database, &self.config, "error", "transport", "envelope_delivery_failed", peer_id.as_deref().unwrap_or("unknown"), envelope_id, json!({"error":error,"backup":backup}));
        }
        let output = database.outbound.get(envelope_id).cloned().ok_or_else(|| "envelope disappeared".to_string())?;
        self.persist_locked(&database)?;
        Ok(output)
    }

    pub fn peers_due_for_heartbeat(&self) -> Vec<PeerRecord> {
        if self.config.region.operating_mode != MODE_AUTHORITATIVE {
            return Vec::new();
        }
        let database = self.inner.lock().expect("transit state poisoned");
        let now = Utc::now();
        let interval = Duration::seconds(self.config.transport.heartbeat_interval_secs as i64);
        database
            .peers
            .values()
            .filter(|peer| peer.admin_state == "enabled" && peer.protocol_version == TRANSIT_PROTOCOL_VERSION)
            .filter(|peer| peer.last_heartbeat_sent_at.as_ref().is_none_or(|last| now.signed_duration_since(last.clone()) >= interval))
            .cloned()
            .collect()
    }

    pub fn heartbeat_payload(&self) -> PeerHeartbeatInput {
        let mut database = self.inner.lock().expect("transit state poisoned");
        let sequence = database.next_heartbeat_sequence;
        database.next_heartbeat_sequence += 1;
        PeerHeartbeatInput {
            region_id: self.config.region.region_id.clone(),
            swmi_id: self.config.region.swmi_id.clone(),
            display_name: self.config.region.display_name.clone(),
            advertised_endpoint: self.config.region.advertised_endpoint.clone(),
            protocol_version: TRANSIT_PROTOCOL_VERSION.to_string(),
            capabilities: self.config.region.capabilities.clone(),
            sent_at: Utc::now().to_rfc3339(),
            sequence,
        }
    }

    pub fn record_heartbeat_result(&self, peer_id: &str, success: bool, error: Option<String>, latency_ms: Option<f64>) -> Result<(), String> {
        let now = Utc::now();
        let mut database = self.inner.lock().map_err(|_| "state lock poisoned".to_string())?;
        let became_down = {
            let peer = database.peers.get_mut(peer_id).ok_or_else(|| "peer not found".to_string())?;
            peer.last_heartbeat_sent_at = Some(now);
            if success {
                peer.oper_state = "up".to_string();
                peer.failure_count = 0;
                peer.last_error = None;
                if let Some(latency) = latency_ms {
                    peer.latency_ms = Some(latency);
                }
            } else {
                peer.failure_count += 1;
                peer.last_error = error.clone();
                peer.oper_state = if peer.failure_count >= self.config.transport.max_attempts { "down" } else { "degraded" }.to_string();
            }
            peer.updated_at = now;
            !success && peer.oper_state == "down"
        };
        if became_down {
            failover_from_peer(&mut database, &self.config, peer_id, now, "heartbeat");
        }
        self.persist_locked(&database)
    }

    pub fn maintenance_tick(&self, _input: MaintenanceInput) -> Result<TransitStatus, String> {
        let now = Utc::now();
        let mut database = self.inner.lock().map_err(|_| "state lock poisoned".to_string())?;
        let timeout = Duration::seconds(self.config.transport.peer_timeout_secs as i64);
        let mut down_peers = Vec::new();
        for peer in database.peers.values_mut() {
            if peer.admin_state == "enabled" && peer.last_seen_at.as_ref().is_some_and(|last| now.signed_duration_since(last.clone()) > timeout) {
                if peer.oper_state != "down" {
                    peer.oper_state = "down".to_string();
                    peer.last_error = Some("peer heartbeat timeout".to_string());
                    peer.updated_at = now;
                    down_peers.push(peer.peer_id.clone());
                }
            }
        }
        for peer_id in down_peers {
            failover_from_peer(&mut database, &self.config, &peer_id, now, "maintenance");
            record_event(&mut database, &self.config, "error", "peer", "peer_timeout", "maintenance", &peer_id, json!({}));
        }
        prune_database(&mut database, &self.config, now);
        database.last_maintenance_at = Some(now);
        self.persist_locked(&database)?;
        drop(database);
        Ok(self.status())
    }

    pub fn backup(&self) -> Result<BackupResult, String> {
        let database = self.inner.lock().map_err(|_| "state lock poisoned".to_string())?;
        let bytes = serde_json::to_vec_pretty(&*database).map_err(|error| error.to_string())?;
        write_atomic(&self.config.storage.backup_path, &bytes)?;
        Ok(BackupResult {
            path: self.config.storage.backup_path.display().to_string(),
            bytes: bytes.len() as u64,
            created_at: Utc::now(),
        })
    }

    pub fn export(&self) -> TransitDatabase {
        self.inner.lock().expect("transit state poisoned").clone()
    }

    pub fn metrics(&self) -> String {
        let status = self.status();
        format!(
            concat!(
                "# HELP netcore_transit_peers Transit peers by state.\n",
                "# TYPE netcore_transit_peers gauge\n",
                "netcore_transit_peers{{state=\"up\"}} {}\n",
                "netcore_transit_peers{{state=\"degraded\"}} {}\n",
                "netcore_transit_peers{{state=\"blocked\"}} {}\n",
                "# HELP netcore_transit_routes Configured regional routes.\n",
                "# TYPE netcore_transit_routes gauge\n",
                "netcore_transit_routes {}\n",
                "# HELP netcore_transit_sessions_active Active transit sessions.\n",
                "# TYPE netcore_transit_sessions_active gauge\n",
                "netcore_transit_sessions_active {}\n",
                "# HELP netcore_transit_outbound_pending Pending outbound envelopes.\n",
                "# TYPE netcore_transit_outbound_pending gauge\n",
                "netcore_transit_outbound_pending {}\n",
                "# HELP netcore_transit_local_deliveries_pending Pending local deliveries.\n",
                "# TYPE netcore_transit_local_deliveries_pending gauge\n",
                "netcore_transit_local_deliveries_pending {}\n",
                "# HELP netcore_transit_loop_rejections Rejected regional loops.\n",
                "# TYPE netcore_transit_loop_rejections counter\n",
                "netcore_transit_loop_rejections {}\n",
                "# HELP netcore_transit_authoritative Whether peer transmission is authoritative.\n",
                "# TYPE netcore_transit_authoritative gauge\n",
                "netcore_transit_authoritative {}\n"
            ),
            status.peers_up,
            status.peers_degraded,
            status.peers_blocked,
            status.routes_total,
            status.sessions_active,
            status.outbound_pending,
            status.local_deliveries_pending,
            status.loop_rejections,
            usize::from(status.authoritative),
        )
    }

    fn persist(&self) -> Result<(), String> {
        let database = self.inner.lock().map_err(|_| "state lock poisoned".to_string())?;
        self.persist_locked(&database)
    }

    fn persist_locked(&self, database: &TransitDatabase) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(database).map_err(|error| error.to_string())?;
        write_atomic(&self.config.storage.database_path, &bytes)
    }
}

fn local_region_from_config(config: &TransitConfig, now: DateTime<Utc>) -> LocalRegionRecord {
    LocalRegionRecord {
        region_id: config.region.region_id.clone(),
        swmi_id: config.region.swmi_id.clone(),
        display_name: config.region.display_name.clone(),
        advertised_endpoint: config.region.advertised_endpoint.clone(),
        protocol_version: config.region.protocol_version.clone(),
        capabilities: sorted_unique(config.region.capabilities.clone()),
        updated_at: now,
    }
}

fn resolve_route(
    database: &TransitDatabase,
    config: &TransitConfig,
    service: &str,
    destination_kind: &str,
    destination: &str,
    target_region: &str,
    trace: &[String],
) -> RouteDecision {
    let mut candidates: Vec<(Option<RouteRecord>, PeerRecord, i32, u32)> = Vec::new();
    let now = Utc::now();
    for route in database.routes.values() {
        if !route.enabled || route.destination_region != target_region {
            continue;
        }
        if route.expires_at.as_ref().is_some_and(|expires| expires <= &now) {
            continue;
        }
        if !route_matches(route, service, destination_kind, destination, target_region) {
            continue;
        }
        let Some(peer) = database.peers.get(&route.peer_id) else {
            continue;
        };
        if peer_usable(peer, service, trace, config) {
            candidates.push((Some(route.clone()), peer.clone(), route.preference, route.metric));
        }
    }
    if config.routing.prefer_direct_region_peer {
        for peer in database.peers.values() {
            if peer.region_id == target_region && peer_usable(peer, service, trace, config) {
                candidates.push((None, peer.clone(), 10_000 + peer.priority, 0));
            }
        }
    }
    candidates.sort_by(|left, right| {
        right
            .2
            .cmp(&left.2)
            .then(left.3.cmp(&right.3))
            .then_with(|| {
                left.1
                    .latency_ms
                    .unwrap_or(f64::MAX)
                    .partial_cmp(&right.1.latency_ms.unwrap_or(f64::MAX))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then(right.1.priority.cmp(&left.1.priority))
    });
    let mut unique = Vec::new();
    let mut seen = HashSet::new();
    for candidate in candidates {
        if seen.insert(candidate.1.peer_id.clone()) {
            unique.push(candidate);
        }
    }
    let Some(first) = unique.first() else {
        return RouteDecision {
            accepted: false,
            target_region: Some(target_region.to_string()),
            selected_peer: None,
            backup_peers: Vec::new(),
            route_id: None,
            reason: "no healthy route or peer for target region".to_string(),
            candidate_count: 0,
            trace: trace.to_vec(),
        };
    };
    RouteDecision {
        accepted: true,
        target_region: Some(target_region.to_string()),
        selected_peer: Some(first.1.peer_id.clone()),
        backup_peers: unique.iter().skip(1).map(|entry| entry.1.peer_id.clone()).collect(),
        route_id: first.0.as_ref().map(|route| route.route_id.clone()),
        reason: if first.0.is_some() { "matched configured route" } else { "selected direct regional peer" }.to_string(),
        candidate_count: unique.len(),
        trace: trace.to_vec(),
    }
}

fn route_matches(route: &RouteRecord, service: &str, destination_kind: &str, destination: &str, target_region: &str) -> bool {
    if route.service != "*" && route.service != service {
        return false;
    }
    match route.selector_type.as_str() {
        "default" => true,
        "region" => route.selector_value == target_region,
        "issi" => destination_kind.eq_ignore_ascii_case("issi") && route.selector_value == destination,
        "gssi" => destination_kind.eq_ignore_ascii_case("gssi") && route.selector_value == destination,
        "prefix" => destination.starts_with(&route.selector_value),
        _ => false,
    }
}

fn peer_usable(peer: &PeerRecord, service: &str, trace: &[String], config: &TransitConfig) -> bool {
    if peer.admin_state != "enabled" || !matches!(peer.oper_state.as_str(), "up" | "degraded" | "unknown") {
        return false;
    }
    if peer.protocol_version != TRANSIT_PROTOCOL_VERSION {
        return false;
    }
    if trace.iter().any(|region| region == &peer.region_id) {
        return false;
    }
    if !peer.capabilities.is_empty() && !peer.capabilities.iter().any(|capability| capability == service || capability == "*") {
        return false;
    }
    trace.len() < config.routing.max_hops as usize
}

fn determine_targets(
    database: &TransitDatabase,
    service: &str,
    destination_kind: &str,
    destination: &str,
    explicit_target: Option<&str>,
    local_region: &str,
) -> Result<Vec<String>, String> {
    if let Some(target) = explicit_target.filter(|value| !value.trim().is_empty()) {
        return Ok(vec![target.to_string()]);
    }
    if destination_kind.eq_ignore_ascii_case("issi") {
        let issi = destination.parse::<u32>().map_err(|_| "destination ISSI is not numeric".to_string())?;
        validate_ssi(issi, "ISSI")?;
        if let Some(location) = database.subscriber_locations.get(&issi.to_string()) {
            return Ok(vec![location.current_region.clone()]);
        }
    }
    if destination_kind.eq_ignore_ascii_case("gssi") || service == "group_call" || service == "media" {
        let gssi = destination.parse::<u32>().map_err(|_| "destination GSSI is not numeric".to_string())?;
        validate_ssi(gssi, "GSSI")?;
        if let Some(group) = database.group_reachability.get(&gssi.to_string()) {
            let mut regions = group.regions.clone();
            if !regions.contains(&local_region.to_string()) && service == "group_call" {
                regions.retain(|region| region != local_region);
            }
            return Ok(sorted_unique(regions));
        }
    }
    let mut inferred: Vec<String> = database
        .routes
        .values()
        .filter(|route| route.enabled && route_matches(route, service, destination_kind, destination, &route.destination_region))
        .map(|route| route.destination_region.clone())
        .collect();
    inferred = sorted_unique(inferred);
    Ok(inferred)
}

fn determine_single_target(database: &TransitDatabase, service: &str, destination_kind: &str, destination: &str, explicit_target: Option<&str>) -> Result<String, String> {
    determine_targets(database, service, destination_kind, destination, explicit_target, "")?
        .into_iter()
        .next()
        .ok_or_else(|| "no target region can be determined".to_string())
}

fn make_local_delivery_from_submit(input: &TransitSubmitInput, service: &str, session_id: &str, envelope_id: &str, trace: &[String], now: DateTime<Utc>) -> LocalDelivery {
    LocalDelivery {
        delivery_id: Uuid::new_v4().to_string(),
        envelope_id: envelope_id.to_string(),
        service: service.to_string(),
        operation: input.operation.clone(),
        source_region: trace.first().cloned().unwrap_or_default(),
        source_kind: input.source_kind.clone(),
        source: input.source.clone(),
        destination_kind: input.destination_kind.clone(),
        destination: input.destination.clone(),
        session_id: session_id.to_string(),
        correlation_id: input.correlation_id.clone(),
        priority: input.priority.unwrap_or(5),
        payload: input.payload.clone(),
        trace: trace.to_vec(),
        state: "pending".to_string(),
        created_at: now,
        acknowledged_at: None,
        last_error: None,
    }
}

fn upsert_session_from_inbound(database: &mut TransitDatabase, input: &TransitEnvelopeInput, now: DateTime<Utc>) {
    let session = database.sessions.entry(input.session_id.clone()).or_insert_with(|| SessionRecord {
        session_id: input.session_id.clone(),
        service: input.service.clone(),
        source_kind: input.source_kind.clone(),
        source: input.source.clone(),
        destination_kind: input.destination_kind.clone(),
        destination: input.destination.clone(),
        origin_region: input.origin_region.clone(),
        correlation_id: input.correlation_id.clone(),
        state: "active".to_string(),
        legs: Vec::new(),
        envelope_count: 0,
        created_at: now,
        updated_at: now,
        closed_at: None,
        last_error: None,
    });
    session.envelope_count += 1;
    session.updated_at = now;
    if !session.legs.iter().any(|leg| leg.target_region == input.target_region) {
        session.legs.push(SessionLeg {
            target_region: input.target_region.clone(),
            selected_peer: None,
            backup_peers: Vec::new(),
            state: "received".to_string(),
            failover_count: 0,
            last_error: None,
            updated_at: now,
        });
    }
}

fn merge_legs(existing: &mut Vec<SessionLeg>, incoming: Vec<SessionLeg>) {
    for leg in incoming {
        if let Some(current) = existing.iter_mut().find(|current| current.target_region == leg.target_region) {
            *current = leg;
        } else {
            existing.push(leg);
        }
    }
}

fn choose_backup_peer(database: &TransitDatabase, config: &TransitConfig, envelope: &OutboundEnvelope) -> Option<String> {
    envelope.backup_peers.iter().find(|peer_id| {
        database.peers.get(*peer_id).is_some_and(|peer| peer_usable(peer, &envelope.service, &envelope.trace, config))
    }).cloned()
}

fn failover_from_peer(database: &mut TransitDatabase, config: &TransitConfig, peer_id: &str, now: DateTime<Utc>, actor: &str) {
    let envelope_ids: Vec<String> = database
        .outbound
        .values()
        .filter(|envelope| envelope.selected_peer.as_deref() == Some(peer_id) && matches!(envelope.state.as_str(), "queued" | "retry" | "in_flight"))
        .map(|envelope| envelope.envelope_id.clone())
        .collect();
    for envelope_id in envelope_ids {
        let Some(current) = database.outbound.get(&envelope_id).cloned() else {
            continue;
        };
        let backup = current.backup_peers.iter().find(|candidate| {
            database.peers.get(*candidate).is_some_and(|peer| peer_usable(peer, &current.service, &current.trace, config))
        }).cloned();
        let transition = {
            let Some(envelope) = database.outbound.get_mut(&envelope_id) else {
                continue;
            };
            if let Some(backup_peer) = backup.clone() {
                envelope.selected_peer = Some(backup_peer.clone());
                envelope.backup_peers.retain(|candidate| candidate != &backup_peer);
                envelope.backup_peers.push(peer_id.to_string());
                envelope.state = if config.region.operating_mode == MODE_AUTHORITATIVE { "retry" } else { "shadow" }.to_string();
                envelope.next_attempt_at = now;
                let message = format!("failover from {peer_id}");
                envelope.last_error = Some(message.clone());
                Some((backup_peer, message))
            } else {
                let message = format!("no backup route after peer {peer_id} became unavailable");
                envelope.state = "failed".to_string();
                envelope.last_error = Some(message.clone());
                None
            }
        };
        if let Some((backup_peer, message)) = transition {
            increment_session_failover(database, &current.session_id, &current.target_region, &backup_peer, Some(message), now);
        } else {
            update_session_leg_state(database, &current.session_id, &current.target_region, "failed", Some(format!("no backup route after peer {peer_id} became unavailable")), now);
        }
        record_event(database, config, "warning", "routing", "peer_failover", actor, &envelope_id, json!({"failed_peer":peer_id,"backup":backup}));
    }
}

fn force_session_failover(database: &mut TransitDatabase, config: &TransitConfig, session_id: &str, now: DateTime<Utc>, actor: &str) -> Result<(), String> {
    if !database.sessions.contains_key(session_id) {
        return Err("session not found".to_string());
    }
    let envelope_ids: Vec<String> = database.outbound.values().filter(|entry| entry.session_id == session_id && !matches!(entry.state.as_str(), "delivered" | "failed" | "expired" | "cancelled")).map(|entry| entry.envelope_id.clone()).collect();
    if envelope_ids.is_empty() {
        return Err("session has no failover-capable outbound envelope".to_string());
    }
    for envelope_id in envelope_ids {
        let current = database.outbound.get(&envelope_id).cloned().ok_or_else(|| "envelope not found".to_string())?;
        let Some(current_peer) = current.selected_peer.clone() else {
            continue;
        };
        let backup = current.backup_peers.iter().find(|candidate| database.peers.get(*candidate).is_some_and(|peer| peer_usable(peer, &current.service, &current.trace, config))).cloned();
        let Some(backup_peer) = backup else {
            continue;
        };
        if let Some(envelope) = database.outbound.get_mut(&envelope_id) {
            envelope.selected_peer = Some(backup_peer.clone());
            envelope.backup_peers.retain(|candidate| candidate != &backup_peer);
            envelope.backup_peers.push(current_peer);
            envelope.state = if config.region.operating_mode == MODE_AUTHORITATIVE { "retry" } else { "shadow" }.to_string();
            envelope.next_attempt_at = now;
            envelope.last_error = Some("controlled operator failover".to_string());
        }
        increment_session_failover(database, session_id, &current.target_region, &backup_peer, Some("controlled operator failover".to_string()), now);
        record_event(database, config, "warning", "routing", "controlled_failover", actor, &envelope_id, json!({"new_peer":backup_peer}));
    }
    Ok(())
}

fn increment_session_failover(database: &mut TransitDatabase, session_id: &str, target_region: &str, new_peer: &str, error: Option<String>, now: DateTime<Utc>) {
    if let Some(session) = database.sessions.get_mut(session_id) {
        session.updated_at = now;
        if let Some(leg) = session.legs.iter_mut().find(|leg| leg.target_region == target_region) {
            leg.selected_peer = Some(new_peer.to_string());
            leg.failover_count += 1;
            leg.state = "failover".to_string();
            leg.last_error = error;
            leg.updated_at = now;
        }
    }
}

fn update_session_leg_state(database: &mut TransitDatabase, session_id: &str, target_region: &str, state: &str, error: Option<String>, now: DateTime<Utc>) {
    if let Some(session) = database.sessions.get_mut(session_id) {
        session.updated_at = now;
        if let Some(leg) = session.legs.iter_mut().find(|leg| leg.target_region == target_region) {
            leg.state = state.to_string();
            leg.last_error = error.clone();
            leg.updated_at = now;
        }
        if state == "failed" {
            session.last_error = error;
            if session.legs.iter().all(|leg| leg.state == "failed") {
                session.state = "failed".to_string();
            }
        } else if state == "active" {
            session.state = "active".to_string();
        }
    }
}

fn mark_peer_success(database: &mut TransitDatabase, peer_id: &str, latency_ms: Option<f64>, now: DateTime<Utc>) {
    if let Some(peer) = database.peers.get_mut(peer_id) {
        peer.oper_state = "up".to_string();
        peer.failure_count = 0;
        peer.last_seen_at = Some(now);
        peer.last_error = None;
        peer.updated_at = now;
        if let Some(latency) = latency_ms {
            peer.latency_ms = Some(latency);
        }
    }
}

fn mark_peer_failure(database: &mut TransitDatabase, peer_id: &str, error: Option<String>, now: DateTime<Utc>) {
    if let Some(peer) = database.peers.get_mut(peer_id) {
        peer.failure_count += 1;
        peer.oper_state = "degraded".to_string();
        peer.last_error = error;
        peer.updated_at = now;
    }
}

fn prune_database(database: &mut TransitDatabase, config: &TransitConfig, now: DateTime<Utc>) {
    database.dedupe.retain(|_, entry| entry.expires_at > now);
    database.routes.retain(|_, route| route.expires_at.as_ref().is_none_or(|expires| expires > &now));
    for envelope in database.outbound.values_mut() {
        if envelope.expires_at <= now && !matches!(envelope.state.as_str(), "delivered" | "failed" | "expired" | "cancelled") {
            envelope.state = "expired".to_string();
            envelope.last_error = Some("transit TTL expired".to_string());
        }
    }
    let session_ttl = Duration::seconds(config.routing.session_idle_ttl_secs as i64);
    for session in database.sessions.values_mut() {
        if now.signed_duration_since(session.updated_at.clone()) > session_ttl && !matches!(session.state.as_str(), "closed" | "failed" | "expired") {
            session.state = "expired".to_string();
            session.closed_at = Some(now);
        }
    }
    truncate_map_by_time(&mut database.outbound, config.limits.max_envelopes, |entry| entry.created_at);
    truncate_map_by_time(&mut database.local_deliveries, config.limits.max_local_deliveries, |entry| entry.created_at);
    truncate_map_by_time(&mut database.sessions, config.limits.max_sessions, |entry| entry.updated_at);
    if database.events.len() > config.limits.max_events {
        let drain = database.events.len() - config.limits.max_events;
        database.events.drain(0..drain);
    }
}

fn truncate_map_by_time<T, F>(map: &mut HashMap<String, T>, limit: usize, time: F)
where
    F: Fn(&T) -> DateTime<Utc>,
{
    if map.len() <= limit {
        return;
    }
    let mut entries: Vec<_> = map.iter().map(|(key, value)| (key.clone(), time(value))).collect();
    entries.sort_by_key(|entry| entry.1.clone());
    for (key, _) in entries.into_iter().take(map.len() - limit) {
        map.remove(&key);
    }
}

fn record_event(database: &mut TransitDatabase, config: &TransitConfig, severity: &str, category: &str, action: &str, actor: &str, target: &str, detail: Value) {
    let event = TransitEvent {
        sequence: database.next_event_sequence,
        timestamp: Utc::now(),
        severity: severity.to_string(),
        category: category.to_string(),
        action: action.to_string(),
        actor: actor.to_string(),
        target: target.to_string(),
        detail,
    };
    database.next_event_sequence += 1;
    database.events.push(event);
    if database.events.len() > config.limits.max_events {
        let drain = database.events.len() - config.limits.max_events;
        database.events.drain(0..drain);
    }
}

fn validate_route_input(input: &RouteCreateInput) -> Result<(), String> {
    normalise_service(&input.service)?;
    if !matches!(input.selector_type.to_ascii_lowercase().as_str(), "default" | "region" | "issi" | "gssi" | "prefix") {
        return Err("selector_type must be default, region, issi, gssi or prefix".to_string());
    }
    if input.destination_region.trim().is_empty() || input.peer_id.trim().is_empty() {
        return Err("destination_region and peer_id must not be empty".to_string());
    }
    if input.selector_type.eq_ignore_ascii_case("issi") {
        validate_address("issi", &input.selector_value)?;
    }
    if input.selector_type.eq_ignore_ascii_case("gssi") {
        validate_address("gssi", &input.selector_value)?;
    }
    Ok(())
}

fn normalise_service(value: &str) -> Result<String, String> {
    let service = value.trim().to_ascii_lowercase();
    if matches!(service.as_str(), "*" | "mobility" | "individual_call" | "group_call" | "sds" | "media" | "supplementary_service" | "packet_data") {
        Ok(service)
    } else {
        Err("unsupported service; use mobility, individual_call, group_call, sds, media, supplementary_service, packet_data or *".to_string())
    }
}

fn validate_address(kind: &str, value: &str) -> Result<(), String> {
    if kind.eq_ignore_ascii_case("issi") || kind.eq_ignore_ascii_case("gssi") {
        let parsed = value.parse::<u32>().map_err(|_| format!("{kind} address must be numeric"))?;
        validate_ssi(parsed, kind)?;
    } else if value.trim().is_empty() {
        return Err("destination must not be empty".to_string());
    }
    Ok(())
}

fn validate_ssi(value: u32, label: &str) -> Result<(), String> {
    if value > MAX_SSI {
        Err(format!("{label} exceeds the 24-bit SSI range"))
    } else {
        Ok(())
    }
}

fn validate_priority(value: u8) -> Result<(), String> {
    if value > 15 {
        Err("priority must be between 0 and 15".to_string())
    } else {
        Ok(())
    }
}

fn validate_identifier(value: &str, label: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{label} must not be empty"));
    }
    if !value.chars().all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')) {
        return Err(format!("{label} contains unsupported characters"));
    }
    Ok(())
}

fn parse_time(value: &str, label: &str) -> Result<DateTime<Utc>, String> {
    DateTime::parse_from_rfc3339(value)
        .map(|time| time.with_timezone(&Utc))
        .map_err(|error| format!("invalid {label}: {error}"))
}

fn default_ttl(service: &str) -> u64 {
    match service {
        "media" => 30,
        "group_call" | "individual_call" => 300,
        "mobility" => 60,
        "sds" => 900,
        _ => 300,
    }
}

fn sorted_unique(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

fn sanitise_id(value: &str) -> String {
    value.chars().map(|character| if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') { character } else { '-' }).collect()
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    let temporary = path.with_extension(format!("tmp-{}", Uuid::new_v4()));
    fs::write(&temporary, bytes).map_err(|error| format!("write {}: {error}", temporary.display()))?;
    fs::rename(&temporary, path).map_err(|error| format!("rename {} to {}: {error}", temporary.display(), path.display()))
}

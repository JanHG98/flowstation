use std::collections::{BTreeMap, VecDeque};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::net::Ipv4Addr;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::config::{parse_cidr, IpGatewayConfig, MODE_AUTHORITATIVE};
use crate::protocol::{
    BlockAddressInput, CaptureStartInput, FirewallRuleInput, NatRuleInput, PacketCoreContext,
    RouteRuleInput, StaticDnsInput,
};

const DATABASE_SCHEMA_VERSION: u32 = 1;
const OPEN_LAB_WARNING: &str =
    "OPEN LAB: no authentication, no tokens and no TLS; isolated test network only";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteRule {
    pub id: String,
    pub name: String,
    pub destination: String,
    pub gateway: Option<String>,
    pub interface: Option<String>,
    pub metric: Option<u32>,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NatRule {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub source_cidr: Option<String>,
    pub destination_cidr: Option<String>,
    pub protocol: Option<String>,
    pub destination_port: Option<u16>,
    pub out_interface: Option<String>,
    pub to_address: Option<String>,
    pub to_port: Option<u16>,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirewallRule {
    pub id: String,
    pub name: String,
    pub chain: String,
    pub action: String,
    pub protocol: String,
    pub source_cidr: Option<String>,
    pub destination_cidr: Option<String>,
    pub source_port: Option<u16>,
    pub destination_port: Option<u16>,
    pub in_interface: Option<String>,
    pub out_interface: Option<String>,
    pub priority: i32,
    pub log: bool,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticDnsRecord {
    pub id: String,
    pub name: String,
    pub address: String,
    pub source: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockedAddress {
    pub address: String,
    pub reason: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaptureState {
    Active,
    Stopped,
    Full,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureRecord {
    pub id: String,
    pub name: String,
    pub state: CaptureState,
    pub direction: String,
    pub host: Option<String>,
    pub protocol: Option<String>,
    pub port: Option<u16>,
    pub path: String,
    pub created_at: String,
    pub stopped_at: Option<String>,
    pub packet_count: u64,
    pub captured_bytes: u64,
    pub original_bytes: u64,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowRecord {
    pub id: String,
    pub key: String,
    pub direction: String,
    pub protocol: String,
    pub source: String,
    pub destination: String,
    pub source_port: Option<u16>,
    pub destination_port: Option<u16>,
    pub issi: Option<u32>,
    pub nsapi: Option<u8>,
    pub node_id: Option<String>,
    pub first_seen: String,
    pub last_seen: String,
    pub packets: u64,
    pub bytes: u64,
    pub blocked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayEventRecord {
    pub sequence: u64,
    pub timestamp: String,
    pub kind: String,
    pub detail: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GatewayDatabase {
    schema_version: u32,
    revision: u64,
    next_event_sequence: u64,
    routes: BTreeMap<String, RouteRule>,
    nat_rules: BTreeMap<String, NatRule>,
    firewall_rules: BTreeMap<String, FirewallRule>,
    dns_records: BTreeMap<String, StaticDnsRecord>,
    blocked_addresses: BTreeMap<String, BlockedAddress>,
    captures: BTreeMap<String, CaptureRecord>,
    flows: BTreeMap<String, FlowRecord>,
    events: VecDeque<GatewayEventRecord>,
}

impl Default for GatewayDatabase {
    fn default() -> Self {
        Self {
            schema_version: DATABASE_SCHEMA_VERSION,
            revision: 0,
            next_event_sequence: 1,
            routes: BTreeMap::new(),
            nat_rules: BTreeMap::new(),
            firewall_rules: BTreeMap::new(),
            dns_records: BTreeMap::new(),
            blocked_addresses: BTreeMap::new(),
            captures: BTreeMap::new(),
            flows: BTreeMap::new(),
            events: VecDeque::new(),
        }
    }
}

struct GatewayState {
    config: IpGatewayConfig,
    database: GatewayDatabase,
    contexts: BTreeMap<String, PacketCoreContext>,
    started_at: String,
    packet_core_connected: bool,
    packet_core_mode: Option<String>,
    packet_core_last_error: Option<String>,
    packet_core_last_seen: Option<String>,
    tun_open: bool,
    tun_name: String,
    tun_last_error: Option<String>,
    kernel_applied_revision: Option<u64>,
    kernel_last_reconcile: Option<String>,
    kernel_last_error: Option<String>,
    packets_uplink: u64,
    bytes_uplink: u64,
    packets_downlink: u64,
    bytes_downlink: u64,
    packets_dropped: u64,
    packet_core_deletes: u64,
    packet_core_downlinks: u64,
    dns_queries: u64,
    test_requests: u64,
}

#[derive(Clone)]
pub struct SharedGateway {
    inner: Arc<Mutex<GatewayState>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GatewayStatus {
    pub service: &'static str,
    pub version: &'static str,
    pub started_at: String,
    pub security_mode: &'static str,
    pub warning: &'static str,
    pub mode: String,
    pub authoritative: bool,
    pub packet_core_connected: bool,
    pub packet_core_mode: Option<String>,
    pub packet_core_last_error: Option<String>,
    pub packet_core_last_seen: Option<String>,
    pub tun_open: bool,
    pub tun_name: String,
    pub tun_last_error: Option<String>,
    pub kernel_revision: u64,
    pub kernel_applied_revision: Option<u64>,
    pub kernel_last_reconcile: Option<String>,
    pub kernel_last_error: Option<String>,
    pub contexts: usize,
    pub routes: usize,
    pub nat_rules: usize,
    pub firewall_rules: usize,
    pub blocked_addresses: usize,
    pub flows: usize,
    pub captures_active: usize,
    pub packets_uplink: u64,
    pub bytes_uplink: u64,
    pub packets_downlink: u64,
    pub bytes_downlink: u64,
    pub packets_dropped: u64,
    pub packet_core_deletes: u64,
    pub packet_core_downlinks: u64,
    pub dns_queries: u64,
    pub test_requests: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct KernelStateSnapshot {
    pub revision: u64,
    pub routes: Vec<RouteRule>,
    pub nat_rules: Vec<NatRule>,
    pub firewall_rules: Vec<FirewallRule>,
    pub blocked_addresses: Vec<BlockedAddress>,
}

#[derive(Debug, Clone)]
pub struct PacketObservation {
    pub source: Ipv4Addr,
    pub destination: Ipv4Addr,
    pub protocol: String,
    pub source_port: Option<u16>,
    pub destination_port: Option<u16>,
}

impl SharedGateway {
    pub fn load(config: IpGatewayConfig) -> Result<Self, Box<dyn std::error::Error>> {
        fs::create_dir_all(&config.capture.directory)?;
        let mut database = match fs::read(&config.storage.database_path) {
            Ok(bytes) => match serde_json::from_slice::<GatewayDatabase>(&bytes) {
                Ok(database) if database.schema_version == DATABASE_SCHEMA_VERSION => database,
                Ok(_) => return Err("unsupported IP Gateway database schema".into()),
                Err(error) => {
                    tracing::warn!("IP Gateway database invalid, trying backup: {error}");
                    read_backup(&config)?
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => GatewayDatabase::default(),
            Err(error) => return Err(error.into()),
        };
        for capture in database.captures.values_mut() {
            if capture.state == CaptureState::Active {
                capture.state = CaptureState::Stopped;
                capture.stopped_at = Some(now_iso());
                capture.last_error = Some("capture stopped during service restart".to_string());
            }
        }
        if config.dns.enabled || config.test_server.enabled {
            ensure_builtin_dns_records(&config, &mut database);
        }
        let gateway = Self {
            inner: Arc::new(Mutex::new(GatewayState {
                tun_name: config.interface.name.clone(),
                config,
                database,
                contexts: BTreeMap::new(),
                started_at: now_iso(),
                packet_core_connected: false,
                packet_core_mode: None,
                packet_core_last_error: None,
                packet_core_last_seen: None,
                tun_open: false,
                tun_last_error: None,
                kernel_applied_revision: None,
                kernel_last_reconcile: None,
                kernel_last_error: None,
                packets_uplink: 0,
                bytes_uplink: 0,
                packets_downlink: 0,
                bytes_downlink: 0,
                packets_dropped: 0,
                packet_core_deletes: 0,
                packet_core_downlinks: 0,
                dns_queries: 0,
                test_requests: 0,
            })),
        };
        gateway.persist()?;
        Ok(gateway)
    }

    pub fn status(&self) -> GatewayStatus {
        let state = self.lock();
        GatewayStatus {
            service: "netcore-ip-gateway",
            version: env!("CARGO_PKG_VERSION"),
            started_at: state.started_at.clone(),
            security_mode: "open_lab",
            warning: OPEN_LAB_WARNING,
            mode: state.config.interface.mode.clone(),
            authoritative: state.config.interface.mode == MODE_AUTHORITATIVE,
            packet_core_connected: state.packet_core_connected,
            packet_core_mode: state.packet_core_mode.clone(),
            packet_core_last_error: state.packet_core_last_error.clone(),
            packet_core_last_seen: state.packet_core_last_seen.clone(),
            tun_open: state.tun_open,
            tun_name: state.tun_name.clone(),
            tun_last_error: state.tun_last_error.clone(),
            kernel_revision: state.database.revision,
            kernel_applied_revision: state.kernel_applied_revision,
            kernel_last_reconcile: state.kernel_last_reconcile.clone(),
            kernel_last_error: state.kernel_last_error.clone(),
            contexts: state.contexts.len(),
            routes: state.database.routes.len(),
            nat_rules: state.database.nat_rules.len(),
            firewall_rules: state.database.firewall_rules.len(),
            blocked_addresses: state.database.blocked_addresses.len(),
            flows: state.database.flows.len(),
            captures_active: state
                .database
                .captures
                .values()
                .filter(|capture| capture.state == CaptureState::Active)
                .count(),
            packets_uplink: state.packets_uplink,
            bytes_uplink: state.bytes_uplink,
            packets_downlink: state.packets_downlink,
            bytes_downlink: state.bytes_downlink,
            packets_dropped: state.packets_dropped,
            packet_core_deletes: state.packet_core_deletes,
            packet_core_downlinks: state.packet_core_downlinks,
            dns_queries: state.dns_queries,
            test_requests: state.test_requests,
        }
    }

    pub fn packet_core_connected(&self, mode: String) {
        let mut state = self.lock();
        let changed = !state.packet_core_connected || state.packet_core_mode.as_deref() != Some(&mode);
        state.packet_core_connected = true;
        state.packet_core_mode = Some(mode.clone());
        state.packet_core_last_error = None;
        state.packet_core_last_seen = Some(now_iso());
        if changed {
            state.event("packet_core_connected", json!({"mode":mode}));
        }
    }

    pub fn packet_core_disconnected(&self, error: String) {
        let mut state = self.lock();
        let changed = state.packet_core_connected || state.packet_core_last_error.as_deref() != Some(&error);
        state.packet_core_connected = false;
        state.packet_core_last_error = Some(error.clone());
        if changed {
            state.event("packet_core_disconnected", json!({"error":error}));
        }
    }

    pub fn replace_contexts(&self, contexts: Vec<PacketCoreContext>) {
        let mut state = self.lock();
        state.contexts = contexts
            .into_iter()
            .map(|context| (context.ipv4.clone(), context))
            .collect();
        state.packet_core_last_seen = Some(now_iso());
    }

    pub fn contexts(&self) -> Vec<PacketCoreContext> {
        self.lock().contexts.values().cloned().collect()
    }

    pub fn context_by_ipv4(&self, address: Ipv4Addr) -> Option<PacketCoreContext> {
        self.lock().contexts.get(&address.to_string()).cloned()
    }

    pub fn tun_opened(&self, name: String) {
        let mut state = self.lock();
        state.tun_open = true;
        state.tun_name = name.clone();
        state.tun_last_error = None;
        state.event("tun_opened", json!({"interface":name}));
    }

    pub fn tun_closed(&self, error: Option<String>) {
        let mut state = self.lock();
        let changed = state.tun_open || state.tun_last_error != error;
        state.tun_open = false;
        state.tun_last_error = error.clone();
        if changed {
            state.event("tun_closed", json!({"error":error}));
        }
    }

    pub fn kernel_reconciled(&self, revision: u64, error: Option<String>) {
        let mut state = self.lock();
        state.kernel_last_reconcile = Some(now_iso());
        state.kernel_last_error = error.clone();
        if error.is_none() {
            state.kernel_applied_revision = Some(revision);
        }
        state.event(
            if error.is_none() {
                "kernel_reconciled"
            } else {
                "kernel_reconcile_failed"
            },
            json!({"revision":revision,"error":error}),
        );
    }

    pub fn record_packet_core_delete(&self) {
        let mut state = self.lock();
        state.packet_core_deletes = state.packet_core_deletes.saturating_add(1);
    }

    pub fn record_packet_core_downlink(&self) {
        let mut state = self.lock();
        state.packet_core_downlinks = state.packet_core_downlinks.saturating_add(1);
    }

    pub fn record_drop(&self, reason: &str, detail: Value) {
        let mut state = self.lock();
        state.packets_dropped = state.packets_dropped.saturating_add(1);
        state.event("packet_dropped", json!({"reason":reason,"detail":detail}));
    }

    pub fn record_packet(
        &self,
        direction: &str,
        packet: &[u8],
        context: Option<&PacketCoreContext>,
    ) -> Result<PacketObservation, String> {
        let observation = parse_ipv4(packet)?;
        let mut state = self.lock();
        match direction {
            "uplink" => {
                state.packets_uplink = state.packets_uplink.saturating_add(1);
                state.bytes_uplink = state.bytes_uplink.saturating_add(packet.len() as u64);
            }
            "downlink" => {
                state.packets_downlink = state.packets_downlink.saturating_add(1);
                state.bytes_downlink = state.bytes_downlink.saturating_add(packet.len() as u64);
            }
            _ => return Err("packet direction must be uplink or downlink".to_string()),
        }
        let inferred_context = if context.is_none() && direction == "downlink" {
            state.contexts.get(&observation.destination.to_string()).cloned()
        } else {
            None
        };
        let context = context.or(inferred_context.as_ref());
        let key = format!(
            "{}|{}|{}|{}|{}|{}",
            direction,
            observation.protocol,
            observation.source,
            observation.source_port.unwrap_or(0),
            observation.destination,
            observation.destination_port.unwrap_or(0)
        );
        let flow_id = state
            .database
            .flows
            .iter()
            .find_map(|(id, flow)| (flow.key == key).then(|| id.clone()))
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let now = now_iso();
        let blocked = state
            .database
            .blocked_addresses
            .contains_key(&observation.source.to_string())
            || state
                .database
                .blocked_addresses
                .contains_key(&observation.destination.to_string());
        let flow = state.database.flows.entry(flow_id.clone()).or_insert_with(|| FlowRecord {
            id: flow_id,
            key,
            direction: direction.to_string(),
            protocol: observation.protocol.clone(),
            source: observation.source.to_string(),
            destination: observation.destination.to_string(),
            source_port: observation.source_port,
            destination_port: observation.destination_port,
            issi: context.map(|context| context.issi),
            nsapi: context.map(|context| context.nsapi),
            node_id: context.map(|context| context.node_id.clone()),
            first_seen: now.clone(),
            last_seen: now.clone(),
            packets: 0,
            bytes: 0,
            blocked,
        });
        flow.last_seen = now;
        flow.packets = flow.packets.saturating_add(1);
        flow.bytes = flow.bytes.saturating_add(packet.len() as u64);
        flow.blocked = blocked;
        if flow.issi.is_none() {
            flow.issi = context.map(|context| context.issi);
            flow.nsapi = context.map(|context| context.nsapi);
            flow.node_id = context.map(|context| context.node_id.clone());
        }
        enforce_flow_limit(&mut state);
        capture_packet(&mut state, direction, packet, &observation);
        Ok(observation)
    }

    pub fn routes(&self) -> Vec<RouteRule> {
        self.lock().database.routes.values().cloned().collect()
    }

    pub fn nat_rules(&self) -> Vec<NatRule> {
        self.lock().database.nat_rules.values().cloned().collect()
    }

    pub fn firewall_rules(&self) -> Vec<FirewallRule> {
        self.lock().database.firewall_rules.values().cloned().collect()
    }

    pub fn dns_records(&self) -> Vec<StaticDnsRecord> {
        self.lock().database.dns_records.values().cloned().collect()
    }

    pub fn dns_lookup(&self, name: &str) -> Option<Ipv4Addr> {
        let name = name.trim_end_matches('.').to_ascii_lowercase();
        self.lock()
            .database
            .dns_records
            .values()
            .find(|record| record.name.eq_ignore_ascii_case(&name))
            .and_then(|record| record.address.parse().ok())
    }

    pub fn blocked_addresses(&self) -> Vec<BlockedAddress> {
        self.lock()
            .database
            .blocked_addresses
            .values()
            .cloned()
            .collect()
    }

    pub fn flows(&self, limit: usize) -> Vec<FlowRecord> {
        let mut flows: Vec<_> = self.lock().database.flows.values().cloned().collect();
        flows.sort_by(|left, right| right.last_seen.cmp(&left.last_seen));
        flows.truncate(limit);
        flows
    }

    pub fn captures(&self) -> Vec<CaptureRecord> {
        let mut captures: Vec<_> = self.lock().database.captures.values().cloned().collect();
        captures.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        captures
    }

    pub fn capture(&self, id: &str) -> Option<CaptureRecord> {
        self.lock().database.captures.get(id).cloned()
    }

    pub fn kernel_snapshot(&self) -> KernelStateSnapshot {
        let state = self.lock();
        KernelStateSnapshot {
            revision: state.database.revision,
            routes: state.database.routes.values().cloned().collect(),
            nat_rules: state.database.nat_rules.values().cloned().collect(),
            firewall_rules: state.database.firewall_rules.values().cloned().collect(),
            blocked_addresses: state
                .database
                .blocked_addresses
                .values()
                .cloned()
                .collect(),
        }
    }

    pub fn upsert_route(&self, id: Option<&str>, input: RouteRuleInput) -> Result<RouteRule, String> {
        validate_route(&input)?;
        let mut state = self.lock();
        let id = id.map(str::to_string).unwrap_or_else(|| Uuid::new_v4().to_string());
        let now = now_iso();
        let created_at = state
            .database
            .routes
            .get(&id)
            .map(|record| record.created_at.clone())
            .unwrap_or_else(|| now.clone());
        let record = RouteRule {
            id: id.clone(),
            name: input.name,
            destination: input.destination,
            gateway: input.gateway,
            interface: input.interface,
            metric: input.metric,
            enabled: input.enabled,
            created_at,
            updated_at: now,
        };
        state.database.routes.insert(id.clone(), record.clone());
        state.touch();
        state.event("route_upserted", json!({"id":id,"destination":record.destination}));
        state.persist().map_err(|error| error.to_string())?;
        Ok(record)
    }

    pub fn delete_route(&self, id: &str) -> Result<(), String> {
        self.delete_record(id, "route")
    }

    pub fn upsert_nat(&self, id: Option<&str>, input: NatRuleInput) -> Result<NatRule, String> {
        validate_nat(&input)?;
        let mut state = self.lock();
        let id = id.map(str::to_string).unwrap_or_else(|| Uuid::new_v4().to_string());
        let now = now_iso();
        let created_at = state
            .database
            .nat_rules
            .get(&id)
            .map(|record| record.created_at.clone())
            .unwrap_or_else(|| now.clone());
        let record = NatRule {
            id: id.clone(),
            name: input.name,
            kind: input.kind,
            source_cidr: input.source_cidr,
            destination_cidr: input.destination_cidr,
            protocol: input.protocol,
            destination_port: input.destination_port,
            out_interface: input.out_interface,
            to_address: input.to_address,
            to_port: input.to_port,
            enabled: input.enabled,
            created_at,
            updated_at: now,
        };
        state.database.nat_rules.insert(id.clone(), record.clone());
        state.touch();
        state.event("nat_rule_upserted", json!({"id":id,"kind":record.kind}));
        state.persist().map_err(|error| error.to_string())?;
        Ok(record)
    }

    pub fn delete_nat(&self, id: &str) -> Result<(), String> {
        self.delete_record(id, "nat")
    }

    pub fn upsert_firewall(
        &self,
        id: Option<&str>,
        input: FirewallRuleInput,
    ) -> Result<FirewallRule, String> {
        validate_firewall(&input)?;
        let mut state = self.lock();
        let id = id.map(str::to_string).unwrap_or_else(|| Uuid::new_v4().to_string());
        let now = now_iso();
        let created_at = state
            .database
            .firewall_rules
            .get(&id)
            .map(|record| record.created_at.clone())
            .unwrap_or_else(|| now.clone());
        let record = FirewallRule {
            id: id.clone(),
            name: input.name,
            chain: input.chain,
            action: input.action,
            protocol: input.protocol,
            source_cidr: input.source_cidr,
            destination_cidr: input.destination_cidr,
            source_port: input.source_port,
            destination_port: input.destination_port,
            in_interface: input.in_interface,
            out_interface: input.out_interface,
            priority: input.priority,
            log: input.log,
            enabled: input.enabled,
            created_at,
            updated_at: now,
        };
        state
            .database
            .firewall_rules
            .insert(id.clone(), record.clone());
        state.touch();
        state.event(
            "firewall_rule_upserted",
            json!({"id":id,"chain":record.chain,"action":record.action}),
        );
        state.persist().map_err(|error| error.to_string())?;
        Ok(record)
    }

    pub fn delete_firewall(&self, id: &str) -> Result<(), String> {
        self.delete_record(id, "firewall")
    }

    pub fn upsert_dns(&self, id: Option<&str>, input: StaticDnsInput) -> Result<StaticDnsRecord, String> {
        let name = normalise_dns_name(&input.name)?;
        let address = input
            .address
            .parse::<Ipv4Addr>()
            .map_err(|_| "DNS address must be IPv4".to_string())?
            .to_string();
        let mut state = self.lock();
        let id = id.map(str::to_string).unwrap_or_else(|| Uuid::new_v4().to_string());
        let now = now_iso();
        let created_at = state
            .database
            .dns_records
            .get(&id)
            .map(|record| record.created_at.clone())
            .unwrap_or_else(|| now.clone());
        let record = StaticDnsRecord {
            id: id.clone(),
            name,
            address,
            source: "operator".to_string(),
            created_at,
            updated_at: now,
        };
        state.database.dns_records.insert(id.clone(), record.clone());
        state.touch();
        state.event("dns_record_upserted", json!({"id":id,"name":record.name}));
        state.persist().map_err(|error| error.to_string())?;
        Ok(record)
    }

    pub fn delete_dns(&self, id: &str) -> Result<(), String> {
        let mut state = self.lock();
        let Some(record) = state.database.dns_records.get(id) else {
            return Err("DNS record not found".to_string());
        };
        if record.source == "builtin" {
            return Err("builtin DNS records cannot be deleted".to_string());
        }
        state.database.dns_records.remove(id);
        state.touch();
        state.event("dns_record_deleted", json!({"id":id}));
        state.persist().map_err(|error| error.to_string())
    }

    pub fn block_address(&self, input: BlockAddressInput) -> Result<BlockedAddress, String> {
        let address = input
            .address
            .parse::<Ipv4Addr>()
            .map_err(|_| "blocked address must be IPv4".to_string())?
            .to_string();
        let mut state = self.lock();
        let record = BlockedAddress {
            address: address.clone(),
            reason: input.reason.unwrap_or_else(|| "operator block".to_string()),
            created_at: now_iso(),
        };
        state
            .database
            .blocked_addresses
            .insert(address.clone(), record.clone());
        for flow in state.database.flows.values_mut() {
            if flow.source == address || flow.destination == address {
                flow.blocked = true;
            }
        }
        state.touch();
        state.event("address_blocked", json!({"address":address,"reason":record.reason}));
        state.persist().map_err(|error| error.to_string())?;
        Ok(record)
    }

    pub fn unblock_address(&self, address: &str) -> Result<(), String> {
        let address = address
            .parse::<Ipv4Addr>()
            .map_err(|_| "blocked address must be IPv4".to_string())?
            .to_string();
        let mut state = self.lock();
        if state.database.blocked_addresses.remove(&address).is_none() {
            return Err("blocked address not found".to_string());
        }
        let blocked: std::collections::BTreeSet<String> =
            state.database.blocked_addresses.keys().cloned().collect();
        for flow in state.database.flows.values_mut() {
            flow.blocked = blocked.contains(&flow.source) || blocked.contains(&flow.destination);
        }
        state.touch();
        state.event("address_unblocked", json!({"address":address}));
        state.persist().map_err(|error| error.to_string())
    }

    pub fn start_capture(&self, input: CaptureStartInput) -> Result<CaptureRecord, String> {
        validate_capture(&input)?;
        let mut state = self.lock();
        let active = state
            .database
            .captures
            .values()
            .filter(|capture| capture.state == CaptureState::Active)
            .count();
        if active >= state.config.capture.max_captures {
            return Err("maximum number of active captures reached".to_string());
        }
        let id = Uuid::new_v4().to_string();
        let name = if input.name.trim().is_empty() {
            format!("capture-{}", &id[..8])
        } else {
            input.name.trim().to_string()
        };
        let path = state.config.capture.directory.join(format!("{id}.pcap"));
        create_pcap(&path, state.config.capture.snaplen)?;
        let record = CaptureRecord {
            id: id.clone(),
            name,
            state: CaptureState::Active,
            direction: input.direction,
            host: input.host,
            protocol: input.protocol.map(|value| value.to_ascii_lowercase()),
            port: input.port,
            path: path.to_string_lossy().to_string(),
            created_at: now_iso(),
            stopped_at: None,
            packet_count: 0,
            captured_bytes: 0,
            original_bytes: 0,
            last_error: None,
        };
        state.database.captures.insert(id.clone(), record.clone());
        state.touch();
        state.event("capture_started", json!({"id":id,"name":record.name}));
        state.persist().map_err(|error| error.to_string())?;
        Ok(record)
    }

    pub fn stop_capture(&self, id: &str) -> Result<CaptureRecord, String> {
        let mut state = self.lock();
        let record = state
            .database
            .captures
            .get_mut(id)
            .ok_or_else(|| "capture not found".to_string())?;
        if record.state == CaptureState::Active {
            record.state = CaptureState::Stopped;
            record.stopped_at = Some(now_iso());
        }
        let result = record.clone();
        state.touch();
        state.event("capture_stopped", json!({"id":id}));
        state.persist().map_err(|error| error.to_string())?;
        Ok(result)
    }

    pub fn delete_capture(&self, id: &str) -> Result<(), String> {
        let mut state = self.lock();
        let record = state
            .database
            .captures
            .remove(id)
            .ok_or_else(|| "capture not found".to_string())?;
        let _ = fs::remove_file(&record.path);
        state.touch();
        state.event("capture_deleted", json!({"id":id}));
        state.persist().map_err(|error| error.to_string())
    }

    pub fn record_dns_query(&self, name: &str, source: &str, result: &str) {
        let mut state = self.lock();
        state.dns_queries = state.dns_queries.saturating_add(1);
        state.event(
            "dns_query",
            json!({"name":name,"source":source,"result":result}),
        );
    }

    pub fn record_test_request(&self, kind: &str, source: &str) {
        let mut state = self.lock();
        state.test_requests = state.test_requests.saturating_add(1);
        state.event("test_request", json!({"kind":kind,"source":source}));
    }

    pub fn recent_events(&self, limit: usize) -> Vec<GatewayEventRecord> {
        self.lock()
            .database
            .events
            .iter()
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn export(&self) -> Value {
        let state = self.lock();
        json!({
            "status": self_status(&state),
            "config": state.config,
            "contexts": state.contexts,
            "routes": state.database.routes,
            "nat_rules": state.database.nat_rules,
            "firewall_rules": state.database.firewall_rules,
            "dns_records": state.database.dns_records,
            "blocked_addresses": state.database.blocked_addresses,
            "captures": state.database.captures,
            "flows": state.database.flows,
            "events": state.database.events,
        })
    }

    pub fn metrics(&self) -> String {
        let status = self.status();
        format!(
            concat!(
                "# HELP netcore_ip_gateway_packet_core_connected Packet Core reachability\n",
                "# TYPE netcore_ip_gateway_packet_core_connected gauge\n",
                "netcore_ip_gateway_packet_core_connected {}\n",
                "# HELP netcore_ip_gateway_tun_open TUN device state\n",
                "# TYPE netcore_ip_gateway_tun_open gauge\n",
                "netcore_ip_gateway_tun_open {}\n",
                "# HELP netcore_ip_gateway_packets_total Packets by direction\n",
                "# TYPE netcore_ip_gateway_packets_total counter\n",
                "netcore_ip_gateway_packets_total{{direction=\"uplink\"}} {}\n",
                "netcore_ip_gateway_packets_total{{direction=\"downlink\"}} {}\n",
                "# HELP netcore_ip_gateway_bytes_total Bytes by direction\n",
                "# TYPE netcore_ip_gateway_bytes_total counter\n",
                "netcore_ip_gateway_bytes_total{{direction=\"uplink\"}} {}\n",
                "netcore_ip_gateway_bytes_total{{direction=\"downlink\"}} {}\n",
                "# HELP netcore_ip_gateway_dropped_packets_total Dropped packets\n",
                "# TYPE netcore_ip_gateway_dropped_packets_total counter\n",
                "netcore_ip_gateway_dropped_packets_total {}\n",
                "# HELP netcore_ip_gateway_flows Active remembered flows\n",
                "# TYPE netcore_ip_gateway_flows gauge\n",
                "netcore_ip_gateway_flows {}\n",
                "# HELP netcore_ip_gateway_captures_active Active packet captures\n",
                "# TYPE netcore_ip_gateway_captures_active gauge\n",
                "netcore_ip_gateway_captures_active {}\n"
            ),
            u8::from(status.packet_core_connected),
            u8::from(status.tun_open),
            status.packets_uplink,
            status.packets_downlink,
            status.bytes_uplink,
            status.bytes_downlink,
            status.packets_dropped,
            status.flows,
            status.captures_active,
        )
    }

    pub fn persist(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.lock().persist()
    }

    fn delete_record(&self, id: &str, kind: &str) -> Result<(), String> {
        let mut state = self.lock();
        let removed = match kind {
            "route" => state.database.routes.remove(id).is_some(),
            "nat" => state.database.nat_rules.remove(id).is_some(),
            "firewall" => state.database.firewall_rules.remove(id).is_some(),
            _ => false,
        };
        if !removed {
            return Err(format!("{kind} record not found"));
        }
        state.touch();
        state.event(&format!("{kind}_deleted"), json!({"id":id}));
        state.persist().map_err(|error| error.to_string())
    }

    fn lock(&self) -> MutexGuard<'_, GatewayState> {
        self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl GatewayState {
    fn touch(&mut self) {
        self.database.revision = self.database.revision.saturating_add(1);
    }

    fn event(&mut self, kind: &str, detail: Value) {
        let event = GatewayEventRecord {
            sequence: self.database.next_event_sequence,
            timestamp: now_iso(),
            kind: kind.to_string(),
            detail,
        };
        self.database.next_event_sequence = self.database.next_event_sequence.saturating_add(1);
        self.database.events.push_back(event);
        while self.database.events.len() > self.config.limits.max_events {
            self.database.events.pop_front();
        }
    }

    fn persist(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = self.config.storage.database_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(&self.database)?;
        let temporary = self.config.storage.database_path.with_extension("json.tmp");
        fs::write(&temporary, &bytes)?;
        if self.config.storage.database_path.exists() {
            let _ = fs::copy(
                &self.config.storage.database_path,
                &self.config.storage.backup_path,
            );
        }
        fs::rename(temporary, &self.config.storage.database_path)?;
        Ok(())
    }
}

fn read_backup(config: &IpGatewayConfig) -> Result<GatewayDatabase, Box<dyn std::error::Error>> {
    let bytes = fs::read(&config.storage.backup_path)?;
    let database = serde_json::from_slice::<GatewayDatabase>(&bytes)?;
    if database.schema_version != DATABASE_SCHEMA_VERSION {
        return Err("unsupported IP Gateway backup schema".into());
    }
    Ok(database)
}

fn ensure_builtin_dns_records(config: &IpGatewayConfig, database: &mut GatewayDatabase) {
    let domain = config.dns.local_domain.trim_end_matches('.').to_ascii_lowercase();
    let address = config.gateway_ipv4().to_string();
    for (id, name) in [
        ("builtin-gateway", domain.clone()),
        ("builtin-wap", format!("wap.{domain}")),
        ("builtin-test", format!("test.{domain}")),
    ] {
        let now = now_iso();
        database.dns_records.insert(
            id.to_string(),
            StaticDnsRecord {
                id: id.to_string(),
                name,
                address: address.clone(),
                source: "builtin".to_string(),
                created_at: now.clone(),
                updated_at: now,
            },
        );
    }
}

fn validate_route(input: &RouteRuleInput) -> Result<(), String> {
    parse_cidr(&input.destination)?;
    if let Some(gateway) = &input.gateway {
        gateway
            .parse::<Ipv4Addr>()
            .map_err(|_| "route.gateway must be IPv4".to_string())?;
    }
    if input.gateway.is_none() && input.interface.is_none() {
        return Err("route requires gateway or interface".to_string());
    }
    validate_optional_interface(input.interface.as_deref())?;
    Ok(())
}

fn validate_nat(input: &NatRuleInput) -> Result<(), String> {
    if !matches!(input.kind.as_str(), "masquerade" | "snat" | "dnat") {
        return Err("NAT kind must be masquerade, snat or dnat".to_string());
    }
    if let Some(value) = &input.source_cidr {
        parse_cidr(value)?;
    }
    if let Some(value) = &input.destination_cidr {
        parse_cidr(value)?;
    }
    if let Some(value) = &input.protocol
        && !matches!(value.as_str(), "tcp" | "udp")
    {
        return Err("NAT protocol must be tcp or udp".to_string());
    }
    if input.destination_port.is_some() && input.protocol.is_none() {
        return Err("NAT destination_port requires tcp or udp protocol".to_string());
    }
    if input.to_port.is_some() && input.protocol.is_none() {
        return Err("NAT to_port requires tcp or udp protocol".to_string());
    }
    if matches!(input.kind.as_str(), "snat" | "dnat") && input.to_address.is_none() {
        return Err("SNAT/DNAT requires to_address".to_string());
    }
    if let Some(value) = &input.to_address {
        value
            .parse::<Ipv4Addr>()
            .map_err(|_| "NAT to_address must be IPv4".to_string())?;
    }
    validate_optional_interface(input.out_interface.as_deref())?;
    Ok(())
}

fn validate_firewall(input: &FirewallRuleInput) -> Result<(), String> {
    if !matches!(input.chain.as_str(), "input" | "forward" | "output") {
        return Err("firewall.chain must be input, forward or output".to_string());
    }
    if !matches!(input.action.as_str(), "accept" | "drop" | "reject") {
        return Err("firewall.action must be accept, drop or reject".to_string());
    }
    if !matches!(input.protocol.as_str(), "any" | "tcp" | "udp" | "icmp") {
        return Err("firewall.protocol must be any, tcp, udp or icmp".to_string());
    }
    if input.protocol == "any" && (input.source_port.is_some() || input.destination_port.is_some()) {
        return Err("ports require tcp or udp protocol".to_string());
    }
    if let Some(value) = &input.source_cidr {
        parse_cidr(value)?;
    }
    if let Some(value) = &input.destination_cidr {
        parse_cidr(value)?;
    }
    validate_optional_interface(input.in_interface.as_deref())?;
    validate_optional_interface(input.out_interface.as_deref())?;
    Ok(())
}

fn validate_optional_interface(value: Option<&str>) -> Result<(), String> {
    if let Some(value) = value {
        if value.is_empty()
            || value.len() >= libc::IFNAMSIZ
            || !value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')
            })
        {
            return Err("invalid network interface name".to_string());
        }
    }
    Ok(())
}

fn validate_capture(input: &CaptureStartInput) -> Result<(), String> {
    if !matches!(input.direction.as_str(), "uplink" | "downlink" | "both") {
        return Err("capture.direction must be uplink, downlink or both".to_string());
    }
    if let Some(host) = &input.host {
        host.parse::<Ipv4Addr>()
            .map_err(|_| "capture.host must be IPv4".to_string())?;
    }
    if let Some(protocol) = &input.protocol
        && !matches!(protocol.to_ascii_lowercase().as_str(), "tcp" | "udp" | "icmp")
    {
        return Err("capture.protocol must be tcp, udp or icmp".to_string());
    }
    Ok(())
}

fn normalise_dns_name(value: &str) -> Result<String, String> {
    let value = value.trim().trim_end_matches('.').to_ascii_lowercase();
    if value.is_empty() || value.len() > 253 {
        return Err("invalid DNS name length".to_string());
    }
    if !value
        .split('.')
        .all(|label| !label.is_empty() && label.len() <= 63 && label.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-'))
    {
        return Err("invalid DNS name".to_string());
    }
    Ok(value)
}

fn enforce_flow_limit(state: &mut GatewayState) {
    while state.database.flows.len() > state.config.limits.max_flows {
        let oldest = state
            .database
            .flows
            .iter()
            .min_by(|(_, left), (_, right)| left.last_seen.cmp(&right.last_seen))
            .map(|(id, _)| id.clone());
        if let Some(id) = oldest {
            state.database.flows.remove(&id);
        } else {
            break;
        }
    }
}

fn parse_ipv4(packet: &[u8]) -> Result<PacketObservation, String> {
    if packet.len() < 20 || packet[0] >> 4 != 4 {
        return Err("packet is not a complete IPv4 datagram".to_string());
    }
    let header_len = usize::from(packet[0] & 0x0f) * 4;
    if header_len < 20 || packet.len() < header_len {
        return Err("invalid IPv4 header length".to_string());
    }
    let total_len = usize::from(u16::from_be_bytes([packet[2], packet[3]]));
    if total_len < header_len || total_len > packet.len() {
        return Err("invalid IPv4 total length".to_string());
    }
    let source = Ipv4Addr::new(packet[12], packet[13], packet[14], packet[15]);
    let destination = Ipv4Addr::new(packet[16], packet[17], packet[18], packet[19]);
    let protocol_number = packet[9];
    let protocol = match protocol_number {
        1 => "icmp",
        6 => "tcp",
        17 => "udp",
        _ => "other",
    }
    .to_string();
    let (source_port, destination_port) = if matches!(protocol_number, 6 | 17)
        && total_len >= header_len + 4
    {
        (
            Some(u16::from_be_bytes([packet[header_len], packet[header_len + 1]])),
            Some(u16::from_be_bytes([
                packet[header_len + 2],
                packet[header_len + 3],
            ])),
        )
    } else {
        (None, None)
    };
    Ok(PacketObservation {
        source,
        destination,
        protocol,
        source_port,
        destination_port,
    })
}

fn capture_packet(
    state: &mut GatewayState,
    direction: &str,
    packet: &[u8],
    observation: &PacketObservation,
) {
    let snaplen = state.config.capture.snaplen;
    let max_file_bytes = state.config.capture.max_file_bytes;
    let mut events = Vec::new();
    for capture in state.database.captures.values_mut() {
        if capture.state != CaptureState::Active
            || !capture_matches(capture, direction, observation)
        {
            continue;
        }
        let current_size = fs::metadata(&capture.path).map(|value| value.len()).unwrap_or(0);
        let next_record_size = 16_u64.saturating_add(packet.len().min(snaplen) as u64);
        if current_size.saturating_add(next_record_size) > max_file_bytes {
            capture.state = CaptureState::Full;
            capture.stopped_at = Some(now_iso());
            events.push(("capture_full", capture.id.clone(), None));
            continue;
        }
        match append_pcap(Path::new(&capture.path), packet, snaplen) {
            Ok(captured) => {
                capture.packet_count = capture.packet_count.saturating_add(1);
                capture.captured_bytes = capture.captured_bytes.saturating_add(captured as u64);
                capture.original_bytes = capture.original_bytes.saturating_add(packet.len() as u64);
            }
            Err(error) => {
                capture.state = CaptureState::Error;
                capture.stopped_at = Some(now_iso());
                capture.last_error = Some(error.clone());
                events.push(("capture_error", capture.id.clone(), Some(error)));
            }
        }
    }
    for (kind, id, error) in events {
        state.event(kind, json!({"id":id,"error":error}));
    }
}

fn capture_matches(
    capture: &CaptureRecord,
    direction: &str,
    observation: &PacketObservation,
) -> bool {
    if capture.direction != "both" && capture.direction != direction {
        return false;
    }
    if let Some(host) = &capture.host
        && host != &observation.source.to_string()
        && host != &observation.destination.to_string()
    {
        return false;
    }
    if let Some(protocol) = &capture.protocol
        && protocol != &observation.protocol
    {
        return false;
    }
    if let Some(port) = capture.port
        && observation.source_port != Some(port)
        && observation.destination_port != Some(port)
    {
        return false;
    }
    true
}

fn create_pcap(path: &Path, snaplen: usize) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let mut file = fs::File::create(path).map_err(|error| error.to_string())?;
    file.write_all(&0xa1b2c3d4u32.to_le_bytes())
        .and_then(|()| file.write_all(&2u16.to_le_bytes()))
        .and_then(|()| file.write_all(&4u16.to_le_bytes()))
        .and_then(|()| file.write_all(&0i32.to_le_bytes()))
        .and_then(|()| file.write_all(&0u32.to_le_bytes()))
        .and_then(|()| file.write_all(&(snaplen as u32).to_le_bytes()))
        .and_then(|()| file.write_all(&101u32.to_le_bytes()))
        .map_err(|error| error.to_string())
}

fn append_pcap(path: &Path, packet: &[u8], snaplen: usize) -> Result<usize, String> {
    let captured = packet.len().min(snaplen);
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?;
    let mut file = OpenOptions::new()
        .append(true)
        .open(path)
        .map_err(|error| error.to_string())?;
    file.write_all(&(duration.as_secs() as u32).to_le_bytes())
        .and_then(|()| file.write_all(&duration.subsec_micros().to_le_bytes()))
        .and_then(|()| file.write_all(&(captured as u32).to_le_bytes()))
        .and_then(|()| file.write_all(&(packet.len() as u32).to_le_bytes()))
        .and_then(|()| file.write_all(&packet[..captured]))
        .map_err(|error| error.to_string())?;
    Ok(captured)
}

fn self_status(state: &GatewayState) -> Value {
    json!({
        "service":"netcore-ip-gateway",
        "started_at":state.started_at,
        "mode":state.config.interface.mode,
        "packet_core_connected":state.packet_core_connected,
        "tun_open":state.tun_open,
        "kernel_revision":state.database.revision,
        "kernel_applied_revision":state.kernel_applied_revision,
    })
}

fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

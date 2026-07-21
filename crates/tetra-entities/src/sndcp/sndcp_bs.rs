use std::collections::{HashMap, HashSet};
use std::net::Ipv4Addr;
use std::time::Instant;

use crate::{MessageQueue, TetraEntityTrait};
use tetra_config::bluestation::SharedConfig;
use tetra_core::tetra_entities::TetraEntity;
use tetra_core::{BitBuffer, Sap, TimeslotOwner};
use tetra_saps::lcmc::enums::{alloc_type::ChanAllocType, ul_dl_assignment::UlDlAssignment};
use tetra_saps::lcmc::fields::chan_alloc_req::CmceChanAllocReq;
use tetra_saps::ltpd::LtpdMleUnitdataInd;
use tetra_saps::tla::{TlaTlDataReqBl, TlaTlUnitdataReqBl};
use tetra_saps::{SapMsg, SapMsgInner};

use super::wap_ip::{WapEndpoint, WapPolicy, build_response_npdu};
use super::wap_status::WapStatusSnapshot;

const MLE_DISCRIMINATOR_SNDCP: u64 = 0b100;
const SN_ACTIVATE_PDP_CONTEXT: u8 = 0;
const SN_DEACTIVATE_PDP_CONTEXT_ACCEPT: u8 = 1;
const SN_DEACTIVATE_PDP_CONTEXT_DEMAND: u8 = 2;
const SN_ACTIVATE_PDP_CONTEXT_REJECT: u8 = 3;
const SN_UNITDATA: u8 = 4;
const SN_DATA: u8 = 5;
const SN_DATA_TRANSMIT_REQUEST: u8 = 6;
const SN_DATA_TRANSMIT_RESPONSE: u8 = 7;
const SN_END_OF_DATA: u8 = 8;
const SNDCP_PDCH_LOGICAL_TS: u8 = 2;
const TRANSMIT_REJECT_SYSTEM_RESOURCES_NOT_AVAILABLE: u8 = 2;
const ACTIVATE_REJECT_DYNAMIC_POOL_EMPTY: u8 = 7;
const ACTIVATE_REJECT_STATIC_ADDRESS_NOT_CORRECT: u8 = 8;
const ACTIVATE_REJECT_STATIC_ADDRESS_IN_USE: u8 = 9;
const ACTIVATE_REJECT_STATIC_ADDRESS_NOT_ALLOWED: u8 = 10;
const ACTIVATE_REJECT_SNDCP_VERSION_NOT_SUPPORTED: u8 = 16;
const ACTIVATE_REJECT_UNSUPPORTED_ADDRESS_TYPE: u8 = 34;

const PDU_PRIORITY_MAX: u64 = 4;
const READY_TIMER: u64 = 8;
const STANDBY_TIMER: u64 = 4;
const RESPONSE_WAIT_TIMER: u64 = 7;
const TIA_IPV4_STATIC: u64 = 1;
const TIA_IPV4_DYNAMIC: u64 = 2;
/// A 576-octet N-PDU is the interoperable one-slot profile used by the Openwave browser budget.
const MTU_576: u64 = 2;

const PCO_TYPE34_ID: u64 = 1;
const PPP_PROTO_CHAP: u64 = 0xC223;
const PPP_CONFIG_PROTOCOL_PPP: u64 = 0;
const CHAP_CODE_SUCCESS: u64 = 3;
const PCO_CHAP_SUCCESS_BITS: u64 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PdpState {
    Standby,
    Ready,
}

#[derive(Debug, Clone, Copy)]
struct PdpContext {
    address: [u8; 4],
    state: PdpState,
    pdch_ts: Option<u8>,
}

pub struct Sndcp {
    config: SharedConfig,
    contexts: HashMap<(u32, u8), PdpContext>,
    started_at: Instant,
    last_activity: String,
}

impl Sndcp {
    pub fn new(config: SharedConfig) -> Self {
        Self {
            config,
            contexts: HashMap::new(),
            started_at: Instant::now(),
            last_activity: "SNDCP ready".to_string(),
        }
    }

    fn profile_enabled(&self) -> bool {
        self.config.config().cell.wap_ip_sndcp_profile_enabled()
    }

    /// MLE advances the incoming buffer cursor by exactly three bits before routing it to SNDCP.
    /// Tests and older callers may still hand us a cursor-at-zero buffer containing the discriminator,
    /// so support both forms without ever dropping three bytes by accident.
    fn rebase_sndcp_pdu(sdu: &BitBuffer) -> BitBuffer {
        if sdu.get_pos() != 0 {
            return BitBuffer::from_bitbuffer_pos(sdu);
        }
        let mut probe = BitBuffer::from_bitbuffer(sdu);
        if probe.peek_bits(3) == Some(MLE_DISCRIMINATOR_SNDCP) {
            let _ = probe.read_bits(3);
            BitBuffer::from_bitbuffer_pos(&probe)
        } else {
            probe
        }
    }

    fn dynamic_address(&self, issi: u32, nsapi: u8) -> Option<[u8; 4]> {
        if let Some(existing) = self.contexts.get(&(issi, nsapi)) {
            return Some(existing.address);
        }
        let cfg = self.config.config();
        let prefix = cfg.cell.wap_ip.pool_prefix_octets()?;
        let used: HashSet<[u8; 4]> = self.contexts.values().map(|ctx| ctx.address).collect();
        (cfg.cell.wap_ip.dynamic_pool_first_host..=cfg.cell.wap_ip.dynamic_pool_last_host)
            .map(|host| [prefix[0], prefix[1], prefix[2], host])
            .find(|address| !used.contains(address))
    }

    fn reserve_mvp_pdch(&self) -> bool {
        let mut state = self.config.state_write();
        state
            .timeslot_alloc
            .reserve(TimeslotOwner::Sndcp, SNDCP_PDCH_LOGICAL_TS)
            .is_ok()
    }

    fn release_pdch(&self, ts: u8) {
        let mut state = self.config.state_write();
        if let Err(error) = state.timeslot_alloc.release(TimeslotOwner::Sndcp, ts) {
            tracing::warn!("SNDCP: failed to release PDCH TS{}: {:?}", ts, error);
        }
    }

    fn queue_activate_reject(&self, queue: &mut MessageQueue, ind: &LtpdMleUnitdataInd, nsapi: u8, cause: u8) {
        let mut response = BitBuffer::new(17);
        response.write_bits(u64::from(SN_ACTIVATE_PDP_CONTEXT_REJECT), 4);
        response.write_bits(u64::from(nsapi), 4);
        response.write_bits(u64::from(cause), 8);
        response.write_bits(0, 1); // no optional elements
        response.seek(0);
        self.queue_acked(queue, ind, Self::wrap_sndcp(&mut response), None);
    }

    fn static_address_available(&self, key: (u32, u8), address: [u8; 4]) -> bool {
        !self
            .contexts
            .iter()
            .any(|(existing_key, context)| *existing_key != key && context.address == address)
    }

    fn queue_data_transmit_reject(&self, queue: &mut MessageQueue, ind: &LtpdMleUnitdataInd, nsapi: u8, cause: u8) {
        let mut response = BitBuffer::new(18);
        response.write_bits(u64::from(SN_DATA_TRANSMIT_RESPONSE), 4);
        response.write_bits(u64::from(nsapi), 4);
        response.write_bits(0, 1); // reject
        response.write_bits(u64::from(cause), 8);
        response.write_bits(0, 1); // no optional elements
        response.seek(0);
        self.queue_acked(queue, ind, Self::wrap_sndcp(&mut response), None);
    }

    fn build_pdp_accept_pdu(nsapi: u8, tia: u64, address: [u8; 4], chap_id: Option<u8>) -> BitBuffer {
        let mut sn = BitBuffer::new_autoexpand(16);
        sn.write_bits(SN_ACTIVATE_PDP_CONTEXT as u64, 4);
        sn.write_bits(u64::from(nsapi), 4);
        sn.write_bits(PDU_PRIORITY_MAX, 3);
        sn.write_bits(READY_TIMER, 4);
        sn.write_bits(STANDBY_TIMER, 4);
        sn.write_bits(RESPONSE_WAIT_TIMER, 4);
        sn.write_bits(tia, 3);
        for octet in address {
            sn.write_bits(u64::from(octet), 8);
        }
        sn.write_bits(0, 8);
        sn.write_bits(MTU_576, 3);
        if let Some(id) = chap_id {
            for bit in chap_success_optional_section(id).bytes() {
                sn.write_bits(u64::from(bit - b'0'), 1);
            }
        } else {
            sn.write_bits(0, 1);
        }
        sn.seek(0);
        sn
    }

    fn queue_acked(&self, queue: &mut MessageQueue, ind: &LtpdMleUnitdataInd, tl_sdu: BitBuffer, chan_alloc: Option<CmceChanAllocReq>) {
        queue.push_back(SapMsg {
            sap: Sap::TlaSap,
            src: TetraEntity::Sndcp,
            dest: TetraEntity::Llc,
            msg: SapMsgInner::TlaTlDataReqBl(TlaTlDataReqBl {
                main_address: ind.received_tetra_address,
                link_id: ind.link_id,
                endpoint_id: ind.endpoint_id,
                tl_sdu,
                stealing_permission: false,
                subscriber_class: 0,
                fcs_flag: false,
                air_interface_encryption: None,
                stealing_repeats_flag: None,
                data_class_info: None,
                req_handle: 0,
                graceful_degradation: None,
                chan_alloc,
                tx_reporter: None,
            }),
        });
    }

    fn queue_unacked(&self, queue: &mut MessageQueue, ind: &LtpdMleUnitdataInd, tl_sdu: BitBuffer, chan_alloc: Option<CmceChanAllocReq>) {
        queue.push_back(SapMsg {
            sap: Sap::TlaSap,
            src: TetraEntity::Sndcp,
            dest: TetraEntity::Llc,
            msg: SapMsgInner::TlaTlUnitdataReqBl(TlaTlUnitdataReqBl {
                main_address: ind.received_tetra_address,
                link_id: ind.link_id,
                endpoint_id: ind.endpoint_id,
                tl_sdu,
                stealing_permission: false,
                subscriber_class: 0,
                fcs_flag: false,
                air_interface_encryption: None,
                packet_data_flag: true,
                n_tlsdu_repeats: 0,
                data_class_info: None,
                req_handle: 0,
                chan_alloc,
                tx_reporter: None,
            }),
        });
    }

    fn wrap_sndcp(sn_pdu: &mut BitBuffer) -> BitBuffer {
        let len = sn_pdu.get_len_remaining();
        let mut tl_sdu = BitBuffer::new(3 + len);
        tl_sdu.write_bits(MLE_DISCRIMINATOR_SNDCP, 3);
        tl_sdu.copy_bits(sn_pdu, len);
        tl_sdu.seek(0);
        tl_sdu
    }

    fn bytes_from_remaining(pdu: &BitBuffer) -> Option<Vec<u8>> {
        if pdu.get_len_remaining() % 8 != 0 {
            return None;
        }
        let mut pdu = BitBuffer::from_bitbuffer_pos(pdu);
        let mut out = vec![0u8; pdu.get_len_remaining() / 8];
        let bits = pdu.get_len_remaining();
        pdu.read_bits_into_slice(bits, &mut out)?;
        Some(out)
    }

    fn snapshot(&self) -> WapStatusSnapshot {
        let state = self.config.state_read();
        let active_calls = state.active_call_ts.values().copied().collect::<HashSet<_>>().len();
        WapStatusSnapshot {
            title: "NetCore-Tetra".to_string(),
            state: if state.network_connected { "ONLINE" } else { "STANDALONE" }.to_string(),
            version: format!("v{}", env!("CARGO_PKG_VERSION")),
            registered_ms: state.subscribers.registered_count(),
            attached_groups: state.subscribers.attached_group_count(),
            active_calls,
            queued_sds: state.live_sds_queue.len(),
            uptime_secs: self.started_at.elapsed().as_secs(),
            last_activity: self.last_activity.clone(),
            health: "OK".to_string(),
        }
    }

    fn handle_activate(&mut self, queue: &mut MessageQueue, ind: &LtpdMleUnitdataInd, pdu: &BitBuffer) {
        let demand = pdu.to_bitstr();
        let bits = |off: usize, n: usize| -> Option<u64> { demand.get(off..off + n).and_then(|s| u64::from_str_radix(s, 2).ok()) };
        let version = bits(4, 4).unwrap_or(0);
        let nsapi = bits(8, 4).unwrap_or(0) as u8;
        let atid = bits(12, 3).unwrap_or(7);
        if !(1..=14).contains(&nsapi) {
            tracing::warn!("SNDCP: rejecting malformed activation NSAPI={}", nsapi);
            return;
        }
        if version != 1 {
            tracing::warn!("SNDCP: unsupported SNDCP version {}", version);
            self.queue_activate_reject(queue, ind, nsapi, ACTIVATE_REJECT_SNDCP_VERSION_NOT_SUPPORTED);
            return;
        }

        let key = (ind.received_tetra_address.ssi, nsapi);
        let (allow_static_ipv4, endpoint_address) = {
            let cfg = self.config.config();
            (cfg.cell.wap_ip.allow_static_ipv4, cfg.cell.wap_ip.address.octets())
        };
        let (tia, address) = match atid {
            0 if allow_static_ipv4 => {
                let raw = bits(15, 32).unwrap_or_default() as u32;
                let address = raw.to_be_bytes();
                let ip = Ipv4Addr::from(address);
                if ip.is_unspecified()
                    || ip.is_multicast()
                    || address == Ipv4Addr::BROADCAST.octets()
                    || address == endpoint_address
                {
                    tracing::warn!("SNDCP: invalid static IPv4 request {} from ISSI {}", ip, key.0);
                    self.queue_activate_reject(queue, ind, nsapi, ACTIVATE_REJECT_STATIC_ADDRESS_NOT_CORRECT);
                    return;
                }
                if !self.static_address_available(key, address) {
                    tracing::warn!("SNDCP: static IPv4 {} already in use", ip);
                    self.queue_activate_reject(queue, ind, nsapi, ACTIVATE_REJECT_STATIC_ADDRESS_IN_USE);
                    return;
                }
                (TIA_IPV4_STATIC, address)
            }
            0 => {
                tracing::warn!("SNDCP: static IPv4 request rejected by WAP policy for ISSI {}", key.0);
                self.queue_activate_reject(queue, ind, nsapi, ACTIVATE_REJECT_STATIC_ADDRESS_NOT_ALLOWED);
                return;
            }
            1 => {
                let Some(address) = self.dynamic_address(key.0, nsapi) else {
                    tracing::warn!("SNDCP: dynamic IPv4 pool exhausted");
                    self.queue_activate_reject(queue, ind, nsapi, ACTIVATE_REJECT_DYNAMIC_POOL_EMPTY);
                    return;
                };
                (TIA_IPV4_DYNAMIC, address)
            }
            other => {
                tracing::warn!("SNDCP: unsupported address type identifier {}", other);
                self.queue_activate_reject(queue, ind, nsapi, ACTIVATE_REJECT_UNSUPPORTED_ADDRESS_TYPE);
                return;
            }
        };

        let chap_id = find_chap_response_id(&demand);
        let mut sn = Self::build_pdp_accept_pdu(nsapi, tia, address, chap_id);

        if let Some(previous) = self.contexts.remove(&key) {
            if let Some(ts) = previous.pdch_ts {
                self.release_pdch(ts);
            }
        }
        self.contexts.insert(
            key,
            PdpContext { address, state: PdpState::Standby, pdch_ts: None },
        );
        self.last_activity = format!("PDP {} NSAPI{}", ind.received_tetra_address.ssi, nsapi);
        tracing::info!(
            "SNDCP: PDP context accepted ISSI={} NSAPI={} IPv4={} CHAP={}",
            ind.received_tetra_address.ssi,
            nsapi,
            Ipv4Addr::from(address),
            chap_id.is_some()
        );
        self.queue_acked(queue, ind, Self::wrap_sndcp(&mut sn), None);
    }

    fn handle_data_transmit_request(&mut self, queue: &mut MessageQueue, ind: &LtpdMleUnitdataInd, pdu: &BitBuffer) {
        let mut read = BitBuffer::from_bitbuffer(pdu);
        let Some(kind) = read.read_bits(4) else { return };
        if kind != u64::from(SN_DATA_TRANSMIT_REQUEST) {
            return;
        }
        let Some(nsapi) = read.read_bits(4).map(|v| v as u8) else { return };
        let key = (ind.received_tetra_address.ssi, nsapi);
        let Some(existing) = self.contexts.get(&key).copied() else {
            tracing::warn!("SNDCP: transmit request without PDP context ISSI={} NSAPI={}", key.0, key.1);
            self.queue_data_transmit_reject(queue, ind, nsapi, 1); // unknown NSAPI
            return;
        };

        let already_reserved = existing.pdch_ts == Some(SNDCP_PDCH_LOGICAL_TS);
        if !already_reserved && !self.reserve_mvp_pdch() {
            tracing::warn!("SNDCP: TS2 unavailable for packet data ISSI={} NSAPI={}", key.0, key.1);
            self.queue_data_transmit_reject(
                queue,
                ind,
                nsapi,
                TRANSMIT_REJECT_SYSTEM_RESOURCES_NOT_AVAILABLE,
            );
            return;
        }

        let Some(ctx) = self.contexts.get_mut(&key) else {
            if !already_reserved {
                self.release_pdch(SNDCP_PDCH_LOGICAL_TS);
            }
            return;
        };
        ctx.state = PdpState::Ready;
        ctx.pdch_ts = Some(SNDCP_PDCH_LOGICAL_TS);

        let mut response = BitBuffer::new(10);
        response.write_bits(u64::from(SN_DATA_TRANSMIT_RESPONSE), 4);
        response.write_bits(u64::from(nsapi), 4);
        response.write_bits(1, 1); // accept
        response.write_bits(0, 1); // no optional elements
        response.seek(0);
        let alloc = CmceChanAllocReq {
            usage: None,
            carrier: None,
            timeslots: [false, true, false, false],
            alloc_type: ChanAllocType::Replace,
            ul_dl_assigned: UlDlAssignment::Both,
        };
        self.last_activity = format!("PDCH {} NSAPI{}", key.0, key.1);
        self.queue_acked(queue, ind, Self::wrap_sndcp(&mut response), Some(alloc));
    }

    fn handle_unitdata(&mut self, queue: &mut MessageQueue, ind: &LtpdMleUnitdataInd, pdu: &BitBuffer) {
        let mut read = BitBuffer::from_bitbuffer(pdu);
        let Some(kind) = read.read_bits(4).map(|v| v as u8) else { return };
        if !matches!(kind, SN_UNITDATA | SN_DATA) {
            return;
        }
        let Some(nsapi) = read.read_bits(4).map(|v| v as u8) else { return };
        let Some(pcomp) = read.read_bits(4) else { return };
        let Some(dcomp) = read.read_bits(4) else { return };
        if pcomp != 0 || dcomp != 0 || !(1..=14).contains(&nsapi) {
            tracing::warn!("SNDCP: unsupported compression or NSAPI pcomp={} dcomp={} nsapi={}", pcomp, dcomp, nsapi);
            return;
        }
        let Some(npdu) = Self::bytes_from_remaining(&read) else {
            tracing::warn!("SNDCP: N-PDU is not octet-aligned");
            return;
        };
        let key = (ind.received_tetra_address.ssi, nsapi);
        let Some(ctx) = self.contexts.get(&key).copied() else {
            tracing::warn!("SNDCP: unitdata without PDP context ISSI={} NSAPI={}", key.0, key.1);
            return;
        };

        let cfg = self.config.config();
        let endpoint = WapEndpoint {
            address: cfg.cell.wap_ip.address.octets(),
            port: cfg.cell.wap_ip.port,
            ttl: cfg.cell.wap_ip.response_ttl,
        };
        let policy = WapPolicy {
            accept_empty_probe: cfg.cell.wap_ip.accept_empty_probe,
            accept_root_path: cfg.cell.wap_ip.accept_root_path,
            accept_status_path: cfg.cell.wap_ip.accept_status_path,
            accept_status_wml_path: cfg.cell.wap_ip.accept_status_wml_path,
            max_request_payload_bytes: cfg.cell.wap_ip.max_request_payload_bytes,
        };
        drop(cfg);

        // Build the complete response before the READY gate, matching the terminal-visible error ordering.
        let snapshot = self.snapshot();
        let response = match build_response_npdu(&npdu, endpoint, policy, &snapshot) {
            Ok(Some(response)) => response,
            Ok(None) => return, // WTP ACK/ABORT
            Err(error) => {
                tracing::warn!("SNDCP WAP/IP request rejected from ISSI {}: {:?}", key.0, error);
                return;
            }
        };
        let source_address: [u8; 4] = npdu
            .get(12..16)
            .and_then(|octets| octets.try_into().ok())
            .unwrap_or([0; 4]);
        if ctx.address != source_address {
            tracing::warn!("SNDCP: source address does not match PDP context for ISSI {}", key.0);
            return;
        }
        let assumed_ready = self.config.config().cell.wap_ip.assume_pdch_ready_after_data_transmit;
        if ctx.state != PdpState::Ready && !assumed_ready {
            tracing::warn!("SNDCP: WAP response built but PDP context is not READY for ISSI {} NSAPI {}", key.0, key.1);
            return;
        }

        let mut sn = BitBuffer::new(16 + response.len() * 8);
        sn.write_bits(u64::from(SN_UNITDATA), 4);
        sn.write_bits(u64::from(nsapi), 4);
        sn.write_bits(0, 4);
        sn.write_bits(0, 4);
        for octet in response {
            sn.write_bits(u64::from(octet), 8);
        }
        sn.seek(0);
        self.last_activity = format!("WAP {}", key.0);
        self.queue_unacked(queue, ind, Self::wrap_sndcp(&mut sn), None);
    }

    fn handle_deactivate(&mut self, queue: &mut MessageQueue, ind: &LtpdMleUnitdataInd, pdu: &BitBuffer) {
        let mut read = BitBuffer::from_bitbuffer(pdu);
        let _ = read.read_bits(4);
        let Some(deactivation_type) = read.read_bits(8).map(|v| v as u8) else { return };
        let nsapi = if deactivation_type == 1 { read.read_bits(4).map(|v| v as u8) } else { None };
        let mut released = Vec::new();
        match nsapi {
            Some(nsapi) => {
                if let Some(ctx) = self.contexts.remove(&(ind.received_tetra_address.ssi, nsapi)) {
                    if let Some(ts) = ctx.pdch_ts {
                        released.push(ts);
                    }
                }
            }
            None if deactivation_type == 0 => {
                let keys: Vec<_> = self
                    .contexts
                    .keys()
                    .filter(|(issi, _)| *issi == ind.received_tetra_address.ssi)
                    .copied()
                    .collect();
                for key in keys {
                    if let Some(ctx) = self.contexts.remove(&key) {
                        if let Some(ts) = ctx.pdch_ts {
                            released.push(ts);
                        }
                    }
                }
            }
            None => return,
        }
        for ts in released {
            self.release_pdch(ts);
        }

        let mut response = BitBuffer::new(if nsapi.is_some() { 17 } else { 13 });
        response.write_bits(u64::from(SN_DEACTIVATE_PDP_CONTEXT_ACCEPT), 4);
        response.write_bits(u64::from(deactivation_type), 8);
        if let Some(nsapi) = nsapi {
            response.write_bits(u64::from(nsapi), 4);
        }
        response.write_bits(0, 1);
        response.seek(0);
        self.last_activity = format!("PDP off {}", ind.received_tetra_address.ssi);
        self.queue_acked(queue, ind, Self::wrap_sndcp(&mut response), None);
    }

    fn handle_end_of_data(&mut self, queue: &mut MessageQueue, ind: &LtpdMleUnitdataInd) {
        let mut released = Vec::new();
        for ((issi, _), ctx) in &mut self.contexts {
            if *issi == ind.received_tetra_address.ssi {
                ctx.state = PdpState::Standby;
                if let Some(ts) = ctx.pdch_ts.take() {
                    released.push(ts);
                }
            }
        }
        for ts in released {
            self.release_pdch(ts);
        }

        // The MAC primitive supports a channel allocation without higher-layer payload. Send an
        // empty BL-UDATA with QuitAndGo so the terminal returns from TS2 to common control.
        let alloc = CmceChanAllocReq {
            usage: None,
            carrier: None,
            timeslots: [false; 4],
            alloc_type: ChanAllocType::QuitAndGo,
            ul_dl_assigned: UlDlAssignment::Both,
        };
        self.queue_unacked(queue, ind, BitBuffer::new(0), Some(alloc));
    }

}

fn chap_success_optional_section(chap_id: u8) -> String {
    let mut s = String::with_capacity(81);
    s.push('1');
    s.push_str("000");
    s.push('1');
    s.push_str(&format!("{PCO_TYPE34_ID:04b}"));
    s.push_str(&format!("{PCO_CHAP_SUCCESS_BITS:011b}"));
    s.push_str(&format!("{PPP_CONFIG_PROTOCOL_PPP:04b}"));
    s.push_str(&format!("{PPP_PROTO_CHAP:016b}"));
    s.push_str(&format!("{:08b}", 4));
    s.push_str(&format!("{CHAP_CODE_SUCCESS:08b}"));
    s.push_str(&format!("{chap_id:08b}"));
    s.push_str(&format!("{:016b}", 4));
    s.push('0');
    s
}

fn find_chap_response_id(demand: &str) -> Option<u8> {
    const CHAP_PROTO_ID: &str = "1100001000100011";
    let read = |off: usize| -> Option<u8> { demand.get(off..off + 8).and_then(|s| u8::from_str_radix(s, 2).ok()) };
    let mut fallback = None;
    let mut from = 0;
    while let Some(rel) = demand.get(from..).and_then(|s| s.find(CHAP_PROTO_ID)) {
        let marker = from + rel;
        match (read(marker + 24), read(marker + 32)) {
            (Some(2), Some(id)) => return Some(id),
            (Some(1), Some(id)) if fallback.is_none() => fallback = Some(id),
            _ => {}
        }
        from = marker + CHAP_PROTO_ID.len();
    }
    fallback
}

impl TetraEntityTrait for Sndcp {
    fn entity(&self) -> TetraEntity {
        TetraEntity::Sndcp
    }

    fn rx_prim(&mut self, queue: &mut MessageQueue, message: SapMsg) {
        let SapMsgInner::LtpdMleUnitdataInd(ind) = &message.msg else {
            tracing::debug!("SNDCP: unhandled primitive {:?}", message.msg);
            return;
        };
        if !self.profile_enabled() {
            tracing::debug!("SNDCP: WAP/IP profile disabled; packet-data PDU ignored");
            return;
        }

        let pdu = Self::rebase_sndcp_pdu(&ind.sdu);
        let Some(sn_type) = pdu.peek_bits(4).map(|v| v as u8) else {
            tracing::warn!("SNDCP: PDU shorter than type field");
            return;
        };
        tracing::info!(
            "SNDCP: <- type={} from ISSI={} bits={}",
            sn_type,
            ind.received_tetra_address.ssi,
            pdu.get_len()
        );
        match sn_type {
            SN_ACTIVATE_PDP_CONTEXT => self.handle_activate(queue, ind, &pdu),
            SN_DEACTIVATE_PDP_CONTEXT_DEMAND => self.handle_deactivate(queue, ind, &pdu),
            SN_UNITDATA | SN_DATA => self.handle_unitdata(queue, ind, &pdu),
            SN_DATA_TRANSMIT_REQUEST => self.handle_data_transmit_request(queue, ind, &pdu),
            SN_END_OF_DATA => self.handle_end_of_data(queue, ind),
            _ => tracing::debug!("SNDCP: unsupported SN-PDU type {}", sn_type),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex_to_bits(hex: &str) -> String {
        hex.chars().filter(|c| !c.is_whitespace()).map(|c| format!("{:04b}", c.to_digit(16).unwrap())).collect()
    }

    #[test]
    fn finds_chap_response_identifier_in_real_demand_pco() {
        let pco = hex_to_bits(
            "0c22318010500180aac20e0caf974bc75e02f44494d455452415f50\
             c2231a0205001a10db3b2df8c57cce0db8712b16aa9cb5a361646d696",
        );
        assert_eq!(find_chap_response_id(&pco), Some(5));
    }

    #[test]
    fn prefers_response_over_challenge_and_skips_non_chap_bits() {
        let mut s = String::from("101");
        s.push_str("1100001000100011");
        s.push_str("00000110");
        s.push_str("00000001");
        s.push_str("00001001");
        s.push_str("1100001000100011");
        s.push_str("00000110");
        s.push_str("00000010");
        s.push_str("00000111");
        assert_eq!(find_chap_response_id(&s), Some(7));
    }

    #[test]
    fn optional_section_layout_matches_spec() {
        let sec = chap_success_optional_section(5);
        assert_eq!(sec.len(), 81);
        assert_eq!(&sec[0..4], "1000");
        assert_eq!(&sec[5..9], "0001");
        assert_eq!(&sec[24..40], "1100001000100011");
        assert_eq!(&sec[48..56], "00000011");
        assert_eq!(&sec[56..64], "00000101");
        assert_eq!(&sec[80..81], "0");
    }

    #[test]
    fn rebases_both_mle_cursor_forms_without_dropping_payload_bits() {
        let mut complete = BitBuffer::new(3 + 8);
        complete.write_bits(MLE_DISCRIMINATOR_SNDCP, 3);
        complete.write_bits(0x42, 8);
        complete.seek(0);

        let from_start = Sndcp::rebase_sndcp_pdu(&complete);
        assert_eq!(from_start.get_len(), 8);
        assert_eq!(from_start.peek_bits(8), Some(0x42));

        let mut already_routed = BitBuffer::from_bitbuffer(&complete);
        assert_eq!(already_routed.read_bits(3), Some(MLE_DISCRIMINATOR_SNDCP));
        let from_cursor = Sndcp::rebase_sndcp_pdu(&already_routed);
        assert_eq!(from_cursor.get_len(), 8);
        assert_eq!(from_cursor.peek_bits(8), Some(0x42));
    }

    #[test]
    fn dynamic_pdp_accept_matches_reference_vector() {
        let pdu = Sndcp::build_pdp_accept_pdu(2, TIA_IPV4_DYNAMIC, [10, 0, 0, 226], None);
        assert_eq!(pdu.get_len(), 70);
        assert_eq!(pdu.into_bytes(), vec![0x02, 0x90, 0x8e, 0x82, 0x80, 0x00, 0x38, 0x80, 0x10]);
    }

    #[test]
    fn unitdata_header_reference_vector() {
        let npdu = [0x45, 0x00, 0x00, 0x14];
        let mut sn = BitBuffer::new(16 + npdu.len() * 8);
        sn.write_bits(4, 4);
        sn.write_bits(2, 4);
        sn.write_bits(0, 4);
        sn.write_bits(0, 4);
        for b in npdu { sn.write_bits(u64::from(b), 8); }
        assert_eq!(sn.into_bytes(), vec![0x42, 0x00, 0x45, 0x00, 0x00, 0x14]);
    }
}

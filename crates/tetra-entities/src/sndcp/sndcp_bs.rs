use std::collections::{HashMap, HashSet, VecDeque};
use std::net::Ipv4Addr;
use std::time::{Duration, Instant};

use crate::{MessageQueue, TetraEntityTrait};
use tetra_config::bluestation::SharedConfig;
use tetra_core::address::TetraAddress;
use tetra_core::tetra_entities::TetraEntity;
use tetra_core::{BitBuffer, Sap, TdmaTime, TimeslotOwner};
use tetra_saps::lcmc::enums::{alloc_type::ChanAllocType, ul_dl_assignment::UlDlAssignment};
use tetra_saps::lcmc::fields::chan_alloc_req::CmceChanAllocReq;
use tetra_saps::ltpd::LtpdMleUnitdataInd;
use tetra_saps::tla::{TlaTlDataReqBl, TlaTlUnitdataReqBl};
use tetra_saps::control::brew::BrewSubscriberAction;
use tetra_saps::{SapMsg, SapMsgInner};

use super::fragment::{fragment_ipv4_packet, Ipv4Reassembler};
use super::ip::{
    parse_ipv4_packet, parse_ipv4_packet_any, parse_udp_datagram, transport_ports, Ipv4Packet,
    IPV4_PROTOCOL_TCP, IPV4_PROTOCOL_UDP,
};
use super::packet_gateway::{PacketGateway, PacketGatewayConfig};
use super::protocol::{
    self, ActivateAccept, ActivateAddressAccept, ActivateAddressDemand, ActivateDemand, ActivateReject, DataPriority,
    DataPriorityDetails, DataTransmitRequest, DataTransmitResponse, Deactivate, EndOfData, Modify, RawBits, Reconnect,
    SnDirection, SnPdu, UserData,
};
use super::qos::QosProfile;
use super::resource::PhaseModulationResourceRequest;
use super::state::{
    ContextAvailability, ContextKey, ContextTable, ContextUsage, PdpContext, PdpState, TimerEvent, mtu_octets,
};
use super::wap_ip::{WapEndpoint, WapPolicy, build_response_npdu};
use super::wap_status::WapStatusSnapshot;

const MLE_DISCRIMINATOR_SNDCP: u64 = 0b100;
const SNDCP_PDCH_LOGICAL_TS: u8 = 2;
const RESPONSE_CACHE_TTL: Duration = Duration::from_secs(30);
const CACHE_MAX_ENTRIES: usize = 256;

// EN 300 392-2 activation reject causes used by the advertised capability profile.
const ACT_REJECT_IPV4_NOT_SUPPORTED: u8 = 2;
const ACT_REJECT_IPV6_NOT_SUPPORTED: u8 = 3;
const ACT_REJECT_POOL_EMPTY: u8 = 7;
const ACT_REJECT_STATIC_NOT_CORRECT: u8 = 8;
const ACT_REJECT_STATIC_IN_USE: u8 = 9;
const ACT_REJECT_STATIC_NOT_ALLOWED: u8 = 10;
const ACT_REJECT_MS_TYPE_NOT_SUPPORTED: u8 = 15;
const ACT_REJECT_MOBILE_IPV4_NOT_SUPPORTED: u8 = 17;
const ACT_REJECT_MOBILE_IPV4_COLOCATED_NOT_SUPPORTED: u8 = 18;
const ACT_REJECT_VERSION_NOT_SUPPORTED: u8 = 16;
const ACT_REJECT_MAX_CONTEXTS: u8 = 19;
const ACT_REJECT_MIN_THROUGHPUT: u8 = 23;
const ACT_REJECT_SCHEDULE_NOT_SUPPORTED: u8 = 24;
const ACT_REJECT_SCHEDULE_NOT_AVAILABLE: u8 = 25;
const ACT_REJECT_QOS_NOT_AVAILABLE: u8 = 26;
const ACT_REJECT_PRIMARY_MISSING: u8 = 28;
const ACT_REJECT_ASYMMETRIC_QOS_NOT_SUPPORTED: u8 = 29;
const ACT_REJECT_AUTOMATIC_FILTER_NOT_SUPPORTED: u8 = 30;
const ACT_REJECT_SPECIFIED_FILTER_NOT_SUPPORTED: u8 = 31;
const ACT_REJECT_FILTER_TYPE_NOT_SUPPORTED: u8 = 32;
const ACT_REJECT_FILTER_FOR_PRIMARY: u8 = 33;
const ACT_REJECT_TEMPORARILY_UNAVAILABLE: u8 = 34;

const TX_REJECT_UNKNOWN_NSAPI: u8 = 1;
const TX_REJECT_SYSTEM_RESOURCES: u8 = 2;
const TX_REJECT_MIN_THROUGHPUT: u8 = 23;
const TX_REJECT_SCHEDULE: u8 = 25;
const TX_REJECT_TEMPORARILY_UNAVAILABLE: u8 = 34;

const MODIFY_REJECT_UNKNOWN_NSAPI: u8 = 1;
const MODIFY_REJECT_MIN_THROUGHPUT: u8 = 23;
const MODIFY_REJECT_SCHEDULE_NOT_SUPPORTED: u8 = 24;
const MODIFY_REJECT_SCHEDULE_NOT_AVAILABLE: u8 = 25;
const MODIFY_REJECT_QOS_NOT_AVAILABLE: u8 = 26;
const MODIFY_REJECT_ASYMMETRIC_QOS_NOT_SUPPORTED: u8 = 29;
const MODIFY_REJECT_AUTOMATIC_FILTER_NOT_SUPPORTED: u8 = 30;
const MODIFY_REJECT_SPECIFIED_FILTER_NOT_SUPPORTED: u8 = 31;
const MODIFY_REJECT_FILTER_TYPE_NOT_SUPPORTED: u8 = 32;
const MODIFY_REJECT_FILTER_FOR_PRIMARY: u8 = 33;
const MODIFY_REJECT_TEMPORARILY_UNAVAILABLE: u8 = 34;

const PCO_TYPE34_ID: u64 = 1;
const PPP_PROTO_CHAP: u16 = 0xC223;
const PPP_PROTO_IPCP: u16 = 0x8021;
const PPP_CONFIG_PROTOCOL_PPP: u8 = 0;
const CHAP_CODE_SUCCESS: u8 = 3;
const IPCP_CODE_CONFIGURE_REQUEST: u8 = 1;
const IPCP_CODE_CONFIGURE_ACK: u8 = 2;
const IPCP_CODE_CONFIGURE_NAK: u8 = 3;
const IPCP_PRIMARY_DNS_OPTION: u8 = 129;
const IPCP_SECONDARY_DNS_OPTION: u8 = 131;

#[derive(Debug, Clone, Copy)]
struct RuntimeProfile {
    wap_enabled: bool,
    gateway_enabled: bool,
    gateway_prefix_len: u8,
    dns_servers: [Option<[u8; 4]>; 2],
    endpoint: [u8; 4],
    port: u16,
    ttl: u8,
    pool_prefix: [u8; 3],
    pool_first: u8,
    pool_last: u8,
    allow_static: bool,
    accept_empty_probe: bool,
    accept_root_path: bool,
    accept_status_path: bool,
    accept_status_wml_path: bool,
    max_request_payload_bytes: usize,
    assume_ready: bool,
    pdu_priority_max: u8,
    ready_timer_code: u8,
    standby_timer_code: u8,
    response_wait_timer_code: u8,
    mtu_code: u8,
    mtu_octets: usize,
    network_default_priority: u8,
    max_contexts_per_issi: usize,
    max_total_contexts: usize,
    strict_source_address: bool,
}

#[derive(Debug, Clone, Copy)]
struct SubscriberRoute {
    main_address: TetraAddress,
    link_id: u32,
    endpoint_id: u32,
}

#[derive(Debug, Clone)]
struct CachedReply {
    sn_pdu: BitBuffer,
    acknowledged: bool,
    include_pdch_allocation: bool,
    quit_channel: bool,
}

#[derive(Debug, Clone)]
struct CachedExchange {
    expires_at: Instant,
    replies: Vec<CachedReply>,
}

#[derive(Debug, Clone)]
struct PendingPacket {
    queued_at: Instant,
    octets: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct AutomaticFlowKey {
    protocol: u8,
    mobile_address: [u8; 4],
    mobile_port: u16,
    remote_address: [u8; 4],
    remote_port: u16,
}

#[derive(Debug, Clone, Copy)]
struct AutomaticBinding {
    context: ContextKey,
    expires_at: Instant,
}

pub struct Sndcp {
    config: SharedConfig,
    contexts: ContextTable,
    started_at: Instant,
    last_activity: String,
    pdch_reserved: bool,
    response_cache: HashMap<(u32, String), CachedExchange>,
    routes: HashMap<u32, SubscriberRoute>,
    gateway: Option<PacketGateway>,
    gateway_config: Option<PacketGatewayConfig>,
    gateway_retry_at: Instant,
    pending_downlink: HashMap<ContextKey, VecDeque<PendingPacket>>,
    page_inflight: HashMap<ContextKey, Instant>,
    automatic_bindings: HashMap<AutomaticFlowKey, AutomaticBinding>,
    uplink_reassembler: Ipv4Reassembler,
    downlink_reassembler: Ipv4Reassembler,
    last_timer_sweep: Instant,
}

impl Sndcp {
    pub fn new(config: SharedConfig) -> Self {
        let now = Instant::now();
        let packet = config.config().cell.packet_data_gateway.clone();
        let reassembly_timeout = Duration::from_secs(packet.fragment_reassembly_timeout_secs);
        let reassembly_datagrams = packet.fragment_reassembly_max_datagrams;
        let reassembly_bytes = packet.fragment_reassembly_max_bytes;
        Self {
            config,
            contexts: ContextTable::default(),
            started_at: now,
            last_activity: "SNDCP ready".to_string(),
            pdch_reserved: false,
            response_cache: HashMap::new(),
            routes: HashMap::new(),
            gateway: None,
            gateway_config: None,
            gateway_retry_at: now,
            pending_downlink: HashMap::new(),
            page_inflight: HashMap::new(),
            automatic_bindings: HashMap::new(),
            uplink_reassembler: Ipv4Reassembler::new(reassembly_timeout, reassembly_datagrams, reassembly_bytes),
            downlink_reassembler: Ipv4Reassembler::new(reassembly_timeout, reassembly_datagrams, reassembly_bytes),
            last_timer_sweep: now,
        }
    }

    fn profile_enabled(&self) -> bool {
        self.config.config().cell.wap_ip_sndcp_profile_enabled()
    }

    fn profile(&self) -> Option<RuntimeProfile> {
        let cfg = self.config.config();
        let wap = &cfg.cell.wap_ip;
        let pool_prefix = wap.pool_prefix_octets()?;
        let mtu_octets = mtu_octets(wap.mtu_code)?;
        let gateway = &cfg.cell.packet_data_gateway;
        let mut dns_servers = [None, None];
        for (slot, address) in dns_servers.iter_mut().zip(gateway.dns_servers.iter().copied()) {
            *slot = Some(address.octets());
        }
        Some(RuntimeProfile {
            wap_enabled: wap.enabled,
            gateway_enabled: gateway.enabled,
            gateway_prefix_len: gateway.prefix_len,
            dns_servers,
            endpoint: wap.address.octets(),
            port: wap.port,
            ttl: wap.response_ttl,
            pool_prefix,
            pool_first: wap.dynamic_pool_first_host,
            pool_last: wap.dynamic_pool_last_host,
            allow_static: wap.allow_static_ipv4,
            accept_empty_probe: wap.accept_empty_probe,
            accept_root_path: wap.accept_root_path,
            accept_status_path: wap.accept_status_path,
            accept_status_wml_path: wap.accept_status_wml_path,
            max_request_payload_bytes: wap.max_request_payload_bytes,
            assume_ready: wap.assume_pdch_ready_after_data_transmit,
            pdu_priority_max: wap.pdu_priority_max,
            ready_timer_code: wap.ready_timer_code,
            standby_timer_code: wap.standby_timer_code,
            response_wait_timer_code: wap.response_wait_timer_code,
            mtu_code: wap.mtu_code,
            mtu_octets,
            network_default_priority: wap.network_default_data_priority,
            max_contexts_per_issi: wap.max_contexts_per_issi,
            max_total_contexts: wap.max_total_contexts,
            strict_source_address: wap.strict_source_address,
        })
    }

    /// MLE normally leaves the cursor just after its three-bit discriminator. Older
    /// callers/tests may pass a cursor-at-zero buffer, so support both forms.
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

    fn wrap_sndcp(sn_pdu: &BitBuffer) -> BitBuffer {
        let mut source = BitBuffer::from_bitbuffer(sn_pdu);
        source.seek(0);
        let len = source.get_len_remaining();
        let mut tl_sdu = BitBuffer::new(3 + len);
        tl_sdu.write_bits(MLE_DISCRIMINATOR_SNDCP, 3);
        tl_sdu.copy_bits(&mut source, len);
        tl_sdu.seek(0);
        tl_sdu
    }

    fn pdch_allocation() -> CmceChanAllocReq {
        CmceChanAllocReq {
            usage: None,
            carrier: None,
            timeslots: [false, true, false, false],
            alloc_type: ChanAllocType::Replace,
            ul_dl_assigned: UlDlAssignment::Both,
        }
    }

    fn quit_allocation() -> CmceChanAllocReq {
        CmceChanAllocReq {
            usage: None,
            carrier: None,
            timeslots: [false; 4],
            alloc_type: ChanAllocType::QuitAndGo,
            ul_dl_assigned: UlDlAssignment::Both,
        }
    }

    fn reserve_pdch(&mut self) -> bool {
        if self.pdch_reserved {
            return true;
        }
        let reserved = self
            .config
            .state_write()
            .timeslot_alloc
            .reserve(TimeslotOwner::Sndcp, SNDCP_PDCH_LOGICAL_TS)
            .is_ok();
        self.pdch_reserved = reserved;
        reserved
    }

    fn release_pdch(&mut self) {
        if !self.pdch_reserved {
            return;
        }
        let result = self
            .config
            .state_write()
            .timeslot_alloc
            .release(TimeslotOwner::Sndcp, SNDCP_PDCH_LOGICAL_TS);
        if let Err(error) = result {
            tracing::warn!("SNDCP: failed to release PDCH TS{}: {:?}", SNDCP_PDCH_LOGICAL_TS, error);
        }
        self.pdch_reserved = false;
    }

    fn queue_acked_to(
        &self,
        queue: &mut MessageQueue,
        route: SubscriberRoute,
        sn_pdu: &BitBuffer,
        chan_alloc: Option<CmceChanAllocReq>,
    ) {
        queue.push_back(SapMsg {
            sap: Sap::TlaSap,
            src: TetraEntity::Sndcp,
            dest: TetraEntity::Llc,
            msg: SapMsgInner::TlaTlDataReqBl(TlaTlDataReqBl {
                main_address: route.main_address,
                link_id: route.link_id,
                endpoint_id: route.endpoint_id,
                tl_sdu: Self::wrap_sndcp(sn_pdu),
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

    fn queue_unacked_to(
        &self,
        queue: &mut MessageQueue,
        route: SubscriberRoute,
        sn_pdu: &BitBuffer,
        chan_alloc: Option<CmceChanAllocReq>,
    ) {
        queue.push_back(SapMsg {
            sap: Sap::TlaSap,
            src: TetraEntity::Sndcp,
            dest: TetraEntity::Llc,
            msg: SapMsgInner::TlaTlUnitdataReqBl(TlaTlUnitdataReqBl {
                main_address: route.main_address,
                link_id: route.link_id,
                endpoint_id: route.endpoint_id,
                tl_sdu: Self::wrap_sndcp(sn_pdu),
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

    fn queue_acked(
        &self,
        queue: &mut MessageQueue,
        ind: &LtpdMleUnitdataInd,
        sn_pdu: &BitBuffer,
        chan_alloc: Option<CmceChanAllocReq>,
    ) {
        self.queue_acked_to(
            queue,
            SubscriberRoute {
                main_address: ind.received_tetra_address,
                link_id: ind.link_id,
                endpoint_id: ind.endpoint_id,
            },
            sn_pdu,
            chan_alloc,
        );
    }

    fn queue_unacked(
        &self,
        queue: &mut MessageQueue,
        ind: &LtpdMleUnitdataInd,
        sn_pdu: &BitBuffer,
        chan_alloc: Option<CmceChanAllocReq>,
    ) {
        queue.push_back(SapMsg {
            sap: Sap::TlaSap,
            src: TetraEntity::Sndcp,
            dest: TetraEntity::Llc,
            msg: SapMsgInner::TlaTlUnitdataReqBl(TlaTlUnitdataReqBl {
                main_address: ind.received_tetra_address,
                link_id: ind.link_id,
                endpoint_id: ind.endpoint_id,
                tl_sdu: Self::wrap_sndcp(sn_pdu),
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

    fn queue_quit(&self, queue: &mut MessageQueue, ind: &LtpdMleUnitdataInd) {
        queue.push_back(SapMsg {
            sap: Sap::TlaSap,
            src: TetraEntity::Sndcp,
            dest: TetraEntity::Llc,
            msg: SapMsgInner::TlaTlUnitdataReqBl(TlaTlUnitdataReqBl {
                main_address: ind.received_tetra_address,
                link_id: ind.link_id,
                endpoint_id: ind.endpoint_id,
                tl_sdu: BitBuffer::new(0),
                stealing_permission: false,
                subscriber_class: 0,
                fcs_flag: false,
                air_interface_encryption: None,
                packet_data_flag: true,
                n_tlsdu_repeats: 0,
                data_class_info: None,
                req_handle: 0,
                chan_alloc: Some(Self::quit_allocation()),
                tx_reporter: None,
            }),
        });
    }

    fn emit_reply(&self, queue: &mut MessageQueue, ind: &LtpdMleUnitdataInd, reply: &CachedReply) {
        if reply.quit_channel && reply.sn_pdu.get_len() == 0 {
            self.queue_quit(queue, ind);
            return;
        }
        let alloc = if reply.quit_channel {
            Some(Self::quit_allocation())
        } else {
            reply.include_pdch_allocation.then(Self::pdch_allocation)
        };
        if reply.acknowledged {
            self.queue_acked(queue, ind, &reply.sn_pdu, alloc);
        } else {
            self.queue_unacked(queue, ind, &reply.sn_pdu, alloc);
        }
    }

    fn cache_and_emit(
        &mut self,
        queue: &mut MessageQueue,
        ind: &LtpdMleUnitdataInd,
        request_fingerprint: &str,
        replies: Vec<CachedReply>,
    ) {
        for reply in &replies {
            self.emit_reply(queue, ind, reply);
        }
        if self.response_cache.len() >= CACHE_MAX_ENTRIES {
            let now = Instant::now();
            self.response_cache.retain(|_, exchange| exchange.expires_at > now);
            if self.response_cache.len() >= CACHE_MAX_ENTRIES {
                self.response_cache.clear();
            }
        }
        self.response_cache.insert(
            (ind.received_tetra_address.ssi, request_fingerprint.to_string()),
            CachedExchange { expires_at: Instant::now() + RESPONSE_CACHE_TTL, replies },
        );
    }

    fn replay_cached(
        &mut self,
        queue: &mut MessageQueue,
        ind: &LtpdMleUnitdataInd,
        request_fingerprint: &str,
    ) -> bool {
        let key = (ind.received_tetra_address.ssi, request_fingerprint.to_string());
        let now = Instant::now();
        let Some(exchange) = self.response_cache.get(&key).cloned() else {
            return false;
        };
        if exchange.expires_at <= now {
            self.response_cache.remove(&key);
            return false;
        }
        tracing::debug!("SNDCP: replaying cached response for ISSI={}", key.0);
        for reply in &exchange.replies {
            self.emit_reply(queue, ind, reply);
        }
        true
    }

    fn zero_optional() -> RawBits {
        RawBits { bytes: vec![0], bit_len: 1 }
    }

    fn raw_bits_from_string(bits: &str) -> RawBits {
        let mut bytes = vec![0u8; bits.len().div_ceil(8)];
        for (index, bit) in bits.bytes().enumerate() {
            if bit == b'1' {
                bytes[index / 8] |= 1 << (7 - (index % 8));
            }
        }
        RawBits { bytes, bit_len: bits.len() }
    }

    fn encoded(pdu: SnPdu) -> BitBuffer {
        protocol::encode(&pdu)
    }

    fn reply(pdu: SnPdu, acknowledged: bool, include_pdch_allocation: bool) -> CachedReply {
        CachedReply {
            sn_pdu: Self::encoded(pdu),
            acknowledged,
            include_pdch_allocation,
            quit_channel: false,
        }
    }

    fn activation_reject(nsapi: u8, cause: u8) -> CachedReply {
        Self::reply(
            SnPdu::ActivateReject(ActivateReject { nsapi, cause, optional: Self::zero_optional() }),
            true,
            false,
        )
    }

    fn transmit_response_nsapis(
        mut nsapis: Vec<u8>,
        accepted: bool,
        reject_cause: Option<u8>,
        with_alloc: bool,
        snei: Option<u16>,
    ) -> CachedReply {
        let mut seen = HashSet::new();
        nsapis.retain(|nsapi| (1..=14).contains(nsapi) && seen.insert(*nsapi));
        if nsapis.is_empty() {
            nsapis.push(1);
        }
        let optional = Self::raw_bits_from_string(&data_transmit_response_optional_section(snei, &nsapis[1..]));
        Self::reply(
            SnPdu::DataTransmitResponse(DataTransmitResponse {
                nsapis,
                accepted,
                reject_cause,
                optional,
            }),
            true,
            with_alloc,
        )
    }

    fn transmit_response(
        nsapi: u8,
        accepted: bool,
        reject_cause: Option<u8>,
        with_alloc: bool,
        snei: Option<u16>,
    ) -> CachedReply {
        Self::transmit_response_nsapis(vec![nsapi], accepted, reject_cause, with_alloc, snei)
    }

    fn check_network_endpoint_id(&self, issi: u32, supplied: Option<u16>, pdu_name: &str) {
        let Some(supplied) = supplied else {
            return;
        };
        if let Some(expected) = self.contexts.network_endpoint_id(issi) {
            if supplied != expected {
                tracing::warn!(
                    "SNDCP: {} SNEI mismatch ISSI={} supplied={} expected={}",
                    pdu_name,
                    issi,
                    supplied,
                    expected
                );
            }
        }
    }

    fn validate_qos(qos: &QosProfile, primary_context: bool) -> Result<(), u8> {
        let QosProfile::Negotiated { asymmetrical, filter, scheduled, uplink, downlink, .. } = qos else {
            return Ok(());
        };
        if *asymmetrical || downlink.is_some() {
            return Err(ACT_REJECT_ASYMMETRIC_QOS_NOT_SUPPORTED);
        }
        if scheduled.is_some() {
            return Err(ACT_REJECT_SCHEDULE_NOT_SUPPORTED);
        }
        // The one-slot bearer supports the standard QoS metadata, but cannot
        // guarantee a minimum above the configured one-slot resource.
        if uplink.minimum_peak_throughput > 8 {
            return Err(ACT_REJECT_MIN_THROUGHPUT);
        }
        if let Some(filter) = filter {
            if primary_context {
                return Err(ACT_REJECT_FILTER_FOR_PRIMARY);
            }
            if filter.is_reserved_type() {
                return Err(ACT_REJECT_FILTER_TYPE_NOT_SUPPORTED);
            }
            // Automatic and explicit filters are retained in the context. WAP
            // replies remain bound to the requesting NSAPI, while generic
            // downlink selection can use the same stored filter metadata.
        }
        Ok(())
    }

    fn validate_resource_request(request: Option<PhaseModulationResourceRequest>) -> Result<(), u8> {
        let Some(request) = request else { return Ok(()); };
        if request.uplink_slots() > 1 || request.downlink_slots() > 1 {
            return Err(TX_REJECT_SYSTEM_RESOURCES);
        }
        // Relative throughput values are valid on a one-slot allocation. Value
        // 6 (unspecified resource) has already been cross-validated by the codec.
        Ok(())
    }

    fn modify_qos_cause(cause: u8) -> u8 {
        match cause {
            ACT_REJECT_MIN_THROUGHPUT => MODIFY_REJECT_MIN_THROUGHPUT,
            ACT_REJECT_SCHEDULE_NOT_SUPPORTED => MODIFY_REJECT_SCHEDULE_NOT_SUPPORTED,
            ACT_REJECT_SCHEDULE_NOT_AVAILABLE => MODIFY_REJECT_SCHEDULE_NOT_AVAILABLE,
            ACT_REJECT_ASYMMETRIC_QOS_NOT_SUPPORTED => MODIFY_REJECT_ASYMMETRIC_QOS_NOT_SUPPORTED,
            ACT_REJECT_AUTOMATIC_FILTER_NOT_SUPPORTED => MODIFY_REJECT_AUTOMATIC_FILTER_NOT_SUPPORTED,
            ACT_REJECT_SPECIFIED_FILTER_NOT_SUPPORTED => MODIFY_REJECT_SPECIFIED_FILTER_NOT_SUPPORTED,
            ACT_REJECT_FILTER_TYPE_NOT_SUPPORTED => MODIFY_REJECT_FILTER_TYPE_NOT_SUPPORTED,
            ACT_REJECT_FILTER_FOR_PRIMARY => MODIFY_REJECT_FILTER_FOR_PRIMARY,
            ACT_REJECT_TEMPORARILY_UNAVAILABLE => MODIFY_REJECT_TEMPORARILY_UNAVAILABLE,
            _ => MODIFY_REJECT_QOS_NOT_AVAILABLE,
        }
    }

    fn not_supported(pdu_type: u8) -> CachedReply {
        Self::reply(SnPdu::NotSupported { pdu_type }, true, false)
    }

    fn dynamic_address(&self, key: ContextKey, profile: RuntimeProfile) -> Option<[u8; 4]> {
        if let Some(existing) = self.contexts.get(key) {
            return Some(existing.address);
        }
        let used = self.contexts.addresses().collect::<HashSet<_>>();
        (profile.pool_first..=profile.pool_last)
            .map(|host| [profile.pool_prefix[0], profile.pool_prefix[1], profile.pool_prefix[2], host])
            .find(|address| !used.contains(address))
    }

    fn valid_static_address(address: [u8; 4], endpoint: [u8; 4], prefix_len: u8, routed: bool) -> bool {
        let ip = Ipv4Addr::from(address);
        if ip.is_unspecified() || ip.is_multicast() || address == Ipv4Addr::BROADCAST.octets() || address == endpoint {
            return false;
        }
        if !routed {
            return true;
        }
        let mask = u32::MAX << (32 - prefix_len);
        let network = u32::from(Ipv4Addr::from(endpoint)) & mask;
        let candidate = u32::from(ip);
        let broadcast = network | !mask;
        candidate & mask == network && candidate != network && candidate != broadcast
    }

    fn ensure_gateway(&mut self, profile: RuntimeProfile) {
        if !profile.gateway_enabled {
            self.gateway = None;
            self.gateway_config = None;
            return;
        }
        let cfg = self.config.config().cell.packet_data_gateway.clone();
        let desired = PacketGatewayConfig::from_cell(&cfg, Ipv4Addr::from(profile.endpoint), profile.mtu_octets);
        if self.gateway.is_some() && self.gateway_config.as_ref() == Some(&desired) {
            return;
        }
        if self.gateway_config.as_ref().is_some_and(|current| current != &desired) {
            self.gateway = None;
            self.gateway_config = None;
        }
        let now = Instant::now();
        if now < self.gateway_retry_at {
            return;
        }
        match PacketGateway::spawn(desired.clone()) {
            Ok(gateway) => {
                tracing::info!(
                    "SNDCP: packet gateway up interface={} subnet={} NAT={:?}",
                    desired.interface_name,
                    desired.network_cidr(),
                    desired.nat_mode
                );
                self.gateway = Some(gateway);
                self.gateway_config = Some(desired);
            }
            Err(error) => {
                tracing::error!("SNDCP: packet gateway startup failed: {:?}", error);
                self.gateway_retry_at = now + Duration::from_secs(30);
            }
        }
    }

    fn automatic_flow_key_uplink(ip: &Ipv4Packet<'_>) -> Option<AutomaticFlowKey> {
        let ports = transport_ports(ip)?;
        Some(AutomaticFlowKey {
            protocol: ip.protocol,
            mobile_address: ip.source,
            mobile_port: ports.source,
            remote_address: ip.destination,
            remote_port: ports.destination,
        })
    }

    fn automatic_flow_key_downlink(ip: &Ipv4Packet<'_>) -> Option<AutomaticFlowKey> {
        let ports = transport_ports(ip)?;
        Some(AutomaticFlowKey {
            protocol: ip.protocol,
            mobile_address: ip.destination,
            mobile_port: ports.destination,
            remote_address: ip.source,
            remote_port: ports.source,
        })
    }

    fn static_filter_matches(ip: &Ipv4Packet<'_>, filter: super::qos::QosFilter) -> bool {
        if filter.operation == 2 {
            return false;
        }
        if filter.filter_type == 7 {
            return filter.first.is_some_and(|value| (value & 0x3f) as u8 == ip.dscp());
        }
        let Some(ports) = transport_ports(ip) else { return false; };
        let protocol_matches = match filter.filter_type {
            1 | 4 => matches!(ip.protocol, IPV4_PROTOCOL_TCP | IPV4_PROTOCOL_UDP),
            2 | 5 => ip.protocol == IPV4_PROTOCOL_UDP,
            3 | 6 => ip.protocol == IPV4_PROTOCOL_TCP,
            _ => false,
        };
        if !protocol_matches {
            return false;
        }
        match filter.filter_type {
            1..=3 => filter.first == Some(ports.destination),
            4..=6 => filter
                .first
                .zip(filter.second)
                .is_some_and(|(low, high)| low <= ports.destination && ports.destination <= high),
            _ => false,
        }
    }

    fn learn_automatic_binding(&mut self, key: ContextKey, ip: &Ipv4Packet<'_>) {
        let filter = self.contexts.get(key).and_then(|context| context.qos.filter());
        let Some(filter) = filter else { return; };
        if !filter.is_automatic() || filter.operation == 2 {
            return;
        }
        let Some(flow) = Self::automatic_flow_key_uplink(ip) else { return; };
        let cfg = self.config.config().cell.packet_data_gateway.clone();
        let now = Instant::now();
        self.automatic_bindings.retain(|_, binding| binding.expires_at > now);
        if !self.automatic_bindings.contains_key(&flow)
            && self.automatic_bindings.len() >= cfg.automatic_filter_max_bindings
        {
            if let Some(oldest) = self.automatic_bindings.iter().min_by_key(|(_, binding)| binding.expires_at).map(|(flow, _)| *flow) {
                self.automatic_bindings.remove(&oldest);
            }
        }
        self.automatic_bindings.insert(flow, AutomaticBinding {
            context: key,
            expires_at: now + Duration::from_secs(cfg.automatic_filter_ttl_secs),
        });
    }

    fn select_downlink_context(&self, packet: &Ipv4Packet<'_>) -> Option<ContextKey> {
        if let Some(flow) = Self::automatic_flow_key_downlink(packet)
            && let Some(binding) = self.automatic_bindings.get(&flow)
            && binding.expires_at > Instant::now()
            && self.contexts.get(binding.context).is_some_and(|context| {
                context.address == packet.destination
                    && context.availability == ContextAvailability::Available
                    && context.usage == ContextUsage::Active
            })
        {
            return Some(binding.context);
        }
        self.contexts
            .keys_for_address(packet.destination)
            .filter_map(|key| {
                let context = self.contexts.get(key)?;
                if context.availability != ContextAvailability::Available || context.usage != ContextUsage::Active {
                    return None;
                }
                let filter_rank = match context.qos.filter() {
                    Some(filter) if filter.is_automatic() => return None,
                    Some(filter) if Self::static_filter_matches(packet, filter) => 0u8,
                    Some(_) => return None,
                    None if context.primary_nsapi.is_none() => 1,
                    None => 2,
                };
                let state_rank = match context.state {
                    PdpState::Ready => 0u8,
                    PdpState::Quiescent => 1,
                    PdpState::Standby => 2,
                    PdpState::Suspended => 3,
                };
                Some(((filter_rank, state_rank, key.nsapi), key))
            })
            .min_by_key(|(rank, _)| *rank)
            .map(|(_, key)| key)
    }

    fn enqueue_pending_packet(&mut self, key: ContextKey, packet: Vec<u8>) {
        let cfg = self.config.config().cell.packet_data_gateway.clone();
        let now = Instant::now();
        let queue = self.pending_downlink.entry(key).or_default();
        let mut bytes = queue.iter().map(|item| item.octets.len()).sum::<usize>();
        while !queue.is_empty()
            && (queue.len() >= cfg.downlink_queue_packets_per_context
                || bytes.saturating_add(packet.len()) > cfg.downlink_queue_bytes_per_context)
        {
            if let Some(dropped) = queue.pop_front() {
                bytes = bytes.saturating_sub(dropped.octets.len());
            }
        }
        if packet.len() <= cfg.downlink_queue_bytes_per_context {
            queue.push_back(PendingPacket { queued_at: now, octets: packet });
        }
    }

    fn pending_replies(&mut self, key: ContextKey, mtu: usize) -> Vec<CachedReply> {
        let Some(mut queue) = self.pending_downlink.remove(&key) else { return Vec::new(); };
        let mut replies = Vec::new();
        while let Some(packet) = queue.pop_front() {
            match fragment_ipv4_packet(&packet.octets, mtu) {
                Ok(fragments) => replies.extend(fragments.into_iter().map(|fragment| Self::reply(
                    SnPdu::Unitdata(UserData {
                        acknowledged: false,
                        nsapi: key.nsapi,
                        pcomp: 0,
                        dcomp: 0,
                        n_pdu: protocol::raw_octets(fragment),
                    }),
                    false,
                    false,
                ))),
                Err(error) => tracing::warn!("SNDCP: queued downlink fragmentation failed: {:?}", error),
            }
        }
        self.page_inflight.remove(&key);
        replies
    }

    fn page_context(&mut self, queue: &mut MessageQueue, key: ContextKey) {
        let cfg = self.config.config().cell.packet_data_gateway.clone();
        let now = Instant::now();
        if self.page_inflight.get(&key).is_some_and(|last| now.duration_since(*last) < Duration::from_secs(cfg.page_retry_secs)) {
            return;
        }
        let Some(route) = self.routes.get(&key.issi).copied() else { return; };
        let optional = Self::raw_bits_from_string(&snei_optional_section(self.contexts.network_endpoint_id(key.issi)));
        let pdu = Self::encoded(SnPdu::PageRequest(protocol::PageRequest {
            nsapi: key.nsapi,
            reply_requested: true,
            optional,
        }));
        self.queue_acked_to(queue, route, &pdu, None);
        self.page_inflight.insert(key, now);
        tracing::debug!("SNDCP: paged ISSI={} NSAPI={} for queued downlink", key.issi, key.nsapi);
    }

    fn flush_pending_to_queue(&mut self, queue: &mut MessageQueue, key: ContextKey) {
        let Some(context) = self.contexts.get(key).cloned() else { return; };
        let Some(route) = self.routes.get(&key.issi).copied() else { return; };
        for reply in self.pending_replies(key, context.mtu_octets) {
            self.queue_unacked_to(queue, route, &reply.sn_pdu, None);
        }
    }

    fn route_gateway_downlink(&mut self, queue: &mut MessageQueue, packet: Vec<u8>) {
        let now = Instant::now();
        let parsed = match parse_ipv4_packet_any(&packet) {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!("SNDCP: kernel emitted invalid IPv4 packet: {:?}", error);
                return;
            }
        };
        let packet = if parsed.is_fragmented() {
            match self.downlink_reassembler.push(&packet[..parsed.total_len], now) {
                Ok(Some(value)) => value,
                Ok(None) => return,
                Err(error) => {
                    tracing::warn!("SNDCP: downlink IPv4 reassembly failed: {:?}", error);
                    return;
                }
            }
        } else {
            packet[..parsed.total_len].to_vec()
        };
        let parsed = match parse_ipv4_packet(&packet) {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!("SNDCP: reassembled downlink packet invalid: {:?}", error);
                return;
            }
        };
        let Some(key) = self.select_downlink_context(&parsed) else {
            tracing::debug!("SNDCP: no active PDP context for downlink destination={}", Ipv4Addr::from(parsed.destination));
            return;
        };
        let direct = self.contexts.bearer_owner() == Some(key.issi)
            && self.contexts.get(key).is_some_and(|context| context.state == PdpState::Ready);
        if direct {
            self.enqueue_pending_packet(key, packet);
            self.flush_pending_to_queue(queue, key);
        } else {
            self.enqueue_pending_packet(key, packet);
            self.page_context(queue, key);
        }
    }

    fn poll_gateway(&mut self, queue: &mut MessageQueue, profile: RuntimeProfile) {
        self.ensure_gateway(profile);
        for _ in 0..64 {
            let result = match self.gateway.as_ref() {
                Some(gateway) => gateway.try_packet_to_mobile(),
                None => break,
            };
            match result {
                Ok(Some(packet)) => self.route_gateway_downlink(queue, packet),
                Ok(None) => break,
                Err(error) => {
                    tracing::warn!("SNDCP: packet gateway receive failed: {:?}", error);
                    self.gateway = None;
                    self.gateway_config = None;
                    self.gateway_retry_at = Instant::now() + Duration::from_secs(30);
                    break;
                }
            }
        }
        let cfg = self.config.config().cell.packet_data_gateway.clone();
        let now = Instant::now();
        let ttl = Duration::from_secs(cfg.downlink_queue_ttl_secs);
        self.pending_downlink.retain(|_, packets| {
            packets.retain(|packet| now.duration_since(packet.queued_at) < ttl);
            !packets.is_empty()
        });
        let keys = self.pending_downlink.keys().copied().collect::<Vec<_>>();
        for key in keys {
            if self.contexts.bearer_owner() == Some(key.issi)
                && self.contexts.get(key).is_some_and(|context| context.state == PdpState::Ready)
            {
                self.flush_pending_to_queue(queue, key);
            } else {
                self.page_context(queue, key);
            }
        }
    }

    fn handle_activate(
        &mut self,
        demand: ActivateDemand,
        raw_request: &BitBuffer,
        ind: &LtpdMleUnitdataInd,
        profile: RuntimeProfile,
    ) -> Vec<CachedReply> {
        let issi = ind.received_tetra_address.ssi;
        let key = ContextKey { issi, nsapi: demand.nsapi };
        if demand.version != protocol::SNDCP_VERSION_1 {
            return vec![Self::activation_reject(demand.nsapi, ACT_REJECT_VERSION_NOT_SUPPORTED)];
        }
        if demand.packet_data_ms_type > 3 {
            return vec![Self::activation_reject(demand.nsapi, ACT_REJECT_MS_TYPE_NOT_SUPPORTED)];
        }
        let replacing = self.contexts.get(key).is_some();
        if !replacing && self.contexts.len() >= profile.max_total_contexts {
            return vec![Self::activation_reject(demand.nsapi, ACT_REJECT_TEMPORARILY_UNAVAILABLE)];
        }
        if !replacing && self.contexts.contexts_for_issi(issi) >= profile.max_contexts_per_issi {
            return vec![Self::activation_reject(demand.nsapi, ACT_REJECT_MAX_CONTEXTS)];
        }
        let qos = match demand.qos() {
            Ok(Some(qos)) => qos,
            Ok(None) => QosProfile::Background,
            Err(error) => {
                tracing::warn!("SNDCP: invalid activation QoS ISSI={} NSAPI={}: {:?}", issi, demand.nsapi, error);
                return vec![Self::activation_reject(demand.nsapi, ACT_REJECT_QOS_NOT_AVAILABLE)];
            }
        };

        let (address, accepted_address, primary_nsapi) = match demand.address {
            ActivateAddressDemand::Ipv4Static(address) if profile.allow_static => {
                if !Self::valid_static_address(address, profile.endpoint, profile.gateway_prefix_len, profile.gateway_enabled) {
                    return vec![Self::activation_reject(demand.nsapi, ACT_REJECT_STATIC_NOT_CORRECT)];
                }
                if self.contexts.address_in_use_by_other(key, address) {
                    return vec![Self::activation_reject(demand.nsapi, ACT_REJECT_STATIC_IN_USE)];
                }
                (address, ActivateAddressAccept::Ipv4Static(address), None)
            }
            ActivateAddressDemand::Ipv4Static(_) => {
                return vec![Self::activation_reject(demand.nsapi, ACT_REJECT_STATIC_NOT_ALLOWED)];
            }
            ActivateAddressDemand::Ipv4Dynamic => {
                let Some(address) = self.dynamic_address(key, profile) else {
                    return vec![Self::activation_reject(demand.nsapi, ACT_REJECT_POOL_EMPTY)];
                };
                (address, ActivateAddressAccept::Ipv4Dynamic(address), None)
            }
            ActivateAddressDemand::Ipv6 => {
                return vec![Self::activation_reject(demand.nsapi, ACT_REJECT_IPV6_NOT_SUPPORTED)];
            }
            ActivateAddressDemand::MobileIpv4ForeignAgent => {
                return vec![Self::activation_reject(demand.nsapi, ACT_REJECT_MOBILE_IPV4_NOT_SUPPORTED)];
            }
            ActivateAddressDemand::MobileIpv4CoLocated => {
                return vec![Self::activation_reject(
                    demand.nsapi,
                    ACT_REJECT_MOBILE_IPV4_COLOCATED_NOT_SUPPORTED,
                )];
            }
            ActivateAddressDemand::Secondary { primary_nsapi } => {
                let Some(primary) = self.contexts.get(ContextKey { issi, nsapi: primary_nsapi }).cloned() else {
                    return vec![Self::activation_reject(demand.nsapi, ACT_REJECT_PRIMARY_MISSING)];
                };
                if primary.primary_nsapi.is_some() {
                    return vec![Self::activation_reject(demand.nsapi, ACT_REJECT_PRIMARY_MISSING)];
                }
                (primary.address, ActivateAddressAccept::None, Some(primary_nsapi))
            }
            ActivateAddressDemand::Reserved(_) => {
                return vec![Self::activation_reject(demand.nsapi, ACT_REJECT_IPV4_NOT_SUPPORTED)];
            }
        };

        if let Err(cause) = Self::validate_qos(&qos, primary_nsapi.is_none()) {
            return vec![Self::activation_reject(demand.nsapi, cause)];
        }

        // Compression negotiation is capability negotiation, not a demand that the
        // SwMI must accept. NetCore-Tetra answers PCOMP=0 and later rejects compressed
        // user PDUs rather than falsely advertising algorithms it does not implement.
        let now = Instant::now();
        if replacing {
            self.pending_downlink.remove(&key);
            self.page_inflight.remove(&key);
            self.automatic_bindings.retain(|_, binding| binding.context != key);
        }
        if replacing && self.contexts.release_bearer_nsapis(issi, &[demand.nsapi]) {
            self.release_pdch();
        }
        let snei = self.contexts.ensure_network_endpoint_id(issi);
        let mut context = PdpContext::new(
            address,
            profile.pdu_priority_max,
            profile.mtu_octets,
            profile.standby_timer_code,
            now,
        );
        context.requested_pcomp = demand.pcomp_negotiation;
        context.network_endpoint_id = Some(snei);
        context.primary_nsapi = primary_nsapi;
        context.packet_data_ms_type = demand.packet_data_ms_type;
        context.qos = qos;
        self.contexts.insert(key, context);
        self.contexts.update_ms_type(issi, demand.packet_data_ms_type);
        if primary_nsapi.is_none() {
            self.contexts.update_secondary_addresses(issi, demand.nsapi, address);
        }

        let request_bits = raw_request.to_bitstr();
        let chap_id = find_chap_response_id(&request_bits);
        let ipcp_response = build_ipcp_dns_response(&request_bits, profile.dns_servers);
        let optional = Self::raw_bits_from_string(&activation_accept_optional_section(
            snei,
            chap_id,
            ipcp_response.as_deref(),
        ));
        let accept = SnPdu::ActivateAccept(ActivateAccept {
            nsapi: demand.nsapi,
            pdu_priority_max: profile.pdu_priority_max,
            ready_timer: profile.ready_timer_code,
            standby_timer: profile.standby_timer_code,
            response_wait_timer: profile.response_wait_timer_code,
            address: accepted_address,
            pcomp_negotiation: 0,
            vj_slots: None,
            rfc2507: None,
            mtu_code: profile.mtu_code,
            optional,
        });
        self.last_activity = format!("PDP {} NSAPI{}", issi, demand.nsapi);
        tracing::info!(
            "SNDCP: PDP accepted ISSI={} NSAPI={} primary={:?} SNEI={} IPv4={} requested_pcomp={:#04x} CHAP={} IPCP_DNS={}",
            issi,
            demand.nsapi,
            primary_nsapi,
            snei,
            Ipv4Addr::from(address),
            demand.pcomp_negotiation,
            chap_id.is_some(),
            ipcp_response.is_some()
        );
        vec![Self::reply(accept, true, false)]
    }

    fn activate_requested_nsapis(
        &mut self,
        issi: u32,
        nsapis: &[u8],
        profile: RuntimeProfile,
    ) -> Result<bool, u8> {
        if nsapis.is_empty() {
            return Err(TX_REJECT_UNKNOWN_NSAPI);
        }
        if nsapis
            .iter()
            .any(|nsapi| self.contexts.get(ContextKey { issi, nsapi: *nsapi }).is_none())
        {
            return Err(TX_REJECT_UNKNOWN_NSAPI);
        }
        if nsapis.iter().any(|nsapi| {
            self.contexts
                .get(ContextKey { issi, nsapi: *nsapi })
                .is_some_and(|context| context.availability != ContextAvailability::Available)
        }) {
            return Err(TX_REJECT_TEMPORARILY_UNAVAILABLE);
        }
        if !self.contexts.can_claim_bearer(issi) {
            return Err(TX_REJECT_SYSTEM_RESOURCES);
        }
        let newly_reserved = self.contexts.bearer_owner().is_none();
        if newly_reserved && !self.reserve_pdch() {
            return Err(TX_REJECT_SYSTEM_RESOURCES);
        }
        if !self.contexts.claim_bearer(issi, nsapis) {
            if newly_reserved {
                self.release_pdch();
            }
            return Err(TX_REJECT_SYSTEM_RESOURCES);
        }
        let now = Instant::now();
        for nsapi in nsapis {
            if let Some(context) = self.contexts.get_mut(ContextKey { issi, nsapi: *nsapi }) {
                context.enter_ready(profile.ready_timer_code, now);
            }
        }
        let _ = self.contexts.refresh_bearer_ready(issi, profile.ready_timer_code, now);
        Ok(newly_reserved)
    }

    fn handle_data_transmit_request(
        &mut self,
        request: DataTransmitRequest,
        ind: &LtpdMleUnitdataInd,
        profile: RuntimeProfile,
    ) -> Vec<CachedReply> {
        let issi = ind.received_tetra_address.ssi;
        self.check_network_endpoint_id(issi, request.network_endpoint_id(), "SN-DATA TRANSMIT REQUEST");
        let primary = request.nsapis.first().copied().unwrap_or(1);
        if let Err(cause) = Self::validate_resource_request(request.resource_request) {
            return vec![Self::transmit_response(primary, false, Some(cause), false, self.contexts.network_endpoint_id(issi))];
        }
        match self.activate_requested_nsapis(issi, &request.nsapis, profile) {
            Ok(_) => {
                self.last_activity = format!("PDCH {} NSAPI{:?}", issi, request.nsapis);
                let nsapis = request.nsapis;
                let mut replies = vec![Self::transmit_response_nsapis(
                    nsapis.clone(), true, None, true, self.contexts.network_endpoint_id(issi),
                )];
                for nsapi in nsapis {
                    let key = ContextKey { issi, nsapi };
                    if let Some(mtu) = self.contexts.get(key).map(|context| context.mtu_octets) {
                        replies.extend(self.pending_replies(key, mtu));
                    }
                }
                replies
            }
            Err(cause) => vec![Self::transmit_response(primary, false, Some(cause), false, self.contexts.network_endpoint_id(issi))],
        }
    }

    fn handle_reconnect(
        &mut self,
        reconnect: Reconnect,
        ind: &LtpdMleUnitdataInd,
        profile: RuntimeProfile,
    ) -> Vec<CachedReply> {
        let issi = ind.received_tetra_address.ssi;
        self.check_network_endpoint_id(issi, reconnect.network_endpoint_id(), "SN-RECONNECT");
        let nsapis = reconnect.nsapi_values();
        if !reconnect.any_data_to_send() {
            let now = Instant::now();
            for nsapi in &nsapis {
                if let Some(context) = self.contexts.get_mut(ContextKey { issi, nsapi: *nsapi }) {
                    context.enter_standby(profile.standby_timer_code, now);
                }
            }
            let released = if nsapis.is_empty() {
                self.contexts.release_bearer_for_issi(issi)
            } else {
                self.contexts.release_bearer_nsapis(issi, &nsapis)
            };
            if released {
                self.release_pdch();
            }
            self.last_activity = format!("Reconnect standby {}", issi);
            return Vec::new();
        }
        let primary = nsapis.first().copied().unwrap_or(1);
        if let Err(cause) = Self::validate_resource_request(reconnect.resource_request) {
            return vec![Self::transmit_response(primary, false, Some(cause), false, self.contexts.network_endpoint_id(issi))];
        }
        match self.activate_requested_nsapis(issi, &nsapis, profile) {
            Ok(_) => {
                let mut replies = vec![Self::transmit_response_nsapis(
                    nsapis.clone(), true, None, true, self.contexts.network_endpoint_id(issi),
                )];
                for nsapi in nsapis {
                    let key = ContextKey { issi, nsapi };
                    if let Some(mtu) = self.contexts.get(key).map(|context| context.mtu_octets) {
                        replies.extend(self.pending_replies(key, mtu));
                    }
                }
                replies
            }
            Err(cause) => vec![Self::transmit_response(primary, false, Some(cause), false, self.contexts.network_endpoint_id(issi))],
        }
    }

    fn user_data_octets(data: &UserData) -> Option<Vec<u8>> {
        if data.n_pdu.bit_len % 8 != 0 {
            return None;
        }
        Some(data.n_pdu.bytes[..data.n_pdu.bit_len / 8].to_vec())
    }

    fn handle_user_data(
        &mut self,
        data: UserData,
        ind: &LtpdMleUnitdataInd,
        profile: RuntimeProfile,
    ) -> Vec<CachedReply> {
        let issi = ind.received_tetra_address.ssi;
        let key = ContextKey { issi, nsapi: data.nsapi };
        let Some(context) = self.contexts.get(key).cloned() else {
            tracing::warn!("SNDCP: user data without context ISSI={} NSAPI={}", issi, data.nsapi);
            return vec![Self::not_supported(if data.acknowledged { protocol::SN_DATA } else { protocol::SN_UNITDATA })];
        };
        if data.pcomp != protocol::PCOMP_NONE || data.dcomp != protocol::DCOMP_NONE {
            tracing::warn!("SNDCP: compressed N-PDU rejected PCOMP={} DCOMP={}", data.pcomp, data.dcomp);
            return Vec::new();
        }
        let Some(raw_npdu) = Self::user_data_octets(&data) else { return Vec::new(); };
        if raw_npdu.len() > context.mtu_octets {
            tracing::warn!("SNDCP: N-PDU {} exceeds negotiated MTU {}", raw_npdu.len(), context.mtu_octets);
            return Vec::new();
        }
        let raw_ip = match parse_ipv4_packet_any(&raw_npdu) {
            Ok(ip) => ip,
            Err(error) => {
                tracing::warn!("SNDCP: invalid IPv4 N-PDU from ISSI {}: {:?}", issi, error);
                return Vec::new();
            }
        };
        if profile.strict_source_address && raw_ip.source != context.address {
            tracing::warn!(
                "SNDCP: source-address mismatch ISSI={} NSAPI={} expected={} actual={}",
                issi,
                data.nsapi,
                Ipv4Addr::from(context.address),
                Ipv4Addr::from(raw_ip.source)
            );
            return Vec::new();
        }
        if context.availability != ContextAvailability::Available || context.usage != ContextUsage::Active {
            return Vec::new();
        }
        if context.state != PdpState::Ready && !profile.assume_ready {
            tracing::warn!("SNDCP: context not READY ISSI={} NSAPI={}", issi, data.nsapi);
            return Vec::new();
        }
        let activity_now = Instant::now();
        if let Some(context) = self.contexts.get_mut(key) {
            context.refresh_ready(profile.ready_timer_code, activity_now);
        }
        let _ = self.contexts.refresh_bearer_ready(issi, profile.ready_timer_code, activity_now);

        let npdu = if raw_ip.is_fragmented() {
            match self.uplink_reassembler.push(&raw_npdu[..raw_ip.total_len], activity_now) {
                Ok(Some(packet)) => packet,
                Ok(None) => return Vec::new(),
                Err(error) => {
                    tracing::warn!("SNDCP: IPv4 reassembly failed ISSI={} error={:?}", issi, error);
                    return Vec::new();
                }
            }
        } else {
            raw_npdu[..raw_ip.total_len].to_vec()
        };
        let ip = match parse_ipv4_packet(&npdu) {
            Ok(ip) => ip,
            Err(error) => {
                tracing::warn!("SNDCP: routed IPv4 N-PDU became invalid ISSI={} error={:?}", issi, error);
                return Vec::new();
            }
        };
        self.learn_automatic_binding(key, &ip);

        let is_wap_request = profile.wap_enabled
            && ip.destination == profile.endpoint
            && ip.protocol == IPV4_PROTOCOL_UDP
            && parse_udp_datagram(ip.payload).is_ok_and(|udp| udp.destination_port == profile.port);
        if is_wap_request {
            let endpoint = WapEndpoint { address: profile.endpoint, port: profile.port, ttl: profile.ttl };
            let policy = WapPolicy {
                accept_empty_probe: profile.accept_empty_probe,
                accept_root_path: profile.accept_root_path,
                accept_status_path: profile.accept_status_path,
                accept_status_wml_path: profile.accept_status_wml_path,
                max_request_payload_bytes: profile.max_request_payload_bytes,
            };
            let snapshot = self.snapshot();
            let response = match build_response_npdu(&npdu, endpoint, policy, &snapshot) {
                Ok(Some(response)) => response,
                Ok(None) => return Vec::new(),
                Err(error) => {
                    tracing::warn!("SNDCP WAP/IP request rejected from ISSI {}: {:?}", issi, error);
                    return Vec::new();
                }
            };
            let fragments = match fragment_ipv4_packet(&response, context.mtu_octets) {
                Ok(fragments) => fragments,
                Err(error) => {
                    tracing::warn!("SNDCP: generated WAP response cannot fit negotiated MTU: {:?}", error);
                    return Vec::new();
                }
            };
            let response_now = Instant::now();
            if let Some(context) = self.contexts.get_mut(key) { context.refresh_ready(profile.ready_timer_code, response_now); }
            let _ = self.contexts.refresh_bearer_ready(issi, profile.ready_timer_code, response_now);
            self.last_activity = format!("WAP {}", issi);
            return fragments.into_iter().map(|fragment| Self::reply(
                SnPdu::Unitdata(UserData {
                    acknowledged: false,
                    nsapi: data.nsapi,
                    pcomp: 0,
                    dcomp: 0,
                    n_pdu: protocol::raw_octets(fragment),
                }),
                false,
                false,
            )).collect();
        }

        if profile.gateway_enabled {
            self.ensure_gateway(profile);
            let Some(gateway) = self.gateway.as_ref() else {
                tracing::warn!("SNDCP: general packet gateway unavailable; dropping N-PDU ISSI={}", issi);
                return Vec::new();
            };
            if let Err(error) = gateway.inject_from_mobile(npdu[..ip.total_len].to_vec()) {
                tracing::warn!("SNDCP: TUN injection failed ISSI={} error={:?}", issi, error);
                return Vec::new();
            }
            self.last_activity = format!("IP uplink {} NSAPI{}", issi, data.nsapi);
            return Vec::new();
        }

        tracing::debug!(
            "SNDCP: IPv4 packet has no enabled local service destination={} protocol={}",
            Ipv4Addr::from(ip.destination),
            ip.protocol
        );
        Vec::new()
    }

    fn handle_deactivate(
        &mut self,
        deactivate: Deactivate,
        ind: &LtpdMleUnitdataInd,
    ) -> Vec<CachedReply> {
        let issi = ind.received_tetra_address.ssi;
        self.check_network_endpoint_id(issi, deactivate.network_endpoint_id(), "SN-DEACTIVATE");
        let nsapi = deactivate.nsapi;
        match (deactivate.deactivation_type, nsapi) {
            (0, _) => {
                self.contexts.remove_all_for_issi(issi);
                self.contexts.release_bearer_for_issi(issi);
                self.pending_downlink.retain(|key, _| key.issi != issi);
                self.page_inflight.retain(|key, _| key.issi != issi);
                self.automatic_bindings.retain(|_, binding| binding.context.issi != issi);
            }
            (1, Some(nsapi)) => {
                let family = self.contexts.family_nsapis(issi, nsapi);
                for family_nsapi in &family {
                    let key = ContextKey { issi, nsapi: *family_nsapi };
                    self.contexts.remove(key);
                    self.pending_downlink.remove(&key);
                    self.page_inflight.remove(&key);
                    self.automatic_bindings.retain(|_, binding| binding.context != key);
                }
                self.contexts.release_bearer_nsapis(issi, &family);
            }
            _ => return vec![Self::not_supported(protocol::SN_DEACTIVATE_PDP_CONTEXT_DEMAND)],
        }
        if self.contexts.bearer_owner().is_none() {
            self.release_pdch();
        }
        if self.contexts.contexts_for_issi(issi) == 0 {
            self.routes.remove(&issi);
            self.response_cache.retain(|(cached_issi, _), _| *cached_issi != issi);
        }
        self.last_activity = format!("PDP off {}", issi);
        vec![Self::reply(
            SnPdu::DeactivateAccept(Deactivate {
                deactivation_type: deactivate.deactivation_type,
                nsapi,
                optional: Self::zero_optional(),
            }),
            true,
            false,
        )]
    }

    fn handle_end_of_data(
        &mut self,
        end: EndOfData,
        ind: &LtpdMleUnitdataInd,
        profile: RuntimeProfile,
    ) -> Vec<CachedReply> {
        let issi = ind.received_tetra_address.ssi;
        if self.contexts.bearer_owner() != Some(issi) {
            tracing::warn!("SNDCP: SN-END OF DATA from ISSI={} without owned bearer", issi);
            return Vec::new();
        }
        let active_nsapis = self.contexts.bearer_nsapis().collect::<Vec<_>>();
        let now = Instant::now();
        for nsapi in &active_nsapis {
            if let Some(context) = self.contexts.get_mut(ContextKey { issi, nsapi: *nsapi }) {
                if end.immediate_service_change {
                    context.suspend(profile.standby_timer_code, now);
                } else {
                    context.enter_standby(profile.standby_timer_code, now);
                }
            }
        }
        if self.contexts.release_bearer_for_issi(issi) {
            self.release_pdch();
        }
        self.last_activity = format!("End data {}", issi);
        if end.immediate_service_change {
            vec![CachedReply {
                sn_pdu: BitBuffer::new(0),
                acknowledged: false,
                include_pdch_allocation: false,
                quit_channel: true,
            }]
        } else {
            vec![CachedReply {
                sn_pdu: Self::encoded(SnPdu::EndOfData(EndOfData {
                    immediate_service_change: false,
                    optional: Self::zero_optional(),
                })),
                acknowledged: true,
                include_pdch_allocation: false,
                quit_channel: true,
            }]
        }
    }

    fn handle_page_response(
        &mut self,
        response: protocol::PageResponse,
        ind: &LtpdMleUnitdataInd,
        profile: RuntimeProfile,
    ) -> Vec<CachedReply> {
        let key = ContextKey { issi: ind.received_tetra_address.ssi, nsapi: response.nsapi };
        self.page_inflight.remove(&key);
        self.check_network_endpoint_id(key.issi, response.network_endpoint_id(), "SN-PAGE RESPONSE");
        if self.contexts.get(key).is_none() {
            self.pending_downlink.remove(&key);
            return vec![Self::not_supported(protocol::SN_PAGE)];
        }
        if !response.pd_service_available {
            if let Some(context) = self.contexts.get_mut(key) {
                context.suspend(profile.standby_timer_code, Instant::now());
            }
            if self.contexts.release_bearer_nsapis(key.issi, &[key.nsapi]) { self.release_pdch(); }
            self.pending_downlink.remove(&key);
            return Vec::new();
        }
        if let Err(cause) = Self::validate_resource_request(response.resource_request) {
            self.page_inflight.insert(key, Instant::now());
            return vec![Self::transmit_response(
                key.nsapi, false, Some(cause), false, self.contexts.network_endpoint_id(key.issi),
            )];
        }
        match self.activate_requested_nsapis(key.issi, &[key.nsapi], profile) {
            Ok(_) => {
                let mut replies = vec![Self::transmit_response(
                    key.nsapi, true, None, true, self.contexts.network_endpoint_id(key.issi),
                )];
                if let Some(mtu) = self.contexts.get(key).map(|context| context.mtu_octets) {
                    replies.extend(self.pending_replies(key, mtu));
                }
                replies
            }
            Err(cause) => {
                self.page_inflight.insert(key, Instant::now());
                vec![Self::transmit_response(
                    key.nsapi, false, Some(cause), false, self.contexts.network_endpoint_id(key.issi),
                )]
            }
        }
    }

    fn handle_data_priority(
        &mut self,
        priority: DataPriority,
        ind: &LtpdMleUnitdataInd,
        profile: RuntimeProfile,
    ) -> Vec<CachedReply> {
        let issi = ind.received_tetra_address.ssi;
        let details = DataPriorityDetails::default_for_network(profile.network_default_priority);
        let DataPriority::Request { request_type } = priority else {
            tracing::debug!("SNDCP: ignoring unexpected downlink data-priority subtype from MS");
            return Vec::new();
        };
        let (accepted, current) = match request_type {
            0..=7 => {
                self.contexts.set_default_priority(issi, request_type);
                (true, request_type)
            }
            8 => {
                self.contexts.track_network_default_priority(issi);
                (true, profile.network_default_priority)
            }
            9 => (true, self.contexts.default_priority(issi, profile.network_default_priority)),
            _ => (false, self.contexts.default_priority(issi, profile.network_default_priority)),
        };
        let mut replies = vec![Self::reply(
            SnPdu::DataPriority(DataPriority::Acknowledgement {
                accepted,
                details,
                ms_default: accepted.then_some(current),
            }),
            true,
            false,
        )];
        if request_type == 9 && accepted {
            replies.push(Self::reply(
                SnPdu::DataPriority(DataPriority::Information { details, ms_default: Some(current) }),
                true,
                false,
            ));
        }
        replies
    }

    fn handle_modify(
        &mut self,
        modify: Modify,
        ind: &LtpdMleUnitdataInd,
        profile: RuntimeProfile,
    ) -> Vec<CachedReply> {
        let issi = ind.received_tetra_address.ssi;
        match modify {
            Modify::Request { nsapi, qos } => {
                let key = ContextKey { issi, nsapi };
                if self.contexts.get(key).is_none() {
                    return vec![Self::reply(
                        SnPdu::Modify(Modify::ResponseRejected {
                            nsapi,
                            cause: MODIFY_REJECT_UNKNOWN_NSAPI,
                            optional: RawBits::empty(),
                        }),
                        true,
                        false,
                    )];
                }
                let decoded_qos = match QosProfile::decode(&qos) {
                    Ok(qos) => qos,
                    Err(error) => {
                        tracing::warn!("SNDCP: malformed MODIFY QoS ISSI={} NSAPI={}: {:?}", issi, nsapi, error);
                        return vec![Self::reply(
                            SnPdu::Modify(Modify::ResponseRejected {
                                nsapi,
                                cause: MODIFY_REJECT_QOS_NOT_AVAILABLE,
                                optional: RawBits::empty(),
                            }),
                            true,
                            false,
                        )];
                    }
                };
                let primary_context = self.contexts.get(key).is_some_and(|context| context.primary_nsapi.is_none());
                if let Err(cause) = Self::validate_qos(&decoded_qos, primary_context) {
                    return vec![Self::reply(
                        SnPdu::Modify(Modify::ResponseRejected {
                            nsapi,
                            cause: Self::modify_qos_cause(cause),
                            optional: RawBits::empty(),
                        }),
                        true,
                        false,
                    )];
                }
                self.automatic_bindings.retain(|_, binding| binding.context != key);
                if let Some(context) = self.contexts.get_mut(key) {
                    context.qos = decoded_qos;
                    if context.state == PdpState::Ready {
                        context.refresh_ready(profile.ready_timer_code, Instant::now());
                    }
                }
                vec![Self::reply(
                    SnPdu::Modify(Modify::ResponseApplied {
                        nsapi,
                        pdu_priority_max: profile.pdu_priority_max,
                        qos,
                    }),
                    true,
                    false,
                )]
            }
            Modify::Availability { nsapi, availability, .. } => {
                let requested = ContextAvailability::from_code(availability);
                if matches!(requested, ContextAvailability::Reserved(_)) {
                    return vec![Self::not_supported(protocol::SN_MODIFY)];
                }
                let key = ContextKey { issi, nsapi };
                let should_release = {
                    let Some(context) = self.contexts.get_mut(key) else {
                        return vec![Self::not_supported(protocol::SN_MODIFY)];
                    };
                    let now = Instant::now();
                    match requested {
                        ContextAvailability::Available => {
                            if context.state == PdpState::Suspended {
                                context.enter_standby(profile.standby_timer_code, now);
                            }
                            context.availability = ContextAvailability::Available;
                            false
                        }
                        ContextAvailability::ScheduleSuspended => {
                            context.suspend(profile.standby_timer_code, now);
                            context.availability = ContextAvailability::ScheduleSuspended;
                            true
                        }
                        ContextAvailability::Reserved(_) => unreachable!(),
                    }
                };
                if should_release {
                    self.automatic_bindings.retain(|_, binding| binding.context != key);
                }
                if should_release && self.contexts.release_bearer_nsapis(issi, &[nsapi]) {
                    self.release_pdch();
                }
                Vec::new()
            }
            Modify::Usage { nsapi, usage, .. } => {
                let requested = ContextUsage::from_code(usage);
                if matches!(requested, ContextUsage::Reserved(_)) {
                    return vec![Self::not_supported(protocol::SN_MODIFY)];
                }
                let key = ContextKey { issi, nsapi };
                {
                    let Some(context) = self.contexts.get_mut(key) else {
                        return vec![Self::not_supported(protocol::SN_MODIFY)];
                    };
                    context.enter_standby(profile.standby_timer_code, Instant::now());
                    context.usage = requested;
                }
                if requested != ContextUsage::Active {
                    self.automatic_bindings.retain(|_, binding| binding.context != key);
                }
                if self.contexts.release_bearer_nsapis(issi, &[nsapi]) {
                    self.release_pdch();
                }
                Vec::new()
            }
            Modify::ResponseApplied { .. } | Modify::ResponseRejected { .. } | Modify::Reserved { .. } => Vec::new(),
        }
    }

    fn dispatch(
        &mut self,
        pdu: SnPdu,
        raw_request: &BitBuffer,
        ind: &LtpdMleUnitdataInd,
        profile: RuntimeProfile,
    ) -> Vec<CachedReply> {
        match pdu {
            SnPdu::ActivateDemand(demand) => self.handle_activate(demand, raw_request, ind, profile),
            SnPdu::DeactivateDemand(deactivate) => self.handle_deactivate(deactivate, ind),
            SnPdu::Unitdata(data) | SnPdu::Data(data) => self.handle_user_data(data, ind, profile),
            SnPdu::DataTransmitRequest(request) => self.handle_data_transmit_request(request, ind, profile),
            SnPdu::EndOfData(end) => self.handle_end_of_data(end, ind, profile),
            SnPdu::Reconnect(reconnect) => self.handle_reconnect(reconnect, ind, profile),
            SnPdu::PageResponse(response) => self.handle_page_response(response, ind, profile),
            SnPdu::NotSupported { pdu_type } => {
                tracing::warn!("SNDCP: MS reports unsupported downlink SN-PDU type {}", pdu_type);
                Vec::new()
            }
            SnPdu::DataPriority(priority) => self.handle_data_priority(priority, ind, profile),
            SnPdu::Modify(modify) => self.handle_modify(modify, ind, profile),
            SnPdu::DeactivateAccept(_)
            | SnPdu::ActivateAccept(_)
            | SnPdu::ActivateReject(_)
            | SnPdu::DataTransmitResponse(_)
            | SnPdu::PageRequest(_) => vec![Self::not_supported(raw_request.peek_bits(4).unwrap_or(15) as u8)],
            SnPdu::Reserved { pdu_type, .. } => vec![Self::not_supported(pdu_type)],
        }
    }

    fn snapshot(&self) -> WapStatusSnapshot {
        let state = self.config.state_read();
        let active_calls = state.active_call_ts.values().copied().collect::<HashSet<_>>().len();
        let gateway = self.gateway.as_ref().map(PacketGateway::stats);
        let queued_packets = self.pending_downlink.values().map(|queue| queue.len()).sum::<usize>();
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
            health: format!(
                "OK PDP={} PDCH={} GW={} UL={}/{} DL={}/{} Q={}",
                self.contexts.len(),
                self.contexts.bearer_owner().map(|issi| issi.to_string()).unwrap_or_else(|| "free".to_string()),
                gateway.map(|stats| if stats.running { "UP" } else { "DOWN" }).unwrap_or("OFF"),
                gateway.map(|stats| stats.packets_from_mobile).unwrap_or(0),
                gateway.map(|stats| stats.bytes_from_mobile).unwrap_or(0),
                gateway.map(|stats| stats.packets_to_mobile).unwrap_or(0),
                gateway.map(|stats| stats.bytes_to_mobile).unwrap_or(0),
                queued_packets,
            ),
        }
    }

    fn cleanup_subscriber(&mut self, issi: u32, reason: &str) {
        let owned_bearer = self.contexts.bearer_owner() == Some(issi);
        self.contexts.remove_all_for_issi(issi);
        self.routes.remove(&issi);
        self.response_cache.retain(|(cached_issi, _), _| *cached_issi != issi);
        self.pending_downlink.retain(|key, _| key.issi != issi);
        self.page_inflight.retain(|key, _| key.issi != issi);
        self.automatic_bindings.retain(|_, binding| binding.context.issi != issi);
        if owned_bearer {
            self.release_pdch();
        }
        self.last_activity = format!("PDP cleanup {} ({})", issi, reason);
        tracing::info!("SNDCP: subscriber cleanup ISSI={} reason={}", issi, reason);
    }

    fn sweep_timers(&mut self, queue: &mut MessageQueue, profile: RuntimeProfile) {
        let now = Instant::now();
        if now.duration_since(self.last_timer_sweep) < Duration::from_millis(250) {
            return;
        }
        self.last_timer_sweep = now;
        let had_bearer = self.contexts.bearer_owner().is_some();
        let events = self.contexts.tick(now, profile.standby_timer_code);
        let mut end_of_data_issis = HashSet::new();
        let mut expired_issis = HashSet::new();
        for event in events {
            match event {
                TimerEvent::ReadyExpired(issi) => {
                    tracing::info!("SNDCP: global READY timer expired ISSI={}", issi);
                    end_of_data_issis.insert(issi);
                }
                TimerEvent::ContextReadyExpired(key) => {
                    tracing::debug!(
                        "SNDCP: CONTEXT_READY timer expired ISSI={} NSAPI={} (bearer remains active)",
                        key.issi,
                        key.nsapi
                    );
                }
                TimerEvent::StandbyExpired(key) => {
                    tracing::info!("SNDCP: STANDBY timer expired ISSI={} NSAPI={}", key.issi, key.nsapi);
                    expired_issis.insert(key.issi);
                }
            }
        }

        for issi in end_of_data_issis {
            let Some(route) = self.routes.get(&issi).copied() else {
                tracing::warn!("SNDCP: cannot send timer SN-END OF DATA; no route for ISSI={}", issi);
                continue;
            };
            let pdu = Self::encoded(SnPdu::EndOfData(EndOfData {
                immediate_service_change: false,
                optional: Self::zero_optional(),
            }));
            self.queue_acked_to(queue, route, &pdu, Some(Self::quit_allocation()));
            self.last_activity = format!("READY timeout {}", issi);
        }

        for issi in expired_issis {
            self.routes.remove(&issi);
            self.response_cache.retain(|(cached_issi, _), _| *cached_issi != issi);
            self.pending_downlink.retain(|key, _| key.issi != issi);
            self.page_inflight.retain(|key, _| key.issi != issi);
            self.automatic_bindings.retain(|_, binding| binding.context.issi != issi);
        }
        if had_bearer && self.contexts.bearer_owner().is_none() {
            self.release_pdch();
        }
        self.response_cache.retain(|_, exchange| exchange.expires_at > now);
        self.automatic_bindings.retain(|_, binding| binding.expires_at > now);
        self.uplink_reassembler.sweep(now);
        self.downlink_reassembler.sweep(now);
    }

}


fn data_transmit_response_optional_section(snei: Option<u16>, additional_nsapis: &[u8]) -> String {
    let mut seen = HashSet::new();
    let additional = additional_nsapis
        .iter()
        .copied()
        .filter(|nsapi| (1..=14).contains(nsapi) && seen.insert(*nsapi))
        .take(63)
        .collect::<Vec<_>>();
    if snei.is_none() && additional.is_empty() {
        return "0".to_string();
    }

    let mut s = String::new();
    s.push('1'); // O-bit
    if let Some(snei) = snei {
        s.push('1'); // SNEI Type-2 present
        s.push_str(&format!("{snei:016b}"));
    } else {
        s.push('0');
    }
    if additional.is_empty() {
        s.push('0'); // no Type-3/4 element
        return s;
    }

    s.push('1'); // Type-4 element follows
    s.push_str("0100"); // additional NSAPI information
    let length = 6 + additional.len() * 6;
    s.push_str(&format!("{length:011b}"));
    s.push_str(&format!("{:06b}", additional.len()));
    for nsapi in additional {
        s.push_str(&format!("{nsapi:04b}"));
        s.push_str("00"); // reserved
    }
    s.push('0'); // no further Type-3/4 element
    s
}

fn pco_ppp_element(protocol: u16, payload: &[u8]) -> String {
    let mut contents = String::new();
    contents.push_str(&format!("{PPP_CONFIG_PROTOCOL_PPP:04b}"));
    contents.push_str(&format!("{protocol:016b}"));
    contents.push_str(&format!("{:08b}", payload.len().min(255)));
    for byte in payload.iter().copied().take(255) {
        contents.push_str(&format!("{byte:08b}"));
    }
    let mut element = String::new();
    element.push_str(&format!("{PCO_TYPE34_ID:04b}"));
    element.push_str(&format!("{:011b}", contents.len()));
    element.push_str(&contents);
    element
}

fn activation_accept_optional_section(snei: u16, chap_id: Option<u8>, ipcp_response: Option<&[u8]>) -> String {
    let mut s = String::new();
    s.push('1'); // O-bit
    s.push('1'); // SNEI Type-2 element present
    s.push_str(&format!("{snei:016b}"));
    s.push('0'); // SwMI IPv6 information absent
    s.push('0'); // SwMI Mobile IPv4 information absent
    let mut pco = Vec::new();
    if let Some(chap_id) = chap_id {
        pco.push(pco_ppp_element(PPP_PROTO_CHAP, &[CHAP_CODE_SUCCESS, chap_id, 0, 4]));
    }
    if let Some(ipcp) = ipcp_response {
        pco.push(pco_ppp_element(PPP_PROTO_IPCP, ipcp));
    }
    for element in pco {
        s.push('1'); // another Type-3/4 element follows
        s.push_str(&element);
    }
    s.push('0');
    s
}

fn bits_to_u8(bits: &str, offset: usize) -> Option<u8> {
    bits.get(offset..offset + 8).and_then(|value| u8::from_str_radix(value, 2).ok())
}

fn find_ppp_payload(demand: &str, protocol: u16) -> Option<Vec<u8>> {
    let marker = format!("{protocol:016b}");
    let mut from = 0usize;
    while let Some(relative) = demand.get(from..)?.find(&marker) {
        let start = from + relative + 16;
        let payload_len = usize::from(bits_to_u8(demand, start)?);
        let payload_start = start + 8;
        let payload_end = payload_start.checked_add(payload_len.checked_mul(8)?)?;
        if payload_end <= demand.len() {
            let mut payload = Vec::with_capacity(payload_len);
            for offset in (payload_start..payload_end).step_by(8) {
                payload.push(bits_to_u8(demand, offset)?);
            }
            return Some(payload);
        }
        from = start;
    }
    None
}

fn build_ipcp_dns_response(demand: &str, configured: [Option<[u8; 4]>; 2]) -> Option<Vec<u8>> {
    let request = find_ppp_payload(demand, PPP_PROTO_IPCP)?;
    if request.len() < 4 || request[0] != IPCP_CODE_CONFIGURE_REQUEST {
        return None;
    }
    let declared_len = usize::from(u16::from_be_bytes([request[2], request[3]]));
    if declared_len < 4 || declared_len > request.len() {
        return None;
    }
    let mut requested = [None, None];
    let mut offset = 4usize;
    while offset + 2 <= declared_len {
        let option = request[offset];
        let length = usize::from(request[offset + 1]);
        if length < 2 || offset + length > declared_len {
            return None;
        }
        if length == 6 && matches!(option, IPCP_PRIMARY_DNS_OPTION | IPCP_SECONDARY_DNS_OPTION) {
            let address = [request[offset + 2], request[offset + 3], request[offset + 4], request[offset + 5]];
            requested[usize::from(option == IPCP_SECONDARY_DNS_OPTION)] = Some(address);
        }
        offset += length;
    }
    if configured.iter().all(Option::is_none) || requested.iter().all(Option::is_none) {
        return None;
    }
    let matches = configured.iter().zip(requested).all(|(configured, requested)| requested.is_none() || *configured == requested);
    let mut response = vec![if matches { IPCP_CODE_CONFIGURE_ACK } else { IPCP_CODE_CONFIGURE_NAK }, request[1], 0, 0];
    if matches {
        response.extend_from_slice(&request[4..declared_len]);
    } else {
        for (index, address) in configured.into_iter().enumerate() {
            if let Some(address) = address {
                response.push(if index == 0 { IPCP_PRIMARY_DNS_OPTION } else { IPCP_SECONDARY_DNS_OPTION });
                response.push(6);
                response.extend_from_slice(&address);
            }
        }
    }
    let length = response.len() as u16;
    response[2..4].copy_from_slice(&length.to_be_bytes());
    Some(response)
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
        if let SapMsgInner::MmSubscriberUpdate(update) = &message.msg {
            if update.action == BrewSubscriberAction::Deregister {
                self.cleanup_subscriber(update.issi, "MM deregistration");
            }
            return;
        }
        let SapMsgInner::LtpdMleUnitdataInd(ind) = &message.msg else {
            tracing::debug!("SNDCP: unhandled primitive {:?}", message.msg);
            return;
        };
        if !self.profile_enabled() {
            tracing::debug!("SNDCP: profile disabled; PDU ignored");
            return;
        }
        let Some(profile) = self.profile() else {
            tracing::error!("SNDCP: invalid runtime profile despite validated configuration");
            return;
        };
        self.routes.insert(
            ind.received_tetra_address.ssi,
            SubscriberRoute {
                main_address: ind.received_tetra_address,
                link_id: ind.link_id,
                endpoint_id: ind.endpoint_id,
            },
        );
        let pdu = Self::rebase_sndcp_pdu(&ind.sdu);
        let fingerprint = pdu.to_bitstr();
        if self.replay_cached(queue, ind, &fingerprint) {
            return;
        }
        let sn_type = pdu.peek_bits(4).unwrap_or(15) as u8;
        tracing::info!(
            "SNDCP: <- type={} ISSI={} bits={}",
            sn_type,
            ind.received_tetra_address.ssi,
            pdu.get_len()
        );
        let decoded = match protocol::decode(&pdu, SnDirection::Uplink) {
            Ok(decoded) => decoded,
            Err(error) => {
                tracing::warn!("SNDCP: malformed type={} from ISSI={}: {:?}", sn_type, ind.received_tetra_address.ssi, error);
                // SN-NOT SUPPORTED denotes an unsupported function/PDU, not a
                // malformed encoding. Silently discard malformed frames after logging.
                return;
            }
        };
        let replies = self.dispatch(decoded, &pdu, ind, profile);
        if replies.is_empty() {
            return;
        }
        self.cache_and_emit(queue, ind, &fingerprint, replies);
    }

    fn tick_start(&mut self, queue: &mut MessageQueue, _ts: TdmaTime) {
        if let Some(profile) = self.profile() {
            if profile.gateway_enabled {
                self.poll_gateway(queue, profile);
            } else {
                self.gateway = None;
                self.gateway_config = None;
            }
            self.sweep_timers(queue, profile);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex_to_bits(hex: &str) -> String {
        hex.chars()
            .filter(|c| !c.is_whitespace())
            .map(|c| format!("{:04b}", c.to_digit(16).unwrap()))
            .collect()
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
    fn optional_section_layout_matches_motorola_profile() {
        let sec = activation_accept_optional_section(0x1234, Some(5), None);
        assert_eq!(sec.len(), 97);
        assert_eq!(&sec[0..2], "11");
        assert_eq!(&sec[2..18], "0001001000110100");
        assert_eq!(&sec[18..21], "001");
        assert_eq!(&sec[21..25], "0001");
        assert_eq!(&sec[40..56], "1100001000100011");
        assert_eq!(&sec[64..72], "00000011");
        assert_eq!(&sec[72..80], "00000101");
        assert_eq!(&sec[96..97], "0");
    }

    #[test]
    fn ipcp_dns_request_receives_configure_nak() {
        let payload = [1u8, 7, 0, 10, IPCP_PRIMARY_DNS_OPTION, 6, 0, 0, 0, 0];
        let mut bits = format!("{PPP_PROTO_IPCP:016b}{:08b}", payload.len());
        for byte in payload { bits.push_str(&format!("{byte:08b}")); }
        let response = build_ipcp_dns_response(&bits, [Some([1, 1, 1, 1]), Some([9, 9, 9, 9])]).unwrap();
        assert_eq!(response, vec![3, 7, 0, 16, 129, 6, 1, 1, 1, 1, 131, 6, 9, 9, 9, 9]);
    }

    #[test]
    fn rebases_both_mle_cursor_forms() {
        let mut complete = BitBuffer::new(11);
        complete.write_bits(MLE_DISCRIMINATOR_SNDCP, 3);
        complete.write_bits(0x42, 8);
        complete.seek(0);
        let from_start = Sndcp::rebase_sndcp_pdu(&complete);
        assert_eq!(from_start.get_len(), 8);
        assert_eq!(from_start.peek_bits(8), Some(0x42));
        let mut already_routed = BitBuffer::from_bitbuffer(&complete);
        assert_eq!(already_routed.read_bits(3), Some(MLE_DISCRIMINATOR_SNDCP));
        let from_cursor = Sndcp::rebase_sndcp_pdu(&already_routed);
        assert_eq!(from_cursor.peek_bits(8), Some(0x42));
    }

    #[test]
    fn qos_port_filter_matches_downlink_mobile_port() {
        let packet = crate::sndcp::ip::build_ipv4_udp_npdu(
            [198, 51, 100, 7], [10, 0, 0, 2], 443, 4711, 1, 32, b"x",
        ).unwrap();
        let ip = parse_ipv4_packet(&packet).unwrap();
        let filter = super::super::qos::QosFilter { operation: 0, filter_type: 2, first: Some(4711), second: None };
        assert!(Sndcp::static_filter_matches(&ip, filter));
    }

    #[test]
    fn automatic_flow_key_is_symmetric_across_direction() {
        let uplink = crate::sndcp::ip::build_ipv4_udp_npdu(
            [10, 0, 0, 2], [198, 51, 100, 7], 4711, 443, 1, 32, b"x",
        ).unwrap();
        let downlink = crate::sndcp::ip::build_ipv4_udp_npdu(
            [198, 51, 100, 7], [10, 0, 0, 2], 443, 4711, 2, 32, b"y",
        ).unwrap();
        assert_eq!(
            Sndcp::automatic_flow_key_uplink(&parse_ipv4_packet(&uplink).unwrap()),
            Sndcp::automatic_flow_key_downlink(&parse_ipv4_packet(&downlink).unwrap()),
        );
    }

    #[test]
    fn activate_accept_reference_vector_stays_stable() {
        let pdu = protocol::encode(&SnPdu::ActivateAccept(ActivateAccept {
            nsapi: 2,
            pdu_priority_max: 4,
            ready_timer: 8,
            standby_timer: 4,
            response_wait_timer: 7,
            address: ActivateAddressAccept::Ipv4Dynamic([10, 0, 0, 226]),
            pcomp_negotiation: 0,
            vj_slots: None,
            rfc2507: None,
            mtu_code: 2,
            optional: Sndcp::zero_optional(),
        }));
        assert_eq!(pdu.get_len(), 70);
        assert_eq!(pdu.into_bytes(), vec![0x02, 0x90, 0x8e, 0x82, 0x80, 0x00, 0x38, 0x80, 0x10]);
    }
}

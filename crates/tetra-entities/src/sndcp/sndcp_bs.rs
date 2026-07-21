use std::collections::{HashMap, HashSet};
use std::time::Instant;

use crate::{MessageQueue, TetraEntityTrait};
use tetra_config::bluestation::{CfgWapIp, SharedConfig};
use tetra_core::tetra_entities::TetraEntity;
use tetra_core::{BitBuffer, Sap};
use tetra_saps::lcmc::enums::{alloc_type::ChanAllocType, ul_dl_assignment::UlDlAssignment};
use tetra_saps::lcmc::fields::chan_alloc_req::CmceChanAllocReq;
use tetra_saps::ltpd::LtpdMleUnitdataInd;
use tetra_saps::tla::{TlaTlDataReqBl, TlaTlUnitdataReqBl};
use tetra_saps::{SapMsg, SapMsgInner};

use super::wap::{WapEndpoint, WapStatusSnapshot, build_response};

const MLE_DISCRIMINATOR_SNDCP: u64 = 0b100;
const SN_ACTIVATE_PDP: u8 = 0;
const SN_DEACTIVATE_ACCEPT: u8 = 1;
const SN_DEACTIVATE_DEMAND: u8 = 2;
const SN_ACTIVATE_REJECT: u8 = 3;
const SN_UNITDATA: u8 = 4;
const SN_DATA: u8 = 5;
const SN_DATA_TRANSMIT_REQUEST: u8 = 6;
const SN_DATA_TRANSMIT_RESPONSE: u8 = 7;
const SN_END_OF_DATA: u8 = 8;
const SN_RECONNECT: u8 = 9;

const PDU_PRIORITY_MAX: u64 = 4;
const READY_TIMER: u64 = 8;
const STANDBY_TIMER: u64 = 4;
const RESPONSE_WAIT_TIMER: u64 = 7;
const TIA_IPV4_STATIC: u64 = 1;
const TIA_IPV4_DYNAMIC: u64 = 2;
const MTU_576: u64 = 2;

const PCO_TYPE34_ID: u64 = 1;
const PPP_PROTO_CHAP: u64 = 0xC223;
const PPP_CONFIG_PROTOCOL_PPP: u64 = 0;
const CHAP_CODE_SUCCESS: u64 = 3;
const PCO_CHAP_SUCCESS_BITS: u64 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PacketState {
    Standby,
    Ready,
}

#[derive(Debug, Clone, Copy)]
struct PdpContext {
    address: [u8; 4],
    state: PacketState,
}

#[derive(Debug, Clone, Copy)]
struct ActivateDemand {
    nsapi: u8,
    static_address: Option<[u8; 4]>,
}

pub struct Sndcp {
    config: SharedConfig,
    wap: CfgWapIp,
    contexts: HashMap<(u32, u8), PdpContext>,
    leases: HashMap<u32, [u8; 4]>,
    started: Instant,
}

impl Sndcp {
    pub fn new(config: SharedConfig) -> Self {
        let wap = config.config().cell.wap_ip.clone();
        Self {
            config,
            wap,
            contexts: HashMap::new(),
            leases: HashMap::new(),
            started: Instant::now(),
        }
    }

    fn handle_indication(&mut self, queue: &mut MessageQueue, ind: &LtpdMleUnitdataInd) {
        if !self.config.config().cell.sndcp_service {
            tracing::debug!(
                "SNDCP: service disabled; ignoring packet-data PDU from {}",
                ind.received_tetra_address
            );
            return;
        }

        let Some(mut pdu) = rebase_sn_pdu(&ind.sdu) else {
            tracing::warn!("SNDCP: packet is too short for the MLE discriminator");
            return;
        };
        let Some(sn_type) = pdu.peek_bits(4).map(|value| value as u8) else {
            tracing::warn!("SNDCP: empty SN-PDU from {}", ind.received_tetra_address);
            return;
        };
        let issi = ind.received_tetra_address.ssi;
        tracing::info!(
            "SNDCP/WAP: <- {} type={} bits={} state={:?}",
            ind.received_tetra_address,
            sn_type,
            pdu.get_len(),
            self.contexts
                .iter()
                .find_map(|(&(context_issi, _), context)| (context_issi == issi).then_some(context.state))
        );

        let result = match sn_type {
            SN_ACTIVATE_PDP => self.handle_activate(queue, ind, &pdu),
            SN_DEACTIVATE_DEMAND => self.handle_deactivate(queue, ind, &pdu),
            SN_DATA_TRANSMIT_REQUEST => self.handle_data_transmit(queue, ind, &pdu),
            SN_END_OF_DATA => self.handle_end_of_data(queue, ind, &pdu),
            SN_RECONNECT => self.handle_reconnect(queue, ind, &pdu),
            SN_UNITDATA | SN_DATA => self.handle_user_data(queue, ind, &mut pdu),
            _ => {
                tracing::warn!(
                    "SNDCP/WAP: unsupported inbound SN-PDU type {} from {}",
                    sn_type,
                    ind.received_tetra_address
                );
                Ok(())
            }
        };
        if let Err(error) = result {
            tracing::warn!("SNDCP/WAP: request from {} rejected: {}", ind.received_tetra_address, error);
        }
    }

    fn handle_activate(&mut self, queue: &mut MessageQueue, ind: &LtpdMleUnitdataInd, pdu: &BitBuffer) -> Result<(), String> {
        let nsapi_hint = pdu.peek_bits_startoffset(8, 4).map(|value| value as u8);
        let demand = match decode_activate_demand(pdu) {
            Ok(demand) => demand,
            Err((cause, detail)) => {
                if let Some(nsapi) = nsapi_hint.filter(|nsapi| (1..=14).contains(nsapi)) {
                    self.send_control(queue, ind, encode_activate_reject(nsapi, cause), None);
                }
                return Err(detail);
            }
        };

        let issi = ind.received_tetra_address.ssi;
        let key = (issi, demand.nsapi);
        if !self.contexts.contains_key(&key) && self.contexts.keys().filter(|(context_issi, _)| *context_issi == issi).count() >= 4 {
            self.send_control(queue, ind, encode_activate_reject(demand.nsapi, 19), None);
            return Err("maximum of four PDP contexts per ISSI exceeded".into());
        }

        let (address, tia) = match demand.static_address {
            Some(address) => {
                if !self.wap.allow_static_ipv4 {
                    self.send_control(queue, ind, encode_activate_reject(demand.nsapi, 10), None);
                    return Err("static IPv4 PDP contexts are disabled".into());
                }
                if address == self.wap.address || matches!(address[3], 0 | 255) {
                    self.send_control(queue, ind, encode_activate_reject(demand.nsapi, 8), None);
                    return Err(format!("invalid static IPv4 address {address:?}"));
                }
                let in_use = self.contexts.iter().any(|(&(context_issi, context_nsapi), context)| {
                    (context_issi, context_nsapi) != (ind.received_tetra_address.ssi, demand.nsapi) && context.address == address
                });
                if in_use {
                    self.send_control(queue, ind, encode_activate_reject(demand.nsapi, 9), None);
                    return Err(format!("static IPv4 address {address:?} is already in use"));
                }
                (address, TIA_IPV4_STATIC)
            }
            None => match self.allocate_dynamic_address(ind.received_tetra_address.ssi) {
                Ok(address) => (address, TIA_IPV4_DYNAMIC),
                Err(error) => {
                    self.send_control(queue, ind, encode_activate_reject(demand.nsapi, 7), None);
                    return Err(error);
                }
            },
        };
        self.contexts.insert(
            (ind.received_tetra_address.ssi, demand.nsapi),
            PdpContext {
                address,
                state: PacketState::Standby,
            },
        );

        let chap_id = find_chap_response_id(&pdu.to_bitstr());
        let accept = encode_activate_accept(demand.nsapi, tia, address, chap_id);
        self.send_control(queue, ind, accept, None);
        tracing::info!(
            "SNDCP/WAP: PDP context accepted ISSI={} NSAPI={} IPv4={}.{}.{}.{} CHAP={:?}",
            ind.received_tetra_address.ssi,
            demand.nsapi,
            address[0],
            address[1],
            address[2],
            address[3],
            chap_id
        );
        Ok(())
    }

    fn handle_deactivate(&mut self, queue: &mut MessageQueue, ind: &LtpdMleUnitdataInd, pdu: &BitBuffer) -> Result<(), String> {
        let mut reader = reader(pdu);
        expect(&mut reader, 4, SN_DEACTIVATE_DEMAND as u64, "deactivate type")?;
        let selector = read(&mut reader, 8, "deactivation selector")? as u8;
        let nsapi = if selector == 1 {
            Some(read(&mut reader, 4, "deactivation NSAPI")? as u8)
        } else {
            None
        };
        let issi = ind.received_tetra_address.ssi;
        if let Some(nsapi) = nsapi {
            self.contexts.remove(&(issi, nsapi));
        } else {
            self.contexts.retain(|(context_issi, _), _| *context_issi != issi);
        }
        if !self.contexts.keys().any(|(context_issi, _)| *context_issi == issi) {
            self.leases.remove(&issi);
        }
        let mut response = BitBuffer::new_autoexpand(24);
        response.write_bits(SN_DEACTIVATE_ACCEPT as u64, 4);
        response.write_bits(selector as u64, 8);
        if let Some(nsapi) = nsapi {
            response.write_bits(nsapi as u64, 4);
        }
        response.write_bits(0, 1);
        response.seek(0);
        self.send_control(queue, ind, response, Some(return_to_control_channel()));
        Ok(())
    }

    fn handle_data_transmit(&mut self, queue: &mut MessageQueue, ind: &LtpdMleUnitdataInd, pdu: &BitBuffer) -> Result<(), String> {
        let mut reader = reader(pdu);
        expect(&mut reader, 4, SN_DATA_TRANSMIT_REQUEST as u64, "data transmit type")?;
        let nsapi = read(&mut reader, 4, "NSAPI")? as u8;
        let key = (ind.received_tetra_address.ssi, nsapi);
        let Some(context) = self.contexts.get_mut(&key) else {
            let reject = encode_data_transmit_response(nsapi, false, Some(1));
            self.send_control(queue, ind, reject, None);
            return Err(format!("no PDP context for NSAPI {nsapi}"));
        };
        context.state = PacketState::Ready;
        let response = encode_data_transmit_response(nsapi, true, None);
        self.send_control(queue, ind, response, Some(packet_data_channel()));
        tracing::info!(
            "SNDCP/WAP: ISSI={} NSAPI={} entered READY on TS2",
            ind.received_tetra_address.ssi,
            nsapi
        );
        Ok(())
    }

    fn handle_reconnect(&mut self, queue: &mut MessageQueue, ind: &LtpdMleUnitdataInd, pdu: &BitBuffer) -> Result<(), String> {
        let mut reader = reader(pdu);
        expect(&mut reader, 4, SN_RECONNECT as u64, "reconnect type")?;
        let has_data = read(&mut reader, 1, "data to send")? != 0;
        if !has_data {
            return Ok(());
        }
        let nsapi = read(&mut reader, 4, "NSAPI")? as u8;
        let key = (ind.received_tetra_address.ssi, nsapi);
        if let Some(context) = self.contexts.get_mut(&key) {
            context.state = PacketState::Ready;
            self.send_control(
                queue,
                ind,
                encode_data_transmit_response(nsapi, true, None),
                Some(packet_data_channel()),
            );
            Ok(())
        } else {
            Err(format!("reconnect without PDP context for NSAPI {nsapi}"))
        }
    }

    fn handle_end_of_data(&mut self, queue: &mut MessageQueue, ind: &LtpdMleUnitdataInd, pdu: &BitBuffer) -> Result<(), String> {
        let mut reader = reader(pdu);
        expect(&mut reader, 4, SN_END_OF_DATA as u64, "end-of-data type")?;
        let immediate = read(&mut reader, 1, "immediate service change")?;
        for ((issi, _), context) in &mut self.contexts {
            if *issi == ind.received_tetra_address.ssi {
                context.state = PacketState::Standby;
            }
        }
        if immediate == 0 {
            let mut response = BitBuffer::new(6);
            response.write_bits(SN_END_OF_DATA as u64, 4);
            response.write_bits(0, 1);
            response.write_bits(0, 1);
            response.seek(0);
            self.send_control(queue, ind, response, Some(return_to_control_channel()));
        }
        Ok(())
    }

    fn handle_user_data(&mut self, queue: &mut MessageQueue, ind: &LtpdMleUnitdataInd, pdu: &mut BitBuffer) -> Result<(), String> {
        if !self.wap.enabled {
            return Err("WAP/IP endpoint is disabled".into());
        }
        let _kind = read(pdu, 4, "SN user-data type")? as u8;
        let nsapi = read(pdu, 4, "NSAPI")? as u8;
        let pcomp = read(pdu, 4, "PCOMP")? as u8;
        let dcomp = read(pdu, 4, "DCOMP")? as u8;
        if pcomp != 0 || dcomp != 0 {
            return Err(format!("compression is unsupported (PCOMP={pcomp}, DCOMP={dcomp})"));
        }
        if pdu.get_len_remaining() == 0 || pdu.get_len_remaining() % 8 != 0 {
            return Err(format!("N-PDU is not octet aligned ({} bits)", pdu.get_len_remaining()));
        }
        let npdu_bits = pdu.get_len_remaining();
        let mut npdu = vec![0u8; npdu_bits / 8];
        pdu.read_bits_into_slice(npdu_bits, &mut npdu)
            .ok_or_else(|| "truncated N-PDU".to_string())?;

        let key = (ind.received_tetra_address.ssi, nsapi);
        let context = self
            .contexts
            .get(&key)
            .copied()
            .ok_or_else(|| format!("missing PDP context for NSAPI {nsapi}"))?;
        let request_source = npdu.get(12..16).ok_or_else(|| "IPv4 N-PDU too short".to_string())?;
        if request_source != &context.address[..] {
            return Err(format!(
                "source IPv4 mismatch: context={:?} packet={:?}",
                context.address, request_source
            ));
        }

        let endpoint = WapEndpoint {
            address: self.wap.address,
            port: self.wap.port,
            ttl: self.wap.response_ttl,
            max_request_bytes: self.wap.max_request_payload_bytes,
        };
        let snapshot = self.status_snapshot();
        let response = build_response(&npdu, endpoint, &snapshot).map_err(|error| format!("WAP/IP: {error:?}"))?;
        let Some(response) = response else {
            tracing::debug!("SNDCP/WAP: WTP control PDU requires no response");
            return Ok(());
        };
        if context.state != PacketState::Ready {
            return Err(format!("PDP context NSAPI {nsapi} is not READY"));
        }
        if response.len() > 576 {
            return Err(format!("response exceeds negotiated 576-octet MTU: {}", response.len()));
        }
        let sn = encode_sn_unitdata(nsapi, &response);
        self.send_unitdata(queue, ind, sn);
        tracing::info!(
            "SNDCP/WAP: -> ISSI={} NSAPI={} IPv4/UDP response {} octets",
            ind.received_tetra_address.ssi,
            nsapi,
            response.len()
        );
        Ok(())
    }

    fn allocate_dynamic_address(&mut self, issi: u32) -> Result<[u8; 4], String> {
        if let Some(address) = self.leases.get(&issi).copied() {
            return Ok(address);
        }
        let used: HashSet<[u8; 4]> = self.contexts.values().map(|context| context.address).collect();
        let first = self.wap.dynamic_pool_first_host;
        let last = self.wap.dynamic_pool_last_host;
        let span = (last as u16).saturating_sub(first as u16) + 1;
        let start = first as u16 + ((issi % u32::from(span.max(1))) as u16);
        for offset in 0..span {
            let host = first as u16 + ((start - first as u16 + offset) % span);
            let address = [
                self.wap.dynamic_pool_prefix[0],
                self.wap.dynamic_pool_prefix[1],
                self.wap.dynamic_pool_prefix[2],
                host as u8,
            ];
            if address != self.wap.address && !used.contains(&address) {
                self.leases.insert(issi, address);
                return Ok(address);
            }
        }
        Err("dynamic IPv4 pool exhausted".into())
    }

    fn status_snapshot(&self) -> WapStatusSnapshot {
        let cfg = self.config.config();
        let state = self.config.state_read();
        WapStatusSnapshot {
            title: self.wap.title.clone(),
            service_state: if cfg.cell.sndcp_service { "ON AIR".into() } else { "OFF".into() },
            registered_ms: state.subscribers.all_registered_issis().count(),
            active_calls: state.active_call_ts.len(),
            queued_sds: state.live_sds_queue.len(),
            uptime_secs: self.started.elapsed().as_secs(),
            carrier: cfg.cell.main_carrier,
            mcc: cfg.net.mcc,
            mnc: cfg.net.mnc,
        }
    }

    fn send_control(&self, queue: &mut MessageQueue, ind: &LtpdMleUnitdataInd, sn_pdu: BitBuffer, chan_alloc: Option<CmceChanAllocReq>) {
        queue.push_back(SapMsg {
            sap: Sap::TlaSap,
            src: TetraEntity::Sndcp,
            dest: TetraEntity::Llc,
            msg: SapMsgInner::TlaTlDataReqBl(TlaTlDataReqBl {
                main_address: ind.received_tetra_address,
                link_id: ind.link_id,
                endpoint_id: ind.endpoint_id,
                tl_sdu: prepend_mle_discriminator(sn_pdu),
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

    fn send_unitdata(&self, queue: &mut MessageQueue, ind: &LtpdMleUnitdataInd, sn_pdu: BitBuffer) {
        queue.push_back(SapMsg {
            sap: Sap::TlaSap,
            src: TetraEntity::Sndcp,
            dest: TetraEntity::Llc,
            msg: SapMsgInner::TlaTlUnitdataReqBl(TlaTlUnitdataReqBl {
                main_address: ind.received_tetra_address,
                link_id: ind.link_id,
                endpoint_id: ind.endpoint_id,
                tl_sdu: prepend_mle_discriminator(sn_pdu),
                stealing_permission: false,
                subscriber_class: 0,
                fcs_flag: false,
                air_interface_encryption: None,
                packet_data_flag: true,
                n_tlsdu_repeats: 0,
                data_class_info: None,
                req_handle: 0,
                chan_alloc: None,
                tx_reporter: None,
            }),
        });
    }
}

impl TetraEntityTrait for Sndcp {
    fn entity(&self) -> TetraEntity {
        TetraEntity::Sndcp
    }

    fn rx_prim(&mut self, queue: &mut MessageQueue, message: SapMsg) {
        let SapMsgInner::LtpdMleUnitdataInd(ind) = &message.msg else {
            tracing::debug!("SNDCP: unhandled primitive on {:?}: {:?}", message.sap, message.msg);
            return;
        };
        self.handle_indication(queue, ind);
    }
}

fn rebase_sn_pdu(input: &BitBuffer) -> Option<BitBuffer> {
    if input.get_pos() >= 3 {
        return Some(BitBuffer::from_bitbuffer_pos(input));
    }
    let mut clone = BitBuffer::from_bitbuffer(input);
    clone.seek(0);
    if clone.read_bits(3)? != MLE_DISCRIMINATOR_SNDCP {
        return None;
    }
    Some(BitBuffer::from_bitbuffer_pos(&clone))
}

fn prepend_mle_discriminator(mut sn_pdu: BitBuffer) -> BitBuffer {
    sn_pdu.seek(0);
    let len = sn_pdu.get_len();
    let mut out = BitBuffer::new(3 + len);
    out.write_bits(MLE_DISCRIMINATOR_SNDCP, 3);
    out.copy_bits(&mut sn_pdu, len);
    out.seek(0);
    out
}

fn decode_activate_demand(pdu: &BitBuffer) -> Result<ActivateDemand, (u8, String)> {
    let mut reader = reader(pdu);
    expect(&mut reader, 4, SN_ACTIVATE_PDP as u64, "activate type").map_err(|detail| (34, detail))?;
    let version = read(&mut reader, 4, "SNDCP version").map_err(|detail| (34, detail))?;
    if version != 1 {
        return Err((16, format!("unsupported SNDCP version {version}")));
    }
    let nsapi = read(&mut reader, 4, "NSAPI").map_err(|detail| (34, detail))? as u8;
    if !(1..=14).contains(&nsapi) {
        return Err((34, format!("reserved NSAPI {nsapi}")));
    }
    let atid = read(&mut reader, 3, "ATID").map_err(|detail| (34, detail))? as u8;
    let static_address = match atid {
        0 => {
            let raw = read(&mut reader, 32, "static IPv4").map_err(|detail| (8, detail))? as u32;
            Some(raw.to_be_bytes())
        }
        1 => None,
        2 => return Err((3, "IPv6 PDP contexts are not supported".into())),
        3 | 4 => return Err((2, "mobile IPv4 PDP contexts are not supported".into())),
        5 => return Err((27, "secondary PDP contexts are not supported".into())),
        other => return Err((34, format!("unsupported ATID {other}"))),
    };
    let ms_type = read(&mut reader, 4, "packet-data MS type").map_err(|detail| (34, detail))? as u8;
    if ms_type > 2 {
        return Err((15, format!("unsupported packet-data MS type {ms_type}")));
    }
    let pcomp = read(&mut reader, 8, "PCOMP negotiation").map_err(|detail| (34, detail))? as u8;
    if pcomp != 0 {
        return Err((34, format!("unsupported PCOMP negotiation {pcomp}")));
    }
    let _optional_elements = read(&mut reader, 1, "O-bit").map_err(|detail| (34, detail))? != 0;
    Ok(ActivateDemand { nsapi, static_address })
}

fn encode_activate_accept(nsapi: u8, tia: u64, address: [u8; 4], chap_id: Option<u8>) -> BitBuffer {
    let mut pdu = BitBuffer::new_autoexpand(192);
    pdu.write_bits(SN_ACTIVATE_PDP as u64, 4);
    pdu.write_bits(nsapi as u64, 4);
    pdu.write_bits(PDU_PRIORITY_MAX, 3);
    pdu.write_bits(READY_TIMER, 4);
    pdu.write_bits(STANDBY_TIMER, 4);
    pdu.write_bits(RESPONSE_WAIT_TIMER, 4);
    pdu.write_bits(tia, 3);
    pdu.write_bits(u32::from_be_bytes(address) as u64, 32);
    pdu.write_bits(0, 8);
    pdu.write_bits(MTU_576, 3);
    if let Some(id) = chap_id {
        for bit in chap_success_optional_section(id).bytes() {
            pdu.write_bits((bit - b'0') as u64, 1);
        }
    } else {
        pdu.write_bits(0, 1);
    }
    pdu.seek(0);
    pdu
}

fn encode_activate_reject(nsapi: u8, cause: u8) -> BitBuffer {
    let mut pdu = BitBuffer::new(17);
    pdu.write_bits(SN_ACTIVATE_REJECT as u64, 4);
    pdu.write_bits(nsapi as u64, 4);
    pdu.write_bits(cause as u64, 8);
    pdu.write_bits(0, 1);
    pdu.seek(0);
    pdu
}

fn encode_data_transmit_response(nsapi: u8, accepted: bool, cause: Option<u8>) -> BitBuffer {
    let mut pdu = BitBuffer::new_autoexpand(24);
    pdu.write_bits(SN_DATA_TRANSMIT_RESPONSE as u64, 4);
    pdu.write_bits(nsapi as u64, 4);
    pdu.write_bits(accepted as u64, 1);
    if !accepted {
        pdu.write_bits(cause.unwrap_or(0) as u64, 8);
    }
    pdu.write_bits(0, 1);
    pdu.seek(0);
    pdu
}

fn encode_sn_unitdata(nsapi: u8, npdu: &[u8]) -> BitBuffer {
    let mut pdu = BitBuffer::new(16 + npdu.len() * 8);
    pdu.write_bits(SN_UNITDATA as u64, 4);
    pdu.write_bits(nsapi as u64, 4);
    pdu.write_bits(0, 4);
    pdu.write_bits(0, 4);
    for byte in npdu {
        pdu.write_bits(*byte as u64, 8);
    }
    pdu.seek(0);
    pdu
}

fn packet_data_channel() -> CmceChanAllocReq {
    CmceChanAllocReq {
        usage: None,
        carrier: None,
        timeslots: [false, true, false, false],
        alloc_type: ChanAllocType::Replace,
        ul_dl_assigned: UlDlAssignment::Both,
    }
}

fn return_to_control_channel() -> CmceChanAllocReq {
    CmceChanAllocReq {
        usage: None,
        carrier: None,
        timeslots: [false, false, false, false],
        alloc_type: ChanAllocType::QuitAndGo,
        ul_dl_assigned: UlDlAssignment::Both,
    }
}

fn reader(pdu: &BitBuffer) -> BitBuffer {
    let mut reader = BitBuffer::from_bitbuffer(pdu);
    reader.seek(0);
    reader
}

fn read(reader: &mut BitBuffer, bits: usize, field: &'static str) -> Result<u64, String> {
    reader.read_bits(bits).ok_or_else(|| format!("truncated field {field}"))
}

fn expect(reader: &mut BitBuffer, bits: usize, expected: u64, field: &'static str) -> Result<(), String> {
    let actual = read(reader, bits, field)?;
    if actual == expected {
        Ok(())
    } else {
        Err(format!("unexpected {field}: expected {expected}, got {actual}"))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn hex_to_bits(hex: &str) -> String {
        hex.chars().map(|c| format!("{:04b}", c.to_digit(16).unwrap())).collect()
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
    fn optional_section_layout_matches_spec() {
        let section = chap_success_optional_section(5);
        assert_eq!(section.len(), 81);
        assert!(section.starts_with("1000100010000011110000001100001000100011000001000000001100000101"));
        assert!(section.ends_with("00000000000001000"));
    }

    #[test]
    fn unitdata_header_is_byte_exact() {
        let pdu = encode_sn_unitdata(2, &[0x45, 0x00, 0x00, 0x14]);
        assert_eq!(pdu.to_bitstr(), "010000100000000001000101000000000000000000010100");
    }

    #[test]
    fn dynamic_activation_demand_decodes_byte_exact_header() {
        let mut pdu = BitBuffer::new(28);
        pdu.write_bits(SN_ACTIVATE_PDP as u64, 4);
        pdu.write_bits(1, 4);
        pdu.write_bits(2, 4);
        pdu.write_bits(1, 3);
        pdu.write_bits(0, 4);
        pdu.write_bits(0, 8);
        pdu.write_bits(0, 1);
        pdu.seek(0);

        let demand = decode_activate_demand(&pdu).expect("dynamic IPv4 demand should decode");
        assert_eq!(demand.nsapi, 2);
        assert_eq!(demand.static_address, None);
    }

    #[test]
    fn dynamic_activation_accept_matches_reference_vector() {
        let pdu = encode_activate_accept(2, TIA_IPV4_DYNAMIC, [10, 0, 0, 226], None);
        assert_eq!(
            pdu.to_bitstr(),
            "0000001010010000100011101000001010000000000000000011100010000000000100"
        );
    }

    #[test]
    fn unsupported_ipv6_demand_maps_to_ipv6_reject_cause() {
        let mut pdu = BitBuffer::new(28);
        pdu.write_bits(SN_ACTIVATE_PDP as u64, 4);
        pdu.write_bits(1, 4);
        pdu.write_bits(2, 4);
        pdu.write_bits(2, 3);
        pdu.write_bits(0, 4);
        pdu.write_bits(0, 8);
        pdu.write_bits(0, 1);
        pdu.seek(0);

        let error = decode_activate_demand(&pdu).expect_err("IPv6 must be rejected by the IPv4 WAP profile");
        assert_eq!(error.0, 3);
    }
}

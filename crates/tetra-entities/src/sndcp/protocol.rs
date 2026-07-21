//! ETSI EN 300 392-2 clause 28 SNDCP wire primitives.
//!
//! This module deliberately keeps optional Type-2/Type-3 information elements as
//! opaque bit strings unless NetCore-Tetra actively consumes them. That makes the
//! decoder lossless and forward-compatible without pretending support for optional
//! compression/Mobile-IP profiles that are not advertised by this base station.

use tetra_core::BitBuffer;

use super::qos::{QosError, QosProfile};
use super::resource::{PhaseModulationResourceRequest, ResourceError};

pub const SN_ACTIVATE_PDP_CONTEXT: u8 = 0;
pub const SN_DEACTIVATE_PDP_CONTEXT_ACCEPT: u8 = 1;
pub const SN_DEACTIVATE_PDP_CONTEXT_DEMAND: u8 = 2;
pub const SN_ACTIVATE_PDP_CONTEXT_REJECT: u8 = 3;
pub const SN_UNITDATA: u8 = 4;
pub const SN_DATA: u8 = 5;
pub const SN_DATA_TRANSMIT_REQUEST: u8 = 6;
pub const SN_DATA_TRANSMIT_RESPONSE: u8 = 7;
pub const SN_END_OF_DATA: u8 = 8;
pub const SN_RECONNECT: u8 = 9;
pub const SN_PAGE: u8 = 10;
pub const SN_NOT_SUPPORTED: u8 = 11;
pub const SN_DATA_PRIORITY: u8 = 12;
pub const SN_MODIFY: u8 = 13;

pub const DATA_PRIORITY_ACKNOWLEDGEMENT: u8 = 0;
pub const DATA_PRIORITY_INFORMATION: u8 = 1;
pub const DATA_PRIORITY_REQUEST: u8 = 2;

pub const MODIFY_REQUEST: u8 = 0;
pub const MODIFY_RESPONSE: u8 = 1;
pub const MODIFY_AVAILABILITY: u8 = 3;
pub const MODIFY_USAGE: u8 = 4;

pub const SNDCP_VERSION_1: u8 = 1;
pub const PCOMP_NONE: u8 = 0;
pub const DCOMP_NONE: u8 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnDirection {
    Uplink,
    Downlink,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RawBits {
    pub bytes: Vec<u8>,
    pub bit_len: usize,
}

impl RawBits {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn from_remaining(reader: &BitBuffer) -> Self {
        let mut copy = BitBuffer::from_bitbuffer_pos(reader);
        let bit_len = copy.get_len_remaining();
        let mut bytes = vec![0u8; bit_len.div_ceil(8)];
        if bit_len != 0 {
            let _ = copy.read_bits_into_slice(bit_len, &mut bytes);
        }
        Self { bytes, bit_len }
    }

    pub fn write_to(&self, out: &mut BitBuffer) {
        for bit_index in 0..self.bit_len {
            let byte = self.bytes.get(bit_index / 8).copied().unwrap_or(0);
            let bit = (byte >> (7 - (bit_index % 8))) & 1;
            out.write_bits(u64::from(bit), 1);
        }
    }

    pub fn bit_string(&self) -> String {
        let mut out = String::with_capacity(self.bit_len);
        for i in 0..self.bit_len {
            let b = self.bytes.get(i / 8).copied().unwrap_or(0);
            out.push(if ((b >> (7 - (i % 8))) & 1) != 0 { '1' } else { '0' });
        }
        out
    }

    pub(crate) fn from_reader_exact(reader: &mut BitBuffer, bit_len: usize) -> Result<Self, ProtocolError> {
        if reader.get_len_remaining() < bit_len {
            return Err(ProtocolError::TooShort("raw_bits"));
        }
        let mut bytes = vec![0u8; bit_len.div_ceil(8)];
        if bit_len != 0 && reader.read_bits_into_slice(bit_len, &mut bytes).is_none() {
            return Err(ProtocolError::TooShort("raw_bits"));
        }
        Ok(Self { bytes, bit_len })
    }

    pub(crate) fn reader(&self) -> BitBuffer {
        let mut source = BitBuffer::from_bytes(&self.bytes);
        source.seek(0);
        let mut exact = BitBuffer::new(self.bit_len);
        if self.bit_len != 0 {
            exact.copy_bits(&mut source, self.bit_len);
        }
        exact.seek(0);
        exact
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Type34Element {
    pub identifier: u8,
    pub payload: RawBits,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionalElements {
    pub type2: Vec<Option<RawBits>>,
    pub type34: Vec<Type34Element>,
}

impl OptionalElements {
    pub fn find_type34(&self, identifier: u8) -> Option<&RawBits> {
        self.type34.iter().find(|element| element.identifier == identifier).map(|element| &element.payload)
    }
}

pub fn parse_optional_elements(raw: &RawBits, type2_lengths: &[usize]) -> Result<OptionalElements, ProtocolError> {
    if raw.bit_len == 0 {
        return Ok(OptionalElements { type2: vec![None; type2_lengths.len()], type34: Vec::new() });
    }
    let mut reader = raw.reader();
    let optional_follows = read(&mut reader, 1, "optional.o_bit")? != 0;
    if !optional_follows {
        if reader.get_len_remaining() != 0 {
            return Err(ProtocolError::MalformedOptional("trailing bits after O=0"));
        }
        return Ok(OptionalElements { type2: vec![None; type2_lengths.len()], type34: Vec::new() });
    }
    let mut type2 = Vec::with_capacity(type2_lengths.len());
    for length in type2_lengths.iter().copied() {
        let present = read(&mut reader, 1, "optional.p_bit")? != 0;
        type2.push(if present { Some(RawBits::from_reader_exact(&mut reader, length)?) } else { None });
    }
    let mut type34 = Vec::new();
    let mut more = read(&mut reader, 1, "optional.m_bit")? != 0;
    while more {
        let identifier = read_u8(&mut reader, 4, "optional.type34_identifier")?;
        let length = read(&mut reader, 11, "optional.type34_length")? as usize;
        let payload = RawBits::from_reader_exact(&mut reader, length)?;
        type34.push(Type34Element { identifier, payload });
        more = read(&mut reader, 1, "optional.m_bit")? != 0;
    }
    if reader.get_len_remaining() != 0 {
        return Err(ProtocolError::MalformedOptional("trailing bits after optional chain"));
    }
    Ok(OptionalElements { type2, type34 })
}

pub fn encode_optional_elements(type2: &[Option<RawBits>], type34: &[Type34Element]) -> RawBits {
    if type2.iter().all(Option::is_none) && type34.is_empty() {
        return RawBits { bytes: vec![0], bit_len: 1 };
    }
    let mut out = BitBuffer::new_autoexpand(128);
    out.write_bits(1, 1);
    for element in type2 {
        out.write_bits(element.is_some() as u64, 1);
        if let Some(element) = element {
            element.write_to(&mut out);
        }
    }
    for (index, element) in type34.iter().enumerate() {
        assert!(element.identifier <= 0x0f, "Type-3/4 identifier exceeds four bits");
        assert!(element.payload.bit_len <= 0x07ff, "Type-3/4 payload exceeds 2047 bits");
        out.write_bits(1, 1);
        out.write_bits(u64::from(element.identifier), 4);
        out.write_bits(element.payload.bit_len as u64, 11);
        element.payload.write_to(&mut out);
        if index + 1 == type34.len() {
            out.write_bits(0, 1);
        }
    }
    if type34.is_empty() {
        out.write_bits(0, 1);
    }
    let bit_len = out.get_pos();
    out.seek(0);
    RawBits::from_reader_exact(&mut out, bit_len).expect("locally encoded optional chain must be readable")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivateAddressDemand {
    Ipv4Static([u8; 4]),
    Ipv4Dynamic,
    Ipv6,
    MobileIpv4ForeignAgent,
    MobileIpv4CoLocated,
    Secondary { primary_nsapi: u8 },
    Reserved(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivateAddressAccept {
    None,
    Ipv4Static([u8; 4]),
    Ipv4Dynamic([u8; 4]),
    Reserved(u8),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivateDemand {
    pub version: u8,
    pub nsapi: u8,
    pub address: ActivateAddressDemand,
    pub packet_data_ms_type: u8,
    pub pcomp_negotiation: u8,
    pub vj_slots: Option<u8>,
    pub rfc2507: Option<Rfc2507Negotiation>,
    pub optional: RawBits,
}

impl ActivateDemand {
    pub fn optional_elements(&self) -> Result<OptionalElements, ProtocolError> {
        parse_optional_elements(&self.optional, &[16])
    }

    pub fn qos(&self) -> Result<Option<QosProfile>, ProtocolError> {
        let elements = self.optional_elements()?;
        elements.find_type34(3).map(QosProfile::decode).transpose().map_err(Into::into)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rfc2507Negotiation {
    pub tcp_slots: u8,
    pub non_tcp_slots: u16,
    pub max_header_interval: u8,
    pub max_header_time: u8,
    pub largest_header: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivateAccept {
    pub nsapi: u8,
    pub pdu_priority_max: u8,
    pub ready_timer: u8,
    pub standby_timer: u8,
    pub response_wait_timer: u8,
    pub address: ActivateAddressAccept,
    pub pcomp_negotiation: u8,
    pub vj_slots: Option<u8>,
    pub rfc2507: Option<Rfc2507Negotiation>,
    pub mtu_code: u8,
    pub optional: RawBits,
}

impl ActivateAccept {
    pub fn optional_elements(&self) -> Result<OptionalElements, ProtocolError> {
        parse_optional_elements(&self.optional, &[16, 98, 71])
    }

    pub fn qos(&self) -> Result<Option<QosProfile>, ProtocolError> {
        let elements = self.optional_elements()?;
        elements.find_type34(3).map(QosProfile::decode).transpose().map_err(Into::into)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivateReject {
    pub nsapi: u8,
    pub cause: u8,
    pub optional: RawBits,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserData {
    pub acknowledged: bool,
    pub nsapi: u8,
    pub pcomp: u8,
    pub dcomp: u8,
    pub n_pdu: RawBits,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataTransmitRequest {
    pub nsapis: Vec<u8>,
    pub logical_link_status: bool,
    pub enhanced_service: bool,
    pub resource_request: Option<PhaseModulationResourceRequest>,
    pub optional: RawBits,
}

impl DataTransmitRequest {
    pub fn network_endpoint_id(&self) -> Option<u16> {
        parse_optional_type2_u16(&self.optional, &[16, 20], 0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataTransmitResponse {
    pub nsapis: Vec<u8>,
    pub accepted: bool,
    pub reject_cause: Option<u8>,
    pub optional: RawBits,
}

impl DataTransmitResponse {
    pub fn network_endpoint_id(&self) -> Option<u16> {
        parse_optional_type2_u16(&self.optional, &[16], 0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deactivate {
    pub deactivation_type: u8,
    pub nsapi: Option<u8>,
    pub optional: RawBits,
}

impl Deactivate {
    pub fn network_endpoint_id(&self) -> Option<u16> {
        parse_optional_type2_u16(&self.optional, &[16, 11], 0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndOfData {
    pub immediate_service_change: bool,
    pub optional: RawBits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReconnectNsapi {
    pub nsapi: u8,
    pub data_to_send: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reconnect {
    /// Ordered by the priority sent by the MS. Duplicate NSAPIs are removed,
    /// preserving the first occurrence as required by EN 300 392-2.
    pub nsapis: Vec<ReconnectNsapi>,
    pub enhanced_service: bool,
    pub resource_request: Option<PhaseModulationResourceRequest>,
    pub optional: RawBits,
}

impl Reconnect {
    pub fn network_endpoint_id(&self) -> Option<u16> {
        parse_optional_type2_u16(&self.optional, &[16, 19], 0)
    }

    pub fn any_data_to_send(&self) -> bool {
        self.nsapis.iter().any(|entry| entry.data_to_send)
    }

    pub fn nsapi_values(&self) -> Vec<u8> {
        self.nsapis.iter().map(|entry| entry.nsapi).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageRequest {
    pub nsapi: u8,
    pub reply_requested: bool,
    pub optional: RawBits,
}

impl PageRequest {
    pub fn network_endpoint_id(&self) -> Option<u16> {
        parse_optional_type2_u16(&self.optional, &[16], 0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageResponse {
    pub nsapi: u8,
    pub pd_service_available: bool,
    pub logical_link_status: bool,
    pub enhanced_service: bool,
    pub resource_request: Option<PhaseModulationResourceRequest>,
    pub optional: RawBits,
}

impl PageResponse {
    pub fn network_endpoint_id(&self) -> Option<u16> {
        parse_optional_type2_u16(&self.optional, &[16, 18], 0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataPriorityDetails {
    pub network_default: u8,
    pub lifetime: u8,
    pub signalling_delay: u8,
    pub random_access_delay: u8,
}

impl DataPriorityDetails {
    pub fn default_for_network(network_default: u8) -> Self {
        Self {
            network_default: network_default.min(7),
            lifetime: 8,
            signalling_delay: 2,
            random_access_delay: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataPriority {
    Acknowledgement {
        accepted: bool,
        details: DataPriorityDetails,
        ms_default: Option<u8>,
    },
    Information {
        details: DataPriorityDetails,
        ms_default: Option<u8>,
    },
    Request {
        request_type: u8,
    },
    Reserved {
        subtype: u8,
        raw: RawBits,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Modify {
    Request {
        nsapi: u8,
        qos: RawBits,
    },
    ResponseApplied {
        nsapi: u8,
        pdu_priority_max: u8,
        qos: RawBits,
    },
    ResponseRejected {
        nsapi: u8,
        cause: u8,
        optional: RawBits,
    },
    Availability {
        nsapi: u8,
        availability: u8,
        optional: RawBits,
    },
    Usage {
        nsapi: u8,
        usage: u8,
        optional: RawBits,
    },
    Reserved {
        subtype: u8,
        raw: RawBits,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnPdu {
    ActivateDemand(ActivateDemand),
    ActivateAccept(ActivateAccept),
    ActivateReject(ActivateReject),
    DeactivateDemand(Deactivate),
    DeactivateAccept(Deactivate),
    Unitdata(UserData),
    Data(UserData),
    DataTransmitRequest(DataTransmitRequest),
    DataTransmitResponse(DataTransmitResponse),
    EndOfData(EndOfData),
    Reconnect(Reconnect),
    PageRequest(PageRequest),
    PageResponse(PageResponse),
    NotSupported { pdu_type: u8 },
    DataPriority(DataPriority),
    Modify(Modify),
    Reserved { pdu_type: u8, raw: RawBits },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    TooShort(&'static str),
    InvalidNsapi(u8),
    InvalidField { field: &'static str, value: u64 },
    NonOctetAlignedNPdu(usize),
    WrongDirection { pdu_type: u8, direction: SnDirection },
    Qos(QosError),
    Resource(ResourceError),
    MalformedOptional(&'static str),
}

impl From<QosError> for ProtocolError {
    fn from(value: QosError) -> Self { Self::Qos(value) }
}

impl From<ResourceError> for ProtocolError {
    fn from(value: ResourceError) -> Self { Self::Resource(value) }
}

fn read(reader: &mut BitBuffer, bits: usize, name: &'static str) -> Result<u64, ProtocolError> {
    reader.read_bits(bits).ok_or(ProtocolError::TooShort(name))
}

fn read_u8(reader: &mut BitBuffer, bits: usize, name: &'static str) -> Result<u8, ProtocolError> {
    Ok(read(reader, bits, name)? as u8)
}

fn read_ipv4(reader: &mut BitBuffer) -> Result<[u8; 4], ProtocolError> {
    Ok([
        read_u8(reader, 8, "ipv4[0]")?,
        read_u8(reader, 8, "ipv4[1]")?,
        read_u8(reader, 8, "ipv4[2]")?,
        read_u8(reader, 8, "ipv4[3]")?,
    ])
}

fn validate_nsapi(nsapi: u8) -> Result<u8, ProtocolError> {
    if (1..=14).contains(&nsapi) {
        Ok(nsapi)
    } else {
        Err(ProtocolError::InvalidNsapi(nsapi))
    }
}


fn parse_optional_type2_u16(raw: &RawBits, type2_lengths: &[usize], target_index: usize) -> Option<u16> {
    let mut reader = raw.reader();
    if raw.bit_len == 0 || reader.read_bits(1)? != 1 {
        return None;
    }
    for (index, length) in type2_lengths.iter().copied().enumerate() {
        let present = reader.read_bits(1)? != 0;
        if !present {
            continue;
        }
        let value = reader.read_bits(length)?;
        if index == target_index && length == 16 {
            return Some(value as u16);
        }
    }
    None
}

fn parse_optional_type4_nsapis(raw: &RawBits, type2_lengths: &[usize], type4_identifier: u8, entry_bits: usize) -> Vec<u8> {
    let mut reader = raw.reader();
    if raw.bit_len == 0 || reader.read_bits(1) != Some(1) {
        return Vec::new();
    }
    // Type-2 P-bits appear in the same order as the PDU table. Their contents are
    // irrelevant to the NSAPI list but must be skipped before the M-bit.
    for length in type2_lengths {
        let Some(present) = reader.read_bits(1) else {
            return Vec::new();
        };
        if present != 0 && reader.read_bits(*length).is_none() {
            return Vec::new();
        }
    }
    let mut more = match reader.read_bits(1) {
        Some(value) => value != 0,
        None => return Vec::new(),
    };
    let mut result = Vec::new();
    while more {
        let Some(identifier) = reader.read_bits(4).map(|value| value as u8) else {
            break;
        };
        let Some(length) = reader.read_bits(11).map(|value| value as usize) else {
            break;
        };
        if length > reader.get_len_remaining() {
            break;
        }
        let payload_start = reader.get_pos();
        if identifier == type4_identifier && length >= 6 {
            let count = reader.read_bits(6).unwrap_or(0) as usize;
            for _ in 0..count {
                if reader.get_pos().saturating_sub(payload_start) + entry_bits > length {
                    break;
                }
                let nsapi = reader.read_bits(4).unwrap_or(0) as u8;
                let tail_bits = entry_bits.saturating_sub(4);
                if tail_bits != 0 {
                    let _ = reader.read_bits(tail_bits);
                }
                if (1..=14).contains(&nsapi) && !result.contains(&nsapi) {
                    result.push(nsapi);
                }
            }
        }
        let consumed = reader.get_pos().saturating_sub(payload_start);
        if consumed < length {
            let _ = reader.read_bits(length - consumed);
        }
        more = reader.read_bits(1).unwrap_or(0) != 0;
    }
    result
}

fn parse_optional_reconnect_nsapis(raw: &RawBits) -> Vec<ReconnectNsapi> {
    let mut reader = raw.reader();
    if raw.bit_len == 0 || reader.read_bits(1) != Some(1) {
        return Vec::new();
    }
    // Optional Type-2 fields in table 28.42: SNEI (16) and reserved (19).
    for length in [16usize, 19] {
        let Some(present) = reader.read_bits(1) else {
            return Vec::new();
        };
        if present != 0 && reader.read_bits(length).is_none() {
            return Vec::new();
        }
    }
    let mut more = reader.read_bits(1).unwrap_or(0) != 0;
    let mut result = Vec::new();
    while more {
        let Some(identifier) = reader.read_bits(4).map(|value| value as u8) else {
            break;
        };
        let Some(length) = reader.read_bits(11).map(|value| value as usize) else {
            break;
        };
        if length > reader.get_len_remaining() {
            break;
        }
        let payload_start = reader.get_pos();
        if identifier == 5 && length >= 6 {
            let count = reader.read_bits(6).unwrap_or(0) as usize;
            for _ in 0..count {
                if reader.get_pos().saturating_sub(payload_start) + 5 > length {
                    break;
                }
                let nsapi = reader.read_bits(4).unwrap_or(0) as u8;
                let data_to_send = reader.read_bits(1).unwrap_or(0) != 0;
                if (1..=14).contains(&nsapi) && !result.iter().any(|entry: &ReconnectNsapi| entry.nsapi == nsapi) {
                    result.push(ReconnectNsapi { nsapi, data_to_send });
                }
            }
        }
        let consumed = reader.get_pos().saturating_sub(payload_start);
        if consumed < length {
            let _ = reader.read_bits(length - consumed);
        }
        more = reader.read_bits(1).unwrap_or(0) != 0;
    }
    result
}

fn write_optional_type4_nsapis(
    out: &mut BitBuffer,
    type2_count: usize,
    type4_identifier: u8,
    nsapis: &[u8],
    entry_tail_bits: usize,
    entry_tail_value: u64,
) {
    let mut values = Vec::new();
    for nsapi in nsapis.iter().copied() {
        if (1..=14).contains(&nsapi) && !values.contains(&nsapi) {
            values.push(nsapi);
        }
    }
    if values.is_empty() {
        out.write_bits(0, 1); // O-bit
        return;
    }
    out.write_bits(1, 1); // optional elements follow
    for _ in 0..type2_count {
        out.write_bits(0, 1); // optional Type-2 element absent
    }
    out.write_bits(1, 1); // Type-3/4 element follows
    out.write_bits(u64::from(type4_identifier & 0x0f), 4);
    let entry_bits = 4 + entry_tail_bits;
    let length = 6 + values.len() * entry_bits;
    out.write_bits(length as u64, 11);
    out.write_bits(values.len().min(63) as u64, 6);
    for nsapi in values.into_iter().take(63) {
        out.write_bits(u64::from(nsapi), 4);
        if entry_tail_bits != 0 {
            out.write_bits(entry_tail_value, entry_tail_bits);
        }
    }
    out.write_bits(0, 1); // no more Type-3/4 elements
}

fn write_optional_reconnect_nsapis(out: &mut BitBuffer, entries: &[ReconnectNsapi]) {
    let mut entries_unique = Vec::new();
    for entry in entries.iter().copied() {
        if (1..=14).contains(&entry.nsapi)
            && !entries_unique.iter().any(|existing: &ReconnectNsapi| existing.nsapi == entry.nsapi)
        {
            entries_unique.push(entry);
        }
        if entries_unique.len() == 14 {
            break;
        }
    }
    let entries = entries_unique;
    if entries.is_empty() {
        out.write_bits(0, 1); // O-bit
        return;
    }
    out.write_bits(1, 1);
    out.write_bits(0, 1); // SNEI absent
    out.write_bits(0, 1); // reserved Type-2 absent
    out.write_bits(1, 1); // Type-4 follows
    out.write_bits(5, 4); // NSAPI for reconnection
    out.write_bits((6 + entries.len() * 5) as u64, 11);
    out.write_bits(entries.len() as u64, 6);
    for entry in entries {
        out.write_bits(u64::from(entry.nsapi), 4);
        out.write_bits(entry.data_to_send as u64, 1);
    }
    out.write_bits(0, 1);
}

fn read_priority_details(reader: &mut BitBuffer) -> Result<DataPriorityDetails, ProtocolError> {
    let network_default = read_u8(reader, 3, "network_default_data_priority")?;
    let lifetime = read_u8(reader, 6, "priority_lifetime")?;
    let signalling_delay = read_u8(reader, 3, "priority_signalling_delay")?;
    let random_access_delay = read_u8(reader, 3, "priority_random_access_delay")?;
    let _reserved = read(reader, 9, "priority_reserved")?;
    Ok(DataPriorityDetails { network_default, lifetime, signalling_delay, random_access_delay })
}

fn write_priority_details(out: &mut BitBuffer, details: DataPriorityDetails) {
    out.write_bits(u64::from(details.network_default.min(7)), 3);
    out.write_bits(u64::from(details.lifetime.min(63)), 6);
    out.write_bits(u64::from(details.signalling_delay.min(7)), 3);
    out.write_bits(u64::from(details.random_access_delay.min(7)), 3);
    out.write_bits(0, 9);
}

pub fn decode(pdu: &BitBuffer, direction: SnDirection) -> Result<SnPdu, ProtocolError> {
    let mut reader = BitBuffer::from_bitbuffer(pdu);
    let pdu_type = read_u8(&mut reader, 4, "sn_pdu_type")?;
    match pdu_type {
        SN_ACTIVATE_PDP_CONTEXT => match direction {
            SnDirection::Uplink => decode_activate_demand(&mut reader).map(SnPdu::ActivateDemand),
            SnDirection::Downlink => decode_activate_accept(&mut reader).map(SnPdu::ActivateAccept),
        },
        SN_DEACTIVATE_PDP_CONTEXT_ACCEPT => decode_deactivate(&mut reader).map(SnPdu::DeactivateAccept),
        SN_DEACTIVATE_PDP_CONTEXT_DEMAND => decode_deactivate(&mut reader).map(SnPdu::DeactivateDemand),
        SN_ACTIVATE_PDP_CONTEXT_REJECT => decode_activate_reject(&mut reader).map(SnPdu::ActivateReject),
        SN_UNITDATA => decode_user_data(&mut reader, false).map(SnPdu::Unitdata),
        SN_DATA => decode_user_data(&mut reader, true).map(SnPdu::Data),
        SN_DATA_TRANSMIT_REQUEST => decode_data_transmit_request(&mut reader).map(SnPdu::DataTransmitRequest),
        SN_DATA_TRANSMIT_RESPONSE => decode_data_transmit_response(&mut reader).map(SnPdu::DataTransmitResponse),
        SN_END_OF_DATA => {
            let immediate_service_change = read(&mut reader, 1, "immediate_service_change")? != 0;
            Ok(SnPdu::EndOfData(EndOfData { immediate_service_change, optional: RawBits::from_remaining(&reader) }))
        }
        SN_RECONNECT => decode_reconnect(&mut reader).map(SnPdu::Reconnect),
        SN_PAGE => match direction {
            SnDirection::Uplink => decode_page_response(&mut reader).map(SnPdu::PageResponse),
            SnDirection::Downlink => decode_page_request(&mut reader).map(SnPdu::PageRequest),
        },
        SN_NOT_SUPPORTED => {
            let pdu_type = read_u8(&mut reader, 4, "not_supported_pdu_type")?;
            Ok(SnPdu::NotSupported { pdu_type })
        }
        SN_DATA_PRIORITY => decode_data_priority(&mut reader).map(SnPdu::DataPriority),
        SN_MODIFY => decode_modify(&mut reader).map(SnPdu::Modify),
        other => Ok(SnPdu::Reserved { pdu_type: other, raw: RawBits::from_remaining(&reader) }),
    }
}

fn decode_activate_demand(reader: &mut BitBuffer) -> Result<ActivateDemand, ProtocolError> {
    let version = read_u8(reader, 4, "sndcp_version")?;
    let nsapi = validate_nsapi(read_u8(reader, 4, "nsapi")?)?;
    let atid = read_u8(reader, 3, "atid")?;
    let address = match atid {
        0 => ActivateAddressDemand::Ipv4Static(read_ipv4(reader)?),
        1 => ActivateAddressDemand::Ipv4Dynamic,
        2 => ActivateAddressDemand::Ipv6,
        3 => ActivateAddressDemand::MobileIpv4ForeignAgent,
        4 => ActivateAddressDemand::MobileIpv4CoLocated,
        5 => ActivateAddressDemand::Secondary { primary_nsapi: validate_nsapi(read_u8(reader, 4, "primary_nsapi")?)? },
        other => ActivateAddressDemand::Reserved(other),
    };
    let packet_data_ms_type = read_u8(reader, 4, "packet_data_ms_type")?;
    let pcomp_negotiation = read_u8(reader, 8, "pcomp_negotiation")?;
    let vj_slots = if pcomp_negotiation & 0x01 != 0 { Some(read_u8(reader, 8, "vj_slots")?) } else { None };
    let rfc2507 = if pcomp_negotiation & 0x02 != 0 {
        Some(Rfc2507Negotiation {
            tcp_slots: read_u8(reader, 8, "tcp_slots")?,
            non_tcp_slots: read(reader, 16, "non_tcp_slots")? as u16,
            max_header_interval: read_u8(reader, 8, "max_header_interval")?,
            max_header_time: read_u8(reader, 8, "max_header_time")?,
            largest_header: read_u8(reader, 8, "largest_header")?,
        })
    } else {
        None
    };
    Ok(ActivateDemand {
        version,
        nsapi,
        address,
        packet_data_ms_type,
        pcomp_negotiation,
        vj_slots,
        rfc2507,
        optional: RawBits::from_remaining(reader),
    })
}

fn decode_activate_accept(reader: &mut BitBuffer) -> Result<ActivateAccept, ProtocolError> {
    let nsapi = validate_nsapi(read_u8(reader, 4, "nsapi")?)?;
    let pdu_priority_max = read_u8(reader, 3, "pdu_priority_max")?;
    let ready_timer = read_u8(reader, 4, "ready_timer")?;
    let standby_timer = read_u8(reader, 4, "standby_timer")?;
    let response_wait_timer = read_u8(reader, 4, "response_wait_timer")?;
    let tia = read_u8(reader, 3, "type_identifier_accept")?;
    let address = match tia {
        0 => ActivateAddressAccept::None,
        1 => ActivateAddressAccept::Ipv4Static(read_ipv4(reader)?),
        2 => ActivateAddressAccept::Ipv4Dynamic(read_ipv4(reader)?),
        other => ActivateAddressAccept::Reserved(other),
    };
    let pcomp_negotiation = read_u8(reader, 8, "pcomp_negotiation")?;
    let vj_slots = if pcomp_negotiation & 0x01 != 0 { Some(read_u8(reader, 8, "vj_slots")?) } else { None };
    let rfc2507 = if pcomp_negotiation & 0x02 != 0 {
        Some(Rfc2507Negotiation {
            tcp_slots: read_u8(reader, 8, "tcp_slots")?,
            non_tcp_slots: read(reader, 16, "non_tcp_slots")? as u16,
            max_header_interval: read_u8(reader, 8, "max_header_interval")?,
            max_header_time: read_u8(reader, 8, "max_header_time")?,
            largest_header: read_u8(reader, 8, "largest_header")?,
        })
    } else {
        None
    };
    let mtu_code = read_u8(reader, 3, "mtu_code")?;
    Ok(ActivateAccept {
        nsapi,
        pdu_priority_max,
        ready_timer,
        standby_timer,
        response_wait_timer,
        address,
        pcomp_negotiation,
        vj_slots,
        rfc2507,
        mtu_code,
        optional: RawBits::from_remaining(reader),
    })
}

fn decode_activate_reject(reader: &mut BitBuffer) -> Result<ActivateReject, ProtocolError> {
    Ok(ActivateReject {
        nsapi: validate_nsapi(read_u8(reader, 4, "nsapi")?)?,
        cause: read_u8(reader, 8, "activation_reject_cause")?,
        optional: RawBits::from_remaining(reader),
    })
}

fn decode_user_data(reader: &mut BitBuffer, acknowledged: bool) -> Result<UserData, ProtocolError> {
    let nsapi = validate_nsapi(read_u8(reader, 4, "nsapi")?)?;
    let pcomp = read_u8(reader, 4, "pcomp")?;
    let dcomp = read_u8(reader, 4, "dcomp")?;
    let n_pdu = RawBits::from_remaining(reader);
    if n_pdu.bit_len % 8 != 0 {
        return Err(ProtocolError::NonOctetAlignedNPdu(n_pdu.bit_len));
    }
    Ok(UserData { acknowledged, nsapi, pcomp, dcomp, n_pdu })
}

fn decode_data_transmit_request(reader: &mut BitBuffer) -> Result<DataTransmitRequest, ProtocolError> {
    let primary = validate_nsapi(read_u8(reader, 4, "nsapi")?)?;
    let logical_link_status = read(reader, 1, "logical_link_status")? != 0;
    let enhanced_service = read(reader, 1, "enhanced_service")? != 0;
    let raw = RawBits::from_remaining(reader);
    let (resource_request, optional) = if enhanced_service {
        let (request, optional) = PhaseModulationResourceRequest::decode_prefix(&raw)?;
        (Some(request), optional)
    } else {
        (None, raw)
    };
    let mut nsapis = vec![primary];
    for nsapi in parse_optional_type4_nsapis(&optional, &[16, 20], 4, 6) {
        if !nsapis.contains(&nsapi) {
            nsapis.push(nsapi);
        }
    }
    Ok(DataTransmitRequest { nsapis, logical_link_status, enhanced_service, resource_request, optional })
}

fn decode_data_transmit_response(reader: &mut BitBuffer) -> Result<DataTransmitResponse, ProtocolError> {
    let primary = validate_nsapi(read_u8(reader, 4, "nsapi")?)?;
    let accepted = read(reader, 1, "accept_reject")? != 0;
    let reject_cause = if accepted { None } else { Some(read_u8(reader, 8, "reject_cause")?) };
    let raw = RawBits::from_remaining(reader);
    let mut nsapis = vec![primary];
    for nsapi in parse_optional_type4_nsapis(&raw, &[16], 4, 6) {
        if !nsapis.contains(&nsapi) {
            nsapis.push(nsapi);
        }
    }
    Ok(DataTransmitResponse { nsapis, accepted, reject_cause, optional: raw })
}

fn decode_deactivate(reader: &mut BitBuffer) -> Result<Deactivate, ProtocolError> {
    let deactivation_type = read_u8(reader, 8, "deactivation_type")?;
    let nsapi = match deactivation_type {
        0 => None,
        _ => Some(validate_nsapi(read_u8(reader, 4, "nsapi")?)?),
    };
    Ok(Deactivate { deactivation_type, nsapi, optional: RawBits::from_remaining(reader) })
}

fn decode_reconnect(reader: &mut BitBuffer) -> Result<Reconnect, ProtocolError> {
    let data_to_send = read(reader, 1, "data_to_send")? != 0;
    let primary = if data_to_send {
        Some(ReconnectNsapi {
            nsapi: validate_nsapi(read_u8(reader, 4, "nsapi")?)?,
            data_to_send: true,
        })
    } else {
        None
    };
    let enhanced_service = read(reader, 1, "enhanced_service")? != 0;
    let raw = RawBits::from_remaining(reader);
    let (resource_request, optional) = if enhanced_service {
        let (request, optional) = PhaseModulationResourceRequest::decode_prefix(&raw)?;
        (Some(request), optional)
    } else {
        (None, raw)
    };
    let mut nsapis = primary.into_iter().collect::<Vec<_>>();
    for entry in parse_optional_reconnect_nsapis(&optional) {
        if !nsapis.iter().any(|existing| existing.nsapi == entry.nsapi) {
            nsapis.push(entry);
        }
    }
    Ok(Reconnect { nsapis, enhanced_service, resource_request, optional })
}

fn decode_page_request(reader: &mut BitBuffer) -> Result<PageRequest, ProtocolError> {
    Ok(PageRequest {
        nsapi: validate_nsapi(read_u8(reader, 4, "nsapi")?)?,
        reply_requested: read(reader, 1, "reply_requested")? != 0,
        optional: RawBits::from_remaining(reader),
    })
}

fn decode_page_response(reader: &mut BitBuffer) -> Result<PageResponse, ProtocolError> {
    let nsapi = validate_nsapi(read_u8(reader, 4, "nsapi")?)?;
    let pd_service_available = read(reader, 1, "pd_service_status")? != 0;
    let logical_link_status = read(reader, 1, "logical_link_status")? != 0;
    let enhanced_service = read(reader, 1, "enhanced_service")? != 0;
    let raw = RawBits::from_remaining(reader);
    let (resource_request, optional) = if enhanced_service {
        let (request, optional) = PhaseModulationResourceRequest::decode_prefix(&raw)?;
        (Some(request), optional)
    } else {
        (None, raw)
    };
    Ok(PageResponse { nsapi, pd_service_available, logical_link_status, enhanced_service, resource_request, optional })
}

fn decode_data_priority(reader: &mut BitBuffer) -> Result<DataPriority, ProtocolError> {
    let subtype = read_u8(reader, 4, "data_priority_subtype")?;
    match subtype {
        DATA_PRIORITY_ACKNOWLEDGEMENT => {
            let accepted = read(reader, 1, "request_result")? == 0;
            let details = read_priority_details(reader)?;
            let ms_default = if accepted { Some(read_u8(reader, 4, "ms_default_priority")?) } else { None };
            Ok(DataPriority::Acknowledgement { accepted, details, ms_default })
        }
        DATA_PRIORITY_INFORMATION => {
            let details = read_priority_details(reader)?;
            let included = read(reader, 1, "ms_default_flag")? != 0;
            let ms_default = if included { Some(read_u8(reader, 4, "ms_default_priority")?) } else { None };
            Ok(DataPriority::Information { details, ms_default })
        }
        DATA_PRIORITY_REQUEST => Ok(DataPriority::Request { request_type: read_u8(reader, 4, "request_type")? }),
        other => Ok(DataPriority::Reserved { subtype: other, raw: RawBits::from_remaining(reader) }),
    }
}

fn decode_modify(reader: &mut BitBuffer) -> Result<Modify, ProtocolError> {
    let subtype = read_u8(reader, 4, "modify_subtype")?;
    match subtype {
        MODIFY_REQUEST => Ok(Modify::Request {
            nsapi: validate_nsapi(read_u8(reader, 4, "nsapi")?)?,
            qos: RawBits::from_remaining(reader),
        }),
        MODIFY_RESPONSE => {
            let nsapi = validate_nsapi(read_u8(reader, 4, "nsapi")?)?;
            let rejected = read(reader, 1, "modification_result")? != 0;
            if rejected {
                Ok(Modify::ResponseRejected {
                    nsapi,
                    cause: read_u8(reader, 8, "modification_reject_cause")?,
                    optional: RawBits::from_remaining(reader),
                })
            } else {
                Ok(Modify::ResponseApplied {
                    nsapi,
                    pdu_priority_max: read_u8(reader, 3, "pdu_priority_max")?,
                    qos: RawBits::from_remaining(reader),
                })
            }
        }
        MODIFY_AVAILABILITY => Ok(Modify::Availability {
            nsapi: validate_nsapi(read_u8(reader, 4, "nsapi")?)?,
            availability: read_u8(reader, 3, "availability")?,
            optional: RawBits::from_remaining(reader),
        }),
        MODIFY_USAGE => Ok(Modify::Usage {
            nsapi: validate_nsapi(read_u8(reader, 4, "nsapi")?)?,
            usage: read_u8(reader, 3, "usage")?,
            optional: RawBits::from_remaining(reader),
        }),
        other => Ok(Modify::Reserved { subtype: other, raw: RawBits::from_remaining(reader) }),
    }
}

fn write_optional_or_absent(out: &mut BitBuffer, optional: &RawBits) {
    if optional.bit_len == 0 {
        out.write_bits(0, 1);
    } else {
        optional.write_to(out);
    }
}

pub fn encode(pdu: &SnPdu) -> BitBuffer {
    let mut out = BitBuffer::new_autoexpand(128);
    match pdu {
        SnPdu::ActivateDemand(v) => encode_activate_demand(&mut out, v),
        SnPdu::ActivateAccept(v) => encode_activate_accept(&mut out, v),
        SnPdu::ActivateReject(v) => {
            out.write_bits(u64::from(SN_ACTIVATE_PDP_CONTEXT_REJECT), 4);
            out.write_bits(u64::from(v.nsapi), 4);
            out.write_bits(u64::from(v.cause), 8);
            write_optional_or_absent(&mut out, &v.optional);
        }
        SnPdu::DeactivateDemand(v) => encode_deactivate(&mut out, SN_DEACTIVATE_PDP_CONTEXT_DEMAND, v),
        SnPdu::DeactivateAccept(v) => encode_deactivate(&mut out, SN_DEACTIVATE_PDP_CONTEXT_ACCEPT, v),
        SnPdu::Unitdata(v) | SnPdu::Data(v) => encode_user_data(&mut out, v),
        SnPdu::DataTransmitRequest(v) => encode_data_transmit_request(&mut out, v),
        SnPdu::DataTransmitResponse(v) => encode_data_transmit_response(&mut out, v),
        SnPdu::EndOfData(v) => {
            out.write_bits(u64::from(SN_END_OF_DATA), 4);
            out.write_bits(v.immediate_service_change as u64, 1);
            write_optional_or_absent(&mut out, &v.optional);
        }
        SnPdu::Reconnect(v) => encode_reconnect(&mut out, v),
        SnPdu::PageRequest(v) => {
            out.write_bits(u64::from(SN_PAGE), 4);
            out.write_bits(u64::from(v.nsapi), 4);
            out.write_bits(v.reply_requested as u64, 1);
            write_optional_or_absent(&mut out, &v.optional);
        }
        SnPdu::PageResponse(v) => {
            out.write_bits(u64::from(SN_PAGE), 4);
            out.write_bits(u64::from(v.nsapi), 4);
            out.write_bits(v.pd_service_available as u64, 1);
            out.write_bits(v.logical_link_status as u64, 1);
            out.write_bits(v.enhanced_service as u64, 1);
            if v.enhanced_service {
                v.resource_request.unwrap_or_else(PhaseModulationResourceRequest::one_slot_symmetric).encode(&mut out);
            }
            write_optional_or_absent(&mut out, &v.optional);
        }
        SnPdu::NotSupported { pdu_type } => {
            out.write_bits(u64::from(SN_NOT_SUPPORTED), 4);
            out.write_bits(u64::from(*pdu_type & 0x0f), 4);
        }
        SnPdu::DataPriority(v) => encode_data_priority(&mut out, v),
        SnPdu::Modify(v) => encode_modify(&mut out, v),
        SnPdu::Reserved { pdu_type, raw } => {
            out.write_bits(u64::from(*pdu_type & 0x0f), 4);
            raw.write_to(&mut out);
        }
    }
    out.seek(0);
    out
}

fn write_ipv4(out: &mut BitBuffer, address: [u8; 4]) {
    for octet in address {
        out.write_bits(u64::from(octet), 8);
    }
}

fn encode_activate_demand(out: &mut BitBuffer, v: &ActivateDemand) {
    out.write_bits(u64::from(SN_ACTIVATE_PDP_CONTEXT), 4);
    out.write_bits(u64::from(v.version), 4);
    out.write_bits(u64::from(v.nsapi), 4);
    match v.address {
        ActivateAddressDemand::Ipv4Static(address) => {
            out.write_bits(0, 3);
            write_ipv4(out, address);
        }
        ActivateAddressDemand::Ipv4Dynamic => out.write_bits(1, 3),
        ActivateAddressDemand::Ipv6 => out.write_bits(2, 3),
        ActivateAddressDemand::MobileIpv4ForeignAgent => out.write_bits(3, 3),
        ActivateAddressDemand::MobileIpv4CoLocated => out.write_bits(4, 3),
        ActivateAddressDemand::Secondary { primary_nsapi } => {
            out.write_bits(5, 3);
            out.write_bits(u64::from(primary_nsapi), 4);
        }
        ActivateAddressDemand::Reserved(code) => out.write_bits(u64::from(code & 7), 3),
    }
    out.write_bits(u64::from(v.packet_data_ms_type), 4);
    out.write_bits(u64::from(v.pcomp_negotiation), 8);
    if v.pcomp_negotiation & 0x01 != 0 {
        out.write_bits(u64::from(v.vj_slots.unwrap_or(0)), 8);
    }
    if v.pcomp_negotiation & 0x02 != 0 {
        let p = v.rfc2507.unwrap_or(Rfc2507Negotiation {
            tcp_slots: 0,
            non_tcp_slots: 0,
            max_header_interval: 0,
            max_header_time: 0,
            largest_header: 0,
        });
        out.write_bits(u64::from(p.tcp_slots), 8);
        out.write_bits(u64::from(p.non_tcp_slots), 16);
        out.write_bits(u64::from(p.max_header_interval), 8);
        out.write_bits(u64::from(p.max_header_time), 8);
        out.write_bits(u64::from(p.largest_header), 8);
    }
    write_optional_or_absent(out, &v.optional);
}

fn encode_activate_accept(out: &mut BitBuffer, v: &ActivateAccept) {
    out.write_bits(u64::from(SN_ACTIVATE_PDP_CONTEXT), 4);
    out.write_bits(u64::from(v.nsapi), 4);
    out.write_bits(u64::from(v.pdu_priority_max.min(7)), 3);
    out.write_bits(u64::from(v.ready_timer & 0x0f), 4);
    out.write_bits(u64::from(v.standby_timer & 0x0f), 4);
    out.write_bits(u64::from(v.response_wait_timer & 0x0f), 4);
    match v.address {
        ActivateAddressAccept::None => out.write_bits(0, 3),
        ActivateAddressAccept::Ipv4Static(address) => {
            out.write_bits(1, 3);
            write_ipv4(out, address);
        }
        ActivateAddressAccept::Ipv4Dynamic(address) => {
            out.write_bits(2, 3);
            write_ipv4(out, address);
        }
        ActivateAddressAccept::Reserved(code) => out.write_bits(u64::from(code & 7), 3),
    }
    out.write_bits(u64::from(v.pcomp_negotiation), 8);
    if v.pcomp_negotiation & 0x01 != 0 {
        out.write_bits(u64::from(v.vj_slots.unwrap_or(0)), 8);
    }
    if v.pcomp_negotiation & 0x02 != 0 {
        let p = v.rfc2507.unwrap_or(Rfc2507Negotiation {
            tcp_slots: 0,
            non_tcp_slots: 0,
            max_header_interval: 0,
            max_header_time: 0,
            largest_header: 0,
        });
        out.write_bits(u64::from(p.tcp_slots), 8);
        out.write_bits(u64::from(p.non_tcp_slots), 16);
        out.write_bits(u64::from(p.max_header_interval), 8);
        out.write_bits(u64::from(p.max_header_time), 8);
        out.write_bits(u64::from(p.largest_header), 8);
    }
    out.write_bits(u64::from(v.mtu_code & 7), 3);
    write_optional_or_absent(out, &v.optional);
}

fn encode_user_data(out: &mut BitBuffer, v: &UserData) {
    out.write_bits(u64::from(if v.acknowledged { SN_DATA } else { SN_UNITDATA }), 4);
    out.write_bits(u64::from(v.nsapi), 4);
    out.write_bits(u64::from(v.pcomp & 0x0f), 4);
    out.write_bits(u64::from(v.dcomp & 0x0f), 4);
    v.n_pdu.write_to(out);
}

fn encode_data_transmit_request(out: &mut BitBuffer, v: &DataTransmitRequest) {
    out.write_bits(u64::from(SN_DATA_TRANSMIT_REQUEST), 4);
    out.write_bits(u64::from(v.nsapis.first().copied().unwrap_or(1)), 4);
    out.write_bits(v.logical_link_status as u64, 1);
    out.write_bits(v.enhanced_service as u64, 1);
    if v.enhanced_service {
        v.resource_request.unwrap_or_else(PhaseModulationResourceRequest::one_slot_symmetric).encode(out);
    }
    if v.optional.bit_len != 0 {
        v.optional.write_to(out);
    } else if v.nsapis.len() > 1 {
        write_optional_type4_nsapis(out, 2, 4, &v.nsapis[1..], 2, 0);
    } else {
        out.write_bits(0, 1);
    }
}

fn encode_data_transmit_response(out: &mut BitBuffer, v: &DataTransmitResponse) {
    out.write_bits(u64::from(SN_DATA_TRANSMIT_RESPONSE), 4);
    out.write_bits(u64::from(v.nsapis.first().copied().unwrap_or(1)), 4);
    out.write_bits(v.accepted as u64, 1);
    if !v.accepted {
        out.write_bits(u64::from(v.reject_cause.unwrap_or(0)), 8);
    }
    if v.optional.bit_len != 0 {
        v.optional.write_to(out);
    } else if v.nsapis.len() > 1 && v.accepted {
        write_optional_type4_nsapis(out, 1, 4, &v.nsapis[1..], 2, 0);
    } else {
        out.write_bits(0, 1);
    }
}

fn encode_deactivate(out: &mut BitBuffer, pdu_type: u8, v: &Deactivate) {
    out.write_bits(u64::from(pdu_type), 4);
    out.write_bits(u64::from(v.deactivation_type), 8);
    if v.deactivation_type != 0 {
        out.write_bits(u64::from(v.nsapi.unwrap_or(1)), 4);
    }
    write_optional_or_absent(out, &v.optional);
}

fn encode_reconnect(out: &mut BitBuffer, v: &Reconnect) {
    out.write_bits(u64::from(SN_RECONNECT), 4);
    let primary_index = v.nsapis.iter().position(|entry| entry.data_to_send);
    out.write_bits(primary_index.is_some() as u64, 1);
    if let Some(index) = primary_index {
        out.write_bits(u64::from(v.nsapis[index].nsapi), 4);
    }
    out.write_bits(v.enhanced_service as u64, 1);
    if v.enhanced_service {
        v.resource_request.unwrap_or_else(PhaseModulationResourceRequest::one_slot_symmetric).encode(out);
    }
    if v.optional.bit_len != 0 {
        v.optional.write_to(out);
    } else {
        let additional = v
            .nsapis
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| (Some(index) != primary_index).then_some(*entry))
            .collect::<Vec<_>>();
        write_optional_reconnect_nsapis(out, &additional);
    }
}

fn encode_data_priority(out: &mut BitBuffer, v: &DataPriority) {
    out.write_bits(u64::from(SN_DATA_PRIORITY), 4);
    match v {
        DataPriority::Acknowledgement { accepted, details, ms_default } => {
            out.write_bits(u64::from(DATA_PRIORITY_ACKNOWLEDGEMENT), 4);
            out.write_bits((!*accepted) as u64, 1);
            write_priority_details(out, *details);
            if *accepted {
                out.write_bits(u64::from(ms_default.unwrap_or(8) & 0x0f), 4);
            }
        }
        DataPriority::Information { details, ms_default } => {
            out.write_bits(u64::from(DATA_PRIORITY_INFORMATION), 4);
            write_priority_details(out, *details);
            out.write_bits(ms_default.is_some() as u64, 1);
            if let Some(priority) = ms_default {
                out.write_bits(u64::from(*priority & 0x0f), 4);
            }
        }
        DataPriority::Request { request_type } => {
            out.write_bits(u64::from(DATA_PRIORITY_REQUEST), 4);
            out.write_bits(u64::from(*request_type & 0x0f), 4);
        }
        DataPriority::Reserved { subtype, raw } => {
            out.write_bits(u64::from(*subtype & 0x0f), 4);
            raw.write_to(out);
        }
    }
}

fn encode_modify(out: &mut BitBuffer, v: &Modify) {
    out.write_bits(u64::from(SN_MODIFY), 4);
    match v {
        Modify::Request { nsapi, qos } => {
            out.write_bits(u64::from(MODIFY_REQUEST), 4);
            out.write_bits(u64::from(*nsapi), 4);
            qos.write_to(out);
        }
        Modify::ResponseApplied { nsapi, pdu_priority_max, qos } => {
            out.write_bits(u64::from(MODIFY_RESPONSE), 4);
            out.write_bits(u64::from(*nsapi), 4);
            out.write_bits(0, 1);
            out.write_bits(u64::from(*pdu_priority_max & 7), 3);
            qos.write_to(out);
        }
        Modify::ResponseRejected { nsapi, cause, optional } => {
            out.write_bits(u64::from(MODIFY_RESPONSE), 4);
            out.write_bits(u64::from(*nsapi), 4);
            out.write_bits(1, 1);
            out.write_bits(u64::from(*cause), 8);
            optional.write_to(out);
        }
        Modify::Availability { nsapi, availability, optional } => {
            out.write_bits(u64::from(MODIFY_AVAILABILITY), 4);
            out.write_bits(u64::from(*nsapi), 4);
            out.write_bits(u64::from(*availability & 7), 3);
            optional.write_to(out);
        }
        Modify::Usage { nsapi, usage, optional } => {
            out.write_bits(u64::from(MODIFY_USAGE), 4);
            out.write_bits(u64::from(*nsapi), 4);
            out.write_bits(u64::from(*usage & 7), 3);
            write_optional_or_absent(out, optional);
        }
        Modify::Reserved { subtype, raw } => {
            out.write_bits(u64::from(*subtype & 0x0f), 4);
            raw.write_to(out);
        }
    }
}

pub fn raw_octets(bytes: Vec<u8>) -> RawBits {
    RawBits { bit_len: bytes.len() * 8, bytes }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bits(hex: &[u8]) -> BitBuffer {
        BitBuffer::from_bytes(hex)
    }

    #[test]
    fn all_top_level_types_are_directionally_decodable() {
        let activate = SnPdu::ActivateDemand(ActivateDemand {
            version: 1,
            nsapi: 2,
            address: ActivateAddressDemand::Ipv4Dynamic,
            packet_data_ms_type: 0,
            pcomp_negotiation: 0,
            vj_slots: None,
            rfc2507: None,
            optional: RawBits { bytes: vec![0], bit_len: 1 },
        });
        assert!(matches!(decode(&encode(&activate), SnDirection::Uplink), Ok(SnPdu::ActivateDemand(_))));
        assert!(matches!(decode(&bits(&[0xb4]), SnDirection::Uplink), Ok(SnPdu::NotSupported { pdu_type: 4 })));
    }

    #[test]
    fn every_standard_pdu_family_and_subtype_round_trips() {
        let zero = RawBits { bytes: vec![0], bit_len: 1 };
        let qos = QosProfile::Background.encode();
        let priority = DataPriorityDetails::default_for_network(4);
        let cases = vec![
            (SnPdu::ActivateDemand(ActivateDemand {
                version: 1,
                nsapi: 2,
                address: ActivateAddressDemand::Ipv4Dynamic,
                packet_data_ms_type: 0,
                pcomp_negotiation: 0,
                vj_slots: None,
                rfc2507: None,
                optional: zero.clone(),
            }), SnDirection::Uplink),
            (SnPdu::ActivateAccept(ActivateAccept {
                nsapi: 2,
                pdu_priority_max: 4,
                ready_timer: 8,
                standby_timer: 4,
                response_wait_timer: 7,
                address: ActivateAddressAccept::Ipv4Dynamic([10, 0, 0, 2]),
                pcomp_negotiation: 0,
                vj_slots: None,
                rfc2507: None,
                mtu_code: 2,
                optional: zero.clone(),
            }), SnDirection::Downlink),
            (SnPdu::ActivateReject(ActivateReject { nsapi: 2, cause: 34, optional: zero.clone() }), SnDirection::Downlink),
            (SnPdu::DeactivateDemand(Deactivate { deactivation_type: 1, nsapi: Some(2), optional: zero.clone() }), SnDirection::Uplink),
            (SnPdu::DeactivateAccept(Deactivate { deactivation_type: 1, nsapi: Some(2), optional: zero.clone() }), SnDirection::Downlink),
            (SnPdu::Unitdata(UserData {
                acknowledged: false,
                nsapi: 2,
                pcomp: 0,
                dcomp: 0,
                n_pdu: raw_octets(vec![0x45, 0x00, 0x00, 0x14]),
            }), SnDirection::Uplink),
            (SnPdu::Data(UserData {
                acknowledged: true,
                nsapi: 2,
                pcomp: 0,
                dcomp: 0,
                n_pdu: raw_octets(vec![0x45, 0x00, 0x00, 0x14]),
            }), SnDirection::Uplink),
            (SnPdu::DataTransmitRequest(DataTransmitRequest {
                nsapis: vec![2],
                logical_link_status: false,
                enhanced_service: false,
                resource_request: None,
                optional: zero.clone(),
            }), SnDirection::Uplink),
            (SnPdu::DataTransmitResponse(DataTransmitResponse {
                nsapis: vec![2],
                accepted: true,
                reject_cause: None,
                optional: zero.clone(),
            }), SnDirection::Downlink),
            (SnPdu::EndOfData(EndOfData { immediate_service_change: false, optional: zero.clone() }), SnDirection::Uplink),
            (SnPdu::Reconnect(Reconnect {
                nsapis: Vec::new(),
                enhanced_service: false,
                resource_request: None,
                optional: zero.clone(),
            }), SnDirection::Uplink),
            (SnPdu::PageRequest(PageRequest { nsapi: 2, reply_requested: true, optional: zero.clone() }), SnDirection::Downlink),
            (SnPdu::PageResponse(PageResponse {
                nsapi: 2,
                pd_service_available: true,
                logical_link_status: false,
                enhanced_service: false,
                resource_request: None,
                optional: zero.clone(),
            }), SnDirection::Uplink),
            (SnPdu::NotSupported { pdu_type: 15 }, SnDirection::Uplink),
            (SnPdu::DataPriority(DataPriority::Acknowledgement {
                accepted: true,
                details: priority,
                ms_default: Some(4),
            }), SnDirection::Downlink),
            (SnPdu::DataPriority(DataPriority::Information {
                details: priority,
                ms_default: Some(4),
            }), SnDirection::Downlink),
            (SnPdu::DataPriority(DataPriority::Request { request_type: 9 }), SnDirection::Uplink),
            (SnPdu::Modify(Modify::Request { nsapi: 2, qos: qos.clone() }), SnDirection::Uplink),
            (SnPdu::Modify(Modify::ResponseApplied { nsapi: 2, pdu_priority_max: 4, qos: qos.clone() }), SnDirection::Downlink),
            (SnPdu::Modify(Modify::ResponseRejected { nsapi: 2, cause: 26, optional: RawBits::empty() }), SnDirection::Downlink),
            (SnPdu::Modify(Modify::Availability { nsapi: 2, availability: 0, optional: RawBits::empty() }), SnDirection::Uplink),
            (SnPdu::Modify(Modify::Usage { nsapi: 2, usage: 1, optional: zero }), SnDirection::Uplink),
        ];

        for (expected, direction) in cases {
            let encoded = encode(&expected);
            assert_eq!(decode(&encoded, direction), Ok(expected));
        }
    }

    #[test]
    fn unitdata_roundtrip_preserves_npdu() {
        let pdu = SnPdu::Unitdata(UserData {
            acknowledged: false,
            nsapi: 2,
            pcomp: 0,
            dcomp: 0,
            n_pdu: raw_octets(vec![0x45, 0x00, 0x00, 0x14]),
        });
        let encoded = encode(&pdu);
        assert_eq!(encoded.clone().into_bytes(), vec![0x42, 0x00, 0x45, 0x00, 0x00, 0x14]);
        assert_eq!(decode(&encoded, SnDirection::Uplink), Ok(pdu));
    }

    #[test]
    fn data_priority_request_roundtrip() {
        let pdu = SnPdu::DataPriority(DataPriority::Request { request_type: 9 });
        let encoded = encode(&pdu);
        assert_eq!(decode(&encoded, SnDirection::Uplink), Ok(pdu));
    }

    #[test]
    fn modify_usage_roundtrip() {
        let pdu = SnPdu::Modify(Modify::Usage {
            nsapi: 2,
            usage: 1,
            optional: RawBits { bytes: vec![0], bit_len: 1 },
        });
        let encoded = encode(&pdu);
        assert_eq!(decode(&encoded, SnDirection::Uplink), Ok(pdu));
    }

    #[test]
    fn data_transmit_type4_additional_nsapis_roundtrip() {
        let pdu = SnPdu::DataTransmitRequest(DataTransmitRequest {
            nsapis: vec![2, 3, 4],
            logical_link_status: true,
            enhanced_service: false,
            resource_request: None,
            optional: RawBits::empty(),
        });
        let encoded = encode(&pdu);
        let decoded = decode(&encoded, SnDirection::Uplink).unwrap();
        let SnPdu::DataTransmitRequest(decoded) = decoded else { panic!("wrong PDU") };
        assert_eq!(decoded.nsapis, vec![2, 3, 4]);
        assert!(decoded.logical_link_status);
        assert!(!decoded.enhanced_service);
    }

    #[test]
    fn reconnect_type4_nsapis_roundtrip() {
        let pdu = SnPdu::Reconnect(Reconnect {
            nsapis: vec![
                ReconnectNsapi { nsapi: 2, data_to_send: true },
                ReconnectNsapi { nsapi: 3, data_to_send: false },
            ],
            enhanced_service: false,
            resource_request: None,
            optional: RawBits::empty(),
        });
        let encoded = encode(&pdu);
        let decoded = decode(&encoded, SnDirection::Uplink).unwrap();
        let SnPdu::Reconnect(decoded) = decoded else { panic!("wrong PDU") };
        assert_eq!(
            decoded.nsapis,
            vec![
                ReconnectNsapi { nsapi: 2, data_to_send: true },
                ReconnectNsapi { nsapi: 3, data_to_send: false },
            ]
        );
        assert!(!decoded.enhanced_service);
    }
    #[test]
    fn activation_qos_type3_roundtrip() {
        let qos = QosProfile::Negotiated {
            context_ready_timer: 8,
            asymmetrical: false,
            uplink: super::super::qos::QosSet {
                data_class: 1,
                minimum_peak_throughput: 0,
                mean_throughput: 0,
                mean_active_throughput: 0,
                delay_class: 1,
                reliability_class: 1,
            },
            downlink: None,
            filter: None,
            scheduled: None,
            additional_a: None,
            additional_b: None,
        };
        let optional = encode_optional_elements(
            &[None],
            &[Type34Element { identifier: 3, payload: qos.encode() }],
        );
        let demand = ActivateDemand {
            version: 1,
            nsapi: 2,
            address: ActivateAddressDemand::Ipv4Dynamic,
            packet_data_ms_type: 0,
            pcomp_negotiation: 0,
            vj_slots: None,
            rfc2507: None,
            optional,
        };
        assert_eq!(demand.qos().unwrap(), Some(qos));
    }

    #[test]
    fn enhanced_resource_request_roundtrip() {
        let pdu = SnPdu::DataTransmitRequest(DataTransmitRequest {
            nsapis: vec![2],
            logical_link_status: true,
            enhanced_service: true,
            resource_request: Some(PhaseModulationResourceRequest::one_slot_symmetric()),
            optional: RawBits { bytes: vec![0], bit_len: 1 },
        });
        let decoded = decode(&encode(&pdu), SnDirection::Uplink).unwrap();
        assert_eq!(decoded, pdu);
    }

}

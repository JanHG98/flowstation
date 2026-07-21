//! Structured ETSI EN 300 392-2 clause 28.4.5.31a QoS information element.
//!
//! The codec is wire-complete for the QoS IE. Runtime policy remains separate:
//! decoding a valid request does not imply that the configured cell can satisfy
//! asymmetrical, scheduled or filtered service.

use tetra_core::BitBuffer;

use super::protocol::RawBits;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QosSet {
    pub data_class: u8,
    pub minimum_peak_throughput: u8,
    pub mean_throughput: u8,
    pub mean_active_throughput: u8,
    pub delay_class: u8,
    pub reliability_class: u8,
}

impl QosSet {
    fn decode(reader: &mut BitBuffer) -> Result<Self, QosError> {
        let value = Self {
            data_class: read_u8(reader, 3, "qos.data_class")?,
            minimum_peak_throughput: read_u8(reader, 4, "qos.minimum_peak_throughput")?,
            mean_throughput: read_u8(reader, 5, "qos.mean_throughput")?,
            mean_active_throughput: read_u8(reader, 4, "qos.mean_active_throughput")?,
            delay_class: read_u8(reader, 2, "qos.delay_class")?,
            reliability_class: read_u8(reader, 2, "qos.reliability_class")?,
        };
        value.validate()?;
        Ok(value)
    }

    fn encode(&self, out: &mut BitBuffer) {
        self.validate().expect("invalid QoS set passed to encoder");
        out.write_bits(u64::from(self.data_class), 3);
        out.write_bits(u64::from(self.minimum_peak_throughput), 4);
        out.write_bits(u64::from(self.mean_throughput), 5);
        out.write_bits(u64::from(self.mean_active_throughput), 4);
        out.write_bits(u64::from(self.delay_class), 2);
        out.write_bits(u64::from(self.reliability_class), 2);
    }

    pub fn validate(&self) -> Result<(), QosError> {
        if self.data_class > 2 {
            return Err(QosError::ReservedValue { field: "qos.data_class", value: u64::from(self.data_class) });
        }
        if self.minimum_peak_throughput > 8 {
            return Err(QosError::ReservedValue {
                field: "qos.minimum_peak_throughput",
                value: u64::from(self.minimum_peak_throughput),
            });
        }
        if !(self.mean_throughput <= 18 || self.mean_throughput == 31) {
            return Err(QosError::ReservedValue { field: "qos.mean_throughput", value: u64::from(self.mean_throughput) });
        }
        if self.mean_active_throughput > 8 {
            return Err(QosError::ReservedValue {
                field: "qos.mean_active_throughput",
                value: u64::from(self.mean_active_throughput),
            });
        }
        if self.reliability_class > 2 {
            return Err(QosError::ReservedValue {
                field: "qos.reliability_class",
                value: u64::from(self.reliability_class),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QosFilter {
    pub operation: u8,
    pub filter_type: u8,
    pub first: Option<u16>,
    pub second: Option<u16>,
}

impl QosFilter {
    fn decode(reader: &mut BitBuffer) -> Result<Self, QosError> {
        let operation = read_u8(reader, 2, "qos.filter.operation")?;
        if operation > 2 {
            return Err(QosError::ReservedValue { field: "qos.filter.operation", value: u64::from(operation) });
        }
        let filter_type = read_u8(reader, 4, "qos.filter.type")?;
        let (first, second) = match filter_type {
            0 => (None, None),
            1..=3 => (Some(read_u16(reader, "qos.filter.port")?), None),
            4..=6 => (
                Some(read_u16(reader, "qos.filter.port_low")?),
                Some(read_u16(reader, "qos.filter.port_high")?),
            ),
            7 => (Some(read_u16(reader, "qos.filter.diffserv")?), None),
            8..=11 => (Some(read_u16(reader, "qos.filter.reserved_a")?), None),
            12..=15 => (
                Some(read_u16(reader, "qos.filter.reserved_a")?),
                Some(read_u16(reader, "qos.filter.reserved_b")?),
            ),
            _ => unreachable!(),
        };
        Ok(Self { operation, filter_type, first, second })
    }

    fn encode(&self, out: &mut BitBuffer) {
        self.validate().expect("invalid QoS filter passed to encoder");
        out.write_bits(u64::from(self.operation), 2);
        out.write_bits(u64::from(self.filter_type & 0x0f), 4);
        match self.filter_type {
            0 => {}
            1..=3 | 7 | 8..=11 => out.write_bits(u64::from(self.first.unwrap_or(0)), 16),
            4..=6 | 12..=15 => {
                out.write_bits(u64::from(self.first.unwrap_or(0)), 16);
                out.write_bits(u64::from(self.second.unwrap_or(0)), 16);
            }
            _ => unreachable!(),
        }
    }

    pub fn validate(&self) -> Result<(), QosError> {
        if self.operation > 2 {
            return Err(QosError::ReservedValue { field: "qos.filter.operation", value: u64::from(self.operation) });
        }
        match self.filter_type {
            0 if self.first.is_none() && self.second.is_none() => Ok(()),
            1..=3 | 7 | 8..=11 if self.first.is_some() && self.second.is_none() => Ok(()),
            4..=6 | 12..=15 if self.first.is_some() && self.second.is_some() => Ok(()),
            0..=15 => Err(QosError::InvalidCombination("qos.filter payload does not match filter type")),
            _ => Err(QosError::ReservedValue { field: "qos.filter.type", value: u64::from(self.filter_type) }),
        }
    }

    pub fn is_automatic(&self) -> bool {
        self.filter_type == 0
    }

    pub fn is_reserved_type(&self) -> bool {
        self.filter_type >= 8
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledAccess {
    pub repetition_period_slots: u16,
    pub timing_error: u8,
    pub pdu_sizes_octets: Vec<u16>,
}

impl ScheduledAccess {
    fn decode(reader: &mut BitBuffer) -> Result<Self, QosError> {
        let repetition_period_slots = read_u16_bits(reader, 10, "qos.schedule.repetition_period")?;
        if !(4..=706).contains(&repetition_period_slots) {
            return Err(QosError::ReservedValue {
                field: "qos.schedule.repetition_period",
                value: u64::from(repetition_period_slots),
            });
        }
        let timing_error = read_u8(reader, 3, "qos.schedule.timing_error")?;
        let count = read_u8(reader, 3, "qos.schedule.pdu_count")?;
        if count == 0 {
            return Err(QosError::ReservedValue { field: "qos.schedule.pdu_count", value: 0 });
        }
        let mut pdu_sizes_octets = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let size = read_u16_bits(reader, 12, "qos.schedule.pdu_size")?;
            if !(1..=2002).contains(&size) {
                return Err(QosError::ReservedValue { field: "qos.schedule.pdu_size", value: u64::from(size) });
            }
            pdu_sizes_octets.push(size);
        }
        Ok(Self { repetition_period_slots, timing_error, pdu_sizes_octets })
    }

    fn encode(&self, out: &mut BitBuffer) {
        self.validate().expect("invalid scheduled-access request passed to encoder");
        out.write_bits(u64::from(self.repetition_period_slots), 10);
        out.write_bits(u64::from(self.timing_error & 7), 3);
        out.write_bits(self.pdu_sizes_octets.len().min(7) as u64, 3);
        for size in self.pdu_sizes_octets.iter().copied().take(7) {
            out.write_bits(u64::from(size), 12);
        }
    }

    pub fn validate(&self) -> Result<(), QosError> {
        if !(4..=706).contains(&self.repetition_period_slots) {
            return Err(QosError::ReservedValue {
                field: "qos.schedule.repetition_period",
                value: u64::from(self.repetition_period_slots),
            });
        }
        if self.timing_error > 7 {
            return Err(QosError::ReservedValue { field: "qos.schedule.timing_error", value: u64::from(self.timing_error) });
        }
        if self.pdu_sizes_octets.is_empty() || self.pdu_sizes_octets.len() > 7 {
            return Err(QosError::InvalidCombination("qos.schedule requires 1..=7 PDU sizes"));
        }
        if let Some(size) = self.pdu_sizes_octets.iter().copied().find(|size| !(1..=2002).contains(size)) {
            return Err(QosError::ReservedValue { field: "qos.schedule.pdu_size", value: u64::from(size) });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QosProfile {
    Background,
    Negotiated {
        context_ready_timer: u8,
        asymmetrical: bool,
        uplink: QosSet,
        downlink: Option<QosSet>,
        filter: Option<QosFilter>,
        scheduled: Option<ScheduledAccess>,
        additional_a: Option<u16>,
        additional_b: Option<u16>,
    },
}

impl Default for QosProfile {
    fn default() -> Self {
        Self::Background
    }
}

impl QosProfile {
    pub fn decode(raw: &RawBits) -> Result<Self, QosError> {
        let mut reader = raw.reader();
        let background = read_u8(&mut reader, 1, "qos.background_class_request")?;
        if background == 0 {
            ensure_consumed(&reader)?;
            return Ok(Self::Background);
        }
        let context_ready_timer = read_u8(&mut reader, 4, "qos.context_ready_timer")?;
        if context_ready_timer == 15 {
            return Err(QosError::ReservedValue { field: "qos.context_ready_timer", value: 15 });
        }
        let asymmetrical = read_u8(&mut reader, 1, "qos.asymmetrical")? != 0;
        let uplink = QosSet::decode(&mut reader)?;
        let downlink = if asymmetrical { Some(QosSet::decode(&mut reader)?) } else { None };
        let filter = if read_u8(&mut reader, 1, "qos.filter_included")? != 0 {
            Some(QosFilter::decode(&mut reader)?)
        } else {
            None
        };
        let scheduled = if read_u8(&mut reader, 1, "qos.scheduled_included")? != 0 {
            Some(ScheduledAccess::decode(&mut reader)?)
        } else {
            None
        };
        let additional_a = if read_u8(&mut reader, 1, "qos.additional_a_included")? != 0 {
            Some(read_u16(&mut reader, "qos.additional_a")?)
        } else {
            None
        };
        let additional_b = if read_u8(&mut reader, 1, "qos.additional_b_included")? != 0 {
            Some(read_u16(&mut reader, "qos.additional_b")?)
        } else {
            None
        };
        ensure_consumed(&reader)?;
        Ok(Self::Negotiated {
            context_ready_timer,
            asymmetrical,
            uplink,
            downlink,
            filter,
            scheduled,
            additional_a,
            additional_b,
        })
    }

    pub fn encode(&self) -> RawBits {
        self.validate().expect("invalid QoS profile passed to encoder");
        let mut out = BitBuffer::new_autoexpand(96);
        match self {
            Self::Background => out.write_bits(0, 1),
            Self::Negotiated {
                context_ready_timer,
                asymmetrical,
                uplink,
                downlink,
                filter,
                scheduled,
                additional_a,
                additional_b,
            } => {
                out.write_bits(1, 1);
                out.write_bits(u64::from(*context_ready_timer & 0x0f), 4);
                out.write_bits(*asymmetrical as u64, 1);
                uplink.encode(&mut out);
                if *asymmetrical {
                    downlink.as_ref().copied().unwrap_or(*uplink).encode(&mut out);
                }
                out.write_bits(filter.is_some() as u64, 1);
                if let Some(filter) = filter {
                    filter.encode(&mut out);
                }
                out.write_bits(scheduled.is_some() as u64, 1);
                if let Some(schedule) = scheduled {
                    schedule.encode(&mut out);
                }
                out.write_bits(additional_a.is_some() as u64, 1);
                if let Some(value) = additional_a {
                    out.write_bits(u64::from(*value), 16);
                }
                out.write_bits(additional_b.is_some() as u64, 1);
                if let Some(value) = additional_b {
                    out.write_bits(u64::from(*value), 16);
                }
            }
        }
        let bit_len = out.get_pos();
        out.seek(0);
        RawBits::from_reader_exact(&mut out, bit_len).expect("locally encoded QoS must be readable")
    }

    pub fn validate(&self) -> Result<(), QosError> {
        match self {
            Self::Background => Ok(()),
            Self::Negotiated {
                context_ready_timer,
                asymmetrical,
                uplink,
                downlink,
                filter,
                scheduled,
                ..
            } => {
                if *context_ready_timer == 15 {
                    return Err(QosError::ReservedValue { field: "qos.context_ready_timer", value: 15 });
                }
                uplink.validate()?;
                match (*asymmetrical, downlink.as_ref()) {
                    (true, Some(downlink)) => downlink.validate()?,
                    (true, None) => return Err(QosError::InvalidCombination("asymmetrical QoS requires a downlink set")),
                    (false, Some(_)) => return Err(QosError::InvalidCombination("symmetric QoS must not contain a downlink set")),
                    (false, None) => {}
                }
                if let Some(filter) = filter {
                    filter.validate()?;
                }
                if let Some(schedule) = scheduled {
                    schedule.validate()?;
                }
                Ok(())
            }
        }
    }

    pub fn context_ready_timer(&self) -> u8 {
        match self {
            Self::Background => 0,
            Self::Negotiated { context_ready_timer, .. } => *context_ready_timer,
        }
    }

    pub fn filter(&self) -> Option<QosFilter> {
        match self {
            Self::Background => None,
            Self::Negotiated { filter, .. } => *filter,
        }
    }

    pub fn has_schedule(&self) -> bool {
        matches!(self, Self::Negotiated { scheduled: Some(_), .. })
    }

    pub fn is_asymmetrical(&self) -> bool {
        matches!(self, Self::Negotiated { asymmetrical: true, .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QosError {
    TooShort(&'static str),
    ReservedValue { field: &'static str, value: u64 },
    InvalidCombination(&'static str),
    TrailingBits(usize),
}

fn read_u8(reader: &mut BitBuffer, bits: usize, field: &'static str) -> Result<u8, QosError> {
    reader.read_bits(bits).map(|value| value as u8).ok_or(QosError::TooShort(field))
}

fn read_u16(reader: &mut BitBuffer, field: &'static str) -> Result<u16, QosError> {
    read_u16_bits(reader, 16, field)
}

fn read_u16_bits(reader: &mut BitBuffer, bits: usize, field: &'static str) -> Result<u16, QosError> {
    reader.read_bits(bits).map(|value| value as u16).ok_or(QosError::TooShort(field))
}

fn ensure_consumed(reader: &BitBuffer) -> Result<(), QosError> {
    let remaining = reader.get_len_remaining();
    if remaining == 0 { Ok(()) } else { Err(QosError::TrailingBits(remaining)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set() -> QosSet {
        QosSet {
            data_class: 1,
            minimum_peak_throughput: 2,
            mean_throughput: 8,
            mean_active_throughput: 3,
            delay_class: 1,
            reliability_class: 2,
        }
    }

    #[test]
    fn background_round_trips_as_one_bit() {
        let raw = QosProfile::Background.encode();
        assert_eq!(raw.bit_len, 1);
        assert_eq!(QosProfile::decode(&raw).unwrap(), QosProfile::Background);
    }

    #[test]
    fn complete_qos_round_trips() {
        let qos = QosProfile::Negotiated {
            context_ready_timer: 8,
            asymmetrical: true,
            uplink: set(),
            downlink: Some(QosSet { data_class: 2, ..set() }),
            filter: Some(QosFilter { operation: 1, filter_type: 5, first: Some(9200), second: Some(9201) }),
            scheduled: Some(ScheduledAccess {
                repetition_period_slots: 100,
                timing_error: 2,
                pdu_sizes_octets: vec![100, 200],
            }),
            additional_a: Some(0x1234),
            additional_b: None,
        };
        let raw = qos.encode();
        assert_eq!(QosProfile::decode(&raw).unwrap(), qos);
    }
}

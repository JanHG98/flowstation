//! Phase-modulation resource request information element (table 28.115).

use tetra_core::BitBuffer;

use super::protocol::RawBits;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhaseModulationResourceRequest {
    pub asymmetric: bool,
    pub mean_throughput: u8,
    /// ETSI wire code: 0..3 means 1..4 uplink (or symmetric) slots.
    pub uplink_slots_code: u8,
    /// Present only for asymmetric requests; wire code 0..3 means 1..4 slots.
    pub downlink_slots_code: Option<u8>,
    /// ETSI wire code: 0..3 means a full phase-modulation capability of 1..4 slots.
    pub full_capability_code: u8,
}

impl PhaseModulationResourceRequest {
    pub fn one_slot_symmetric() -> Self {
        Self {
            asymmetric: false,
            mean_throughput: 0,
            uplink_slots_code: 0,
            downlink_slots_code: None,
            full_capability_code: 0,
        }
    }

    pub fn uplink_slots(self) -> u8 {
        self.uplink_slots_code + 1
    }

    pub fn downlink_slots(self) -> u8 {
        self.downlink_slots_code.unwrap_or(self.uplink_slots_code) + 1
    }

    pub fn full_capability_slots(self) -> u8 {
        self.full_capability_code + 1
    }

    pub fn validate(self) -> Result<Self, ResourceError> {
        if self.mean_throughput > 7 {
            return Err(ResourceError::ReservedValue { field: "resource.mean_throughput", value: u64::from(self.mean_throughput) });
        }
        if self.uplink_slots_code > 3 {
            return Err(ResourceError::ReservedValue { field: "resource.uplink_slots", value: u64::from(self.uplink_slots_code) });
        }
        if self.full_capability_code > 3 {
            return Err(ResourceError::ReservedValue {
                field: "resource.full_capability",
                value: u64::from(self.full_capability_code),
            });
        }
        if self.asymmetric {
            let Some(code) = self.downlink_slots_code else {
                return Err(ResourceError::MissingField("resource.downlink_slots"));
            };
            if code > 3 {
                return Err(ResourceError::ReservedValue { field: "resource.downlink_slots", value: u64::from(code) });
            }
        }
        if self.mean_throughput == 6
            && (self.asymmetric || self.uplink_slots_code != self.full_capability_code)
        {
            return Err(ResourceError::InvalidCombination("unspecified throughput requires symmetric full capability"));
        }
        Ok(self)
    }

    pub fn decode_prefix(raw: &RawBits) -> Result<(Self, RawBits), ResourceError> {
        let mut reader = raw.reader();
        let asymmetric = read(&mut reader, 1, "resource.symmetry")? != 0;
        let mean_throughput = read(&mut reader, 3, "resource.mean_throughput")? as u8;
        let uplink_slots_code = read(&mut reader, 2, "resource.uplink_slots")? as u8;
        let downlink_slots_code = if asymmetric {
            Some(read(&mut reader, 2, "resource.downlink_slots")? as u8)
        } else {
            None
        };
        let full_capability_code = read(&mut reader, 2, "resource.full_capability")? as u8;
        let reserved = read(&mut reader, 2, "resource.reserved")? as u8;
        if reserved != 3 {
            return Err(ResourceError::ReservedValue { field: "resource.reserved", value: u64::from(reserved) });
        }
        let request = Self {
            asymmetric,
            mean_throughput,
            uplink_slots_code,
            downlink_slots_code,
            full_capability_code,
        }
        .validate()?;
        Ok((request, RawBits::from_remaining(&reader)))
    }

    pub fn encode(&self, out: &mut BitBuffer) {
        self.validate().expect("invalid phase-modulation resource request passed to encoder");
        out.write_bits(self.asymmetric as u64, 1);
        out.write_bits(u64::from(self.mean_throughput & 7), 3);
        out.write_bits(u64::from(self.uplink_slots_code & 3), 2);
        if self.asymmetric {
            out.write_bits(u64::from(self.downlink_slots_code.unwrap_or(0) & 3), 2);
        }
        out.write_bits(u64::from(self.full_capability_code & 3), 2);
        out.write_bits(3, 2);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceError {
    TooShort(&'static str),
    MissingField(&'static str),
    ReservedValue { field: &'static str, value: u64 },
    InvalidCombination(&'static str),
}

fn read(reader: &mut BitBuffer, bits: usize, field: &'static str) -> Result<u64, ResourceError> {
    reader.read_bits(bits).ok_or(ResourceError::TooShort(field))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symmetric_resource_round_trips() {
        let request = PhaseModulationResourceRequest::one_slot_symmetric();
        let mut bits = BitBuffer::new_autoexpand(16);
        request.encode(&mut bits);
        bits.write_bits(0, 1); // O-bit: no optional fields
        let len = bits.get_pos();
        bits.seek(0);
        let raw = RawBits::from_reader_exact(&mut bits, len).unwrap();
        let (decoded, optional) = PhaseModulationResourceRequest::decode_prefix(&raw).unwrap();
        assert_eq!(decoded, request);
        assert_eq!(optional.bit_string(), "0");
    }
}

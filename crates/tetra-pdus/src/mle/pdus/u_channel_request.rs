use core::fmt;

use tetra_core::typed_pdu_fields::delimiters;
use tetra_core::{BitBuffer, expect_pdu_type, pdu_parse_error::PduParseErr};
use tetra_saps::common::MleChannelRequestReason;

use crate::mle::enums::mle_pdu_type_ul::MlePduTypeUl;

/// U-CHANNEL-REQUEST PDU (ETSI EN 300 392-2, clause 18.4.1.4.9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UChannelRequest {
    pub reason_for_the_channel_request: MleChannelRequestReason,
    pub requested_channel_class_identifiers: Vec<u8>,
    pub requested_channel_identifiers: Vec<u8>,
    pub reserved: Option<u8>,
}

impl UChannelRequest {
    pub fn from_bitbuf(buffer: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let pdu_type = buffer.read_field(3, "pdu_type")?;
        expect_pdu_type!(pdu_type, MlePduTypeUl::UChannelRequest)?;

        let reason_for_the_channel_request = MleChannelRequestReason::from_raw(
            buffer.read_field(3, "reason_for_the_channel_request")? as u8,
        );
        let obit = delimiters::read_obit(buffer)?;

        let requested_channel_class_identifiers = if obit && delimiters::read_pbit(buffer)? {
            let count = buffer
                .read_field(3, "number_of_requested_channel_class_identifiers")?
                as usize;
            let mut values = Vec::with_capacity(count);
            for _ in 0..count {
                values.push(buffer.read_field(4, "channel_class_identifier")? as u8);
            }
            values
        } else {
            Vec::new()
        };

        let requested_channel_identifiers = if obit && delimiters::read_pbit(buffer)? {
            let count = buffer
                .read_field(3, "number_of_requested_channel_identifiers")?
                as usize;
            let mut values = Vec::with_capacity(count);
            for _ in 0..count {
                values.push(buffer.read_field(5, "channel_identifier")? as u8);
            }
            values
        } else {
            Vec::new()
        };

        let reserved = if obit && delimiters::read_pbit(buffer)? {
            Some(buffer.read_field(6, "reserved")? as u8)
        } else {
            None
        };

        if obit && delimiters::read_mbit(buffer)? {
            return Err(PduParseErr::InvalidTrailingMbitValue);
        }

        Ok(Self {
            reason_for_the_channel_request,
            requested_channel_class_identifiers,
            requested_channel_identifiers,
            reserved,
        })
    }

    pub fn to_bitbuf(&self, buffer: &mut BitBuffer) -> Result<(), PduParseErr> {
        if self.requested_channel_class_identifiers.len() > 7 {
            return Err(PduParseErr::InvalidValue {
                field: "number_of_requested_channel_class_identifiers",
                value: self.requested_channel_class_identifiers.len() as u64,
            });
        }
        if self.requested_channel_identifiers.len() > 7 {
            return Err(PduParseErr::InvalidValue {
                field: "number_of_requested_channel_identifiers",
                value: self.requested_channel_identifiers.len() as u64,
            });
        }
        if self
            .requested_channel_class_identifiers
            .iter()
            .any(|value| *value > 0b1111)
        {
            return Err(PduParseErr::InvalidValue {
                field: "channel_class_identifier",
                value: 16,
            });
        }
        if self
            .requested_channel_identifiers
            .iter()
            .any(|value| *value > 0b1_1111)
        {
            return Err(PduParseErr::InvalidValue {
                field: "channel_identifier",
                value: 32,
            });
        }
        if let Some(value) = self.reserved
            && value > 0b11_1111
        {
            return Err(PduParseErr::InvalidValue {
                field: "reserved",
                value: value as u64,
            });
        }

        buffer.write_bits(MlePduTypeUl::UChannelRequest.into_raw(), 3);
        buffer.write_bits(self.reason_for_the_channel_request.into_raw() as u64, 3);

        let obit = !self.requested_channel_class_identifiers.is_empty()
            || !self.requested_channel_identifiers.is_empty()
            || self.reserved.is_some();
        delimiters::write_obit(buffer, obit as u8);
        if !obit {
            return Ok(());
        }

        delimiters::write_pbit(
            buffer,
            (!self.requested_channel_class_identifiers.is_empty()) as u8,
        );
        if !self.requested_channel_class_identifiers.is_empty() {
            buffer.write_bits(self.requested_channel_class_identifiers.len() as u64, 3);
            for value in &self.requested_channel_class_identifiers {
                buffer.write_bits(u64::from(*value), 4);
            }
        }

        delimiters::write_pbit(
            buffer,
            (!self.requested_channel_identifiers.is_empty()) as u8,
        );
        if !self.requested_channel_identifiers.is_empty() {
            buffer.write_bits(self.requested_channel_identifiers.len() as u64, 3);
            for value in &self.requested_channel_identifiers {
                buffer.write_bits(u64::from(*value), 5);
            }
        }

        delimiters::write_pbit(buffer, self.reserved.is_some() as u8);
        if let Some(value) = self.reserved {
            buffer.write_bits(u64::from(value), 6);
        }
        delimiters::write_mbit(buffer, 0);
        Ok(())
    }
}

impl fmt::Display for UChannelRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "UChannelRequest {{ reason: {:?}, classes: {:?}, channels: {:?} }}",
            self.reason_for_the_channel_request,
            self.requested_channel_class_identifiers,
            self.requested_channel_identifiers,
        )
    }
}

use core::fmt;

use tetra_core::typed_pdu_fields::delimiters;
use tetra_core::{BitBuffer, expect_pdu_type, pdu_parse_error::PduParseErr};

use crate::mle::enums::mle_pdu_type_dl::MlePduTypeDl;
use crate::mle::pdus::trailing_sdu::{read_trailing_sdu, write_trailing_sdu};

/// D-RESTORE-ACK PDU (ETSI EN 300 392-2, clause 18.4.1.4.4).
#[derive(Debug, Clone)]
pub struct DRestoreAck {
    /// Embedded CMCE D-CALL RESTORE PDU.
    pub sdu: Option<BitBuffer>,
}

impl DRestoreAck {
    pub fn from_bitbuf(buffer: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let pdu_type = buffer.read_field(3, "pdu_type")?;
        expect_pdu_type!(pdu_type, MlePduTypeDl::DRestoreAck)?;

        let obit = delimiters::read_obit(buffer)?;
        if obit {
            return Err(PduParseErr::InvalidValue {
                field: "d_restore_ack_obit",
                value: 1,
            });
        }

        Ok(Self {
            sdu: read_trailing_sdu(buffer),
        })
    }

    pub fn to_bitbuf(&self, buffer: &mut BitBuffer) -> Result<(), PduParseErr> {
        buffer.write_bits(MlePduTypeDl::DRestoreAck.into_raw(), 3);
        delimiters::write_obit(buffer, 0);
        write_trailing_sdu(buffer, &self.sdu);
        Ok(())
    }
}

impl fmt::Display for DRestoreAck {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "DRestoreAck {{ sdu_bits: {} }}",
            self.sdu.as_ref().map_or(0, BitBuffer::get_len),
        )
    }
}

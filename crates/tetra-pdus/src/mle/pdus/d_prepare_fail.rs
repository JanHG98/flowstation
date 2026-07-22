use core::fmt;

use tetra_core::typed_pdu_fields::delimiters;
use tetra_core::{BitBuffer, expect_pdu_type, pdu_parse_error::PduParseErr};
use tetra_saps::common::MleFailCause;

use crate::mle::enums::mle_pdu_type_dl::MlePduTypeDl;
use crate::mle::pdus::trailing_sdu::{read_trailing_sdu, write_trailing_sdu};

/// D-PREPARE-FAIL PDU (ETSI EN 300 392-2, clause 18.4.1.4.3).
#[derive(Debug, Clone)]
pub struct DPrepareFail {
    pub fail_cause: MleFailCause,
    pub sdu: Option<BitBuffer>,
}

impl DPrepareFail {
    pub fn from_bitbuf(buffer: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let pdu_type = buffer.read_field(3, "pdu_type")?;
        expect_pdu_type!(pdu_type, MlePduTypeDl::DPrepareFail)?;

        let fail_cause = MleFailCause::from_raw(buffer.read_field(2, "fail_cause")? as u8);
        let obit = delimiters::read_obit(buffer)?;
        if obit {
            return Err(PduParseErr::InvalidValue {
                field: "d_prepare_fail_obit",
                value: 1,
            });
        }

        Ok(Self {
            fail_cause,
            sdu: read_trailing_sdu(buffer),
        })
    }

    pub fn to_bitbuf(&self, buffer: &mut BitBuffer) -> Result<(), PduParseErr> {
        buffer.write_bits(MlePduTypeDl::DPrepareFail.into_raw(), 3);
        buffer.write_bits(self.fail_cause.into_raw() as u64, 2);
        delimiters::write_obit(buffer, 0);
        write_trailing_sdu(buffer, &self.sdu);
        Ok(())
    }
}

impl fmt::Display for DPrepareFail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "DPrepareFail {{ fail_cause: {:?}, sdu_bits: {} }}",
            self.fail_cause,
            self.sdu.as_ref().map_or(0, BitBuffer::get_len),
        )
    }
}

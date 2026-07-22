use core::fmt;

use tetra_core::typed_pdu_fields::delimiters;
use tetra_core::{BitBuffer, expect_pdu_type, pdu_parse_error::PduParseErr};
use tetra_saps::common::MleFailCause;

use crate::mle::enums::mle_pdu_type_dl::MlePduTypeDl;

/// D-RESTORE-FAIL PDU (ETSI EN 300 392-2, clause 18.4.1.4.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DRestoreFail {
    pub fail_cause: MleFailCause,
}

impl DRestoreFail {
    pub fn from_bitbuf(buffer: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let pdu_type = buffer.read_field(3, "pdu_type")?;
        expect_pdu_type!(pdu_type, MlePduTypeDl::DRestoreFail)?;

        let fail_cause = MleFailCause::from_raw(buffer.read_field(2, "fail_cause")? as u8);
        let obit = delimiters::read_obit(buffer)?;
        if obit {
            return Err(PduParseErr::InvalidValue {
                field: "d_restore_fail_obit",
                value: 1,
            });
        }

        Ok(Self { fail_cause })
    }

    pub fn to_bitbuf(&self, buffer: &mut BitBuffer) -> Result<(), PduParseErr> {
        buffer.write_bits(MlePduTypeDl::DRestoreFail.into_raw(), 3);
        buffer.write_bits(self.fail_cause.into_raw() as u64, 2);
        delimiters::write_obit(buffer, 0);
        Ok(())
    }
}

impl fmt::Display for DRestoreFail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DRestoreFail {{ fail_cause: {:?} }}", self.fail_cause)
    }
}

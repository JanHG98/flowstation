use core::fmt;

use tetra_core::typed_pdu_fields::{delimiters, typed};
use tetra_core::{BitBuffer, expect_pdu_type, pdu_parse_error::PduParseErr};

use crate::mle::enums::mle_pdu_type_ul::MlePduTypeUl;
use crate::mle::pdus::trailing_sdu::{read_trailing_sdu, write_trailing_sdu};

/// U-PREPARE PDU (ETSI EN 300 392-2, clause 18.4.1.4.6).
#[derive(Debug, Clone)]
pub struct UPrepare {
    pub cell_identifier_ca: Option<u8>,
    /// Optional embedded MM/OTAR PDU.
    pub sdu: Option<BitBuffer>,
}

impl UPrepare {
    pub fn from_bitbuf(buffer: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let pdu_type = buffer.read_field(3, "pdu_type")?;
        expect_pdu_type!(pdu_type, MlePduTypeUl::UPrepare)?;

        let obit = delimiters::read_obit(buffer)?;
        let cell_identifier_ca = typed::parse_type2_generic(
            obit,
            buffer,
            5,
            "cell_identifier_ca",
        )?
        .map(|value| value as u8);

        Ok(Self {
            cell_identifier_ca,
            sdu: read_trailing_sdu(buffer),
        })
    }

    pub fn to_bitbuf(&self, buffer: &mut BitBuffer) -> Result<(), PduParseErr> {
        if let Some(value) = self.cell_identifier_ca
            && value > 0b1_1111
        {
            return Err(PduParseErr::InvalidValue {
                field: "cell_identifier_ca",
                value: value as u64,
            });
        }

        buffer.write_bits(MlePduTypeUl::UPrepare.into_raw(), 3);
        let obit = self.cell_identifier_ca.is_some();
        delimiters::write_obit(buffer, obit as u8);
        typed::write_type2_generic(
            obit,
            buffer,
            self.cell_identifier_ca.map(u64::from),
            5,
        );
        write_trailing_sdu(buffer, &self.sdu);
        Ok(())
    }
}

impl fmt::Display for UPrepare {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "UPrepare {{ cell_identifier_ca: {:?}, sdu_bits: {} }}",
            self.cell_identifier_ca,
            self.sdu.as_ref().map_or(0, BitBuffer::get_len),
        )
    }
}

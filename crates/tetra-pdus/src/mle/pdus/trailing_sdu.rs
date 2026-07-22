use tetra_core::BitBuffer;

/// Copy every bit remaining in `buffer` into a standalone SDU.
///
/// MLE cell-change PDUs carry their MM/CMCE SDU directly after the PDU's
/// O-/P-bit section. There is no synthetic P-bit or length field before the
/// embedded PDU, so the only reliable boundary is the enclosing LLC SDU.
pub(crate) fn read_trailing_sdu(buffer: &mut BitBuffer) -> Option<BitBuffer> {
    if buffer.get_len_remaining() == 0 {
        return None;
    }

    let sdu = BitBuffer::from_bitbuffer_pos(buffer);
    buffer.seek(buffer.get_len());
    Some(sdu)
}

/// Append a complete nested MM/CMCE PDU without adding delimiters.
pub(crate) fn write_trailing_sdu(buffer: &mut BitBuffer, sdu: &Option<BitBuffer>) {
    let Some(sdu) = sdu else {
        return;
    };

    let mut copy = BitBuffer::from_bitbuffer(sdu);
    let len = copy.get_len();
    buffer.copy_bits(&mut copy, len);
}

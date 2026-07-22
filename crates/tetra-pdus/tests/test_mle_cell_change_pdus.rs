use tetra_core::BitBuffer;
use tetra_pdus::mle::pdus::d_channel_response::DChannelResponse;
use tetra_pdus::mle::pdus::d_new_cell::DNewCell;
use tetra_pdus::mle::pdus::d_prepare_fail::DPrepareFail;
use tetra_pdus::mle::pdus::d_restore_ack::DRestoreAck;
use tetra_pdus::mle::pdus::d_restore_fail::DRestoreFail;
use tetra_pdus::mle::pdus::u_channel_request::UChannelRequest;
use tetra_pdus::mle::pdus::u_prepare::UPrepare;
use tetra_pdus::mle::pdus::u_restore::URestore;
use tetra_saps::common::{
    MleChannelCommandValid, MleChannelRequestReason, MleChannelRequestRetryDelay,
    MleChannelResponseType, MleFailCause,
};

fn nested_sdu() -> BitBuffer {
    BitBuffer::from_bitstr(
        "10100101101001011010010110100101101001011010010110100101101001011010010110100101",
    )
}

#[test]
fn downlink_baseline_vectors_match_the_normative_field_order() {
    let mut new_cell = BitBuffer::new_autoexpand(16);
    DNewCell {
        channel_command_valid: MleChannelCommandValid::NoChannelChange,
        sdu: None,
    }
    .to_bitbuf(&mut new_cell)
    .unwrap();
    assert_eq!(new_cell.dump_bin_unformatted(), "000100");

    let mut prepare_fail = BitBuffer::new_autoexpand(16);
    DPrepareFail {
        fail_cause: MleFailCause::MsNotAllowedOnCell,
        sdu: None,
    }
    .to_bitbuf(&mut prepare_fail)
    .unwrap();
    assert_eq!(prepare_fail.dump_bin_unformatted(), "001100");

    let mut restore_ack = BitBuffer::new_autoexpand(16);
    DRestoreAck { sdu: None }
        .to_bitbuf(&mut restore_ack)
        .unwrap();
    assert_eq!(restore_ack.dump_bin_unformatted(), "1000");

    let mut restore_fail = BitBuffer::new_autoexpand(16);
    DRestoreFail {
        fail_cause: MleFailCause::RestorationCannotBeDoneOnCell,
    }
    .to_bitbuf(&mut restore_fail)
    .unwrap();
    assert_eq!(restore_fail.dump_bin_unformatted(), "101110");

    let mut channel_response = BitBuffer::new_autoexpand(24);
    DChannelResponse {
        channel_response_type: MleChannelResponseType::Accepted,
        reason_for_the_channel_request: MleChannelRequestReason::CurrentChannelRadioImprovable,
        channel_request_retry_delay: MleChannelRequestRetryDelay::NoDelay,
        reserved1: None,
        reserved2: None,
    }
    .to_bitbuf(&mut channel_response)
    .unwrap();
    assert_eq!(channel_response.dump_bin_unformatted(), "110001000000");
}

#[test]
fn cell_change_pdus_roundtrip_and_preserve_long_embedded_sdus() {
    let nested = nested_sdu();

    let mut encoded = BitBuffer::new_autoexpand(128);
    UPrepare {
        cell_identifier_ca: Some(17),
        sdu: Some(nested.clone()),
    }
    .to_bitbuf(&mut encoded)
    .unwrap();
    encoded.seek(0);
    let decoded = UPrepare::from_bitbuf(&mut encoded).unwrap();
    assert_eq!(decoded.cell_identifier_ca, Some(17));
    assert_eq!(
        decoded.sdu.unwrap().dump_bin_unformatted(),
        nested.dump_bin_unformatted()
    );

    let mut encoded = BitBuffer::new_autoexpand(128);
    URestore {
        mcc: Some(262),
        mnc: Some(1),
        la: Some(42),
        sdu: Some(nested.clone()),
    }
    .to_bitbuf(&mut encoded)
    .unwrap();
    encoded.seek(0);
    let decoded = URestore::from_bitbuf(&mut encoded).unwrap();
    assert_eq!(decoded.mcc, Some(262));
    assert_eq!(decoded.mnc, Some(1));
    assert_eq!(decoded.la, Some(42));
    assert_eq!(
        decoded.sdu.unwrap().dump_bin_unformatted(),
        nested.dump_bin_unformatted()
    );

    let mut encoded = BitBuffer::new_autoexpand(128);
    DNewCell {
        channel_command_valid: MleChannelCommandValid::ChangeChannelImmediately,
        sdu: Some(nested.clone()),
    }
    .to_bitbuf(&mut encoded)
    .unwrap();
    encoded.seek(0);
    let decoded = DNewCell::from_bitbuf(&mut encoded).unwrap();
    assert_eq!(
        decoded.channel_command_valid,
        MleChannelCommandValid::ChangeChannelImmediately
    );
    assert_eq!(
        decoded.sdu.unwrap().dump_bin_unformatted(),
        nested.dump_bin_unformatted()
    );

    let mut encoded = BitBuffer::new_autoexpand(128);
    DPrepareFail {
        fail_cause: MleFailCause::CellReselectionTypeNotSupported,
        sdu: Some(nested.clone()),
    }
    .to_bitbuf(&mut encoded)
    .unwrap();
    encoded.seek(0);
    let decoded = DPrepareFail::from_bitbuf(&mut encoded).unwrap();
    assert_eq!(
        decoded.fail_cause,
        MleFailCause::CellReselectionTypeNotSupported
    );
    assert_eq!(
        decoded.sdu.unwrap().dump_bin_unformatted(),
        nested.dump_bin_unformatted()
    );

    let mut encoded = BitBuffer::new_autoexpand(128);
    DRestoreAck {
        sdu: Some(nested.clone()),
    }
    .to_bitbuf(&mut encoded)
    .unwrap();
    encoded.seek(0);
    let decoded = DRestoreAck::from_bitbuf(&mut encoded).unwrap();
    assert_eq!(
        decoded.sdu.unwrap().dump_bin_unformatted(),
        nested.dump_bin_unformatted()
    );
}

#[test]
fn channel_request_and_response_roundtrip() {
    let original = UChannelRequest {
        reason_for_the_channel_request: MleChannelRequestReason::HigherLevelOfServiceRequested,
        requested_channel_class_identifiers: vec![1, 7, 15],
        requested_channel_identifiers: vec![2, 8, 31],
        reserved: Some(12),
    };
    let mut encoded = BitBuffer::new_autoexpand(64);
    original.to_bitbuf(&mut encoded).unwrap();
    encoded.seek(0);
    let decoded = UChannelRequest::from_bitbuf(&mut encoded).unwrap();
    assert_eq!(decoded, original);

    let original = DChannelResponse {
        channel_response_type: MleChannelResponseType::Rejected,
        reason_for_the_channel_request: MleChannelRequestReason::HigherLevelOfServiceRequested,
        channel_request_retry_delay: MleChannelRequestRetryDelay::Seconds20,
        reserved1: Some(0x12),
        reserved2: Some(0x34),
    };
    let mut encoded = BitBuffer::new_autoexpand(64);
    original.to_bitbuf(&mut encoded).unwrap();
    encoded.seek(0);
    let decoded = DChannelResponse::from_bitbuf(&mut encoded).unwrap();
    assert_eq!(decoded, original);
}

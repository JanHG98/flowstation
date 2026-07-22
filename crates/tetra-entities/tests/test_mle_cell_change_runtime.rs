use tetra_core::{BitBuffer, SsiType, TdmaTime, TetraAddress};
use tetra_entities::mle::cell_change_runtime::{
    CELL_CHANGE_TRANSACTION_TIMEOUT_SLOTS, MleCellChangeError, MleCellChangePhase,
    MleCellChangeRuntime,
};
use tetra_pdus::mle::pdus::d_channel_response::DChannelResponse;
use tetra_pdus::mle::pdus::d_new_cell::DNewCell;
use tetra_pdus::mle::pdus::d_prepare_fail::DPrepareFail;
use tetra_pdus::mle::pdus::d_restore_ack::DRestoreAck;
use tetra_pdus::mle::pdus::d_restore_fail::DRestoreFail;
use tetra_pdus::mle::pdus::u_channel_request::UChannelRequest;
use tetra_pdus::mle::pdus::u_prepare::UPrepare;
use tetra_pdus::mle::pdus::u_restore::URestore;
use tetra_saps::common::{
    CellIdentity, CellType, MleChannelCommandValid, MleChannelRequestReason,
    MleChannelRequestRetryDelay, MleChannelResponseType, MleFailCause,
};
use tetra_saps::control::mle_cell_change::MleCellChangeControl;
use tetra_saps::lcmc::{
    enums::{alloc_type::ChanAllocType, ul_dl_assignment::UlDlAssignment},
    fields::chan_alloc_req::CmceChanAllocReq,
};

fn subscriber(issi: u32) -> TetraAddress {
    TetraAddress::new(issi, SsiType::Issi)
}

fn target_cell() -> CellIdentity {
    CellIdentity {
        mcc: 262,
        mnc: 1,
        location_area: Some(11),
        colour_code: Some(2),
        main_carrier: 1522,
        cell_type: CellType::ConventionalAccess,
    }
}

#[test]
fn prepare_can_be_granted_deferred_and_rejected() {
    let mut runtime = MleCellChangeRuntime::new();
    let address = subscriber(3001);
    let now = TdmaTime::default();
    runtime.observe_prepare(
        address,
        4,
        5,
        &UPrepare {
            cell_identifier_ca: Some(9),
            sdu: Some(BitBuffer::from_bitstr("10101010")),
        },
        now,
    );

    let outbound = runtime
        .handle_control(
            MleCellChangeControl::GrantPrepare {
                subscriber: address,
                command: MleChannelCommandValid::NoChannelChange,
                target_cell: Some(target_cell()),
                mm_sdu: None,
            },
            now.add_timeslots(1),
        )
        .unwrap();
    assert_eq!(outbound.endpoint_id, 4);
    assert_eq!(outbound.link_id, 5);
    let mut body = outbound.pdu;
    let decoded = DNewCell::from_bitbuf(&mut body).unwrap();
    assert_eq!(
        decoded.channel_command_valid,
        MleChannelCommandValid::NoChannelChange
    );
    assert_eq!(
        runtime.snapshot(now.add_timeslots(1)).transactions[0].phase,
        MleCellChangePhase::PrepareDeferred
    );

    let outbound = runtime
        .handle_control(
            MleCellChangeControl::RejectPrepare {
                subscriber: address,
                cause: MleFailCause::MsNotAllowedOnCell,
                mm_sdu: Some(BitBuffer::from_bitstr("1100")),
            },
            now.add_timeslots(2),
        )
        .unwrap();
    let mut body = outbound.pdu;
    let decoded = DPrepareFail::from_bitbuf(&mut body).unwrap();
    assert_eq!(decoded.fail_cause, MleFailCause::MsNotAllowedOnCell);
    assert_eq!(decoded.sdu.unwrap().dump_bin_unformatted(), "1100");
    let snapshot = runtime.snapshot(now.add_timeslots(2));
    assert_eq!(snapshot.transactions[0].phase, MleCellChangePhase::Rejected);
    assert_eq!(snapshot.counters.prepares_received, 1);
    assert_eq!(snapshot.counters.prepare_grants, 1);
    assert_eq!(snapshot.counters.prepare_rejects, 1);
}

#[test]
fn restore_acknowledgement_and_failure_use_the_learned_local_route() {
    let mut runtime = MleCellChangeRuntime::new();
    let now = TdmaTime::default();
    let address = subscriber(3002);
    runtime.observe_restore(
        address,
        7,
        8,
        &URestore {
            mcc: Some(262),
            mnc: Some(1),
            la: Some(10),
            sdu: Some(BitBuffer::from_bitstr("00110011")),
        },
        now,
    );

    let expected_allocation = CmceChanAllocReq {
        usage: Some(9),
        alloc_type: ChanAllocType::Replace,
        carrier: None,
        timeslots: [false, true, false, false],
        ul_dl_assigned: UlDlAssignment::Both,
    };
    let outbound = runtime
        .handle_control(
            MleCellChangeControl::AcknowledgeRestore {
                subscriber: address,
                cmce_sdu: BitBuffer::from_bitstr("11110000"),
                chan_alloc: Some(expected_allocation.clone()),
            },
            now.add_timeslots(1),
        )
        .unwrap();
    assert_eq!((outbound.endpoint_id, outbound.link_id), (7, 8));
    let allocation = outbound.chan_alloc.expect("D-RESTORE-ACK lost channel allocation");
    assert_eq!(allocation.usage, expected_allocation.usage);
    assert_eq!(allocation.alloc_type, expected_allocation.alloc_type);
    assert_eq!(allocation.timeslots, expected_allocation.timeslots);
    assert_eq!(allocation.ul_dl_assigned, expected_allocation.ul_dl_assigned);
    assert!(allocation.carrier.is_none());
    let mut body = outbound.pdu;
    let decoded = DRestoreAck::from_bitbuf(&mut body).unwrap();
    assert_eq!(decoded.sdu.unwrap().dump_bin_unformatted(), "11110000");
    assert_eq!(
        runtime.snapshot(now.add_timeslots(1)).transactions[0].phase,
        MleCellChangePhase::Restored
    );

    let other = subscriber(3003);
    runtime.observe_restore(
        other,
        9,
        10,
        &URestore {
            mcc: None,
            mnc: None,
            la: None,
            sdu: None,
        },
        now,
    );
    let outbound = runtime
        .handle_control(
            MleCellChangeControl::RejectRestore {
                subscriber: other,
                cause: MleFailCause::RestorationCannotBeDoneOnCell,
            },
            now.add_timeslots(1),
        )
        .unwrap();
    let mut body = outbound.pdu;
    let decoded = DRestoreFail::from_bitbuf(&mut body).unwrap();
    assert_eq!(
        decoded.fail_cause,
        MleFailCause::RestorationCannotBeDoneOnCell
    );
}

#[test]
fn channel_request_response_and_invalid_transition_are_accounted() {
    let mut runtime = MleCellChangeRuntime::new();
    let address = subscriber(3004);
    let now = TdmaTime::default();
    runtime.observe_channel_request(
        address,
        11,
        12,
        &UChannelRequest {
            reason_for_the_channel_request: MleChannelRequestReason::CurrentChannelRadioImprovable,
            requested_channel_class_identifiers: vec![1, 2],
            requested_channel_identifiers: vec![3],
            reserved: None,
        },
        now,
    );
    let outbound = runtime
        .handle_control(
            MleCellChangeControl::RespondChannelRequest {
                subscriber: address,
                response: MleChannelResponseType::Accepted,
                reason: MleChannelRequestReason::CurrentChannelRadioImprovable,
                retry_delay: MleChannelRequestRetryDelay::NoDelay,
            },
            now.add_timeslots(1),
        )
        .unwrap();
    let mut body = outbound.pdu;
    let decoded = DChannelResponse::from_bitbuf(&mut body).unwrap();
    assert_eq!(decoded.channel_response_type, MleChannelResponseType::Accepted);

    let error = runtime
        .handle_control(
            MleCellChangeControl::RespondChannelRequest {
                subscriber: address,
                response: MleChannelResponseType::Rejected,
                reason: MleChannelRequestReason::Unspecified,
                retry_delay: MleChannelRequestRetryDelay::Seconds5,
            },
            now.add_timeslots(2),
        )
        .unwrap_err();
    assert!(matches!(error, MleCellChangeError::InvalidPhase { .. }));
    assert_eq!(runtime.snapshot(now.add_timeslots(2)).counters.invalid_controls, 1);
}

#[test]
fn pending_transactions_receive_deterministic_timeout_responses() {
    let mut runtime = MleCellChangeRuntime::new();
    let now = TdmaTime::default();
    let prepare = subscriber(3005);
    let restore = subscriber(3006);
    runtime.observe_prepare(
        prepare,
        13,
        14,
        &UPrepare {
            cell_identifier_ca: None,
            sdu: None,
        },
        now,
    );
    runtime.observe_restore(
        restore,
        15,
        16,
        &URestore {
            mcc: None,
            mnc: None,
            la: None,
            sdu: None,
        },
        now,
    );

    let responses = runtime.tick(now.add_timeslots(CELL_CHANGE_TRANSACTION_TIMEOUT_SLOTS));
    assert_eq!(responses.len(), 2);
    let snapshot = runtime.snapshot(now.add_timeslots(CELL_CHANGE_TRANSACTION_TIMEOUT_SLOTS));
    assert!(snapshot
        .transactions
        .iter()
        .all(|transaction| transaction.phase == MleCellChangePhase::TimedOut));
    assert_eq!(snapshot.counters.timeouts, 2);
}

use tetra_core::{SsiType, TdmaTime, TetraAddress, tetra_entities::TetraEntity};
use tetra_entities::cmce::call_restore_runtime::{
    CALL_RESTORE_TRANSACTION_TIMEOUT_SLOTS, CallRestoreContext, CallRestoreRequest,
    CallRestoreRuntime, CallRestoreRuntimeError, GroupCallRestoreContext,
    GroupRestoreOrigin, IndividualCallRestoreContext, RestoreCallKind, RestorePhase,
    RestoreRejectReason,
};
use tetra_pdus::cmce::enums::{call_timeout::CallTimeout, transmission_grant::TransmissionGrant};
use tetra_saps::control::enums::{
    circuit_mode_type::CircuitModeType, communication_type::CommunicationType,
};

fn issi(value: u32) -> TetraAddress {
    TetraAddress::new(value, SsiType::Issi)
}

fn group_context(call_id: u16) -> CallRestoreContext {
    CallRestoreContext::Group(GroupCallRestoreContext {
        call_id,
        dest_gssi: 15501,
        source_issi: 4101,
        floor_holder: Some(4101),
        priority: 7,
        call_timeout: CallTimeout::T5m,
        created_at: TdmaTime::default(),
        tx_active: true,
        origin: GroupRestoreOrigin::Local { caller: issi(4101) },
        communication_type: CommunicationType::P2Mp,
        circuit_mode_type: CircuitModeType::TchS,
        speech_service: Some(0),
        etee_encrypted: false,
    })
}

fn individual_context(call_id: u16) -> CallRestoreContext {
    CallRestoreContext::Individual(IndividualCallRestoreContext {
        call_id,
        calling_addr: issi(5001),
        called_addr: issi(5002),
        simplex_duplex: false,
        priority: 6,
        call_timeout: CallTimeout::T5m,
        active_timer_started: Some(TdmaTime::default()),
        floor_holder: Some(5002),
        called_over_brew: false,
        calling_over_brew: false,
        brew_uuid: None,
        network_entity: Some(TetraEntity::Brew),
        network_call: None,
        communication_type: CommunicationType::P2p,
        circuit_mode_type: CircuitModeType::TchS,
        speech_service: Some(0),
        etee_encrypted: false,
    })
}

fn request(subscriber: TetraAddress, call_id: u16) -> CallRestoreRequest {
    CallRestoreRequest {
        subscriber,
        old_call_id: call_id,
        endpoint_id: 6,
        link_id: 7,
        request_to_transmit: true,
        other_party_ssi: None,
        previous_mcc: Some(262),
        previous_mnc: Some(1),
        previous_location_area: Some(10),
    }
}

#[test]
fn group_restore_tracks_context_resource_and_floor() {
    let mut runtime = CallRestoreRuntime::new();
    runtime.install_context(group_context(77));
    let now = TdmaTime::default();
    let key = runtime.begin(request(issi(4101), 77), now).unwrap();
    runtime
        .mark_context_matched(key, RestoreCallKind::Group, now.add_timeslots(1))
        .unwrap();
    runtime
        .mark_resource_allocated(key, 77, 2, 4, now.add_timeslots(2))
        .unwrap();
    runtime
        .mark_restored(key, 77, TransmissionGrant::Granted, now.add_timeslots(3))
        .unwrap();

    let snapshot = runtime.snapshot(now.add_timeslots(3));
    assert_eq!(snapshot.contexts, 1);
    assert_eq!(snapshot.transactions.len(), 1);
    assert_eq!(snapshot.transactions[0].phase, RestorePhase::Restored);
    assert_eq!(snapshot.transactions[0].timeslot, Some(2));
    assert_eq!(snapshot.counters.group_restores, 1);
    assert_eq!(snapshot.counters.floor_grants, 1);
}

#[test]
fn individual_restore_preserves_other_floor_holder() {
    let mut runtime = CallRestoreRuntime::new();
    runtime.install_context(individual_context(88));
    let now = TdmaTime::default();
    let key = runtime.begin(request(issi(5001), 88), now).unwrap();
    runtime
        .mark_context_matched(key, RestoreCallKind::Individual, now.add_timeslots(1))
        .unwrap();
    runtime
        .mark_restored(
            key,
            88,
            TransmissionGrant::GrantedToOtherUser,
            now.add_timeslots(2),
        )
        .unwrap();

    let snapshot = runtime.snapshot(now.add_timeslots(2));
    assert_eq!(snapshot.counters.individual_restores, 1);
    assert_eq!(snapshot.counters.floor_grants_to_other, 1);
    assert_eq!(
        snapshot.transactions[0].transmission_grant,
        Some(TransmissionGrant::GrantedToOtherUser)
    );
}

#[test]
fn replay_is_idempotent_and_pending_duplicate_is_rejected() {
    let mut runtime = CallRestoreRuntime::new();
    let now = TdmaTime::default();
    let req = request(issi(6001), 99);
    let key = runtime.begin(req.clone(), now).unwrap();
    assert!(matches!(
        runtime.begin(req.clone(), now.add_timeslots(1)),
        Err(CallRestoreRuntimeError::DuplicatePending(found)) if found == key
    ));
    runtime
        .mark_context_matched(key, RestoreCallKind::Group, now.add_timeslots(2))
        .unwrap();
    runtime
        .mark_restored(key, 99, TransmissionGrant::NotGranted, now.add_timeslots(3))
        .unwrap();
    assert!(matches!(
        runtime.begin(req, now.add_timeslots(4)),
        Err(CallRestoreRuntimeError::DuplicateTerminal(found)) if found == key
    ));
    assert_eq!(runtime.snapshot(now.add_timeslots(4)).counters.duplicate_requests, 2);
}

#[test]
fn unanswered_restore_times_out_and_is_visible_to_webui_snapshot() {
    let mut runtime = CallRestoreRuntime::new();
    let now = TdmaTime::default();
    let key = runtime.begin(request(issi(7001), 101), now).unwrap();
    let expired = runtime.tick(now.add_timeslots(CALL_RESTORE_TRANSACTION_TIMEOUT_SLOTS));
    assert_eq!(expired, vec![key]);
    let snapshot = runtime.snapshot(now.add_timeslots(CALL_RESTORE_TRANSACTION_TIMEOUT_SLOTS));
    assert_eq!(snapshot.transactions[0].phase, RestorePhase::TimedOut);
    assert_eq!(snapshot.transactions[0].reject_reason, Some(RestoreRejectReason::Timeout));
    assert_eq!(snapshot.counters.timeouts, 1);
}

#[test]
fn queued_restore_replays_then_completes_when_a_bearer_is_allocated() {
    let mut runtime = CallRestoreRuntime::new();
    runtime.install_context(group_context(120));
    let now = TdmaTime::default();
    let req = request(issi(8120), 120);
    let key = runtime.begin(req.clone(), now).unwrap();
    runtime
        .mark_context_matched(key, RestoreCallKind::Group, now.add_timeslots(1))
        .unwrap();
    runtime
        .mark_queued(
            key,
            121,
            TransmissionGrant::RequestQueued,
            now.add_timeslots(2),
        )
        .unwrap();

    assert_eq!(runtime.resolved_call_id(120), Some(121));
    assert!(matches!(
        runtime.begin(req, now.add_timeslots(3)),
        Err(CallRestoreRuntimeError::DuplicateQueued(found)) if found == key
    ));

    runtime
        .mark_resource_allocated(key, 121, 3, 9, now.add_timeslots(4))
        .unwrap();
    runtime
        .mark_restored(
            key,
            121,
            TransmissionGrant::GrantedToOtherUser,
            now.add_timeslots(5),
        )
        .unwrap();

    let snapshot = runtime.snapshot(now.add_timeslots(5));
    assert_eq!(snapshot.transactions[0].phase, RestorePhase::Restored);
    assert_eq!(snapshot.transactions[0].new_call_id, Some(121));
    assert_eq!(snapshot.counters.queued_restores, 1);
    assert_eq!(snapshot.counters.queued_allocations_completed, 1);
    assert_eq!(snapshot.counters.call_id_changes, 1);
}

#[test]
fn queued_restore_tx_request_can_be_cancelled_and_requeued_by_old_or_new_call_id() {
    let mut runtime = CallRestoreRuntime::new();
    runtime.install_context(group_context(140));
    let now = TdmaTime::default();
    let subscriber = issi(8140);
    let mut req = request(subscriber, 140);
    req.request_to_transmit = true;
    let key = runtime.begin(req, now).unwrap();
    runtime
        .mark_context_matched(key, RestoreCallKind::Group, now.add_timeslots(1))
        .unwrap();
    runtime
        .mark_queued(
            key,
            141,
            TransmissionGrant::RequestQueued,
            now.add_timeslots(2),
        )
        .unwrap();

    assert_eq!(runtime.queued_key_for_call(subscriber, 140), Some(key));
    assert_eq!(runtime.queued_key_for_call(subscriber, 141), Some(key));

    assert_eq!(
        runtime
            .set_queued_transmission_request(key, false, now.add_timeslots(3))
            .unwrap(),
        TransmissionGrant::NotGranted
    );
    let cancelled = runtime.snapshot(now.add_timeslots(3));
    assert!(!cancelled.transactions[0].request_to_transmit);
    assert_eq!(
        cancelled.transactions[0].transmission_grant,
        Some(TransmissionGrant::NotGranted)
    );
    assert_eq!(cancelled.counters.queued_tx_cancellations, 1);

    assert_eq!(
        runtime
            .set_queued_transmission_request(key, true, now.add_timeslots(4))
            .unwrap(),
        TransmissionGrant::RequestQueued
    );
    let requeued = runtime.snapshot(now.add_timeslots(4));
    assert!(requeued.transactions[0].request_to_transmit);
    assert_eq!(
        requeued.transactions[0].transmission_grant,
        Some(TransmissionGrant::RequestQueued)
    );
    assert_eq!(requeued.counters.queued_tx_requests, 1);
}

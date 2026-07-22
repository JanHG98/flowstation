mod common;

use common::ComponentTest;
use tetra_config::bluestation::StackMode;
use tetra_core::tetra_entities::TetraEntity;
use tetra_core::{BitBuffer, Sap, SsiType, TdmaTime, TetraAddress};
use tetra_entities::cmce::call_restore_runtime::{
    CallRestoreContext, GroupCallRestoreContext, GroupRestoreOrigin,
    IndividualCallRestoreContext, RestoreCallKind, RestorePhase,
};
use tetra_entities::cmce::cmce_bs::CmceBs;
use tetra_pdus::cmce::enums::{
    call_status::CallStatus, call_timeout::CallTimeout,
    transmission_grant::TransmissionGrant,
};
use tetra_pdus::cmce::fields::basic_service_information::BasicServiceInformation;
use tetra_pdus::cmce::pdus::{d_call_restore::DCallRestore, u_call_restore::UCallRestore};
use tetra_pdus::mle::enums::mle_protocol_discriminator::MleProtocolDiscriminator;
use tetra_pdus::mle::pdus::{d_restore_ack::DRestoreAck, u_restore::URestore};
use tetra_saps::control::enums::{
    circuit_mode_type::CircuitModeType, communication_type::CommunicationType,
};
use tetra_saps::lcmc::fields::chan_alloc_req::CmceChanAllocReq;
use tetra_saps::tla::TlaTlDataIndBl;
use tetra_saps::{SapMsg, SapMsgInner};

fn issi(value: u32) -> TetraAddress {
    TetraAddress::new(value, SsiType::Issi)
}

fn make_cell(main_carrier: u16, location_area: u8, colour_code: u8) -> ComponentTest {
    let mut config = ComponentTest::get_default_test_config(StackMode::Bs);
    config.cell.main_carrier = main_carrier;
    config.cell.location_area = location_area;
    config.cell.colour_code = colour_code;
    let mut cell = ComponentTest::from_config(config, Some(TdmaTime::default()));
    cell.populate_entities(
        vec![TetraEntity::Mle, TetraEntity::Cmce],
        vec![TetraEntity::Llc, TetraEntity::Umac],
    );
    cell
}

fn with_cmce<R>(cell: &mut ComponentTest, f: impl FnOnce(&mut CmceBs) -> R) -> R {
    let entity = cell
        .router
        .get_entity(TetraEntity::Cmce)
        .expect("CMCE missing");
    let cmce = entity
        .as_any_mut()
        .downcast_mut::<CmceBs>()
        .expect("CMCE downcast failed");
    f(cmce)
}

fn transfer_context(source: &mut ComponentTest, target: &mut ComponentTest, call_id: u16) {
    let context = with_cmce(source, |cmce| {
        cmce
            .export_call_restore_context(call_id)
            .expect("source restore context missing")
    });
    with_cmce(target, |cmce| cmce.install_call_restore_context(context));
}

fn submit_restore(
    cell: &mut ComponentTest,
    subscriber: TetraAddress,
    endpoint_id: u32,
    link_id: u32,
    call_restore: UCallRestore,
) {
    let mut cmce_sdu = BitBuffer::new_autoexpand(160);
    call_restore
        .to_bitbuf(&mut cmce_sdu)
        .expect("encode U-CALL RESTORE");
    cmce_sdu.seek(0);

    let restore = URestore {
        mcc: Some(262),
        mnc: Some(1),
        la: Some(10),
        sdu: Some(cmce_sdu),
    };
    let mut body = BitBuffer::new_autoexpand(256);
    restore.to_bitbuf(&mut body).expect("encode U-RESTORE");
    body.seek(0);

    let body_len = body.get_len();
    let mut sdu = BitBuffer::new(3 + body_len);
    sdu.write_bits(MleProtocolDiscriminator::Mle.into_raw(), 3);
    sdu.copy_bits(&mut body, body_len);
    sdu.seek(0);

    cell.submit_message(SapMsg {
        sap: Sap::TlaSap,
        src: TetraEntity::Llc,
        dest: TetraEntity::Mle,
        msg: SapMsgInner::TlaTlDataIndBl(TlaTlDataIndBl {
            main_address: subscriber,
            link_id,
            endpoint_id,
            new_endpoint_id: None,
            css_endpoint_id: None,
            tl_sdu: Some(sdu),
            scrambling_code: 0,
            fcs_flag: false,
            air_interface_encryption: 0,
            chan_change_resp_req: false,
            chan_change_handle: None,
            chan_info: None,
            req_handle: 0,
        }),
    });
    cell.deliver_all_messages();
}

fn take_call_restore(
    cell: &mut ComponentTest,
) -> (DCallRestore, Option<CmceChanAllocReq>) {
    for message in cell.dump_sinks() {
        if let SapMsgInner::TlaTlDataReqBl(primitive) = message.msg {
            let mut sdu = primitive.tl_sdu;
            sdu.seek(0);
            if sdu.read_bits(3) != Some(MleProtocolDiscriminator::Mle.into_raw()) {
                continue;
            }
            let mut mle_body = BitBuffer::from_bitbuffer_pos(&sdu);
            if let Ok(ack) = DRestoreAck::from_bitbuf(&mut mle_body) {
                let mut cmce_body = ack.sdu.expect("D-RESTORE-ACK missing CMCE SDU");
                let restore = DCallRestore::from_bitbuf(&mut cmce_body)
                    .expect("embedded D-CALL RESTORE malformed");
                return (restore, primitive.chan_alloc);
            }
        }
    }
    panic!("no D-RESTORE-ACK/D-CALL RESTORE found");
}

fn speech_service(communication_type: CommunicationType) -> BasicServiceInformation {
    BasicServiceInformation {
        circuit_mode_type: CircuitModeType::TchS,
        encryption_flag: false,
        communication_type,
        slots_per_frame: None,
        speech_service: Some(0),
    }
}

#[test]
fn running_group_call_is_restored_on_target_cell_with_floor_and_priority_context() {
    let mut source = make_cell(1521, 10, 1);
    let mut target = make_cell(1522, 11, 2);
    let call_id = 77;
    let subscriber = issi(4101);

    with_cmce(&mut source, |cmce| {
        cmce.install_call_restore_context(CallRestoreContext::Group(GroupCallRestoreContext {
            call_id,
            dest_gssi: 12001,
            source_issi: subscriber.ssi,
            floor_holder: Some(subscriber.ssi),
            priority: 12,
            call_timeout: CallTimeout::T5m,
            created_at: TdmaTime::default(),
            tx_active: true,
            origin: GroupRestoreOrigin::Local { caller: subscriber },
            communication_type: CommunicationType::P2Mp,
            circuit_mode_type: CircuitModeType::TchS,
            speech_service: Some(0),
            etee_encrypted: false,
        }));
    });
    transfer_context(&mut source, &mut target, call_id);

    submit_restore(
        &mut target,
        subscriber,
        6,
        7,
        UCallRestore {
            call_identifier: call_id,
            request_to_transmit_send_data: true,
            other_party_type_identifier: 1,
            other_party_short_number_address: None,
            other_party_ssi: Some(12001),
            other_party_extension: None,
            basic_service_information: Some(speech_service(CommunicationType::P2Mp)),
            facility: None,
            dm_ms_address: None,
            proprietary: None,
        },
    );

    let (response, allocation) = take_call_restore(&mut target);
    assert_eq!(response.call_identifier, call_id);
    assert_eq!(response.new_call_identifier, None);
    assert_eq!(
        TransmissionGrant::try_from(response.transmission_grant as u64).unwrap(),
        TransmissionGrant::Granted
    );
    assert_eq!(response.call_time_out, None);
    assert!(!response.reset_call_time_out_timer_t310_);
    assert!(allocation.is_some(), "restored U-plane needs a channel allocation");

    let snapshot = with_cmce(&mut target, |cmce| cmce.call_restore_snapshot());
    assert_eq!(snapshot.transactions.len(), 1);
    assert_eq!(snapshot.transactions[0].phase, RestorePhase::Restored);
    assert_eq!(snapshot.transactions[0].kind, Some(RestoreCallKind::Group));
    assert_eq!(snapshot.counters.group_restores, 1);
    assert_eq!(snapshot.counters.floor_grants, 1);
}

#[test]
fn running_individual_simplex_call_restores_without_stealing_the_other_floor() {
    let mut source = make_cell(1521, 10, 1);
    let mut target = make_cell(1522, 11, 2);
    let call_id = 88;
    let calling = issi(5001);
    let called = issi(5002);

    with_cmce(&mut source, |cmce| {
        cmce.install_call_restore_context(CallRestoreContext::Individual(
            IndividualCallRestoreContext {
                call_id,
                calling_addr: calling,
                called_addr: called,
                simplex_duplex: false,
                priority: 8,
                call_timeout: CallTimeout::T5m,
                active_timer_started: Some(TdmaTime::default()),
                floor_holder: Some(called.ssi),
                called_over_brew: false,
                calling_over_brew: false,
                brew_uuid: None,
                network_entity: None,
                network_call: None,
                communication_type: CommunicationType::P2p,
                circuit_mode_type: CircuitModeType::TchS,
                speech_service: Some(0),
                etee_encrypted: false,
            },
        ));
    });
    transfer_context(&mut source, &mut target, call_id);

    submit_restore(
        &mut target,
        calling,
        8,
        9,
        UCallRestore {
            call_identifier: call_id,
            request_to_transmit_send_data: true,
            other_party_type_identifier: 1,
            other_party_short_number_address: None,
            other_party_ssi: Some(called.ssi as u64),
            other_party_extension: None,
            basic_service_information: Some(speech_service(CommunicationType::P2p)),
            facility: None,
            dm_ms_address: None,
            proprietary: None,
        },
    );

    let (response, allocation) = take_call_restore(&mut target);
    assert_eq!(response.call_identifier, call_id);
    assert_eq!(
        TransmissionGrant::try_from(response.transmission_grant as u64).unwrap(),
        TransmissionGrant::GrantedToOtherUser
    );
    assert!(allocation.is_some(), "restored receive U-plane needs a channel allocation");

    let snapshot = with_cmce(&mut target, |cmce| cmce.call_restore_snapshot());
    assert_eq!(snapshot.transactions[0].kind, Some(RestoreCallKind::Individual));
    assert_eq!(snapshot.counters.individual_restores, 1);
    assert_eq!(snapshot.counters.floor_grants_to_other, 1);
}

#[test]
fn call_id_collision_is_remapped_once_and_reused_by_later_group_participants() {
    let mut target = make_cell(1522, 11, 2);
    let old_call_id = 130;
    let first_calling = issi(9001);
    let first_called = issi(9002);

    // Occupy the old call identifier on the target cell with an unrelated active call.
    with_cmce(&mut target, |cmce| {
        cmce.install_call_restore_context(CallRestoreContext::Individual(
            IndividualCallRestoreContext {
                call_id: old_call_id,
                calling_addr: first_calling,
                called_addr: first_called,
                simplex_duplex: false,
                priority: 4,
                call_timeout: CallTimeout::T5m,
                active_timer_started: Some(TdmaTime::default()),
                floor_holder: Some(first_called.ssi),
                called_over_brew: false,
                calling_over_brew: false,
                brew_uuid: None,
                network_entity: None,
                network_call: None,
                communication_type: CommunicationType::P2p,
                circuit_mode_type: CircuitModeType::TchS,
                speech_service: Some(0),
                etee_encrypted: false,
            },
        ));
    });
    submit_restore(
        &mut target,
        first_calling,
        20,
        21,
        UCallRestore {
            call_identifier: old_call_id,
            request_to_transmit_send_data: false,
            other_party_type_identifier: 1,
            other_party_short_number_address: None,
            other_party_ssi: Some(first_called.ssi as u64),
            other_party_extension: None,
            basic_service_information: Some(speech_service(CommunicationType::P2p)),
            facility: None,
            dm_ms_address: None,
            proprietary: None,
        },
    );
    let _ = take_call_restore(&mut target);

    let speaker = issi(9101);
    let listener = issi(9102);
    with_cmce(&mut target, |cmce| {
        cmce.install_call_restore_context(CallRestoreContext::Group(
            GroupCallRestoreContext {
                call_id: old_call_id,
                dest_gssi: 19100,
                source_issi: speaker.ssi,
                floor_holder: Some(speaker.ssi),
                priority: 10,
                call_timeout: CallTimeout::T5m,
                created_at: TdmaTime::default(),
                tx_active: true,
                origin: GroupRestoreOrigin::Local { caller: speaker },
                communication_type: CommunicationType::P2Mp,
                circuit_mode_type: CircuitModeType::TchS,
                speech_service: Some(0),
                etee_encrypted: false,
            },
        ));
    });

    submit_restore(
        &mut target,
        speaker,
        22,
        23,
        UCallRestore {
            call_identifier: old_call_id,
            request_to_transmit_send_data: true,
            other_party_type_identifier: 1,
            other_party_short_number_address: None,
            other_party_ssi: Some(19100),
            other_party_extension: None,
            basic_service_information: Some(speech_service(CommunicationType::P2Mp)),
            facility: None,
            dm_ms_address: None,
            proprietary: None,
        },
    );
    let (speaker_response, speaker_allocation) = take_call_restore(&mut target);
    let remapped = speaker_response
        .new_call_identifier
        .expect("collision must produce a new call identifier") as u16;
    assert_ne!(remapped, old_call_id);
    assert!(speaker_allocation.is_some());

    submit_restore(
        &mut target,
        listener,
        24,
        25,
        UCallRestore {
            call_identifier: old_call_id,
            request_to_transmit_send_data: false,
            other_party_type_identifier: 1,
            other_party_short_number_address: None,
            other_party_ssi: Some(19100),
            other_party_extension: None,
            basic_service_information: Some(speech_service(CommunicationType::P2Mp)),
            facility: None,
            dm_ms_address: None,
            proprietary: None,
        },
    );
    let (listener_response, listener_allocation) = take_call_restore(&mut target);
    assert_eq!(listener_response.new_call_identifier, Some(remapped as u64));
    assert!(listener_allocation.is_some());

    let snapshot = with_cmce(&mut target, |cmce| cmce.call_restore_snapshot());
    assert_eq!(snapshot.counters.group_restores, 2);
    assert_eq!(snapshot.counters.call_id_changes, 1);
}

#[test]
fn congested_target_acknowledges_restore_as_queued_without_a_channel_allocation() {
    let mut target = make_cell(1522, 11, 2);

    for index in 0..4_u16 {
        let call_id = 200 + index;
        let subscriber = issi(12_000 + u32::from(index));
        let gssi = 22_000 + u32::from(index);
        with_cmce(&mut target, |cmce| {
            cmce.install_call_restore_context(CallRestoreContext::Group(
                GroupCallRestoreContext {
                    call_id,
                    dest_gssi: gssi,
                    source_issi: subscriber.ssi,
                    floor_holder: Some(subscriber.ssi),
                    priority: 1,
                    call_timeout: CallTimeout::T5m,
                    created_at: TdmaTime::default(),
                    tx_active: true,
                    origin: GroupRestoreOrigin::Local { caller: subscriber },
                    communication_type: CommunicationType::P2Mp,
                    circuit_mode_type: CircuitModeType::TchS,
                    speech_service: Some(0),
                    etee_encrypted: false,
                },
            ));
        });
        submit_restore(
            &mut target,
            subscriber,
            30 + u32::from(index),
            40 + u32::from(index),
            UCallRestore {
                call_identifier: call_id,
                request_to_transmit_send_data: true,
                other_party_type_identifier: 1,
                other_party_short_number_address: None,
                other_party_ssi: Some(gssi as u64),
                other_party_extension: None,
                basic_service_information: Some(speech_service(CommunicationType::P2Mp)),
                facility: None,
                dm_ms_address: None,
                proprietary: None,
            },
        );
        let (response, allocation) = take_call_restore(&mut target);
        if index < 3 {
            assert_eq!(response.call_status, Some(CallStatus::Callcontinue.into_raw()));
            assert!(allocation.is_some());
        } else {
            assert_eq!(response.call_status, Some(CallStatus::Callqueued.into_raw()));
            assert_eq!(
                TransmissionGrant::try_from(response.transmission_grant as u64).unwrap(),
                TransmissionGrant::RequestQueued
            );
            assert!(allocation.is_none());
        }
    }

    let snapshot = with_cmce(&mut target, |cmce| cmce.call_restore_snapshot());
    assert_eq!(snapshot.counters.group_restores, 3);
    assert_eq!(snapshot.counters.queued_restores, 1);
    assert!(snapshot
        .transactions
        .iter()
        .any(|transaction| transaction.phase == RestorePhase::Queued));
}

#[test]
fn replayed_group_restore_keeps_the_same_bearer_allocation() {
    let mut target = make_cell(1522, 11, 2);
    let call_id = 240;
    let subscriber = issi(14_001);
    let gssi = 24_001;

    with_cmce(&mut target, |cmce| {
        cmce.install_call_restore_context(CallRestoreContext::Group(
            GroupCallRestoreContext {
                call_id,
                dest_gssi: gssi,
                source_issi: subscriber.ssi,
                floor_holder: Some(subscriber.ssi),
                priority: 9,
                call_timeout: CallTimeout::T5m,
                created_at: TdmaTime::default(),
                tx_active: true,
                origin: GroupRestoreOrigin::Local { caller: subscriber },
                communication_type: CommunicationType::P2Mp,
                circuit_mode_type: CircuitModeType::TchS,
                speech_service: Some(0),
                etee_encrypted: false,
            },
        ));
    });

    let restore = UCallRestore {
        call_identifier: call_id,
        request_to_transmit_send_data: true,
        other_party_type_identifier: 1,
        other_party_short_number_address: None,
        other_party_ssi: Some(gssi as u64),
        other_party_extension: None,
        basic_service_information: Some(speech_service(CommunicationType::P2Mp)),
        facility: None,
        dm_ms_address: None,
        proprietary: None,
    };

    submit_restore(&mut target, subscriber, 50, 51, restore.clone());
    let (first, first_allocation) = take_call_restore(&mut target);
    let first_allocation = first_allocation.expect("initial restore lost allocation");

    submit_restore(&mut target, subscriber, 50, 51, restore);
    let (replay, replay_allocation) = take_call_restore(&mut target);
    let replay_allocation = replay_allocation.expect("replayed restore lost allocation");

    assert_eq!(replay.call_identifier, first.call_identifier);
    assert_eq!(replay.new_call_identifier, first.new_call_identifier);
    assert_eq!(replay_allocation.usage, first_allocation.usage);
    assert_eq!(replay_allocation.timeslots, first_allocation.timeslots);
    assert_eq!(replay_allocation.ul_dl_assigned, first_allocation.ul_dl_assigned);

    let snapshot = with_cmce(&mut target, |cmce| cmce.call_restore_snapshot());
    assert_eq!(snapshot.counters.group_restores, 1);
    assert_eq!(snapshot.counters.duplicate_requests, 1);
}

#[test]
fn group_listener_restore_keeps_receive_plane_when_another_user_is_speaking() {
    let mut target = make_cell(1522, 11, 2);
    let call_id = 171;
    let speaker = issi(9711);
    let listener = issi(9712);

    with_cmce(&mut target, |cmce| {
        cmce.install_call_restore_context(CallRestoreContext::Group(
            GroupCallRestoreContext {
                call_id,
                dest_gssi: 19710,
                source_issi: speaker.ssi,
                floor_holder: Some(speaker.ssi),
                priority: 9,
                call_timeout: CallTimeout::T5m,
                created_at: TdmaTime::default(),
                tx_active: true,
                origin: GroupRestoreOrigin::Local { caller: speaker },
                communication_type: CommunicationType::P2Mp,
                circuit_mode_type: CircuitModeType::TchS,
                speech_service: Some(0),
                etee_encrypted: false,
            },
        ));
    });

    submit_restore(
        &mut target,
        listener,
        51,
        52,
        UCallRestore {
            call_identifier: call_id,
            request_to_transmit_send_data: false,
            other_party_type_identifier: 1,
            other_party_short_number_address: None,
            other_party_ssi: Some(19710),
            other_party_extension: None,
            basic_service_information: Some(speech_service(CommunicationType::P2Mp)),
            facility: None,
            dm_ms_address: None,
            proprietary: None,
        },
    );

    let (response, allocation) = take_call_restore(&mut target);
    assert_eq!(
        TransmissionGrant::try_from(response.transmission_grant as u64).unwrap(),
        TransmissionGrant::GrantedToOtherUser
    );
    assert!(allocation.is_some(), "a restoring listener needs the receive bearer");
}

#[test]
fn duplex_individual_restore_is_granted_even_without_a_tx_request_bit() {
    let mut target = make_cell(1522, 11, 2);
    let call_id = 172;
    let calling = issi(9721);
    let called = issi(9722);

    with_cmce(&mut target, |cmce| {
        cmce.install_call_restore_context(CallRestoreContext::Individual(
            IndividualCallRestoreContext {
                call_id,
                calling_addr: calling,
                called_addr: called,
                simplex_duplex: true,
                priority: 7,
                call_timeout: CallTimeout::T5m,
                active_timer_started: Some(TdmaTime::default()),
                floor_holder: None,
                called_over_brew: false,
                calling_over_brew: false,
                brew_uuid: None,
                network_entity: None,
                network_call: None,
                communication_type: CommunicationType::P2p,
                circuit_mode_type: CircuitModeType::TchS,
                speech_service: Some(0),
                etee_encrypted: false,
            },
        ));
    });

    submit_restore(
        &mut target,
        calling,
        53,
        54,
        UCallRestore {
            call_identifier: call_id,
            request_to_transmit_send_data: false,
            other_party_type_identifier: 1,
            other_party_short_number_address: None,
            other_party_ssi: Some(called.ssi as u64),
            other_party_extension: None,
            basic_service_information: Some(speech_service(CommunicationType::P2p)),
            facility: None,
            dm_ms_address: None,
            proprietary: None,
        },
    );

    let (response, allocation) = take_call_restore(&mut target);
    assert_eq!(
        TransmissionGrant::try_from(response.transmission_grant as u64).unwrap(),
        TransmissionGrant::Granted
    );
    assert!(allocation.is_some(), "duplex restoration requires an active bearer");
}

mod common;

use common::ComponentTest;
use tetra_config::bluestation::StackMode;
use tetra_core::tetra_entities::TetraEntity;
use tetra_core::{BitBuffer, Layer2Service, Sap, SsiType, TdmaTime, TetraAddress};
use tetra_entities::mle::mle_bs::MleBs;
use tetra_entities::sndcp::sndcp_bs::Sndcp;
use tetra_saps::common::{
    ChannelAdvice, DataClass, DataPriority, Layer2Qos, Layer2Report,
    LowerLayerResourceAvailability, LowerLayerResourceReason, PduPriority,
    ReconnectionResult, RequestHandle, ReservationInfo, ScheduledDataStatus,
    SetupReport, StealingPermission, TransferResult,
};
use tetra_saps::ltpd::{
    LtpdMleConnectReq, LtpdMleDisconnectReq, LtpdMleReconnectReq,
    LtpdMleUnitdataReq,
};
use tetra_saps::tla::TlaTlDataIndBl;
use tetra_saps::tlmc::TlmcConfigureInd;
use tetra_saps::{SapMsg, SapMsgInner};

fn incoming_sndcp(address: TetraAddress, endpoint_id: u32, link_id: u32) -> SapMsg {
    let mut sdu = BitBuffer::new(11);
    sdu.write_bits(0b100, 3);
    sdu.write_bits(0x21, 8);
    sdu.seek(0);
    SapMsg {
        sap: Sap::TlaSap,
        src: TetraEntity::Llc,
        dest: TetraEntity::Mle,
        msg: SapMsgInner::TlaTlDataIndBl(TlaTlDataIndBl {
            main_address: address,
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
    }
}

fn unitdata(address: Option<TetraAddress>, handle: u32, endpoint_id: u32, link_id: u32) -> SapMsg {
    let mut sdu = BitBuffer::new(8);
    sdu.write_bits(0x42, 8);
    sdu.seek(0);
    SapMsg {
        sap: Sap::TlpdSap,
        src: TetraEntity::Sndcp,
        dest: TetraEntity::Mle,
        msg: SapMsgInner::LtpdMleUnitdataReq(LtpdMleUnitdataReq {
            sdu,
            handle: RequestHandle(handle),
            address,
            layer2service: Layer2Service::Acknowledged,
            unacknowledged_basic_link_repetitions: 0,
            pdu_priority: PduPriority::default(),
            endpoint_id,
            link_id,
            stealing_permission: StealingPermission::NotRequired,
            stealing_repeats_flag: false,
            channel_advice: ChannelAdvice::NotRequested,
            data_class_information: DataClass::NonClassified,
            data_priority: DataPriority::Undefined,
            mle_data_priority_flag: false,
            packet_data_flag: true,
            scheduled_data_status: ScheduledDataStatus::NotScheduled,
            maximum_schedule_interval_slots: None,
            fcs_flag: false,
            chan_alloc: None,
        }),
    }
}

#[test]
fn initial_open_and_info_update_the_sndcp_client_snapshot() {
    let mut test = ComponentTest::new(StackMode::Bs, Some(TdmaTime::default()));
    test.populate_entities(vec![TetraEntity::Mle, TetraEntity::Sndcp], vec![]);

    test.router.tick_start();
    test.deliver_all_messages();

    let component = test
        .router
        .get_entity(TetraEntity::Sndcp)
        .expect("SNDCP missing");
    let sndcp = component
        .as_any_mut()
        .downcast_mut::<Sndcp>()
        .expect("SNDCP downcast failed");
    let snapshot = sndcp.ltpd_snapshot();
    assert!(snapshot.network.is_some());
    assert_eq!(snapshot.link_state, tetra_saps::common::LtpdLinkState::Open);
    assert!(!snapshot.busy);
    assert!(!snapshot.disabled);
}

#[test]
fn inbound_unitdata_registers_route_and_reaches_sndcp() {
    let mut test = ComponentTest::new(StackMode::Bs, None);
    test.populate_entities(vec![TetraEntity::Mle], vec![TetraEntity::Sndcp]);
    let address = TetraAddress::new(1001, SsiType::Issi);

    test.submit_message(incoming_sndcp(address, 2, 3));
    test.deliver_all_messages();

    let messages = test.dump_sinks();
    assert!(messages.iter().any(|message| {
        matches!(
            &message.msg,
            SapMsgInner::LtpdMleUnitdataInd(indication)
                if indication.received_tetra_address == address
                    && indication.endpoint_id == 2
                    && indication.link_id == 3
        )
    }));

    let component = test.router.get_entity(TetraEntity::Mle).expect("MLE missing");
    let mle = component
        .as_any_mut()
        .downcast_mut::<MleBs>()
        .expect("MLE-BS downcast failed");
    let snapshot = mle.ltpd_snapshot();
    assert_eq!(snapshot.links.len(), 1);
    assert_eq!(snapshot.links[0].address, address);
}

#[test]
fn downlink_unitdata_is_wrapped_by_mle_and_reported_to_sndcp() {
    let mut test = ComponentTest::new(StackMode::Bs, None);
    test.populate_entities(
        vec![TetraEntity::Mle],
        vec![TetraEntity::Sndcp, TetraEntity::Llc],
    );
    let address = TetraAddress::new(1002, SsiType::Issi);
    test.submit_message(incoming_sndcp(address, 4, 5));
    test.deliver_all_messages();
    let _ = test.dump_sinks();

    test.submit_message(unitdata(None, 77, 4, 5));
    test.deliver_all_messages();
    let messages = test.dump_sinks();

    assert!(messages.iter().any(|message| {
        matches!(
            &message.msg,
            SapMsgInner::TlaTlDataReqBl(request)
                if request.main_address == address
                    && request.endpoint_id == 4
                    && request.link_id == 5
                    && request.tl_sdu.peek_bits(3) == Some(0b100)
        )
    }));
    assert!(messages.iter().any(|message| {
        matches!(
            &message.msg,
            SapMsgInner::LtpdMleReportInd(report)
                if report.handle == RequestHandle(77)
                    && report.transfer_result == TransferResult::SuccessBufferEmpty
        )
    }));
}

#[test]
fn route_hint_rebuilds_context_after_local_restart() {
    let mut test = ComponentTest::new(StackMode::Bs, None);
    test.populate_entities(
        vec![TetraEntity::Mle],
        vec![TetraEntity::Sndcp, TetraEntity::Llc],
    );
    let address = TetraAddress::new(1003, SsiType::Issi);

    test.submit_message(unitdata(Some(address), 78, 6, 7));
    test.deliver_all_messages();
    let messages = test.dump_sinks();

    assert!(messages.iter().any(|message| {
        matches!(
            &message.msg,
            SapMsgInner::TlaTlDataReqBl(request) if request.main_address == address
        )
    }));
}

#[test]
fn unknown_route_without_hint_is_rejected() {
    let mut test = ComponentTest::new(StackMode::Bs, None);
    test.populate_entities(
        vec![TetraEntity::Mle],
        vec![TetraEntity::Sndcp, TetraEntity::Llc],
    );

    test.submit_message(unitdata(None, 79, 8, 9));
    test.deliver_all_messages();
    let messages = test.dump_sinks();

    assert!(!messages
        .iter()
        .any(|message| matches!(&message.msg, SapMsgInner::TlaTlDataReqBl(_))));
    assert!(messages.iter().any(|message| {
        matches!(
            &message.msg,
            SapMsgInner::LtpdMleReportInd(report)
                if report.handle == RequestHandle(79)
                    && report.transfer_result == TransferResult::FailedRemovedFromBuffer
        )
    }));
}

#[test]
fn connect_disconnect_and_reconnect_have_explicit_results() {
    let mut test = ComponentTest::new(StackMode::Bs, None);
    test.populate_entities(vec![TetraEntity::Mle], vec![TetraEntity::Sndcp]);
    let address = TetraAddress::new(1004, SsiType::Issi);

    test.submit_message(SapMsg {
        sap: Sap::TlpdSap,
        src: TetraEntity::Sndcp,
        dest: TetraEntity::Mle,
        msg: SapMsgInner::LtpdMleConnectReq(LtpdMleConnectReq {
            address,
            endpoint_id: 10,
            link_id: 11,
            reservation_information: ReservationInfo { octets_available: 512 },
            pdu_priority: PduPriority::default(),
            layer_2_qos: Layer2Qos::default(),
            encryption_flag: false,
            setup_report: SetupReport::Success,
        }),
    });
    test.deliver_all_messages();
    let connected = test.dump_sinks();
    assert!(connected.iter().any(|message| {
        matches!(
            &message.msg,
            SapMsgInner::LtpdMleConnectConfirm(confirm)
                if confirm.setup_report == SetupReport::Success
        )
    }));

    test.submit_message(SapMsg {
        sap: Sap::TlpdSap,
        src: TetraEntity::Sndcp,
        dest: TetraEntity::Mle,
        msg: SapMsgInner::LtpdMleDisconnectReq(LtpdMleDisconnectReq {
            endpoint_id: 10,
            link_id: 11,
            pdu_priority: PduPriority::default(),
            encryption_flag: false,
            report: Layer2Report::LocalDisconnection,
        }),
    });
    test.deliver_all_messages();
    let disconnected = test.dump_sinks();
    assert!(disconnected.iter().any(|message| {
        matches!(
            &message.msg,
            SapMsgInner::LtpdMleDisconnectInd(indication)
                if indication.report == Layer2Report::LocalDisconnection
        )
    }));

    test.submit_message(SapMsg {
        sap: Sap::TlpdSap,
        src: TetraEntity::Sndcp,
        dest: TetraEntity::Mle,
        msg: SapMsgInner::LtpdMleReconnectReq(LtpdMleReconnectReq {
            endpoint_id: 10,
            link_id: 11,
            reservation_information: ReservationInfo { octets_available: 128 },
            pdu_priority: PduPriority::default(),
            encryption_flag: false,
            stealing_permission: StealingPermission::NotRequired,
        }),
    });
    test.deliver_all_messages();
    let reconnected = test.dump_sinks();
    assert!(reconnected.iter().any(|message| {
        matches!(
            &message.msg,
            SapMsgInner::LtpdMleReconnectConfirm(confirm)
                if confirm.reconnection_result == ReconnectionResult::Success
        )
    }));
}

#[test]
fn tlmc_resource_edges_drive_break_and_resume() {
    let mut test = ComponentTest::new(StackMode::Bs, Some(TdmaTime::default()));
    test.populate_entities(vec![TetraEntity::Mle], vec![TetraEntity::Sndcp]);

    test.submit_message(SapMsg {
        sap: Sap::TlmcSap,
        src: TetraEntity::Umac,
        dest: TetraEntity::Mle,
        msg: SapMsgInner::TlmcConfigureInd(TlmcConfigureInd {
            endpoint_id: 0,
            lower_layer_resource_availability: LowerLayerResourceAvailability::Unavailable,
            reason: LowerLayerResourceReason::LossOfRadioResources,
        }),
    });
    test.deliver_all_messages();
    let broken = test.dump_sinks();
    assert!(broken
        .iter()
        .any(|message| matches!(&message.msg, SapMsgInner::LtpdMleBreakInd(_))));

    test.submit_message(SapMsg {
        sap: Sap::TlmcSap,
        src: TetraEntity::Umac,
        dest: TetraEntity::Mle,
        msg: SapMsgInner::TlmcConfigureInd(TlmcConfigureInd {
            endpoint_id: 0,
            lower_layer_resource_availability: LowerLayerResourceAvailability::Available,
            reason: LowerLayerResourceReason::RecoveryOfRadioResources,
        }),
    });
    test.deliver_all_messages();
    let resumed = test.dump_sinks();
    assert!(resumed
        .iter()
        .any(|message| matches!(&message.msg, SapMsgInner::LtpdMleResumeInd(_))));
}

mod common;

use tetra_config::bluestation::StackMode;
use tetra_core::tetra_entities::TetraEntity;
use tetra_core::{BitBuffer, PhyBlockNum, Sap, TdmaTime};
use tetra_saps::common::{
    RfChannelNumber, ScanRequestId, ScanningMeasurementMethod,
};
use tetra_saps::sapmsg::{SapMsg, SapMsgInner};
use tetra_saps::tlmc::{TlmcConfigureReq, TlmcScanReq, TlmcValidAddress};
use tetra_saps::tmv::{TmvUnitdataInd, enums::logical_chans::LogicalChannel};
use tetra_entities::umac::umac_ms::UmacMs;

use crate::common::ComponentTest;

fn sync_message(carrier_num: u16, rssi_dbfs: f32) -> SapMsg {
    SapMsg {
        sap: Sap::TmvSap,
        src: TetraEntity::Lmac,
        dest: TetraEntity::Umac,
        msg: SapMsgInner::TmvUnitdataInd(TmvUnitdataInd {
            carrier_num,
            pdu: BitBuffer::from_bitstr("000100000111010110010010000000001101001000000100010101110011"),
            block_num: PhyBlockNum::Block1,
            logical_channel: LogicalChannel::Bsch,
            crc_pass: true,
            scrambling_code: 0,
            rssi_dbfs,
        }),
    }
}

#[test]
fn configure_request_returns_confirmation() {
    let mut test = ComponentTest::new(StackMode::Ms, None);
    test.populate_entities(vec![TetraEntity::Umac], vec![TetraEntity::Mle]);

    test.submit_message(SapMsg {
        sap: Sap::TlmcSap,
        src: TetraEntity::Mle,
        dest: TetraEntity::Umac,
        msg: SapMsgInner::TlmcConfigureReq(TlmcConfigureReq {
            valid_addresses: Some(TlmcValidAddress { mcc: 262, mnc: 1 }),
            endpoint_id: Some(0),
            ..Default::default()
        }),
    });
    test.deliver_all_messages();

    let messages = test.dump_sinks();
    assert!(messages
        .iter()
        .any(|message| matches!(&message.msg, SapMsgInner::TlmcConfigureConf(_))));
}

#[test]
fn scan_completes_on_valid_sync_for_requested_carrier() {
    let mut test = ComponentTest::new(StackMode::Ms, None);
    test.populate_entities(
        vec![TetraEntity::Umac],
        vec![TetraEntity::Mle, TetraEntity::Lmac],
    );

    test.submit_message(SapMsg {
        sap: Sap::TlmcSap,
        src: TetraEntity::Mle,
        dest: TetraEntity::Umac,
        msg: SapMsgInner::TlmcScanReq(TlmcScanReq {
            request_id: ScanRequestId(77),
            channel_number: RfChannelNumber(1521),
            measurement_method: ScanningMeasurementMethod::NonInterrupting,
            characteristics: None,
            threshold_level: None,
            channel_classes: Vec::new(),
        }),
    });
    test.deliver_all_messages();

    let lower_messages = test.dump_sinks();
    assert!(lower_messages
        .iter()
        .any(|message| matches!(&message.msg, SapMsgInner::TmvConfigureReq(_))));

    test.submit_message(sync_message(1521, -42.0));
    test.deliver_all_messages();
    let messages = test.dump_sinks();
    assert!(messages.iter().any(|message| {
        matches!(
            &message.msg,
            SapMsgInner::TlmcScanConf(confirm) if confirm.request_id == ScanRequestId(77)
        )
    }));
}

#[test]
fn resource_loss_and_recovery_are_edge_triggered() {
    let mut test = ComponentTest::new(StackMode::Ms, None);
    test.populate_entities(vec![TetraEntity::Umac], vec![TetraEntity::Mle]);

    test.submit_message(sync_message(1521, -45.0));
    test.deliver_all_messages();
    let first = test.dump_sinks();
    assert!(first
        .iter()
        .any(|message| matches!(&message.msg, SapMsgInner::TlmcConfigureInd(_))));

    test.router.set_dl_time(TdmaTime::default().add_timeslots(500));
    test.router.tick_start();
    test.deliver_all_messages();
    let lost = test.dump_sinks();
    assert!(lost.iter().any(|message| {
        matches!(
            &message.msg,
            SapMsgInner::TlmcConfigureInd(indication)
                if indication.lower_layer_resource_availability
                    == tetra_saps::common::LowerLayerResourceAvailability::Unavailable
        )
    }));

    test.submit_message(sync_message(1521, -44.0));
    test.deliver_all_messages();
    let recovered = test.dump_sinks();
    assert!(recovered.iter().any(|message| {
        matches!(
            &message.msg,
            SapMsgInner::TlmcConfigureInd(indication)
                if indication.lower_layer_resource_availability
                    == tetra_saps::common::LowerLayerResourceAvailability::Available
        )
    }));
}
#[test]
fn runtime_snapshot_is_available_for_tbs_diagnostics() {
    let mut test = ComponentTest::new(StackMode::Ms, None);
    test.populate_entities(vec![TetraEntity::Umac], vec![]);

    let component = test.router.get_entity(TetraEntity::Umac).expect("UMAC missing");
    let umac = component
        .as_any_mut()
        .downcast_mut::<UmacMs>()
        .expect("UMAC-MS downcast failed");
    let snapshot = umac.tlmc_snapshot();
    assert_eq!(snapshot.known_resource_count, 0);
    assert!(snapshot.monitored_channels.is_empty());
}

#[test]
fn scan_times_out_with_negative_confirmation() {
    let mut test = ComponentTest::new(StackMode::Ms, None);
    test.populate_entities(vec![TetraEntity::Umac], vec![TetraEntity::Mle, TetraEntity::Lmac]);

    test.submit_message(SapMsg {
        sap: Sap::TlmcSap,
        src: TetraEntity::Mle,
        dest: TetraEntity::Umac,
        msg: SapMsgInner::TlmcScanReq(TlmcScanReq {
            request_id: ScanRequestId(91),
            channel_number: RfChannelNumber(1600),
            measurement_method: ScanningMeasurementMethod::NonInterrupting,
            characteristics: None,
            threshold_level: None,
            channel_classes: Vec::new(),
        }),
    });
    test.deliver_all_messages();
    let _ = test.dump_sinks();

    test.router.set_dl_time(TdmaTime::default().add_timeslots(500));
    test.router.tick_start();
    test.deliver_all_messages();

    let messages = test.dump_sinks();
    assert!(messages.iter().any(|message| {
        matches!(
            &message.msg,
            SapMsgInner::TlmcScanConf(confirm)
                if confirm.request_id == ScanRequestId(91)
                    && confirm.report
                        == tetra_saps::common::Layer2Report::ServiceTemporarilyUnavailable
        )
    }));
}


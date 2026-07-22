mod common;

use tetra_config::bluestation::StackMode;
use tetra_core::tetra_entities::TetraEntity;
use tetra_core::{BitBuffer, Sap, SsiType, TdmaTime, TetraAddress};
use tetra_entities::mm::components::client_state::{MmClientMobilityContext, MmClientState};
use tetra_entities::mm::mm_bs::MmBs;
use tetra_pdus::mm::enums::energy_saving_mode::EnergySavingMode;
use tetra_pdus::mm::enums::location_update_type::LocationUpdateType;
use tetra_pdus::mm::pdus::d_location_update_accept::DLocationUpdateAccept;
use tetra_pdus::mm::pdus::d_location_update_proceeding::DLocationUpdateProceeding;
use tetra_pdus::mm::pdus::u_location_update_demand::ULocationUpdateDemand;
use tetra_saps::control::mle_cell_change::MleCellChangeControl;
use tetra_saps::lmm::{LmmMlePrepareInd, LmmMleUnitdataInd};
use tetra_saps::{SapMsg, SapMsgInner};

use crate::common::ComponentTest;

const HOME_ISSI: u32 = 2_260_575;
const HOME_MNI: u64 = 0x04_0001;

fn context() -> MmClientMobilityContext {
    MmClientMobilityContext {
        issi: HOME_ISSI,
        state: MmClientState::Attached,
        groups: vec![100, 1200, 15501],
        energy_saving_mode: EnergySavingMode::Eg2,
        monitoring_frame: Some(5),
        monitoring_multiframe: Some(2),
        class_of_ms: None,
        last_handle: 17,
        tei: Some(0x1122_3344),
    }
}

fn cell() -> ComponentTest {
    let mut test = ComponentTest::new(StackMode::Bs, Some(TdmaTime::default()));
    test.populate_entities(vec![], vec![TetraEntity::Mle]);
    test.register_entity(MmBs::new(test.get_shared_config(), None, None));
    test
}

fn with_mm<R>(test: &mut ComponentTest, f: impl FnOnce(&mut MmBs) -> R) -> R {
    let entity = test
        .router
        .get_entity(TetraEntity::Mm)
        .expect("MM missing from test cell");
    let mm = entity
        .as_any_mut()
        .downcast_mut::<MmBs>()
        .expect("MM-BS downcast failed");
    f(mm)
}

fn demand(kind: LocationUpdateType) -> ULocationUpdateDemand {
    ULocationUpdateDemand {
        location_update_type: kind,
        request_to_append_la: false,
        cipher_control: false,
        ciphering_parameters: None,
        class_of_ms: None,
        energy_saving_mode: None,
        la_information: None,
        ssi: None,
        address_extension: None,
        group_identity_location_demand: None,
        group_report_response: None,
        authentication_uplink: None,
        extended_capabilities: None,
        proprietary: None,
    }
}

fn submit_demand(test: &mut ComponentTest, address: TetraAddress, handle: u32, pdu: ULocationUpdateDemand) {
    let mut sdu = BitBuffer::new_autoexpand(64);
    pdu.to_bitbuf(&mut sdu).expect("encode U-LOCATION-UPDATE-DEMAND");
    sdu.seek(0);
    test.submit_message(SapMsg {
        sap: Sap::LmmSap,
        src: TetraEntity::Mle,
        dest: TetraEntity::Mm,
        msg: SapMsgInner::LmmMleUnitdataInd(LmmMleUnitdataInd {
            sdu,
            handle,
            received_address: address,
        }),
    });
    test.run_stack(Some(2));
}

fn proceeding_from(messages: &[SapMsg]) -> DLocationUpdateProceeding {
    messages
        .iter()
        .find_map(|message| {
            let SapMsgInner::LmmMleUnitdataReq(request) = &message.msg else {
                return None;
            };
            let mut sdu = BitBuffer::from_bitstr(&request.sdu.to_bitstr());
            DLocationUpdateProceeding::from_bitbuf(&mut sdu).ok()
        })
        .expect("D-LOCATION-UPDATE-PROCEEDING missing")
}

fn accept_from(messages: &[SapMsg]) -> DLocationUpdateAccept {
    messages
        .iter()
        .find_map(|message| {
            let SapMsgInner::LmmMleUnitdataReq(request) = &message.msg else {
                return None;
            };
            let mut sdu = BitBuffer::from_bitstr(&request.sdu.to_bitstr());
            DLocationUpdateAccept::from_bitbuf(&mut sdu).ok()
        })
        .expect("D-LOCATION-UPDATE-ACCEPT missing")
}

#[test]
fn migration_context_moves_between_two_mm_cells_and_survives_under_vassi() {
    let mut home = cell();
    let mut visited = cell();
    with_mm(&mut home, |mm| mm.import_mobility_context(HOME_ISSI, &context()));

    let mut first = demand(LocationUpdateType::MigratingLocationUpdating);
    first.ssi = Some(HOME_ISSI as u64);
    first.address_extension = Some(HOME_MNI);
    submit_demand(
        &mut visited,
        TetraAddress {
            ssi_type: SsiType::Ussi,
            ssi: 0x123456,
        },
        42,
        first,
    );
    let proceeding = proceeding_from(&visited.dump_sinks());

    let exported = with_mm(&mut home, |mm| {
        mm.export_mobility_context(HOME_ISSI)
            .expect("home context missing")
    });
    with_mm(&mut visited, |mm| {
        mm.provide_migration_context(proceeding.ssi, exported)
            .expect("context import failed")
    });

    let mut second = demand(LocationUpdateType::DemandLocationUpdating);
    second.ssi = Some(HOME_ISSI as u64);
    second.address_extension = Some(HOME_MNI);
    submit_demand(
        &mut visited,
        TetraAddress::issi(proceeding.ssi),
        43,
        second,
    );
    let accept = accept_from(&visited.dump_sinks());
    assert_eq!(accept.ssi, Some(proceeding.ssi as u64));

    let restored = with_mm(&mut visited, |mm| {
        mm.export_mobility_context(proceeding.ssi)
            .expect("visited context missing")
    });
    assert_eq!(restored.groups, context().groups);
    assert_eq!(restored.energy_saving_mode, EnergySavingMode::Eg2);
    assert_eq!(restored.tei, Some(0x1122_3344));
}

#[test]
fn forward_registration_embeds_accept_and_exports_context_for_target_cell() {
    let mut source = cell();
    let mut target = cell();
    with_mm(&mut source, |mm| mm.import_mobility_context(HOME_ISSI, &context()));

    let mut request = demand(LocationUpdateType::ServiceRestorationRoamingLocationUpdating);
    request.la_information = Some(11);
    request.ssi = Some(HOME_ISSI as u64);
    let mut sdu = BitBuffer::new_autoexpand(64);
    request.to_bitbuf(&mut sdu).expect("encode forward registration");
    sdu.seek(0);
    source.submit_message(SapMsg {
        sap: Sap::LmmSap,
        src: TetraEntity::Mle,
        dest: TetraEntity::Mm,
        msg: SapMsgInner::LmmMlePrepareInd(LmmMlePrepareInd {
            sdu,
            subscriber: TetraAddress::issi(HOME_ISSI),
            endpoint_id: 3,
            link_id: 9,
            cell_identifier_ca: Some(7),
        }),
    });
    source.run_stack(Some(2));

    let control = source
        .dump_sinks()
        .into_iter()
        .find_map(|message| match message.msg {
            SapMsgInner::MleCellChangeControl(control) => Some(control),
            _ => None,
        })
        .expect("forward-registration decision missing");
    let MleCellChangeControl::GrantPrepare { mm_sdu: Some(mut mm_sdu), .. } = control else {
        panic!("expected GrantPrepare with embedded MM accept");
    };
    let accept = DLocationUpdateAccept::from_bitbuf(&mut mm_sdu).expect("decode embedded accept");
    assert_eq!(accept.location_update_accept_type, LocationUpdateType::ServiceRestorationRoamingLocationUpdating);

    let transferred = with_mm(&mut source, |mm| {
        mm.take_forward_context(HOME_ISSI)
            .expect("forward context missing")
    });
    with_mm(&mut target, |mm| mm.import_mobility_context(HOME_ISSI, &transferred));
    let target_context = with_mm(&mut target, |mm| {
        mm.export_mobility_context(HOME_ISSI)
            .expect("target context missing")
    });
    assert_eq!(target_context.groups, context().groups);
    assert_eq!(target_context.last_handle, 17);
}

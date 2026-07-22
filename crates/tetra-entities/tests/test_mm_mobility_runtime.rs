use tetra_core::{TdmaTime, TetraAddress};
use tetra_entities::mm::components::client_state::{MmClientMobilityContext, MmClientState};
use tetra_entities::mm::mobility_runtime::{
    MmMobilityError, MmMobilityPhase, MmMobilityRuntime, MmMobilityTimeout,
    MM_MOBILITY_TIMEOUT_SLOTS,
};
use tetra_pdus::mm::enums::energy_saving_mode::EnergySavingMode;
use tetra_pdus::mm::enums::location_update_type::LocationUpdateType;
use tetra_pdus::mm::pdus::u_location_update_demand::ULocationUpdateDemand;

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

fn context(issi: u32) -> MmClientMobilityContext {
    MmClientMobilityContext {
        issi,
        state: MmClientState::Attached,
        groups: vec![100, 101],
        energy_saving_mode: EnergySavingMode::Eg2,
        monitoring_frame: Some(2),
        monitoring_multiframe: Some(1),
        class_of_ms: None,
        last_handle: 0,
        tei: Some(0x1234),
    }
}

#[test]
fn two_stage_migration_allocates_vassi_and_transfers_context() {
    let now = TdmaTime::default();
    let mut runtime = MmMobilityRuntime::new();
    let mut first = demand(LocationUpdateType::MigratingLocationUpdating);
    first.address_extension = Some(0x0400_01);
    let (vassi, home_mni) = runtime
        .begin_migration(TetraAddress::issi(0x123456), 7, &first, now, |_| false)
        .unwrap();
    assert_eq!(home_mni, 0x0400_01);
    runtime
        .provide_migration_context(vassi, context(2_260_575), now.add_timeslots(1))
        .unwrap();

    let mut second = demand(LocationUpdateType::DemandLocationUpdating);
    second.ssi = Some(2_260_575);
    second.address_extension = Some(home_mni as u64);
    let completion = runtime
        .complete_migration(vassi, &second, now.add_timeslots(2))
        .unwrap();
    assert_eq!(completion.local_issi, vassi);
    assert_eq!(completion.home_issi, 2_260_575);
    assert_eq!(completion.imported_context.unwrap().groups, vec![100, 101]);
    assert_eq!(runtime.snapshot(now.add_timeslots(2)).migrations[0].phase, MmMobilityPhase::MigrationAccepted);
}

#[test]
fn migration_rejects_a_changed_home_identity() {
    let now = TdmaTime::default();
    let mut runtime = MmMobilityRuntime::new();
    let mut first = demand(LocationUpdateType::MigratingLocationUpdating);
    first.ssi = Some(2_260_575);
    first.address_extension = Some(0x0400_01);
    let (vassi, _) = runtime
        .begin_migration(TetraAddress::issi(0x123456), 0, &first, now, |_| false)
        .unwrap();
    let mut second = demand(LocationUpdateType::DemandLocationUpdating);
    second.ssi = Some(9_999_999);
    second.address_extension = Some(0x0400_01);
    assert!(matches!(
        runtime.complete_migration(vassi, &second, now.add_timeslots(1)),
        Err(MmMobilityError::IdentityMismatch)
    ));
}

#[test]
fn forward_registration_exports_the_existing_context() {
    let now = TdmaTime::default();
    let mut runtime = MmMobilityRuntime::new();
    let mut request = demand(LocationUpdateType::ServiceRestorationRoamingLocationUpdating);
    request.la_information = Some(11);
    let subscriber = TetraAddress::issi(2_260_575);
    let result = runtime
        .begin_forward_registration(subscriber, Some(3), &request, context(subscriber.ssi), now)
        .unwrap();
    assert_eq!(result.target_location_area, 11);
    runtime.accept_forward_registration(subscriber.ssi, now.add_timeslots(1)).unwrap();
    assert_eq!(runtime.take_forward_context(subscriber.ssi).unwrap().groups, vec![100, 101]);
}

#[test]
fn pending_migration_has_a_bounded_timeout() {
    let now = TdmaTime::default();
    let mut runtime = MmMobilityRuntime::new();
    let mut first = demand(LocationUpdateType::MigratingLocationUpdating);
    first.address_extension = Some(0x0400_01);
    runtime
        .begin_migration(TetraAddress::issi(0x123456), 9, &first, now, |_| false)
        .unwrap();
    let timeouts = runtime.tick(now.add_timeslots(MM_MOBILITY_TIMEOUT_SLOTS));
    assert!(matches!(timeouts.as_slice(), [MmMobilityTimeout::Migration { handle: 9, .. }]));
}

#[test]
fn terminal_migration_history_is_bounded_and_vassi_can_be_reused() {
    use tetra_entities::mm::mobility_runtime::MM_MOBILITY_HISTORY_SLOTS;

    let now = TdmaTime::default();
    let mut runtime = MmMobilityRuntime::new();
    let subscriber = TetraAddress::issi(0x123456);
    let mut first = demand(LocationUpdateType::MigratingLocationUpdating);
    first.ssi = Some(2_260_575);
    first.address_extension = Some(0x0400_01);
    let (first_vassi, _) = runtime
        .begin_migration(subscriber, 1, &first, now, |_| false)
        .unwrap();
    let mut second = demand(LocationUpdateType::DemandLocationUpdating);
    second.ssi = Some(2_260_575);
    second.address_extension = Some(0x0400_01);
    runtime
        .complete_migration(first_vassi, &second, now.add_timeslots(1))
        .unwrap();

    runtime.tick(now.add_timeslots(1 + MM_MOBILITY_HISTORY_SLOTS));
    assert!(runtime.snapshot(now.add_timeslots(1 + MM_MOBILITY_HISTORY_SLOTS)).migrations.is_empty());

    let (second_vassi, _) = runtime
        .begin_migration(
            subscriber,
            2,
            &first,
            now.add_timeslots(2 + MM_MOBILITY_HISTORY_SLOTS),
            |_| false,
        )
        .unwrap();
    assert_ne!(second_vassi, 0);
}

mod common;

use common::{TestCell, TwoCellHarness};
use tetra_core::{SsiType, TetraAddress};
use tetra_saps::common::LtpdLinkState;
use tetra_saps::SapMsgInner;

#[test]
fn two_cells_keep_independent_identity_and_packet_contexts() {
    let mut harness = TwoCellHarness::new();
    harness.start();

    assert_eq!(harness.cell(TestCell::A).config.config().cell.main_carrier, 1521);
    assert_eq!(harness.cell(TestCell::B).config.config().cell.main_carrier, 1522);
    assert_eq!(harness.cell(TestCell::A).config.config().cell.location_area, 10);
    assert_eq!(harness.cell(TestCell::B).config.config().cell.location_area, 11);

    let cell_a_start = harness.drain(TestCell::A);
    let cell_b_start = harness.drain(TestCell::B);
    assert!(cell_a_start.iter().any(|message| matches!(
        &message.msg,
        SapMsgInner::LtpdMleInfoInd(info)
            if info.broadcast_parameters.main_carrier == Some(1521)
                && info.broadcast_parameters.location_area == Some(10)
                && info.broadcast_parameters.colour_code == Some(1)
    )));
    assert!(cell_b_start.iter().any(|message| matches!(
        &message.msg,
        SapMsgInner::LtpdMleInfoInd(info)
            if info.broadcast_parameters.main_carrier == Some(1522)
                && info.broadcast_parameters.location_area == Some(11)
                && info.broadcast_parameters.colour_code == Some(2)
    )));

    let address = TetraAddress::new(2001, SsiType::Issi);
    harness.learn_route(TestCell::A, address, 1, 1);
    let a = harness.ltpd_snapshot(TestCell::A);
    let b = harness.ltpd_snapshot(TestCell::B);
    assert_eq!(a.links.len(), 1);
    assert_eq!(a.links[0].address, address);
    assert!(b.links.is_empty());
}

#[test]
fn packet_context_can_move_between_cells_without_cross_contamination() {
    let mut harness = TwoCellHarness::new();
    let address = TetraAddress::new(2002, SsiType::Issi);
    harness.learn_route(TestCell::A, address, 2, 3);
    let _ = harness.drain(TestCell::A);

    harness.transfer_route(TestCell::A, TestCell::B, address, 2, 3, 4, 5);
    let a = harness.ltpd_snapshot(TestCell::A);
    let b = harness.ltpd_snapshot(TestCell::B);

    assert_eq!(a.links.len(), 1);
    assert_eq!(a.links[0].state, LtpdLinkState::Closed);
    assert_eq!(a.links[0].endpoint_id, 2);
    assert_eq!(b.links.len(), 1);
    assert_eq!(b.links[0].state, LtpdLinkState::Connected);
    assert_eq!(b.links[0].endpoint_id, 4);
    assert_eq!(b.links[0].link_id, 5);
    assert_eq!(b.links[0].address, address);
}

#[test]
fn lower_layer_failure_is_isolated_to_one_cell() {
    let mut harness = TwoCellHarness::new();
    let address_a = TetraAddress::new(2003, SsiType::Issi);
    let address_b = TetraAddress::new(2004, SsiType::Issi);
    harness.learn_route(TestCell::A, address_a, 6, 7);
    harness.learn_route(TestCell::B, address_b, 8, 9);
    let _ = harness.drain(TestCell::A);
    let _ = harness.drain(TestCell::B);

    harness.set_resources_available(TestCell::A, false);
    let a = harness.ltpd_snapshot(TestCell::A);
    let b = harness.ltpd_snapshot(TestCell::B);

    assert!(!a.lower_layer_available);
    assert_eq!(a.links[0].state, LtpdLinkState::Broken);
    assert!(b.lower_layer_available);
    assert_eq!(b.links[0].state, LtpdLinkState::Connected);
}

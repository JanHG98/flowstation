use tetra_config::bluestation::StackMode;
use tetra_core::tetra_entities::TetraEntity;
use tetra_core::{BitBuffer, Sap, TdmaTime, TetraAddress};
use tetra_entities::mle::ltpd_runtime::LtpdRuntimeSnapshot;
use tetra_entities::mle::mle_bs::MleBs;
use tetra_saps::common::{
    LowerLayerResourceAvailability, LowerLayerResourceReason, PduPriority,
};
use tetra_saps::ltpd::LtpdMleDisconnectReq;
use tetra_saps::tla::TlaTlDataIndBl;
use tetra_saps::tlmc::TlmcConfigureInd;
use tetra_saps::{SapMsg, SapMsgInner};

use super::ComponentTest;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestCell {
    A,
    B,
}

/// First reusable two-cell harness for the SWMI roadmap.
///
/// It deliberately stays below real RF and D-NEW-CELL signalling. Each cell has
/// an independent MLE/TLPD runtime and independent message router. Later phases
/// can extend this harness with mobility-core coordination and real restore PDUs
/// without replacing the basic test topology.
pub struct TwoCellHarness {
    pub cell_a: ComponentTest,
    pub cell_b: ComponentTest,
}

impl TwoCellHarness {
    pub fn new() -> Self {
        let mut config_a = ComponentTest::get_default_test_config(StackMode::Bs);
        config_a.cell.main_carrier = 1521;
        config_a.cell.location_area = 10;
        config_a.cell.colour_code = 1;
        config_a.cell.sndcp_service = true;

        let mut config_b = config_a.clone();
        config_b.cell.main_carrier = 1522;
        config_b.cell.location_area = 11;
        config_b.cell.colour_code = 2;

        let mut cell_a = ComponentTest::from_config(config_a, Some(TdmaTime::default()));
        let mut cell_b = ComponentTest::from_config(config_b, Some(TdmaTime::default()));
        for cell in [&mut cell_a, &mut cell_b] {
            cell.populate_entities(
                vec![TetraEntity::Mle],
                vec![TetraEntity::Sndcp, TetraEntity::Llc],
            );
        }

        Self { cell_a, cell_b }
    }

    pub fn cell(&self, cell: TestCell) -> &ComponentTest {
        match cell {
            TestCell::A => &self.cell_a,
            TestCell::B => &self.cell_b,
        }
    }

    pub fn cell_mut(&mut self, cell: TestCell) -> &mut ComponentTest {
        match cell {
            TestCell::A => &mut self.cell_a,
            TestCell::B => &mut self.cell_b,
        }
    }

    pub fn start(&mut self) {
        for cell in [&mut self.cell_a, &mut self.cell_b] {
            cell.router.tick_start();
            cell.deliver_all_messages();
        }
    }

    pub fn learn_route(
        &mut self,
        cell: TestCell,
        address: TetraAddress,
        endpoint_id: u32,
        link_id: u32,
    ) {
        let mut sdu = BitBuffer::new(11);
        sdu.write_bits(0b100, 3);
        sdu.write_bits(0x21, 8);
        sdu.seek(0);
        self.cell_mut(cell).submit_message(SapMsg {
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
        });
        self.cell_mut(cell).deliver_all_messages();
    }

    pub fn disconnect_route(&mut self, cell: TestCell, endpoint_id: u32, link_id: u32) {
        self.cell_mut(cell).submit_message(SapMsg {
            sap: Sap::TlpdSap,
            src: TetraEntity::Sndcp,
            dest: TetraEntity::Mle,
            msg: SapMsgInner::LtpdMleDisconnectReq(LtpdMleDisconnectReq {
                endpoint_id,
                link_id,
                pdu_priority: PduPriority::default(),
                encryption_flag: false,
                report: tetra_saps::common::Layer2Report::LocalDisconnection,
            }),
        });
        self.cell_mut(cell).deliver_all_messages();
    }

    pub fn transfer_route(
        &mut self,
        source: TestCell,
        target: TestCell,
        address: TetraAddress,
        old_endpoint_id: u32,
        old_link_id: u32,
        new_endpoint_id: u32,
        new_link_id: u32,
    ) {
        self.disconnect_route(source, old_endpoint_id, old_link_id);
        self.learn_route(target, address, new_endpoint_id, new_link_id);
    }

    pub fn set_resources_available(&mut self, cell: TestCell, available: bool) {
        self.cell_mut(cell).submit_message(SapMsg {
            sap: Sap::TlmcSap,
            src: TetraEntity::Umac,
            dest: TetraEntity::Mle,
            msg: SapMsgInner::TlmcConfigureInd(TlmcConfigureInd {
                endpoint_id: 0,
                lower_layer_resource_availability: if available {
                    LowerLayerResourceAvailability::Available
                } else {
                    LowerLayerResourceAvailability::Unavailable
                },
                reason: if available {
                    LowerLayerResourceReason::RecoveryOfRadioResources
                } else {
                    LowerLayerResourceReason::LossOfRadioResources
                },
            }),
        });
        self.cell_mut(cell).deliver_all_messages();
    }

    pub fn ltpd_snapshot(&mut self, cell: TestCell) -> LtpdRuntimeSnapshot {
        let component = self
            .cell_mut(cell)
            .router
            .get_entity(TetraEntity::Mle)
            .expect("MLE missing from two-cell harness");
        component
            .as_any_mut()
            .downcast_mut::<MleBs>()
            .expect("MLE-BS downcast failed")
            .ltpd_snapshot()
    }

    pub fn drain(&mut self, cell: TestCell) -> Vec<SapMsg> {
        self.cell_mut(cell).dump_sinks()
    }
}

impl Default for TwoCellHarness {
    fn default() -> Self {
        Self::new()
    }
}

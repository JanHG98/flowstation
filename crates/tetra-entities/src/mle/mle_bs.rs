use crate::mle::cell_change_runtime::{
    MleCellChangeOutbound, MleCellChangeRuntime, MleCellChangeRuntimeSnapshot,
};
use crate::mle::components::broadcast::MleBroadcast;
use crate::mle::ltpd_runtime::{LtpdRuntime, LtpdRuntimeRole, LtpdRuntimeSnapshot};
use crate::{MessageQueue, TetraEntityTrait};
use tetra_config::bluestation::SharedConfig;
use tetra_core::tetra_entities::TetraEntity;
use tetra_core::{
    BitBuffer, EndpointId, Layer2Service, LinkId, Sap, TdmaTime, TetraAddress,
    unimplemented_log,
};
use tetra_pdus::mle::enums::mle_pdu_type_ul::MlePduTypeUl;
use tetra_pdus::mle::enums::mle_protocol_discriminator::MleProtocolDiscriminator;
use tetra_pdus::mle::pdus::u_channel_request::UChannelRequest;
use tetra_pdus::mle::pdus::u_prepare::UPrepare;
use tetra_pdus::mle::pdus::u_restore::URestore;
use tetra_saps::common::{
    ChannelChangeHandle, LowerLayerResourceAvailability, MleBroadcastParameters,
    PermittedTemporaryServices, ReceivedAddressType,
};
use tetra_saps::control::mle_cell_change::MleCellChangeControl;
use tetra_saps::lcmc::{LcmcMleRestoreInd, LcmcMleUnitdataInd};
use tetra_saps::lmm::LmmMleUnitdataInd;
use tetra_saps::ltpd::LtpdMleUnitdataInd;
use tetra_saps::tla::{TlaTlDataReqBl, TlaTlUnitdataReqBl};
use tetra_saps::{SapMsg, SapMsgInner};

pub struct MleBs {
    config: SharedConfig,
    broadcast: MleBroadcast,
    ltpd: LtpdRuntime,
    cell_change: MleCellChangeRuntime,
    current_time: TdmaTime,
}

/// Multiframes at which D-NWRK-BROADCAST is sent within each hyperframe.
/// Two broadcasts per hyperframe (~30.6s interval) for faster time/date display on terminals.
/// BlueStation default was 1 per hyperframe (~61.2s) which is slow on cold attach.
/// We don't use the first multiframe to avoid congestion with other hyperframe-triggered events.
const MLE_BROADCAST_MULTIFRAMES: [u8; 2] = [20, 50];
/// Frame at which D-NWRK-BROADCAST is sent within the broadcast multiframe.
const MLE_BROADCAST_FRAME: u8 = 1;

impl MleBs {
    pub fn new(config: SharedConfig) -> Self {
        let broadcast = MleBroadcast::new(config.clone());
        let ltpd = {
            let cfg = config.config();
            LtpdRuntime::new(
                LtpdRuntimeRole::Swmi,
                cfg.net.mcc,
                cfg.net.mnc,
                MleBroadcastParameters {
                    mcc: Some(cfg.net.mcc),
                    mnc: Some(cfg.net.mnc),
                    location_area: Some(cfg.cell.location_area),
                    colour_code: Some(cfg.cell.colour_code),
                    main_carrier: Some(cfg.cell.main_carrier),
                    packet_data_supported: Some(cfg.cell.sndcp_service),
                    data_priority_supported: Some(true),
                },
            )
        };
        Self {
            config,
            broadcast,
            ltpd,
            cell_change: MleCellChangeRuntime::new(),
            current_time: TdmaTime::default(),
        }
    }

    /// Read-only packet-data SAP state for the TBS WebUI and future Node Gateway.
    pub fn ltpd_snapshot(&self) -> LtpdRuntimeSnapshot {
        self.ltpd.snapshot()
    }

    /// Read-only cell-change state for the local TBS WebUI and future Node Gateway.
    pub fn cell_change_snapshot(&self) -> MleCellChangeRuntimeSnapshot {
        self.cell_change.snapshot(self.current_time)
    }

    /// Notify SNDCP that lower-layer packet-data resources disappeared.
    pub fn ltpd_notify_break(&mut self, queue: &mut MessageQueue) {
        self.ltpd.notify_break(queue);
    }

    /// Notify SNDCP that lower-layer packet-data resources are available again.
    pub fn ltpd_notify_resume(&mut self, queue: &mut MessageQueue) {
        self.ltpd.notify_resume(queue);
    }

    pub fn ltpd_set_busy(&mut self, queue: &mut MessageQueue, busy: bool) {
        self.ltpd.set_busy(queue, busy);
    }

    pub fn ltpd_set_disabled(
        &mut self,
        queue: &mut MessageQueue,
        disabled: bool,
        permitted_services: PermittedTemporaryServices,
    ) {
        self.ltpd.set_disabled(queue, disabled, permitted_services);
    }

    fn rx_tla_mle_pdu(
        &mut self,
        queue: &mut MessageQueue,
        subscriber: TetraAddress,
        endpoint_id: EndpointId,
        link_id: LinkId,
        mut sdu: BitBuffer,
    ) {
        tracing::trace!(
            %subscriber,
            endpoint_id,
            link_id,
            "MLE: received infrastructure-side cell-change PDU"
        );

        let Some(bits) = sdu.peek_bits(3) else {
            tracing::warn!("MLE: cell-change PDU is shorter than the PDU type");
            self.cell_change.record_parse_error();
            return;
        };
        let Ok(pdu_type) = MlePduTypeUl::try_from(bits) else {
            tracing::warn!("MLE: invalid uplink PDU type {} in {}", bits, sdu.dump_bin());
            self.cell_change.record_parse_error();
            return;
        };

        match pdu_type {
            MlePduTypeUl::UPrepare => match UPrepare::from_bitbuf(&mut sdu) {
                Ok(pdu) => {
                    self.cell_change.observe_prepare(
                        subscriber,
                        endpoint_id,
                        link_id,
                        &pdu,
                        self.current_time,
                    );
                    if let Some(mm_sdu) = pdu.sdu {
                        queue.push_back(SapMsg {
                            sap: Sap::LmmSap,
                            src: TetraEntity::Mle,
                            dest: TetraEntity::Mm,
                            msg: SapMsgInner::LmmMleUnitdataInd(LmmMleUnitdataInd {
                                sdu: mm_sdu,
                                handle: 0,
                                received_address: subscriber,
                            }),
                        });
                    }
                }
                Err(error) => {
                    tracing::warn!(?error, %subscriber, "MLE: failed to parse U-PREPARE");
                    self.cell_change.record_parse_error();
                }
            },
            MlePduTypeUl::URestore => match URestore::from_bitbuf(&mut sdu) {
                Ok(pdu) => {
                    self.cell_change.observe_restore(
                        subscriber,
                        endpoint_id,
                        link_id,
                        &pdu,
                        self.current_time,
                    );
                    if let Some(cmce_sdu) = pdu.sdu {
                        queue.push_back(SapMsg {
                            sap: Sap::LcmcSap,
                            src: TetraEntity::Mle,
                            dest: TetraEntity::Cmce,
                            msg: SapMsgInner::LcmcMleRestoreInd(LcmcMleRestoreInd {
                                sdu: cmce_sdu,
                                subscriber,
                                endpoint_id,
                                link_id,
                                previous_mcc: pdu.mcc,
                                previous_mnc: pdu.mnc,
                                previous_location_area: pdu.la,
                            }),
                        });
                    } else {
                        tracing::warn!(%subscriber, "MLE: U-RESTORE contains no CMCE restore SDU");
                    }
                }
                Err(error) => {
                    tracing::warn!(?error, %subscriber, "MLE: failed to parse U-RESTORE");
                    self.cell_change.record_parse_error();
                }
            },
            MlePduTypeUl::UChannelRequest => match UChannelRequest::from_bitbuf(&mut sdu) {
                Ok(pdu) => self.cell_change.observe_channel_request(
                    subscriber,
                    endpoint_id,
                    link_id,
                    &pdu,
                    self.current_time,
                ),
                Err(error) => {
                    tracing::warn!(?error, %subscriber, "MLE: failed to parse U-CHANNEL-REQUEST");
                    self.cell_change.record_parse_error();
                }
            },
            MlePduTypeUl::UPrepareDa => {
                unimplemented_log!("U-PREPARE-DA remains outside the conventional-access baseline")
            }
            MlePduTypeUl::UIrregularChannelAdvice => {
                unimplemented_log!("U-IRREGULAR-CHANNEL-ADVICE")
            }
            MlePduTypeUl::UChannelClassAdvice => {
                unimplemented_log!("U-CHANNEL-CLASS-ADVICE")
            }
            MlePduTypeUl::ExtPdu => unimplemented_log!("MLE uplink extension PDU"),
        }
    }

    fn rx_tla_prim(&mut self, queue: &mut MessageQueue, message: SapMsg) {
        tracing::trace!("rx_tla_prim");
        match message.msg {
            SapMsgInner::TlaTlDataIndBl(_) => {
                self.rx_tla_data_ind_bl(queue, message);
            }
            SapMsgInner::TlaTlUnitdataIndBl(_) => {
                self.rx_tla_unitdata_ind_bl(queue, message);
            }
            _ => {
                tracing::error!("BUG: unexpected message or state -- routing error");
                return;
            }
        }
    }

    /// Route unacknowledged basic-link traffic. SNDCP SN-UNITDATA uses this path;
    /// dropping TL-UNITDATA here leaves PDP activation working while every browser request
    /// disappears between LLC and SNDCP.
    fn rx_tla_unitdata_ind_bl(&mut self, queue: &mut MessageQueue, mut message: SapMsg) {
        let SapMsgInner::TlaTlUnitdataIndBl(prim) = &mut message.msg else {
            tracing::error!("BUG: unexpected message or state -- routing error");
            return;
        };
        let Some(mut sdu) = prim.tl_sdu.take() else {
            tracing::warn!("MLE: rx_tla_unitdata_ind_bl received message with no tl_sdu, ignoring");
            return;
        };
        if sdu.get_pos() != 0 {
            tracing::warn!(
                "MLE: rx_tla_unitdata_ind_bl sdu not at start position (pos={}), seeking to 0",
                sdu.get_pos()
            );
            sdu.seek(0);
        }
        let Some(bits) = sdu.read_bits(3) else {
            tracing::warn!("insufficient bits: {}", sdu.dump_bin());
            return;
        };
        let Ok(pdu_type) = MleProtocolDiscriminator::try_from(bits) else {
            tracing::warn!("invalid pdu type: {} in {}", bits, sdu.dump_bin());
            return;
        };

        match pdu_type {
            MleProtocolDiscriminator::Sndcp => {
                self.ltpd.observe_inbound(
                    prim.main_address,
                    prim.endpoint_id,
                    prim.link_id,
                    prim.air_interface_encryption != 0,
                    self.current_time,
                );
                let indication = LtpdMleUnitdataInd {
                    sdu,
                    endpoint_id: prim.endpoint_id,
                    link_id: prim.link_id,
                    received_tetra_address: prim.main_address,
                    received_address_type: ReceivedAddressType::from_tetra_address(prim.main_address),
                    chan_change_resp_req: prim.chan_change_resp_req,
                    chan_change_handle: prim
                        .chan_change_handle
                        .and_then(|value| u32::try_from(value).ok())
                        .map(ChannelChangeHandle),
                };
                queue.push_back(SapMsg {
                    sap: Sap::TlpdSap,
                    src: TetraEntity::Mle,
                    dest: TetraEntity::Sndcp,
                    msg: SapMsgInner::LtpdMleUnitdataInd(indication),
                });
            }
            other => {
                tracing::warn!("MLE: unsupported TL-UNITDATA discriminator {:?}, ignoring", other);
            }
        }
    }

    fn rx_tla_data_ind_bl(&mut self, queue: &mut MessageQueue, mut message: SapMsg) {
        // Take ownership of bitbuf and read protocol discriminator
        let SapMsgInner::TlaTlDataIndBl(prim) = &mut message.msg else {
            tracing::error!("BUG: unexpected message or state -- routing error");
            return;
        };
        let Some(mut sdu) = prim.tl_sdu.take() else {
            tracing::warn!("MLE: rx_tla_data_ind_bl received message with no tl_sdu, ignoring");
            return;
        };
        if sdu.get_pos() != 0 {
            tracing::warn!(
                "MLE: rx_tla_data_ind_bl sdu not at start position (pos={}), seeking to 0",
                sdu.get_pos()
            );
            sdu.seek(0);
        }
        let Some(bits) = sdu.read_bits(3) else {
            tracing::warn!("insufficient bits: {}", sdu.dump_bin());
            return;
        };
        let Ok(pdu_type) = MleProtocolDiscriminator::try_from(bits) else {
            tracing::warn!("invalid pdu type: {} in {}", bits, sdu.dump_bin());
            return;
        };

        // Dispatch to appropriate component (or to self if for MLE)
        match pdu_type {
            MleProtocolDiscriminator::Mm => {
                let m = LmmMleUnitdataInd {
                    sdu,
                    handle: 0,
                    received_address: prim.main_address,
                };
                let msg = SapMsg {
                    sap: Sap::LmmSap,
                    src: TetraEntity::Mle,
                    dest: TetraEntity::Mm,
                    msg: SapMsgInner::LmmMleUnitdataInd(m),
                };
                queue.push_back(msg);
            }
            MleProtocolDiscriminator::Cmce => {
                let m = LcmcMleUnitdataInd {
                    sdu,
                    handle: 0,
                    received_tetra_address: prim.main_address,
                    endpoint_id: prim.endpoint_id,
                    link_id: prim.link_id,
                    chan_change_resp_req: false, // TODO FIXME
                    chan_change_handle: None,    // TODO FIXME
                };
                let msg = SapMsg {
                    sap: Sap::LcmcSap,
                    src: TetraEntity::Mle,
                    dest: TetraEntity::Cmce,
                    msg: SapMsgInner::LcmcMleUnitdataInd(m),
                };
                queue.push_back(msg);
            }
            MleProtocolDiscriminator::Sndcp => {
                self.ltpd.observe_inbound(
                    prim.main_address,
                    prim.endpoint_id,
                    prim.link_id,
                    prim.air_interface_encryption != 0,
                    self.current_time,
                );
                let m = LtpdMleUnitdataInd {
                    sdu,
                    endpoint_id: prim.endpoint_id,
                    link_id: prim.link_id,
                    received_tetra_address: prim.main_address,
                    received_address_type: ReceivedAddressType::from_tetra_address(prim.main_address),
                    chan_change_resp_req: false, // TODO FIXME
                    chan_change_handle: None,    // TODO FIXME
                };
                // SNDCP (packet data, MLE protocol discriminator 4) belongs to the SNDCP entity,
                // not CMCE. Route it over the TLPD SAP so the packet-data layer receives it.
                let msg = SapMsg {
                    sap: Sap::TlpdSap,
                    src: TetraEntity::Mle,
                    dest: TetraEntity::Sndcp,
                    msg: SapMsgInner::LtpdMleUnitdataInd(m),
                };
                queue.push_back(msg);
            }
            MleProtocolDiscriminator::Mle => {
                self.rx_tla_mle_pdu(
                    queue,
                    prim.main_address,
                    prim.endpoint_id,
                    prim.link_id,
                    sdu,
                );
            }
            MleProtocolDiscriminator::TetraManagementEntity => {
                unimplemented_log!("MleProtocolDiscriminator::TetraManagementEntity");
            }
        }
    }

    fn queue_cell_change_outbound(
        &mut self,
        queue: &mut MessageQueue,
        mut outbound: MleCellChangeOutbound,
    ) {
        outbound.pdu.seek(0);
        let pdu_len = outbound.pdu.get_len();
        let mut tl_sdu = BitBuffer::new(3 + pdu_len);
        tl_sdu.write_bits(MleProtocolDiscriminator::Mle.into_raw(), 3);
        tl_sdu.copy_bits(&mut outbound.pdu, pdu_len);
        tl_sdu.seek(0);

        queue.push_back(SapMsg {
            sap: Sap::TlaSap,
            src: TetraEntity::Mle,
            dest: TetraEntity::Llc,
            msg: SapMsgInner::TlaTlDataReqBl(TlaTlDataReqBl {
                main_address: outbound.subscriber,
                link_id: outbound.link_id,
                endpoint_id: outbound.endpoint_id,
                tl_sdu,
                stealing_permission: false,
                subscriber_class: 0,
                fcs_flag: false,
                air_interface_encryption: None,
                stealing_repeats_flag: None,
                data_class_info: None,
                req_handle: 0,
                graceful_degradation: None,
                chan_alloc: outbound.chan_alloc,
                tx_reporter: None,
            }),
        });
    }

    fn rx_cell_change_control(&mut self, queue: &mut MessageQueue, control: MleCellChangeControl) {
        match self.cell_change.handle_control(control, self.current_time) {
            Ok(outbound) => self.queue_cell_change_outbound(queue, outbound),
            Err(error) => tracing::warn!(?error, "MLE: rejected cell-change control command"),
        }
    }

    fn rx_tlmc_prim(&mut self, queue: &mut MessageQueue, message: SapMsg) {
        tracing::trace!("rx_tlmc_prim");
        match message.msg {
            SapMsgInner::TlmcConfigureInd(indication) => {
                tracing::info!(
                    "TLMC: endpoint {} resource {:?} ({:?})",
                    indication.endpoint_id,
                    indication.lower_layer_resource_availability,
                    indication.reason
                );
                match indication.lower_layer_resource_availability {
                    LowerLayerResourceAvailability::Available => self.ltpd.notify_resume(queue),
                    LowerLayerResourceAvailability::Unavailable => self.ltpd.notify_break(queue),
                }
            }
            SapMsgInner::TlmcConfigureConf(confirm) => {
                tracing::debug!("TLMC: lower-layer configuration confirmed: {:?}", confirm);
            }
            SapMsgInner::TlmcMeasurementInd(indication) => {
                tracing::trace!("TLMC: serving-channel measurement: {:?}", indication.measurement);
            }
            SapMsgInner::TlmcMonitorInd(indication) => {
                tracing::trace!("TLMC: monitor indication: {:?}", indication);
            }
            SapMsgInner::TlmcAssessmentInd(indication) => {
                tracing::trace!("TLMC: assessment indication: {:?}", indication);
            }
            SapMsgInner::TlmcScanConf(confirm) => {
                tracing::debug!("TLMC: scan completed: {:?}", confirm);
            }
            SapMsgInner::TlmcScanReportInd(indication) => {
                tracing::trace!("TLMC: scan report: {:?}", indication);
            }
            SapMsgInner::TlmcCellReadConf(confirm) => {
                tracing::debug!("TLMC: cell read completed: {:?}", confirm);
            }
            SapMsgInner::TlmcSelectInd(indication) => {
                tracing::debug!("TLMC: lower-layer channel-change indication: {:?}", indication);
            }
            SapMsgInner::TlmcSelectConf(confirm) => {
                tracing::debug!("TLMC: selection completed: {:?}", confirm);
            }
            SapMsgInner::TlmcReportInd(report) => {
                tracing::warn!("TLMC: lower-layer report: {:?}", report);
            }
            other => {
                tracing::warn!("TLMC: MLE-BS received unexpected request primitive: {:?}", other);
            }
        }
    }

    fn rx_lmm_mle_unitdata_req(&mut self, queue: &mut MessageQueue, mut message: SapMsg) {
        tracing::trace!("rx_lmm_mle_unitdata_req");
        let SapMsgInner::LmmMleUnitdataReq(prim) = &mut message.msg else {
            tracing::error!("BUG: unexpected message or state -- routing error");
            return;
        };

        let mle_prot_discriminator = MleProtocolDiscriminator::Mm;
        let sdu_len = prim.sdu.get_len();
        let mut pdu = BitBuffer::new(3 + sdu_len);
        pdu.write_bits(mle_prot_discriminator.into_raw(), 3);
        pdu.copy_bits(&mut prim.sdu, sdu_len);
        pdu.seek(0);

        if prim.layer2service == Layer2Service::Unacknowledged {
            tracing::warn!("MLE: rx_lmm_mle_unitdata_req with Unacknowledged layer2service not implemented, ignoring");
            return;
        }

        // let (addr, link, endpoint) = self.router.use_handle(prim.handle, message.dltime);
        // assert_eq!(addr.ssi, prim.address.ssi);
        let sapmsg = SapMsg {
            sap: Sap::TlaSap,
            src: TetraEntity::Mle,
            dest: TetraEntity::Llc,
            msg: SapMsgInner::TlaTlDataReqBl(TlaTlDataReqBl {
                main_address: prim.address,
                link_id: 0,
                endpoint_id: 0,
                tl_sdu: pdu,
                stealing_permission: false,
                subscriber_class: 0, // TODO fixme
                fcs_flag: false,
                air_interface_encryption: None,
                stealing_repeats_flag: None,
                data_class_info: None,
                req_handle: 0, // TODO FIXME; should we pass the same handle here?
                graceful_degradation: None,
                chan_alloc: None,
                tx_reporter: prim.tx_reporter.take(),
            }),
        };
        queue.push_back(sapmsg);
    }

    fn rx_lmm_prim(&mut self, queue: &mut MessageQueue, message: SapMsg) {
        tracing::trace!("rx_lmm_prim");
        match &message.msg {
            SapMsgInner::LmmMleUnitdataReq(_prim) => {
                self.rx_lmm_mle_unitdata_req(queue, message);
            }
            _ => {
                tracing::warn!("unhandled match variant, ignoring");
            }
        }
    }

    fn rx_tlpd_prim(&mut self, queue: &mut MessageQueue, message: SapMsg) {
        tracing::trace!("rx_tlpd_prim");
        self.ltpd.handle_primitive(queue, message, self.current_time);
    }

    fn rx_lcmc_mle_unitdata_req(&mut self, queue: &mut MessageQueue, mut message: SapMsg) {
        tracing::trace!("rx_lcmc_mle_unitdata_req");
        let SapMsgInner::LcmcMleUnitdataReq(prim) = &mut message.msg else {
            tracing::error!("BUG: unexpected message or state -- routing error");
            return;
        };

        let mle_prot_discriminator = MleProtocolDiscriminator::Cmce;
        let sdu_len = prim.sdu.get_len();
        let mut pdu = BitBuffer::new(3 + sdu_len);
        pdu.write_bits(mle_prot_discriminator.into_raw(), 3);
        pdu.copy_bits(&mut prim.sdu, sdu_len);
        pdu.seek(0);

        // let (_addr, link, endpoint) = self.router.use_handle(prim.handle, message.dltime);
        // assert_eq!(link, prim.link_id);
        // assert_eq!(endpoint, prim.endpoint_id);
        // Take Channel Allocation Request if any
        let chan_alloc = prim.chan_alloc.take();

        let sapmsg = if prim.layer2service == Layer2Service::Unacknowledged {
            // Unacknowledged service, send a TlUnitdataReqBl
            SapMsg {
                sap: Sap::TlaSap,
                src: TetraEntity::Mle,
                dest: TetraEntity::Llc,
                msg: SapMsgInner::TlaTlUnitdataReqBl(TlaTlUnitdataReqBl {
                    main_address: prim.main_address,
                    link_id: prim.link_id,
                    endpoint_id: prim.endpoint_id,
                    tl_sdu: pdu,
                    stealing_permission: prim.stealing_permission,
                    subscriber_class: 0, // TODO fixme
                    fcs_flag: false,
                    air_interface_encryption: None,
                    packet_data_flag: false,
                    n_tlsdu_repeats: 0,
                    data_class_info: None,
                    // Preserve the CMCE request handle. Most callers use zero; the
                    // frame-18 common-SCCH path uses a BS-internal marker that LLC/UMAC
                    // must retain until the scheduler pins the PDU to that control slot.
                    req_handle: prim.handle as i32,

                    chan_alloc,
                    tx_reporter: prim.tx_reporter.take(),
                }),
            }
        } else {
            // Acknowledged service, send a TlDataReqBl
            SapMsg {
                sap: Sap::TlaSap,
                src: TetraEntity::Mle,
                dest: TetraEntity::Llc,
                msg: SapMsgInner::TlaTlDataReqBl(TlaTlDataReqBl {
                    main_address: prim.main_address,
                    link_id: prim.link_id,
                    endpoint_id: prim.endpoint_id,
                    tl_sdu: pdu,
                    stealing_permission: prim.stealing_permission,
                    subscriber_class: 0, // TODO fixme
                    fcs_flag: false,
                    air_interface_encryption: None,
                    stealing_repeats_flag: None,
                    data_class_info: None,
                    req_handle: 0, // TODO FIXME
                    graceful_degradation: None,
                    chan_alloc,
                    tx_reporter: prim.tx_reporter.take(),
                }),
            }
        };

        queue.push_back(sapmsg);
    }

    fn rx_lcmc_prim(&mut self, queue: &mut MessageQueue, message: SapMsg) {
        tracing::trace!("rx_lcmc_prim");
        match &message.msg {
            SapMsgInner::LcmcMleUnitdataReq(_) => {
                self.rx_lcmc_mle_unitdata_req(queue, message);
            }
            _ => {
                tracing::warn!("unhandled match variant, ignoring");
            }
        }
    }
}

impl TetraEntityTrait for MleBs {
    fn entity(&self) -> TetraEntity {
        TetraEntity::Mle
    }

    fn tick_start(&mut self, queue: &mut MessageQueue, ts: TdmaTime) {
        self.current_time = ts;
        self.ltpd.tick(queue, ts);
        let timed_out = self.cell_change.tick(ts);
        for outbound in timed_out {
            self.queue_cell_change_outbound(queue, outbound);
        }
        // Broadcast D-NWRK-BROADCAST twice per hyperframe (~30.6s interval) if timezone is configured.
        // Two evenly-spaced slots [20, 50] avoid congestion with other hyperframe-triggered events
        // and give terminals a faster time/date update after cold attach.
        if MLE_BROADCAST_MULTIFRAMES.contains(&ts.m) && ts.f == MLE_BROADCAST_FRAME && ts.t == 1 {
            tracing::debug!("MLE: hyperframe broadcast slot (hf={} m={} f={} t={})", ts.h, ts.m, ts.f, ts.t);
            self.broadcast.send_broadcast(queue);
        }
    }

    fn rx_prim(&mut self, queue: &mut MessageQueue, message: SapMsg) {
        tracing::debug!("rx_prim: {:?}", message);
        // tracing::debug!(ts=%message.dltime, "rx_prim: {:?}", message);

        match message.sap {
            Sap::TlaSap => {
                self.rx_tla_prim(queue, message);
            }
            Sap::TlmbSap => {
                tracing::warn!("MLE: BS received unexpected broadcast message on TlmbSap, ignoring");
            }
            Sap::TlmcSap => {
                self.rx_tlmc_prim(queue, message);
            }
            Sap::LmmSap => {
                self.rx_lmm_prim(queue, message);
            }
            Sap::TlpdSap => {
                self.rx_tlpd_prim(queue, message);
            }
            Sap::LcmcSap => {
                self.rx_lcmc_prim(queue, message);
            }
            Sap::Control => match message.msg {
                SapMsgInner::MleCellChangeControl(control) => {
                    self.rx_cell_change_control(queue, control);
                }
                other => {
                    tracing::warn!(?other, "MLE: unsupported control primitive");
                }
            },
            _ => {
                tracing::error!("BUG: unexpected message or state -- routing error");
                return;
            }
        }
    }
}

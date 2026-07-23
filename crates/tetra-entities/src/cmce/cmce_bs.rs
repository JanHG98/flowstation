use crate::net_control::{ControlCommand, ControlEndpoint, ControlResponse};
use crate::net_telemetry::TelemetrySink;
use crate::{MessageQueue, TetraEntityTrait};
use tetra_config::bluestation::SharedConfig;
use tetra_core::tetra_entities::TetraEntity;
use tetra_core::{Sap, TdmaTime, unimplemented_log};
use tetra_saps::control::mle_cell_change::MleCellChangeControl;
use tetra_saps::{SapMsg, SapMsgInner};

use super::components::pc_bs::{ControlRoute, LcmcRoute, PcBs};
use super::call_restore_runtime::{CallRestoreContext, CallRestoreRuntimeSnapshot};
use super::subentities::cc_bs::CcBsSubentity;
use super::subentities::sds_bs::{SdsBsSubentity, SdsPendingAction};
use super::subentities::ss_bs::SsBsSubentity;

pub struct CmceBs {
    config: SharedConfig,
    telemetry: Option<TelemetrySink>,
    control: Option<ControlEndpoint>,
    dashboard_control: Option<ControlEndpoint>,

    pc: PcBs,
    cc: CcBsSubentity,
    sds: SdsBsSubentity,
    ss: SsBsSubentity,
}

impl CmceBs {
    pub fn new(config: SharedConfig, telemetry: Option<TelemetrySink>, control: Option<ControlEndpoint>) -> Self {
        let mut sds = SdsBsSubentity::new(config.clone());
        if let Some(ref sink) = telemetry {
            sds.set_telemetry(sink.clone());
        }

        let mut cc = CcBsSubentity::new(config.clone());
        if let Some(ref sink) = telemetry {
            cc.set_telemetry(sink.clone());
        }

        Self {
            config: config.clone(),
            telemetry,
            control,
            dashboard_control: None,
            pc: PcBs::new(),
            sds,
            cc,
            ss: SsBsSubentity::new(),
        }
    }

    pub fn set_dashboard_control(&mut self, endpoint: ControlEndpoint) {
        self.dashboard_control = Some(endpoint);
    }

    pub fn set_wx_cmd_sender(&mut self, tx: crossbeam_channel::Sender<ControlCommand>) {
        self.sds.set_wx_cmd_sender(tx);
    }

    /// Execute a single control command. Shared by both the main `control` link (where a
    /// `responder` is supplied so request/response commands can reply) and the dashboard
    /// control link (where `responder` is `None`). Unknown commands are logged, never panic —
    /// a control-plane peer must not be able to crash the base station.
    fn do_control_command(
        sds: &mut SdsBsSubentity,
        cc: &mut CcBsSubentity,
        queue: &mut MessageQueue,
        cmd: ControlCommand,
        responder: Option<&ControlEndpoint>,
    ) {
        match cmd {
            ControlCommand::SendSds { handle, .. } => {
                let success = sds.rx_sds_from_control(queue, cmd);
                if let Some(cep) = responder {
                    cep.respond(ControlResponse::SendSdsResponse { handle, success });
                }
            }
            ControlCommand::SendRawSdsType4 { handle, .. } => {
                let success = sds.rx_sds_from_control(queue, cmd);
                if let Some(cep) = responder {
                    cep.respond(ControlResponse::SendSdsResponse { handle, success });
                }
            }
            ControlCommand::DeliverSds { handle, .. } => {
                let success = sds.rx_sds_from_control(queue, cmd);
                if let Some(cep) = responder {
                    cep.respond(ControlResponse::SdsDeliveryResponse {
                        handle,
                        success,
                        message: if success {
                            "SDS accepted by local edge".to_string()
                        } else {
                            "SDS rejected by local edge".to_string()
                        },
                    });
                }
            }
            ControlCommand::SendStatus { handle, source_ssi, dest_ssi, pre_coded_status } => {
                let success = sds.send_status_from_control(
                    queue,
                    source_ssi,
                    dest_ssi,
                    pre_coded_status,
                );
                if let Some(cep) = responder {
                    cep.respond(ControlResponse::SdsDeliveryResponse {
                        handle,
                        success,
                        message: if success {
                            "status accepted by local edge".to_string()
                        } else {
                            "status rejected by local edge".to_string()
                        },
                    });
                }
            }
            ControlCommand::KickMs { issi } => {
                tracing::info!("CMCE: KickMs issi={} requested", issi);
                let success = cc.kick_ms(queue, issi);
                if let Some(cep) = responder {
                    cep.respond(ControlResponse::KickMsResponse { issi, success });
                }
            }
            ControlCommand::MobilityExportContext { .. }
            | ControlCommand::MobilityImportContext { .. }
            | ControlCommand::MobilityRemoveContext { .. }
            | ControlCommand::SubscriberAccessPolicyApply { .. }
            | ControlCommand::GroupAccessPolicyApply { .. }
            | ControlCommand::GroupDgnaApply { .. }
            | ControlCommand::PacketDataContextDeactivate { .. }
            | ControlCommand::PacketDataContextModify { .. }
            | ControlCommand::PacketDataWake { .. }
            | ControlCommand::PacketDataEndOfData { .. } => {
                tracing::warn!("CMCE: MM command reached CMCE instead of MM; ignoring");
            }
            ControlCommand::Dgna { issi, gssi, attach } => {
                // The dashboard control channel terminates at CMCE, but DGNA is a Mobility
                // Management procedure — group attach/detach state and the D-ATTACH/DETACH GROUP
                // IDENTITY send path both live in MM. Forward the request there.
                tracing::info!(
                    "CMCE: forwarding DGNA {} of GSSI {} on ISSI {} to MM",
                    if attach { "assign" } else { "deassign" },
                    gssi,
                    issi
                );
                queue.push_back(SapMsg {
                    sap: Sap::Control,
                    src: TetraEntity::Cmce,
                    dest: TetraEntity::Mm,
                    msg: SapMsgInner::MmDgnaRequest { issi, gssi, attach },
                });
            }
            ControlCommand::CallControlGroupStart {
                handle,
                operation_id,
                source_issi,
                gssi,
                priority,
            } => {
                let result = cc.control_start_group_call(
                    queue,
                    &operation_id,
                    source_issi,
                    gssi,
                    priority,
                );
                if let Some(cep) = responder {
                    cep.respond(ControlResponse::CallControlLegStarted {
                        handle,
                        operation_id,
                        kind: result.kind,
                        success: result.success,
                        call_id: result.call_id,
                        timeslot: result.timeslot,
                        usage: result.usage,
                        floor_holder: result.floor_holder,
                        message: result.message,
                    });
                }
            }
            ControlCommand::CallControlIndividualStart {
                handle,
                operation_id,
                calling_issi,
                called_issi,
                simplex,
                priority,
            } => {
                let result = cc.control_start_individual_call(
                    queue,
                    &operation_id,
                    calling_issi,
                    called_issi,
                    simplex,
                    priority,
                );
                if let Some(cep) = responder {
                    cep.respond(ControlResponse::CallControlLegStarted {
                        handle,
                        operation_id,
                        kind: result.kind,
                        success: result.success,
                        call_id: result.call_id,
                        timeslot: result.timeslot,
                        usage: result.usage,
                        floor_holder: result.floor_holder,
                        message: result.message,
                    });
                }
            }
            ControlCommand::CallControlRelease { handle, call_id, cause } => {
                let result = cc.control_release_call(queue, call_id, cause);
                if let Some(cep) = responder {
                    cep.respond(ControlResponse::CallControlLegReleased {
                        handle,
                        call_id,
                        success: result.success,
                        message: result.message,
                    });
                }
            }
            ControlCommand::CallControlFloorRequest {
                handle,
                call_id,
                source_issi,
                force,
            } => {
                let result = cc.control_request_floor(
                    queue,
                    call_id,
                    source_issi,
                    force,
                );
                if let Some(cep) = responder {
                    cep.respond(ControlResponse::CallControlFloorChanged {
                        handle,
                        call_id,
                        success: result.success,
                        floor_holder: result.floor_holder,
                        queued_issi: result.queued_issi,
                        message: result.message,
                    });
                }
            }
            ControlCommand::CallControlFloorRelease { handle, call_id } => {
                let result = cc.control_release_floor(queue, call_id);
                if let Some(cep) = responder {
                    cep.respond(ControlResponse::CallControlFloorChanged {
                        handle,
                        call_id,
                        success: result.success,
                        floor_holder: result.floor_holder,
                        queued_issi: result.queued_issi,
                        message: result.message,
                    });
                }
            }
            ControlCommand::CallControlExportRestoreContext { handle, call_id } => {
                let context = cc.control_export_restore_context(call_id);
                let found = context.is_some();
                if let Some(cep) = responder {
                    cep.respond(ControlResponse::CallControlRestoreContextExported {
                        handle,
                        call_id,
                        found,
                        context,
                        message: if found {
                            "restore context exported".to_string()
                        } else {
                            "call or restore context not found".to_string()
                        },
                    });
                }
            }
            ControlCommand::CallControlImportRestoreContext { handle, context } => {
                let call_id = match &context {
                    crate::net_control::ManagedCallRestoreContextPayload::Group { call_id, .. }
                    | crate::net_control::ManagedCallRestoreContextPayload::Individual { call_id, .. } => *call_id,
                };
                let result = cc.control_import_restore_context(context);
                if let Some(cep) = responder {
                    cep.respond(ControlResponse::CallControlRestoreContextImported {
                        handle,
                        call_id,
                        success: result.is_ok(),
                        message: result
                            .map(|_| "restore context installed".to_string())
                            .unwrap_or_else(|error| error),
                    });
                }
            }
            ControlCommand::CallControlRemoveRestoreContext { handle, call_id } => {
                let removed = cc.control_remove_restore_context(call_id);
                if let Some(cep) = responder {
                    cep.respond(ControlResponse::CallControlRestoreContextRemoved {
                        handle,
                        call_id,
                        success: removed,
                        message: if removed {
                            "restore context removed".to_string()
                        } else {
                            "restore context not found".to_string()
                        },
                    });
                }
            }
            ControlCommand::RestartService => {
                tracing::info!("CMCE: RestartService requested");
                crate::service_control::schedule_service_action(
                    crate::service_control::ServiceAction::Restart,
                    std::time::Duration::from_millis(500),
                );
            }
            ControlCommand::ShutdownService => {
                tracing::info!("CMCE: ShutdownService requested");
                crate::service_control::schedule_service_action(
                    crate::service_control::ServiceAction::Stop,
                    std::time::Duration::from_millis(500),
                );
            }
            ControlCommand::AddLiveSds {
                text,
                protocol_id,
                source_issi,
                repeat_count,
            } => {
                let mut state = sds.shared_config().state_write();
                let id = state.next_live_sds_id;
                state.next_live_sds_id = state.next_live_sds_id.wrapping_add(1).max(1);
                state.live_sds_queue.push_back(tetra_config::bluestation::LiveSdsMessage {
                    id,
                    text: text.clone(),
                    protocol_id,
                    source_issi,
                    repeat_count,
                    sent_count: 0,
                });
                tracing::info!("CMCE: AddLiveSds id={} repeat={} text={:?}", id, repeat_count, text);
            }
            ControlCommand::DeleteLiveSds { id } => {
                let mut state = sds.shared_config().state_write();
                let before = state.live_sds_queue.len();
                state.live_sds_queue.retain(|m| m.id != id);
                let removed = before - state.live_sds_queue.len();
                tracing::info!("CMCE: DeleteLiveSds id={} removed={}", id, removed);
            }
            ControlCommand::ClearLiveSds => {
                let mut state = sds.shared_config().state_write();
                let n = state.live_sds_queue.len();
                state.live_sds_queue.clear();
                tracing::info!("CMCE: ClearLiveSds removed={}", n);
            }
            ControlCommand::ClearEmergency { issi } => {
                tracing::info!("CMCE: ClearEmergency issi={} (operator)", issi);

                // First perform the SwMI/Call-Control part: emergency button calls arrive as
                // priority-15 CC calls, so clearing them from the dashboard must release the
                // traffic call, not only remove a local banner.
                let released = cc.clear_emergency_calls_for_issi(queue, issi);
                if released > 0 {
                    tracing::warn!(
                        "CMCE: ClearEmergency issi={} released {} active emergency-priority call(s)",
                        issi,
                        released
                    );
                }

                // Then clear any status-based emergency session/banner for the same ISSI. If the
                // emergency came purely from a priority-15 call this is harmless/no-op, while a
                // U-STATUS emergency still gets the existing EmergencyCancel telemetry.
                sds.clear_emergency_command(queue, issi);
            }
            _ => {
                tracing::warn!("CMCE: ignoring unsupported control command {:?}", cmd);
            }
        }
    }

    pub fn rx_lcmc_mle_unitdata_ind(&mut self, _queue: &mut MessageQueue, mut message: SapMsg) {
        tracing::trace!("rx_lcmc_mle_unitdata_ind");

        let Some(route) = self.pc.route_lcmc_unitdata_ind(&mut message) else {
            return;
        };

        match route {
            LcmcRoute::CcRd => {
                self.cc.route_rd_deliver(_queue, message);
            }
            LcmcRoute::SdsStatus => {
                self.sds.route_status_deliver(_queue, message);
            }
            LcmcRoute::SdsRf => {
                self.sds.route_rf_deliver(_queue, message);
            }
            LcmcRoute::SsRe => {
                self.ss.route_re_deliver(_queue, message);
            }
            LcmcRoute::Unsupported(pdu_type) => {
                unimplemented_log!("{:?}", pdu_type);
            }
        };
    }

    /// Handle an MLE U-RESTORE indication and return the embedded CMCE response
    /// through D-RESTORE-ACK, or a standards-defined D-RESTORE-FAIL when no call
    /// context can be restored on this cell.
    pub fn rx_lcmc_mle_restore_ind(&mut self, queue: &mut MessageQueue, message: SapMsg) {
        let SapMsgInner::LcmcMleRestoreInd(indication) = message.msg else {
            tracing::error!("CMCE: invalid primitive routed to restore indication handler");
            return;
        };

        match self.cc.handle_mle_call_restore(
            queue,
            indication.subscriber,
            indication.endpoint_id,
            indication.link_id,
            indication.previous_mcc,
            indication.previous_mnc,
            indication.previous_location_area,
            indication.sdu,
        ) {
            super::subentities::cc_bs::MleCallRestoreDecision::Acknowledge {
                cmce_sdu,
                chan_alloc,
            } => {
                queue.push_back(SapMsg {
                    sap: Sap::Control,
                    src: TetraEntity::Cmce,
                    dest: TetraEntity::Mle,
                    msg: SapMsgInner::MleCellChangeControl(MleCellChangeControl::AcknowledgeRestore {
                        subscriber: indication.subscriber,
                        cmce_sdu,
                        chan_alloc,
                    }),
                });
            }
            super::subentities::cc_bs::MleCallRestoreDecision::Reject(cause) => {
                queue.push_back(SapMsg {
                    sap: Sap::Control,
                    src: TetraEntity::Cmce,
                    dest: TetraEntity::Mle,
                    msg: SapMsgInner::MleCellChangeControl(MleCellChangeControl::RejectRestore {
                        subscriber: indication.subscriber,
                        cause,
                    }),
                });
            }
        }
    }

    /// Read-only restore diagnostics for the TBS WebUI and Node Gateway.
    pub fn call_restore_snapshot(&self) -> CallRestoreRuntimeSnapshot {
        self.cc.call_restore_snapshot()
    }

    /// Install a call context received from the source TBS or future call core.
    pub fn install_call_restore_context(&mut self, context: CallRestoreContext) {
        self.cc.install_call_restore_context(context);
    }

    /// Export an active local call as a transferable restore context.
    pub fn export_call_restore_context(&self, call_id: u16) -> Option<CallRestoreContext> {
        self.cc.export_call_restore_context(call_id)
    }
}

impl TetraEntityTrait for CmceBs {
    fn entity(&self) -> TetraEntity {
        TetraEntity::Cmce
    }

    fn set_config(&mut self, config: SharedConfig) {
        self.config = config;
    }

    fn tick_start(&mut self, queue: &mut MessageQueue, ts: TdmaTime) {
        // Propagate tick to subentities
        self.cc.tick_start(queue, ts);
        // Republish the in-call ISSI→timeslot map so SDS can FACCH-steal to in-call radios
        // (FH-BUG-034). Rebuilt from the live call tables every tick.
        self.cc.publish_active_call_ts();
        self.sds.tick_start(queue, ts);
        self.sds.tick_periodic_wx();

        // Process incoming control commands, if the main control link is enabled (request/response).
        if let Some(cep) = &self.control {
            while let Some(cmd) = cep.try_recv() {
                CmceBs::do_control_command(&mut self.sds, &mut self.cc, queue, cmd, Some(cep));
            }
        }
        // Process commands from the dashboard control link (fire-and-forget, no responder).
        if let Some(cep) = &self.dashboard_control {
            while let Some(cmd) = cep.try_recv() {
                CmceBs::do_control_command(&mut self.sds, &mut self.cc, queue, cmd, None);
            }
        }

        // Drain SDS-triggered actions that require access to CcBsSubentity.
        let pending = std::mem::take(&mut self.sds.pending_actions);
        for action in pending {
            match action {
                SdsPendingAction::KickAll => {
                    let issis: Vec<u32> = self.cc.subscriber_issis();
                    tracing::info!("SDS-CMD: kick_all — deregistering {} subscribers", issis.len());
                    for issi in issis {
                        self.cc.kick_ms(queue, issi);
                    }
                }
            }
        }
    }

    fn rx_prim(&mut self, queue: &mut MessageQueue, message: SapMsg) {
        tracing::debug!("rx_prim: {:?}", message);
        // tracing::debug!(ts=%message.dltime, "rx_prim: {:?}", message);

        match message.sap {
            Sap::LcmcSap => match message.msg {
                SapMsgInner::LcmcMleUnitdataInd(_) => {
                    self.rx_lcmc_mle_unitdata_ind(queue, message);
                }
                SapMsgInner::LcmcMleRestoreInd(_) => {
                    self.rx_lcmc_mle_restore_ind(queue, message);
                }
                _ => {
                    panic!("Unexpected message on LcmcSap: {:?}", message.msg);
                }
            },
            Sap::Control => match self.pc.route_control(&message) {
                ControlRoute::CcRa => {
                    self.cc.rx_call_control(queue, message);
                }
                ControlRoute::CcSubscriberUpdate => {
                    let SapMsgInner::MmSubscriberUpdate(update) = message.msg else {
                        unreachable!();
                    };
                    self.sds.handle_subscriber_update(queue, &update);
                    self.cc.handle_subscriber_update(queue, update);
                }
                ControlRoute::SdsRc => {
                    self.sds.rx_sds_from_brew(queue, message);
                }
                ControlRoute::Unsupported => {
                    panic!("Unexpected control message: {:?}", message.msg);
                }
            },
            _ => {
                panic!("Unexpected SAP: {:?}", message.sap);
            }
        }
    }
}

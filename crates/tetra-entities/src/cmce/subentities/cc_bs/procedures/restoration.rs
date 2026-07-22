use super::*;
use crate::cmce::call_restore_runtime::{
    CallRestoreRequest, CallRestoreRuntimeError, GroupRestoreOrigin, RestoreCallKind,
    RestoreRejectReason,
};
use tetra_saps::common::MleFailCause;

#[derive(Debug)]
pub(in crate::cmce) enum MleCallRestoreDecision {
    Acknowledge {
        cmce_sdu: BitBuffer,
        chan_alloc: Option<CmceChanAllocReq>,
    },
    Reject(MleFailCause),
}

#[derive(Debug)]
struct RestoredCall {
    old_call_id: u16,
    new_call_id: u16,
    grant: TransmissionGrant,
    call_status: CallStatus,
    chan_alloc: Option<CmceChanAllocReq>,
}

impl CcBsSubentity {
    pub fn call_restore_snapshot(&self) -> CallRestoreRuntimeSnapshot {
        self.call_restore.snapshot(self.dltime)
    }

    pub fn install_call_restore_context(&mut self, context: CallRestoreContext) {
        self.call_restore.install_context(context);
    }

    pub fn remove_call_restore_context(&mut self, call_id: u16) -> Option<CallRestoreContext> {
        self.call_restore.remove_context(call_id)
    }

    pub fn export_call_restore_context(&self, call_id: u16) -> Option<CallRestoreContext> {
        if let Some(call) = self.active_calls.get(&call_id) {
            let circuit = self
                .circuits
                .dl
                .get(call.ts.saturating_sub(1) as usize)
                .and_then(Option::as_ref);
            let origin = match &call.origin {
                CallOrigin::Local { caller_addr } => GroupRestoreOrigin::Local {
                    caller: *caller_addr,
                },
                CallOrigin::Network {
                    network_entity,
                    brew_uuid,
                } => GroupRestoreOrigin::Network {
                    network_entity: *network_entity,
                    brew_uuid: *brew_uuid,
                },
            };
            return Some(CallRestoreContext::Group(GroupCallRestoreContext {
                call_id,
                dest_gssi: call.dest_gssi,
                source_issi: call.source_issi,
                floor_holder: call.tx_active.then_some(call.source_issi),
                priority: call.priority,
                call_timeout: call.call_timeout,
                created_at: call.created_at,
                tx_active: call.tx_active,
                origin,
                communication_type: circuit
                    .map(|value| value.comm_type)
                    .unwrap_or(CommunicationType::P2Mp),
                circuit_mode_type: circuit
                    .map(|value| value.circuit_mode)
                    .unwrap_or(CircuitModeType::TchS),
                speech_service: circuit.and_then(|value| value.speech_service),
                etee_encrypted: circuit.is_some_and(|value| value.etee_encrypted),
            }));
        }

        if let Some(call) = self.individual_calls.get(&call_id) {
            let circuit = self
                .circuits
                .dl
                .get(call.calling_ts.saturating_sub(1) as usize)
                .and_then(Option::as_ref);
            return Some(CallRestoreContext::Individual(IndividualCallRestoreContext {
                call_id,
                calling_addr: call.calling_addr,
                called_addr: call.called_addr,
                simplex_duplex: call.simplex_duplex,
                priority: call.priority,
                call_timeout: call.call_timeout,
                active_timer_started: call.active_timer_started,
                floor_holder: call.floor_holder,
                called_over_brew: call.called_over_brew,
                calling_over_brew: call.calling_over_brew,
                brew_uuid: call.brew_uuid,
                network_entity: call.network_entity,
                network_call: call.network_call.clone(),
                communication_type: circuit
                    .map(|value| value.comm_type)
                    .unwrap_or(CommunicationType::P2p),
                circuit_mode_type: circuit
                    .map(|value| value.circuit_mode)
                    .unwrap_or(CircuitModeType::TchS),
                speech_service: circuit.and_then(|value| value.speech_service),
                etee_encrypted: circuit.is_some_and(|value| value.etee_encrypted),
            }));
        }

        self.call_restore.context(call_id).cloned()
    }

    pub(in crate::cmce) fn handle_mle_call_restore(
        &mut self,
        queue: &mut MessageQueue,
        sender: TetraAddress,
        endpoint_id: u32,
        link_id: u32,
        previous_mcc: Option<u16>,
        previous_mnc: Option<u16>,
        previous_location_area: Option<u16>,
        mut sdu: BitBuffer,
    ) -> MleCallRestoreDecision {
        let pdu = match UCallRestore::from_bitbuf(&mut sdu) {
            Ok(pdu) => pdu,
            Err(error) => {
                tracing::warn!(?error, %sender, "CMCE: malformed embedded U-CALL RESTORE");
                return MleCallRestoreDecision::Reject(MleFailCause::RestorationCannotBeDoneOnCell);
            }
        };

        match self.process_call_restore(
            queue,
            sender,
            endpoint_id,
            link_id,
            previous_mcc,
            previous_mnc,
            previous_location_area,
            pdu,
        ) {
            Ok(restored) => MleCallRestoreDecision::Acknowledge {
                cmce_sdu: Self::build_d_call_restore_extended(
                    restored.old_call_id,
                    restored.grant,
                    (restored.new_call_id != restored.old_call_id).then_some(restored.new_call_id),
                    None, // Preserve the already-running T310 across restoration.
                    Some(restored.call_status),
                ),
                chan_alloc: restored.chan_alloc,
            },
            Err(reason) => {
                tracing::info!(?reason, %sender, "CMCE: MLE call restoration rejected");
                MleCallRestoreDecision::Reject(Self::mle_fail_cause_for_restore_reject(reason))
            }
        }
    }

    pub(in crate::cmce::subentities::cc_bs) fn fsm_on_u_call_restore(
        &mut self,
        queue: &mut MessageQueue,
        sender: TetraAddress,
        handle: u32,
        link_id: u32,
        endpoint_id: u32,
        pdu: UCallRestore,
    ) {
        let old_call_id = pdu.call_identifier;
        match self.process_call_restore(queue, sender, endpoint_id, link_id, None, None, None, pdu) {
            Ok(restored) => {
                let sdu = Self::build_d_call_restore_extended(
                    restored.old_call_id,
                    restored.grant,
                    (restored.new_call_id != restored.old_call_id).then_some(restored.new_call_id),
                    None, // Preserve the already-running T310 across restoration.
                    Some(restored.call_status),
                );
                let changed_call_id = restored.new_call_id != restored.old_call_id;
                let no_channel_allocation = restored.chan_alloc.is_none();
                let mut response = Self::build_sapmsg_direct_with_allocation(
                    sdu,
                    self.dltime,
                    sender,
                    handle,
                    link_id,
                    endpoint_id,
                    restored.chan_alloc,
                );
                // ETSI recommends acknowledged layer 2 delivery when D-CALL RESTORE
                // changes the call identifier but does not yet carry a channel
                // allocation. This is the queued/congested restore path.
                if changed_call_id && no_channel_allocation {
                    if let SapMsgInner::LcmcMleUnitdataReq(ref mut primitive) = response.msg {
                        primitive.layer2service = Layer2Service::Acknowledged;
                    }
                }
                queue.push_back(response);
            }
            Err(reason) => {
                tracing::info!(?reason, "CMCE: rejecting direct U-CALL RESTORE call_id={}", old_call_id);
                let sdu = Self::build_d_release(
                    old_call_id,
                    DisconnectCause::CallRestorationOfTheOtherUserFailed,
                );
                queue.push_back(Self::build_sapmsg_direct(
                    sdu,
                    self.dltime,
                    sender,
                    handle,
                    link_id,
                    endpoint_id,
                ));
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn process_call_restore(
        &mut self,
        queue: &mut MessageQueue,
        sender: TetraAddress,
        endpoint_id: u32,
        link_id: u32,
        previous_mcc: Option<u16>,
        previous_mnc: Option<u16>,
        previous_location_area: Option<u16>,
        pdu: UCallRestore,
    ) -> Result<RestoredCall, RestoreRejectReason> {
        let old_call_id = pdu.call_identifier;
        let other_party_ssi = pdu.other_party_ssi.map(|value| value as u32);
        let request = CallRestoreRequest {
            subscriber: sender,
            old_call_id,
            endpoint_id,
            link_id,
            request_to_transmit: pdu.request_to_transmit_send_data,
            other_party_ssi,
            previous_mcc,
            previous_mnc,
            previous_location_area,
        };
        let key = match self.call_restore.begin(request, self.dltime) {
            Ok(key) => key,
            Err(CallRestoreRuntimeError::DuplicateTerminal(key)) => {
                let Some(transaction) = self.call_restore.transaction(key) else {
                    return Err(RestoreRejectReason::DuplicateRequest);
                };
                if transaction.phase != crate::cmce::call_restore_runtime::RestorePhase::Restored {
                    return Err(transaction
                        .reject_reason
                        .unwrap_or(RestoreRejectReason::InvalidState));
                }
                let Some(new_call_id) = transaction.new_call_id else {
                    return Err(RestoreRejectReason::DuplicateRequest);
                };
                let Some(grant) = transaction.transmission_grant else {
                    return Err(RestoreRejectReason::DuplicateRequest);
                };
                let chan_alloc = transaction.timeslot.map(|timeslot| {
                    Self::chan_alloc_for_ts(
                        transaction.usage,
                        timeslot,
                        ChanAllocType::Replace,
                        UlDlAssignment::Both,
                    )
                });
                return Ok(RestoredCall {
                    old_call_id,
                    new_call_id,
                    grant,
                    call_status: CallStatus::Callcontinue,
                    chan_alloc,
                });
            }
            Err(CallRestoreRuntimeError::DuplicateQueued(key)) => {
                let Some(transaction) = self.call_restore.transaction(key) else {
                    return Err(RestoreRejectReason::DuplicateRequest);
                };
                return Ok(RestoredCall {
                    old_call_id,
                    new_call_id: transaction.new_call_id.unwrap_or(old_call_id),
                    grant: transaction
                        .transmission_grant
                        .unwrap_or(TransmissionGrant::NotGranted),
                    call_status: CallStatus::Callqueued,
                    chan_alloc: None,
                });
            }
            Err(CallRestoreRuntimeError::DuplicatePending(_)) => {
                return Err(RestoreRejectReason::DuplicateRequest)
            }
            Err(CallRestoreRuntimeError::UnknownTransaction(_)
            | CallRestoreRuntimeError::InvalidPhase { .. }) => {
                return Err(RestoreRejectReason::InvalidState)
            }
        };

        let context = self
            .export_call_restore_context(old_call_id)
            .or_else(|| self.call_restore.context(old_call_id).cloned());
        let Some(context) = context else {
            let _ = self
                .call_restore
                .reject(key, RestoreRejectReason::UnknownCall, self.dltime);
            return Err(RestoreRejectReason::UnknownCall);
        };

        if !context.permits_subscriber(sender, other_party_ssi) {
            let _ = self
                .call_restore
                .reject(key, RestoreRejectReason::ParticipantMismatch, self.dltime);
            return Err(RestoreRejectReason::ParticipantMismatch);
        }
        if !Self::restore_service_matches(&context, pdu.basic_service_information.as_ref()) {
            let _ = self
                .call_restore
                .reject(key, RestoreRejectReason::ServiceMismatch, self.dltime);
            return Err(RestoreRejectReason::ServiceMismatch);
        }

        self.call_restore
            .mark_context_matched(key, context.kind(), self.dltime)
            .map_err(|_| RestoreRejectReason::InvalidState)?;

        let result = match context {
            CallRestoreContext::Group(context) => {
                self.restore_group_call(queue, key, sender, pdu.request_to_transmit_send_data, context)
            }
            CallRestoreContext::Individual(context) => {
                self.restore_individual_call(queue, key, sender, endpoint_id, link_id, pdu.request_to_transmit_send_data, context)
            }
        };

        match result {
            Ok(restored) => Ok(restored),
            Err(RestoreRejectReason::NoRadioResource) => {
                let local_call_id = self.reserve_restore_call_id(old_call_id);
                let grant = if pdu.request_to_transmit_send_data {
                    TransmissionGrant::RequestQueued
                } else {
                    TransmissionGrant::NotGranted
                };
                self.call_restore
                    .mark_queued(key, local_call_id, grant, self.dltime)
                    .map_err(|_| RestoreRejectReason::InvalidState)?;
                Ok(RestoredCall {
                    old_call_id,
                    new_call_id: local_call_id,
                    grant,
                    call_status: CallStatus::Callqueued,
                    chan_alloc: None,
                })
            }
            Err(reason) => {
                let _ = self.call_restore.reject(key, reason, self.dltime);
                Err(reason)
            }
        }
    }

    fn restore_group_call(
        &mut self,
        queue: &mut MessageQueue,
        key: crate::cmce::call_restore_runtime::RestoreTransactionKey,
        sender: TetraAddress,
        request_to_transmit: bool,
        context: GroupCallRestoreContext,
    ) -> Result<RestoredCall, RestoreRejectReason> {
        let old_call_id = context.call_id;
        let local_call_id = self
            .call_restore
            .resolved_call_id(old_call_id)
            .unwrap_or(old_call_id);
        let (new_call_id, ts, usage) = if let Some(call) = self.active_calls.get_mut(&local_call_id) {
            call.begin_restore()
                .map_err(|_| RestoreRejectReason::InvalidState)?;
            (local_call_id, call.ts, call.usage)
        } else {
            self.preempt_for_priority(queue, 1, context.priority);
            let collision = self.individual_calls.contains_key(&local_call_id)
                || self.active_calls.contains_key(&local_call_id)
                || self
                    .circuits
                    .dl
                    .iter()
                    .flatten()
                    .any(|circuit| circuit.call_id == local_call_id)
                || self
                    .circuits
                    .ul_only
                    .iter()
                    .flatten()
                    .any(|circuit| circuit.call_id == local_call_id);
            let traffic_slot_capacity = self.traffic_slot_capacity();
            let circuit = {
                let mut state = self.config.state_write();
                let allocated = if collision {
                    self.circuits.allocate_circuit_with_capacity(
                        Direction::Both,
                        context.communication_type,
                        false,
                        &mut state.timeslot_alloc,
                        TimeslotOwner::Cmce,
                        traffic_slot_capacity,
                    )
                } else {
                    self.circuits.allocate_circuit_for_call_with_capacity(
                        local_call_id,
                        Direction::Both,
                        context.communication_type,
                        false,
                        &mut state.timeslot_alloc,
                        TimeslotOwner::Cmce,
                        traffic_slot_capacity,
                    )
                };
                allocated.cloned().map_err(|_| RestoreRejectReason::NoRadioResource)?
            };

            Self::signal_umac_circuit_open(
                queue,
                &circuit,
                self.dltime,
                None,
                if request_to_transmit && sender.ssi == context.source_issi {
                    CircuitDlMediaSource::LocalLoopback
                } else {
                    CircuitDlMediaSource::SwMI
                },
            );

            let mut call = match context.origin.clone() {
                GroupRestoreOrigin::Local { caller } => ActiveCall::new_local(
                    caller,
                    context.dest_gssi,
                    context.source_issi,
                    circuit.ts,
                    circuit.usage,
                    context.created_at,
                    context.call_timeout,
                    context.priority,
                ),
                GroupRestoreOrigin::Network {
                    network_entity,
                    brew_uuid,
                } => ActiveCall::new_network(
                    network_entity,
                    brew_uuid,
                    context.dest_gssi,
                    context.source_issi,
                    circuit.ts,
                    circuit.usage,
                    context.created_at,
                    context.call_timeout,
                    context.priority,
                ),
            };
            if !context.tx_active {
                call.enter_hangtime(self.dltime);
            }
            if let Some(holder) = context.floor_holder {
                call.source_issi = holder;
                call.tx_active = context.tx_active;
            }
            call.begin_restore()
                .map_err(|_| RestoreRejectReason::InvalidState)?;
            self.install_restored_group_setup(circuit.call_id, &context);
            self.active_calls.insert(circuit.call_id, call);
            self.emit(crate::net_telemetry::TelemetryEvent::GroupCallStarted {
                call_id: circuit.call_id,
                gssi: context.dest_gssi,
                caller_issi: context.source_issi,
                ts: circuit.ts,
                carrier_num: self.carrier_num_for_logical_ts(circuit.ts),
                priority: context.priority,
                source: "restored".to_string(),
            });
            (circuit.call_id, circuit.ts, circuit.usage)
        };

        // Record the local bearer for every restored participant, including
        // additional participants joining an already restored group call. This
        // keeps replayed D-CALL RESTORE responses able to carry the allocation.
        self.call_restore
            .mark_resource_allocated(key, new_call_id, ts, usage, self.dltime)
            .map_err(|_| RestoreRejectReason::InvalidState)?;

        let (grant, floor_granted, floor_released, dest_gssi, source_issi, brew_notification) = {
            let call = self
                .active_calls
                .get_mut(&new_call_id)
                .ok_or(RestoreRejectReason::InvalidState)?;
            let sender_was_speaker = call.tx_active && call.source_issi == sender.ssi;
            let grant = if call.tx_active && call.source_issi != sender.ssi {
                if request_to_transmit {
                    let _ = call.queue_tx_demand(sender);
                }
                TransmissionGrant::GrantedToOtherUser
            } else if request_to_transmit {
                call.grant_floor(sender.ssi, Some(sender));
                TransmissionGrant::Granted
            } else {
                if sender_was_speaker {
                    call.enter_hangtime(self.dltime);
                }
                TransmissionGrant::NotGranted
            };
            let source_issi = call.source_issi;
            (
                grant,
                grant == TransmissionGrant::Granted,
                sender_was_speaker && !request_to_transmit,
                call.dest_gssi,
                source_issi,
                Self::brew_notification_for_group_call(call, source_issi),
            )
        };

        if floor_granted {
            self.notify_floor_granted(
                queue,
                GroupFloorGrant {
                    call_id: new_call_id,
                    source_issi,
                    dest_gssi,
                    dest_is_group: true,
                    ts,
                },
                true,
                brew_notification,
            );
        } else if floor_released {
            self.notify_floor_released(
                queue,
                CallTimeslot {
                    call_id: new_call_id,
                    ts,
                },
                true,
                brew_notification,
            );
        }
        if let Some(call) = self.active_calls.get_mut(&new_call_id) {
            call.complete_restore();
        }
        self.call_restore
            .mark_restored(key, new_call_id, grant, self.dltime)
            .map_err(|_| RestoreRejectReason::InvalidState)?;
        Ok(RestoredCall {
            old_call_id,
            new_call_id,
            grant,
            call_status: CallStatus::Callcontinue,
            chan_alloc: Some(Self::chan_alloc_for_ts(
                Some(usage),
                ts,
                ChanAllocType::Replace,
                UlDlAssignment::Both,
            )),
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn restore_individual_call(
        &mut self,
        queue: &mut MessageQueue,
        key: crate::cmce::call_restore_runtime::RestoreTransactionKey,
        sender: TetraAddress,
        endpoint_id: u32,
        link_id: u32,
        request_to_transmit: bool,
        context: IndividualCallRestoreContext,
    ) -> Result<RestoredCall, RestoreRejectReason> {
        let old_call_id = context.call_id;
        let local_call_id = self
            .call_restore
            .resolved_call_id(old_call_id)
            .unwrap_or(old_call_id);
        let (new_call_id, ts, usage) = if let Some(call) = self.individual_calls.get_mut(&local_call_id) {
            call.begin_restore()
                .map_err(|_| RestoreRejectReason::InvalidState)?;
            let (ts, usage) = if sender.ssi == call.calling_addr.ssi {
                (call.calling_ts, call.calling_usage)
            } else {
                (call.called_ts, call.called_usage)
            };
            (local_call_id, ts, usage)
        } else {
            self.preempt_for_priority(queue, 1, context.priority);
            let collision = self.individual_calls.contains_key(&local_call_id)
                || self.active_calls.contains_key(&local_call_id)
                || self
                    .circuits
                    .dl
                    .iter()
                    .flatten()
                    .any(|circuit| circuit.call_id == local_call_id)
                || self
                    .circuits
                    .ul_only
                    .iter()
                    .flatten()
                    .any(|circuit| circuit.call_id == local_call_id);
            let traffic_slot_capacity = self.traffic_slot_capacity();
            let circuit = {
                let mut state = self.config.state_write();
                let allocated = if collision {
                    self.circuits.allocate_circuit_with_capacity(
                        Direction::Both,
                        context.communication_type,
                        context.simplex_duplex,
                        &mut state.timeslot_alloc,
                        TimeslotOwner::Cmce,
                        traffic_slot_capacity,
                    )
                } else {
                    self.circuits.allocate_circuit_for_call_with_capacity(
                        local_call_id,
                        Direction::Both,
                        context.communication_type,
                        context.simplex_duplex,
                        &mut state.timeslot_alloc,
                        TimeslotOwner::Cmce,
                        traffic_slot_capacity,
                    )
                };
                allocated.cloned().map_err(|_| RestoreRejectReason::NoRadioResource)?
            };
            Self::signal_umac_circuit_open(
                queue,
                &circuit,
                self.dltime,
                None,
                CircuitDlMediaSource::SwMI,
            );

            let sender_is_calling = sender.ssi == context.calling_addr.ssi;
            let mut call = IndividualCall {
                calling_addr: context.calling_addr,
                called_addr: context.called_addr,
                calling_handle: 0,
                calling_link_id: if sender_is_calling { link_id } else { 0 },
                calling_endpoint_id: if sender_is_calling { endpoint_id } else { 0 },
                called_handle: (!sender_is_calling).then_some(0),
                called_link_id: (!sender_is_calling).then_some(link_id),
                called_endpoint_id: (!sender_is_calling).then_some(endpoint_id),
                calling_ts: circuit.ts,
                called_ts: circuit.ts,
                calling_usage: circuit.usage,
                called_usage: circuit.usage,
                simplex_duplex: context.simplex_duplex,
                priority: context.priority,
                state: IndividualCallState::Active,
                formal_state: CcFormalState::Active,
                setup_timer_started: None,
                setup_timeout: None,
                active_timer_started: context.active_timer_started.or(Some(self.dltime)),
                call_timeout: context.call_timeout,
                called_over_brew: context.called_over_brew,
                calling_over_brew: context.calling_over_brew,
                brew_uuid: context.brew_uuid,
                network_entity: context.network_entity,
                network_call: context.network_call.clone(),
                connect_request_sent: false,
                floor_holder: context.floor_holder,
                queued_tx_demand: None,
            };
            call.begin_restore()
                .map_err(|_| RestoreRejectReason::InvalidState)?;
            self.install_restored_individual_setup(circuit.call_id, &context);
            self.individual_calls.insert(circuit.call_id, call);
            self.emit(crate::net_telemetry::TelemetryEvent::IndividualCallStarted {
                call_id: circuit.call_id,
                calling_issi: context.calling_addr.ssi,
                called_issi: context.called_addr.ssi,
                simplex: !context.simplex_duplex,
                ts: circuit.ts,
                carrier_num: self.carrier_num_for_logical_ts(circuit.ts),
                priority: context.priority,
                source: "restored".to_string(),
            });
            (circuit.call_id, circuit.ts, circuit.usage)
        };

        self.call_restore
            .mark_resource_allocated(key, new_call_id, ts, usage, self.dltime)
            .map_err(|_| RestoreRejectReason::InvalidState)?;

        let (grant, floor_granted, floor_released) = {
            let call = self
                .individual_calls
                .get_mut(&new_call_id)
                .ok_or(RestoreRejectReason::InvalidState)?;
            if sender.ssi == call.calling_addr.ssi {
                call.calling_link_id = link_id;
                call.calling_endpoint_id = endpoint_id;
            } else {
                call.called_link_id = Some(link_id);
                call.called_endpoint_id = Some(endpoint_id);
            }

            if !call.is_simplex() {
                (TransmissionGrant::Granted, false, false)
            } else {
                let sender_had_floor = call.floor_holder == Some(sender.ssi);
                let grant = match call.floor_holder {
                    Some(holder) if holder != sender.ssi => {
                        if request_to_transmit {
                            let _ = call.queue_tx_demand(sender);
                        }
                        TransmissionGrant::GrantedToOtherUser
                    }
                    _ if request_to_transmit => {
                        call.grant_floor(sender);
                        TransmissionGrant::Granted
                    }
                    _ => {
                        if sender_had_floor {
                            call.release_floor();
                        }
                        TransmissionGrant::NotGranted
                    }
                };
                (
                    grant,
                    grant == TransmissionGrant::Granted,
                    sender_had_floor && !request_to_transmit,
                )
            }
        };

        if floor_granted {
            queue.push_back(SapMsg {
                sap: Sap::Control,
                src: TetraEntity::Cmce,
                dest: TetraEntity::Umac,
                msg: SapMsgInner::CmceCallControl(CallControl::FloorGranted {
                    call_id: new_call_id,
                    source_issi: sender.ssi,
                    dest_gssi: if sender.ssi == context.calling_addr.ssi {
                        context.called_addr.ssi
                    } else {
                        context.calling_addr.ssi
                    },
                    dest_is_group: false,
                    ts,
                }),
            });
        } else if floor_released {
            self.notify_floor_released(
                queue,
                CallTimeslot {
                    call_id: new_call_id,
                    ts,
                },
                true,
                BrewNotification::Never,
            );
        }

        if let Some(call) = self.individual_calls.get_mut(&new_call_id) {
            call.complete_restore();
        }
        self.call_restore
            .mark_restored(key, new_call_id, grant, self.dltime)
            .map_err(|_| RestoreRejectReason::InvalidState)?;
        Ok(RestoredCall {
            old_call_id,
            new_call_id,
            grant,
            call_status: CallStatus::Callcontinue,
            chan_alloc: Some(Self::chan_alloc_for_ts(
                Some(usage),
                ts,
                ChanAllocType::Replace,
                UlDlAssignment::Both,
            )),
        })
    }

    /// Apply U-TX CEASED to a participant whose call restoration is waiting
    /// for a traffic bearer. Returns true when the PDU belonged to such a
    /// transaction and therefore must not be passed to active-call handling.
    pub(super) fn handle_queued_restore_tx_ceased(
        &mut self,
        sender: TetraAddress,
        call_id: u16,
    ) -> bool {
        let Some(key) = self.call_restore.queued_key_for_call(sender, call_id) else {
            return false;
        };
        match self
            .call_restore
            .set_queued_transmission_request(key, false, self.dltime)
        {
            Ok(_) => tracing::info!(
                %sender,
                call_id,
                "CMCE: queued call restoration transmission request cancelled"
            ),
            Err(error) => tracing::warn!(
                ?error,
                %sender,
                call_id,
                "CMCE: failed to cancel queued restoration transmission request"
            ),
        }
        true
    }

    /// Apply U-TX DEMAND while restoration is queued and acknowledge that the
    /// request remains queued. The actual bearer is delivered later with a
    /// second D-TX GRANTED carrying channel allocation.
    pub(super) fn handle_queued_restore_tx_demand(
        &mut self,
        queue: &mut MessageQueue,
        sender: TetraAddress,
        call_id: u16,
    ) -> bool {
        let Some(key) = self.call_restore.queued_key_for_call(sender, call_id) else {
            return false;
        };
        let grant = match self
            .call_restore
            .set_queued_transmission_request(key, true, self.dltime)
        {
            Ok(grant) => grant,
            Err(error) => {
                tracing::warn!(
                    ?error,
                    %sender,
                    call_id,
                    "CMCE: failed to queue restoration transmission request"
                );
                return true;
            }
        };
        let Some(transaction) = self.call_restore.transaction(key).cloned() else {
            return true;
        };
        let restored = RestoredCall {
            old_call_id: key.old_call_id,
            new_call_id: transaction.new_call_id.unwrap_or(key.old_call_id),
            grant,
            call_status: CallStatus::Callqueued,
            chan_alloc: None,
        };
        self.send_restore_tx_granted(queue, &transaction, &restored);
        true
    }

    pub(super) fn drive_queued_call_restores(&mut self, queue: &mut MessageQueue) {
        let queued = self.call_restore.queued_transactions();
        for transaction in queued {
            let Some(context) = self.call_restore.context(transaction.key.old_call_id).cloned() else {
                let _ = self.call_restore.reject(
                    transaction.key,
                    RestoreRejectReason::UnknownCall,
                    self.dltime,
                );
                self.send_restore_failure_release(queue, &transaction);
                continue;
            };

            let result = match context {
                CallRestoreContext::Group(context) => self.restore_group_call(
                    queue,
                    transaction.key,
                    transaction.key.subscriber,
                    transaction.request_to_transmit,
                    context,
                ),
                CallRestoreContext::Individual(context) => self.restore_individual_call(
                    queue,
                    transaction.key,
                    transaction.key.subscriber,
                    transaction.endpoint_id,
                    transaction.link_id,
                    transaction.request_to_transmit,
                    context,
                ),
            };

            match result {
                Ok(restored) => {
                    self.send_restore_tx_granted(
                        queue,
                        &transaction,
                        &restored,
                    );
                }
                Err(RestoreRejectReason::NoRadioResource) => {
                    // Remain queued. The bounded call-restore timer will release the
                    // participant if no bearer becomes available.
                }
                Err(reason) => {
                    let _ = self.call_restore.reject(transaction.key, reason, self.dltime);
                    self.send_restore_failure_release(queue, &transaction);
                }
            }
        }
    }

    pub(super) fn send_timed_out_restore_release(
        &self,
        queue: &mut MessageQueue,
        key: crate::cmce::call_restore_runtime::RestoreTransactionKey,
    ) {
        if let Some(transaction) = self.call_restore.transaction(key) {
            self.send_restore_failure_release(queue, transaction);
        }
    }

    fn send_restore_tx_granted(
        &self,
        queue: &mut MessageQueue,
        transaction: &crate::cmce::call_restore_runtime::CallRestoreTransaction,
        restored: &RestoredCall,
    ) {
        let transmitting_party = match transaction.kind {
            Some(RestoreCallKind::Group) => self
                .active_calls
                .get(&restored.new_call_id)
                .filter(|call| call.tx_active)
                .map(|call| call.source_issi)
                .or_else(|| match self.call_restore.context(transaction.key.old_call_id) {
                    Some(CallRestoreContext::Group(context)) if context.tx_active => {
                        context.floor_holder.or(Some(context.source_issi))
                    }
                    _ => None,
                }),
            Some(RestoreCallKind::Individual) => self
                .individual_calls
                .get(&restored.new_call_id)
                .and_then(|call| call.floor_holder)
                .or_else(|| match self.call_restore.context(transaction.key.old_call_id) {
                    Some(CallRestoreContext::Individual(context)) => context.floor_holder,
                    _ => None,
                }),
            None => None,
        };
        let pdu = DTxGranted {
            call_identifier: restored.new_call_id,
            transmission_grant: restored.grant.into_raw() as u8,
            transmission_request_permission: false,
            encryption_control: false,
            reserved: false,
            notification_indicator: None,
            transmitting_party_type_identifier: transmitting_party.map(|_| 1),
            transmitting_party_address_ssi: transmitting_party.map(u64::from),
            transmitting_party_extension: None,
            external_subscriber_number: None,
            facility: None,
            dm_ms_address: None,
            proprietary: None,
        };
        let mut sdu = BitBuffer::new_autoexpand(48);
        if let Err(error) = pdu.to_bitbuf(&mut sdu) {
            tracing::error!(?error, "CMCE: failed to encode queued restore D-TX GRANTED");
            return;
        }
        sdu.seek(0);
        queue.push_back(Self::build_sapmsg_direct_with_allocation(
            sdu,
            self.dltime,
            transaction.key.subscriber,
            0,
            transaction.link_id,
            transaction.endpoint_id,
            restored.chan_alloc.clone(),
        ));
    }

    fn send_restore_failure_release(
        &self,
        queue: &mut MessageQueue,
        transaction: &crate::cmce::call_restore_runtime::CallRestoreTransaction,
    ) {
        let call_id = transaction
            .new_call_id
            .unwrap_or(transaction.key.old_call_id);
        let sdu = Self::build_d_release(
            call_id,
            DisconnectCause::CallRestorationOfTheOtherUserFailed,
        );
        queue.push_back(Self::build_sapmsg_direct(
            sdu,
            self.dltime,
            transaction.key.subscriber,
            0,
            transaction.link_id,
            transaction.endpoint_id,
        ));
    }

    fn reserve_restore_call_id(&mut self, old_call_id: u16) -> u16 {
        if let Some(call_id) = self.call_restore.resolved_call_id(old_call_id) {
            return call_id;
        }

        let in_use = |call_id: u16, this: &Self| {
            this.active_calls.contains_key(&call_id)
                || this.individual_calls.contains_key(&call_id)
                || this
                    .circuits
                    .dl
                    .iter()
                    .flatten()
                    .any(|circuit| circuit.call_id == call_id)
                || this
                    .circuits
                    .ul_only
                    .iter()
                    .flatten()
                    .any(|circuit| circuit.call_id == call_id)
        };

        let local_call_id = if in_use(old_call_id, self) {
            loop {
                let candidate = self.circuits.get_next_call_id();
                if !in_use(candidate, self) {
                    break candidate;
                }
            }
        } else {
            old_call_id
        };
        self.call_restore.reserve_call_id(old_call_id, local_call_id);
        local_call_id
    }

    fn install_restored_group_setup(&mut self, call_id: u16, context: &GroupCallRestoreContext) {
        let dest_addr = TetraAddress::new(context.dest_gssi, SsiType::Gssi);
        self.cached_setups.entry(call_id).or_insert_with(|| CachedSetup {
            pdu: DSetup {
                call_identifier: call_id,
                call_time_out: context.call_timeout,
                hook_method_selection: false,
                simplex_duplex_selection: false,
                basic_service_information: BasicServiceInformation {
                    circuit_mode_type: context.circuit_mode_type,
                    encryption_flag: context.etee_encrypted,
                    communication_type: context.communication_type,
                    slots_per_frame: None,
                    speech_service: context.speech_service,
                },
                transmission_grant: if context.tx_active {
                    TransmissionGrant::GrantedToOtherUser
                } else {
                    TransmissionGrant::NotGranted
                },
                transmission_request_permission: false,
                call_priority: context.priority,
                notification_indicator: None,
                temporary_address: None,
                calling_party_address_ssi: Some(context.source_issi),
                calling_party_extension: None,
                external_subscriber_number: None,
                facility: None,
                dm_ms_address: None,
                proprietary: None,
            },
            dest_addr,
            resend: true,
            tx_receipt: None,
        });
    }

    fn install_restored_individual_setup(
        &mut self,
        call_id: u16,
        context: &IndividualCallRestoreContext,
    ) {
        self.cached_setups.entry(call_id).or_insert_with(|| CachedSetup {
            pdu: DSetup {
                call_identifier: call_id,
                call_time_out: context.call_timeout,
                hook_method_selection: false,
                simplex_duplex_selection: context.simplex_duplex,
                basic_service_information: BasicServiceInformation {
                    circuit_mode_type: context.circuit_mode_type,
                    encryption_flag: context.etee_encrypted,
                    communication_type: context.communication_type,
                    slots_per_frame: None,
                    speech_service: context.speech_service,
                },
                transmission_grant: if context.simplex_duplex {
                    TransmissionGrant::NotGranted
                } else {
                    TransmissionGrant::GrantedToOtherUser
                },
                transmission_request_permission: false,
                call_priority: context.priority,
                notification_indicator: None,
                temporary_address: None,
                calling_party_address_ssi: Some(context.calling_addr.ssi),
                calling_party_extension: None,
                external_subscriber_number: None,
                facility: None,
                dm_ms_address: None,
                proprietary: None,
            },
            dest_addr: context.called_addr,
            resend: false,
            tx_receipt: None,
        });
    }

    fn restore_service_matches(
        context: &CallRestoreContext,
        basic_service: Option<&BasicServiceInformation>,
    ) -> bool {
        let locally_supported = |
            circuit_mode_type: CircuitModeType,
            speech_service: Option<u8>,
            etee_encrypted: bool,
        | {
            circuit_mode_type == CircuitModeType::TchS
                && speech_service.unwrap_or(0) == 0
                && !etee_encrypted
        };

        let (communication_type, circuit_mode_type, speech_service, etee_encrypted) = match context {
            CallRestoreContext::Group(context) => (
                context.communication_type,
                context.circuit_mode_type,
                context.speech_service,
                context.etee_encrypted,
            ),
            CallRestoreContext::Individual(context) => (
                context.communication_type,
                context.circuit_mode_type,
                context.speech_service,
                context.etee_encrypted,
            ),
        };

        if !locally_supported(circuit_mode_type, speech_service, etee_encrypted) {
            return false;
        }

        let Some(basic_service) = basic_service else {
            return true;
        };
        communication_type == basic_service.communication_type
            && circuit_mode_type == basic_service.circuit_mode_type
            && etee_encrypted == basic_service.encryption_flag
    }

    fn mle_fail_cause_for_restore_reject(reason: RestoreRejectReason) -> MleFailCause {
        match reason {
            RestoreRejectReason::NoRadioResource => MleFailCause::NeighbourCellEnquiryUnavailableOrTemporaryBreak,
            RestoreRejectReason::ParticipantMismatch | RestoreRejectReason::ServiceMismatch => {
                MleFailCause::MsNotAllowedOnCell
            }
            RestoreRejectReason::MalformedPdu
            | RestoreRejectReason::UnknownCall
            | RestoreRejectReason::InvalidState
            | RestoreRejectReason::DuplicateRequest
            | RestoreRejectReason::Timeout => MleFailCause::RestorationCannotBeDoneOnCell,
        }
    }
}

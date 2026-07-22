//! Local LTPD runtime between SNDCP and MLE.
//!
//! The runtime owns packet-data link state inside one TBS process. It deliberately
//! does not cross the future Edge/Core boundary: timing-sensitive MLE/LLC routing
//! remains local while read-only snapshots can later be exposed by the TBS WebUI.

use std::collections::HashMap;

use tetra_core::tetra_entities::TetraEntity;
use tetra_core::{
    BitBuffer, EndpointId, Layer2Service, LinkId, Sap, TdmaTime, TetraAddress, TxReporter,
    TxState,
};
use tetra_pdus::mle::enums::mle_protocol_discriminator::MleProtocolDiscriminator;
use tetra_saps::common::{
    Layer2Qos, Layer2Report, LtpdLinkState, MleBroadcastParameters, ReconnectionResult,
    RequestHandle, SetupReport, SleepMode, SndcpStatus, TransferResult,
};
use tetra_saps::ltpd::*;
use tetra_saps::tla::{TlaTlDataReqBl, TlaTlUnitdataReqBl};
use tetra_saps::{SapMsg, SapMsgInner};

use crate::MessageQueue;

/// Maximum age of an outgoing LTPD request before it is failed locally.
/// 432 timeslots are six multiframes and bound a stuck LLC/MAC transaction.
const TRANSFER_TIMEOUT_SLOTS: i32 = 432;

/// Completed request handles remain guarded for one hyperframe. This prevents
/// delayed or replayed SNDCP requests from being transmitted twice.
const COMPLETED_HANDLE_RETENTION_SLOTS: i32 = 4 * 18 * 60;

const RESULT_UNKNOWN_HANDLE: u8 = 1;
const RESULT_DUPLICATE_HANDLE: u8 = 2;
const RESULT_CANCEL_TOO_LATE: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LtpdRuntimeRole {
    MobileStation,
    Swmi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct LinkKey {
    endpoint_id: EndpointId,
    link_id: LinkId,
}

#[derive(Debug, Clone)]
struct LinkContext {
    address: TetraAddress,
    endpoint_id: EndpointId,
    link_id: LinkId,
    state: LtpdLinkState,
    qos: Layer2Qos,
    encrypted: bool,
    sndcp_status: SndcpStatus,
    last_activity: TdmaTime,
    successful_transfers: u64,
    failed_transfers: u64,
}

#[derive(Debug, Clone)]
struct PendingTransfer {
    endpoint_id: EndpointId,
    link_id: LinkId,
    queued_at: TdmaTime,
    tx_reporter: TxReporter,
}

#[derive(Debug, Clone, Copy)]
struct CompletedTransfer {
    result: TransferResult,
    completed_at: TdmaTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LtpdPendingTransferSnapshot {
    pub handle: RequestHandle,
    pub endpoint_id: EndpointId,
    pub link_id: LinkId,
    pub age_slots: i32,
    pub tx_state: TxState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LtpdCompletedTransferSnapshot {
    pub handle: RequestHandle,
    pub result: TransferResult,
    pub age_slots: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LtpdLinkSnapshot {
    pub address: TetraAddress,
    pub endpoint_id: EndpointId,
    pub link_id: LinkId,
    pub state: LtpdLinkState,
    pub qos: Layer2Qos,
    pub encrypted: bool,
    pub sndcp_status: SndcpStatus,
    pub successful_transfers: u64,
    pub failed_transfers: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LtpdRuntimeSnapshot {
    pub role: LtpdRuntimeRole,
    pub network_open: bool,
    pub mcc: Option<u16>,
    pub mnc: Option<u16>,
    pub lower_layer_available: bool,
    pub disabled: bool,
    pub busy: bool,
    pub sleep_mode: SleepMode,
    pub pending_transfer_count: usize,
    pub replay_guard_count: usize,
    pub duplicate_handle_rejections: u64,
    pub cancel_requests: u64,
    pub cancelled_transfers: u64,
    pub timed_out_transfers: u64,
    pub invalid_transition_rejections: u64,
    pub pending_transfers: Vec<LtpdPendingTransferSnapshot>,
    pub completed_transfers: Vec<LtpdCompletedTransferSnapshot>,
    pub links: Vec<LtpdLinkSnapshot>,
}

pub struct LtpdRuntime {
    role: LtpdRuntimeRole,
    mcc: u16,
    mnc: u16,
    network_open: bool,
    initial_open_pending: bool,
    lower_layer_available: bool,
    disabled: bool,
    busy: bool,
    sleep_mode: SleepMode,
    broadcast: MleBroadcastParameters,
    links: HashMap<LinkKey, LinkContext>,
    pending: HashMap<RequestHandle, PendingTransfer>,
    completed: HashMap<RequestHandle, CompletedTransfer>,
    current_time: TdmaTime,
    duplicate_handle_rejections: u64,
    cancel_requests: u64,
    cancelled_transfers: u64,
    timed_out_transfers: u64,
    invalid_transition_rejections: u64,
}

impl LtpdRuntime {
    pub fn new(
        role: LtpdRuntimeRole,
        mcc: u16,
        mnc: u16,
        broadcast: MleBroadcastParameters,
    ) -> Self {
        Self {
            role,
            mcc,
            mnc,
            network_open: true,
            initial_open_pending: true,
            lower_layer_available: true,
            disabled: false,
            busy: false,
            sleep_mode: SleepMode::SleepPermitted,
            broadcast,
            links: HashMap::new(),
            pending: HashMap::new(),
            completed: HashMap::new(),
            current_time: TdmaTime::default(),
            duplicate_handle_rejections: 0,
            cancel_requests: 0,
            cancelled_transfers: 0,
            timed_out_transfers: 0,
            invalid_transition_rejections: 0,
        }
    }

    pub fn snapshot(&self) -> LtpdRuntimeSnapshot {
        let mut links = self
            .links
            .values()
            .map(|context| LtpdLinkSnapshot {
                address: context.address,
                endpoint_id: context.endpoint_id,
                link_id: context.link_id,
                state: context.state,
                qos: context.qos,
                encrypted: context.encrypted,
                sndcp_status: context.sndcp_status,
                successful_transfers: context.successful_transfers,
                failed_transfers: context.failed_transfers,
            })
            .collect::<Vec<_>>();
        links.sort_by_key(|item| (item.address.ssi, item.endpoint_id, item.link_id));

        let mut pending_transfers = self
            .pending
            .iter()
            .map(|(handle, transfer)| LtpdPendingTransferSnapshot {
                handle: *handle,
                endpoint_id: transfer.endpoint_id,
                link_id: transfer.link_id,
                age_slots: transfer.queued_at.age(self.current_time).max(0),
                tx_state: transfer.tx_reporter.get_state(),
            })
            .collect::<Vec<_>>();
        pending_transfers.sort_by_key(|item| item.handle.0);

        let mut completed_transfers = self
            .completed
            .iter()
            .map(|(handle, transfer)| LtpdCompletedTransferSnapshot {
                handle: *handle,
                result: transfer.result,
                age_slots: transfer.completed_at.age(self.current_time).max(0),
            })
            .collect::<Vec<_>>();
        completed_transfers.sort_by_key(|item| item.handle.0);

        LtpdRuntimeSnapshot {
            role: self.role,
            network_open: self.network_open,
            mcc: self.network_open.then_some(self.mcc),
            mnc: self.network_open.then_some(self.mnc),
            lower_layer_available: self.lower_layer_available,
            disabled: self.disabled,
            busy: self.busy,
            sleep_mode: self.sleep_mode,
            pending_transfer_count: self.pending.len(),
            replay_guard_count: self.completed.len(),
            duplicate_handle_rejections: self.duplicate_handle_rejections,
            cancel_requests: self.cancel_requests,
            cancelled_transfers: self.cancelled_transfers,
            timed_out_transfers: self.timed_out_transfers,
            invalid_transition_rejections: self.invalid_transition_rejections,
            pending_transfers,
            completed_transfers,
            links,
        }
    }

    /// Learn or refresh the basic-link route used by an incoming SNDCP PDU.
    pub fn observe_inbound(
        &mut self,
        address: TetraAddress,
        endpoint_id: EndpointId,
        link_id: LinkId,
        encrypted: bool,
        now: TdmaTime,
    ) {
        self.current_time = now;
        let key = LinkKey { endpoint_id, link_id };
        let context = self.links.entry(key).or_insert_with(|| LinkContext {
            address,
            endpoint_id,
            link_id,
            state: LtpdLinkState::Connected,
            qos: Layer2Qos::default(),
            encrypted,
            sndcp_status: SndcpStatus::Ready,
            last_activity: now,
            successful_transfers: 0,
            failed_transfers: 0,
        });
        context.address = address;
        context.encrypted = encrypted;
        context.last_activity = now;
        context.sndcp_status = SndcpStatus::Ready;
        if !matches!(context.state, LtpdLinkState::Disabled | LtpdLinkState::Broken) {
            context.state = LtpdLinkState::Connected;
        }
    }

    pub fn notify_break(&mut self, queue: &mut MessageQueue) {
        if !self.lower_layer_available {
            return;
        }
        self.lower_layer_available = false;
        self.fail_all_pending(queue, TransferResult::FailedRemovedFromBuffer);
        for context in self.links.values_mut() {
            if !matches!(context.state, LtpdLinkState::Closed | LtpdLinkState::Disabled) {
                context.state = LtpdLinkState::Broken;
            }
        }
        self.to_sndcp(queue, SapMsgInner::LtpdMleBreakInd(LtpdMleBreakInd));
    }

    pub fn notify_resume(&mut self, queue: &mut MessageQueue) {
        if self.lower_layer_available {
            return;
        }
        self.lower_layer_available = true;
        for context in self.links.values_mut() {
            if context.state == LtpdLinkState::Broken {
                context.state = LtpdLinkState::Open;
            }
        }
        self.to_sndcp(
            queue,
            SapMsgInner::LtpdMleResumeInd(LtpdMleResumeInd {
                mcc: self.mcc,
                mnc: self.mnc,
            }),
        );
    }

    pub fn set_busy(&mut self, queue: &mut MessageQueue, busy: bool) {
        if self.busy == busy {
            return;
        }
        self.busy = busy;
        if busy {
            for context in self.links.values_mut() {
                if context.state == LtpdLinkState::Connected {
                    context.state = LtpdLinkState::Busy;
                }
            }
            self.to_sndcp(queue, SapMsgInner::LtpdMleBusyInd(LtpdMleBusyInd));
        } else {
            for context in self.links.values_mut() {
                if context.state == LtpdLinkState::Busy {
                    context.state = LtpdLinkState::Connected;
                }
            }
            self.to_sndcp(queue, SapMsgInner::LtpdMleIdleInd(LtpdMleIdleInd));
        }
    }

    pub fn set_disabled(
        &mut self,
        queue: &mut MessageQueue,
        disabled: bool,
        permitted_services: tetra_saps::common::PermittedTemporaryServices,
    ) {
        if self.disabled == disabled {
            return;
        }
        self.disabled = disabled;
        if disabled {
            self.fail_all_pending(queue, TransferResult::FailedRemovedFromBuffer);
            for context in self.links.values_mut() {
                context.state = LtpdLinkState::Disabled;
            }
            self.to_sndcp(
                queue,
                SapMsgInner::LtpdMleDisableInd(LtpdMleDisableInd { permitted_services }),
            );
        } else {
            for context in self.links.values_mut() {
                if context.state == LtpdLinkState::Disabled {
                    context.state = if self.lower_layer_available {
                        LtpdLinkState::Open
                    } else {
                        LtpdLinkState::Broken
                    };
                }
            }
            self.to_sndcp(queue, SapMsgInner::LtpdMleEnableInd(LtpdMleEnableInd));
        }
    }

    pub fn close_network(&mut self, queue: &mut MessageQueue) {
        if !self.network_open {
            return;
        }
        self.network_open = false;
        self.fail_all_pending(queue, TransferResult::FailedRemovedFromBuffer);
        for context in self.links.values_mut() {
            context.state = LtpdLinkState::Closed;
        }
        self.to_sndcp(queue, SapMsgInner::LtpdMleCloseInd(LtpdMleCloseInd));
    }

    pub fn open_network(&mut self, queue: &mut MessageQueue, mcc: u16, mnc: u16) {
        self.mcc = mcc;
        self.mnc = mnc;
        self.network_open = true;
        self.initial_open_pending = false;
        self.to_sndcp(
            queue,
            SapMsgInner::LtpdMleOpenInd(LtpdMleOpenInd { mcc, mnc }),
        );
        self.to_sndcp(
            queue,
            SapMsgInner::LtpdMleInfoInd(LtpdMleInfoInd {
                broadcast_parameters: self.broadcast.clone(),
                subscriber_class_match: true,
                schedule_timing_prompt: None,
                permitted_cell_information: tetra_saps::common::PermittedCellInformation::Permitted,
            }),
        );
    }

    pub fn tick(&mut self, queue: &mut MessageQueue, now: TdmaTime) {
        self.current_time = now;
        if self.initial_open_pending {
            self.open_network(queue, self.mcc, self.mnc);
        }

        self.completed
            .retain(|_, completed| completed.completed_at.age(now) < COMPLETED_HANDLE_RETENTION_SLOTS);

        let mut finished = Vec::new();
        let mut timed_out = 0_u64;
        for (handle, pending) in &self.pending {
            let age = pending.queued_at.age(now);
            let result = match pending.tx_reporter.get_state() {
                TxState::Discarded | TxState::Lost => Some(TransferResult::FailedRemovedFromBuffer),
                TxState::Acknowledged => Some(TransferResult::SuccessBufferEmpty),
                TxState::Transmitted if pending.tx_reporter.is_in_final_state() => {
                    Some(TransferResult::SuccessBufferEmpty)
                }
                _ if age >= TRANSFER_TIMEOUT_SLOTS => {
                    timed_out = timed_out.saturating_add(1);
                    Some(TransferResult::FailedRemovedFromBuffer)
                }
                _ => None,
            };
            if let Some(result) = result {
                finished.push((*handle, result));
            }
        }
        self.timed_out_transfers = self.timed_out_transfers.saturating_add(timed_out);
        for (handle, result) in finished {
            self.complete_transfer(queue, handle, result, now);
        }
    }

    pub fn handle_primitive(&mut self, queue: &mut MessageQueue, message: SapMsg, now: TdmaTime) {
        self.current_time = now;
        match message.msg {
            SapMsgInner::LtpdMleActivityReq(request) => {
                self.sleep_mode = request.sleep_mode;
            }
            SapMsgInner::LtpdMleCancelReq(request) => {
                self.cancel_requests = self.cancel_requests.saturating_add(1);
                if self.pending.contains_key(&request.handle) {
                    self.cancelled_transfers = self.cancelled_transfers.saturating_add(1);
                    self.complete_transfer(
                        queue,
                        request.handle,
                        TransferResult::FailedRemovedFromBuffer,
                        now,
                    );
                } else if self.completed.contains_key(&request.handle) {
                    self.report(
                        queue,
                        request.handle,
                        TransferResult::Other(RESULT_CANCEL_TOO_LATE),
                    );
                } else {
                    self.report(
                        queue,
                        request.handle,
                        TransferResult::Other(RESULT_UNKNOWN_HANDLE),
                    );
                }
            }
            SapMsgInner::LtpdMleConfigureReq(request) => {
                self.configure(queue, request, now);
            }
            SapMsgInner::LtpdMleConnectReq(request) => {
                self.connect(queue, request, now);
            }
            SapMsgInner::LtpdMleConnectResp(response) => {
                self.connect_response(queue, response, now);
            }
            SapMsgInner::LtpdMleDisconnectReq(request) => {
                self.disconnect(queue, request);
            }
            SapMsgInner::LtpdMleReconnectReq(request) => {
                self.reconnect(queue, request, now);
            }
            SapMsgInner::LtpdMleReleaseReq(request) => {
                self.release(queue, request.link_id, now);
            }
            SapMsgInner::LtpdMleUnitdataReq(request) => {
                self.unitdata(queue, request, now);
            }
            other => {
                tracing::warn!("LTPD: MLE received unexpected SNDCP primitive: {:?}", other);
            }
        }
    }

    fn configure(&mut self, queue: &mut MessageQueue, request: LtpdMleConfigureReq, now: TdmaTime) {
        let release_requested =
            request.call_release == tetra_saps::common::CallReleaseInstruction::Release;
        if let Some(context) = self
            .links
            .values_mut()
            .find(|context| context.endpoint_id == request.endpoint_id)
        {
            context.encrypted = request.encryption_flag;
            context.sndcp_status = request.sndcp_status;
            context.last_activity = now;
            if release_requested {
                context.state = LtpdLinkState::Releasing;
            }
        }
        if release_requested {
            let handles = self.pending_handles_for_endpoint(request.endpoint_id);
            for handle in handles {
                self.complete_transfer(
                    queue,
                    handle,
                    TransferResult::FailedRemovedFromBuffer,
                    now,
                );
            }
        }

        if let Some(handle) = request.channel_change_handle
            && request.channel_change_accepted == Some(tetra_saps::common::ChannelChangeDecision::Reject)
        {
            self.to_sndcp(
                queue,
                SapMsgInner::LtpdMleConfigureInd(LtpdMleConfigureInd {
                    endpoint_id: request.endpoint_id,
                    channel_change_response_required: false,
                    channel_change_handle: Some(handle),
                    reason: tetra_saps::common::LowerLayerResourceReason::LossOfRadioResources,
                    conflicting_endpoint_id: None,
                }),
            );
        }
    }

    fn connect(&mut self, queue: &mut MessageQueue, request: LtpdMleConnectReq, now: TdmaTime) {
        let key = LinkKey {
            endpoint_id: request.endpoint_id,
            link_id: request.link_id,
        };
        let existing_allows_reset = self
            .links
            .get(&key)
            .map(|context| matches!(context.state, LtpdLinkState::Closed | LtpdLinkState::Null))
            .unwrap_or(true);
        let valid = request.layer_2_qos.validate().is_ok()
            && self.network_open
            && self.lower_layer_available
            && !self.disabled
            && existing_allows_reset;
        let report = if valid {
            SetupReport::Success
        } else {
            self.invalid_transition_rejections =
                self.invalid_transition_rejections.saturating_add(1);
            SetupReport::ParametersNotAcceptable
        };

        if valid {
            self.links.insert(
                key,
                LinkContext {
                    address: request.address,
                    endpoint_id: request.endpoint_id,
                    link_id: request.link_id,
                    state: LtpdLinkState::Connected,
                    qos: request.layer_2_qos,
                    encrypted: request.encryption_flag,
                    sndcp_status: SndcpStatus::Ready,
                    last_activity: now,
                    successful_transfers: 0,
                    failed_transfers: 0,
                },
            );
        }
        self.to_sndcp(
            queue,
            SapMsgInner::LtpdMleConnectConfirm(LtpdMleConnectConfirm {
                address: request.address,
                endpoint_id: request.endpoint_id,
                link_id: request.link_id,
                layer_2_qos: request.layer_2_qos,
                encryption_flag: request.encryption_flag,
                channel_change_response_required: false,
                channel_change_handle: None,
                setup_report: report,
            }),
        );
    }

    fn connect_response(
        &mut self,
        queue: &mut MessageQueue,
        response: LtpdMleConnectResp,
        now: TdmaTime,
    ) {
        let key = LinkKey {
            endpoint_id: response.endpoint_id,
            link_id: response.link_id,
        };
        let state_allows_response = self
            .links
            .get(&key)
            .map(|context| context.state == LtpdLinkState::Connecting)
            .unwrap_or(false);
        let accepted = state_allows_response && response.setup_report == SetupReport::Success;
        let report = if accepted {
            SetupReport::Success
        } else {
            self.invalid_transition_rejections =
                self.invalid_transition_rejections.saturating_add(1);
            SetupReport::ParametersNotAcceptable
        };
        if accepted {
            let context = self.links.entry(key).or_insert(LinkContext {
                address: response.address,
                endpoint_id: response.endpoint_id,
                link_id: response.link_id,
                state: LtpdLinkState::Connecting,
                qos: response.layer_2_qos,
                encrypted: response.encryption_flag,
                sndcp_status: SndcpStatus::Ready,
                last_activity: now,
                successful_transfers: 0,
                failed_transfers: 0,
            });
            context.state = LtpdLinkState::Connected;
            context.qos = response.layer_2_qos;
            context.encrypted = response.encryption_flag;
            context.last_activity = now;
        }
        self.to_sndcp(
            queue,
            SapMsgInner::LtpdMleConnectConfirm(LtpdMleConnectConfirm {
                address: response.address,
                endpoint_id: response.endpoint_id,
                link_id: response.link_id,
                layer_2_qos: response.layer_2_qos,
                encryption_flag: response.encryption_flag,
                channel_change_response_required: false,
                channel_change_handle: None,
                setup_report: report,
            }),
        );
    }

    fn disconnect(&mut self, queue: &mut MessageQueue, request: LtpdMleDisconnectReq) {
        let key = LinkKey {
            endpoint_id: request.endpoint_id,
            link_id: request.link_id,
        };
        let existed = if let Some(context) = self.links.get_mut(&key) {
            if matches!(context.state, LtpdLinkState::Closed | LtpdLinkState::Null) {
                false
            } else {
                context.state = LtpdLinkState::Closed;
                true
            }
        } else {
            false
        };
        if !existed {
            self.invalid_transition_rejections =
                self.invalid_transition_rejections.saturating_add(1);
        }
        self.to_sndcp(
            queue,
            SapMsgInner::LtpdMleDisconnectInd(LtpdMleDisconnectInd {
                endpoint_id: request.endpoint_id,
                new_endpoint_id: None,
                link_id: request.link_id,
                encryption_flag: request.encryption_flag,
                channel_change_response_required: false,
                channel_change_handle: None,
                report: if existed {
                    Layer2Report::LocalDisconnection
                } else {
                    Layer2Report::DisconnectionFailure
                },
            }),
        );
    }

    fn reconnect(&mut self, queue: &mut MessageQueue, request: LtpdMleReconnectReq, now: TdmaTime) {
        let key = LinkKey {
            endpoint_id: request.endpoint_id,
            link_id: request.link_id,
        };
        let (new_endpoint_id, report, result) = if let Some(context) = self.links.get_mut(&key) {
            if matches!(
                context.state,
                LtpdLinkState::Open
                    | LtpdLinkState::Broken
                    | LtpdLinkState::Reconnecting
                    | LtpdLinkState::Closed
            ) && self.network_open
                && self.lower_layer_available
                && !self.disabled
            {
                context.state = LtpdLinkState::Connected;
                context.encrypted = request.encryption_flag;
                context.last_activity = now;
                (None, Layer2Report::Success, ReconnectionResult::Success)
            } else {
                (None, Layer2Report::Reject, ReconnectionResult::Reject)
            }
        } else {
            (None, Layer2Report::Reject, ReconnectionResult::Reject)
        };
        if result == ReconnectionResult::Reject {
            self.invalid_transition_rejections =
                self.invalid_transition_rejections.saturating_add(1);
        }
        self.to_sndcp(
            queue,
            SapMsgInner::LtpdMleReconnectConfirm(LtpdMleReconnectConfirm {
                endpoint_id: request.endpoint_id,
                new_endpoint_id,
                link_id: request.link_id,
                encryption_flag: request.encryption_flag,
                report,
                reconnection_result: result,
            }),
        );
    }

    fn release(&mut self, queue: &mut MessageQueue, link_id: LinkId, now: TdmaTime) {
        for context in self.links.values_mut().filter(|context| context.link_id == link_id) {
            context.state = LtpdLinkState::Closed;
        }
        let handles = self.pending_handles_for_link(link_id);
        for handle in handles {
            self.complete_transfer(
                queue,
                handle,
                TransferResult::FailedRemovedFromBuffer,
                now,
            );
        }
    }

    fn unitdata(&mut self, queue: &mut MessageQueue, mut request: LtpdMleUnitdataReq, now: TdmaTime) {
        if self.pending.contains_key(&request.handle) || self.completed.contains_key(&request.handle) {
            self.duplicate_handle_rejections = self.duplicate_handle_rejections.saturating_add(1);
            self.report(
                queue,
                request.handle,
                TransferResult::Other(RESULT_DUPLICATE_HANDLE),
            );
            return;
        }
        let key = LinkKey {
            endpoint_id: request.endpoint_id,
            link_id: request.link_id,
        };
        if !self.links.contains_key(&key)
            && let Some(address) = request.address
        {
            self.observe_inbound(
                address,
                request.endpoint_id,
                request.link_id,
                false,
                now,
            );
        }
        let (address, route_available) = {
            let Some(context) = self.links.get_mut(&key) else {
                self.report(queue, request.handle, TransferResult::FailedRemovedFromBuffer);
                self.completed.insert(
                    request.handle,
                    CompletedTransfer {
                        result: TransferResult::FailedRemovedFromBuffer,
                        completed_at: now,
                    },
                );
                return;
            };
            let route_available = self.network_open
                && self.lower_layer_available
                && !self.disabled
                && !self.busy
                && !matches!(
                    context.state,
                    LtpdLinkState::Busy
                        | LtpdLinkState::Broken
                        | LtpdLinkState::Closed
                        | LtpdLinkState::Disabled
                        | LtpdLinkState::Releasing
                );
            if route_available {
                context.last_activity = now;
            } else {
                context.failed_transfers = context.failed_transfers.saturating_add(1);
            }
            (context.address, route_available)
        };
        if !route_available {
            self.report(queue, request.handle, TransferResult::FailedRemovedFromBuffer);
            self.completed.insert(
                request.handle,
                CompletedTransfer {
                    result: TransferResult::FailedRemovedFromBuffer,
                    completed_at: now,
                },
            );
            return;
        }

        let tx_reporter = match request.layer2service {
            Layer2Service::Unacknowledged => TxReporter::new_unacked(),
            Layer2Service::Acknowledged | Layer2Service::Todo => TxReporter::new(),
        };
        self.pending.insert(
            request.handle,
            PendingTransfer {
                endpoint_id: request.endpoint_id,
                link_id: request.link_id,
                queued_at: now,
                tx_reporter: tx_reporter.clone(),
            },
        );

        let allocation_only = request.sdu.get_len() == 0 && request.chan_alloc.is_some();
        let encryption_todo = request.packet_data_flag.then_some(0);
        let tl_sdu = if allocation_only {
            BitBuffer::new(0)
        } else {
            let mut source = request.sdu;
            source.seek(0);
            let source_len = source.get_len();
            let mut wrapped = BitBuffer::new(3 + source_len);
            wrapped.write_bits(MleProtocolDiscriminator::Sndcp.into_raw(), 3);
            wrapped.copy_bits(&mut source, source_len);
            wrapped.seek(0);
            wrapped
        };
        let stealing_permission = !matches!(
            request.stealing_permission,
            tetra_saps::common::StealingPermission::NotRequired
        );
        let request_handle = i32::try_from(request.handle.0).unwrap_or(i32::MAX);
        let message = match request.layer2service {
            Layer2Service::Unacknowledged => SapMsgInner::TlaTlUnitdataReqBl(TlaTlUnitdataReqBl {
                main_address: address,
                link_id: request.link_id,
                endpoint_id: request.endpoint_id,
                tl_sdu,
                stealing_permission,
                subscriber_class: 0,
                fcs_flag: request.fcs_flag,
                air_interface_encryption: encryption_todo,
                packet_data_flag: request.packet_data_flag,
                n_tlsdu_repeats: request.unacknowledged_basic_link_repetitions,
                data_class_info: None,
                req_handle: request_handle,
                chan_alloc: request.chan_alloc.take(),
                tx_reporter: Some(tx_reporter),
            }),
            Layer2Service::Acknowledged | Layer2Service::Todo => {
                SapMsgInner::TlaTlDataReqBl(TlaTlDataReqBl {
                    main_address: address,
                    link_id: request.link_id,
                    endpoint_id: request.endpoint_id,
                    tl_sdu,
                    stealing_permission,
                    subscriber_class: 0,
                    fcs_flag: request.fcs_flag,
                    air_interface_encryption: encryption_todo,
                    stealing_repeats_flag: Some(request.stealing_repeats_flag),
                    data_class_info: None,
                    req_handle: request_handle,
                    graceful_degradation: None,
                    chan_alloc: request.chan_alloc.take(),
                    tx_reporter: Some(tx_reporter),
                })
            }
        };
        queue.push_back(SapMsg::new(
            Sap::TlaSap,
            TetraEntity::Mle,
            TetraEntity::Llc,
            message,
        ));
    }

    fn record_transfer_result(&mut self, endpoint_id: EndpointId, link_id: LinkId, success: bool) {
        if let Some(context) = self.links.get_mut(&LinkKey { endpoint_id, link_id }) {
            if success {
                context.successful_transfers = context.successful_transfers.saturating_add(1);
            } else {
                context.failed_transfers = context.failed_transfers.saturating_add(1);
            }
        }
    }

    fn complete_transfer(
        &mut self,
        queue: &mut MessageQueue,
        handle: RequestHandle,
        result: TransferResult,
        now: TdmaTime,
    ) {
        if let Some(pending) = self.pending.remove(&handle) {
            let success = matches!(
                result,
                TransferResult::SuccessMoreDataBuffered | TransferResult::SuccessBufferEmpty
            );
            self.record_transfer_result(pending.endpoint_id, pending.link_id, success);
        }
        self.completed.insert(
            handle,
            CompletedTransfer {
                result,
                completed_at: now,
            },
        );
        self.report(queue, handle, result);
    }

    fn fail_all_pending(&mut self, queue: &mut MessageQueue, result: TransferResult) {
        let handles = self.pending.keys().copied().collect::<Vec<_>>();
        for handle in handles {
            self.complete_transfer(queue, handle, result, self.current_time);
        }
    }

    fn pending_handles_for_link(&self, link_id: LinkId) -> Vec<RequestHandle> {
        self.pending
            .iter()
            .filter_map(|(handle, pending)| (pending.link_id == link_id).then_some(*handle))
            .collect()
    }

    fn pending_handles_for_endpoint(&self, endpoint_id: EndpointId) -> Vec<RequestHandle> {
        self.pending
            .iter()
            .filter_map(|(handle, pending)| {
                (pending.endpoint_id == endpoint_id).then_some(*handle)
            })
            .collect()
    }

    fn report(&self, queue: &mut MessageQueue, handle: RequestHandle, result: TransferResult) {
        self.to_sndcp(
            queue,
            SapMsgInner::LtpdMleReportInd(LtpdMleReportInd {
                handle,
                transfer_result: result,
            }),
        );
    }

    fn to_sndcp(&self, queue: &mut MessageQueue, message: SapMsgInner) {
        queue.push_back(SapMsg::new(
            Sap::TlpdSap,
            TetraEntity::Mle,
            TetraEntity::Sndcp,
            message,
        ));
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use tetra_core::SsiType;
    use tetra_saps::common::{
        ChannelAdvice, DataClass, DataPriority, PduPriority, ScheduledDataStatus,
        StealingPermission,
    };

    fn runtime() -> LtpdRuntime {
        let mut runtime = LtpdRuntime::new(
            LtpdRuntimeRole::Swmi,
            262,
            1,
            MleBroadcastParameters::default(),
        );
        runtime.initial_open_pending = false;
        runtime
    }

    fn unitdata(handle: u32) -> LtpdMleUnitdataReq {
        LtpdMleUnitdataReq {
            sdu: BitBuffer::new(0),
            handle: RequestHandle(handle),
            address: None,
            layer2service: Layer2Service::Acknowledged,
            unacknowledged_basic_link_repetitions: 0,
            pdu_priority: PduPriority::default(),
            endpoint_id: 2,
            link_id: 3,
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
        }
    }

    fn ready_runtime() -> LtpdRuntime {
        let mut runtime = runtime();
        runtime.observe_inbound(
            TetraAddress::new(1001, SsiType::Issi),
            2,
            3,
            false,
            TdmaTime::default(),
        );
        runtime
    }

    #[test]
    fn unknown_route_is_rejected_and_replay_guarded() {
        let mut runtime = runtime();
        let mut queue = MessageQueue::new();
        runtime.handle_primitive(
            &mut queue,
            SapMsg::new(
                Sap::TlpdSap,
                TetraEntity::Sndcp,
                TetraEntity::Mle,
                SapMsgInner::LtpdMleUnitdataReq(unitdata(7)),
            ),
            TdmaTime::default(),
        );
        assert_eq!(queue.len(), 1);
        assert!(matches!(
            queue.pop_front().unwrap().msg,
            SapMsgInner::LtpdMleReportInd(LtpdMleReportInd {
                transfer_result: TransferResult::FailedRemovedFromBuffer,
                ..
            })
        ));
        assert_eq!(runtime.snapshot().replay_guard_count, 1);
    }

    #[test]
    fn tx_reporter_completes_acknowledged_transfer() {
        let mut runtime = ready_runtime();
        let mut queue = MessageQueue::new();
        runtime.handle_primitive(
            &mut queue,
            SapMsg::new(
                Sap::TlpdSap,
                TetraEntity::Sndcp,
                TetraEntity::Mle,
                SapMsgInner::LtpdMleUnitdataReq(unitdata(8)),
            ),
            TdmaTime::default(),
        );
        let lower = queue.pop_front().expect("lower-layer request missing");
        let reporter = match lower.msg {
            SapMsgInner::TlaTlDataReqBl(request) => request.tx_reporter.expect("TxReporter missing"),
            other => panic!("unexpected lower-layer primitive: {:?}", other),
        };
        assert!(queue.is_empty());
        reporter.mark_transmitted();
        reporter.mark_acknowledged();
        runtime.tick(&mut queue, TdmaTime::default().add_timeslots(1));
        assert!(matches!(
            queue.pop_front().unwrap().msg,
            SapMsgInner::LtpdMleReportInd(LtpdMleReportInd {
                handle: RequestHandle(8),
                transfer_result: TransferResult::SuccessBufferEmpty,
            })
        ));
        assert_eq!(runtime.snapshot().links[0].successful_transfers, 1);
    }

    #[test]
    fn duplicate_handle_is_rejected_while_pending_and_after_completion() {
        let mut runtime = ready_runtime();
        let mut queue = MessageQueue::new();
        let now = TdmaTime::default();
        for _ in 0..2 {
            runtime.handle_primitive(
                &mut queue,
                SapMsg::new(
                    Sap::TlpdSap,
                    TetraEntity::Sndcp,
                    TetraEntity::Mle,
                    SapMsgInner::LtpdMleUnitdataReq(unitdata(9)),
                ),
                now,
            );
        }
        assert_eq!(runtime.snapshot().pending_transfer_count, 1);
        assert_eq!(runtime.snapshot().duplicate_handle_rejections, 1);
        assert!(queue.iter().any(|message| matches!(
            &message.msg,
            SapMsgInner::LtpdMleReportInd(LtpdMleReportInd {
                transfer_result: TransferResult::Other(RESULT_DUPLICATE_HANDLE),
                ..
            })
        )));

        let reporter = queue
            .iter()
            .find_map(|message| match &message.msg {
                SapMsgInner::TlaTlDataReqBl(request) => request.tx_reporter.clone(),
                _ => None,
            })
            .expect("TxReporter missing");
        reporter.mark_transmitted();
        reporter.mark_acknowledged();
        runtime.tick(&mut queue, now.add_timeslots(1));
        runtime.handle_primitive(
            &mut queue,
            SapMsg::new(
                Sap::TlpdSap,
                TetraEntity::Sndcp,
                TetraEntity::Mle,
                SapMsgInner::LtpdMleUnitdataReq(unitdata(9)),
            ),
            now.add_timeslots(2),
        );
        assert_eq!(runtime.snapshot().duplicate_handle_rejections, 2);
    }

    #[test]
    fn cancel_is_idempotent_and_does_not_orphan_pending_transfer() {
        let mut runtime = ready_runtime();
        let mut queue = MessageQueue::new();
        let now = TdmaTime::default();
        runtime.handle_primitive(
            &mut queue,
            SapMsg::new(
                Sap::TlpdSap,
                TetraEntity::Sndcp,
                TetraEntity::Mle,
                SapMsgInner::LtpdMleUnitdataReq(unitdata(10)),
            ),
            now,
        );
        runtime.handle_primitive(
            &mut queue,
            SapMsg::new(
                Sap::TlpdSap,
                TetraEntity::Sndcp,
                TetraEntity::Mle,
                SapMsgInner::LtpdMleCancelReq(LtpdMleCancelReq {
                    handle: RequestHandle(10),
                }),
            ),
            now,
        );
        assert_eq!(runtime.snapshot().pending_transfer_count, 0);
        assert_eq!(runtime.snapshot().cancelled_transfers, 1);

        runtime.handle_primitive(
            &mut queue,
            SapMsg::new(
                Sap::TlpdSap,
                TetraEntity::Sndcp,
                TetraEntity::Mle,
                SapMsgInner::LtpdMleCancelReq(LtpdMleCancelReq {
                    handle: RequestHandle(10),
                }),
            ),
            now,
        );
        assert!(queue.iter().any(|message| matches!(
            &message.msg,
            SapMsgInner::LtpdMleReportInd(LtpdMleReportInd {
                transfer_result: TransferResult::Other(RESULT_CANCEL_TOO_LATE),
                ..
            })
        )));
    }

    #[test]
    fn pending_transfer_times_out_without_tx_progress() {
        let mut runtime = ready_runtime();
        let mut queue = MessageQueue::new();
        let now = TdmaTime::default();
        runtime.handle_primitive(
            &mut queue,
            SapMsg::new(
                Sap::TlpdSap,
                TetraEntity::Sndcp,
                TetraEntity::Mle,
                SapMsgInner::LtpdMleUnitdataReq(unitdata(11)),
            ),
            now,
        );
        let _lower = queue.pop_front().expect("lower-layer request missing");
        runtime.tick(&mut queue, now.add_timeslots(TRANSFER_TIMEOUT_SLOTS));
        assert!(matches!(
            queue.pop_front().unwrap().msg,
            SapMsgInner::LtpdMleReportInd(LtpdMleReportInd {
                handle: RequestHandle(11),
                transfer_result: TransferResult::FailedRemovedFromBuffer,
            })
        ));
        assert_eq!(runtime.snapshot().timed_out_transfers, 1);
        assert_eq!(runtime.snapshot().pending_transfer_count, 0);
    }

    #[test]
    fn illegal_reconnect_from_connected_state_is_rejected() {
        let mut runtime = ready_runtime();
        let mut queue = MessageQueue::new();
        runtime.reconnect(
            &mut queue,
            LtpdMleReconnectReq {
                endpoint_id: 2,
                link_id: 3,
                reservation_information: tetra_saps::common::ReservationInfo::default(),
                pdu_priority: PduPriority::default(),
                encryption_flag: false,
                stealing_permission: StealingPermission::NotRequired,
            },
            TdmaTime::default(),
        );
        assert!(matches!(
            queue.pop_front().unwrap().msg,
            SapMsgInner::LtpdMleReconnectConfirm(LtpdMleReconnectConfirm {
                reconnection_result: ReconnectionResult::Reject,
                ..
            })
        ));
        assert_eq!(runtime.snapshot().invalid_transition_rejections, 1);
    }
}

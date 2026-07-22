//! Infrastructure-side MLE cell-change transaction runtime.
//!
//! The runtime owns the local state between an uplink U-PREPARE,
//! U-RESTORE or U-CHANNEL-REQUEST and the matching downlink response.  It is
//! intentionally kept inside the TBS because endpoint/link identifiers and
//! timers are local air-interface state.  A future Mobility Core may decide
//! the outcome through `MleCellChangeControl`, but it must not own this local
//! transaction registry.

use std::collections::HashMap;

use tetra_core::{BitBuffer, EndpointId, LinkId, TdmaTime, TetraAddress};
use tetra_pdus::mle::pdus::d_channel_response::DChannelResponse;
use tetra_pdus::mle::pdus::d_new_cell::DNewCell;
use tetra_pdus::mle::pdus::d_prepare_fail::DPrepareFail;
use tetra_pdus::mle::pdus::d_restore_ack::DRestoreAck;
use tetra_pdus::mle::pdus::d_restore_fail::DRestoreFail;
use tetra_pdus::mle::pdus::u_channel_request::UChannelRequest;
use tetra_pdus::mle::pdus::u_prepare::UPrepare;
use tetra_pdus::mle::pdus::u_restore::URestore;
use tetra_saps::common::{
    CellIdentity, MleChannelCommandValid, MleChannelRequestReason,
    MleChannelRequestRetryDelay, MleChannelResponseType, MleFailCause,
};
use tetra_saps::control::mle_cell_change::MleCellChangeControl;

/// Upper bound for a local cell-change decision.  432 slots are about 6.12 s
/// and deliberately match the robustness window used by the TLPD foundation.
pub const CELL_CHANGE_TRANSACTION_TIMEOUT_SLOTS: i32 = 432;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MleCellChangePhase {
    PrepareReceived,
    PrepareDeferred,
    NewCellGranted,
    RestoreReceived,
    Restored,
    Rejected,
    ChannelRequestReceived,
    ChannelResponseSent,
    TimedOut,
}

#[derive(Debug, Clone)]
struct CellChangeTransaction {
    subscriber: TetraAddress,
    endpoint_id: EndpointId,
    link_id: LinkId,
    phase: MleCellChangePhase,
    created_at: TdmaTime,
    updated_at: TdmaTime,
    cell_identifier_ca: Option<u8>,
    target_cell: Option<CellIdentity>,
    old_mcc: Option<u16>,
    old_mnc: Option<u16>,
    old_location_area: Option<u16>,
    last_channel_request_reason: Option<MleChannelRequestReason>,
    requested_channel_classes: Vec<u8>,
    requested_channels: Vec<u8>,
    embedded_sdu_bits: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MleCellChangeTransactionSnapshot {
    pub subscriber: TetraAddress,
    pub endpoint_id: EndpointId,
    pub link_id: LinkId,
    pub phase: MleCellChangePhase,
    pub created_at: TdmaTime,
    pub updated_at: TdmaTime,
    pub age_slots: i32,
    pub cell_identifier_ca: Option<u8>,
    pub target_cell: Option<CellIdentity>,
    pub old_mcc: Option<u16>,
    pub old_mnc: Option<u16>,
    pub old_location_area: Option<u16>,
    pub last_channel_request_reason: Option<MleChannelRequestReason>,
    pub requested_channel_classes: Vec<u8>,
    pub requested_channels: Vec<u8>,
    pub embedded_sdu_bits: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MleCellChangeCounters {
    pub prepares_received: u64,
    pub prepare_grants: u64,
    pub prepare_rejects: u64,
    pub restores_received: u64,
    pub restore_acknowledgements: u64,
    pub restore_rejects: u64,
    pub channel_requests_received: u64,
    pub channel_responses_sent: u64,
    pub invalid_controls: u64,
    pub parse_errors: u64,
    pub timeouts: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MleCellChangeRuntimeSnapshot {
    pub transactions: Vec<MleCellChangeTransactionSnapshot>,
    pub counters: MleCellChangeCounters,
    pub timeout_slots: i32,
}

#[derive(Debug, Clone)]
pub struct MleCellChangeOutbound {
    pub subscriber: TetraAddress,
    pub endpoint_id: EndpointId,
    pub link_id: LinkId,
    /// Encoded MLE PDU body.  The outer three-bit MLE protocol discriminator
    /// is added by `MleBs` before handing the SDU to LLC.
    pub pdu: BitBuffer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MleCellChangeError {
    UnknownTransaction(TetraAddress),
    InvalidPhase {
        subscriber: TetraAddress,
        phase: MleCellChangePhase,
        operation: &'static str,
    },
    EncodingFailed(&'static str),
}

#[derive(Debug, Default)]
pub struct MleCellChangeRuntime {
    transactions: HashMap<TetraAddress, CellChangeTransaction>,
    counters: MleCellChangeCounters,
}

impl MleCellChangeRuntime {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_parse_error(&mut self) {
        self.counters.parse_errors = self.counters.parse_errors.saturating_add(1);
    }

    pub fn observe_prepare(
        &mut self,
        subscriber: TetraAddress,
        endpoint_id: EndpointId,
        link_id: LinkId,
        pdu: &UPrepare,
        now: TdmaTime,
    ) {
        self.counters.prepares_received = self.counters.prepares_received.saturating_add(1);
        let transaction = CellChangeTransaction {
            subscriber,
            endpoint_id,
            link_id,
            phase: MleCellChangePhase::PrepareReceived,
            created_at: now,
            updated_at: now,
            cell_identifier_ca: pdu.cell_identifier_ca,
            target_cell: None,
            old_mcc: None,
            old_mnc: None,
            old_location_area: None,
            last_channel_request_reason: None,
            requested_channel_classes: Vec::new(),
            requested_channels: Vec::new(),
            embedded_sdu_bits: pdu.sdu.as_ref().map_or(0, BitBuffer::get_len),
        };
        self.transactions.insert(subscriber, transaction);
    }

    pub fn observe_restore(
        &mut self,
        subscriber: TetraAddress,
        endpoint_id: EndpointId,
        link_id: LinkId,
        pdu: &URestore,
        now: TdmaTime,
    ) {
        self.counters.restores_received = self.counters.restores_received.saturating_add(1);
        let transaction = CellChangeTransaction {
            subscriber,
            endpoint_id,
            link_id,
            phase: MleCellChangePhase::RestoreReceived,
            created_at: now,
            updated_at: now,
            cell_identifier_ca: None,
            target_cell: None,
            old_mcc: pdu.mcc,
            old_mnc: pdu.mnc,
            old_location_area: pdu.la,
            last_channel_request_reason: None,
            requested_channel_classes: Vec::new(),
            requested_channels: Vec::new(),
            embedded_sdu_bits: pdu.sdu.as_ref().map_or(0, BitBuffer::get_len),
        };
        self.transactions.insert(subscriber, transaction);
    }

    pub fn observe_channel_request(
        &mut self,
        subscriber: TetraAddress,
        endpoint_id: EndpointId,
        link_id: LinkId,
        pdu: &UChannelRequest,
        now: TdmaTime,
    ) {
        self.counters.channel_requests_received =
            self.counters.channel_requests_received.saturating_add(1);
        let transaction = CellChangeTransaction {
            subscriber,
            endpoint_id,
            link_id,
            phase: MleCellChangePhase::ChannelRequestReceived,
            created_at: now,
            updated_at: now,
            cell_identifier_ca: None,
            target_cell: None,
            old_mcc: None,
            old_mnc: None,
            old_location_area: None,
            last_channel_request_reason: Some(pdu.reason_for_the_channel_request),
            requested_channel_classes: pdu.requested_channel_class_identifiers.clone(),
            requested_channels: pdu.requested_channel_identifiers.clone(),
            embedded_sdu_bits: 0,
        };
        self.transactions.insert(subscriber, transaction);
    }

    pub fn handle_control(
        &mut self,
        control: MleCellChangeControl,
        now: TdmaTime,
    ) -> Result<MleCellChangeOutbound, MleCellChangeError> {
        let result = self.handle_control_inner(control, now);
        if result.is_err() {
            self.counters.invalid_controls = self.counters.invalid_controls.saturating_add(1);
        }
        result
    }

    fn handle_control_inner(
        &mut self,
        control: MleCellChangeControl,
        now: TdmaTime,
    ) -> Result<MleCellChangeOutbound, MleCellChangeError> {
        match control {
            MleCellChangeControl::GrantPrepare {
                subscriber,
                command,
                target_cell,
                mm_sdu,
            } => {
                let transaction = self.transaction_for_phase(
                    subscriber,
                    &[
                        MleCellChangePhase::PrepareReceived,
                        MleCellChangePhase::PrepareDeferred,
                    ],
                    "grant prepare",
                )?;
                transaction.target_cell = target_cell;
                transaction.phase = if command == MleChannelCommandValid::NoChannelChange {
                    MleCellChangePhase::PrepareDeferred
                } else {
                    MleCellChangePhase::NewCellGranted
                };
                transaction.updated_at = now;
                let route = (transaction.endpoint_id, transaction.link_id);
                self.counters.prepare_grants = self.counters.prepare_grants.saturating_add(1);
                let pdu = Self::encode_d_new_cell(command, mm_sdu)?;
                Ok(Self::outbound(subscriber, route, pdu))
            }
            MleCellChangeControl::RejectPrepare {
                subscriber,
                cause,
                mm_sdu,
            } => {
                let transaction = self.transaction_for_phase(
                    subscriber,
                    &[
                        MleCellChangePhase::PrepareReceived,
                        MleCellChangePhase::PrepareDeferred,
                    ],
                    "reject prepare",
                )?;
                transaction.phase = MleCellChangePhase::Rejected;
                transaction.updated_at = now;
                let route = (transaction.endpoint_id, transaction.link_id);
                self.counters.prepare_rejects = self.counters.prepare_rejects.saturating_add(1);
                let pdu = Self::encode_d_prepare_fail(cause, mm_sdu)?;
                Ok(Self::outbound(subscriber, route, pdu))
            }
            MleCellChangeControl::AcknowledgeRestore {
                subscriber,
                cmce_sdu,
            } => {
                let transaction = self.transaction_for_phase(
                    subscriber,
                    &[MleCellChangePhase::RestoreReceived],
                    "acknowledge restore",
                )?;
                transaction.phase = MleCellChangePhase::Restored;
                transaction.updated_at = now;
                let route = (transaction.endpoint_id, transaction.link_id);
                self.counters.restore_acknowledgements =
                    self.counters.restore_acknowledgements.saturating_add(1);
                let pdu = Self::encode_d_restore_ack(cmce_sdu)?;
                Ok(Self::outbound(subscriber, route, pdu))
            }
            MleCellChangeControl::RejectRestore { subscriber, cause } => {
                let transaction = self.transaction_for_phase(
                    subscriber,
                    &[MleCellChangePhase::RestoreReceived],
                    "reject restore",
                )?;
                transaction.phase = MleCellChangePhase::Rejected;
                transaction.updated_at = now;
                let route = (transaction.endpoint_id, transaction.link_id);
                self.counters.restore_rejects = self.counters.restore_rejects.saturating_add(1);
                let pdu = Self::encode_d_restore_fail(cause)?;
                Ok(Self::outbound(subscriber, route, pdu))
            }
            MleCellChangeControl::RespondChannelRequest {
                subscriber,
                response,
                reason,
                retry_delay,
            } => {
                let transaction = self.transaction_for_phase(
                    subscriber,
                    &[MleCellChangePhase::ChannelRequestReceived],
                    "respond to channel request",
                )?;
                transaction.phase = MleCellChangePhase::ChannelResponseSent;
                transaction.updated_at = now;
                let route = (transaction.endpoint_id, transaction.link_id);
                self.counters.channel_responses_sent =
                    self.counters.channel_responses_sent.saturating_add(1);
                let pdu = Self::encode_d_channel_response(response, reason, retry_delay)?;
                Ok(Self::outbound(subscriber, route, pdu))
            }
        }
    }

    /// Expire local transactions and generate deterministic negative responses.
    pub fn tick(&mut self, now: TdmaTime) -> Vec<MleCellChangeOutbound> {
        let expired: Vec<(TetraAddress, MleCellChangePhase)> = self
            .transactions
            .values()
            .filter(|transaction| {
                matches!(
                    transaction.phase,
                    MleCellChangePhase::PrepareReceived
                        | MleCellChangePhase::PrepareDeferred
                        | MleCellChangePhase::RestoreReceived
                        | MleCellChangePhase::ChannelRequestReceived
                ) && transaction.updated_at.age(now) >= CELL_CHANGE_TRANSACTION_TIMEOUT_SLOTS
            })
            .map(|transaction| (transaction.subscriber, transaction.phase))
            .collect();

        let mut outbound = Vec::with_capacity(expired.len());
        for (subscriber, phase) in expired {
            let Some(transaction) = self.transactions.get_mut(&subscriber) else {
                continue;
            };
            let route = (transaction.endpoint_id, transaction.link_id);
            transaction.phase = MleCellChangePhase::TimedOut;
            transaction.updated_at = now;
            self.counters.timeouts = self.counters.timeouts.saturating_add(1);

            let encoded = match phase {
                MleCellChangePhase::PrepareReceived | MleCellChangePhase::PrepareDeferred => {
                    Self::encode_d_prepare_fail(
                        MleFailCause::NeighbourCellEnquiryUnavailableOrTemporaryBreak,
                        None,
                    )
                }
                MleCellChangePhase::RestoreReceived => Self::encode_d_restore_fail(
                    MleFailCause::RestorationCannotBeDoneOnCell,
                ),
                MleCellChangePhase::ChannelRequestReceived => Self::encode_d_channel_response(
                    MleChannelResponseType::Rejected,
                    transaction
                        .last_channel_request_reason
                        .unwrap_or(MleChannelRequestReason::Unspecified),
                    MleChannelRequestRetryDelay::RetransmissionNotPermitted,
                ),
                _ => continue,
            };
            if let Ok(pdu) = encoded {
                outbound.push(Self::outbound(subscriber, route, pdu));
            }
        }
        outbound
    }

    pub fn snapshot(&self, now: TdmaTime) -> MleCellChangeRuntimeSnapshot {
        let mut transactions: Vec<_> = self
            .transactions
            .values()
            .map(|transaction| MleCellChangeTransactionSnapshot {
                subscriber: transaction.subscriber,
                endpoint_id: transaction.endpoint_id,
                link_id: transaction.link_id,
                phase: transaction.phase,
                created_at: transaction.created_at,
                updated_at: transaction.updated_at,
                age_slots: transaction.updated_at.age(now).max(0),
                cell_identifier_ca: transaction.cell_identifier_ca,
                target_cell: transaction.target_cell.clone(),
                old_mcc: transaction.old_mcc,
                old_mnc: transaction.old_mnc,
                old_location_area: transaction.old_location_area,
                last_channel_request_reason: transaction.last_channel_request_reason,
                requested_channel_classes: transaction.requested_channel_classes.clone(),
                requested_channels: transaction.requested_channels.clone(),
                embedded_sdu_bits: transaction.embedded_sdu_bits,
            })
            .collect();
        transactions.sort_by_key(|transaction| transaction.subscriber.ssi);
        MleCellChangeRuntimeSnapshot {
            transactions,
            counters: self.counters.clone(),
            timeout_slots: CELL_CHANGE_TRANSACTION_TIMEOUT_SLOTS,
        }
    }

    fn transaction_for_phase(
        &mut self,
        subscriber: TetraAddress,
        allowed: &[MleCellChangePhase],
        operation: &'static str,
    ) -> Result<&mut CellChangeTransaction, MleCellChangeError> {
        let transaction = self
            .transactions
            .get_mut(&subscriber)
            .ok_or(MleCellChangeError::UnknownTransaction(subscriber))?;
        if !allowed.contains(&transaction.phase) {
            return Err(MleCellChangeError::InvalidPhase {
                subscriber,
                phase: transaction.phase,
                operation,
            });
        }
        Ok(transaction)
    }

    fn outbound(
        subscriber: TetraAddress,
        route: (EndpointId, LinkId),
        pdu: BitBuffer,
    ) -> MleCellChangeOutbound {
        MleCellChangeOutbound {
            subscriber,
            endpoint_id: route.0,
            link_id: route.1,
            pdu,
        }
    }

    fn encode_d_new_cell(
        command: MleChannelCommandValid,
        sdu: Option<BitBuffer>,
    ) -> Result<BitBuffer, MleCellChangeError> {
        let mut buffer = BitBuffer::new_autoexpand(64);
        DNewCell {
            channel_command_valid: command,
            sdu,
        }
        .to_bitbuf(&mut buffer)
        .map_err(|_| MleCellChangeError::EncodingFailed("D-NEW-CELL"))?;
        buffer.seek(0);
        Ok(buffer)
    }

    fn encode_d_prepare_fail(
        cause: MleFailCause,
        sdu: Option<BitBuffer>,
    ) -> Result<BitBuffer, MleCellChangeError> {
        let mut buffer = BitBuffer::new_autoexpand(64);
        DPrepareFail {
            fail_cause: cause,
            sdu,
        }
        .to_bitbuf(&mut buffer)
        .map_err(|_| MleCellChangeError::EncodingFailed("D-PREPARE-FAIL"))?;
        buffer.seek(0);
        Ok(buffer)
    }

    fn encode_d_restore_ack(sdu: BitBuffer) -> Result<BitBuffer, MleCellChangeError> {
        let mut buffer = BitBuffer::new_autoexpand(64);
        DRestoreAck { sdu: Some(sdu) }
            .to_bitbuf(&mut buffer)
            .map_err(|_| MleCellChangeError::EncodingFailed("D-RESTORE-ACK"))?;
        buffer.seek(0);
        Ok(buffer)
    }

    fn encode_d_restore_fail(cause: MleFailCause) -> Result<BitBuffer, MleCellChangeError> {
        let mut buffer = BitBuffer::new_autoexpand(16);
        DRestoreFail { fail_cause: cause }
            .to_bitbuf(&mut buffer)
            .map_err(|_| MleCellChangeError::EncodingFailed("D-RESTORE-FAIL"))?;
        buffer.seek(0);
        Ok(buffer)
    }

    fn encode_d_channel_response(
        response: MleChannelResponseType,
        reason: MleChannelRequestReason,
        retry_delay: MleChannelRequestRetryDelay,
    ) -> Result<BitBuffer, MleCellChangeError> {
        let mut buffer = BitBuffer::new_autoexpand(24);
        DChannelResponse {
            channel_response_type: response,
            reason_for_the_channel_request: reason,
            channel_request_retry_delay: retry_delay,
            reserved1: None,
            reserved2: None,
        }
        .to_bitbuf(&mut buffer)
        .map_err(|_| MleCellChangeError::EncodingFailed("D-CHANNEL-RESPONSE"))?;
        buffer.seek(0);
        Ok(buffer)
    }
}

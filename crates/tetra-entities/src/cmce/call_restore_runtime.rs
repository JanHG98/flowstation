//! Local CMCE call-restoration transaction and context registry.
//!
//! The registry is deliberately local to the TBS. A future mobility/call core may
//! transport [`CallRestoreContext`] values between nodes, but the air-interface
//! transaction, endpoint/link binding and timeout remain owned by the target TBS.

use std::collections::HashMap;

use tetra_core::{SsiType, TdmaTime, TetraAddress, tetra_entities::TetraEntity};
use tetra_pdus::cmce::enums::{call_timeout::CallTimeout, transmission_grant::TransmissionGrant};
use tetra_saps::control::{
    call_control::NetworkCircuitCall,
    enums::{circuit_mode_type::CircuitModeType, communication_type::CommunicationType},
};

/// Bounded lifetime of an unanswered CMCE restore transaction (~6.1 seconds).
pub const CALL_RESTORE_TRANSACTION_TIMEOUT_SLOTS: i32 = 432;
/// Keep terminal results briefly so retransmitted U-RESTORE requests are idempotent.
pub const CALL_RESTORE_REPLAY_WINDOW_SLOTS: i32 = 18 * 4 * 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreCallKind {
    Group,
    Individual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestorePhase {
    Requested,
    ContextMatched,
    Queued,
    ResourceAllocated,
    Restored,
    Rejected,
    TimedOut,
    Superseded,
}

impl RestorePhase {
    #[inline]
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Restored | Self::Rejected | Self::TimedOut | Self::Superseded)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreRejectReason {
    MalformedPdu,
    UnknownCall,
    ParticipantMismatch,
    ServiceMismatch,
    NoRadioResource,
    InvalidState,
    DuplicateRequest,
    Timeout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupRestoreOrigin {
    Local { caller: TetraAddress },
    Network { network_entity: TetraEntity, brew_uuid: uuid::Uuid },
}

#[derive(Debug, Clone)]
pub struct GroupCallRestoreContext {
    pub call_id: u16,
    pub dest_gssi: u32,
    pub source_issi: u32,
    pub floor_holder: Option<u32>,
    pub priority: u8,
    pub call_timeout: CallTimeout,
    /// Original T310 start reference; call length continues across restoration.
    pub created_at: TdmaTime,
    pub tx_active: bool,
    pub origin: GroupRestoreOrigin,
    pub communication_type: CommunicationType,
    pub circuit_mode_type: CircuitModeType,
    pub speech_service: Option<u8>,
    pub etee_encrypted: bool,
}

#[derive(Debug, Clone)]
pub struct IndividualCallRestoreContext {
    pub call_id: u16,
    pub calling_addr: TetraAddress,
    pub called_addr: TetraAddress,
    pub simplex_duplex: bool,
    pub priority: u8,
    pub call_timeout: CallTimeout,
    /// Original active-call T310 start reference.
    pub active_timer_started: Option<TdmaTime>,
    pub floor_holder: Option<u32>,
    pub called_over_brew: bool,
    pub calling_over_brew: bool,
    pub brew_uuid: Option<uuid::Uuid>,
    pub network_entity: Option<TetraEntity>,
    pub network_call: Option<NetworkCircuitCall>,
    pub communication_type: CommunicationType,
    pub circuit_mode_type: CircuitModeType,
    pub speech_service: Option<u8>,
    pub etee_encrypted: bool,
}

#[derive(Debug, Clone)]
pub enum CallRestoreContext {
    Group(GroupCallRestoreContext),
    Individual(IndividualCallRestoreContext),
}

impl CallRestoreContext {
    #[inline]
    pub fn call_id(&self) -> u16 {
        match self {
            Self::Group(context) => context.call_id,
            Self::Individual(context) => context.call_id,
        }
    }

    #[inline]
    pub fn kind(&self) -> RestoreCallKind {
        match self {
            Self::Group(_) => RestoreCallKind::Group,
            Self::Individual(_) => RestoreCallKind::Individual,
        }
    }

    pub fn permits_subscriber(&self, subscriber: TetraAddress, other_party_ssi: Option<u32>) -> bool {
        match self {
            Self::Group(context) => {
                other_party_ssi.map_or(true, |ssi| ssi == context.dest_gssi)
                    && (subscriber.ssi == context.source_issi || subscriber.ssi_type == SsiType::Issi)
            }
            Self::Individual(context) => {
                let is_party = subscriber.ssi == context.calling_addr.ssi || subscriber.ssi == context.called_addr.ssi;
                let peer = if subscriber.ssi == context.calling_addr.ssi {
                    context.called_addr.ssi
                } else {
                    context.calling_addr.ssi
                };
                is_party && other_party_ssi.map_or(true, |ssi| ssi == peer)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RestoreTransactionKey {
    pub subscriber: TetraAddress,
    pub old_call_id: u16,
}

#[derive(Debug, Clone)]
pub struct CallRestoreRequest {
    pub subscriber: TetraAddress,
    pub old_call_id: u16,
    pub endpoint_id: u32,
    pub link_id: u32,
    pub request_to_transmit: bool,
    pub other_party_ssi: Option<u32>,
    pub previous_mcc: Option<u16>,
    pub previous_mnc: Option<u16>,
    pub previous_location_area: Option<u16>,
}

impl CallRestoreRequest {
    #[inline]
    pub fn key(&self) -> RestoreTransactionKey {
        RestoreTransactionKey {
            subscriber: self.subscriber,
            old_call_id: self.old_call_id,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CallRestoreTransaction {
    pub key: RestoreTransactionKey,
    pub endpoint_id: u32,
    pub link_id: u32,
    pub request_to_transmit: bool,
    pub other_party_ssi: Option<u32>,
    pub previous_mcc: Option<u16>,
    pub previous_mnc: Option<u16>,
    pub previous_location_area: Option<u16>,
    pub kind: Option<RestoreCallKind>,
    pub phase: RestorePhase,
    pub new_call_id: Option<u16>,
    pub timeslot: Option<u8>,
    pub usage: Option<u8>,
    pub request_to_transmit: bool,
    pub transmission_grant: Option<TransmissionGrant>,
    pub reject_reason: Option<RestoreRejectReason>,
    pub started_at: TdmaTime,
    pub updated_at: TdmaTime,
}

#[derive(Debug, Clone, Default)]
pub struct CallRestoreCounters {
    pub requests: u64,
    pub duplicate_requests: u64,
    pub contexts_installed: u64,
    pub contexts_matched: u64,
    pub queued_restores: u64,
    pub resources_allocated: u64,
    pub queued_allocations_completed: u64,
    pub group_restores: u64,
    pub individual_restores: u64,
    pub rejects: u64,
    pub timeouts: u64,
    pub call_id_changes: u64,
    pub floor_grants: u64,
    pub floor_grants_to_other: u64,
    /// A queued restoring participant issued or refreshed U-TX DEMAND.
    pub queued_tx_requests: u64,
    /// A queued restoring participant cancelled its transmission request with U-TX CEASED.
    pub queued_tx_cancellations: u64,
}

#[derive(Debug, Clone)]
pub struct CallRestoreTransactionSnapshot {
    pub subscriber: TetraAddress,
    pub old_call_id: u16,
    pub new_call_id: Option<u16>,
    pub kind: Option<RestoreCallKind>,
    pub phase: RestorePhase,
    pub endpoint_id: u32,
    pub link_id: u32,
    pub timeslot: Option<u8>,
    pub usage: Option<u8>,
    pub request_to_transmit: bool,
    pub transmission_grant: Option<TransmissionGrant>,
    pub reject_reason: Option<RestoreRejectReason>,
    pub age_slots: i32,
}

#[derive(Debug, Clone, Default)]
pub struct CallRestoreRuntimeSnapshot {
    pub contexts: usize,
    pub transactions: Vec<CallRestoreTransactionSnapshot>,
    pub counters: CallRestoreCounters,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallRestoreRuntimeError {
    DuplicatePending(RestoreTransactionKey),
    DuplicateQueued(RestoreTransactionKey),
    DuplicateTerminal(RestoreTransactionKey),
    UnknownTransaction(RestoreTransactionKey),
    InvalidPhase {
        key: RestoreTransactionKey,
        phase: RestorePhase,
    },
}

#[derive(Default)]
pub struct CallRestoreRuntime {
    contexts: HashMap<u16, CallRestoreContext>,
    transactions: HashMap<RestoreTransactionKey, CallRestoreTransaction>,
    /// Old call identifier to the call identifier allocated on this cell.
    call_id_aliases: HashMap<u16, u16>,
    counters: CallRestoreCounters,
}

impl CallRestoreRuntime {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn install_context(&mut self, context: CallRestoreContext) {
        self.contexts.insert(context.call_id(), context);
        self.counters.contexts_installed = self.counters.contexts_installed.saturating_add(1);
    }

    pub fn remove_context(&mut self, call_id: u16) -> Option<CallRestoreContext> {
        let aliases: Vec<u16> = self
            .call_id_aliases
            .iter()
            .filter_map(|(old, local)| ((*old == call_id) || (*local == call_id)).then_some(*old))
            .collect();
        let mut removed = self.contexts.remove(&call_id);
        for old in aliases {
            self.call_id_aliases.remove(&old);
            if removed.is_none() {
                removed = self.contexts.remove(&old);
            } else {
                self.contexts.remove(&old);
            }
        }
        removed
    }

    pub fn context(&self, call_id: u16) -> Option<&CallRestoreContext> {
        self.contexts.get(&call_id)
    }

    /// Resolve an old call identifier to the call identifier currently used on this cell.
    pub fn resolved_call_id(&self, old_call_id: u16) -> Option<u16> {
        self.call_id_aliases.get(&old_call_id).copied()
    }

    pub fn begin(
        &mut self,
        request: CallRestoreRequest,
        now: TdmaTime,
    ) -> Result<RestoreTransactionKey, CallRestoreRuntimeError> {
        let key = request.key();
        if let Some(existing) = self.transactions.get(&key) {
            self.counters.duplicate_requests = self.counters.duplicate_requests.saturating_add(1);
            if existing.phase == RestorePhase::Queued {
                return Err(CallRestoreRuntimeError::DuplicateQueued(key));
            }
            if existing.phase.is_terminal() && existing.updated_at.age(now) <= CALL_RESTORE_REPLAY_WINDOW_SLOTS {
                return Err(CallRestoreRuntimeError::DuplicateTerminal(key));
            }
            if !existing.phase.is_terminal() {
                return Err(CallRestoreRuntimeError::DuplicatePending(key));
            }
        }

        self.transactions.insert(
            key,
            CallRestoreTransaction {
                key,
                endpoint_id: request.endpoint_id,
                link_id: request.link_id,
                request_to_transmit: request.request_to_transmit,
                other_party_ssi: request.other_party_ssi,
                previous_mcc: request.previous_mcc,
                previous_mnc: request.previous_mnc,
                previous_location_area: request.previous_location_area,
                kind: None,
                phase: RestorePhase::Requested,
                new_call_id: None,
                timeslot: None,
                usage: None,
                transmission_grant: None,
                reject_reason: None,
                started_at: now,
                updated_at: now,
            },
        );
        self.counters.requests = self.counters.requests.saturating_add(1);
        Ok(key)
    }

    pub fn mark_context_matched(
        &mut self,
        key: RestoreTransactionKey,
        kind: RestoreCallKind,
        now: TdmaTime,
    ) -> Result<(), CallRestoreRuntimeError> {
        {
            let transaction = self.transaction_in_phase_mut(key, RestorePhase::Requested)?;
            transaction.kind = Some(kind);
            transaction.phase = RestorePhase::ContextMatched;
            transaction.updated_at = now;
        }
        self.counters.contexts_matched = self.counters.contexts_matched.saturating_add(1);
        Ok(())
    }

    pub fn reserve_call_id(&mut self, old_call_id: u16, local_call_id: u16) {
        self.call_id_aliases.insert(old_call_id, local_call_id);
    }

    pub fn mark_queued(
        &mut self,
        key: RestoreTransactionKey,
        call_id: u16,
        grant: TransmissionGrant,
        now: TdmaTime,
    ) -> Result<(), CallRestoreRuntimeError> {
        {
            let transaction = self.transaction_in_phase_mut(key, RestorePhase::ContextMatched)?;
            transaction.new_call_id = Some(call_id);
            transaction.transmission_grant = Some(grant);
            transaction.phase = RestorePhase::Queued;
            transaction.updated_at = now;
        }
        self.call_id_aliases.insert(key.old_call_id, call_id);
        self.counters.queued_restores = self.counters.queued_restores.saturating_add(1);
        if call_id != key.old_call_id {
            self.counters.call_id_changes = self.counters.call_id_changes.saturating_add(1);
        }
        Ok(())
    }

    pub fn queued_transactions(&self) -> Vec<CallRestoreTransaction> {
        self.transactions
            .values()
            .filter(|transaction| transaction.phase == RestorePhase::Queued)
            .cloned()
            .collect()
    }

    /// Find a queued restore transaction by either its old or target-cell call identifier.
    pub fn queued_key_for_call(
        &self,
        subscriber: TetraAddress,
        call_id: u16,
    ) -> Option<RestoreTransactionKey> {
        self.transactions.values().find_map(|transaction| {
            (transaction.phase == RestorePhase::Queued
                && transaction.key.subscriber == subscriber
                && (transaction.key.old_call_id == call_id
                    || transaction.new_call_id == Some(call_id)))
            .then_some(transaction.key)
        })
    }

    /// Update the transmission request associated with a queued restoration.
    ///
    /// ETSI permits a queued restoring MS to issue U-TX DEMAND or cancel the
    /// request with U-TX CEASED while it is still waiting for a traffic bearer.
    pub fn set_queued_transmission_request(
        &mut self,
        key: RestoreTransactionKey,
        requested: bool,
        now: TdmaTime,
    ) -> Result<TransmissionGrant, CallRestoreRuntimeError> {
        let grant = if requested {
            TransmissionGrant::RequestQueued
        } else {
            TransmissionGrant::NotGranted
        };
        {
            let transaction = self.transaction_in_phase_mut(key, RestorePhase::Queued)?;
            transaction.request_to_transmit = requested;
            transaction.transmission_grant = Some(grant);
            transaction.updated_at = now;
        }
        if requested {
            self.counters.queued_tx_requests =
                self.counters.queued_tx_requests.saturating_add(1);
        } else {
            self.counters.queued_tx_cancellations =
                self.counters.queued_tx_cancellations.saturating_add(1);
        }
        Ok(grant)
    }

    pub fn mark_resource_allocated(
        &mut self,
        key: RestoreTransactionKey,
        call_id: u16,
        timeslot: u8,
        usage: u8,
        now: TdmaTime,
    ) -> Result<(), CallRestoreRuntimeError> {
        let old_call_id = key.old_call_id;
        let call_id_alias_changed = self.call_id_aliases.get(&old_call_id).copied() != Some(call_id);
        let was_queued = self
            .transactions
            .get(&key)
            .ok_or(CallRestoreRuntimeError::UnknownTransaction(key))?
            .phase
            == RestorePhase::Queued;
        {
            let transaction = self
                .transactions
                .get_mut(&key)
                .ok_or(CallRestoreRuntimeError::UnknownTransaction(key))?;
            if !matches!(transaction.phase, RestorePhase::ContextMatched | RestorePhase::Queued) {
                return Err(CallRestoreRuntimeError::InvalidPhase {
                    key,
                    phase: transaction.phase,
                });
            }
            transaction.new_call_id = Some(call_id);
            transaction.timeslot = Some(timeslot);
            transaction.usage = Some(usage);
            transaction.phase = RestorePhase::ResourceAllocated;
            transaction.updated_at = now;
        }
        self.counters.resources_allocated = self.counters.resources_allocated.saturating_add(1);
        if was_queued {
            self.counters.queued_allocations_completed =
                self.counters.queued_allocations_completed.saturating_add(1);
        }
        self.call_id_aliases.insert(old_call_id, call_id);
        if call_id != old_call_id && call_id_alias_changed {
            self.counters.call_id_changes = self.counters.call_id_changes.saturating_add(1);
        }
        Ok(())
    }

    pub fn mark_restored(
        &mut self,
        key: RestoreTransactionKey,
        call_id: u16,
        grant: TransmissionGrant,
        now: TdmaTime,
    ) -> Result<(), CallRestoreRuntimeError> {
        let phase = self
            .transactions
            .get(&key)
            .ok_or(CallRestoreRuntimeError::UnknownTransaction(key))?
            .phase;
        if !matches!(
            phase,
            RestorePhase::ContextMatched | RestorePhase::Queued | RestorePhase::ResourceAllocated
        ) {
            return Err(CallRestoreRuntimeError::InvalidPhase { key, phase });
        }
        let kind = {
            let transaction = self.transactions.get_mut(&key).expect("transaction checked above");
            transaction.new_call_id = Some(call_id);
            transaction.transmission_grant = Some(grant);
            transaction.phase = RestorePhase::Restored;
            transaction.updated_at = now;
            transaction.kind
        };
        self.call_id_aliases.insert(key.old_call_id, call_id);
        match kind {
            Some(RestoreCallKind::Group) => self.counters.group_restores = self.counters.group_restores.saturating_add(1),
            Some(RestoreCallKind::Individual) => {
                self.counters.individual_restores = self.counters.individual_restores.saturating_add(1)
            }
            None => {}
        }
        match grant {
            TransmissionGrant::Granted => self.counters.floor_grants = self.counters.floor_grants.saturating_add(1),
            TransmissionGrant::GrantedToOtherUser => {
                self.counters.floor_grants_to_other = self.counters.floor_grants_to_other.saturating_add(1)
            }
            TransmissionGrant::NotGranted | TransmissionGrant::RequestQueued => {}
        }
        Ok(())
    }

    pub fn reject(
        &mut self,
        key: RestoreTransactionKey,
        reason: RestoreRejectReason,
        now: TdmaTime,
    ) -> Result<(), CallRestoreRuntimeError> {
        {
            let transaction = self
                .transactions
                .get_mut(&key)
                .ok_or(CallRestoreRuntimeError::UnknownTransaction(key))?;
            if transaction.phase.is_terminal() {
                return Err(CallRestoreRuntimeError::InvalidPhase {
                    key,
                    phase: transaction.phase,
                });
            }
            transaction.phase = RestorePhase::Rejected;
            transaction.reject_reason = Some(reason);
            transaction.updated_at = now;
        }
        self.counters.rejects = self.counters.rejects.saturating_add(1);
        Ok(())
    }

    pub fn tick(&mut self, now: TdmaTime) -> Vec<RestoreTransactionKey> {
        let mut expired = Vec::new();
        for (key, transaction) in self.transactions.iter_mut() {
            if !transaction.phase.is_terminal()
                && transaction.started_at.age(now) >= CALL_RESTORE_TRANSACTION_TIMEOUT_SLOTS
            {
                transaction.phase = RestorePhase::TimedOut;
                transaction.reject_reason = Some(RestoreRejectReason::Timeout);
                transaction.updated_at = now;
                self.counters.timeouts = self.counters.timeouts.saturating_add(1);
                expired.push(*key);
            }
        }
        self.transactions.retain(|_, transaction| {
            !transaction.phase.is_terminal()
                || transaction.updated_at.age(now) <= CALL_RESTORE_REPLAY_WINDOW_SLOTS
        });
        expired
    }

    pub fn transaction(&self, key: RestoreTransactionKey) -> Option<&CallRestoreTransaction> {
        self.transactions.get(&key)
    }

    pub fn snapshot(&self, now: TdmaTime) -> CallRestoreRuntimeSnapshot {
        let mut transactions: Vec<_> = self
            .transactions
            .values()
            .map(|transaction| CallRestoreTransactionSnapshot {
                subscriber: transaction.key.subscriber,
                old_call_id: transaction.key.old_call_id,
                new_call_id: transaction.new_call_id,
                kind: transaction.kind,
                phase: transaction.phase,
                endpoint_id: transaction.endpoint_id,
                link_id: transaction.link_id,
                timeslot: transaction.timeslot,
                usage: transaction.usage,
                request_to_transmit: transaction.request_to_transmit,
                transmission_grant: transaction.transmission_grant,
                reject_reason: transaction.reject_reason,
                age_slots: transaction.started_at.age(now),
            })
            .collect();
        transactions.sort_by_key(|transaction| (transaction.subscriber.ssi, transaction.old_call_id));
        CallRestoreRuntimeSnapshot {
            contexts: self.contexts.len(),
            transactions,
            counters: self.counters.clone(),
        }
    }

    fn transaction_in_phase_mut(
        &mut self,
        key: RestoreTransactionKey,
        expected: RestorePhase,
    ) -> Result<&mut CallRestoreTransaction, CallRestoreRuntimeError> {
        let transaction = self
            .transactions
            .get_mut(&key)
            .ok_or(CallRestoreRuntimeError::UnknownTransaction(key))?;
        if transaction.phase != expected {
            return Err(CallRestoreRuntimeError::InvalidPhase {
                key,
                phase: transaction.phase,
            });
        }
        Ok(transaction)
    }
}

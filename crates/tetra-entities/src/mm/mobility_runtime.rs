//! Infrastructure-side MM mobility transaction state.
//!
//! This module owns the local, air-interface-adjacent part of migration and
//! forward registration.  It deliberately does not implement an ISI or Core
//! wire protocol.  A future `mobility-core` service can transport the exported
//! [`MmClientMobilityContext`] between TBS nodes, while this runtime retains the
//! ETSI timers, temporary identities and local transaction lifecycle.

use std::collections::HashMap;

use tetra_core::{TdmaTime, TetraAddress};
use tetra_pdus::mm::enums::location_update_type::LocationUpdateType;
use tetra_pdus::mm::enums::reject_cause::RejectCause;
use tetra_pdus::mm::pdus::u_location_update_demand::ULocationUpdateDemand;

use super::components::client_state::MmClientMobilityContext;

/// Pending mobility transactions are bounded to the same conservative window
/// used by the Foundation runtimes: 432 timeslots (one multiframe).
pub const MM_MOBILITY_TIMEOUT_SLOTS: i32 = 432;

/// Terminal transactions remain visible for diagnostics for two multiframes and
/// are then removed so VASSIs and subscriber keys can be reused safely.
pub const MM_MOBILITY_HISTORY_SLOTS: i32 = MM_MOBILITY_TIMEOUT_SLOTS * 2;

/// Default local VASSI pool used by the standalone/test profile.
///
/// The range is deliberately high in the 24-bit SSI space.  A future
/// Subscriber/Mobility Core will make this pool configurable and globally
/// authoritative.  Until then allocation is collision checked against the
/// local client registry and all active transactions.
pub const DEFAULT_VASSI_MIN: u32 = 0xE0_0000;
pub const DEFAULT_VASSI_MAX: u32 = 0xEF_FFFE;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MmMobilityPhase {
    MigrationRequested,
    ProceedingSent,
    AwaitingSecondDemand,
    MigrationAccepted,
    MigrationRejected,
    ForwardRegistrationRequested,
    ForwardRegistrationAccepted,
    ForwardRegistrationRejected,
    ContextTransferred,
    TimedOut,
}

#[derive(Debug, Clone)]
struct MigrationTransaction {
    original_layer2_address: TetraAddress,
    vassi: u32,
    home_issi: Option<u32>,
    home_mni: u32,
    handle: u32,
    location_update_type: LocationUpdateType,
    service_restoration: bool,
    phase: MmMobilityPhase,
    started_at: TdmaTime,
    updated_at: TdmaTime,
    imported_context: Option<MmClientMobilityContext>,
    reject_cause: Option<RejectCause>,
}

#[derive(Debug, Clone)]
struct ForwardRegistrationTransaction {
    subscriber: TetraAddress,
    target_location_area: u16,
    cell_identifier_ca: Option<u8>,
    phase: MmMobilityPhase,
    started_at: TdmaTime,
    updated_at: TdmaTime,
    context: MmClientMobilityContext,
    reject_cause: Option<RejectCause>,
}

#[derive(Debug, Clone)]
pub struct MigrationCompletion {
    pub local_issi: u32,
    pub home_issi: u32,
    pub home_mni: u32,
    pub service_restoration: bool,
    pub imported_context: Option<MmClientMobilityContext>,
}

#[derive(Debug, Clone)]
pub struct ForwardRegistrationCompletion {
    pub subscriber: TetraAddress,
    pub target_location_area: u16,
    pub cell_identifier_ca: Option<u8>,
    pub context: MmClientMobilityContext,
}

#[derive(Debug, Clone)]
pub enum MmMobilityTimeout {
    Migration {
        subscriber: TetraAddress,
        handle: u32,
        location_update_type: LocationUpdateType,
        address_extension: Option<u64>,
    },
    ForwardRegistration {
        subscriber: TetraAddress,
    },
}

#[derive(Debug, Clone, Default)]
pub struct MmMobilityCounters {
    pub migration_requests: u64,
    pub proceeding_sent: u64,
    pub migration_accepts: u64,
    pub migration_rejects: u64,
    pub forward_registration_requests: u64,
    pub forward_registration_accepts: u64,
    pub forward_registration_rejects: u64,
    pub context_exports: u64,
    pub context_imports: u64,
    pub duplicate_requests: u64,
    pub identity_mismatches: u64,
    pub timeouts: u64,
}

#[derive(Debug, Clone)]
pub struct MmMigrationSnapshot {
    pub original_layer2_address: TetraAddress,
    pub vassi: u32,
    pub home_issi: Option<u32>,
    pub home_mni: u32,
    pub phase: MmMobilityPhase,
    pub age_slots: i32,
    pub service_restoration: bool,
    pub has_imported_context: bool,
    pub reject_cause: Option<RejectCause>,
}

#[derive(Debug, Clone)]
pub struct MmForwardRegistrationSnapshot {
    pub subscriber: TetraAddress,
    pub target_location_area: u16,
    pub cell_identifier_ca: Option<u8>,
    pub phase: MmMobilityPhase,
    pub age_slots: i32,
    pub group_count: usize,
    pub reject_cause: Option<RejectCause>,
}

#[derive(Debug, Clone, Default)]
pub struct MmMobilityRuntimeSnapshot {
    pub migrations: Vec<MmMigrationSnapshot>,
    pub forward_registrations: Vec<MmForwardRegistrationSnapshot>,
    pub counters: MmMobilityCounters,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MmMobilityError {
    MissingHomeMni,
    MissingHomeIssi,
    VassiPoolExhausted,
    UnknownVassi(u32),
    DuplicateMigration(u32),
    DuplicateForwardRegistration(u32),
    IdentityMismatch,
    InvalidForwardRegistration,
}

#[derive(Debug)]
pub struct MmMobilityRuntime {
    migrations_by_vassi: HashMap<u32, MigrationTransaction>,
    vassi_by_original_ssi: HashMap<u32, u32>,
    /// Durable local-to-home identity mapping for migrated subscribers.
    ///
    /// Migration transactions are intentionally removed after a bounded
    /// diagnostic history, while the subscriber can remain registered under
    /// its VASSI for much longer. Admission policy checks therefore must not
    /// depend on the transaction history itself.
    home_issi_by_local_issi: HashMap<u32, u32>,
    forward_registrations: HashMap<u32, ForwardRegistrationTransaction>,
    next_vassi: u32,
    counters: MmMobilityCounters,
}

impl Default for MmMobilityRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl MmMobilityRuntime {
    pub fn new() -> Self {
        Self {
            migrations_by_vassi: HashMap::new(),
            vassi_by_original_ssi: HashMap::new(),
            home_issi_by_local_issi: HashMap::new(),
            forward_registrations: HashMap::new(),
            next_vassi: DEFAULT_VASSI_MIN,
            counters: MmMobilityCounters::default(),
        }
    }

    /// Resolve a local air-interface SSI to the subscriber's home ISSI.
    ///
    /// Migrated subscribers can be registered locally under a VASSI. Central
    /// admission policies are expressed in home identities, so callers must
    /// translate the local VASSI before deciding whether an existing
    /// registration is still authorized.
    pub fn home_issi_for_local(&self, local_issi: u32) -> Option<u32> {
        self.home_issi_by_local_issi
            .get(&local_issi)
            .copied()
            .or_else(|| {
                self.migrations_by_vassi
                    .get(&local_issi)
                    .and_then(|transaction| transaction.home_issi)
            })
    }

    /// Register a durable identity mapping for a locally imported or migrated
    /// subscriber context.
    pub fn register_local_identity(&mut self, local_issi: u32, home_issi: u32) {
        if local_issi != home_issi {
            self.home_issi_by_local_issi.insert(local_issi, home_issi);
        } else {
            self.home_issi_by_local_issi.remove(&local_issi);
        }
    }

    /// Remove a durable local-to-home mapping when the local mobility context
    /// is explicitly detached or transferred away.
    pub fn forget_local_identity(&mut self, local_issi: u32) {
        self.home_issi_by_local_issi.remove(&local_issi);
    }

    pub fn begin_migration<F>(
        &mut self,
        subscriber: TetraAddress,
        handle: u32,
        pdu: &ULocationUpdateDemand,
        now: TdmaTime,
        is_local_ssi_in_use: F,
    ) -> Result<(u32, u32), MmMobilityError>
    where
        F: Fn(u32) -> bool,
    {
        let home_mni = pdu
            .address_extension
            .and_then(|value| u32::try_from(value).ok())
            .ok_or(MmMobilityError::MissingHomeMni)?;

        if let Some(existing) = self.vassi_by_original_ssi.get(&subscriber.ssi).copied() {
            let active = self
                .migrations_by_vassi
                .get(&existing)
                .is_some_and(|transaction| {
                    matches!(
                        transaction.phase,
                        MmMobilityPhase::MigrationRequested
                            | MmMobilityPhase::ProceedingSent
                            | MmMobilityPhase::AwaitingSecondDemand
                    )
                });
            if active {
                self.counters.duplicate_requests += 1;
                let transaction = self
                    .migrations_by_vassi
                    .get(&existing)
                    .ok_or(MmMobilityError::DuplicateMigration(subscriber.ssi))?;
                return Ok((transaction.vassi, transaction.home_mni));
            }
            self.vassi_by_original_ssi.remove(&subscriber.ssi);
        }

        let vassi = self.allocate_vassi(is_local_ssi_in_use)?;
        let service_restoration = matches!(
            pdu.location_update_type,
            LocationUpdateType::ServiceRestorationMigratingLocationUpdating
        );
        let transaction = MigrationTransaction {
            original_layer2_address: subscriber,
            vassi,
            home_issi: pdu.ssi.and_then(|value| u32::try_from(value).ok()),
            home_mni,
            handle,
            location_update_type: pdu.location_update_type,
            service_restoration,
            phase: MmMobilityPhase::ProceedingSent,
            started_at: now,
            updated_at: now,
            imported_context: None,
            reject_cause: None,
        };
        self.vassi_by_original_ssi.insert(subscriber.ssi, vassi);
        self.migrations_by_vassi.insert(vassi, transaction);
        self.counters.migration_requests += 1;
        self.counters.proceeding_sent += 1;
        Ok((vassi, home_mni))
    }

    pub fn has_pending_vassi(&self, vassi: u32) -> bool {
        self.migrations_by_vassi
            .get(&vassi)
            .is_some_and(|tx| {
                matches!(
                    tx.phase,
                    MmMobilityPhase::ProceedingSent | MmMobilityPhase::AwaitingSecondDemand
                )
            })
    }

    pub fn provide_migration_context(
        &mut self,
        vassi: u32,
        context: MmClientMobilityContext,
        now: TdmaTime,
    ) -> Result<(), MmMobilityError> {
        let transaction = self
            .migrations_by_vassi
            .get_mut(&vassi)
            .ok_or(MmMobilityError::UnknownVassi(vassi))?;
        transaction.imported_context = Some(context);
        transaction.updated_at = now;
        self.counters.context_imports += 1;
        Ok(())
    }

    pub fn complete_migration(
        &mut self,
        vassi: u32,
        pdu: &ULocationUpdateDemand,
        now: TdmaTime,
    ) -> Result<MigrationCompletion, MmMobilityError> {
        let transaction = self
            .migrations_by_vassi
            .get_mut(&vassi)
            .ok_or(MmMobilityError::UnknownVassi(vassi))?;

        if pdu.location_update_type != LocationUpdateType::DemandLocationUpdating {
            self.counters.identity_mismatches += 1;
            return Err(MmMobilityError::IdentityMismatch);
        }

        let home_issi = pdu
            .ssi
            .and_then(|value| u32::try_from(value).ok())
            .or(transaction.home_issi)
            .ok_or(MmMobilityError::MissingHomeIssi)?;
        let home_mni = pdu
            .address_extension
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or(transaction.home_mni);

        if home_mni != transaction.home_mni
            || transaction.home_issi.is_some_and(|expected| expected != home_issi)
        {
            transaction.phase = MmMobilityPhase::MigrationRejected;
            transaction.reject_cause = Some(RejectCause::MessageConsistencyError);
            transaction.updated_at = now;
            self.counters.identity_mismatches += 1;
            self.counters.migration_rejects += 1;
            return Err(MmMobilityError::IdentityMismatch);
        }

        transaction.home_issi = Some(home_issi);
        transaction.phase = MmMobilityPhase::MigrationAccepted;
        transaction.updated_at = now;
        self.home_issi_by_local_issi.insert(vassi, home_issi);
        self.counters.migration_accepts += 1;
        let completion = MigrationCompletion {
            local_issi: vassi,
            home_issi,
            home_mni,
            service_restoration: transaction.service_restoration,
            imported_context: transaction.imported_context.clone(),
        };
        Ok(completion)
    }

    pub fn reject_migration(
        &mut self,
        vassi: u32,
        cause: RejectCause,
        now: TdmaTime,
    ) -> Result<(), MmMobilityError> {
        let transaction = self
            .migrations_by_vassi
            .get_mut(&vassi)
            .ok_or(MmMobilityError::UnknownVassi(vassi))?;
        transaction.phase = MmMobilityPhase::MigrationRejected;
        transaction.reject_cause = Some(cause);
        transaction.updated_at = now;
        self.counters.migration_rejects += 1;
        Ok(())
    }

    pub fn begin_forward_registration(
        &mut self,
        subscriber: TetraAddress,
        cell_identifier_ca: Option<u8>,
        pdu: &ULocationUpdateDemand,
        context: MmClientMobilityContext,
        now: TdmaTime,
    ) -> Result<ForwardRegistrationCompletion, MmMobilityError> {
        if !matches!(
            pdu.location_update_type,
            LocationUpdateType::ServiceRestorationRoamingLocationUpdating
                | LocationUpdateType::ServiceRestorationMigratingLocationUpdating
        ) {
            return Err(MmMobilityError::InvalidForwardRegistration);
        }
        let target_location_area = pdu
            .la_information
            .and_then(|value| u16::try_from(value).ok())
            .ok_or(MmMobilityError::InvalidForwardRegistration)?;

        if let Some(existing) = self.forward_registrations.get(&subscriber.ssi) {
            if matches!(existing.phase, MmMobilityPhase::ForwardRegistrationRequested) {
                self.counters.duplicate_requests += 1;
                return Err(MmMobilityError::DuplicateForwardRegistration(subscriber.ssi));
            }
            self.forward_registrations.remove(&subscriber.ssi);
        }

        let completion = ForwardRegistrationCompletion {
            subscriber,
            target_location_area,
            cell_identifier_ca,
            context: context.clone(),
        };
        self.forward_registrations.insert(
            subscriber.ssi,
            ForwardRegistrationTransaction {
                subscriber,
                target_location_area,
                cell_identifier_ca,
                phase: MmMobilityPhase::ForwardRegistrationRequested,
                started_at: now,
                updated_at: now,
                context,
                reject_cause: None,
            },
        );
        self.counters.forward_registration_requests += 1;
        self.counters.context_exports += 1;
        Ok(completion)
    }

    pub fn accept_forward_registration(
        &mut self,
        issi: u32,
        now: TdmaTime,
    ) -> Result<(), MmMobilityError> {
        let transaction = self
            .forward_registrations
            .get_mut(&issi)
            .ok_or(MmMobilityError::InvalidForwardRegistration)?;
        transaction.phase = MmMobilityPhase::ForwardRegistrationAccepted;
        transaction.updated_at = now;
        self.counters.forward_registration_accepts += 1;
        Ok(())
    }

    pub fn reject_forward_registration(
        &mut self,
        issi: u32,
        cause: RejectCause,
        now: TdmaTime,
    ) -> Result<(), MmMobilityError> {
        let transaction = self
            .forward_registrations
            .get_mut(&issi)
            .ok_or(MmMobilityError::InvalidForwardRegistration)?;
        transaction.phase = MmMobilityPhase::ForwardRegistrationRejected;
        transaction.reject_cause = Some(cause);
        transaction.updated_at = now;
        self.counters.forward_registration_rejects += 1;
        Ok(())
    }

    pub fn take_forward_context(&mut self, issi: u32) -> Option<MmClientMobilityContext> {
        let transaction = self.forward_registrations.get_mut(&issi)?;
        transaction.phase = MmMobilityPhase::ContextTransferred;
        self.counters.context_exports += 1;
        Some(transaction.context.clone())
    }

    pub fn tick(&mut self, now: TdmaTime) -> Vec<MmMobilityTimeout> {
        let mut timeouts = Vec::new();

        let expired_migrations: Vec<u32> = self
            .migrations_by_vassi
            .iter()
            .filter_map(|(&vassi, transaction)| {
                let active = matches!(
                    transaction.phase,
                    MmMobilityPhase::MigrationRequested
                        | MmMobilityPhase::ProceedingSent
                        | MmMobilityPhase::AwaitingSecondDemand
                );
                (active
                    && now.diff(transaction.updated_at)
                        >= MM_MOBILITY_TIMEOUT_SLOTS)
                    .then_some(vassi)
            })
            .collect();
        for vassi in expired_migrations {
            if let Some(transaction) = self.migrations_by_vassi.get_mut(&vassi) {
                transaction.phase = MmMobilityPhase::TimedOut;
                transaction.reject_cause = Some(RejectCause::ExpiryOfTimer);
                transaction.updated_at = now;
                timeouts.push(MmMobilityTimeout::Migration {
                    subscriber: transaction.original_layer2_address,
                    handle: transaction.handle,
                    location_update_type: transaction.location_update_type,
                    address_extension: Some(transaction.home_mni as u64),
                });
                self.counters.timeouts += 1;
            }
        }

        let expired_forwards: Vec<u32> = self
            .forward_registrations
            .iter()
            .filter_map(|(&issi, transaction)| {
                let active = matches!(
                    transaction.phase,
                    MmMobilityPhase::ForwardRegistrationRequested
                );
                (active
                    && now.diff(transaction.updated_at)
                        >= MM_MOBILITY_TIMEOUT_SLOTS)
                    .then_some(issi)
            })
            .collect();
        for issi in expired_forwards {
            if let Some(transaction) = self.forward_registrations.get_mut(&issi) {
                transaction.phase = MmMobilityPhase::TimedOut;
                transaction.reject_cause = Some(RejectCause::ForwardRegistrationFailure);
                transaction.updated_at = now;
                timeouts.push(MmMobilityTimeout::ForwardRegistration {
                    subscriber: transaction.subscriber,
                });
                self.counters.timeouts += 1;
            }
        }

        // Keep a bounded terminal history for WebUI diagnostics, then release VASSIs
        // and subscriber keys for future mobility procedures.
        let stale_migrations: Vec<u32> = self
            .migrations_by_vassi
            .iter()
            .filter_map(|(&vassi, transaction)| {
                let terminal = matches!(
                    transaction.phase,
                    MmMobilityPhase::MigrationAccepted
                        | MmMobilityPhase::MigrationRejected
                        | MmMobilityPhase::TimedOut
                );
                (terminal && now.diff(transaction.updated_at) >= MM_MOBILITY_HISTORY_SLOTS)
                    .then_some(vassi)
            })
            .collect();
        for vassi in stale_migrations {
            if let Some(transaction) = self.migrations_by_vassi.remove(&vassi) {
                self.vassi_by_original_ssi
                    .remove(&transaction.original_layer2_address.ssi);
            }
        }

        let stale_forwards: Vec<u32> = self
            .forward_registrations
            .iter()
            .filter_map(|(&issi, transaction)| {
                let terminal = !matches!(
                    transaction.phase,
                    MmMobilityPhase::ForwardRegistrationRequested
                );
                (terminal && now.diff(transaction.updated_at) >= MM_MOBILITY_HISTORY_SLOTS)
                    .then_some(issi)
            })
            .collect();
        for issi in stale_forwards {
            self.forward_registrations.remove(&issi);
        }

        timeouts
    }

    pub fn snapshot(&self, now: TdmaTime) -> MmMobilityRuntimeSnapshot {
        let mut migrations = self
            .migrations_by_vassi
            .values()
            .map(|transaction| MmMigrationSnapshot {
                original_layer2_address: transaction.original_layer2_address,
                vassi: transaction.vassi,
                home_issi: transaction.home_issi,
                home_mni: transaction.home_mni,
                phase: transaction.phase,
                age_slots: now.diff(transaction.started_at),
                service_restoration: transaction.service_restoration,
                has_imported_context: transaction.imported_context.is_some(),
                reject_cause: transaction.reject_cause,
            })
            .collect::<Vec<_>>();
        migrations.sort_by_key(|entry| entry.vassi);

        let mut forward_registrations = self
            .forward_registrations
            .values()
            .map(|transaction| MmForwardRegistrationSnapshot {
                subscriber: transaction.subscriber,
                target_location_area: transaction.target_location_area,
                cell_identifier_ca: transaction.cell_identifier_ca,
                phase: transaction.phase,
                age_slots: now.diff(transaction.started_at),
                group_count: transaction.context.groups.len(),
                reject_cause: transaction.reject_cause,
            })
            .collect::<Vec<_>>();
        forward_registrations.sort_by_key(|entry| entry.subscriber.ssi);

        MmMobilityRuntimeSnapshot {
            migrations,
            forward_registrations,
            counters: self.counters.clone(),
        }
    }

    fn allocate_vassi<F>(&mut self, is_local_ssi_in_use: F) -> Result<u32, MmMobilityError>
    where
        F: Fn(u32) -> bool,
    {
        let pool_size = DEFAULT_VASSI_MAX - DEFAULT_VASSI_MIN + 1;
        for _ in 0..pool_size {
            let candidate = self.next_vassi;
            self.next_vassi = if self.next_vassi >= DEFAULT_VASSI_MAX {
                DEFAULT_VASSI_MIN
            } else {
                self.next_vassi + 1
            };
            if !is_local_ssi_in_use(candidate)
                && !self.migrations_by_vassi.contains_key(&candidate)
            {
                return Ok(candidate);
            }
        }
        Err(MmMobilityError::VassiPoolExhausted)
    }
}

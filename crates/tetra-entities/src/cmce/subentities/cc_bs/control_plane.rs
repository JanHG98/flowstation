//! Adapter between the central Call Control service and the local CMCE state machine.
//!
//! The central service owns logical calls. This module deliberately keeps radio
//! allocation, air-interface PDUs and local floor timing inside the TBS.

use super::*;
use crate::net_control::{ManagedCallKind, ManagedCallRestoreContextPayload};

#[derive(Debug, Clone)]
pub(in crate::cmce) struct ManagedLegResult {
    pub success: bool,
    pub kind: ManagedCallKind,
    pub call_id: Option<u16>,
    pub timeslot: Option<u8>,
    pub usage: Option<u8>,
    pub floor_holder: Option<u32>,
    pub queued_issi: Option<u32>,
    pub message: String,
}

impl ManagedLegResult {
    fn failure(kind: ManagedCallKind, message: impl Into<String>) -> Self {
        Self {
            success: false,
            kind,
            call_id: None,
            timeslot: None,
            usage: None,
            floor_holder: None,
            queued_issi: None,
            message: message.into(),
        }
    }
}

impl CcBsSubentity {
    pub(in crate::cmce) fn control_start_group_call(
        &mut self,
        queue: &mut MessageQueue,
        operation_id: &str,
        source_issi: u32,
        gssi: u32,
        priority: u8,
    ) -> ManagedLegResult {
        let operation_uuid = match uuid::Uuid::parse_str(operation_id) {
            Ok(value) => value,
            Err(error) => {
                return ManagedLegResult::failure(
                    ManagedCallKind::Group,
                    format!("invalid operation UUID: {error}"),
                );
            }
        };
        if source_issi == 0 || source_issi > 0x00ff_ffff {
            return ManagedLegResult::failure(
                ManagedCallKind::Group,
                "source ISSI must be in 1..=16777215",
            );
        }
        if gssi == 0 || gssi > 0x00ff_ffff {
            return ManagedLegResult::failure(
                ManagedCallKind::Group,
                "GSSI must be in 1..=16777215",
            );
        }

        // AudioPlayer is the existing trusted local network-call origin. Media
        // transport will later be switched to media-switch, while the CMCE leg
        // and its UUID already have the correct lifecycle today.
        self.fsm_on_network_call_start(
            queue,
            TetraEntity::AudioPlayer,
            operation_uuid,
            source_issi,
            gssi,
            priority.min(15),
        );

        let Some((call_id, call)) = self
            .active_calls
            .iter()
            .find(|(_, call)| call.brew_uuid == Some(operation_uuid))
            .map(|(call_id, call)| (*call_id, call.clone()))
        else {
            return ManagedLegResult::failure(
                ManagedCallKind::Group,
                "local group-call leg was not admitted (no listener, no resource or policy rejection)",
            );
        };

        ManagedLegResult {
            success: true,
            kind: ManagedCallKind::Group,
            call_id: Some(call_id),
            timeslot: Some(call.ts),
            usage: Some(call.usage),
            floor_holder: call.tx_active.then_some(call.source_issi),
            queued_issi: call.queued_tx_demand.map(|address| address.ssi),
            message: "group-call leg active".to_string(),
        }
    }

    pub(in crate::cmce) fn control_start_individual_call(
        &mut self,
        queue: &mut MessageQueue,
        operation_id: &str,
        calling_issi: u32,
        called_issi: u32,
        simplex: bool,
        priority: u8,
    ) -> ManagedLegResult {
        let operation_uuid = match uuid::Uuid::parse_str(operation_id) {
            Ok(value) => value,
            Err(error) => {
                return ManagedLegResult::failure(
                    ManagedCallKind::Individual,
                    format!("invalid operation UUID: {error}"),
                );
            }
        };
        if calling_issi == 0 || calling_issi > 0x00ff_ffff {
            return ManagedLegResult::failure(
                ManagedCallKind::Individual,
                "calling ISSI must be in 1..=16777215",
            );
        }
        if called_issi == 0 || called_issi > 0x00ff_ffff {
            return ManagedLegResult::failure(
                ManagedCallKind::Individual,
                "called ISSI must be in 1..=16777215",
            );
        }

        let call = NetworkCircuitCall {
            source_issi: calling_issi,
            destination: called_issi,
            number: calling_issi.to_string(),
            priority: priority.min(15),
            service: 0,
            mode: CircuitModeType::TchS.into_raw() as u8,
            duplex: u8::from(!simplex),
            method: 0,
            communication: CommunicationType::P2p.into_raw() as u8,
            grant: TransmissionGrant::GrantedToOtherUser.into_raw() as u8,
            permission: 1,
            timeout: CallTimeout::T5m.into_raw() as u8,
            ownership: 0,
            queued: 0,
        };

        self.fsm_on_network_circuit_setup_request(
            queue,
            TetraEntity::Asterisk,
            operation_uuid,
            call,
        );

        let Some((call_id, leg)) = self.find_brew_individual_call(operation_uuid) else {
            return ManagedLegResult::failure(
                ManagedCallKind::Individual,
                "individual-call leg was not admitted (subscriber offline, busy, no resource or policy rejection)",
            );
        };

        ManagedLegResult {
            success: true,
            kind: ManagedCallKind::Individual,
            call_id: Some(call_id),
            timeslot: Some(leg.called_ts),
            usage: Some(leg.called_usage),
            floor_holder: leg.floor_holder,
            queued_issi: leg.queued_tx_demand.map(|address| address.ssi),
            message: "individual-call setup leg created; awaiting called-party signalling".to_string(),
        }
    }

    pub(in crate::cmce) fn control_release_call(
        &mut self,
        queue: &mut MessageQueue,
        call_id: u16,
        cause: u8,
    ) -> ManagedLegResult {
        let cause = DisconnectCause::try_from(cause as u64)
            .unwrap_or(DisconnectCause::SwmiRequestedDisconnection);
        if self.active_calls.contains_key(&call_id) {
            self.release_group_call(queue, call_id, cause);
            return ManagedLegResult {
                success: true,
                kind: ManagedCallKind::Group,
                call_id: Some(call_id),
                timeslot: None,
                usage: None,
                floor_holder: None,
                queued_issi: None,
                message: "group-call leg released".to_string(),
            };
        }
        if self.individual_calls.contains_key(&call_id) {
            self.release_individual_call(queue, call_id, cause);
            return ManagedLegResult {
                success: true,
                kind: ManagedCallKind::Individual,
                call_id: Some(call_id),
                timeslot: None,
                usage: None,
                floor_holder: None,
                queued_issi: None,
                message: "individual-call leg released".to_string(),
            };
        }
        ManagedLegResult::failure(
            ManagedCallKind::Group,
            format!("unknown local call identifier {call_id}"),
        )
    }

    pub(in crate::cmce) fn control_request_floor(
        &mut self,
        queue: &mut MessageQueue,
        call_id: u16,
        source_issi: u32,
        force: bool,
    ) -> ManagedLegResult {
        let Some(before) = self.control_leg_result(call_id) else {
            return ManagedLegResult::failure(
                ManagedCallKind::Group,
                format!("unknown local call identifier {call_id}"),
            );
        };
        if source_issi == 0 || source_issi > 0x00ff_ffff {
            return ManagedLegResult::failure(before.kind, "source ISSI must be in 1..=16777215");
        }

        let requester = TetraAddress::new(source_issi, SsiType::Issi);
        let current_holder = before.floor_holder;
        if force && current_holder != Some(source_issi) {
            if let Some(call) = self.active_calls.get_mut(&call_id) {
                call.queued_tx_demand = None;
            }
            if let Some(call) = self.individual_calls.get_mut(&call_id) {
                call.queued_tx_demand = None;
            }
        }

        self.fsm_on_u_tx_demand(
            queue,
            requester,
            UTxDemand {
                call_identifier: call_id,
                tx_demand_priority: 3,
                encryption_control: false,
                reserved: false,
                facility: None,
                dm_ms_address: None,
                proprietary: None,
            },
        );

        if force
            && let Some(holder) = current_holder
            && holder != source_issi
        {
            self.fsm_on_u_tx_ceased(
                queue,
                TetraAddress::new(holder, SsiType::Issi),
                UTxCeased {
                    call_identifier: call_id,
                    facility: None,
                    dm_ms_address: None,
                    proprietary: None,
                },
            );
        }

        let Some(mut result) = self.control_leg_result(call_id) else {
            return ManagedLegResult::failure(before.kind, "call ended while changing floor");
        };
        result.success = result.floor_holder == Some(source_issi)
            || result.queued_issi == Some(source_issi);
        result.message = if result.floor_holder == Some(source_issi) {
            "floor granted".to_string()
        } else if result.queued_issi == Some(source_issi) {
            "floor request queued".to_string()
        } else {
            "floor request not granted".to_string()
        };
        result
    }

    pub(in crate::cmce) fn control_release_floor(
        &mut self,
        queue: &mut MessageQueue,
        call_id: u16,
    ) -> ManagedLegResult {
        let Some(before) = self.control_leg_result(call_id) else {
            return ManagedLegResult::failure(
                ManagedCallKind::Group,
                format!("unknown local call identifier {call_id}"),
            );
        };
        let Some(holder) = before.floor_holder else {
            let mut result = before;
            result.success = true;
            result.message = "floor already idle".to_string();
            return result;
        };

        self.fsm_on_u_tx_ceased(
            queue,
            TetraAddress::new(holder, SsiType::Issi),
            UTxCeased {
                call_identifier: call_id,
                facility: None,
                dm_ms_address: None,
                proprietary: None,
            },
        );
        let Some(mut result) = self.control_leg_result(call_id) else {
            return ManagedLegResult {
                success: true,
                kind: before.kind,
                call_id: Some(call_id),
                timeslot: None,
                usage: None,
                floor_holder: None,
                queued_issi: None,
                message: "call ended while releasing floor".to_string(),
            };
        };
        result.success = result.floor_holder != Some(holder);
        result.message = if result.floor_holder.is_some() {
            "floor handed to queued requester".to_string()
        } else {
            "floor released".to_string()
        };
        result
    }

    pub(in crate::cmce) fn control_export_restore_context(
        &self,
        call_id: u16,
    ) -> Option<ManagedCallRestoreContextPayload> {
        self.export_call_restore_context(call_id)
            .map(|context| context.to_managed_payload())
    }

    pub(in crate::cmce) fn control_import_restore_context(
        &mut self,
        payload: ManagedCallRestoreContextPayload,
    ) -> Result<u16, String> {
        let context = CallRestoreContext::from_managed_payload(payload)?;
        let call_id = context.call_id();
        self.install_call_restore_context(context);
        Ok(call_id)
    }

    pub(in crate::cmce) fn control_remove_restore_context(&mut self, call_id: u16) -> bool {
        self.remove_call_restore_context(call_id).is_some()
    }

    fn control_leg_result(&self, call_id: u16) -> Option<ManagedLegResult> {
        if let Some(call) = self.active_calls.get(&call_id) {
            return Some(ManagedLegResult {
                success: true,
                kind: ManagedCallKind::Group,
                call_id: Some(call_id),
                timeslot: Some(call.ts),
                usage: Some(call.usage),
                floor_holder: call.tx_active.then_some(call.source_issi),
                queued_issi: call.queued_tx_demand.map(|address| address.ssi),
                message: "group-call leg found".to_string(),
            });
        }
        self.individual_calls.get(&call_id).map(|call| ManagedLegResult {
            success: true,
            kind: ManagedCallKind::Individual,
            call_id: Some(call_id),
            timeslot: Some(call.called_ts),
            usage: Some(call.called_usage),
            floor_holder: call.floor_holder,
            queued_issi: call.queued_tx_demand.map(|address| address.ssi),
            message: "individual-call leg found".to_string(),
        })
    }
}

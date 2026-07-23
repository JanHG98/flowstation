//! Bidirectional base-station worker for the NetCore Control Room.
//!
//! One WebSocket carries:
//! - node hello/heartbeat
//! - telemetry events BS -> Control Room
//! - control commands Control Room -> BS
//! - accepted/rejected command acks and legacy entity responses BS -> Control Room

use std::collections::HashMap;
use std::time::{Duration, Instant};

use tetra_core::tetra_entities::TetraEntity;

use crate::{
    net_control::{CommandDispatcher, ControlCommand, ControlResponse},
    net_control_room::{
        CONTROL_ROOM_HEARTBEAT_INTERVAL, CONTROL_ROOM_PROTOCOL_VERSION, ControlCommandAck, ControlCommandEnvelope, ControlResponseEnvelope,
        ControlRoomCodecJson, ControlRoomNodeCapabilities, ControlRoomNodeHeartbeat, ControlRoomNodeHello, ControlRoomNodeIdentity,
        ControlRoomToNodeMessage, NodeTelemetryEnvelope, NodeToControlRoomMessage,
    },
    net_telemetry::{TelemetryEvent, TelemetrySource, channel::RecvEvent},
    network::transports::NetworkTransport,
};

const POLL_TIMEOUT: Duration = Duration::from_millis(250);
const RECONNECT_DELAY: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum CommandCorrelationKey {
    Handle(u32),
    KickMs(u32),
    Dgna { issi: u32, gssi: u32, attach: bool },
    Mobility(u32),
    SubscriberPolicy(u32),
    GroupPolicy(u32),
    GroupDgna(u32),
    RestartService,
    ShutdownService,
    LiveSdsAdd { source_issi: u32, protocol_id: u8, text: String },
    LiveSdsDelete(u32),
    LiveSdsClear,
    ClearEmergency(u32),
}

pub struct ControlRoomWorker<T: NetworkTransport> {
    identity: ControlRoomNodeIdentity,
    capabilities: ControlRoomNodeCapabilities,
    telemetry_source: TelemetrySource,
    dispatchers: HashMap<TetraEntity, CommandDispatcher>,
    transport: T,
    connected: bool,
    last_connect_attempt: Option<Instant>,
    last_heartbeat_at: Instant,
    started_at: String,
    seq: u64,
    pending_commands: HashMap<CommandCorrelationKey, (String, TetraEntity)>,
}

impl<T: NetworkTransport> ControlRoomWorker<T> {
    pub fn new(
        identity: ControlRoomNodeIdentity,
        capabilities: ControlRoomNodeCapabilities,
        telemetry_source: TelemetrySource,
        dispatchers: HashMap<TetraEntity, CommandDispatcher>,
        transport: T,
    ) -> Self {
        let now = Instant::now();
        Self {
            identity,
            capabilities,
            telemetry_source,
            dispatchers,
            transport,
            connected: false,
            last_connect_attempt: None,
            last_heartbeat_at: now,
            started_at: now_iso(),
            seq: 0,
            pending_commands: HashMap::new(),
        }
    }

    pub fn run(&mut self) {
        tracing::debug!("ControlRoom worker started for node_id={}", self.identity.node_id);
        self.try_connect();

        loop {
            match self.telemetry_source.recv_timeout(POLL_TIMEOUT) {
                RecvEvent::Event(event) => self.forward_telemetry(event),
                RecvEvent::Timeout => {}
                RecvEvent::Closed => {
                    tracing::debug!("ControlRoom worker: telemetry source closed, shutting down");
                    break;
                }
            }

            if self.connected {
                self.poll_downlink();
                self.collect_responses();
                self.send_periodic_heartbeat();
            } else {
                std::thread::sleep(POLL_TIMEOUT);
            }

            if !self.transport.is_connected() && self.connected {
                tracing::warn!("ControlRoom transport disconnected");
                self.transport.disconnect();
                self.connected = false;
            }

            if !self.connected && self.reconnect_due() {
                self.try_connect();
            }
        }

        self.transport.disconnect();
        tracing::info!("ControlRoom worker exiting");
    }

    fn reconnect_due(&self) -> bool {
        self.last_connect_attempt
            .map(|last| last.elapsed() >= RECONNECT_DELAY)
            .unwrap_or(true)
    }

    fn try_connect(&mut self) {
        self.last_connect_attempt = Some(Instant::now());
        self.transport.disconnect();
        match self.transport.connect() {
            Ok(()) => {
                tracing::info!("ControlRoom transport connected");
                self.connected = true;
                self.last_heartbeat_at = Instant::now() - CONTROL_ROOM_HEARTBEAT_INTERVAL;
                if self.send_hello() {
                    self.send_periodic_heartbeat();
                }
            }
            Err(e) => {
                tracing::warn!("ControlRoom transport connection failed: {}, will retry in {:?}", e, RECONNECT_DELAY);
                self.transport.disconnect();
                self.connected = false;
            }
        }
    }

    fn send_hello(&mut self) -> bool {
        let hello = ControlRoomNodeHello {
            protocol_version: CONTROL_ROOM_PROTOCOL_VERSION.to_string(),
            node: self.identity.clone(),
            capabilities: self.capabilities.clone(),
            started_at: self.started_at.clone(),
        };
        self.send_uplink(&NodeToControlRoomMessage::Hello { hello })
    }

    fn send_periodic_heartbeat(&mut self) {
        if !self.connected || self.last_heartbeat_at.elapsed() < CONTROL_ROOM_HEARTBEAT_INTERVAL {
            return;
        }
        self.seq = self.seq.wrapping_add(1);
        let heartbeat = ControlRoomNodeHeartbeat {
            node_id: self.identity.node_id.clone(),
            seq: self.seq,
            timestamp: now_iso(),
            connected: true,
        };
        if self.send_uplink(&NodeToControlRoomMessage::Heartbeat { heartbeat }) {
            self.last_heartbeat_at = Instant::now();
        }
    }

    fn forward_telemetry(&mut self, event: TelemetryEvent) {
        if !self.ensure_connected() {
            return;
        }
        self.seq = self.seq.wrapping_add(1);
        let envelope = NodeTelemetryEnvelope {
            node_id: self.identity.node_id.clone(),
            seq: self.seq,
            timestamp: now_iso(),
            event,
        };
        self.send_uplink(&NodeToControlRoomMessage::Telemetry { envelope });
    }

    fn poll_downlink(&mut self) {
        for msg in self.transport.receive_reliable() {
            let codec = ControlRoomCodecJson;
            match codec.decode_downlink(&msg.payload) {
                Ok(ControlRoomToNodeMessage::HelloAck { accepted, message }) => {
                    if accepted {
                        tracing::info!("ControlRoom hello accepted: {}", message.unwrap_or_else(|| "ok".to_string()));
                    } else {
                        tracing::warn!("ControlRoom hello rejected: {}", message.unwrap_or_else(|| "no reason".to_string()));
                        self.transport.disconnect();
                        self.connected = false;
                        break;
                    }
                }
                Ok(ControlRoomToNodeMessage::Ping { seq, .. }) => {
                    tracing::trace!("ControlRoom ping seq={}", seq);
                    self.send_periodic_heartbeat();
                }
                Ok(ControlRoomToNodeMessage::Command { envelope }) => self.handle_command(envelope),
                Err(e) => {
                    tracing::warn!("ControlRoom: failed to decode downlink message ({} bytes): {}", msg.payload.len(), e);
                    self.send_error(format!("failed to decode downlink message: {}", e));
                }
            }
        }
    }

    fn handle_command(&mut self, envelope: ControlCommandEnvelope) {
        if envelope.target_node_id != self.identity.node_id && envelope.target_node_id != "*" {
            self.send_ack(
                envelope.command_id,
                false,
                None,
                format!(
                    "command target_node_id={} does not match this node_id={}",
                    envelope.target_node_id, self.identity.node_id
                ),
            );
            return;
        }

        let target = route_control_command(&envelope.command);
        let Some(dispatcher) = self.dispatchers.get(&target) else {
            self.send_ack(envelope.command_id, false, Some(target), format!("no dispatcher registered for {:?}", target));
            return;
        };

        if let Some(key) = correlation_key_for_command(&envelope.command) {
            self.pending_commands.insert(key, (envelope.command_id.clone(), target));
        }

        dispatcher.send(envelope.command);
        self.send_ack(envelope.command_id, true, Some(target), format!("dispatched to {:?}", target));
    }

    fn collect_responses(&mut self) {
        let mut outgoing: Vec<(ControlResponse, Option<String>, Option<TetraEntity>)> = Vec::new();

        for (entity, dispatcher) in &self.dispatchers {
            for response in dispatcher.try_recv_responses() {
                let key = correlation_key_for_response(&response);
                let correlated = key.and_then(|k| self.pending_commands.remove(&k));
                let (command_id, target_entity) = match correlated {
                    Some((id, entity)) => (Some(id), Some(entity)),
                    None => (None, Some(*entity)),
                };
                outgoing.push((response, command_id, target_entity));
            }
        }

        for (response, command_id, target_entity) in outgoing {
            let envelope = ControlResponseEnvelope {
                command_id,
                node_id: self.identity.node_id.clone(),
                target_entity,
                timestamp: now_iso(),
                response,
            };
            self.send_uplink(&NodeToControlRoomMessage::ControlResponse { envelope });
        }
    }

    fn send_ack(&mut self, command_id: String, accepted: bool, target_entity: Option<TetraEntity>, message: String) {
        let ack = ControlCommandAck {
            command_id,
            node_id: self.identity.node_id.clone(),
            accepted,
            target_entity,
            message,
            timestamp: now_iso(),
        };
        self.send_uplink(&NodeToControlRoomMessage::ControlAck { ack });
    }

    fn send_error(&mut self, message: String) {
        let msg = NodeToControlRoomMessage::Error {
            node_id: self.identity.node_id.clone(),
            message,
            timestamp: now_iso(),
        };
        self.send_uplink(&msg);
    }

    fn ensure_connected(&mut self) -> bool {
        if self.connected && self.transport.is_connected() {
            return true;
        }

        if self.connected {
            tracing::warn!("ControlRoom transport no longer connected");
            self.transport.disconnect();
            self.connected = false;
        }

        if !self.reconnect_due() {
            return false;
        }

        self.try_connect();
        self.connected
    }

    fn send_uplink(&mut self, message: &NodeToControlRoomMessage) -> bool {
        if !self.connected {
            return false;
        }
        let codec = ControlRoomCodecJson;
        let payload = codec.encode_uplink(message);
        match self.transport.send_reliable(&payload) {
            Ok(()) => true,
            Err(e) => {
                tracing::warn!("ControlRoom transport send failed: {}, will retry in {:?}", e, RECONNECT_DELAY);
                self.transport.disconnect();
                self.connected = false;
                false
            }
        }
    }
}

pub fn route_control_command(command: &ControlCommand) -> TetraEntity {
    match command {
        ControlCommand::SendSds { .. } => TetraEntity::Cmce,
        ControlCommand::SendRawSdsType4 { .. } => TetraEntity::Cmce,
        ControlCommand::KickMs { .. } => TetraEntity::Cmce,
        ControlCommand::Dgna { .. }
        | ControlCommand::MobilityExportContext { .. }
        | ControlCommand::MobilityImportContext { .. }
        | ControlCommand::MobilityRemoveContext { .. }
        | ControlCommand::SubscriberAccessPolicyApply { .. }
        | ControlCommand::GroupAccessPolicyApply { .. }
        | ControlCommand::GroupDgnaApply { .. } => TetraEntity::Mm,
        ControlCommand::RestartService => TetraEntity::Cmce,
        ControlCommand::ShutdownService => TetraEntity::Cmce,
        ControlCommand::AddLiveSds { .. } => TetraEntity::Cmce,
        ControlCommand::DeleteLiveSds { .. } => TetraEntity::Cmce,
        ControlCommand::ClearLiveSds => TetraEntity::Cmce,
        ControlCommand::ClearEmergency { .. } => TetraEntity::Cmce,
        ControlCommand::CommandA { .. } => TetraEntity::Mm,
        ControlCommand::TestCmdB { .. } => TetraEntity::Cmce,
    }
}

fn correlation_key_for_command(command: &ControlCommand) -> Option<CommandCorrelationKey> {
    match command {
        ControlCommand::SendSds { handle, .. }
        | ControlCommand::SendRawSdsType4 { handle, .. }
        | ControlCommand::CommandA { handle, .. }
        | ControlCommand::TestCmdB { handle, .. } => Some(CommandCorrelationKey::Handle(*handle)),
        ControlCommand::KickMs { issi } => Some(CommandCorrelationKey::KickMs(*issi)),
        ControlCommand::MobilityExportContext { handle, .. }
        | ControlCommand::MobilityImportContext { handle, .. }
        | ControlCommand::MobilityRemoveContext { handle, .. } => {
            Some(CommandCorrelationKey::Mobility(*handle))
        }
        ControlCommand::SubscriberAccessPolicyApply { handle, .. } => {
            Some(CommandCorrelationKey::SubscriberPolicy(*handle))
        }
        ControlCommand::GroupAccessPolicyApply { handle, .. } => {
            Some(CommandCorrelationKey::GroupPolicy(*handle))
        }
        ControlCommand::GroupDgnaApply { handle, .. } => {
            Some(CommandCorrelationKey::GroupDgna(*handle))
        }
        ControlCommand::Dgna { issi, gssi, attach } => Some(CommandCorrelationKey::Dgna {
            issi: *issi,
            gssi: *gssi,
            attach: *attach,
        }),
        ControlCommand::RestartService => Some(CommandCorrelationKey::RestartService),
        ControlCommand::ShutdownService => Some(CommandCorrelationKey::ShutdownService),
        ControlCommand::AddLiveSds {
            text,
            protocol_id,
            source_issi,
            ..
        } => Some(CommandCorrelationKey::LiveSdsAdd {
            source_issi: *source_issi,
            protocol_id: *protocol_id,
            text: text.clone(),
        }),
        ControlCommand::DeleteLiveSds { id } => Some(CommandCorrelationKey::LiveSdsDelete(*id)),
        ControlCommand::ClearLiveSds => Some(CommandCorrelationKey::LiveSdsClear),
        ControlCommand::ClearEmergency { issi } => Some(CommandCorrelationKey::ClearEmergency(*issi)),
    }
}

fn correlation_key_for_response(response: &ControlResponse) -> Option<CommandCorrelationKey> {
    match response {
        ControlResponse::CommandAResponse { handle, .. } | ControlResponse::SendSdsResponse { handle, .. } => {
            Some(CommandCorrelationKey::Handle(*handle))
        }
        ControlResponse::KickMsResponse { issi, .. } => Some(CommandCorrelationKey::KickMs(*issi)),
        ControlResponse::MobilityContextExported { handle, .. }
        | ControlResponse::MobilityContextImported { handle, .. }
        | ControlResponse::MobilityContextRemoved { handle, .. } => {
            Some(CommandCorrelationKey::Mobility(*handle))
        }
        ControlResponse::SubscriberAccessPolicyApplied { handle, .. } => {
            Some(CommandCorrelationKey::SubscriberPolicy(*handle))
        }
        ControlResponse::GroupAccessPolicyApplied { handle, .. } => {
            Some(CommandCorrelationKey::GroupPolicy(*handle))
        }
        ControlResponse::GroupDgnaApplied { handle, .. } => {
            Some(CommandCorrelationKey::GroupDgna(*handle))
        }
    }
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_dgna_to_mm() {
        assert_eq!(
            route_control_command(&ControlCommand::Dgna {
                issi: 1,
                gssi: 2,
                attach: true,
            }),
            TetraEntity::Mm
        );
    }

    #[test]
    fn routes_central_group_commands_to_mm() {
        assert_eq!(
            route_control_command(&ControlCommand::GroupAccessPolicyApply {
                handle: 7,
                revision: 1,
                allow_unlisted_groups: false,
                enforce_memberships: true,
                reconcile_registered: true,
                groups: Vec::new(),
                memberships: Vec::new(),
            }),
            TetraEntity::Mm
        );
        assert_eq!(
            route_control_command(&ControlCommand::GroupDgnaApply {
                handle: 8,
                issi: 1001,
                gssi: 15501,
                attach: true,
                force: false,
            }),
            TetraEntity::Mm
        );
    }

    #[test]
    fn correlates_group_policy_and_dgna_responses() {
        let policy_command = ControlCommand::GroupAccessPolicyApply {
            handle: 77,
            revision: 2,
            allow_unlisted_groups: false,
            enforce_memberships: true,
            reconcile_registered: true,
            groups: Vec::new(),
            memberships: Vec::new(),
        };
        let policy_response = ControlResponse::GroupAccessPolicyApplied {
            handle: 77,
            revision: 2,
            success: true,
            group_count: 0,
            membership_count: 0,
            attached_count: 0,
            detached_count: 0,
            message: String::new(),
        };
        assert_eq!(
            correlation_key_for_command(&policy_command),
            correlation_key_for_response(&policy_response)
        );

        let dgna_command = ControlCommand::GroupDgnaApply {
            handle: 78,
            issi: 1001,
            gssi: 15501,
            attach: true,
            force: false,
        };
        let dgna_response = ControlResponse::GroupDgnaApplied {
            handle: 78,
            issi: 1001,
            gssi: 15501,
            attach: true,
            success: true,
            message: String::new(),
        };
        assert_eq!(
            correlation_key_for_command(&dgna_command),
            correlation_key_for_response(&dgna_response)
        );
    }

    #[test]
    fn correlates_handle_based_sds() {
        let cmd = ControlCommand::SendSds {
            handle: 42,
            source_ssi: 9999,
            dest_ssi: 123,
            dest_is_group: false,
            len_bits: 8,
            payload: vec![1],
        };
        let resp = ControlResponse::SendSdsResponse { handle: 42, success: true };
        assert_eq!(correlation_key_for_command(&cmd), correlation_key_for_response(&resp));
    }
}

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, mpsc};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tetra_entities::net_control::ControlCommand;
use tetra_entities::net_media::MediaDownlinkFrame;
use tetra_entities::net_control_room::{
    CONTROL_ROOM_PROTOCOL_VERSION, ControlCommandEnvelope, ControlRoomNodeCapabilities,
    ControlRoomNodeIdentity, ControlRoomNodeHello,
    ControlRoomToNodeMessage, NodeToControlRoomMessage,
};

use crate::config::NodeGatewayConfig;

#[derive(Debug, Clone)]
pub enum NodeOutbound {
    Protocol(ControlRoomToNodeMessage),
    Close,
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeSnapshot {
    pub node_id: String,
    pub session_id: String,
    pub peer: String,
    pub connected: bool,
    pub stale: bool,
    pub connected_at: String,
    pub last_seen: String,
    pub disconnected_at: Option<String>,
    pub disconnect_reason: Option<String>,
    pub heartbeat_seq: u64,
    pub message_count: u64,
    pub telemetry_count: u64,
    pub control_ack_count: u64,
    pub control_response_count: u64,
    pub media_frame_count: u64,
    pub error_count: u64,
    pub last_message_kind: String,
    pub last_telemetry: Option<Value>,
    pub identity: ControlRoomNodeIdentity,
    pub capabilities: ControlRoomNodeCapabilities,
}

#[derive(Debug, Clone, Serialize)]
pub struct GatewayStatus {
    pub service: &'static str,
    pub started_at: String,
    pub security_mode: &'static str,
    pub warning: &'static str,
    pub remote_management_enabled: bool,
    pub node_path: String,
    pub backend_path: String,
    pub known_nodes: usize,
    pub connected_nodes: usize,
    pub stale_nodes: usize,
    pub backend_clients: usize,
    pub total_node_sessions: u64,
    pub total_node_messages: u64,
    pub total_commands: u64,
    pub total_media_frames: u64,
    pub total_disconnects: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct GatewaySnapshot {
    pub status: GatewayStatus,
    pub nodes: Vec<NodeSnapshot>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventRecord {
    pub seq: u64,
    pub timestamp: String,
    pub kind: String,
    pub node_id: Option<String>,
    pub detail: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BackendEvent {
    Snapshot { snapshot: GatewaySnapshot },
    Event { event: EventRecord },
    NodeMessage { node_id: String, message: NodeToControlRoomMessage },
    ActionResult {
        request_id: Option<String>,
        command_id: Option<String>,
        ok: bool,
        message: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BackendRequest {
    Ping {
        #[serde(default)]
        request_id: Option<String>,
    },
    Command {
        #[serde(default)]
        request_id: Option<String>,
        node_id: String,
        command: ControlCommand,
        operator_id: Option<String>,
    },
    DisconnectNode {
        #[serde(default)]
        request_id: Option<String>,
        node_id: String,
    },
    PingNode {
        #[serde(default)]
        request_id: Option<String>,
        node_id: String,
    },
    /// Select high-rate backend topics. Media frames are opt-in so normal
    /// management services are not flooded with speech traffic.
    Subscribe {
        #[serde(default)]
        request_id: Option<String>,
        #[serde(default)]
        topics: Vec<String>,
    },
    /// High-rate packed media frame. Success is intentionally not acknowledged
    /// per frame; errors are returned as ActionResult events.
    MediaFrame {
        node_id: String,
        frame: MediaDownlinkFrame,
    },
}

struct NodeRuntime {
    snapshot: NodeSnapshot,
    sender: Option<mpsc::Sender<NodeOutbound>>,
}

struct BackendRuntime {
    sender: mpsc::Sender<BackendEvent>,
    media_frames: bool,
}

struct GatewayState {
    config: NodeGatewayConfig,
    started_at: String,
    nodes: HashMap<String, NodeRuntime>,
    backend_clients: HashMap<String, BackendRuntime>,
    events: VecDeque<EventRecord>,
    next_event_seq: u64,
    total_node_sessions: u64,
    total_node_messages: u64,
    total_commands: u64,
    total_media_frames: u64,
    total_disconnects: u64,
}

#[derive(Clone)]
pub struct SharedGateway(Arc<Mutex<GatewayState>>);

impl SharedGateway {
    pub fn new(config: NodeGatewayConfig) -> Self {
        Self(Arc::new(Mutex::new(GatewayState {
            config,
            started_at: now_iso(),
            nodes: HashMap::new(),
            backend_clients: HashMap::new(),
            events: VecDeque::new(),
            next_event_seq: 1,
            total_node_sessions: 0,
            total_node_messages: 0,
            total_commands: 0,
            total_media_frames: 0,
            total_disconnects: 0,
        })))
    }

    pub fn status(&self) -> GatewayStatus {
        let state = self.0.lock().expect("gateway state poisoned");
        status_locked(&state)
    }

    pub fn nodes(&self) -> Vec<NodeSnapshot> {
        self.snapshot().nodes
    }

    pub fn node(&self, node_id: &str) -> Option<NodeSnapshot> {
        let state = self.0.lock().expect("gateway state poisoned");
        state.nodes.get(node_id).map(|node| snapshot_node(node, state.config.server.stale_after_secs))
    }

    pub fn recent_events(&self, limit: usize) -> Vec<EventRecord> {
        let state = self.0.lock().expect("gateway state poisoned");
        state.events.iter().rev().take(limit.min(state.events.len())).cloned().collect()
    }

    pub fn register_node(
        &self,
        hello: &ControlRoomNodeHello,
        session_id: String,
        peer: String,
        sender: mpsc::Sender<NodeOutbound>,
    ) -> Result<(), String> {
        if hello.protocol_version != CONTROL_ROOM_PROTOCOL_VERSION {
            return Err(format!(
                "unsupported protocol_version={}; expected {}",
                hello.protocol_version, CONTROL_ROOM_PROTOCOL_VERSION
            ));
        }
        let node_id = hello.node.node_id.trim();
        if node_id.is_empty() {
            return Err("node_id must not be empty".to_string());
        }

        let mut state = self.0.lock().expect("gateway state poisoned");
        if let Some(previous) = state.nodes.get_mut(node_id) {
            if let Some(old_sender) = previous.sender.take() {
                let _ = old_sender.send(NodeOutbound::Close);
            }
        }

        let now = now_iso();
        let snapshot = NodeSnapshot {
            node_id: node_id.to_string(),
            session_id: session_id.clone(),
            peer,
            connected: true,
            stale: false,
            connected_at: now.clone(),
            last_seen: now,
            disconnected_at: None,
            disconnect_reason: None,
            heartbeat_seq: 0,
            message_count: 1,
            telemetry_count: 0,
            control_ack_count: 0,
            control_response_count: 0,
            media_frame_count: 0,
            error_count: 0,
            last_message_kind: "hello".to_string(),
            last_telemetry: None,
            identity: hello.node.clone(),
            capabilities: hello.capabilities.clone(),
        };
        state.nodes.insert(node_id.to_string(), NodeRuntime { snapshot, sender: Some(sender) });
        state.total_node_sessions = state.total_node_sessions.wrapping_add(1);
        state.total_node_messages = state.total_node_messages.wrapping_add(1);
        push_event_locked(
            &mut state,
            "node_connected",
            Some(node_id.to_string()),
            json!({ "session_id": session_id, "protocol_version": hello.protocol_version }),
        );
        let gateway_snapshot = snapshot_locked(&state);
        broadcast_locked(
            &mut state,
            BackendEvent::Snapshot {
                snapshot: gateway_snapshot,
            },
        );
        Ok(())
    }

    pub fn handle_node_message(&self, node_id: &str, session_id: &str, message: NodeToControlRoomMessage) {
        let mut state = self.0.lock().expect("gateway state poisoned");
        let (kind, is_media) = {
            let Some(runtime) = state.nodes.get_mut(node_id) else {
                return;
            };
            if runtime.snapshot.session_id != session_id {
                return;
            }

            runtime.snapshot.connected = true;
            runtime.snapshot.stale = false;
            runtime.snapshot.last_seen = now_iso();
            runtime.snapshot.message_count = runtime.snapshot.message_count.wrapping_add(1);

            let kind = match &message {
                NodeToControlRoomMessage::Hello { .. } => "hello",
                NodeToControlRoomMessage::Heartbeat { heartbeat } => {
                    runtime.snapshot.heartbeat_seq = heartbeat.seq;
                    "heartbeat"
                }
                NodeToControlRoomMessage::Telemetry { envelope } => {
                    runtime.snapshot.telemetry_count = runtime.snapshot.telemetry_count.wrapping_add(1);
                    runtime.snapshot.last_telemetry = serde_json::to_value(&envelope.event).ok();
                    "telemetry"
                }
                NodeToControlRoomMessage::ControlAck { .. } => {
                    runtime.snapshot.control_ack_count = runtime.snapshot.control_ack_count.wrapping_add(1);
                    "control_ack"
                }
                NodeToControlRoomMessage::ControlResponse { .. } => {
                    runtime.snapshot.control_response_count = runtime.snapshot.control_response_count.wrapping_add(1);
                    "control_response"
                }
                NodeToControlRoomMessage::MediaFrame { .. } => {
                    runtime.snapshot.media_frame_count = runtime.snapshot.media_frame_count.wrapping_add(1);
                    "media_frame"
                }
                NodeToControlRoomMessage::Error { .. } => {
                    runtime.snapshot.error_count = runtime.snapshot.error_count.wrapping_add(1);
                    "error"
                }
            };
            runtime.snapshot.last_message_kind = kind.to_string();
            (kind, matches!(&message, NodeToControlRoomMessage::MediaFrame { .. }))
        };
        state.total_node_messages = state.total_node_messages.wrapping_add(1);
        if is_media {
            state.total_media_frames = state.total_media_frames.wrapping_add(1);
        } else {
            push_event_locked(
                &mut state,
                "node_message",
                Some(node_id.to_string()),
                json!({ "message_kind": kind }),
            );
        }
        // Media frames are broadcast directly but deliberately omitted from the
        // persistent event history to avoid 18 events/s per active speech leg.
        broadcast_locked(
            &mut state,
            BackendEvent::NodeMessage {
                node_id: node_id.to_string(),
                message,
            },
        );
    }

    pub fn mark_disconnected(&self, node_id: &str, session_id: &str, reason: &str) {
        let mut state = self.0.lock().expect("gateway state poisoned");
        {
            let Some(runtime) = state.nodes.get_mut(node_id) else {
                return;
            };
            if runtime.snapshot.session_id != session_id {
                return;
            }
            runtime.snapshot.connected = false;
            runtime.snapshot.stale = false;
            runtime.snapshot.disconnected_at = Some(now_iso());
            runtime.snapshot.disconnect_reason = Some(reason.to_string());
            runtime.sender = None;
        }
        state.total_disconnects = state.total_disconnects.wrapping_add(1);
        push_event_locked(
            &mut state,
            "node_disconnected",
            Some(node_id.to_string()),
            json!({ "session_id": session_id, "reason": reason }),
        );
        let gateway_snapshot = snapshot_locked(&state);
        broadcast_locked(
            &mut state,
            BackendEvent::Snapshot {
                snapshot: gateway_snapshot,
            },
        );
    }

    pub fn ping_node(&self, node_id: &str) -> Result<(), String> {
        let mut state = self.0.lock().expect("gateway state poisoned");
        if !state.config.security.allow_remote_management {
            return Err("remote management is disabled by configuration".to_string());
        }
        let sender = state
            .nodes
            .get(node_id)
            .and_then(|node| node.sender.clone())
            .ok_or_else(|| format!("node {node_id} is not connected"))?;
        let ping = ControlRoomToNodeMessage::Ping {
            seq: chrono::Utc::now().timestamp_millis() as u64,
            timestamp: now_iso(),
        };
        sender.send(NodeOutbound::Protocol(ping)).map_err(|_| format!("node {node_id} send queue is closed"))?;
        push_event_locked(&mut state, "node_ping_requested", Some(node_id.to_string()), json!({}));
        Ok(())
    }

    pub fn disconnect_node(&self, node_id: &str) -> Result<(), String> {
        let mut state = self.0.lock().expect("gateway state poisoned");
        if !state.config.security.allow_remote_management {
            return Err("remote management is disabled by configuration".to_string());
        }
        let sender = state
            .nodes
            .get(node_id)
            .and_then(|node| node.sender.clone())
            .ok_or_else(|| format!("node {node_id} is not connected"))?;
        sender.send(NodeOutbound::Close).map_err(|_| format!("node {node_id} send queue is closed"))?;
        push_event_locked(
            &mut state,
            "node_disconnect_requested",
            Some(node_id.to_string()),
            json!({ "security_mode": "open_lab" }),
        );
        Ok(())
    }

    pub fn send_media_frame(
        &self,
        node_id: &str,
        frame: MediaDownlinkFrame,
    ) -> Result<(), String> {
        let mut state = self.0.lock().expect("gateway state poisoned");
        let sender = state
            .nodes
            .get(node_id)
            .and_then(|node| node.sender.clone())
            .ok_or_else(|| format!("node {node_id} is not connected"))?;
        if !state
            .nodes
            .get(node_id)
            .is_some_and(|node| node.snapshot.capabilities.media_bridge)
        {
            return Err(format!("node {node_id} does not advertise media_bridge"));
        }
        sender
            .send(NodeOutbound::Protocol(ControlRoomToNodeMessage::MediaFrame { frame }))
            .map_err(|_| format!("node {node_id} send queue is closed"))?;
        state.total_media_frames = state.total_media_frames.wrapping_add(1);
        Ok(())
    }

    pub fn send_command(&self, node_id: &str, command: ControlCommand, operator_id: Option<String>) -> Result<String, String> {
        let mut state = self.0.lock().expect("gateway state poisoned");
        if !state.config.security.allow_remote_management {
            return Err("remote management is disabled by configuration".to_string());
        }
        let sender = state
            .nodes
            .get(node_id)
            .and_then(|node| node.sender.clone())
            .ok_or_else(|| format!("node {node_id} is not connected"))?;
        let command_id = uuid::Uuid::new_v4().to_string();
        let envelope = ControlCommandEnvelope {
            command_id: command_id.clone(),
            target_node_id: node_id.to_string(),
            operator_id: Some(operator_id.unwrap_or_else(|| "open-lab".to_string())),
            issued_at: now_iso(),
            command,
        };
        sender
            .send(NodeOutbound::Protocol(ControlRoomToNodeMessage::Command { envelope }))
            .map_err(|_| format!("node {node_id} send queue is closed"))?;
        state.total_commands = state.total_commands.wrapping_add(1);
        push_event_locked(
            &mut state,
            "command_queued",
            Some(node_id.to_string()),
            json!({ "command_id": command_id, "security_mode": "open_lab" }),
        );
        Ok(command_id)
    }

    pub fn register_backend(&self) -> (String, mpsc::Receiver<BackendEvent>) {
        let (tx, rx) = mpsc::channel();
        let id = uuid::Uuid::new_v4().to_string();
        let mut state = self.0.lock().expect("gateway state poisoned");
        let _ = tx.send(BackendEvent::Snapshot { snapshot: snapshot_locked(&state) });
        state.backend_clients.insert(id.clone(), BackendRuntime { sender: tx, media_frames: false });
        push_event_locked(&mut state, "backend_connected", None, json!({ "backend_id": id.clone() }));
        (id, rx)
    }

    pub fn set_backend_topics(
        &self,
        backend_id: &str,
        topics: &[String],
    ) -> Result<Vec<String>, String> {
        let mut state = self.0.lock().expect("gateway state poisoned");
        let backend = state
            .backend_clients
            .get_mut(backend_id)
            .ok_or_else(|| "backend session is no longer registered".to_string())?;
        backend.media_frames = topics.iter().any(|topic| topic == "media_frames");
        let accepted = if backend.media_frames {
            vec!["media_frames".to_string()]
        } else {
            Vec::new()
        };
        push_event_locked(
            &mut state,
            "backend_subscription_changed",
            None,
            json!({"backend_id": backend_id, "topics": &accepted}),
        );
        Ok(accepted)
    }

    pub fn unregister_backend(&self, backend_id: &str) {
        let mut state = self.0.lock().expect("gateway state poisoned");
        state.backend_clients.remove(backend_id);
        push_event_locked(&mut state, "backend_disconnected", None, json!({ "backend_id": backend_id }));
    }

    pub fn metrics(&self) -> String {
        let status = self.status();
        format!(
            concat!(
                "# HELP netcore_node_gateway_up Service liveness.\n",
                "# TYPE netcore_node_gateway_up gauge\n",
                "netcore_node_gateway_up 1\n",
                "# TYPE netcore_node_gateway_known_nodes gauge\n",
                "netcore_node_gateway_known_nodes {}\n",
                "# TYPE netcore_node_gateway_connected_nodes gauge\n",
                "netcore_node_gateway_connected_nodes {}\n",
                "# TYPE netcore_node_gateway_stale_nodes gauge\n",
                "netcore_node_gateway_stale_nodes {}\n",
                "# TYPE netcore_node_gateway_backend_clients gauge\n",
                "netcore_node_gateway_backend_clients {}\n",
                "# TYPE netcore_node_gateway_node_messages_total counter\n",
                "netcore_node_gateway_node_messages_total {}\n",
                "# TYPE netcore_node_gateway_commands_total counter\n",
                "netcore_node_gateway_commands_total {}\n",
                "# TYPE netcore_node_gateway_media_frames_total counter\n",
                "netcore_node_gateway_media_frames_total {}\n"
            ),
            status.known_nodes,
            status.connected_nodes,
            status.stale_nodes,
            status.backend_clients,
            status.total_node_messages,
            status.total_commands,
            status.total_media_frames,
        )
    }
}

fn snapshot_locked(state: &GatewayState) -> GatewaySnapshot {
    let mut nodes: Vec<_> = state
        .nodes
        .values()
        .map(|node| snapshot_node(node, state.config.server.stale_after_secs))
        .collect();
    nodes.sort_by(|a, b| a.node_id.cmp(&b.node_id));
    GatewaySnapshot { status: status_locked(state), nodes }
}

fn status_locked(state: &GatewayState) -> GatewayStatus {
    let nodes: Vec<_> = state
        .nodes
        .values()
        .map(|node| snapshot_node(node, state.config.server.stale_after_secs))
        .collect();
    GatewayStatus {
        service: "netcore-node-gateway",
        started_at: state.started_at.clone(),
        security_mode: "open_lab",
        warning: "NO AUTHENTICATION, NO TOKENS, NO TLS - ISOLATED TEST NETWORK ONLY",
        remote_management_enabled: state.config.security.allow_remote_management,
        node_path: state.config.server.node_path.clone(),
        backend_path: state.config.server.backend_path.clone(),
        known_nodes: nodes.len(),
        connected_nodes: nodes.iter().filter(|node| node.connected).count(),
        stale_nodes: nodes.iter().filter(|node| node.stale).count(),
        backend_clients: state.backend_clients.len(),
        total_node_sessions: state.total_node_sessions,
        total_node_messages: state.total_node_messages,
        total_commands: state.total_commands,
        total_media_frames: state.total_media_frames,
        total_disconnects: state.total_disconnects,
    }
}

fn snapshot_node(node: &NodeRuntime, stale_after_secs: u64) -> NodeSnapshot {
    let mut snapshot = node.snapshot.clone();
    snapshot.stale = snapshot.connected && seconds_since(&snapshot.last_seen).is_some_and(|age| age > stale_after_secs);
    snapshot
}

fn push_event_locked(state: &mut GatewayState, kind: &str, node_id: Option<String>, detail: Value) {
    let event = EventRecord {
        seq: state.next_event_seq,
        timestamp: now_iso(),
        kind: kind.to_string(),
        node_id,
        detail,
    };
    state.next_event_seq = state.next_event_seq.wrapping_add(1);
    state.events.push_back(event.clone());
    while state.events.len() > state.config.server.history_limit {
        state.events.pop_front();
    }
    broadcast_locked(state, BackendEvent::Event { event });
}

fn broadcast_locked(state: &mut GatewayState, event: BackendEvent) {
    let is_media = matches!(
        &event,
        BackendEvent::NodeMessage {
            message: NodeToControlRoomMessage::MediaFrame { .. },
            ..
        }
    );
    state.backend_clients.retain(|_, backend| {
        if is_media && !backend.media_frames {
            true
        } else {
            backend.sender.send(event.clone()).is_ok()
        }
    });
}

fn seconds_since(timestamp: &str) -> Option<u64> {
    let parsed = chrono::DateTime::parse_from_rfc3339(timestamp).ok()?;
    let elapsed = chrono::Utc::now().signed_duration_since(parsed.with_timezone(&chrono::Utc));
    Some(elapsed.num_seconds().max(0) as u64)
}

pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hello(node_id: &str) -> ControlRoomNodeHello {
        ControlRoomNodeHello {
            protocol_version: CONTROL_ROOM_PROTOCOL_VERSION.to_string(),
            node: ControlRoomNodeIdentity {
                node_id: node_id.to_string(),
                station_name: "Test".to_string(),
                site: None,
                stack_version: "test".to_string(),
                mcc: 262,
                mnc: 42,
                location_area: 1,
                main_carrier: 720,
                secondary_carrier: Some(721),
                colour_code: 1,
                system_code: 1,
            },
            capabilities: ControlRoomNodeCapabilities {
                telemetry: true,
                command: true,
                sds: true,
                raw_sds: true,
                dgna: true,
                kick_ms: true,
                emergency_clear: true,
                live_sds: true,
                service_control: true,
                brew_bridge: false,
                dual_carrier: true,
                packet_data: true,
                legacy_wap_sds: true,
                multi_pdch: true,
                subscriber_policy: true,
                group_policy: true,
                call_control: true,
                call_restore_context: true,
                media_bridge: true,
            },
            started_at: now_iso(),
        }
    }

    #[test]
    fn registers_and_disconnects_node_without_auth() {
        let gateway = SharedGateway::new(NodeGatewayConfig::default());
        let (tx, _rx) = mpsc::channel();
        gateway.register_node(&hello("tbs-a"), "s1".to_string(), "127.0.0.1".to_string(), tx).unwrap();
        assert_eq!(gateway.status().connected_nodes, 1);
        gateway.mark_disconnected("tbs-a", "s1", "test");
        assert_eq!(gateway.status().connected_nodes, 0);
    }

    #[test]
    fn media_frames_are_delivered_only_to_subscribed_backends() {
        let gateway = SharedGateway::new(NodeGatewayConfig::default());
        let (node_tx, _node_rx) = mpsc::channel();
        gateway
            .register_node(
                &hello("tbs-a"),
                "s1".to_string(),
                "127.0.0.1".to_string(),
                node_tx,
            )
            .unwrap();
        let (plain_id, plain_rx) = gateway.register_backend();
        let (media_id, media_rx) = gateway.register_backend();
        let _ = plain_rx.try_recv();
        let _ = media_rx.try_recv();
        gateway
            .set_backend_topics(&media_id, &["media_frames".to_string()])
            .unwrap();
        while plain_rx.try_recv().is_ok() {}
        while media_rx.try_recv().is_ok() {}

        gateway.handle_node_message(
            "tbs-a",
            "s1",
            NodeToControlRoomMessage::MediaFrame {
                frame: tetra_entities::net_media::MediaUplinkFrame {
                    node_id: "tbs-a".to_string(),
                    sequence: 1,
                    timestamp: now_iso(),
                    carrier_num: 720,
                    logical_ts: 2,
                    codec: tetra_entities::net_media::MediaCodec::TetraAcelp0,
                    payload: vec![0; tetra_entities::net_media::TETRA_ACELP_FRAME_BYTES],
                },
            },
        );
        assert!(plain_rx.try_recv().is_err());
        assert!(matches!(
            media_rx.try_recv().unwrap(),
            BackendEvent::NodeMessage { .. }
        ));
        gateway.unregister_backend(&plain_id);
    }

    #[test]
    fn duplicate_node_replaces_old_session() {
        let gateway = SharedGateway::new(NodeGatewayConfig::default());
        let (tx1, rx1) = mpsc::channel();
        gateway.register_node(&hello("tbs-a"), "s1".to_string(), "one".to_string(), tx1).unwrap();
        let (tx2, _rx2) = mpsc::channel();
        gateway.register_node(&hello("tbs-a"), "s2".to_string(), "two".to_string(), tx2).unwrap();
        assert!(matches!(rx1.recv().unwrap(), NodeOutbound::Close));
        assert_eq!(gateway.node("tbs-a").unwrap().session_id, "s2");
    }
}

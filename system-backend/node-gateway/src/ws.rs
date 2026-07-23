use std::net::TcpStream;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use tetra_entities::net_control_room::{
    CONTROL_ROOM_PROTOCOL_VERSION, ControlRoomCodecJson, ControlRoomToNodeMessage,
    NodeToControlRoomMessage,
};
use tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tungstenite::http::StatusCode;
use tungstenite::{Message, WebSocket, accept_hdr};

use crate::config::NodeGatewayConfig;
use crate::state::{BackendEvent, BackendRequest, NodeOutbound, SharedGateway, now_iso};

const BACKEND_PROTOCOL_VERSION: &str = "netcore-node-gateway-backend-v1";
const WS_READ_TIMEOUT: Duration = Duration::from_millis(100);

fn reject_websocket(status: StatusCode, message: &str) -> ErrorResponse {
    tungstenite::http::Response::builder()
        .status(status)
        .header("Content-Type", "text/plain; charset=utf-8")
        .header("Content-Length", message.len().to_string())
        .header("Connection", "close")
        .body(Some(message.to_string()))
        .expect("valid websocket rejection response")
}

pub fn handle_websocket_stream(stream: TcpStream, gateway: SharedGateway, config: NodeGatewayConfig) {
    let peer = stream.peer_addr().ok();
    let selected_path = Arc::new(Mutex::new(String::new()));
    let selected_path_cb = selected_path.clone();
    let node_path = config.server.node_path.clone();
    let backend_path = config.server.backend_path.clone();
    let node_path_cb = node_path.clone();
    let backend_path_cb = backend_path.clone();

    let callback = move |req: &Request, mut response: Response| {
        let path = req.uri().path().to_string();
        *selected_path_cb.lock().expect("ws path lock poisoned") = path.clone();
        if path != node_path_cb && path != backend_path_cb {
            return Err(reject_websocket(StatusCode::NOT_FOUND, "unknown websocket endpoint"));
        }

        let expected = if path == node_path_cb {
            CONTROL_ROOM_PROTOCOL_VERSION
        } else {
            BACKEND_PROTOCOL_VERSION
        };
        if let Some(requested) = req.headers().get("sec-websocket-protocol").and_then(|value| value.to_str().ok()) {
            if requested.split(',').map(str::trim).any(|protocol| protocol == expected) {
                response.headers_mut().insert(
                    "Sec-WebSocket-Protocol",
                    expected.parse().expect("valid websocket protocol header"),
                );
            }
        }

        // Compatibility marker required by the existing TBS WebSocket transport.
        response.headers_mut().insert(
            "x-netcore-control-room",
            "1".parse().expect("valid compatibility marker"),
        );
        response.headers_mut().insert(
            "x-netcore-node-gateway",
            "1".parse().expect("valid gateway marker"),
        );
        response.headers_mut().insert(
            "x-netcore-security-mode",
            "open-lab".parse().expect("valid security mode marker"),
        );
        Ok(response)
    };

    let ws = match accept_hdr(stream, callback) {
        Ok(ws) => ws,
        Err(error) => {
            tracing::warn!(?peer, "websocket handshake failed: {}", error);
            return;
        }
    };

    let path = selected_path.lock().expect("ws path lock poisoned").clone();
    if path == node_path {
        handle_node_websocket(ws, gateway, config, peer.map(|value| value.to_string()).unwrap_or_else(|| "unknown".to_string()));
    } else if path == backend_path {
        handle_backend_websocket(ws, gateway, config);
    }
}

fn handle_node_websocket(mut ws: WebSocket<TcpStream>, gateway: SharedGateway, config: NodeGatewayConfig, peer: String) {
    let _ = ws.get_mut().set_read_timeout(Some(WS_READ_TIMEOUT));
    let _ = ws.get_mut().set_nodelay(true);

    let codec = ControlRoomCodecJson;
    let (tx, rx) = mpsc::channel::<NodeOutbound>();
    let session_id = uuid::Uuid::new_v4().to_string();
    let mut node_id: Option<String> = None;
    let connected_at = Instant::now();
    let mut last_ping = Instant::now();

    loop {
        while let Ok(outbound) = rx.try_recv() {
            match outbound {
                NodeOutbound::Protocol(message) => {
                    let payload = codec.encode_downlink(&message);
                    if ws.send(Message::Binary(payload.into())).is_err() {
                        cleanup_node(&gateway, &node_id, &session_id, "send failed");
                        return;
                    }
                }
                NodeOutbound::Close => {
                    let _ = ws.close(None);
                    cleanup_node(&gateway, &node_id, &session_id, "closed by gateway operator");
                    return;
                }
            }
        }

        if last_ping.elapsed() >= Duration::from_secs(config.server.application_ping_secs) {
            let ping = ControlRoomToNodeMessage::Ping {
                seq: chrono::Utc::now().timestamp_millis() as u64,
                timestamp: now_iso(),
            };
            if ws.send(Message::Binary(codec.encode_downlink(&ping).into())).is_err() {
                cleanup_node(&gateway, &node_id, &session_id, "application ping failed");
                return;
            }
            last_ping = Instant::now();
        }

        if node_id.is_none() && connected_at.elapsed() > Duration::from_secs(config.server.hello_timeout_secs) {
            let rejection = ControlRoomToNodeMessage::HelloAck {
                accepted: false,
                message: Some("hello timeout".to_string()),
            };
            let _ = ws.send(Message::Binary(codec.encode_downlink(&rejection).into()));
            let _ = ws.close(None);
            return;
        }

        match ws.read() {
            Ok(Message::Binary(data)) => {
                if data.len() > config.limits.max_message_bytes {
                    cleanup_node(&gateway, &node_id, &session_id, "message too large");
                    let _ = ws.close(None);
                    return;
                }
                match codec.decode_uplink(data.as_ref()) {
                    Ok(message) => {
                        if !handle_node_message(&mut ws, &gateway, &codec, &tx, &session_id, &peer, &mut node_id, message) {
                            cleanup_node(&gateway, &node_id, &session_id, "protocol rejected");
                            return;
                        }
                    }
                    Err(error) => {
                        tracing::warn!(node_id = ?node_id, "node message decode failed: {}", error);
                        let rejection = ControlRoomToNodeMessage::HelloAck {
                            accepted: false,
                            message: Some(format!("decode failed: {error}")),
                        };
                        let _ = ws.send(Message::Binary(codec.encode_downlink(&rejection).into()));
                        cleanup_node(&gateway, &node_id, &session_id, "decode failed");
                        return;
                    }
                }
            }
            Ok(Message::Text(text)) => {
                if text.len() > config.limits.max_message_bytes {
                    cleanup_node(&gateway, &node_id, &session_id, "message too large");
                    return;
                }
                match codec.decode_uplink(text.as_bytes()) {
                    Ok(message) => {
                        if !handle_node_message(&mut ws, &gateway, &codec, &tx, &session_id, &peer, &mut node_id, message) {
                            cleanup_node(&gateway, &node_id, &session_id, "protocol rejected");
                            return;
                        }
                    }
                    Err(error) => {
                        tracing::warn!(node_id = ?node_id, "node text decode failed: {}", error);
                    }
                }
            }
            Ok(Message::Ping(payload)) => {
                let _ = ws.send(Message::Pong(payload));
            }
            Ok(Message::Pong(_)) => {}
            Ok(Message::Close(_)) => {
                cleanup_node(&gateway, &node_id, &session_id, "peer closed websocket");
                return;
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(ref error))
                if error.kind() == std::io::ErrorKind::WouldBlock || error.kind() == std::io::ErrorKind::TimedOut => {}
            Err(tungstenite::Error::ConnectionClosed) => {
                cleanup_node(&gateway, &node_id, &session_id, "connection closed");
                return;
            }
            Err(error) => {
                cleanup_node(&gateway, &node_id, &session_id, &format!("read failed: {error}"));
                return;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_node_message(
    ws: &mut WebSocket<TcpStream>,
    gateway: &SharedGateway,
    codec: &ControlRoomCodecJson,
    sender: &mpsc::Sender<NodeOutbound>,
    session_id: &str,
    peer: &str,
    node_id: &mut Option<String>,
    message: NodeToControlRoomMessage,
) -> bool {
    match &message {
        NodeToControlRoomMessage::Hello { hello } => {
            if let Some(existing) = node_id.as_deref() {
                if existing != hello.node.node_id {
                    let rejection = ControlRoomToNodeMessage::HelloAck {
                        accepted: false,
                        message: Some("node_id changed within one websocket session".to_string()),
                    };
                    let _ = ws.send(Message::Binary(codec.encode_downlink(&rejection).into()));
                    return false;
                }
                gateway.handle_node_message(existing, session_id, message);
                return true;
            }

            match gateway.register_node(hello, session_id.to_string(), peer.to_string(), sender.clone()) {
                Ok(()) => {
                    *node_id = Some(hello.node.node_id.clone());
                    let ack = ControlRoomToNodeMessage::HelloAck {
                        accepted: true,
                        message: Some("NetCore Node Gateway accepted node in OPEN LAB mode".to_string()),
                    };
                    ws.send(Message::Binary(codec.encode_downlink(&ack).into())).is_ok()
                }
                Err(reason) => {
                    let rejection = ControlRoomToNodeMessage::HelloAck {
                        accepted: false,
                        message: Some(reason),
                    };
                    let _ = ws.send(Message::Binary(codec.encode_downlink(&rejection).into()));
                    false
                }
            }
        }
        _ => {
            let Some(id) = node_id.as_deref() else {
                let rejection = ControlRoomToNodeMessage::HelloAck {
                    accepted: false,
                    message: Some("first node message must be hello".to_string()),
                };
                let _ = ws.send(Message::Binary(codec.encode_downlink(&rejection).into()));
                return false;
            };
            gateway.handle_node_message(id, session_id, message);
            true
        }
    }
}

fn cleanup_node(gateway: &SharedGateway, node_id: &Option<String>, session_id: &str, reason: &str) {
    if let Some(node_id) = node_id {
        gateway.mark_disconnected(node_id, session_id, reason);
    }
}

fn handle_backend_websocket(mut ws: WebSocket<TcpStream>, gateway: SharedGateway, config: NodeGatewayConfig) {
    let _ = ws.get_mut().set_read_timeout(Some(WS_READ_TIMEOUT));
    let _ = ws.get_mut().set_nodelay(true);
    let (backend_id, rx) = gateway.register_backend();

    loop {
        while let Ok(event) = rx.try_recv() {
            if send_backend_event(&mut ws, &event).is_err() {
                gateway.unregister_backend(&backend_id);
                return;
            }
        }

        match ws.read() {
            Ok(Message::Binary(data)) => {
                if data.len() > config.limits.max_message_bytes {
                    let _ = send_backend_event(&mut ws, &BackendEvent::ActionResult { ok: false, message: "message too large".to_string() });
                    continue;
                }
                handle_backend_request(&mut ws, &gateway, serde_json::from_slice(data.as_ref()));
            }
            Ok(Message::Text(text)) => {
                if text.len() > config.limits.max_message_bytes {
                    let _ = send_backend_event(&mut ws, &BackendEvent::ActionResult { ok: false, message: "message too large".to_string() });
                    continue;
                }
                handle_backend_request(&mut ws, &gateway, serde_json::from_str(text.as_str()));
            }
            Ok(Message::Ping(payload)) => {
                let _ = ws.send(Message::Pong(payload));
            }
            Ok(Message::Pong(_)) => {}
            Ok(Message::Close(_)) => {
                gateway.unregister_backend(&backend_id);
                return;
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(ref error))
                if error.kind() == std::io::ErrorKind::WouldBlock || error.kind() == std::io::ErrorKind::TimedOut => {}
            Err(_) => {
                gateway.unregister_backend(&backend_id);
                return;
            }
        }
    }
}

fn handle_backend_request(
    ws: &mut WebSocket<TcpStream>,
    gateway: &SharedGateway,
    request: Result<BackendRequest, serde_json::Error>,
) {
    let result = match request {
        Ok(BackendRequest::Ping) => Ok("pong".to_string()),
        Ok(BackendRequest::PingNode { node_id }) => gateway.ping_node(&node_id).map(|_| format!("ping queued for {node_id}")),
        Ok(BackendRequest::DisconnectNode { node_id }) => gateway.disconnect_node(&node_id).map(|_| format!("disconnect queued for {node_id}")),
        Ok(BackendRequest::Command { node_id, command, operator_id }) => gateway
            .send_command(&node_id, command, operator_id)
            .map(|command_id| format!("command queued: {command_id}")),
        Err(error) => Err(format!("invalid backend request: {error}")),
    };
    let event = match result {
        Ok(message) => BackendEvent::ActionResult { ok: true, message },
        Err(message) => BackendEvent::ActionResult { ok: false, message },
    };
    let _ = send_backend_event(ws, &event);
}

fn send_backend_event(ws: &mut WebSocket<TcpStream>, event: &BackendEvent) -> Result<(), tungstenite::Error> {
    let payload = serde_json::to_string(event).unwrap_or_else(|_| {
        "{\"kind\":\"action_result\",\"ok\":false,\"message\":\"serialization failed\"}".to_string()
    });
    ws.send(Message::Text(payload.into()))
}

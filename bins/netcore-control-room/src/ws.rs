use std::net::TcpStream;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use tetra_entities::net_control_room::{
    CONTROL_ROOM_PROTOCOL_VERSION, ControlRoomCodecJson, ControlRoomToNodeMessage, NodeToControlRoomMessage,
};
use tungstenite::handshake::server::{Request, Response};
use tungstenite::{Message, WebSocket, accept_hdr};

use crate::auth::{AuthRole, AuthState};
use crate::state::{SharedControlRoom, UiMessage, now_iso};

const WS_READ_TIMEOUT: Duration = Duration::from_millis(100);
const NODE_PING_INTERVAL: Duration = Duration::from_secs(15);

pub fn handle_websocket_stream(stream: TcpStream, state: SharedControlRoom, node_path: String, ui_path: String, auth: AuthState) {
    let peer = stream.peer_addr().ok();
    let selected_path = Arc::new(Mutex::new(String::new()));
    let selected_path_cb = selected_path.clone();
    let authorized = Arc::new(Mutex::new(false));
    let authorized_cb = authorized.clone();
    let node_path_cb = node_path.clone();
    let ui_path_cb = ui_path.clone();
    let auth_cb = auth.clone();

    let callback = move |req: &Request, mut response: Response| {
        let path = req.uri().path().to_string();
        *selected_path_cb.lock().expect("ws path mutex poisoned") = path.clone();

        let role = if path == node_path_cb {
            Some(AuthRole::Node)
        } else if path == ui_path_cb {
            Some(AuthRole::Viewer)
        } else {
            None
        };
        let ok = role
            .map(|role| auth_cb.authorize_ws_request(role, req).is_ok())
            .unwrap_or(true);
        *authorized_cb.lock().expect("ws auth mutex poisoned") = ok;

        // The BS requests a subprotocol. Echo it when it is the expected one so
        // strict clients and future tooling can see the negotiated protocol.
        if let Some(requested) = req.headers().get("sec-websocket-protocol").and_then(|h| h.to_str().ok()) {
            if requested
                .split(',')
                .map(str::trim)
                .any(|proto| proto == CONTROL_ROOM_PROTOCOL_VERSION)
            {
                response.headers_mut().insert(
                    "Sec-WebSocket-Protocol",
                    CONTROL_ROOM_PROTOCOL_VERSION.parse().expect("valid ws protocol header"),
                );
            }
        }
        Ok(response)
    };

    let ws = match accept_hdr(stream, callback) {
        Ok(ws) => ws,
        Err(e) => {
            tracing::warn!(?peer, "websocket handshake failed: {}", e);
            return;
        }
    };

    let path = selected_path.lock().expect("ws path mutex poisoned").clone();
    tracing::info!(?peer, path = %path, "websocket connected");

    let auth_ok = *authorized.lock().expect("ws auth mutex poisoned");
    if (path == node_path || path == ui_path) && !auth_ok {
        tracing::warn!(?peer, path = %path, "websocket rejected: unauthorized");
        let mut ws = ws;
        let _ = ws.close(None);
        return;
    }

    if path == node_path {
        handle_node_websocket(ws, state);
    } else if path == ui_path {
        handle_ui_websocket(ws, state);
    } else {
        tracing::warn!(?peer, path = %path, "websocket path rejected");
        let mut ws = ws;
        let _ = ws.close(None);
    }
}

fn handle_node_websocket(mut ws: WebSocket<TcpStream>, state: SharedControlRoom) {
    let _ = ws.get_mut().set_read_timeout(Some(WS_READ_TIMEOUT));
    let _ = ws.get_mut().set_nodelay(true);

    let codec = ControlRoomCodecJson;
    let (tx, rx) = mpsc::channel::<ControlRoomToNodeMessage>();
    let mut node_id: Option<String> = None;
    let mut last_ping = std::time::Instant::now();

    loop {
        // First drain commands that were queued by the HTTP/API side.
        while let Ok(msg) = rx.try_recv() {
            let payload = codec.encode_downlink(&msg);
            if let Err(e) = ws.send(Message::Binary(payload.into())) {
                tracing::warn!(node_id = ?node_id, "node send failed: {}", e);
                cleanup_node(&state, &node_id);
                return;
            }
        }

        if last_ping.elapsed() >= NODE_PING_INTERVAL {
            let ping = ControlRoomToNodeMessage::Ping {
                seq: chrono::Utc::now().timestamp_millis() as u64,
                timestamp: now_iso(),
            };
            let payload = codec.encode_downlink(&ping);
            if let Err(e) = ws.send(Message::Binary(payload.into())) {
                tracing::warn!(node_id = ?node_id, "node app-ping failed: {}", e);
                cleanup_node(&state, &node_id);
                return;
            }
            last_ping = std::time::Instant::now();
        }

        match ws.read() {
            Ok(Message::Binary(data)) => match codec.decode_uplink(data.as_ref()) {
                Ok(message) => {
                    let is_hello = matches!(message, NodeToControlRoomMessage::Hello { .. });
                    let seen_node_id = state.handle_node_message(message);
                    if is_hello {
                        if let Some(id) = seen_node_id {
                            if node_id.as_deref() != Some(id.as_str()) {
                                node_id = Some(id.clone());
                                state.register_node_sender(id.clone(), tx.clone());
                            }
                            let ack = ControlRoomToNodeMessage::HelloAck {
                                accepted: true,
                                message: Some("NetCore Control Room accepted node".to_string()),
                            };
                            let payload = codec.encode_downlink(&ack);
                            if let Err(e) = ws.send(Message::Binary(payload.into())) {
                                tracing::warn!(node_id = ?node_id, "hello ack send failed: {}", e);
                                cleanup_node(&state, &node_id);
                                return;
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(node_id = ?node_id, "node message decode failed: {}", e);
                    let error = ControlRoomToNodeMessage::HelloAck {
                        accepted: false,
                        message: Some(format!("decode failed: {}", e)),
                    };
                    let payload = codec.encode_downlink(&error);
                    let _ = ws.send(Message::Binary(payload.into()));
                }
            },
            Ok(Message::Text(text)) => {
                tracing::warn!(node_id = ?node_id, bytes = text.len(), "unexpected node text message");
            }
            Ok(Message::Ping(payload)) => {
                let _ = ws.send(Message::Pong(payload));
            }
            Ok(Message::Pong(_)) => {}
            Ok(Message::Close(_)) => {
                tracing::info!(node_id = ?node_id, "node websocket closed");
                cleanup_node(&state, &node_id);
                return;
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(ref e))
                if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(tungstenite::Error::ConnectionClosed) => {
                cleanup_node(&state, &node_id);
                return;
            }
            Err(e) => {
                tracing::warn!(node_id = ?node_id, "node websocket read failed: {}", e);
                cleanup_node(&state, &node_id);
                return;
            }
        }
    }
}

fn cleanup_node(state: &SharedControlRoom, node_id: &Option<String>) {
    if let Some(id) = node_id {
        state.unregister_node_sender(id);
    }
}

fn handle_ui_websocket(mut ws: WebSocket<TcpStream>, state: SharedControlRoom) {
    let _ = ws.get_mut().set_read_timeout(Some(WS_READ_TIMEOUT));
    let _ = ws.get_mut().set_nodelay(true);

    let (ui_id, rx) = state.register_ui();
    tracing::info!(ui_id = %ui_id, "ui websocket registered");

    loop {
        while let Ok(msg) = rx.try_recv() {
            let payload = match serde_json::to_vec(&msg) {
                Ok(payload) => payload,
                Err(e) => {
                    tracing::warn!(ui_id = %ui_id, "ui message serialisation failed: {}", e);
                    continue;
                }
            };
            if let Err(e) = ws.send(Message::Binary(payload.into())) {
                tracing::warn!(ui_id = %ui_id, "ui send failed: {}", e);
                state.unregister_ui(&ui_id);
                return;
            }
        }

        match ws.read() {
            Ok(Message::Ping(payload)) => {
                let _ = ws.send(Message::Pong(payload));
            }
            Ok(Message::Close(_)) => {
                state.unregister_ui(&ui_id);
                return;
            }
            Ok(Message::Text(text)) => {
                if text.trim() == "state" {
                    let msg = UiMessage::StateSnapshot { snapshot: state.snapshot() };
                    if let Ok(payload) = serde_json::to_vec(&msg) {
                        let _ = ws.send(Message::Binary(payload.into()));
                    }
                }
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(ref e))
                if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(tungstenite::Error::ConnectionClosed) => {
                state.unregister_ui(&ui_id);
                return;
            }
            Err(e) => {
                tracing::warn!(ui_id = %ui_id, "ui websocket read failed: {}", e);
                state.unregister_ui(&ui_id);
                return;
            }
        }
    }
}

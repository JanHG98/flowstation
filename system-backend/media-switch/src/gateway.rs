use std::sync::mpsc::Receiver;
use std::thread;
use std::time::Duration;

use tungstenite::client::IntoClientRequest;
use tungstenite::http::HeaderValue;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket, connect};

use crate::config::MediaSwitchConfig;
use crate::protocol::{BACKEND_PROTOCOL_VERSION, BackendEvent, BackendRequest};
use crate::state::SharedMedia;

pub fn spawn_gateway_worker(
    config: MediaSwitchConfig,
    media: SharedMedia,
    rx: Receiver<BackendRequest>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || loop {
        match connect_gateway(&config) {
            Ok(mut socket) => {
                tracing::info!(
                    "Media Switch connected to Node Gateway {}",
                    config.node_gateway.url
                );
                media.gateway_connected();
                if let Err(error) = connected_loop(&mut socket, &media, &rx) {
                    tracing::warn!("Media Switch gateway connection ended: {}", error);
                    media.gateway_disconnected(error);
                }
            }
            Err(error) => {
                tracing::warn!("Media Switch cannot connect to Node Gateway: {}", error);
                media.gateway_disconnected(error);
            }
        }
        thread::sleep(Duration::from_secs(config.node_gateway.reconnect_secs));
    })
}

fn connect_gateway(
    config: &MediaSwitchConfig,
) -> Result<WebSocket<MaybeTlsStream<std::net::TcpStream>>, String> {
    let mut request = config
        .node_gateway
        .url
        .clone()
        .into_client_request()
        .map_err(|error| format!("invalid Node Gateway URL: {error}"))?;
    request.headers_mut().insert(
        "Sec-WebSocket-Protocol",
        HeaderValue::from_static(BACKEND_PROTOCOL_VERSION),
    );
    let (mut socket, response) =
        connect(request).map_err(|error| format!("Node Gateway connection failed: {error}"))?;
    if response.status() != tungstenite::http::StatusCode::SWITCHING_PROTOCOLS {
        return Err(format!("Node Gateway returned {}", response.status()));
    }
    if let MaybeTlsStream::Plain(stream) = socket.get_mut() {
        let _ = stream.set_read_timeout(Some(Duration::from_millis(10)));
        let _ = stream.set_write_timeout(Some(Duration::from_millis(100)));
        let _ = stream.set_nodelay(true);
    }
    send_request(
        &mut socket,
        &BackendRequest::Subscribe {
            request_id: Some("media-switch-subscribe".to_string()),
            topics: vec!["media_frames".to_string()],
        },
    )?;
    Ok(socket)
}

fn connected_loop(
    socket: &mut WebSocket<MaybeTlsStream<std::net::TcpStream>>,
    media: &SharedMedia,
    rx: &Receiver<BackendRequest>,
) -> Result<(), String> {
    loop {
        for request in media.drain_due_frames() {
            send_request(socket, &request)?;
        }

        loop {
            match rx.try_recv() {
                Ok(request) => send_request(socket, &request)?,
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    return Err("Media Switch command queue closed".to_string());
                }
            }
        }

        match socket.read() {
            Ok(Message::Text(text)) => {
                handle_event(media, serde_json::from_str(text.as_str()))?;
            }
            Ok(Message::Binary(data)) => {
                handle_event(media, serde_json::from_slice(data.as_ref()))?;
            }
            Ok(Message::Ping(payload)) => socket
                .send(Message::Pong(payload))
                .map_err(|error| format!("pong failed: {error}"))?,
            Ok(Message::Pong(_)) => {}
            Ok(Message::Close(_)) => {
                return Err("Node Gateway closed connection".to_string());
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(ref error))
                if error.kind() == std::io::ErrorKind::WouldBlock
                    || error.kind() == std::io::ErrorKind::TimedOut => {}
            Err(error) => return Err(format!("Node Gateway read failed: {error}")),
        }
    }
}

fn handle_event(
    media: &SharedMedia,
    event: Result<BackendEvent, serde_json::Error>,
) -> Result<(), String> {
    let event = event.map_err(|error| format!("invalid Node Gateway event: {error}"))?;
    media.handle_backend_event(event);
    Ok(())
}

fn send_request(
    socket: &mut WebSocket<MaybeTlsStream<std::net::TcpStream>>,
    request: &BackendRequest,
) -> Result<(), String> {
    let payload = serde_json::to_string(request)
        .map_err(|error| format!("request serialization failed: {error}"))?;
    socket
        .send(Message::Text(payload.into()))
        .map_err(|error| format!("Node Gateway send failed: {error}"))
}

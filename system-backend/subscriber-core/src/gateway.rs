use std::sync::mpsc::Receiver;
use std::thread;
use std::time::Duration;

use tungstenite::client::IntoClientRequest;
use tungstenite::http::HeaderValue;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket, connect};

use crate::config::SubscriberCoreConfig;
use crate::protocol::{BACKEND_PROTOCOL_VERSION, BackendEvent, BackendRequest};
use crate::state::SharedSubscribers;

pub fn spawn_gateway_worker(
    config: SubscriberCoreConfig,
    subscribers: SharedSubscribers,
    rx: Receiver<BackendRequest>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || run_gateway_worker(config, subscribers, rx))
}

fn run_gateway_worker(
    config: SubscriberCoreConfig,
    subscribers: SharedSubscribers,
    rx: Receiver<BackendRequest>,
) {
    loop {
        match connect_gateway(&config) {
            Ok(mut socket) => {
                subscribers.gateway_connected();
                if let Err(error) = connected_loop(&mut socket, &subscribers, &rx) {
                    subscribers.gateway_disconnected(error);
                }
            }
            Err(error) => subscribers.gateway_disconnected(error),
        }
        thread::sleep(Duration::from_secs(config.node_gateway.reconnect_secs));
    }
}

fn connect_gateway(
    config: &SubscriberCoreConfig,
) -> Result<WebSocket<MaybeTlsStream<std::net::TcpStream>>, String> {
    let mut request = config.node_gateway.url.clone()
        .into_client_request()
        .map_err(|error| format!("invalid node gateway URL: {error}"))?;
    request.headers_mut().insert(
        "Sec-WebSocket-Protocol",
        HeaderValue::from_static(BACKEND_PROTOCOL_VERSION),
    );
    let (mut socket, response) = connect(request)
        .map_err(|error| format!("node gateway connection failed: {error}"))?;
    if response.status() != tungstenite::http::StatusCode::SWITCHING_PROTOCOLS {
        return Err(format!("node gateway returned {}", response.status()));
    }
    if let MaybeTlsStream::Plain(stream) = socket.get_mut() {
        let _ = stream.set_read_timeout(Some(Duration::from_millis(200)));
        let _ = stream.set_nodelay(true);
    }
    Ok(socket)
}

fn connected_loop(
    socket: &mut WebSocket<MaybeTlsStream<std::net::TcpStream>>,
    subscribers: &SharedSubscribers,
    rx: &Receiver<BackendRequest>,
) -> Result<(), String> {
    loop {
        loop {
            match rx.try_recv() {
                Ok(request) => send_request(socket, &request)?,
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    return Err("subscriber command queue closed".to_string());
                }
            }
        }

        match socket.read() {
            Ok(Message::Text(text)) => {
                handle_event(socket, subscribers, serde_json::from_str(text.as_str()))?;
            }
            Ok(Message::Binary(data)) => {
                handle_event(socket, subscribers, serde_json::from_slice(data.as_ref()))?;
            }
            Ok(Message::Ping(payload)) => {
                socket.send(Message::Pong(payload))
                    .map_err(|error| format!("pong failed: {error}"))?;
            }
            Ok(Message::Pong(_)) => {}
            Ok(Message::Close(_)) => return Err("node gateway closed connection".to_string()),
            Ok(_) => {}
            Err(tungstenite::Error::Io(ref error))
                if error.kind() == std::io::ErrorKind::WouldBlock
                    || error.kind() == std::io::ErrorKind::TimedOut => {}
            Err(error) => return Err(format!("node gateway read failed: {error}")),
        }

        for request in subscribers.expire_syncs() {
            send_request(socket, &request)?;
        }
    }
}

fn handle_event(
    socket: &mut WebSocket<MaybeTlsStream<std::net::TcpStream>>,
    subscribers: &SharedSubscribers,
    event: Result<BackendEvent, serde_json::Error>,
) -> Result<(), String> {
    let event = event.map_err(|error| format!("invalid node gateway event: {error}"))?;
    for request in subscribers.handle_backend_event(event) {
        send_request(socket, &request)?;
    }
    Ok(())
}

fn send_request(
    socket: &mut WebSocket<MaybeTlsStream<std::net::TcpStream>>,
    request: &BackendRequest,
) -> Result<(), String> {
    let payload = serde_json::to_string(request)
        .map_err(|error| format!("request serialization failed: {error}"))?;
    socket.send(Message::Text(payload.into()))
        .map_err(|error| format!("node gateway send failed: {error}"))
}

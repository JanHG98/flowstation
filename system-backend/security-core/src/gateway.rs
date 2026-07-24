use std::thread;
use std::time::{Duration, Instant};

use tungstenite::client::IntoClientRequest;
use tungstenite::http::HeaderValue;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket, connect};

use crate::config::SecurityCoreConfig;
use crate::protocol::{BACKEND_PROTOCOL_VERSION, BackendEvent, BackendRequest};
use crate::state::SharedSecurityCore;

pub fn spawn_gateway_worker(
    config: SecurityCoreConfig,
    core: SharedSecurityCore,
) -> thread::JoinHandle<()> {
    thread::spawn(move || run_gateway_worker(config, core))
}

fn run_gateway_worker(config: SecurityCoreConfig, core: SharedSecurityCore) {
    if !config.node_gateway.observe_nodes {
        tracing::info!("Security Core Node Gateway observation is disabled");
        return;
    }
    loop {
        match connect_gateway(&config) {
            Ok(mut socket) => {
                core.gateway_connected();
                if let Err(error) = connected_loop(&mut socket, &core) {
                    core.gateway_disconnected(error);
                }
            }
            Err(error) => core.gateway_disconnected(error),
        }
        thread::sleep(Duration::from_secs(config.node_gateway.reconnect_secs));
    }
}

fn connect_gateway(
    config: &SecurityCoreConfig,
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
        let _ = stream.set_read_timeout(Some(Duration::from_millis(250)));
        let _ = stream.set_nodelay(true);
    }
    Ok(socket)
}

fn connected_loop(
    socket: &mut WebSocket<MaybeTlsStream<std::net::TcpStream>>,
    core: &SharedSecurityCore,
) -> Result<(), String> {
    let mut last_ping = Instant::now();
    loop {
        match socket.read() {
            Ok(Message::Text(text)) => handle_event(core, serde_json::from_str(text.as_str()))?,
            Ok(Message::Binary(data)) => handle_event(core, serde_json::from_slice(data.as_ref()))?,
            Ok(Message::Ping(payload)) => socket
                .send(Message::Pong(payload))
                .map_err(|error| format!("Node Gateway pong failed: {error}"))?,
            Ok(Message::Pong(_)) => {}
            Ok(Message::Close(_)) => return Err("Node Gateway closed connection".to_string()),
            Ok(_) => {}
            Err(tungstenite::Error::Io(ref error))
                if error.kind() == std::io::ErrorKind::WouldBlock
                    || error.kind() == std::io::ErrorKind::TimedOut => {}
            Err(error) => return Err(format!("Node Gateway read failed: {error}")),
        }

        if last_ping.elapsed() >= Duration::from_secs(15) {
            let request = BackendRequest::Ping {
                request_id: Some(uuid::Uuid::new_v4().to_string()),
            };
            let payload = serde_json::to_string(&request)
                .map_err(|error| format!("ping serialization failed: {error}"))?;
            socket
                .send(Message::Text(payload.into()))
                .map_err(|error| format!("Node Gateway ping failed: {error}"))?;
            last_ping = Instant::now();
        }
    }
}

fn handle_event(
    core: &SharedSecurityCore,
    event: Result<BackendEvent, serde_json::Error>,
) -> Result<(), String> {
    let event = event.map_err(|error| format!("invalid Node Gateway event: {error}"))?;
    core.handle_backend_event(event);
    Ok(())
}

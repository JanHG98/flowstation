use std::io::ErrorKind;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

use serde_json::Value;

use crate::auth::AuthState;
use crate::http::{handle_http_stream, looks_like_websocket_upgrade, SharedDirectory};
use crate::operations::SharedOperations;
use crate::state::SharedControlRoom;
use crate::ws::handle_websocket_stream;

pub struct ControlRoomServer {
    bind: SocketAddr,
    node_path: String,
    ui_path: String,
    state: SharedControlRoom,
    auth: AuthState,
    directory: SharedDirectory,
    operations: SharedOperations,
}

impl ControlRoomServer {
    pub fn new(
        bind: SocketAddr,
        node_path: String,
        ui_path: String,
        state: SharedControlRoom,
        auth: AuthState,
        directory: Value,
        operations: SharedOperations,
    ) -> Self {
        Self {
            bind,
            node_path: normalize_path(node_path),
            ui_path: normalize_path(ui_path),
            state,
            auth,
            directory: std::sync::Arc::new(std::sync::Mutex::new(directory)),
            operations,
        }
    }

    pub fn run(&self) -> std::io::Result<()> {
        let listener = TcpListener::bind(self.bind)?;
        tracing::info!(bind = %self.bind, node_path = %self.node_path, ui_path = %self.ui_path, "NetCore Control Room listening");

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => self.spawn_connection(stream),
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => tracing::warn!("accept failed: {}", e),
            }
        }
        Ok(())
    }

    fn spawn_connection(&self, stream: TcpStream) {
        let state = self.state.clone();
        let node_path = self.node_path.clone();
        let ui_path = self.ui_path.clone();
        let auth = self.auth.clone();
        let directory = self.directory.clone();
        let operations = self.operations.clone();
        let peer = stream.peer_addr().ok();

        let _ = thread::Builder::new()
            .name("control-room-client".to_string())
            .spawn(move || {
                let mut peek = [0u8; 2048];
                let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                match stream.peek(&mut peek) {
                    Ok(n) if n > 0 && looks_like_websocket_upgrade(&peek[..n]) => {
                        handle_websocket_stream(stream, state, node_path, ui_path, auth);
                    }
                    Ok(_) => {
                        handle_http_stream(stream, state, &node_path, &ui_path, auth, directory, operations);
                    }
                    Err(e) => {
                        tracing::warn!(?peer, "initial stream peek failed: {}", e);
                    }
                }
            });
    }
}

fn normalize_path(path: String) -> String {
    if path.starts_with('/') {
        path
    } else {
        format!("/{}", path)
    }
}

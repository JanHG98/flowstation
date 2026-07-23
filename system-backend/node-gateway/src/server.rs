use std::io::ErrorKind;
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

use crate::config::NodeGatewayConfig;
use crate::http::{handle_http_stream, looks_like_websocket_upgrade};
use crate::state::SharedGateway;
use crate::ws::handle_websocket_stream;

pub struct NodeGatewayServer {
    config: NodeGatewayConfig,
    gateway: SharedGateway,
}

impl NodeGatewayServer {
    pub fn new(config: NodeGatewayConfig, gateway: SharedGateway) -> Self {
        Self { config, gateway }
    }

    pub fn run(&self) -> std::io::Result<()> {
        let listener = TcpListener::bind(self.config.server.bind)?;
        tracing::warn!(
            bind = %self.config.server.bind,
            node_path = %self.config.server.node_path,
            backend_path = %self.config.server.backend_path,
            "Node Gateway listening in OPEN LAB mode: no authentication, no tokens, no TLS"
        );

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => self.spawn_connection(stream),
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(error) => tracing::warn!("accept failed: {}", error),
            }
        }
        Ok(())
    }

    fn spawn_connection(&self, stream: TcpStream) {
        let gateway = self.gateway.clone();
        let config = self.config.clone();
        let peer = stream.peer_addr().ok();
        let _ = thread::Builder::new()
            .name("node-gateway-client".to_string())
            .spawn(move || {
                let mut peek = [0u8; 2_048];
                let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                match stream.peek(&mut peek) {
                    Ok(read) if read > 0 && looks_like_websocket_upgrade(&peek[..read]) => {
                        handle_websocket_stream(stream, gateway, config);
                    }
                    Ok(_) => handle_http_stream(stream, gateway, config),
                    Err(error) => tracing::warn!(?peer, "initial stream peek failed: {}", error),
                }
            });
    }
}

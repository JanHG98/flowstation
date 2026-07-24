use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

use serde::de::DeserializeOwned;

use crate::config::PacketCoreClientConfig;
use crate::protocol::{DownlinkNpduInput, PacketCoreContext, PacketCoreNpdu, PacketCoreStatus};

#[derive(Debug, Clone)]
pub struct PacketCoreClient {
    base: HttpBase,
    timeout: Duration,
}

#[derive(Debug, Clone)]
struct HttpBase {
    host: String,
    port: u16,
    base_path: String,
}

struct HttpResponse {
    status: u16,
    body: Vec<u8>,
}

impl PacketCoreClient {
    pub fn new(config: &PacketCoreClientConfig) -> Result<Self, String> {
        Ok(Self {
            base: HttpBase::parse(&config.url)?,
            timeout: Duration::from_millis(config.request_timeout_ms),
        })
    }

    pub fn status(&self) -> Result<PacketCoreStatus, String> {
        self.get_json("/api/v1/status")
    }

    pub fn contexts(&self) -> Result<Vec<PacketCoreContext>, String> {
        self.get_json("/api/v1/contexts")
    }

    pub fn npdu_outbox(&self, limit: usize) -> Result<Vec<PacketCoreNpdu>, String> {
        self.get_json(&format!("/api/v1/npdu-outbox?limit={limit}"))
    }

    pub fn delete_npdu(&self, id: &str) -> Result<(), String> {
        let response = self.request("DELETE", &format!("/api/v1/npdu-outbox/{id}"), None)?;
        if matches!(response.status, 200 | 204 | 404) {
            Ok(())
        } else {
            Err(format!(
                "Packet Core DELETE N-PDU returned HTTP {}: {}",
                response.status,
                String::from_utf8_lossy(&response.body)
            ))
        }
    }

    pub fn queue_downlink(&self, input: &DownlinkNpduInput) -> Result<(), String> {
        let body = serde_json::to_vec(input).map_err(|error| error.to_string())?;
        let response = self.request("POST", "/api/v1/downlink", Some(&body))?;
        if matches!(response.status, 200 | 201 | 202 | 204) {
            Ok(())
        } else {
            Err(format!(
                "Packet Core downlink returned HTTP {}: {}",
                response.status,
                String::from_utf8_lossy(&response.body)
            ))
        }
    }

    fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T, String> {
        let response = self.request("GET", path, None)?;
        if response.status != 200 {
            return Err(format!(
                "Packet Core GET {path} returned HTTP {}: {}",
                response.status,
                String::from_utf8_lossy(&response.body)
            ));
        }
        serde_json::from_slice(&response.body).map_err(|error| error.to_string())
    }

    fn request(&self, method: &str, path: &str, body: Option<&[u8]>) -> Result<HttpResponse, String> {
        let address = resolve_one(&self.base.host, self.base.port)?;
        let mut stream = TcpStream::connect_timeout(&address, self.timeout)
            .map_err(|error| format!("connect Packet Core {address}: {error}"))?;
        stream
            .set_read_timeout(Some(self.timeout))
            .map_err(|error| error.to_string())?;
        stream
            .set_write_timeout(Some(self.timeout))
            .map_err(|error| error.to_string())?;

        let full_path = self.base.join_path(path);
        let body = body.unwrap_or_default();
        let request = format!(
            "{method} {full_path} HTTP/1.1\r\nHost: {}:{}\r\nAccept: application/json\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            self.base.host,
            self.base.port,
            body.len()
        );
        stream
            .write_all(request.as_bytes())
            .and_then(|()| stream.write_all(body))
            .map_err(|error| format!("write Packet Core request: {error}"))?;

        let mut response = Vec::new();
        stream
            .read_to_end(&mut response)
            .map_err(|error| format!("read Packet Core response: {error}"))?;
        parse_http_response(&response)
    }
}

impl HttpBase {
    fn parse(value: &str) -> Result<Self, String> {
        let value = value
            .strip_prefix("http://")
            .ok_or_else(|| "only http:// Packet Core URLs are supported".to_string())?;
        let (authority, path) = match value.split_once('/') {
            Some((authority, path)) => (authority, format!("/{path}")),
            None => (value, String::new()),
        };
        let (host, port) = match authority.rsplit_once(':') {
            Some((host, port)) => {
                let port = port
                    .parse::<u16>()
                    .map_err(|_| "invalid Packet Core port".to_string())?;
                (host.to_string(), port)
            }
            None => (authority.to_string(), 80),
        };
        if host.is_empty() {
            return Err("Packet Core host may not be empty".to_string());
        }
        Ok(Self {
            host,
            port,
            base_path: path.trim_end_matches('/').to_string(),
        })
    }

    fn join_path(&self, path: &str) -> String {
        let path = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{path}")
        };
        format!("{}{}", self.base_path, path)
    }
}

fn resolve_one(host: &str, port: u16) -> Result<SocketAddr, String> {
    (host, port)
        .to_socket_addrs()
        .map_err(|error| format!("resolve Packet Core {host}:{port}: {error}"))?
        .next()
        .ok_or_else(|| format!("Packet Core {host}:{port} resolved to no address"))
}

fn parse_http_response(bytes: &[u8]) -> Result<HttpResponse, String> {
    let split = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| "Packet Core response has no header terminator".to_string())?;
    let header = std::str::from_utf8(&bytes[..split]).map_err(|error| error.to_string())?;
    let status = header
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| "Packet Core response has invalid status line".to_string())?;
    Ok(HttpResponse {
        status,
        body: bytes[split + 4..].to_vec(),
    })
}

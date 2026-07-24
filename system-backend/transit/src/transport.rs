use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::config::{TransitConfig, MODE_AUTHORITATIVE};
use crate::protocol::MaintenanceInput;
use crate::state::{PeerRecord, SharedTransit};

pub fn spawn_transport_worker(
    config: TransitConfig,
    transit: SharedTransit,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut last_maintenance = Instant::now()
            .checked_sub(Duration::from_secs(60))
            .unwrap_or_else(Instant::now);
        loop {
            if last_maintenance.elapsed() >= Duration::from_secs(5) {
                if let Err(error) = transit.maintenance_tick(MaintenanceInput {
                    actor: Some("transport-worker".to_string()),
                }) {
                    tracing::warn!("Transit maintenance failed: {}", error);
                }
                last_maintenance = Instant::now();
            }

            if config.region.operating_mode == MODE_AUTHORITATIVE {
                send_heartbeats(&config, &transit);
                dispatch_envelopes(&config, &transit);
            }
            thread::sleep(Duration::from_millis(500));
        }
    })
}

fn send_heartbeats(config: &TransitConfig, transit: &SharedTransit) {
    for peer in transit.peers_due_for_heartbeat() {
        let payload = transit.heartbeat_payload();
        let started = Instant::now();
        let result = post_json(
            config,
            &peer,
            "/api/v1/peer/heartbeat",
            &payload,
        );
        let latency = started.elapsed().as_secs_f64() * 1_000.0;
        match result {
            Ok(()) => {
                if let Err(error) = transit.record_heartbeat_result(
                    &peer.peer_id,
                    true,
                    None,
                    Some(latency),
                ) {
                    tracing::warn!("Record heartbeat success failed for {}: {}", peer.peer_id, error);
                }
            }
            Err(error) => {
                tracing::debug!("Heartbeat to {} failed: {}", peer.peer_id, error);
                if let Err(record_error) = transit.record_heartbeat_result(
                    &peer.peer_id,
                    false,
                    Some(error),
                    None,
                ) {
                    tracing::warn!("Record heartbeat failure failed for {}: {}", peer.peer_id, record_error);
                }
            }
        }
    }
}

fn dispatch_envelopes(config: &TransitConfig, transit: &SharedTransit) {
    for due in transit.due_outbound(config.transport.max_batch) {
        let peer_id = match due.selected_peer.clone() {
            Some(peer_id) => peer_id,
            None => continue,
        };
        let peer = transit.peers().into_iter().find(|peer| peer.peer_id == peer_id);
        let Some(peer) = peer else {
            let _ = transit.complete_outbound(
                &due.envelope_id,
                false,
                Some(format!("selected peer {peer_id} no longer exists")),
                None,
            );
            continue;
        };
        let envelope = match transit.mark_outbound_attempt(&due.envelope_id) {
            Ok(envelope) => envelope,
            Err(error) => {
                tracing::debug!("Envelope {} was no longer dispatchable: {}", due.envelope_id, error);
                continue;
            }
        };
        let wire = envelope.to_wire(&config.region.region_id);
        let started = Instant::now();
        let result = post_json(config, &peer, "/api/v1/peer/envelopes", &wire);
        let latency = started.elapsed().as_secs_f64() * 1_000.0;
        match result {
            Ok(()) => {
                if let Err(error) = transit.complete_outbound(
                    &envelope.envelope_id,
                    true,
                    None,
                    Some(latency),
                ) {
                    tracing::warn!("Complete outbound success failed: {}", error);
                }
            }
            Err(error) => {
                tracing::warn!(
                    "Transit envelope {} to peer {} failed: {}",
                    envelope.envelope_id,
                    peer.peer_id,
                    error
                );
                if let Err(record_error) = transit.complete_outbound(
                    &envelope.envelope_id,
                    false,
                    Some(error),
                    None,
                ) {
                    tracing::warn!("Complete outbound failure failed: {}", record_error);
                }
            }
        }
    }
}

fn post_json<T: Serialize>(
    config: &TransitConfig,
    peer: &PeerRecord,
    path: &str,
    value: &T,
) -> Result<(), String> {
    let body = serde_json::to_vec(value).map_err(|error| format!("serialize request: {error}"))?;
    let endpoint = parse_http_endpoint(&peer.endpoint, path)?;
    let address = resolve_address(&endpoint.host, endpoint.port)?;
    let connect_timeout = Duration::from_millis(config.transport.connect_timeout_ms);
    let io_timeout = Duration::from_millis(config.transport.io_timeout_ms);
    let mut stream = TcpStream::connect_timeout(&address, connect_timeout)
        .map_err(|error| format!("connect {}:{}: {error}", endpoint.host, endpoint.port))?;
    stream
        .set_read_timeout(Some(io_timeout))
        .map_err(|error| format!("set read timeout: {error}"))?;
    stream
        .set_write_timeout(Some(io_timeout))
        .map_err(|error| format!("set write timeout: {error}"))?;
    let host_header = if endpoint.port == 80 {
        endpoint.host.clone()
    } else {
        format!("{}:{}", endpoint.host, endpoint.port)
    };
    let request = format!(
        concat!(
            "POST {} HTTP/1.1\r\n",
            "Host: {}\r\n",
            "Content-Type: application/json\r\n",
            "Content-Length: {}\r\n",
            "Connection: close\r\n",
            "X-NetCore-Transit-Protocol: {}\r\n",
            "\r\n"
        ),
        endpoint.path,
        host_header,
        body.len(),
        config.region.protocol_version
    );
    stream
        .write_all(request.as_bytes())
        .and_then(|_| stream.write_all(&body))
        .and_then(|_| stream.flush())
        .map_err(|error| format!("write request: {error}"))?;
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|error| format!("read response: {error}"))?;
    let status_line = response
        .split(|byte| *byte == b'\n')
        .next()
        .ok_or_else(|| "peer returned an empty response".to_string())?;
    let status_text = String::from_utf8_lossy(status_line);
    let status = status_text
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| format!("invalid peer HTTP status line: {}", status_text.trim()))?;
    if (200..300).contains(&status) {
        Ok(())
    } else {
        let body = response
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .map(|index| String::from_utf8_lossy(&response[index + 4..]).into_owned())
            .unwrap_or_default();
        Err(format!("peer returned HTTP {status}: {body}"))
    }
}

struct HttpEndpoint {
    host: String,
    port: u16,
    path: String,
}

fn parse_http_endpoint(base: &str, suffix: &str) -> Result<HttpEndpoint, String> {
    let raw = base
        .strip_prefix("http://")
        .ok_or_else(|| "only http:// peer endpoints are supported in open_lab mode".to_string())?;
    let (authority, base_path) = raw.split_once('/').unwrap_or((raw, ""));
    let (host, port) = if authority.starts_with('[') {
        let end = authority.find(']').ok_or_else(|| "invalid IPv6 endpoint".to_string())?;
        let host = authority[1..end].to_string();
        let port = authority[end + 1..]
            .strip_prefix(':')
            .map(|value| value.parse::<u16>())
            .transpose()
            .map_err(|error| format!("invalid peer port: {error}"))?
            .unwrap_or(80);
        (host, port)
    } else if let Some((host, port)) = authority.rsplit_once(':') {
        let port = port
            .parse::<u16>()
            .map_err(|error| format!("invalid peer port: {error}"))?;
        (host.to_string(), port)
    } else {
        (authority.to_string(), 80)
    };
    if host.is_empty() {
        return Err("peer endpoint host is empty".to_string());
    }
    let mut path = String::new();
    if !base_path.is_empty() {
        path.push('/');
        path.push_str(base_path.trim_matches('/'));
    }
    path.push('/');
    path.push_str(suffix.trim_matches('/'));
    Ok(HttpEndpoint { host, port, path })
}

fn resolve_address(host: &str, port: u16) -> Result<SocketAddr, String> {
    (host, port)
        .to_socket_addrs()
        .map_err(|error| format!("resolve {host}:{port}: {error}"))?
        .next()
        .ok_or_else(|| format!("no address found for {host}:{port}"))
}


use std::net::{SocketAddr, UdpSocket};
use std::thread;
use std::time::Duration;

use crate::config::IpGatewayConfig;
use crate::state::SharedGateway;

pub fn spawn_dns(config: IpGatewayConfig, gateway: SharedGateway) -> Option<thread::JoinHandle<()>> {
    if !config.dns.enabled {
        return None;
    }
    Some(thread::spawn(move || run(config, gateway)))
}

fn run(config: IpGatewayConfig, gateway: SharedGateway) {
    let socket = match UdpSocket::bind(config.dns.bind) {
        Ok(socket) => socket,
        Err(error) => {
            tracing::error!("DNS bind {} failed: {error}", config.dns.bind);
            return;
        }
    };
    tracing::info!("IP Gateway DNS listening on udp://{}", config.dns.bind);
    let _ = socket.set_read_timeout(Some(Duration::from_secs(1)));
    let mut buffer = [0u8; 4096];
    loop {
        let (size, peer) = match socket.recv_from(&mut buffer) {
            Ok(value) => value,
            Err(error) if matches!(error.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => continue,
            Err(error) => {
                tracing::warn!("DNS receive failed: {error}");
                continue;
            }
        };
        let query = &buffer[..size];
        let Some((name, qtype, question_end)) = parse_question(query) else {
            gateway.record_dns_query("<malformed>", &peer.to_string(), "malformed");
            continue;
        };
        if qtype == 1 {
            if let Some(address) = gateway.dns_lookup(&name) {
                let response = build_a_response(query, question_end, address.octets(), config.dns.ttl_secs);
                let _ = socket.send_to(&response, peer);
                gateway.record_dns_query(&name, &peer.to_string(), &address.to_string());
                continue;
            }
        }
        match forward_query(query, &config.dns.upstream, config.dns.query_timeout_ms) {
            Ok(response) => {
                let _ = socket.send_to(&response, peer);
                gateway.record_dns_query(&name, &peer.to_string(), "forwarded");
            }
            Err(error) => {
                let response = build_servfail(query, question_end);
                let _ = socket.send_to(&response, peer);
                gateway.record_dns_query(&name, &peer.to_string(), &format!("servfail: {error}"));
            }
        }
    }
}

fn parse_question(packet: &[u8]) -> Option<(String, u16, usize)> {
    if packet.len() < 12 || u16::from_be_bytes([packet[4], packet[5]]) == 0 {
        return None;
    }
    let mut offset = 12;
    let mut labels = Vec::new();
    loop {
        let length = *packet.get(offset)? as usize;
        offset += 1;
        if length == 0 {
            break;
        }
        if length > 63 || offset + length > packet.len() {
            return None;
        }
        labels.push(std::str::from_utf8(&packet[offset..offset + length]).ok()?.to_string());
        offset += length;
    }
    if offset + 4 > packet.len() {
        return None;
    }
    let qtype = u16::from_be_bytes([packet[offset], packet[offset + 1]]);
    let qclass = u16::from_be_bytes([packet[offset + 2], packet[offset + 3]]);
    if qclass != 1 {
        return None;
    }
    Some((labels.join(".").to_ascii_lowercase(), qtype, offset + 4))
}

fn build_a_response(query: &[u8], question_end: usize, address: [u8; 4], ttl: u32) -> Vec<u8> {
    let mut response = Vec::with_capacity(question_end + 16);
    response.extend_from_slice(&query[..2]);
    response.extend_from_slice(&0x8180u16.to_be_bytes());
    response.extend_from_slice(&1u16.to_be_bytes());
    response.extend_from_slice(&1u16.to_be_bytes());
    response.extend_from_slice(&0u16.to_be_bytes());
    response.extend_from_slice(&0u16.to_be_bytes());
    response.extend_from_slice(&query[12..question_end]);
    response.extend_from_slice(&0xc00cu16.to_be_bytes());
    response.extend_from_slice(&1u16.to_be_bytes());
    response.extend_from_slice(&1u16.to_be_bytes());
    response.extend_from_slice(&ttl.to_be_bytes());
    response.extend_from_slice(&4u16.to_be_bytes());
    response.extend_from_slice(&address);
    response
}

fn build_servfail(query: &[u8], question_end: usize) -> Vec<u8> {
    let mut response = Vec::with_capacity(question_end);
    response.extend_from_slice(&query[..2]);
    response.extend_from_slice(&0x8182u16.to_be_bytes());
    response.extend_from_slice(&1u16.to_be_bytes());
    response.extend_from_slice(&0u16.to_be_bytes());
    response.extend_from_slice(&0u16.to_be_bytes());
    response.extend_from_slice(&0u16.to_be_bytes());
    response.extend_from_slice(&query[12..question_end]);
    response
}

fn forward_query(query: &[u8], upstream: &str, timeout_ms: u64) -> Result<Vec<u8>, String> {
    let upstream = upstream
        .parse::<SocketAddr>()
        .map_err(|_| "DNS upstream must currently be an IP socket address".to_string())?;
    let socket = UdpSocket::bind("0.0.0.0:0").map_err(|error| error.to_string())?;
    let timeout = Duration::from_millis(timeout_ms);
    socket
        .set_read_timeout(Some(timeout))
        .map_err(|error| error.to_string())?;
    socket
        .connect(upstream)
        .map_err(|error| error.to_string())?;
    socket.send(query).map_err(|error| error.to_string())?;
    let mut response = vec![0u8; 4096];
    let size = socket
        .recv(&mut response)
        .map_err(|error| error.to_string())?;
    response.truncate(size);
    Ok(response)
}

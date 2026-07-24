use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::thread;
use std::time::Duration;

use serde_json::json;

use crate::config::IpGatewayConfig;
use crate::state::SharedGateway;

pub fn spawn_test_services(config: IpGatewayConfig, gateway: SharedGateway) {
    if !config.test_server.enabled {
        return;
    }
    let http_config = config.clone();
    let http_gateway = gateway.clone();
    thread::spawn(move || run_http(http_config, http_gateway));
    thread::spawn(move || run_udp_echo(config, gateway));
}

fn run_http(config: IpGatewayConfig, gateway: SharedGateway) {
    let listener = match TcpListener::bind(config.test_server.bind) {
        Ok(listener) => listener,
        Err(error) => {
            tracing::error!(
                "packet-data test server bind {} failed: {error}",
                config.test_server.bind
            );
            return;
        }
    };
    tracing::info!(
        "packet-data WAP/test server listening on http://{}",
        config.test_server.bind
    );
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let gateway = gateway.clone();
                let config = config.clone();
                thread::spawn(move || {
                    if let Err(error) = handle_http(stream, gateway, config) {
                        tracing::debug!("test HTTP request failed: {error}");
                    }
                });
            }
            Err(error) => tracing::warn!("test HTTP accept failed: {error}"),
        }
    }
}

fn handle_http(
    mut stream: TcpStream,
    gateway: SharedGateway,
    config: IpGatewayConfig,
) -> Result<(), String> {
    let peer = stream
        .peer_addr()
        .map(|value| value.to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    let mut buffer = [0u8; 8192];
    let size = stream.read(&mut buffer).map_err(|error| error.to_string())?;
    let request = std::str::from_utf8(&buffer[..size]).map_err(|error| error.to_string())?;
    let line = request.lines().next().unwrap_or_default();
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let path = parts.next().unwrap_or("/").split('?').next().unwrap_or("/");
    if method != "GET" && method != "HEAD" {
        return write_response(
            &mut stream,
            405,
            "text/plain; charset=utf-8",
            b"method not allowed\n",
            method == "HEAD",
        );
    }
    gateway.record_test_request(path, &peer);
    let gateway_address = config.gateway_ipv4();
    let domain = config.dns.local_domain.trim_end_matches('.');
    match path {
        "/" => {
            let body = format!(
                "<!doctype html><html><head><meta charset=\"utf-8\"><title>NetCore TETRA Packet Data</title></head><body><h1>NetCore-Tetra IP Gateway</h1><p>Packet data reached the gateway successfully.</p><ul><li><a href=\"/wap/\">WAP test page</a></li><li><a href=\"/test/info\">JSON connection info</a></li><li><a href=\"/test/echo\">HTTP echo test</a></li></ul><p>Gateway: {gateway_address}<br>DNS zone: {domain}</p></body></html>"
            );
            write_response(
                &mut stream,
                200,
                "text/html; charset=utf-8",
                body.as_bytes(),
                method == "HEAD",
            )
        }
        "/wap" | "/wap/" | "/index.wml" => {
            let body = format!(
                "<?xml version=\"1.0\"?><!DOCTYPE wml PUBLIC \"-//WAPFORUM//DTD WML 1.1//EN\" \"http://www.wapforum.org/DTD/wml_1.1.xml\"><wml><card id=\"home\" title=\"NetCore TETRA\"><p>IP packet data OK<br/>Gateway {gateway_address}<br/><a href=\"/wap/status.wml\">Status</a></p></card></wml>"
            );
            write_response(
                &mut stream,
                200,
                "text/vnd.wap.wml; charset=utf-8",
                body.as_bytes(),
                method == "HEAD",
            )
        }
        "/wap/status.wml" => {
            let status = gateway.status();
            let body = format!(
                "<?xml version=\"1.0\"?><!DOCTYPE wml PUBLIC \"-//WAPFORUM//DTD WML 1.1//EN\" \"http://www.wapforum.org/DTD/wml_1.1.xml\"><wml><card id=\"status\" title=\"Status\"><p>Core: {}<br/>TUN: {}<br/>Contexts: {}<br/>UL packets: {}<br/>DL packets: {}</p></card></wml>",
                if status.packet_core_connected { "online" } else { "offline" },
                if status.tun_open { "open" } else { "closed" },
                status.contexts,
                status.packets_uplink,
                status.packets_downlink
            );
            write_response(
                &mut stream,
                200,
                "text/vnd.wap.wml; charset=utf-8",
                body.as_bytes(),
                method == "HEAD",
            )
        }
        "/test/info" => {
            let body = serde_json::to_vec_pretty(&json!({
                "service":"netcore-ip-gateway-test",
                "peer":peer,
                "gateway":gateway_address,
                "dns_domain":domain,
                "status":gateway.status(),
            }))
            .map_err(|error| error.to_string())?;
            write_response(
                &mut stream,
                200,
                "application/json; charset=utf-8",
                &body,
                method == "HEAD",
            )
        }
        "/test/echo" => {
            let body = format!("NetCore-Tetra packet-data echo\npeer={peer}\n");
            write_response(
                &mut stream,
                200,
                "text/plain; charset=utf-8",
                body.as_bytes(),
                method == "HEAD",
            )
        }
        "/health" => write_response(
            &mut stream,
            200,
            "text/plain; charset=utf-8",
            b"ok\n",
            method == "HEAD",
        ),
        _ => write_response(
            &mut stream,
            404,
            "text/plain; charset=utf-8",
            b"not found\n",
            method == "HEAD",
        ),
    }
}

fn write_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
    head_only: bool,
) -> Result<(), String> {
    let reason = match status {
        200 => "OK",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "Error",
    };
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream
        .write_all(header.as_bytes())
        .and_then(|()| {
            if head_only {
                Ok(())
            } else {
                stream.write_all(body)
            }
        })
        .map_err(|error| error.to_string())
}

fn run_udp_echo(config: IpGatewayConfig, gateway: SharedGateway) {
    let socket = match UdpSocket::bind(config.test_server.udp_echo_bind) {
        Ok(socket) => socket,
        Err(error) => {
            tracing::error!(
                "packet-data UDP echo bind {} failed: {error}",
                config.test_server.udp_echo_bind
            );
            return;
        }
    };
    tracing::info!(
        "packet-data UDP echo listening on udp://{}",
        config.test_server.udp_echo_bind
    );
    let mut buffer = [0u8; 65_535];
    loop {
        match socket.recv_from(&mut buffer) {
            Ok((size, peer)) => {
                gateway.record_test_request("udp_echo", &peer.to_string());
                let _ = socket.send_to(&buffer[..size], peer);
            }
            Err(error) => tracing::warn!("UDP echo receive failed: {error}"),
        }
    }
}

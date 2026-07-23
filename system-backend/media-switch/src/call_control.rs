use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::thread;
use std::time::Duration;

use crate::config::MediaSwitchConfig;
use crate::protocol::CallControlCall;
use crate::state::SharedMedia;

pub fn spawn_call_control_worker(
    config: MediaSwitchConfig,
    media: SharedMedia,
) -> thread::JoinHandle<()> {
    thread::spawn(move || loop {
        match fetch_calls(&config) {
            Ok(calls) => media.reconcile_calls(calls),
            Err(error) => media.call_control_failed(error),
        }
        thread::sleep(Duration::from_secs(config.call_control.reconcile_secs));
    })
}

fn fetch_calls(config: &MediaSwitchConfig) -> Result<Vec<CallControlCall>, String> {
    let parsed = ParsedHttpUrl::parse(&config.call_control.url)?;
    let timeout = Duration::from_secs(config.call_control.request_timeout_secs);
    let address = (parsed.host.as_str(), parsed.port)
        .to_socket_addrs()
        .map_err(|error| format!("Call Control DNS failed: {error}"))?
        .next()
        .ok_or_else(|| "Call Control address did not resolve".to_string())?;
    let mut stream = TcpStream::connect_timeout(&address, timeout)
        .map_err(|error| format!("Call Control connection failed: {error}"))?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|error| format!("Call Control read timeout failed: {error}"))?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|error| format!("Call Control write timeout failed: {error}"))?;

    let request = format!(
        concat!(
            "GET {} HTTP/1.1\r\n",
            "Host: {}:{}\r\n",
            "Accept: application/json\r\n",
            "Connection: close\r\n\r\n"
        ),
        parsed.path, parsed.host, parsed.port
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|error| format!("Call Control request failed: {error}"))?;

    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|error| format!("Call Control response failed: {error}"))?;
    let header_end = find_subslice(&response, b"\r\n\r\n")
        .ok_or_else(|| "Call Control returned an invalid HTTP response".to_string())?;
    let header = std::str::from_utf8(&response[..header_end])
        .map_err(|_| "Call Control response headers are not UTF-8".to_string())?;
    let status = header
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| "Call Control response has no valid status".to_string())?;
    if status != 200 {
        return Err(format!("Call Control returned HTTP {status}"));
    }
    serde_json::from_slice(&response[header_end + 4..])
        .map_err(|error| format!("Call Control JSON failed: {error}"))
}

struct ParsedHttpUrl {
    host: String,
    port: u16,
    path: String,
}

impl ParsedHttpUrl {
    fn parse(url: &str) -> Result<Self, String> {
        let rest = url
            .strip_prefix("http://")
            .ok_or_else(|| "Call Control URL must start with http://".to_string())?;
        let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
        let (host, port) = authority
            .rsplit_once(':')
            .map(|(host, port)| {
                port.parse::<u16>()
                    .map(|port| (host.to_string(), port))
                    .map_err(|_| "invalid Call Control port".to_string())
            })
            .transpose()?
            .unwrap_or_else(|| (authority.to_string(), 80));
        if host.trim().is_empty() {
            return Err("Call Control host must not be empty".to_string());
        }
        Ok(Self {
            host,
            port,
            path: format!("/{path}"),
        })
    }
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::ParsedHttpUrl;

    #[test]
    fn parses_open_lab_call_control_url() {
        let url = ParsedHttpUrl::parse("http://127.0.0.1:8120/api/v1/calls")
            .expect("URL parses");
        assert_eq!(url.host, "127.0.0.1");
        assert_eq!(url.port, 8120);
        assert_eq!(url.path, "/api/v1/calls");
    }
}

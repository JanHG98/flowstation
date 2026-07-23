use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::thread;
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;

use crate::config::RecorderConfig;
use crate::protocol::{MediaSwitchSession, RecorderTapBatch};
use crate::state::SharedRecorder;

pub fn spawn_media_switch_worker(
    config: RecorderConfig,
    recorder: SharedRecorder,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut last_session_reconcile = Instant::now()
            .checked_sub(Duration::from_millis(config.media_switch.session_reconcile_ms))
            .unwrap_or_else(Instant::now);
        loop {
            let cursor = recorder.media_cursor();
            let tap_url = append_query(
                &config.media_switch.tap_url,
                &format!("after={cursor}&limit={}", config.media_switch.batch_limit),
            );
            match fetch_json::<RecorderTapBatch>(&tap_url, &config) {
                Ok(batch) => {
                    if batch
                        .newest_available_seq
                        .is_some_and(|newest| newest < cursor)
                    {
                        recorder.media_sequence_reset(newest_or_zero(&batch));
                    } else {
                        recorder.media_switch_connected();
                        if let Err(error) = recorder.ingest_batch(batch) {
                            recorder.record_runtime_error(error);
                        }
                    }
                }
                Err(error) => recorder.media_switch_failed(error),
            }

            if last_session_reconcile.elapsed()
                >= Duration::from_millis(config.media_switch.session_reconcile_ms)
            {
                match fetch_json::<Vec<MediaSwitchSession>>(
                    &config.media_switch.sessions_url,
                    &config,
                ) {
                    Ok(sessions) => recorder.reconcile_sessions(sessions),
                    Err(error) => recorder.media_switch_failed(error),
                }
                last_session_reconcile = Instant::now();
            }

            thread::sleep(Duration::from_millis(
                config.media_switch.poll_interval_ms,
            ));
        }
    })
}

fn newest_or_zero(batch: &RecorderTapBatch) -> u64 {
    batch.newest_available_seq.unwrap_or(0)
}

fn append_query(url: &str, query: &str) -> String {
    if url.contains('?') {
        format!("{url}&{query}")
    } else {
        format!("{url}?{query}")
    }
}

fn fetch_json<T: DeserializeOwned>(url: &str, config: &RecorderConfig) -> Result<T, String> {
    let parsed = ParsedHttpUrl::parse(url)?;
    let timeout = Duration::from_secs(config.media_switch.request_timeout_secs);
    let address = (parsed.host.as_str(), parsed.port)
        .to_socket_addrs()
        .map_err(|error| format!("Media Switch DNS failed: {error}"))?
        .next()
        .ok_or_else(|| "Media Switch address did not resolve".to_string())?;
    let mut stream = TcpStream::connect_timeout(&address, timeout)
        .map_err(|error| format!("Media Switch connection failed: {error}"))?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|error| format!("Media Switch read timeout failed: {error}"))?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|error| format!("Media Switch write timeout failed: {error}"))?;

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
        .map_err(|error| format!("Media Switch request failed: {error}"))?;

    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|error| format!("Media Switch response failed: {error}"))?;
    let header_end = find_subslice(&response, b"\r\n\r\n")
        .ok_or_else(|| "Media Switch returned an invalid HTTP response".to_string())?;
    let header = std::str::from_utf8(&response[..header_end])
        .map_err(|_| "Media Switch response headers are not UTF-8".to_string())?;
    let status = header
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| "Media Switch response has no valid status".to_string())?;
    if status != 200 {
        return Err(format!("Media Switch returned HTTP {status}"));
    }
    serde_json::from_slice(&response[header_end + 4..])
        .map_err(|error| format!("Media Switch JSON failed: {error}"))
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
            .ok_or_else(|| "Media Switch URL must start with http://".to_string())?;
        let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
        let (host, port) = authority
            .rsplit_once(':')
            .map(|(host, port)| {
                port.parse::<u16>()
                    .map(|port| (host.to_string(), port))
                    .map_err(|_| "invalid Media Switch port".to_string())
            })
            .transpose()?
            .unwrap_or_else(|| (authority.to_string(), 80));
        if host.trim().is_empty() {
            return Err("Media Switch host must not be empty".to_string());
        }
        Ok(Self {
            host,
            port,
            path: format!("/{path}"),
        })
    }
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::{ParsedHttpUrl, append_query};

    #[test]
    fn parses_open_lab_media_switch_url() {
        let url = ParsedHttpUrl::parse(
            "http://127.0.0.1:8130/api/v1/recorder/taps?after=4&limit=50",
        )
        .expect("URL parses");
        assert_eq!(url.host, "127.0.0.1");
        assert_eq!(url.port, 8130);
        assert_eq!(
            url.path,
            "/api/v1/recorder/taps?after=4&limit=50"
        );
    }

    #[test]
    fn query_is_appended_without_destroying_existing_query() {
        assert_eq!(
            append_query("http://x/a?foo=bar", "after=7"),
            "http://x/a?foo=bar&after=7"
        );
    }
}

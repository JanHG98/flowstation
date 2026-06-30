use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};
use tetra_entities::net_control::ControlCommand;
use tetra_entities::net_control_room::ControlCommandEnvelope;

use crate::state::{SharedControlRoom, now_iso};

const MAX_HTTP_REQUEST_BYTES: usize = 1024 * 1024;

#[derive(Debug)]
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

#[derive(Debug)]
pub struct HttpResponse {
    pub status: u16,
    pub reason: &'static str,
    pub content_type: &'static str,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn json<T: serde::Serialize>(status: u16, value: &T) -> Self {
        let reason = reason_phrase(status);
        let body = serde_json::to_vec_pretty(value).unwrap_or_else(|_| b"{\"error\":\"json serialisation failed\"}".to_vec());
        Self {
            status,
            reason,
            content_type: "application/json; charset=utf-8",
            body,
        }
    }

    pub fn text(status: u16, text: impl Into<String>) -> Self {
        Self {
            status,
            reason: reason_phrase(status),
            content_type: "text/plain; charset=utf-8",
            body: text.into().into_bytes(),
        }
    }

    pub fn html(status: u16, html: impl Into<String>) -> Self {
        Self {
            status,
            reason: reason_phrase(status),
            content_type: "text/html; charset=utf-8",
            body: html.into().into_bytes(),
        }
    }
}

pub fn handle_http_stream(mut stream: TcpStream, state: SharedControlRoom, node_path: &str, ui_path: &str) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let request = match read_http_request(&mut stream) {
        Ok(req) => req,
        Err(err) => {
            let _ = write_response(&mut stream, &HttpResponse::text(400, format!("bad request: {}\n", err)));
            return;
        }
    };

    tracing::debug!(method = %request.method, path = %request.path, "http request");
    let response = route_http(request, state, node_path, ui_path);
    let _ = write_response(&mut stream, &response);
}

fn route_http(request: HttpRequest, state: SharedControlRoom, node_path: &str, ui_path: &str) -> HttpResponse {
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") => HttpResponse::html(200, index_html(node_path, ui_path)),
        ("GET", "/health") => HttpResponse::json(200, &json!({
            "ok": true,
            "service": "netcore-control-room",
            "timestamp": now_iso(),
        })),
        ("GET", "/api/state") => HttpResponse::json(200, &state.snapshot()),
        ("GET", "/api/nodes") => {
            let snapshot = state.snapshot();
            HttpResponse::json(200, &snapshot.nodes)
        }
        ("GET", "/api/events") => HttpResponse::json(200, &state.recent_events(200)),
        ("POST", "/api/commands") => submit_command_from_body(&request.body, state, None),
        _ if request.method == "POST" && request.path.starts_with("/api/nodes/") && request.path.ends_with("/commands") => {
            let node_id = request
                .path
                .trim_start_matches("/api/nodes/")
                .trim_end_matches("/commands")
                .trim_matches('/')
                .to_string();
            if node_id.is_empty() {
                HttpResponse::json(400, &json!({ "error": "missing node id" }))
            } else {
                submit_command_from_body(&request.body, state, Some(node_id))
            }
        }
        _ => HttpResponse::json(
            404,
            &json!({
                "error": "not found",
                "available": [
                    "GET /",
                    "GET /health",
                    "GET /api/state",
                    "GET /api/nodes",
                    "GET /api/events",
                    "POST /api/commands",
                    "POST /api/nodes/{node_id}/commands",
                    format!("WS {}", node_path),
                    format!("WS {}", ui_path)
                ]
            }),
        ),
    }
}

fn submit_command_from_body(body: &[u8], state: SharedControlRoom, node_from_path: Option<String>) -> HttpResponse {
    let value: Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(e) => return HttpResponse::json(400, &json!({ "error": format!("invalid json: {}", e) })),
    };

    let envelope = match parse_command_request(value, &state, node_from_path) {
        Ok(envelope) => envelope,
        Err(e) => return HttpResponse::json(400, &json!({ "error": e })),
    };

    match state.submit_command(envelope) {
        Ok(queued) => HttpResponse::json(202, &queued),
        Err(e) => HttpResponse::json(409, &json!({ "error": e })),
    }
}

fn parse_command_request(value: Value, state: &SharedControlRoom, node_from_path: Option<String>) -> Result<ControlCommandEnvelope, String> {
    if value.get("target_node_id").is_some() && value.get("command_id").is_some() && value.get("command").is_some() {
        let mut envelope: ControlCommandEnvelope = serde_json::from_value(value).map_err(|e| format!("invalid ControlCommandEnvelope: {}", e))?;
        if let Some(node_id) = node_from_path {
            envelope.target_node_id = node_id;
        }
        return Ok(envelope);
    }

    #[derive(Debug, Deserialize)]
    struct SubmitCommandRequest {
        operator_id: Option<String>,
        command_id: Option<String>,
        issued_at: Option<String>,
        command: ControlCommand,
    }

    let req: SubmitCommandRequest = serde_json::from_value(value).map_err(|e| format!("invalid command request: {}", e))?;
    let target_node_id = node_from_path.ok_or_else(|| {
        "target_node_id is required here; use POST /api/nodes/{node_id}/commands or send a full ControlCommandEnvelope".to_string()
    })?;

    let mut envelope = state.make_envelope(target_node_id, req.operator_id, req.command);
    if let Some(command_id) = req.command_id {
        envelope.command_id = command_id;
    }
    if let Some(issued_at) = req.issued_at {
        envelope.issued_at = issued_at;
    }
    Ok(envelope)
}

pub fn read_http_request(stream: &mut TcpStream) -> Result<HttpRequest, String> {
    let mut buf = Vec::with_capacity(4096);
    let mut chunk = [0u8; 4096];
    let mut header_end = None;

    while header_end.is_none() {
        let n = stream.read(&mut chunk).map_err(|e| format!("read failed: {}", e))?;
        if n == 0 {
            return Err("connection closed before headers".to_string());
        }
        buf.extend_from_slice(&chunk[..n]);
        header_end = find_subslice(&buf, b"\r\n\r\n");
        if buf.len() > MAX_HTTP_REQUEST_BYTES {
            return Err("request too large".to_string());
        }
    }

    let header_end = header_end.expect("header_end checked") + 4;
    let header_text = String::from_utf8_lossy(&buf[..header_end]);
    let mut lines = header_text.lines();
    let request_line = lines.next().ok_or_else(|| "missing request line".to_string())?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().ok_or_else(|| "missing method".to_string())?.to_string();
    let raw_path = parts.next().ok_or_else(|| "missing path".to_string())?;
    let path = raw_path.split('?').next().unwrap_or(raw_path).to_string();

    let mut headers = HashMap::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }

    let content_length = headers
        .get("content-length")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);
    if content_length > MAX_HTTP_REQUEST_BYTES {
        return Err("body too large".to_string());
    }

    let mut body = buf[header_end..].to_vec();
    while body.len() < content_length {
        let n = stream.read(&mut chunk).map_err(|e| format!("body read failed: {}", e))?;
        if n == 0 {
            return Err("connection closed before body complete".to_string());
        }
        body.extend_from_slice(&chunk[..n]);
        if body.len() > MAX_HTTP_REQUEST_BYTES {
            return Err("body too large".to_string());
        }
    }
    body.truncate(content_length);

    Ok(HttpRequest { method, path, headers, body })
}

pub fn write_response(stream: &mut TcpStream, response: &HttpResponse) -> std::io::Result<()> {
    let header = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
        response.status,
        response.reason,
        response.content_type,
        response.body.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(&response.body)?;
    stream.flush()
}

pub fn looks_like_websocket_upgrade(peek: &[u8]) -> bool {
    let text = String::from_utf8_lossy(peek).to_ascii_lowercase();
    text.contains("upgrade: websocket") && text.contains("sec-websocket-key:")
}

pub fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    hay.windows(needle.len()).position(|w| w == needle)
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        404 => "Not Found",
        409 => "Conflict",
        500 => "Internal Server Error",
        _ => "OK",
    }
}

fn index_html(node_path: &str, ui_path: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="de">
<head>
  <meta charset="utf-8">
  <title>NetCore Control Room</title>
  <style>
    body {{ font-family: system-ui, sans-serif; margin: 2rem; max-width: 980px; }}
    code, pre {{ background: #f3f3f3; padding: .2rem .35rem; border-radius: .35rem; }}
    pre {{ padding: 1rem; overflow: auto; }}
    .ok {{ color: #087f23; font-weight: 700; }}
  </style>
</head>
<body>
  <h1>NetCore Control Room Core</h1>
  <p class="ok">Server läuft.</p>
  <p>Base-Station WebSocket: <code>{node_path}</code></p>
  <p>UI/Event WebSocket: <code>{ui_path}</code></p>
  <h2>HTTP API</h2>
  <ul>
    <li><code>GET /health</code></li>
    <li><code>GET /api/state</code></li>
    <li><code>GET /api/nodes</code></li>
    <li><code>GET /api/events</code></li>
    <li><code>POST /api/nodes/&lt;node_id&gt;/commands</code></li>
  </ul>
  <h2>Beispiel: Kick MS</h2>
  <pre>curl -X POST http://127.0.0.1:9010/api/nodes/tbs-04010001/commands \
  -H 'Content-Type: application/json' \
  -d '{{"operator_id":"jan","command":{{"KickMs":{{"issi":2010001}}}}}}'</pre>
</body>
</html>"#
    )
}

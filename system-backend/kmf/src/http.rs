use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

use serde::Serialize;
use serde_json::json;

use crate::config::KmfConfig;
use crate::protocol::{
    BackupInput, EdgeActionAckInput, EdgeClaimInput, KeyCreateInput, KeyRotateInput,
    LifecycleInput, NodeCreateInput, NodeStateInput, OtarApprovalInput, OtarJobCreateInput,
    OtarQueueInput, PolicyInput,
};
use crate::state::SharedKmf;

pub fn spawn_http_server(
    config: KmfConfig,
    kmf: SharedKmf,
) -> std::io::Result<thread::JoinHandle<()>> {
    let listener = TcpListener::bind(config.server.bind)?;
    tracing::info!("KMF WebUI/API listening on http://{}", config.server.bind);
    Ok(thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let kmf = kmf.clone();
                    let config = config.clone();
                    thread::spawn(move || {
                        if let Err(error) = handle_connection(stream, kmf, config) {
                            tracing::warn!("KMF HTTP connection failed: {}", error);
                        }
                    });
                }
                Err(error) => tracing::warn!("KMF HTTP accept failed: {}", error),
            }
        }
    }))
}

struct HttpRequest {
    method: String,
    path: String,
    query: HashMap<String, String>,
    body: Vec<u8>,
}

struct HttpResponse {
    status: u16,
    content_type: &'static str,
    body: Vec<u8>,
    disposition: Option<String>,
}

fn handle_connection(
    mut stream: TcpStream,
    kmf: SharedKmf,
    config: KmfConfig,
) -> Result<(), String> {
    let request = read_request(&mut stream, config.limits.max_body_bytes)?;
    let response = route(request, kmf);
    write_response(&mut stream, response).map_err(|error| error.to_string())
}

fn route(request: HttpRequest, kmf: SharedKmf) -> HttpResponse {
    if request.method == "OPTIONS" {
        return empty(204);
    }
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") => html(INDEX_HTML),
        ("GET", "/health/live") => json_response(200, &json!({"status":"live"})),
        ("GET", "/health/ready") => {
            let status = kmf.status();
            let ready = status.vault_ready;
            json_response(if ready { 200 } else { 503 }, &status)
        }
        ("GET", "/api/v1/status") => json_response(200, &kmf.status()),
        ("GET", "/api/v1/config") => json_response(200, &kmf.redacted_config()),
        ("GET", "/api/v1/policy") => json_response(200, &kmf.policy()),
        ("POST", "/api/v1/policy") => match parse_json::<PolicyInput>(&request.body)
            .and_then(|input| kmf.update_policy(input, "open-lab-api"))
        {
            Ok(policy) => json_response(200, &policy),
            Err(error) => json_response(409, &json!({"error":error})),
        },
        ("GET", "/api/v1/keys") => json_response(200, &kmf.keys()),
        ("POST", "/api/v1/keys") => match parse_json::<KeyCreateInput>(&request.body)
            .and_then(|input| kmf.create_key(input, "open-lab-api"))
        {
            Ok(key) => json_response(201, &key),
            Err(error) => json_response(409, &json!({"error":error})),
        },
        ("GET", "/api/v1/nodes") => json_response(200, &kmf.nodes()),
        ("POST", "/api/v1/nodes") => match parse_json::<NodeCreateInput>(&request.body)
            .and_then(|input| kmf.create_node(input))
        {
            Ok(node) => json_response(201, &node),
            Err(error) => json_response(409, &json!({"error":error})),
        },
        ("GET", "/api/v1/otar/jobs") => json_response(200, &kmf.jobs()),
        ("POST", "/api/v1/otar/jobs") => {
            match parse_json::<OtarJobCreateInput>(&request.body)
                .and_then(|input| kmf.create_job(input))
            {
                Ok(job) => json_response(201, &job),
                Err(error) => json_response(409, &json!({"error":error})),
            }
        }
        ("GET", "/api/v1/otar/actions") => json_response(200, &kmf.actions()),
        ("POST", "/api/v1/edge/actions/claim") => {
            match parse_json::<EdgeClaimInput>(&request.body)
                .and_then(|input| kmf.claim_actions(input))
            {
                Ok(actions) => json_response(200, &json!({
                    "protocol_version":crate::protocol::OTAR_EDGE_PROTOCOL_VERSION,
                    "actions":actions,
                    "raw_keys_returned":false,
                    "warning":"This edge-only endpoint returns key material only as a node-bound sealed envelope. It must never be proxied to an operator browser."
                })),
                Err(error) => json_response(409, &json!({"error":error})),
            }
        }
        ("GET", "/api/v1/audit") => {
            let limit = query_usize(&request, "limit", 500, 10_000);
            json_response(200, &kmf.audit(limit))
        }
        ("GET", "/api/v1/backups") => json_response(200, &kmf.backups()),
        ("POST", "/api/v1/backups") => match parse_json_or_default::<BackupInput>(&request.body)
            .and_then(|input| kmf.create_backup(input))
        {
            Ok(backup) => json_response(201, &backup),
            Err(error) => json_response(500, &json!({"error":error})),
        },
        ("POST", "/api/v1/maintenance/tick") => match kmf.maintenance_tick() {
            Ok(status) => json_response(200, &status),
            Err(error) => json_response(500, &json!({"error":error})),
        },
        ("GET", "/api/v1/export.json") => {
            download_json("netcore-kmf-redacted-export.json", &kmf.export())
        }
        ("GET", "/metrics") => text(
            "text/plain; version=0.0.4; charset=utf-8",
            kmf.metrics(),
        ),
        ("GET", "/openapi.json") => json_response(200, &openapi()),
        _ => dynamic_route(request, kmf),
    }
}

fn dynamic_route(request: HttpRequest, kmf: SharedKmf) -> HttpResponse {
    let parts = request
        .path
        .trim_matches('/')
        .split('/')
        .collect::<Vec<_>>();
    match (request.method.as_str(), parts.as_slice()) {
        ("GET", ["api", "v1", "keys", id]) => match kmf.key(id) {
            Some(key) => json_response(200, &key),
            None => json_response(404, &json!({"error":"key not found"})),
        },
        ("POST", ["api", "v1", "keys", id, "stage"]) => {
            lifecycle_route(&request.body, |input| kmf.stage_key(id, input))
        }
        ("POST", ["api", "v1", "keys", id, "activate"]) => {
            lifecycle_route(&request.body, |input| kmf.activate_key(id, input))
        }
        ("POST", ["api", "v1", "keys", id, "retire"]) => {
            lifecycle_route(&request.body, |input| kmf.retire_key(id, input))
        }
        ("POST", ["api", "v1", "keys", id, "revoke"]) => {
            lifecycle_route(&request.body, |input| kmf.revoke_key(id, input))
        }
        ("POST", ["api", "v1", "keys", id, "destroy"]) => {
            lifecycle_route(&request.body, |input| kmf.destroy_key(id, input))
        }
        ("POST", ["api", "v1", "keys", id, "rotate"]) => {
            match parse_json_or_default::<KeyRotateInput>(&request.body)
                .and_then(|input| kmf.rotate_key(id, input))
            {
                Ok(key) => json_response(201, &key),
                Err(error) => json_response(409, &json!({"error":error})),
            }
        }
        ("POST", ["api", "v1", "nodes", node_id, "enable"]) => {
            node_state_route(&request.body, |input| {
                kmf.set_node_enabled(node_id, true, input)
            })
        }
        ("POST", ["api", "v1", "nodes", node_id, "disable"]) => {
            node_state_route(&request.body, |input| {
                kmf.set_node_enabled(node_id, false, input)
            })
        }
        ("GET", ["api", "v1", "otar", "jobs", id]) => match kmf.job(id) {
            Some(job) => json_response(200, &job),
            None => json_response(404, &json!({"error":"OTAR job not found"})),
        },
        ("POST", ["api", "v1", "otar", "jobs", id, "approve"]) => {
            match parse_json::<OtarApprovalInput>(&request.body)
                .and_then(|input| kmf.approve_job(id, input))
            {
                Ok(job) => json_response(200, &job),
                Err(error) => json_response(409, &json!({"error":error})),
            }
        }
        ("POST", ["api", "v1", "otar", "jobs", id, "queue"]) => {
            match parse_json_or_default::<OtarQueueInput>(&request.body)
                .and_then(|input| kmf.queue_job(id, input))
            {
                Ok(job) => json_response(202, &job),
                Err(error) => json_response(409, &json!({"error":error})),
            }
        }
        ("POST", ["api", "v1", "otar", "jobs", id, "cancel"]) => {
            lifecycle_route(&request.body, |input| kmf.cancel_job(id, input))
        }
        ("POST", ["api", "v1", "edge", "actions", id, "ack"]) => {
            match parse_json_or_default::<EdgeActionAckInput>(&request.body)
                .and_then(|input| kmf.acknowledge_action(id, input))
            {
                Ok(action) => json_response(200, &action),
                Err(error) => json_response(409, &json!({"error":error})),
            }
        }
        _ => json_response(404, &json!({"error":"not found"})),
    }
}

fn lifecycle_route<F, T>(body: &[u8], action: F) -> HttpResponse
where
    F: FnOnce(LifecycleInput) -> Result<T, String>,
    T: Serialize,
{
    match parse_json_or_default::<LifecycleInput>(body).and_then(action) {
        Ok(value) => json_response(200, &value),
        Err(error) => json_response(409, &json!({"error":error})),
    }
}

fn node_state_route<F, T>(body: &[u8], action: F) -> HttpResponse
where
    F: FnOnce(NodeStateInput) -> Result<T, String>,
    T: Serialize,
{
    match parse_json_or_default::<NodeStateInput>(body).and_then(action) {
        Ok(value) => json_response(200, &value),
        Err(error) => json_response(409, &json!({"error":error})),
    }
}

fn parse_json<T: serde::de::DeserializeOwned>(body: &[u8]) -> Result<T, String> {
    serde_json::from_slice(body).map_err(|error| format!("invalid JSON: {error}"))
}

fn parse_json_or_default<T>(body: &[u8]) -> Result<T, String>
where
    T: serde::de::DeserializeOwned + Default,
{
    if body.is_empty() {
        Ok(T::default())
    } else {
        parse_json(body)
    }
}

fn query_usize(request: &HttpRequest, key: &str, default: usize, maximum: usize) -> usize {
    request
        .query
        .get(key)
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(default)
        .min(maximum)
}

fn openapi() -> serde_json::Value {
    json!({
        "openapi":"3.0.3",
        "info":{
            "title":"NetCore KMF",
            "version":env!("CARGO_PKG_VERSION"),
            "description":"OPEN LAB CCK/GCK/SCK lifecycle, crypto-period, rotation, node-bound sealed OTAR envelope, backup and tamper-evident audit API. Normal management responses never contain raw key material."
        },
        "paths":{
            "/api/v1/status":{"get":{}},
            "/api/v1/config":{"get":{}},
            "/api/v1/policy":{"get":{},"post":{}},
            "/api/v1/keys":{"get":{},"post":{}},
            "/api/v1/keys/{id}":{"get":{}},
            "/api/v1/keys/{id}/stage":{"post":{}},
            "/api/v1/keys/{id}/activate":{"post":{}},
            "/api/v1/keys/{id}/retire":{"post":{}},
            "/api/v1/keys/{id}/revoke":{"post":{}},
            "/api/v1/keys/{id}/destroy":{"post":{}},
            "/api/v1/keys/{id}/rotate":{"post":{}},
            "/api/v1/nodes":{"get":{},"post":{}},
            "/api/v1/nodes/{node_id}/enable":{"post":{}},
            "/api/v1/nodes/{node_id}/disable":{"post":{}},
            "/api/v1/otar/jobs":{"get":{},"post":{}},
            "/api/v1/otar/jobs/{id}":{"get":{}},
            "/api/v1/otar/jobs/{id}/approve":{"post":{}},
            "/api/v1/otar/jobs/{id}/queue":{"post":{}},
            "/api/v1/otar/jobs/{id}/cancel":{"post":{}},
            "/api/v1/otar/actions":{"get":{}},
            "/api/v1/edge/actions/claim":{"post":{}},
            "/api/v1/edge/actions/{id}/ack":{"post":{}},
            "/api/v1/audit":{"get":{}},
            "/api/v1/backups":{"get":{},"post":{}},
            "/api/v1/maintenance/tick":{"post":{}},
            "/api/v1/export.json":{"get":{}},
            "/health/live":{"get":{}},
            "/health/ready":{"get":{}},
            "/metrics":{"get":{}}
        }
    })
}

fn json_response<T: Serialize>(status: u16, value: &T) -> HttpResponse {
    match serde_json::to_vec_pretty(value) {
        Ok(body) => HttpResponse {
            status,
            content_type: "application/json; charset=utf-8",
            body,
            disposition: None,
        },
        Err(error) => HttpResponse {
            status: 500,
            content_type: "application/json; charset=utf-8",
            body: format!("{{\"error\":\"serialization failed: {error}\"}}")
                .into_bytes(),
            disposition: None,
        },
    }
}

fn download_json<T: Serialize>(filename: &str, value: &T) -> HttpResponse {
    match serde_json::to_vec_pretty(value) {
        Ok(body) => HttpResponse {
            status: 200,
            content_type: "application/json; charset=utf-8",
            body,
            disposition: Some(format!("attachment; filename=\"{filename}\"")),
        },
        Err(error) => json_response(500, &json!({"error":error.to_string()})),
    }
}

fn html(body: &'static str) -> HttpResponse {
    HttpResponse {
        status: 200,
        content_type: "text/html; charset=utf-8",
        body: body.as_bytes().to_vec(),
        disposition: None,
    }
}

fn text(content_type: &'static str, body: String) -> HttpResponse {
    HttpResponse {
        status: 200,
        content_type,
        body: body.into_bytes(),
        disposition: None,
    }
}

fn empty(status: u16) -> HttpResponse {
    HttpResponse {
        status,
        content_type: "text/plain; charset=utf-8",
        body: Vec::new(),
        disposition: None,
    }
}

fn read_request(stream: &mut TcpStream, max_body_bytes: usize) -> Result<HttpRequest, String> {
    let mut buffer = Vec::with_capacity(8_192);
    let mut temporary = [0_u8; 4_096];
    let header_end;
    loop {
        let count = stream
            .read(&mut temporary)
            .map_err(|error| format!("read request: {error}"))?;
        if count == 0 {
            return Err("connection closed before request was complete".to_string());
        }
        buffer.extend_from_slice(&temporary[..count]);
        if let Some(position) = find_bytes(&buffer, b"\r\n\r\n") {
            header_end = position + 4;
            break;
        }
        if buffer.len() > 65_536 {
            return Err("request header is too large".to_string());
        }
    }

    let header_text = std::str::from_utf8(&buffer[..header_end])
        .map_err(|error| format!("request header is not UTF-8: {error}"))?;
    let mut lines = header_text.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| "missing request line".to_string())?;
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| "missing request method".to_string())?
        .to_string();
    let target = parts
        .next()
        .ok_or_else(|| "missing request target".to_string())?
        .to_string();
    let content_length = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .map(|(_, value)| value.trim().parse::<usize>())
        .transpose()
        .map_err(|error| format!("invalid Content-Length: {error}"))?
        .unwrap_or(0);
    if content_length > max_body_bytes {
        return Err(format!("request body exceeds {max_body_bytes} bytes"));
    }
    while buffer.len() < header_end + content_length {
        let count = stream
            .read(&mut temporary)
            .map_err(|error| format!("read request body: {error}"))?;
        if count == 0 {
            return Err("connection closed before request body was complete".to_string());
        }
        buffer.extend_from_slice(&temporary[..count]);
    }
    let body = buffer[header_end..header_end + content_length].to_vec();
    let (path, query) = split_target(&target);
    Ok(HttpRequest {
        method,
        path,
        query,
        body,
    })
}

fn split_target(target: &str) -> (String, HashMap<String, String>) {
    let (path, raw_query) = target.split_once('?').unwrap_or((target, ""));
    let mut query = HashMap::new();
    for pair in raw_query.split('&').filter(|value| !value.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        query.insert(percent_decode(key), percent_decode(value));
    }
    (percent_decode(path), query)
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() => {
                let high = hex_value(bytes[index + 1]);
                let low = hex_value(bytes[index + 2]);
                if let (Some(high), Some(low)) = (high, low) {
                    output.push((high << 4) | low);
                    index += 3;
                    continue;
                }
                output.push(bytes[index]);
                index += 1;
            }
            b'+' => {
                output.push(b' ');
                index += 1;
            }
            other => {
                output.push(other);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&output).into_owned()
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn write_response(stream: &mut TcpStream, response: HttpResponse) -> std::io::Result<()> {
    let reason = match response.status {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        409 => "Conflict",
        413 => "Payload Too Large",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Response",
    };
    let mut headers = format!(
        concat!(
            "HTTP/1.1 {} {}\r\n",
            "Content-Type: {}\r\n",
            "Content-Length: {}\r\n",
            "Access-Control-Allow-Origin: *\r\n",
            "Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n",
            "Access-Control-Allow-Headers: Content-Type\r\n",
            "X-NetCore-Security-Mode: open-lab\r\n",
            "X-NetCore-Raw-Key-Exposure: disabled\r\n",
            "X-Content-Type-Options: nosniff\r\n",
            "Content-Security-Policy: default-src 'self'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; connect-src 'self'\r\n"
        ),
        response.status,
        reason,
        response.content_type,
        response.body.len()
    );
    if let Some(disposition) = response.disposition {
        headers.push_str(&format!("Content-Disposition: {disposition}\r\n"));
    }
    headers.push_str("Connection: close\r\n\r\n");
    stream.write_all(headers.as_bytes())?;
    stream.write_all(&response.body)?;
    stream.flush()
}

const INDEX_HTML: &str = r#"<!doctype html>
<html lang="de">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>NetCore KMF</title>
<style>
:root{font-family:Inter,system-ui,sans-serif;color:#e8edf7;background:#0b1020;--card:#121a2e;--muted:#91a0ba;--accent:#6aa7ff;--danger:#ff6b7a;--ok:#48d597;--warn:#ffcc66;--line:#27324b}
*{box-sizing:border-box}body{margin:0}.banner{background:#7c2d12;color:#fff;padding:10px 18px;font-weight:800;text-align:center}.layout{display:grid;grid-template-columns:245px 1fr;min-height:calc(100vh - 44px)}aside{background:#0d1425;border-right:1px solid var(--line);padding:22px 14px;position:sticky;top:0;height:calc(100vh - 44px)}h1{font-size:20px;margin:0 0 4px}.sub{color:var(--muted);font-size:12px;margin-bottom:20px}nav button{width:100%;display:block;text-align:left;background:transparent;border:0;color:#dbe5f6;padding:11px 12px;border-radius:9px;margin:3px 0;cursor:pointer}nav button.active,nav button:hover{background:#1a2742;color:#fff}main{padding:24px;min-width:0}.page{display:none}.page.active{display:block}.grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(190px,1fr));gap:14px}.card{background:var(--card);border:1px solid var(--line);border-radius:14px;padding:16px;box-shadow:0 8px 28px #0003;margin-bottom:14px}.metric{font-size:30px;font-weight:800;margin-top:6px}.muted{color:var(--muted)}.ok{color:var(--ok)}.danger{color:var(--danger)}.warn{color:var(--warn)}table{width:100%;border-collapse:collapse;font-size:13px}th,td{padding:10px 8px;border-bottom:1px solid var(--line);vertical-align:top;text-align:left}th{color:#b8c5db;font-weight:700;position:sticky;top:0;background:var(--card)}.scroll{overflow:auto;max-height:65vh}.toolbar{display:flex;gap:8px;flex-wrap:wrap;margin:12px 0}button.action{background:#1e66d0;color:white;border:0;border-radius:8px;padding:9px 12px;cursor:pointer}button.danger{background:#b42335;color:white}button.secondary{background:#263553;color:white}.pill{display:inline-block;border-radius:999px;padding:3px 8px;background:#263553;font-size:11px}.pill.ok{background:#123f31}.pill.warn{background:#4a3814}.pill.danger{background:#4b1921}input,select,textarea{width:100%;background:#0b1324;color:#eef4ff;border:1px solid #34415d;border-radius:8px;padding:9px}label{display:block;font-size:12px;color:#b8c5db;margin:8px 0 4px}.formgrid{display:grid;grid-template-columns:repeat(auto-fit,minmax(170px,1fr));gap:10px}.mono{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;word-break:break-all}pre{background:#080d18;padding:14px;border-radius:10px;overflow:auto;white-space:pre-wrap}.notice{border-left:4px solid var(--warn);background:#352b16;padding:12px;border-radius:8px;margin:12px 0}.secret{border-left-color:var(--danger);background:#32181e}a{color:var(--accent)}@media(max-width:850px){.layout{grid-template-columns:1fr}aside{position:static;height:auto}nav{display:flex;overflow:auto}nav button{min-width:max-content}main{padding:14px}}
</style>
</head>
<body>
<div class="banner">OPEN LAB – keine Anmeldung, keine Tokens, kein TLS. KMF ausschließlich im isolierten Managementnetz betreiben.</div>
<div class="layout">
<aside><h1>Key Management Facility</h1><div class="sub">CCK • GCK • SCK • Rotation • OTAR</div><nav id="nav"></nav></aside>
<main>
<section id="overview" class="page active"><h2>Übersicht</h2><div class="grid" id="metrics"></div><div class="notice secret"><b>Schlüssel bleiben hinter Glas:</b> WebUI, Management-API, Audit und Export enthalten niemals Rohschlüssel. OTAR-Aktionen liefern ausschließlich einen an das Ziel-Node gebundenen, versiegelten Envelope.</div><div class="card"><h3>Status</h3><pre id="statusJson"></pre></div></section>
<section id="keys" class="page"><h2>Schlüssel</h2><div class="card"><h3>CCK/GCK/SCK generieren</h3><div class="formgrid"><div><label>Typ</label><select id="kKind"><option>CCK</option><option>GCK</option><option>SCK</option></select></div><div><label>Scope</label><select id="kScope"><option value="network">network</option><option value="location_area">location_area</option><option value="group">group</option><option value="subscriber">subscriber</option></select></div><div><label>Scope-Wert</label><input id="kScopeValue" placeholder="z. B. 15501 oder LA-1"></div><div><label>Label</label><input id="kLabel" value="NetCore Test Key"></div><div><label>Key-Bytes</label><input id="kBytes" type="number" value="16" min="8" max="32"></div><div><label>Crypto Period Start (RFC3339, leer=jetzt)</label><input id="kStart"></div><div><label>Crypto Period End (RFC3339, leer=Default)</label><input id="kEnd"></div><div><label>Notiz</label><input id="kNotes"></div></div><div class="toolbar"><button class="action" onclick="createKey()">Schlüssel generieren</button></div></div><div class="card"><div class="scroll"><table><thead><tr><th>Typ / Scope</th><th>Label</th><th>Version</th><th>Zustand</th><th>Fingerprint</th><th>Crypto Period</th><th>Aktionen</th></tr></thead><tbody id="keyRows"></tbody></table></div></div></section>
<section id="nodes" class="page"><h2>Node-Transportprofile</h2><div class="card"><div class="formgrid"><div><label>Node-ID</label><input id="nId" placeholder="tbs-04010001"></div><div><label>Anzeigename</label><input id="nName" value="TBS Edge"></div><div><label>Notiz</label><input id="nNotes"></div></div><div class="toolbar"><button class="action" onclick="createNode()">Transportprofil + Bootstrap erzeugen</button></div><div class="notice">Das Bootstrap-Geheimnis wird ausschließlich als Datei mit Modus 0600 auf dem KMF-Host erzeugt. Die API zeigt nur Pfad und Fingerprint. Datei offline zum passenden Edge kopieren und danach löschen.</div></div><div class="card"><div class="scroll"><table><thead><tr><th>Node</th><th>Zustand</th><th>Fingerprint</th><th>Bootstrap-Pfad</th><th>Letzter Claim/ACK</th><th>Aktion</th></tr></thead><tbody id="nodeRows"></tbody></table></div></div></section>
<section id="otar" class="page"><h2>OTAR-Aufträge</h2><div class="card"><div class="formgrid"><div><label>Key-ID</label><input id="oKey" placeholder="UUID"></div><div><label>Ziel-Nodes, Komma</label><input id="oNodes" placeholder="tbs-04010001"></div><div><label>ISSI, Komma</label><input id="oIssi"></div><div><label>GSSI, Komma</label><input id="oGssi"></div><div><label>Ablauf RFC3339, leer=Crypto Period End</label><input id="oExpires"></div><div><label>Notiz</label><input id="oNotes"></div></div><div class="toolbar"><button class="action" onclick="createJob()">OTAR-Auftrag anlegen</button></div><div class="notice">Vier-Augen-Freigabe wird bereits erzwungen. Im Open-Lab-Modus sind die Actor-Namen allerdings nur deklarativ, weil noch keine Anmeldung existiert.</div></div><div class="card"><div class="scroll"><table><thead><tr><th>Job</th><th>Key</th><th>Ziele</th><th>Freigaben</th><th>Zustand</th><th>Delivery</th><th>Aktionen</th></tr></thead><tbody id="jobRows"></tbody></table></div></div></section>
<section id="actions" class="page"><h2>OTAR-Aktionen</h2><div class="card"><div class="scroll"><table><thead><tr><th>Seq</th><th>Node</th><th>Job / Key</th><th>Zustand</th><th>Versuche</th><th>Nächster Versuch</th><th>Fehler</th></tr></thead><tbody id="actionRows"></tbody></table></div></div></section>
<section id="policy" class="page"><h2>Policy</h2><div class="card"><div class="formgrid"><div><label>Betriebsmodus</label><select id="pMode"><option value="shadow">shadow</option><option value="authoritative">authoritative</option></select></div><div><label>Standard-Key-Bytes</label><input id="pBytes" type="number"></div><div><label>Standard-Crypto-Period Sekunden</label><input id="pPeriod" type="number"></div><div><label>Rotation Lead Sekunden</label><input id="pLead" type="number"></div><div><label>Vier Augen</label><select id="pDual"><option value="true">ja</option><option value="false">nein</option></select></div><div><label>Überlappung erlauben</label><select id="pOverlap"><option value="true">ja</option><option value="false">nein</option></select></div><div><label>Vorgänger automatisch stilllegen</label><select id="pRetire"><option value="true">ja</option><option value="false">nein</option></select></div></div><div class="toolbar"><button class="action" onclick="savePolicy()">Policy speichern</button></div><pre id="policyJson"></pre></div></section>
<section id="audit" class="page"><h2>Audit</h2><div class="card"><div class="scroll"><table><thead><tr><th>Seq</th><th>Zeit</th><th>Akteur</th><th>Aktion</th><th>Ziel</th><th>Ergebnis</th><th>Hash-Kette</th><th>Details</th></tr></thead><tbody id="auditRows"></tbody></table></div></div></section>
<section id="maintenance" class="page"><h2>Wartung</h2><div class="card"><div class="toolbar"><button class="action" onclick="post('/api/v1/maintenance/tick',{})">Timer/Rotation jetzt prüfen</button><button class="action secondary" onclick="post('/api/v1/backups',{note:'WebUI backup'})">Verschlüsseltes Backup erzeugen</button><button class="action secondary" onclick="location.href='/api/v1/export.json'">Redacted Export</button></div><div class="notice">Backups enthalten Metadaten und den weiterhin verschlüsselten Vault, nicht den Master-Key. Restore ist bewusst ein Offline-Wartungsworkflow.</div><div class="scroll"><table><thead><tr><th>ID</th><th>Zeit</th><th>Pfad</th><th>State SHA-256</th><th>Vault SHA-256</th><th>Verifiziert</th></tr></thead><tbody id="backupRows"></tbody></table></div></div></section>
<section id="api" class="page"><h2>API</h2><div class="card"><p><a href="/openapi.json">OpenAPI JSON</a> · <a href="/metrics">Prometheus Metrics</a> · <a href="/health/ready">Readiness</a></p><pre>Management: /api/v1/*
Edge-only: POST /api/v1/edge/actions/claim
Protocol: netcore-kmf-otar-edge-v1
Raw key exposure: disabled</pre></div></section>
<section id="about" class="page"><h2>Über</h2><div class="card"><p><b>NetCore-Tetra KMF</b></p><p>CCK-, GCK- und SCK-Lifecycle, Key-Versionen, Crypto Periods, Rotation, nodegebundene OTAR-Envelopes, Backup und hashverkettetes Audit.</p><p class="muted">Die aktuelle lab_file_vault-Implementierung und der SHA-256-Lab-Envelope dienen der Integration. Sie ersetzen weder HSM/PKCS#11 noch TETRA-TA-Algorithmen oder die D-OTAR-Air-Interface-PDUs.</p></div></section>
</main></div>
<script>
const pages=[['overview','Übersicht'],['keys','Schlüssel'],['nodes','Nodes'],['otar','OTAR'],['actions','Aktionen'],['policy','Policy'],['audit','Audit'],['maintenance','Wartung'],['api','API'],['about','Über']];
const nav=document.getElementById('nav');pages.forEach(([id,label],i)=>{const b=document.createElement('button');b.textContent=label;b.className=i===0?'active':'';b.onclick=()=>show(id,b);nav.appendChild(b)});
function show(id,b){document.querySelectorAll('.page').forEach(x=>x.classList.remove('active'));document.getElementById(id).classList.add('active');document.querySelectorAll('nav button').forEach(x=>x.classList.remove('active'));b.classList.add('active');refresh()}
function esc(v){return String(v??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]))}
function pill(v){const s=String(v);const cls=/active|applied|completed|enabled|authoritative/i.test(s)?'ok':/failed|revoked|destroyed|disabled|expired|cancelled/i.test(s)?'danger':/pending|staged|draft|awaiting|in_progress|shadow/i.test(s)?'warn':'';return `<span class="pill ${cls}">${esc(s)}</span>`}
async function api(path,opt={}){const r=await fetch(path,{headers:{'Content-Type':'application/json'},...opt});const t=await r.text();let d={};try{d=t?JSON.parse(t):{}}catch{d={raw:t}}if(!r.ok)throw new Error(d.error||`${r.status} ${r.statusText}`);return d}
async function post(path,data){try{const d=await api(path,{method:'POST',body:JSON.stringify(data)});await refresh();return d}catch(e){alert(e.message);throw e}}
function metric(label,value,detail=''){return `<div class="card"><div class="muted">${esc(label)}</div><div class="metric">${esc(value)}</div><div class="muted">${esc(detail)}</div></div>`}
function csvText(v){return v.split(',').map(x=>x.trim()).filter(Boolean)}function csvNum(v){return csvText(v).map(Number).filter(Number.isFinite)}
async function refresh(){try{const [s,keys,nodes,jobs,actions,audit,backups,policy]=await Promise.all(['/api/v1/status','/api/v1/keys','/api/v1/nodes','/api/v1/otar/jobs','/api/v1/otar/actions','/api/v1/audit?limit=500','/api/v1/backups','/api/v1/policy'].map(api));window.data={s,keys,nodes,jobs,actions,audit,backups,policy};render()}catch(e){console.error(e);statusJson.textContent=e.message}}
function render(){const {s,keys,nodes,jobs,actions,audit,backups,policy}=window.data;metrics.innerHTML=[metric('Betriebsmodus',s.operating_mode,s.authoritative?'Edge-Ausgabe aktiv':'nur vorbereitet'),metric('Schlüssel',s.total_keys,`${s.active_keys} aktiv`),metric('Nodes',s.node_transport_profiles,`${s.enabled_nodes} aktiviert`),metric('OTAR-Jobs',s.otar_jobs),metric('Offene Aktionen',s.pending_actions),metric('Vault',s.vault_ready?'bereit':'Fehler',s.vault_provider)].join('');statusJson.textContent=JSON.stringify(s,null,2);
keyRows.innerHTML=keys.map(k=>`<tr><td>${esc(k.kind)}<br>${esc(k.scope)}:${esc(k.scope_value||'-')}</td><td>${esc(k.label)}</td><td>${k.version}</td><td>${pill(k.state)}</td><td class="mono">${esc(k.fingerprint)}</td><td>${esc(k.crypto_period_start)}<br>${esc(k.crypto_period_end)}</td><td><button class="action secondary" onclick="keyAction('${k.id}','stage')">Stage</button> <button class="action" onclick="keyAction('${k.id}','activate')">Aktiv</button> <button class="action secondary" onclick="rotateKey('${k.id}')">Rotate</button> <button class="action danger" onclick="keyAction('${k.id}','revoke')">Revoke</button> <button class="action danger" onclick="destroyKey('${k.id}')">Destroy</button></td></tr>`).join('')||'<tr><td colspan="7" class="muted">Noch keine Schlüssel</td></tr>';
nodeRows.innerHTML=nodes.map(n=>`<tr><td>${esc(n.display_name)}<br><span class="mono">${esc(n.node_id)}</span></td><td>${pill(n.enabled?'enabled':'disabled')}</td><td class="mono">${esc(n.transport_key_fingerprint)}</td><td class="mono">${esc(n.bootstrap_path)}</td><td>${esc(n.last_claim_at||'-')}<br>${esc(n.last_ack_at||'-')}</td><td><button class="action secondary" onclick="post('/api/v1/nodes/${encodeURIComponent(n.node_id)}/${n.enabled?'disable':'enable'}',{reason:'WebUI'})">${n.enabled?'Sperren':'Freigeben'}</button></td></tr>`).join('')||'<tr><td colspan="6" class="muted">Noch keine Node-Profile</td></tr>';
jobRows.innerHTML=jobs.map(j=>`<tr><td class="mono">${esc(j.id.slice(0,12))}</td><td>${esc(j.key_kind)} v${j.key_version}<br><span class="mono">${esc(j.key_fingerprint)}</span></td><td>${esc(j.target_nodes.join(', '))}<br>ISSI ${esc(j.target_issis.join(', ')||'-')} · GSSI ${esc(j.target_gssis.join(', ')||'-')}</td><td>${j.approvals.length}/${j.required_approvals}<br>${esc(j.approvals.map(a=>a.actor).join(', '))}</td><td>${pill(j.state)}</td><td>${j.deliveries.map(d=>`${esc(d.node_id)} ${pill(d.state)}`).join('<br>')||'-'}</td><td><button class="action secondary" onclick="approve('${j.id}',1)">Approve A</button> <button class="action secondary" onclick="approve('${j.id}',2)">Approve B</button> <button class="action" onclick="post('/api/v1/otar/jobs/${j.id}/queue',{})">Queue</button> <button class="action danger" onclick="post('/api/v1/otar/jobs/${j.id}/cancel',{reason:'WebUI'})">Cancel</button></td></tr>`).join('')||'<tr><td colspan="7" class="muted">Noch keine OTAR-Jobs</td></tr>';
actionRows.innerHTML=actions.map(a=>`<tr><td>${a.sequence}</td><td class="mono">${esc(a.node_id)}</td><td class="mono">${esc(a.job_id.slice(0,10))}<br>${esc(a.key_id.slice(0,10))}</td><td>${pill(a.state)}</td><td>${a.attempts}</td><td>${esc(a.next_attempt_at)}</td><td>${esc(a.last_error||'')}</td></tr>`).join('')||'<tr><td colspan="7" class="muted">Keine Aktionen</td></tr>';
auditRows.innerHTML=audit.map(a=>`<tr><td>${a.sequence}</td><td>${esc(a.timestamp)}</td><td>${esc(a.actor)}</td><td>${esc(a.action)}</td><td class="mono">${esc(a.target)}</td><td>${pill(a.outcome)}</td><td class="mono">${esc(a.previous_hash.slice(0,12))} → ${esc(a.record_hash.slice(0,12))}</td><td class="mono">${esc(JSON.stringify(a.detail))}</td></tr>`).join('');
backupRows.innerHTML=backups.map(b=>`<tr><td class="mono">${esc(b.id)}</td><td>${esc(b.created_at)}</td><td class="mono">${esc(b.directory)}</td><td class="mono">${esc(b.metadata_sha256.slice(0,16))}</td><td class="mono">${esc(b.vault_sha256.slice(0,16))}</td><td>${pill(b.verified?'verified':'failed')}</td></tr>`).join('')||'<tr><td colspan="6" class="muted">Keine Backups</td></tr>';
policyJson.textContent=JSON.stringify(policy,null,2);pMode.value=policy.operating_mode;pBytes.value=policy.default_key_bytes;pPeriod.value=policy.default_crypto_period_secs;pLead.value=policy.rotation_lead_secs;pDual.value=String(policy.require_dual_approval);pOverlap.value=String(policy.allow_overlapping_crypto_periods);pRetire.value=String(policy.auto_retire_predecessor)}
async function createKey(){await post('/api/v1/keys',{kind:kKind.value,scope:kScope.value,scope_value:kScopeValue.value||null,label:kLabel.value,key_bytes:Number(kBytes.value),crypto_period_start:kStart.value||null,crypto_period_end:kEnd.value||null,algorithm_profile:'tetra-key-material-lab-v1',notes:kNotes.value})}
async function keyAction(id,action){await post(`/api/v1/keys/${id}/${action}`,{reason:`WebUI ${action}`})}
async function rotateKey(id){await post(`/api/v1/keys/${id}/rotate`,{actor:'webui-operator',notes:'WebUI rotation'})}
async function destroyKey(id){if(confirm('Schlüsselmaterial endgültig aus dem Vault löschen? Metadaten bleiben als Audit-Spur erhalten.'))await keyAction(id,'destroy')}
async function createNode(){const n=await post('/api/v1/nodes',{node_id:nId.value,display_name:nName.value,notes:nNotes.value});alert(`Bootstrap wurde serverseitig geschrieben: ${n.bootstrap_path}`)}
async function createJob(){await post('/api/v1/otar/jobs',{key_id:oKey.value,target_nodes:csvText(oNodes.value),target_issis:csvNum(oIssi.value),target_gssis:csvNum(oGssi.value),expires_at:oExpires.value||null,not_before:null,notes:oNotes.value})}
async function approve(id,n){const actor=prompt('Freigebender Actor-Name',n===1?'operator-a':'operator-b');if(actor)await post(`/api/v1/otar/jobs/${id}/approve`,{actor,note:'WebUI approval'})}
async function savePolicy(){await post('/api/v1/policy',{operating_mode:pMode.value,default_key_bytes:Number(pBytes.value),default_crypto_period_secs:Number(pPeriod.value),rotation_lead_secs:Number(pLead.value),require_dual_approval:pDual.value==='true',allow_overlapping_crypto_periods:pOverlap.value==='true',auto_retire_predecessor:pRetire.value==='true'})}
refresh();setInterval(refresh,5000);
</script>
</body></html>"#;

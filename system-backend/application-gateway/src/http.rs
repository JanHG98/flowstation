use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};

use crate::config::ApplicationGatewayConfig;
use crate::model::{
    ActionInput, BackupInput, ConnectorInput, DispatchInput, RouteRuleInput, SecretSetInput,
    TemplateInput, TemplateRenderInput, TtsJobInput, TtsPublishInput,
};
use crate::state::SharedGateway;
use crate::worker;

pub fn spawn_http_server(
    config: ApplicationGatewayConfig,
    gateway: SharedGateway,
) -> std::io::Result<thread::JoinHandle<()>> {
    let listener = TcpListener::bind(config.server.bind)?;
    tracing::info!(
        "Application Gateway WebUI/API listening on http://{}",
        config.server.bind
    );
    Ok(thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let config = config.clone();
                    let gateway = gateway.clone();
                    thread::spawn(move || {
                        if let Err(error) = handle_connection(stream, config, gateway) {
                            tracing::warn!("Application Gateway HTTP connection failed: {}", error);
                        }
                    });
                }
                Err(error) => tracing::warn!("Application Gateway HTTP accept failed: {}", error),
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
    extra_headers: Vec<(String, String)>,
}

fn handle_connection(
    mut stream: TcpStream,
    config: ApplicationGatewayConfig,
    gateway: SharedGateway,
) -> Result<(), String> {
    let request = read_request(&mut stream, config.server.max_body_bytes)?;
    let response = route(request, config, gateway);
    write_response(&mut stream, response).map_err(|error| error.to_string())
}

fn route(
    request: HttpRequest,
    config: ApplicationGatewayConfig,
    gateway: SharedGateway,
) -> HttpResponse {
    if request.method == "OPTIONS" {
        return empty(204);
    }
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") => html(INDEX_HTML),
        ("GET", "/health/live") => json_response(200, &json!({"status":"live"})),
        ("GET", "/health/ready") => {
            let status = gateway.status();
            json_response(if status.ready { 200 } else { 503 }, &status)
        }
        ("GET", "/api/v1/status") => json_response(200, &gateway.status()),
        ("GET", "/api/v1/config") => json_response(200, &gateway.redacted_config()),
        ("GET", "/api/v1/connectors") => json_response(200, &gateway.connectors()),
        ("POST", "/api/v1/connectors") => {
            match parse_json::<ConnectorInput>(&request.body)
                .and_then(|input| gateway.create_connector(input))
            {
                Ok(value) => json_response(201, &value),
                Err(error) => conflict(error),
            }
        }
        ("GET", "/api/v1/secrets") => json_response(
            200,
            &gateway.secret_statuses(request.query.get("connector_id").map(String::as_str)),
        ),
        ("GET", "/api/v1/rules") => json_response(200, &gateway.rules()),
        ("POST", "/api/v1/rules") => {
            match parse_json::<RouteRuleInput>(&request.body)
                .and_then(|input| gateway.create_rule(input))
            {
                Ok(value) => json_response(201, &value),
                Err(error) => conflict(error),
            }
        }
        ("GET", "/api/v1/templates") => json_response(200, &gateway.templates()),
        ("POST", "/api/v1/templates") => {
            match parse_json::<TemplateInput>(&request.body)
                .and_then(|input| gateway.create_template(input))
            {
                Ok(value) => json_response(201, &value),
                Err(error) => conflict(error),
            }
        }
        ("GET", "/api/v1/events") => json_response(
            200,
            &gateway.events(
                request.query.get("state").map(String::as_str),
                request.query.get("source").map(String::as_str),
                query_usize(&request, "limit", 500, 5_000),
            ),
        ),
        ("POST", "/api/v1/events") | ("POST", "/api/v1/dispatch") => {
            match parse_json::<DispatchInput>(&request.body).and_then(|input| gateway.dispatch(input))
            {
                Ok(value) => json_response(202, &value),
                Err(error) => conflict(error),
            }
        }
        ("GET", "/api/v1/deliveries") => json_response(
            200,
            &gateway.deliveries(
                request.query.get("state").map(String::as_str),
                request.query.get("connector_id").map(String::as_str),
                query_usize(&request, "limit", 500, 5_000),
            ),
        ),
        ("GET", "/api/v1/tts/jobs") => json_response(
            200,
            &gateway.tts_jobs(
                request.query.get("state").map(String::as_str),
                query_usize(&request, "limit", 500, 5_000),
            ),
        ),
        ("POST", "/api/v1/tts/jobs") => {
            match parse_json::<TtsJobInput>(&request.body)
                .and_then(|input| gateway.create_tts_job(input))
            {
                Ok(value) => json_response(202, &value),
                Err(error) => conflict(error),
            }
        }
        ("GET", "/api/v1/audit") => {
            json_response(200, &gateway.audit(query_usize(&request, "limit", 500, 5_000)))
        }
        ("GET", "/api/v1/backups") => json_response(200, &gateway.backups()),
        ("POST", "/api/v1/backups") => {
            match parse_json_or_default::<BackupInput>(&request.body)
                .and_then(|input| gateway.backup(input))
            {
                Ok(value) => json_response(201, &value),
                Err(error) => json_response(500, &json!({"error":error})),
            }
        }
        ("POST", "/api/v1/maintenance/tick") => {
            match parse_json_or_default::<ActionInput>(&request.body)
                .and_then(|input| gateway.maintenance(input.actor))
            {
                Ok(value) => json_response(200, &value),
                Err(error) => json_response(500, &json!({"error":error})),
            }
        }
        ("POST", "/api/v1/maintenance/process-now") => {
            match worker::build_client() {
                Ok(client) => {
                    worker::run_cycle(&config, &gateway, &client, 100);
                    json_response(200, &gateway.status())
                }
                Err(error) => json_response(500, &json!({"error":error})),
            }
        }
        ("GET", "/api/v1/export.json") => download(
            "netcore-application-gateway-export.json",
            "application/json",
            serde_json::to_vec_pretty(&gateway.export()).unwrap_or_default(),
        ),
        ("GET", "/metrics") => text(
            "text/plain; version=0.0.4; charset=utf-8",
            gateway.metrics(),
        ),
        ("GET", "/openapi.json") => json_response(200, &openapi()),
        _ => dynamic_route(request, config, gateway),
    }
}

fn dynamic_route(
    request: HttpRequest,
    _config: ApplicationGatewayConfig,
    gateway: SharedGateway,
) -> HttpResponse {
    let parts: Vec<&str> = request.path.trim_matches('/').split('/').collect();
    match (request.method.as_str(), parts.as_slice()) {
        ("GET", ["api", "v1", "connectors", connector_id]) => gateway
            .connector(connector_id)
            .map_or_else(|| not_found(format!("connector {connector_id} not found")), |value| json_response(200, &value)),
        ("PUT", ["api", "v1", "connectors", connector_id]) => {
            match parse_json::<ConnectorInput>(&request.body)
                .and_then(|input| gateway.update_connector(connector_id, input))
            {
                Ok(value) => json_response(200, &value),
                Err(error) => conflict(error),
            }
        }
        ("DELETE", ["api", "v1", "connectors", connector_id]) => {
            match parse_json_or_default::<ActionInput>(&request.body)
                .and_then(|input| gateway.delete_connector(connector_id, input))
            {
                Ok(()) => empty(204),
                Err(error) => conflict(error),
            }
        }
        ("POST", ["api", "v1", "connectors", connector_id, "test"]) => {
            let Some(connector) = gateway.connector_for_probe(connector_id) else {
                return not_found(format!("connector {connector_id} not found"));
            };
            match worker::build_client() {
                Ok(client) => {
                    let outcome = worker::test_connector(&client, &connector);
                    match gateway.record_probe(outcome) {
                        Ok(value) => json_response(200, &value),
                        Err(error) => conflict(error),
                    }
                }
                Err(error) => json_response(500, &json!({"error":error})),
            }
        }
        ("POST", ["api", "v1", "connectors", connector_id, action]) => {
            match parse_json_or_default::<ActionInput>(&request.body)
                .and_then(|input| gateway.connector_action(connector_id, action, input))
            {
                Ok(value) => json_response(200, &value),
                Err(error) => conflict(error),
            }
        }
        ("GET", ["api", "v1", "connectors", connector_id, "secrets"]) => {
            json_response(200, &gateway.secret_statuses(Some(connector_id)))
        }
        ("POST", ["api", "v1", "connectors", connector_id, "secrets"]) => {
            match parse_json::<SecretSetInput>(&request.body)
                .and_then(|input| gateway.set_secret(connector_id, input))
            {
                Ok(value) => json_response(200, &value),
                Err(error) => conflict(error),
            }
        }
        ("DELETE", ["api", "v1", "connectors", connector_id, "secrets", name]) => {
            match parse_json_or_default::<ActionInput>(&request.body)
                .and_then(|input| gateway.delete_secret(connector_id, name, input))
            {
                Ok(()) => empty(204),
                Err(error) => not_found(error),
            }
        }
        ("POST", ["api", "v1", "webhooks", connector_id]) => {
            match parse_json::<DispatchInput>(&request.body)
                .and_then(|input| gateway.ingest_webhook(connector_id, input))
            {
                Ok(value) => json_response(202, &value),
                Err(error) => conflict(error),
            }
        }
        ("PUT", ["api", "v1", "rules", rule_id]) => {
            match parse_json::<RouteRuleInput>(&request.body)
                .and_then(|input| gateway.update_rule(rule_id, input))
            {
                Ok(value) => json_response(200, &value),
                Err(error) => conflict(error),
            }
        }
        ("POST", ["api", "v1", "rules", rule_id, action]) => {
            match parse_json_or_default::<ActionInput>(&request.body)
                .and_then(|input| gateway.rule_action(rule_id, action, input))
            {
                Ok(value) => json_response(200, &value),
                Err(error) => conflict(error),
            }
        }
        ("DELETE", ["api", "v1", "rules", rule_id]) => {
            match parse_json_or_default::<ActionInput>(&request.body)
                .and_then(|input| gateway.delete_rule(rule_id, input))
            {
                Ok(()) => empty(204),
                Err(error) => conflict(error),
            }
        }
        ("PUT", ["api", "v1", "templates", template_id]) => {
            match parse_json::<TemplateInput>(&request.body)
                .and_then(|input| gateway.update_template(template_id, input))
            {
                Ok(value) => json_response(200, &value),
                Err(error) => conflict(error),
            }
        }
        ("POST", ["api", "v1", "templates", template_id, "render"]) => {
            match parse_json::<TemplateRenderInput>(&request.body)
                .and_then(|input| gateway.render_template(template_id, input))
            {
                Ok(value) => json_response(200, &value),
                Err(error) => conflict(error),
            }
        }
        ("DELETE", ["api", "v1", "templates", template_id]) => {
            match parse_json_or_default::<ActionInput>(&request.body)
                .and_then(|input| gateway.delete_template(template_id, input))
            {
                Ok(()) => empty(204),
                Err(error) => conflict(error),
            }
        }
        ("GET", ["api", "v1", "deliveries", delivery_id]) => gateway
            .delivery(delivery_id)
            .map_or_else(|| not_found(format!("delivery {delivery_id} not found")), |value| json_response(200, &value)),
        ("POST", ["api", "v1", "deliveries", delivery_id, action]) => {
            match parse_json_or_default::<ActionInput>(&request.body)
                .and_then(|input| gateway.delivery_action(delivery_id, action, input))
            {
                Ok(value) => json_response(200, &value),
                Err(error) => conflict(error),
            }
        }
        ("POST", ["api", "v1", "tts", "jobs", job_id, "publish"]) => {
            match parse_json_or_default::<TtsPublishInput>(&request.body)
                .and_then(|input| gateway.publish_tts_job(job_id, input))
            {
                Ok(value) => json_response(202, &value),
                Err(error) => conflict(error),
            }
        }
        ("GET", ["api", "v1", "tts", "jobs", job_id, "artifact"]) => {
            match gateway.tts_artifact(job_id) {
                Ok((name, bytes)) => download(&name, "audio/wav", bytes),
                Err(error) => not_found(error),
            }
        }
        _ => not_found("route not found"),
    }
}

fn openapi() -> Value {
    json!({
        "openapi":"3.1.0",
        "info":{
            "title":"NetCore-Tetra Application Gateway API",
            "version":"1.0.0",
            "description":"OPEN LAB management API without login, management tokens or TLS"
        },
        "paths":{
            "/api/v1/status":{"get":{}},
            "/api/v1/connectors":{"get":{},"post":{}},
            "/api/v1/connectors/{connector_id}":{"get":{},"put":{},"delete":{}},
            "/api/v1/connectors/{connector_id}/enable":{"post":{}},
            "/api/v1/connectors/{connector_id}/disable":{"post":{}},
            "/api/v1/connectors/{connector_id}/reset-circuit":{"post":{}},
            "/api/v1/connectors/{connector_id}/test":{"post":{}},
            "/api/v1/connectors/{connector_id}/secrets":{"get":{},"post":{}},
            "/api/v1/connectors/{connector_id}/secrets/{name}":{"delete":{}},
            "/api/v1/webhooks/{connector_id}":{"post":{}},
            "/api/v1/rules":{"get":{},"post":{}},
            "/api/v1/rules/{rule_id}":{"put":{},"delete":{}},
            "/api/v1/templates":{"get":{},"post":{}},
            "/api/v1/templates/{template_id}":{"put":{},"delete":{}},
            "/api/v1/templates/{template_id}/render":{"post":{}},
            "/api/v1/events":{"get":{},"post":{}},
            "/api/v1/deliveries":{"get":{}},
            "/api/v1/deliveries/{delivery_id}/{action}":{"post":{}},
            "/api/v1/tts/jobs":{"get":{},"post":{}},
            "/api/v1/tts/jobs/{job_id}/publish":{"post":{}},
            "/api/v1/tts/jobs/{job_id}/artifact":{"get":{}},
            "/api/v1/audit":{"get":{}},
            "/api/v1/backups":{"get":{},"post":{}},
            "/api/v1/maintenance/tick":{"post":{}},
            "/api/v1/maintenance/process-now":{"post":{}},
            "/metrics":{"get":{}},
            "/health/live":{"get":{}},
            "/health/ready":{"get":{}}
        }
    })
}

fn read_request(stream: &mut TcpStream, max_body_bytes: usize) -> Result<HttpRequest, String> {
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 8192];
    let header_end = loop {
        let count = stream.read(&mut buffer).map_err(|error| error.to_string())?;
        if count == 0 {
            return Err("connection closed before request headers".into());
        }
        bytes.extend_from_slice(&buffer[..count]);
        if bytes.len() > 64 * 1024 {
            return Err("request headers too large".into());
        }
        if let Some(position) = find_bytes(&bytes, b"\r\n\r\n") {
            break position + 4;
        }
    };
    let header = std::str::from_utf8(&bytes[..header_end]).map_err(|_| "request header is not UTF-8")?;
    let mut lines = header.split("\r\n");
    let first = lines.next().ok_or("missing request line")?;
    let mut request_line = first.split_whitespace();
    let method = request_line.next().ok_or("missing request method")?.to_string();
    let target = request_line.next().ok_or("missing request target")?;
    let content_length = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim().parse::<usize>().ok())
        .unwrap_or(0);
    if content_length > max_body_bytes {
        return Err(format!("request body exceeds {max_body_bytes} bytes"));
    }
    while bytes.len() < header_end + content_length {
        let count = stream.read(&mut buffer).map_err(|error| error.to_string())?;
        if count == 0 {
            return Err("connection closed during request body".into());
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
    let body = bytes[header_end..header_end + content_length].to_vec();
    let (raw_path, raw_query) = target.split_once('?').unwrap_or((target, ""));
    Ok(HttpRequest {
        method,
        path: percent_decode(raw_path),
        query: parse_query(raw_query),
        body,
    })
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
        _ => "OK",
    };
    write!(
        stream,
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: Content-Type\r\nAccess-Control-Allow-Methods: GET, POST, PUT, DELETE, OPTIONS\r\n",
        response.status,
        reason,
        response.content_type,
        response.body.len()
    )?;
    if let Some(disposition) = response.disposition {
        write!(stream, "Content-Disposition: {disposition}\r\n")?;
    }
    for (name, value) in response.extra_headers {
        write!(stream, "{name}: {value}\r\n")?;
    }
    write!(stream, "\r\n")?;
    stream.write_all(&response.body)?;
    stream.flush()
}

fn parse_json<T: DeserializeOwned>(body: &[u8]) -> Result<T, String> {
    serde_json::from_slice(body).map_err(|error| format!("invalid JSON: {error}"))
}

fn parse_json_or_default<T: DeserializeOwned + Default>(body: &[u8]) -> Result<T, String> {
    if body.is_empty() {
        Ok(T::default())
    } else {
        parse_json(body)
    }
}

fn query_usize(request: &HttpRequest, name: &str, default: usize, max: usize) -> usize {
    request
        .query
        .get(name)
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(default)
        .min(max)
}

fn parse_query(raw: &str) -> HashMap<String, String> {
    raw.split('&')
        .filter(|item| !item.is_empty())
        .map(|item| {
            let (name, value) = item.split_once('=').unwrap_or((item, ""));
            (percent_decode(name), percent_decode(value))
        })
        .collect()
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() => {
                let high = hex(bytes[index + 1]);
                let low = hex(bytes[index + 2]);
                if let (Some(high), Some(low)) = (high, low) {
                    output.push(high * 16 + low);
                    index += 3;
                    continue;
                }
                output.push(bytes[index]);
            }
            b'+' => output.push(b' '),
            byte => output.push(byte),
        }
        index += 1;
    }
    String::from_utf8_lossy(&output).into_owned()
}

fn hex(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
}

fn json_response<T: Serialize>(status: u16, value: &T) -> HttpResponse {
    HttpResponse {
        status,
        content_type: "application/json; charset=utf-8",
        body: serde_json::to_vec_pretty(value).unwrap_or_else(|_| b"{}".to_vec()),
        disposition: None,
        extra_headers: Vec::new(),
    }
}

fn html(value: &'static str) -> HttpResponse {
    HttpResponse {
        status: 200,
        content_type: "text/html; charset=utf-8",
        body: value.as_bytes().to_vec(),
        disposition: None,
        extra_headers: Vec::new(),
    }
}

fn text(content_type: &'static str, value: String) -> HttpResponse {
    HttpResponse {
        status: 200,
        content_type,
        body: value.into_bytes(),
        disposition: None,
        extra_headers: Vec::new(),
    }
}

fn download(name: &str, content_type: &'static str, bytes: Vec<u8>) -> HttpResponse {
    HttpResponse {
        status: 200,
        content_type,
        body: bytes,
        disposition: Some(format!("attachment; filename=\"{}\"", name.replace('"', "_"))),
        extra_headers: Vec::new(),
    }
}

fn empty(status: u16) -> HttpResponse {
    HttpResponse {
        status,
        content_type: "application/json; charset=utf-8",
        body: Vec::new(),
        disposition: None,
        extra_headers: Vec::new(),
    }
}

fn conflict(error: impl ToString) -> HttpResponse {
    json_response(409, &json!({"error":error.to_string()}))
}

fn not_found(error: impl ToString) -> HttpResponse {
    json_response(404, &json!({"error":error.to_string()}))
}

const INDEX_HTML: &str = r##"<!doctype html>
<html lang="de"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>NetCore Application Gateway</title>
<style>
:root{color-scheme:dark;--bg:#071119;--panel:#101f29;--panel2:#152a35;--line:#2b4553;--text:#eef6fa;--muted:#9db1bd;--ok:#50d890;--warn:#ffc857;--bad:#ff6b6b;--accent:#4aa9ff;--purple:#b78cff}*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--text);font:14px system-ui,-apple-system,sans-serif}.lab{padding:10px 18px;background:#8d2020;color:#fff;text-align:center;font-weight:800;letter-spacing:.2px}header{display:flex;justify-content:space-between;gap:20px;padding:20px 26px;background:#0d1921;border-bottom:1px solid var(--line)}h1,h2,h3{margin:0 0 10px}.muted{color:var(--muted)}.ok{color:var(--ok)}.warn{color:var(--warn)}.bad{color:var(--bad)}.layout{display:grid;grid-template-columns:220px minmax(0,1fr);min-height:calc(100vh - 118px)}nav{padding:16px;background:#0b171e;border-right:1px solid var(--line);display:flex;flex-direction:column;gap:5px}nav button{width:100%;text-align:left;background:transparent;border:0;color:var(--muted);padding:10px 12px;border-radius:7px;cursor:pointer}nav button.active,nav button:hover{background:#17303c;color:#fff}main{padding:20px;min-width:0}.page{display:none}.page.active{display:block}.cards{display:grid;grid-template-columns:repeat(auto-fit,minmax(145px,1fr));gap:10px;margin-bottom:16px}.card,.panel{background:var(--panel);border:1px solid var(--line);border-radius:10px;padding:14px}.value{font-size:25px;font-weight:800}.grid2{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:14px}.toolbar{display:flex;gap:8px;flex-wrap:wrap;margin:10px 0}input,select,textarea,button{background:#19303b;color:var(--text);border:1px solid var(--line);border-radius:6px;padding:8px}button{cursor:pointer}.primary{background:#126eae}.danger{background:#8b3138}.secondary{background:#304554}.purple{background:#62488b}table{width:100%;border-collapse:collapse}th,td{text-align:left;padding:8px;border-bottom:1px solid var(--line);vertical-align:top}.scroll{overflow:auto;max-height:520px}.pill{display:inline-block;padding:2px 7px;border-radius:999px;background:#28404d;font-size:12px}.pill.healthy,.pill.delivered,.pill.ready,.pill.published{background:#1d5f41}.pill.degraded,.pill.retry,.pill.queued,.pill.publish_queued{background:#725b1d}.pill.dead_letter,.pill.failed,.pill.publish_failed,.pill.open{background:#7d2d35}.pill.shadowed{background:#503e75}.mono{font-family:ui-monospace,SFMono-Regular,monospace}.notice{border-left:4px solid var(--warn);padding:9px 12px;background:#2b2517;margin:10px 0}dialog{background:var(--panel);color:var(--text);border:1px solid var(--line);border-radius:10px;min-width:min(720px,95vw)}dialog::backdrop{background:#000a}label{display:grid;gap:4px;color:var(--muted)}.formgrid{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:10px}.wide{grid-column:1/-1}pre{white-space:pre-wrap;word-break:break-word;max-height:420px;overflow:auto}.small{font-size:12px}@media(max-width:850px){.layout{grid-template-columns:1fr}nav{position:sticky;top:0;z-index:3;flex-direction:row;overflow:auto;border-right:0;border-bottom:1px solid var(--line)}nav button{width:auto;white-space:nowrap}.grid2,.formgrid{grid-template-columns:1fr}header{display:block}}
</style></head><body>
<div class="lab">⚠ OPEN LAB – keine Anmeldung, keine Management-Tokens, kein TLS. Jeder erreichbare Client darf Connectoren, Routing, Secrets, TTS und externe Aussendungen steuern.</div>
<header><div><h1>Application Gateway</h1><div class="muted">Connectoren, Webhooks, Routing, Vorlagen, Queues und TTS-Orchestrierung</div></div><div><span id="mode" class="pill">…</span> <span id="clock" class="muted"></span></div></header>
<div class="layout"><nav id="nav">
<button data-page="overview" class="active">Übersicht</button><button data-page="connectors">Connectoren</button><button data-page="routing">Routing</button><button data-page="templates">Vorlagen</button><button data-page="dispatch">Aussendung</button><button data-page="tts">TTS</button><button data-page="queue">Queues</button><button data-page="audit">Audit / Wartung</button><button data-page="api">API</button>
</nav><main>
<section id="overview" class="page active"><div class="cards" id="cards"></div><div class="grid2"><div class="panel"><h2>Connector-Lage</h2><div id="connectorSummary"></div></div><div class="panel"><h2>Letzte Ereignisse</h2><pre id="recentEvents">Lade …</pre></div></div></section>
<section id="connectors" class="page"><div class="toolbar"><button class="primary" onclick="openConnector()">Connector anlegen</button><button onclick="refresh()">Aktualisieren</button></div><div class="panel scroll"><table><thead><tr><th>Connector</th><th>Typ / Richtung</th><th>Endpoint</th><th>Health / Circuit</th><th>Zähler</th><th>Secrets</th><th>Aktionen</th></tr></thead><tbody id="connectorRows"></tbody></table></div></section>
<section id="routing" class="page"><div class="toolbar"><button class="primary" onclick="openRule()">Regel anlegen</button></div><div class="panel scroll"><table><thead><tr><th>Priorität</th><th>Match</th><th>Ziel</th><th>Vorlage</th><th>Treffer</th><th>Aktionen</th></tr></thead><tbody id="ruleRows"></tbody></table></div></section>
<section id="templates" class="page"><div class="toolbar"><button class="primary" onclick="openTemplate()">Vorlage anlegen</button></div><div class="panel scroll"><table><thead><tr><th>Vorlage</th><th>Typ</th><th>Ziel</th><th>Body</th><th>Render</th><th>Aktionen</th></tr></thead><tbody id="templateRows"></tbody></table></div></section>
<section id="dispatch" class="page"><div class="grid2"><form class="panel" onsubmit="sendDispatch(event)"><h2>Manuelle Aussendung</h2><div class="formgrid"><label>Event-Typ<input id="dType" value="sds.message"></label><label>Ziel / ISSI / GSSI<input id="dDestination" value="2000"></label><label>Direkter Connector<select id="dConnector"><option value="">Routingregeln verwenden</option></select></label><label>Vorlage<select id="dTemplate"><option value="">keine</option></select></label><label class="wide">Text<textarea id="dText" rows="5">Testnachricht aus dem Application Gateway</textarea></label><label class="wide">Payload JSON<textarea id="dPayload" rows="5">{}</textarea></label></div><div class="toolbar"><button class="primary">Einreihen</button></div></form><div class="panel"><h2>Ergebnis</h2><pre id="dispatchOutput">Noch keine Aussendung.</pre><div class="notice">Im Shadow-Modus wird die Route vollständig erzeugt, der externe Side-Effect aber unterdrückt. Das ist Absicht, kein gelangweilter Worker.</div></div></div></section>
<section id="tts" class="page"><div class="grid2"><form class="panel" onsubmit="createTts(event)"><h2>TTS erzeugen</h2><div class="formgrid"><label>Name<input id="tName" value="Testdurchsage"></label><label>Vorlage<select id="tTemplate"><option value="">ohne Vorlage</option></select></label><label>Stimme<input id="tVoice" value="de_DE-thorsten-medium"></label><label>Geschwindigkeit<input id="tSpeed" type="number" step="0.05" min="0.5" max="2" value="0.95"></label><label>Zieltyp<select id="tDestKind"><option value="group">Gruppe</option><option value="individual">Einzeln</option></select></label><label>Ziel-ID<input id="tDestId" type="number" value="2000"></label><label class="wide">Text<textarea id="tText" rows="5">Dies ist eine Testdurchsage.</textarea></label></div><button class="primary">Synthese einreihen</button></form><div class="panel"><h2>TTS-Ablauf</h2><p>Piper erzeugt die WAV-Datei. Das Gateway speichert sie im Spool und stellt sie anschließend der Media Library per Import-URL bereit. Die eigentliche dauerhafte Ablage und Funkwiedergabe bleibt bewusst Aufgabe der nächsten Dienste.</p><pre id="ttsOutput">Noch kein Job.</pre></div></div><div class="panel scroll" style="margin-top:14px"><table><thead><tr><th>Job</th><th>Stimme / Text</th><th>Status</th><th>Artefakt</th><th>Ziel</th><th>Aktionen</th></tr></thead><tbody id="ttsRows"></tbody></table></div></section>
<section id="queue" class="page"><div class="toolbar"><select id="queueState" onchange="refresh()"><option value="">alle Zustände</option><option>queued</option><option>retry</option><option>in_flight</option><option>delivered</option><option>shadowed</option><option>dead_letter</option><option>cancelled</option></select><button class="primary" onclick="processNow()">Jetzt verarbeiten</button></div><div class="panel scroll"><table><thead><tr><th>Delivery</th><th>Connector</th><th>Event / Ziel</th><th>Status</th><th>Versuche</th><th>Antwort / Fehler</th><th>Aktionen</th></tr></thead><tbody id="deliveryRows"></tbody></table></div></section>
<section id="audit" class="page"><div class="toolbar"><button onclick="maintenance()">Retention / Timer</button><button onclick="backup()">Backup ohne Secrets</button><button onclick="location.href='/api/v1/export.json'">Redacted Export</button></div><div class="grid2"><div class="panel"><h2>Audit</h2><pre id="auditRows"></pre></div><div class="panel"><h2>Backups</h2><pre id="backupRows"></pre></div></div></section>
<section id="api" class="page"><div class="panel"><h2>API</h2><p><a href="/openapi.json">OpenAPI JSON</a> · <a href="/metrics">Prometheus Metrics</a> · <a href="/health/ready">Readiness</a></p><pre>Inbound Webhook: POST /api/v1/webhooks/{connector_id}
Dispatch:        POST /api/v1/dispatch
Connectoren:     /api/v1/connectors/*
Secrets:         /api/v1/connectors/{id}/secrets
Routing:         /api/v1/rules/*
Vorlagen:        /api/v1/templates/*
TTS:             /api/v1/tts/jobs/*
Queues:          /api/v1/deliveries/*</pre><div class="notice">Secret-Werte werden nie über GET, Export, Audit oder Metrics ausgegeben. Connector-Credentials sind nicht dasselbe wie Management-Authentisierung; die Managementoberfläche bleibt in dieser Phase ausdrücklich offen.</div></div></section>
</main></div>
<dialog id="connectorDialog"><form method="dialog" onsubmit="saveConnector(event)"><h2>Connector</h2><div class="formgrid"><label>ID<input id="cId" required></label><label>Name<input id="cName" required></label><label>Typ<select id="cKind"><option>generic_webhook</option><option>sds_router</option><option>telegram_bot</option><option>dapnet_http</option><option>meshcom_http</option><option>snom_notify</option><option>geoalarm_http</option><option>weather_http</option><option>tpg2200_bridge</option><option>directory_http</option><option>piper_tts</option><option>media_library</option></select></label><label>Richtung<select id="cDirection"><option>outbound</option><option>inbound</option><option>bidirectional</option></select></label><label class="wide">Endpoint<input id="cEndpoint" required></label><label class="wide">Health-Endpoint<input id="cHealth"></label><label>Rate/min<input id="cRate" type="number" value="60"></label><label>Timeout ms<input id="cTimeout" type="number" value="5000"></label><label class="wide">Settings JSON<textarea id="cSettings" rows="5">{}</textarea></label><label class="wide">Required Secrets, Komma-getrennt<input id="cSecrets"></label></div><div class="toolbar"><button class="primary">Speichern</button><button value="cancel">Abbrechen</button></div></form></dialog>
<dialog id="ruleDialog"><form method="dialog" onsubmit="saveRule(event)"><h2>Routingregel</h2><div class="formgrid"><label>ID<input id="rId" required></label><label>Name<input id="rName" required></label><label>Quelle<input id="rSource" value="manual"></label><label>Event-Typ<input id="rType" value="sds.message"></label><label>Ziel-Connector<select id="rConnector"></select></label><label>Vorlage<select id="rTemplate"><option value="">keine</option></select></label><label>Priorität<input id="rPriority" type="number" value="0"></label><label>Text enthält<input id="rContains"></label><label>Ziel-Override<input id="rDestination"></label><label><input id="rStop" type="checkbox"> danach stoppen</label></div><div class="toolbar"><button class="primary">Speichern</button><button value="cancel">Abbrechen</button></div></form></dialog>
<dialog id="templateDialog"><form method="dialog" onsubmit="saveTemplate(event)"><h2>Vorlage</h2><div class="formgrid"><label>ID<input id="pId" required></label><label>Name<input id="pName" required></label><label>Typ<select id="pKind"><option>text</option><option>json</option><option>tts</option></select></label><label>Ziel-Connector<select id="pConnector"><option value="">keiner</option></select></label><label class="wide">Body<textarea id="pBody" rows="8" required>{{text}}</textarea></label><label class="wide">Beschreibung<input id="pDescription"></label></div><div class="toolbar"><button class="primary">Speichern</button><button value="cancel">Abbrechen</button></div></form></dialog>
<script>
const $=id=>document.getElementById(id);let data={connectors:[],rules:[],templates:[],deliveries:[],tts:[]};
const esc=v=>String(v??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#039;'}[c]));
const pill=v=>`<span class="pill ${esc(v)}">${esc(v)}</span>`;async function api(path,opt={}){const r=await fetch(path,{headers:{'Content-Type':'application/json'},...opt});const text=await r.text();let body={};try{body=text?JSON.parse(text):{}}catch{body={raw:text}}if(!r.ok)throw new Error(body.error||body.message||`HTTP ${r.status}`);return body}
function initNav(){document.querySelectorAll('#nav button').forEach(b=>b.onclick=()=>{document.querySelectorAll('#nav button').forEach(x=>x.classList.toggle('active',x===b));document.querySelectorAll('.page').forEach(x=>x.classList.toggle('active',x.id===b.dataset.page))})}
function options(rows,id,name,first=''){return first+rows.map(x=>`<option value="${esc(x[id])}">${esc(x[name])}</option>`).join('')}
async function refresh(){try{const q=$('queueState').value;const [s,c,r,t,e,d,j,a,b]=await Promise.all([api('/api/v1/status'),api('/api/v1/connectors'),api('/api/v1/rules'),api('/api/v1/templates'),api('/api/v1/events?limit=30'),api('/api/v1/deliveries?limit=500'+(q?'&state='+encodeURIComponent(q):'')),api('/api/v1/tts/jobs?limit=300'),api('/api/v1/audit?limit=300'),api('/api/v1/backups')]);data={s,connectors:c,rules:r,templates:t,events:e,deliveries:d,tts:j,audit:a,backups:b};render()}catch(err){console.error(err);$('recentEvents').textContent=err.message}}
function render(){const {s,connectors,rules,templates,events,deliveries,tts,audit,backups}=data;$('mode').textContent=s.operating_mode;$('mode').className='pill '+(s.operating_mode==='authoritative'?'healthy':'shadowed');$('cards').innerHTML=[['Connectoren',`${s.connectors_healthy}/${s.connectors_enabled}`],['Circuits offen',s.circuits_open],['Events',s.events_total],['Unrouted',s.events_unrouted],['Queued',s.deliveries_queued],['Retry',s.deliveries_retry],['Dead Letter',s.deliveries_dead_letter],['TTS ready',`${s.tts_jobs_ready}/${s.tts_jobs_total}`],['Secrets fehlen',s.missing_required_secrets]].map(x=>`<div class="card"><div class="muted">${x[0]}</div><div class="value">${x[1]}</div></div>`).join('');$('connectorSummary').innerHTML=connectors.map(c=>`${pill(c.health)} ${esc(c.display_name)} <span class="muted">${esc(c.kind)} · ${esc(c.circuit_state)}</span>`).join('<br>');$('recentEvents').textContent=events.map(x=>`${x.received_at} ${x.source_connector} ${x.event_type} → ${x.destination||'-'} [${x.state}]`).join('\n');
$('connectorRows').innerHTML=connectors.map(c=>`<tr><td><b>${esc(c.display_name)}</b><br><span class="mono small">${esc(c.connector_id)}</span></td><td>${esc(c.kind)}<br>${esc(c.direction)}</td><td class="small">${esc(c.endpoint)}<br><span class="muted">${esc(c.health_endpoint||'kein Probe-Endpoint')}</span></td><td>${pill(c.health)} ${pill(c.circuit_state)}<br>${esc(c.last_error||'')}</td><td>TX ${c.sent_total} / Fehler ${c.failed_total}<br>RX ${c.received_total}</td><td>${c.required_secrets.map(x=>`${x.present?'✓':'✗'} ${esc(x.name)}${x.fingerprint?' '+esc(x.fingerprint):''}`).join('<br>')||'–'}<br><button onclick="setSecret('${esc(c.connector_id)}')">setzen</button></td><td><button onclick="editConnector('${esc(c.connector_id)}')">Edit</button> <button onclick="connectorAction('${esc(c.connector_id)}','test')">Test</button> <button onclick="connectorAction('${esc(c.connector_id)}','${c.enabled?'disable':'enable'}')">${c.enabled?'Aus':'Ein'}</button> <button onclick="connectorAction('${esc(c.connector_id)}','reset-circuit')">Reset</button></td></tr>`).join('');
$('ruleRows').innerHTML=rules.map(r=>`<tr><td>${r.priority}</td><td>${esc(r.source_connector)} / ${esc(r.event_type)}${r.text_contains?'<br>enthält '+esc(r.text_contains):''}</td><td>${esc(r.target_connector)}${r.destination?'<br>→ '+esc(r.destination):''}</td><td>${esc(r.template_id||'–')}</td><td>${r.matched_total}</td><td>${pill(r.enabled?'enabled':'disabled')} <button onclick="editRule('${esc(r.rule_id)}')">Edit</button> <button onclick="ruleAction('${esc(r.rule_id)}','${r.enabled?'disable':'enable'}')">${r.enabled?'Aus':'Ein'}</button> <button class="danger" onclick="deleteRule('${esc(r.rule_id)}')">Löschen</button></td></tr>`).join('');
$('templateRows').innerHTML=templates.map(t=>`<tr><td><b>${esc(t.name)}</b><br><span class="mono small">${esc(t.template_id)}</span></td><td>${esc(t.kind)}</td><td>${esc(t.target_connector||'–')}</td><td class="small mono">${esc(t.body.slice(0,180))}</td><td>${t.render_total}</td><td><button onclick="editTemplate('${esc(t.template_id)}')">Edit</button> <button onclick="previewTemplate('${esc(t.template_id)}')">Test</button> <button class="danger" onclick="deleteTemplate('${esc(t.template_id)}')">Löschen</button></td></tr>`).join('');
$('deliveryRows').innerHTML=deliveries.map(d=>`<tr><td class="mono small">${esc(d.delivery_id.slice(0,12))}<br>${esc(d.created_at)}</td><td>${esc(d.connector_id)}</td><td>${esc(d.event_type)}<br>→ ${esc(d.destination||'–')}</td><td>${pill(d.state)}</td><td>${d.attempts}/${d.max_attempts}<br>${esc(d.next_attempt_at)}</td><td class="small">${esc(d.last_error||d.response_excerpt||'')}</td><td><button onclick="deliveryAction('${d.delivery_id}','retry')">Retry</button> <button onclick="deliveryAction('${d.delivery_id}','requeue')">Neu</button> <button class="danger" onclick="deliveryAction('${d.delivery_id}','cancel')">Stop</button></td></tr>`).join('');
$('ttsRows').innerHTML=tts.map(j=>`<tr><td><b>${esc(j.name)}</b><br><span class="mono small">${esc(j.job_id.slice(0,12))}</span></td><td>${esc(j.voice)} / ${j.speed}<br>${esc(j.rendered_text.slice(0,120))}</td><td>${pill(j.state)}<br>${esc(j.last_error||'')}</td><td>${j.artifact_url?`<a href="${esc(j.artifact_url)}">WAV</a><br>${esc(j.artifact_sha256||'')}`:'–'}</td><td>${esc(j.destination_kind||'–')} ${esc(j.destination_id||'')}</td><td>${j.state==='ready'?`<button class="purple" onclick="publishTts('${j.job_id}')">Media Library</button>`:''}</td></tr>`).join('');
$('auditRows').textContent=audit.map(x=>`${x.timestamp} #${x.sequence} ${x.actor} ${x.category}.${x.action} ${x.object_id} ${x.result}`).join('\n');$('backupRows').textContent=backups.map(x=>`${x.created_at} ${x.backup_id}\n${x.path}\nSHA ${x.state_sha256}\nSecrets: ${x.includes_secrets?'JA':'nein'}`).join('\n\n');
const co=options(connectors,'connector_id','display_name','<option value="">Routingregeln verwenden</option>');$('dConnector').innerHTML=co;$('rConnector').innerHTML=options(connectors,'connector_id','display_name');$('pConnector').innerHTML=options(connectors,'connector_id','display_name','<option value="">keiner</option>');const to=options(templates,'template_id','name','<option value="">keine</option>');$('dTemplate').innerHTML=to;$('rTemplate').innerHTML=to;$('tTemplate').innerHTML=options(templates.filter(x=>x.kind==='tts'||x.kind==='text'),'template_id','name','<option value="">ohne Vorlage</option>')}
function openConnector(c=null){connectorDialog.dataset.edit=c?.connector_id||'';cId.value=c?.connector_id||'';cId.disabled=!!c;cName.value=c?.display_name||'';cKind.value=c?.kind||'generic_webhook';cDirection.value=c?.direction||'outbound';cEndpoint.value=c?.endpoint||'';cHealth.value=c?.health_endpoint||'';cRate.value=c?.rate_limit_per_minute||60;cTimeout.value=c?.timeout_ms||5000;cSettings.value=JSON.stringify(c?.settings||{},null,2);cSecrets.value=(c?.required_secrets||[]).map(x=>typeof x==='string'?x:x.name).join(',');connectorDialog.showModal()}function editConnector(id){openConnector(data.connectors.find(x=>x.connector_id===id))}
async function saveConnector(e){e.preventDefault();const edit=connectorDialog.dataset.edit;const p={connector_id:cId.value,display_name:cName.value,kind:cKind.value,direction:cDirection.value,endpoint:cEndpoint.value,health_endpoint:cHealth.value||null,enabled:edit?data.connectors.find(x=>x.connector_id===edit).enabled:false,timeout_ms:Number(cTimeout.value),rate_limit_per_minute:Number(cRate.value),circuit_failure_threshold:5,circuit_open_secs:60,required_secrets:cSecrets.value.split(',').map(x=>x.trim()).filter(Boolean),settings:JSON.parse(cSettings.value||'{}'),actor:'webui'};await api(edit?'/api/v1/connectors/'+edit:'/api/v1/connectors',{method:edit?'PUT':'POST',body:JSON.stringify(p)});connectorDialog.close();refresh()}
async function connectorAction(id,a){await api(`/api/v1/connectors/${id}/${a}`,{method:'POST',body:'{}'});refresh()}async function setSecret(id){const name=prompt('Secret-Name (z.B. bot_token, api_key, bearer_token):');if(!name)return;const value=prompt('Neuer Secret-Wert (wird nicht wieder angezeigt):');if(value===null||value==='')return;await api(`/api/v1/connectors/${id}/secrets`,{method:'POST',body:JSON.stringify({name,value,actor:'webui'})});refresh()}
function openRule(r=null){ruleDialog.dataset.edit=r?.rule_id||'';rId.value=r?.rule_id||'';rId.disabled=!!r;rName.value=r?.name||'';rSource.value=r?.source_connector||'manual';rType.value=r?.event_type||'sds.message';rConnector.value=r?.target_connector||data.connectors[0]?.connector_id||'';rTemplate.value=r?.template_id||'';rPriority.value=r?.priority||0;rContains.value=r?.text_contains||'';rDestination.value=r?.destination||'';rStop.checked=!!r?.stop_processing;ruleDialog.showModal()}function editRule(id){openRule(data.rules.find(x=>x.rule_id===id))}
async function saveRule(e){e.preventDefault();const edit=ruleDialog.dataset.edit;const p={rule_id:rId.value,name:rName.value,enabled:edit?data.rules.find(x=>x.rule_id===edit).enabled:true,priority:Number(rPriority.value),source_connector:rSource.value,event_type:rType.value,text_contains:rContains.value||null,target_connector:rConnector.value,template_id:rTemplate.value||null,destination:rDestination.value||null,stop_processing:rStop.checked,actor:'webui'};await api(edit?'/api/v1/rules/'+edit:'/api/v1/rules',{method:edit?'PUT':'POST',body:JSON.stringify(p)});ruleDialog.close();refresh()}async function ruleAction(id,a){await api(`/api/v1/rules/${id}/${a}`,{method:'POST',body:'{}'});refresh()}async function deleteRule(id){if(confirm('Regel löschen?')){await api('/api/v1/rules/'+id,{method:'DELETE',body:'{}'});refresh()}}
function openTemplate(t=null){templateDialog.dataset.edit=t?.template_id||'';pId.value=t?.template_id||'';pId.disabled=!!t;pName.value=t?.name||'';pKind.value=t?.kind||'text';pConnector.value=t?.target_connector||'';pBody.value=t?.body||'{{text}}';pDescription.value=t?.description||'';templateDialog.showModal()}function editTemplate(id){openTemplate(data.templates.find(x=>x.template_id===id))}
async function saveTemplate(e){e.preventDefault();const edit=templateDialog.dataset.edit;const p={template_id:pId.value,name:pName.value,kind:pKind.value,body:pBody.value,content_type:pKind.value==='json'?'application/json':'text/plain; charset=utf-8',enabled:true,target_connector:pConnector.value||null,default_destination:null,description:pDescription.value,actor:'webui'};await api(edit?'/api/v1/templates/'+edit:'/api/v1/templates',{method:edit?'PUT':'POST',body:JSON.stringify(p)});templateDialog.close();refresh()}async function previewTemplate(id){const out=await api(`/api/v1/templates/${id}/render`,{method:'POST',body:JSON.stringify({text:'Probe & <Test>',destination:'2000',source:'webui',event_type:'preview',payload:{foo:'bar'}})});alert(JSON.stringify(out,null,2))}async function deleteTemplate(id){if(confirm('Vorlage löschen?')){await api('/api/v1/templates/'+id,{method:'DELETE',body:'{}'});refresh()}}
async function sendDispatch(e){e.preventDefault();const target=dConnector.value?[dConnector.value]:[];const out=await api('/api/v1/dispatch',{method:'POST',body:JSON.stringify({source_connector:'manual',event_type:dType.value,destination:dDestination.value||null,text:dText.value,payload:JSON.parse(dPayload.value||'{}'),target_connectors:target,template_id:dTemplate.value||null,priority:3,ttl_secs:300,actor:'webui'})});dispatchOutput.textContent=JSON.stringify(out,null,2);refresh()}
async function createTts(e){e.preventDefault();const out=await api('/api/v1/tts/jobs',{method:'POST',body:JSON.stringify({name:tName.value,text:tText.value,template_id:tTemplate.value||null,voice:tVoice.value,speed:Number(tSpeed.value),destination_kind:tDestKind.value,destination_id:Number(tDestId.value),priority:3,actor:'webui'})});ttsOutput.textContent=JSON.stringify(out,null,2);refresh()}async function publishTts(id){const out=await api(`/api/v1/tts/jobs/${id}/publish`,{method:'POST',body:JSON.stringify({actor:'webui'})});ttsOutput.textContent=JSON.stringify(out,null,2);refresh()}
async function deliveryAction(id,a){await api(`/api/v1/deliveries/${id}/${a}`,{method:'POST',body:'{}'});refresh()}async function processNow(){await api('/api/v1/maintenance/process-now',{method:'POST',body:'{}'});refresh()}async function maintenance(){const x=await api('/api/v1/maintenance/tick',{method:'POST',body:JSON.stringify({actor:'webui'})});alert(JSON.stringify(x,null,2));refresh()}async function backup(){await api('/api/v1/backups',{method:'POST',body:JSON.stringify({actor:'webui',note:'manual WebUI backup'})});refresh()}
initNav();setInterval(()=>$('clock').textContent=new Date().toLocaleString(),1000);refresh();setInterval(refresh,10000);
</script></body></html>"##;

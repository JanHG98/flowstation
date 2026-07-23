use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};
use tetra_entities::net_control::ControlCommand;

use crate::config::NodeGatewayConfig;
use crate::state::SharedGateway;

#[derive(Debug)]
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
}

impl HttpResponse {
    fn json(status: u16, value: &impl serde::Serialize) -> Self {
        Self {
            status,
            content_type: "application/json; charset=utf-8",
            body: serde_json::to_vec_pretty(value).unwrap_or_else(|_| b"{\"error\":\"serialization failed\"}".to_vec()),
        }
    }

    fn html(status: u16, body: &'static str) -> Self {
        Self { status, content_type: "text/html; charset=utf-8", body: body.as_bytes().to_vec() }
    }

    fn text(status: u16, content_type: &'static str, body: String) -> Self {
        Self { status, content_type, body: body.into_bytes() }
    }
}

#[derive(Debug, Deserialize)]
struct CommandRequest {
    #[serde(default)]
    operator_id: Option<String>,
    command: ControlCommand,
}

pub fn handle_http_stream(mut stream: TcpStream, gateway: SharedGateway, config: NodeGatewayConfig) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let response = match read_http_request(&mut stream, config.limits.max_http_body_bytes) {
        Ok(request) => route(request, &gateway, &config),
        Err(error) => HttpResponse::json(400, &json!({ "error": error })),
    };
    let _ = write_response(&mut stream, response);
}

fn route(request: HttpRequest, gateway: &SharedGateway, config: &NodeGatewayConfig) -> HttpResponse {
    if request.method == "OPTIONS" {
        return HttpResponse::text(204, "text/plain", String::new());
    }

    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") => HttpResponse::html(200, INDEX_HTML),
        ("GET", "/health/live") => HttpResponse::json(200, &json!({
            "ok": true,
            "service": "netcore-node-gateway",
            "security_mode": "open_lab",
        })),
        ("GET", "/health/ready") => HttpResponse::json(200, &json!({
            "ok": true,
            "ready": true,
            "listener": config.server.bind,
            "security_mode": "open_lab",
        })),
        ("GET", "/api/v1/status") => HttpResponse::json(200, &gateway.status()),
        ("GET", "/api/v1/nodes") => HttpResponse::json(200, &gateway.nodes()),
        ("GET", "/api/v1/events") => {
            let limit = request.query.get("limit").and_then(|value| value.parse::<usize>().ok()).unwrap_or(100).min(1_000);
            HttpResponse::json(200, &gateway.recent_events(limit))
        }
        ("GET", "/api/v1/config") => HttpResponse::json(200, &json!({
            "server": &config.server,
            "security": &config.security,
            "limits": &config.limits,
            "effective_warning": "NO AUTHENTICATION, NO TOKENS, NO TLS - TEST NETWORK ONLY"
        })),
        ("GET", "/metrics") => HttpResponse::text(200, "text/plain; version=0.0.4; charset=utf-8", gateway.metrics()),
        ("GET", "/openapi.json") => HttpResponse::json(200, &openapi(config)),
        _ => route_dynamic(request, gateway),
    }
}

fn route_dynamic(request: HttpRequest, gateway: &SharedGateway) -> HttpResponse {
    let Some((node_id, action)) = parse_node_route(&request.path) else {
        return HttpResponse::json(404, &json!({
            "error": "not found",
            "available": [
                "GET /", "GET /health/live", "GET /health/ready", "GET /api/v1/status",
                "GET /api/v1/nodes", "GET /api/v1/nodes/{node_id}", "GET /api/v1/events",
                "GET /api/v1/config", "GET /metrics", "GET /openapi.json",
                "POST /api/v1/nodes/{node_id}/ping", "POST /api/v1/nodes/{node_id}/disconnect",
                "POST /api/v1/nodes/{node_id}/commands"
            ]
        }));
    };

    match (request.method.as_str(), action.as_deref()) {
        ("GET", None) => match gateway.node(&node_id) {
            Some(node) => HttpResponse::json(200, &node),
            None => HttpResponse::json(404, &json!({ "error": "unknown node", "node_id": node_id })),
        },
        ("POST", Some("ping")) => action_response(gateway.ping_node(&node_id)),
        ("POST", Some("disconnect")) => action_response(gateway.disconnect_node(&node_id)),
        ("POST", Some("commands")) => {
            let parsed = serde_json::from_slice::<CommandRequest>(&request.body)
                .map_err(|error| format!("invalid command json: {error}"))
                .and_then(|body| gateway.send_command(&node_id, body.command, body.operator_id));
            match parsed {
                Ok(command_id) => HttpResponse::json(202, &json!({ "ok": true, "command_id": command_id, "node_id": node_id })),
                Err(error) => HttpResponse::json(400, &json!({ "ok": false, "error": error })),
            }
        }
        _ => HttpResponse::json(404, &json!({ "error": "unknown node action", "node_id": node_id, "action": action })),
    }
}

fn action_response(result: Result<(), String>) -> HttpResponse {
    match result {
        Ok(()) => HttpResponse::json(202, &json!({ "ok": true })),
        Err(error) => HttpResponse::json(409, &json!({ "ok": false, "error": error })),
    }
}

fn parse_node_route(path: &str) -> Option<(String, Option<String>)> {
    let tail = path.strip_prefix("/api/v1/nodes/")?;
    let mut parts = tail.split('/');
    let node_id = parts.next()?.trim();
    if node_id.is_empty() {
        return None;
    }
    let action = parts.next().map(str::to_string);
    if parts.next().is_some() {
        return None;
    }
    Some((percentish_decode(node_id), action))
}

fn openapi(config: &NodeGatewayConfig) -> Value {
    json!({
        "openapi": "3.0.3",
        "info": { "title": "NetCore Node Gateway API", "version": "1.0.0-open-lab" },
        "servers": [{ "url": format!("http://{}", config.server.bind) }],
        "x-netcore-security-mode": "open_lab",
        "paths": {
            "/health/live": { "get": { "summary": "Liveness" } },
            "/health/ready": { "get": { "summary": "Readiness" } },
            "/api/v1/status": { "get": { "summary": "Gateway status" } },
            "/api/v1/nodes": { "get": { "summary": "Known TBS nodes" } },
            "/api/v1/nodes/{node_id}": { "get": { "summary": "Node detail" } },
            "/api/v1/nodes/{node_id}/ping": { "post": { "summary": "Queue application ping" } },
            "/api/v1/nodes/{node_id}/disconnect": { "post": { "summary": "Disconnect node" } },
            "/api/v1/nodes/{node_id}/commands": { "post": { "summary": "Queue ControlCommand" } },
            "/api/v1/events": { "get": { "summary": "Recent gateway events" } },
            "/metrics": { "get": { "summary": "Prometheus metrics" } }
        }
    })
}

fn read_http_request(stream: &mut TcpStream, max_body_bytes: usize) -> Result<HttpRequest, String> {
    let mut buffer = Vec::with_capacity(8_192);
    let mut chunk = [0u8; 4_096];
    let header_end = loop {
        let read = stream.read(&mut chunk).map_err(|error| format!("request read failed: {error}"))?;
        if read == 0 {
            return Err("connection closed before request was complete".to_string());
        }
        buffer.extend_from_slice(&chunk[..read]);
        if buffer.len() > max_body_bytes + 65_536 {
            return Err("request too large".to_string());
        }
        if let Some(position) = find_subslice(&buffer, b"\r\n\r\n") {
            break position + 4;
        }
    };

    let header_text = std::str::from_utf8(&buffer[..header_end]).map_err(|_| "request headers are not utf-8".to_string())?;
    let mut lines = header_text.split("\r\n");
    let request_line = lines.next().ok_or_else(|| "missing request line".to_string())?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next().ok_or_else(|| "missing method".to_string())?.to_ascii_uppercase();
    let raw_path = request_parts.next().ok_or_else(|| "missing path".to_string())?;
    let (path, query) = parse_path_and_query(raw_path);

    let mut content_length = 0usize;
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse::<usize>().map_err(|_| "invalid content-length".to_string())?;
            }
        }
    }
    if content_length > max_body_bytes {
        return Err("body too large".to_string());
    }

    let mut body = buffer[header_end..].to_vec();
    while body.len() < content_length {
        let read = stream.read(&mut chunk).map_err(|error| format!("body read failed: {error}"))?;
        if read == 0 {
            return Err("connection closed before body was complete".to_string());
        }
        body.extend_from_slice(&chunk[..read]);
        if body.len() > max_body_bytes {
            return Err("body too large".to_string());
        }
    }
    body.truncate(content_length);
    Ok(HttpRequest { method, path, query, body })
}

fn write_response(stream: &mut TcpStream, response: HttpResponse) -> std::io::Result<()> {
    let header = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nX-NetCore-Security-Mode: open-lab\r\nConnection: close\r\n\r\n",
        response.status,
        reason_phrase(response.status),
        response.content_type,
        response.body.len(),
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(&response.body)?;
    stream.flush()
}

pub fn looks_like_websocket_upgrade(peek: &[u8]) -> bool {
    let text = String::from_utf8_lossy(peek).to_ascii_lowercase();
    text.contains("upgrade: websocket") && text.contains("sec-websocket-key:")
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
}

fn parse_path_and_query(raw: &str) -> (String, HashMap<String, String>) {
    let mut parts = raw.splitn(2, '?');
    let path = parts.next().unwrap_or(raw).to_string();
    let query = parts
        .next()
        .map(|query| {
            query
                .split('&')
                .filter(|pair| !pair.is_empty())
                .map(|pair| {
                    let mut fields = pair.splitn(2, '=');
                    (
                        percentish_decode(fields.next().unwrap_or_default()),
                        percentish_decode(fields.next().unwrap_or("true")),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    (path, query)
}

fn percentish_decode(value: &str) -> String {
    value.replace('+', " ").replace("%20", " ")
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        202 => "Accepted",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        409 => "Conflict",
        500 => "Internal Server Error",
        _ => "OK",
    }
}

const INDEX_HTML: &str = r#"<!doctype html>
<html lang="de">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>NetCore Node Gateway</title>
<style>
:root{color-scheme:dark;--bg:#0b1220;--panel:#121d31;--panel2:#17243c;--text:#e9f0fb;--muted:#91a4c2;--ok:#4ade80;--warn:#facc15;--bad:#fb7185;--line:#2a3b58;--accent:#60a5fa}*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--text);font-family:Inter,system-ui,sans-serif}.wrap{max-width:1450px;margin:auto;padding:20px}.lab{background:#7f1d1d;border:2px solid #fb7185;padding:13px 18px;border-radius:12px;font-weight:800;margin-bottom:16px}h1{margin:.2rem 0}.sub{color:var(--muted);margin-top:4px}.cards{display:grid;grid-template-columns:repeat(auto-fit,minmax(180px,1fr));gap:12px;margin:18px 0}.card,.panel{background:var(--panel);border:1px solid var(--line);border-radius:12px;padding:16px}.value{font-size:2rem;font-weight:800}.label{color:var(--muted)}.panel{margin-top:14px;overflow:auto}table{width:100%;border-collapse:collapse;min-width:1050px}th,td{text-align:left;padding:10px;border-bottom:1px solid var(--line);vertical-align:top}th{color:var(--muted);font-size:.85rem}.pill{display:inline-block;padding:3px 8px;border-radius:99px;font-size:.8rem;font-weight:700}.online{background:#14532d;color:#bbf7d0}.offline{background:#4c0519;color:#fecdd3}.stale{background:#713f12;color:#fef08a}button{border:0;border-radius:8px;padding:8px 10px;margin:2px;background:var(--accent);color:#07111f;font-weight:700;cursor:pointer}.danger{background:var(--bad)}pre{white-space:pre-wrap;color:#c9d7ed;font-size:.8rem}.toolbar{display:flex;gap:8px;align-items:center;justify-content:space-between}.small{font-size:.8rem;color:var(--muted)}a{color:#93c5fd}</style>
</head>
<body><div class="wrap">
<div class="lab">⚠ OFFENER TESTMODUS: KEINE AUTHENTIFIZIERUNG, KEINE TOKENS, KEIN TLS. Nur im isolierten Testnetz verwenden.</div>
<div class="toolbar"><div><h1>NetCore Node Gateway</h1><div class="sub">Zentrale TBS-Annahme, Backend-Transport und Verwaltungs-WebUI</div></div><button onclick="refreshAll()">Aktualisieren</button></div>
<div id="cards" class="cards"></div>
<div class="panel"><h2>Basisstationen</h2><table><thead><tr><th>Status</th><th>Node</th><th>Zelle</th><th>Version</th><th>Letzter Kontakt</th><th>Zähler</th><th>Fähigkeiten</th><th>Aktionen</th></tr></thead><tbody id="nodes"></tbody></table></div>
<div class="panel"><h2>Letzte Gateway-Ereignisse</h2><pre id="events">Lade…</pre></div>
<div class="panel small">API: <a href="/openapi.json">OpenAPI</a> · <a href="/metrics">Metriken</a> · <a href="/api/v1/config">Effektive Konfiguration</a></div>
</div>
<script>
const esc=v=>String(v??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
async function getj(url){const r=await fetch(url);if(!r.ok)throw new Error(await r.text());return r.json()}
async function post(url,body){const r=await fetch(url,{method:'POST',headers:{'Content-Type':'application/json'},body:body?JSON.stringify(body):''});const t=await r.text();if(!r.ok)throw new Error(t);return t?JSON.parse(t):{}}
function card(label,value){return `<div class="card"><div class="value">${esc(value)}</div><div class="label">${esc(label)}</div></div>`}
function statusPill(n){if(n.stale)return '<span class="pill stale">STALE</span>';return n.connected?'<span class="pill online">ONLINE</span>':'<span class="pill offline">OFFLINE</span>'}
function caps(c){return Object.entries(c||{}).filter(([,v])=>v===true).map(([k])=>k).join(', ')}
async function action(id,name){if(name==='disconnect'&&!confirm(`Node ${id} wirklich trennen?`))return;try{await post(`/api/v1/nodes/${encodeURIComponent(id)}/${name}`);setTimeout(refreshAll,300)}catch(e){alert(e.message)}}
async function refreshAll(){try{const [s,n,e]=await Promise.all([getj('/api/v1/status'),getj('/api/v1/nodes'),getj('/api/v1/events?limit=40')]);document.getElementById('cards').innerHTML=[card('Verbunden',s.connected_nodes),card('Bekannt',s.known_nodes),card('Stale',s.stale_nodes),card('Backend-Clients',s.backend_clients),card('Nachrichten',s.total_node_messages),card('Mediaframes',s.total_media_frames),card('Kommandos',s.total_commands)].join('');document.getElementById('nodes').innerHTML=n.map(x=>`<tr><td>${statusPill(x)}</td><td><b>${esc(x.identity.station_name)}</b><br><span class="small">${esc(x.node_id)}<br>${esc(x.peer)}</span></td><td>MCC ${esc(x.identity.mcc)} / MNC ${esc(x.identity.mnc)}<br>LA ${esc(x.identity.location_area)}, CC ${esc(x.identity.colour_code)}<br>Carrier ${esc(x.identity.main_carrier)}${x.identity.secondary_carrier?` / ${esc(x.identity.secondary_carrier)}`:''}</td><td>${esc(x.identity.stack_version)}</td><td>${esc(x.last_seen)}<br><span class="small">${esc(x.last_message_kind)}</span></td><td>Msg ${esc(x.message_count)}<br>Tel ${esc(x.telemetry_count)}<br>Ack ${esc(x.control_ack_count)}<br>Media ${esc(x.media_frame_count)}</td><td class="small">${esc(caps(x.capabilities))}</td><td><button onclick="action('${esc(x.node_id)}','ping')">Ping</button><button class="danger" onclick="action('${esc(x.node_id)}','disconnect')">Trennen</button></td></tr>`).join('')||'<tr><td colspan="8">Noch keine TBS verbunden.</td></tr>';document.getElementById('events').textContent=e.map(x=>`${x.timestamp} #${x.seq} ${x.kind}${x.node_id?' ['+x.node_id+']':''} ${JSON.stringify(x.detail)}`).join('\n')||'Noch keine Ereignisse.'}catch(e){document.getElementById('events').textContent='Fehler: '+e.message}}
refreshAll();setInterval(refreshAll,5000);
</script></body></html>"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_node_routes() {
        assert_eq!(parse_node_route("/api/v1/nodes/tbs-a"), Some(("tbs-a".to_string(), None)));
        assert_eq!(parse_node_route("/api/v1/nodes/tbs-a/ping"), Some(("tbs-a".to_string(), Some("ping".to_string()))));
    }
}

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::Sender;
use std::thread;

use serde::Serialize;
use serde_json::json;

use crate::config::SdsRouterConfig;
use crate::protocol::BackendRequest;
use crate::state::{
    ApplicationAckInput, MessageInput, RouteInput, SharedSdsRouter, parse_message_state,
};

pub fn spawn_http_server(
    config: SdsRouterConfig,
    router: SharedSdsRouter,
    gateway_tx: Sender<BackendRequest>,
) -> std::io::Result<thread::JoinHandle<()>> {
    let listener = TcpListener::bind(config.server.bind)?;
    tracing::info!(
        "SDS Router WebUI/API listening on http://{}",
        config.server.bind
    );
    Ok(thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let router = router.clone();
                    let gateway_tx = gateway_tx.clone();
                    let config = config.clone();
                    thread::spawn(move || {
                        if let Err(error) =
                            handle_connection(stream, router, gateway_tx, config)
                        {
                            tracing::warn!("HTTP connection failed: {}", error);
                        }
                    });
                }
                Err(error) => tracing::warn!("HTTP accept failed: {}", error),
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
    router: SharedSdsRouter,
    gateway_tx: Sender<BackendRequest>,
    config: SdsRouterConfig,
) -> Result<(), String> {
    let request = read_request(&mut stream, config.limits.max_body_bytes)?;
    let response = route(request, router, gateway_tx, config);
    write_response(&mut stream, response).map_err(|error| error.to_string())
}

fn route(
    request: HttpRequest,
    router: SharedSdsRouter,
    gateway_tx: Sender<BackendRequest>,
    config: SdsRouterConfig,
) -> HttpResponse {
    if request.method == "OPTIONS" {
        return empty(204);
    }

    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") => html(INDEX_HTML),
        ("GET", "/health/live") => json_response(200, &json!({"status":"live"})),
        ("GET", "/health/ready") => {
            let status = router.status();
            json_response(if status.node_gateway_connected { 200 } else { 503 }, &status)
        }
        ("GET", "/api/v1/status") => json_response(200, &router.status()),
        ("GET", "/api/v1/config") => json_response(200, &config),
        ("GET", "/api/v1/nodes") => json_response(200, &router.nodes()),
        ("GET", "/api/v1/subscribers") => json_response(200, &router.subscribers()),
        ("GET", "/api/v1/groups") => json_response(200, &router.groups()),
        ("GET", "/api/v1/routes") => json_response(200, &router.routes()),
        ("GET", "/api/v1/events") => {
            let limit = query_usize(&request, "limit", 100, 2_000);
            json_response(200, &router.recent_events(limit))
        }
        ("GET", "/api/v1/messages") => {
            let limit = query_usize(&request, "limit", 250, 5_000);
            let state = request
                .query
                .get("state")
                .and_then(|value| parse_message_state(value));
            json_response(200, &router.messages(limit, state))
        }
        ("GET", "/api/v1/application-outbox") => {
            let limit = query_usize(&request, "limit", 250, 2_000);
            let application = request.query.get("application").map(String::as_str);
            json_response(200, &router.application_outbox(application, limit))
        }
        ("GET", "/api/v1/export.json") => {
            let value = json!({
                "status": router.status(),
                "routes": router.routes(),
                "messages": router.messages(5000, None),
                "nodes": router.nodes(),
                "subscribers": router.subscribers(),
                "groups": router.groups(),
            });
            download_json("netcore-sds-router-export.json", &value)
        }
        ("GET", "/metrics") => text(
            "text/plain; version=0.0.4; charset=utf-8",
            router.metrics(),
        ),
        ("GET", "/openapi.json") => json_response(200, &openapi()),
        ("POST", "/api/v1/messages") => {
            match parse_json::<MessageInput>(&request.body)
                .and_then(|input| router.create_message(input))
            {
                Ok((message, commands)) => {
                    dispatch_response(&gateway_tx, commands, 201, &message)
                }
                Err(error) => json_response(409, &json!({"error": error})),
            }
        }
        ("POST", "/api/v1/routes") => {
            match parse_json::<RouteInput>(&request.body)
                .and_then(|input| router.create_route(input))
            {
                Ok(route) => json_response(201, &route),
                Err(error) => json_response(409, &json!({"error": error})),
            }
        }
        _ => dynamic_route(request, router, gateway_tx),
    }
}

fn dynamic_route(
    request: HttpRequest,
    router: SharedSdsRouter,
    gateway_tx: Sender<BackendRequest>,
) -> HttpResponse {
    let parts: Vec<&str> = request.path.trim_matches('/').split('/').collect();
    match (request.method.as_str(), parts.as_slice()) {
        ("GET", ["api", "v1", "messages", id]) => match router.message(id) {
            Some(message) => json_response(200, &message),
            None => json_response(404, &json!({"error":"message not found"})),
        },
        ("POST", ["api", "v1", "messages", id, "retry"]) => {
            match router.retry_message(id) {
                Ok(commands) => dispatch_response(
                    &gateway_tx,
                    commands,
                    202,
                    &json!({"message_id":id,"queued":true}),
                ),
                Err(error) => json_response(409, &json!({"error":error})),
            }
        }
        ("POST", ["api", "v1", "messages", id, "requeue"]) => {
            match router.requeue_message(id) {
                Ok((message, commands)) => {
                    dispatch_response(&gateway_tx, commands, 202, &message)
                }
                Err(error) => json_response(409, &json!({"error":error})),
            }
        }
        ("POST", ["api", "v1", "messages", id, "cancel"]) => {
            match router.cancel_message(id) {
                Ok(()) => json_response(200, &json!({"message_id":id,"cancelled":true})),
                Err(error) => json_response(409, &json!({"error":error})),
            }
        }
        ("DELETE", ["api", "v1", "messages", id]) => match router.delete_message(id) {
            Ok(()) => empty(204),
            Err(error) => json_response(409, &json!({"error":error})),
        },
        ("PUT", ["api", "v1", "routes", id]) => {
            match parse_json::<RouteInput>(&request.body)
                .and_then(|input| router.update_route(id, input))
            {
                Ok(route) => json_response(200, &route),
                Err(error) => json_response(409, &json!({"error":error})),
            }
        }
        ("DELETE", ["api", "v1", "routes", id]) => match router.delete_route(id) {
            Ok(()) => empty(204),
            Err(error) => json_response(409, &json!({"error":error})),
        },
        ("POST", ["api", "v1", "application-outbox", application, id, "ack"]) => {
            match parse_json::<ApplicationAckInput>(&request.body)
                .and_then(|input| router.acknowledge_application(id, application, input))
            {
                Ok(message) => json_response(200, &message),
                Err(error) => json_response(409, &json!({"error":error})),
            }
        }
        _ => json_response(404, &json!({"error":"not found"})),
    }
}

fn dispatch_response<T: Serialize>(
    tx: &Sender<BackendRequest>,
    commands: Vec<BackendRequest>,
    status: u16,
    value: &T,
) -> HttpResponse {
    match dispatch_all(tx, commands) {
        Ok(()) => json_response(status, value),
        Err(error) => json_response(503, &json!({"error":error})),
    }
}

fn dispatch_all(
    tx: &Sender<BackendRequest>,
    commands: Vec<BackendRequest>,
) -> Result<(), String> {
    for command in commands {
        tx.send(command)
            .map_err(|_| "node gateway worker is unavailable".to_string())?;
    }
    Ok(())
}

fn parse_json<T: serde::de::DeserializeOwned>(body: &[u8]) -> Result<T, String> {
    serde_json::from_slice(body).map_err(|error| format!("invalid JSON: {error}"))
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
            "title":"NetCore SDS Router",
            "version":env!("CARGO_PKG_VERSION"),
            "description":"OPEN LAB API. No authentication, no token and no TLS."
        },
        "paths":{
            "/api/v1/status":{"get":{}},
            "/api/v1/messages":{"get":{},"post":{}},
            "/api/v1/messages/{id}":{"get":{},"delete":{}},
            "/api/v1/messages/{id}/retry":{"post":{}},
            "/api/v1/messages/{id}/requeue":{"post":{}},
            "/api/v1/messages/{id}/cancel":{"post":{}},
            "/api/v1/routes":{"get":{},"post":{}},
            "/api/v1/routes/{id}":{"put":{},"delete":{}},
            "/api/v1/application-outbox":{"get":{}},
            "/api/v1/application-outbox/{application}/{id}/ack":{"post":{}},
            "/api/v1/nodes":{"get":{}},
            "/api/v1/subscribers":{"get":{}},
            "/api/v1/groups":{"get":{}},
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

fn download_json<T: Serialize>(name: &str, value: &T) -> HttpResponse {
    match serde_json::to_vec_pretty(value) {
        Ok(body) => HttpResponse {
            status: 200,
            content_type: "application/json; charset=utf-8",
            body,
            disposition: Some(format!("attachment; filename=\"{name}\"")),
        },
        Err(error) => json_response(500, &json!({"error": error.to_string()})),
    }
}

fn text(content_type: &'static str, value: String) -> HttpResponse {
    HttpResponse {
        status: 200,
        content_type,
        body: value.into_bytes(),
        disposition: None,
    }
}

fn html(value: &str) -> HttpResponse {
    HttpResponse {
        status: 200,
        content_type: "text/html; charset=utf-8",
        body: value.as_bytes().to_vec(),
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
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 8192];
    let header_end;
    loop {
        let read = stream.read(&mut buffer).map_err(|error| error.to_string())?;
        if read == 0 {
            return Err("connection closed before request headers".to_string());
        }
        bytes.extend_from_slice(&buffer[..read]);
        if bytes.len() > max_body_bytes.saturating_add(64 * 1024) {
            return Err("request exceeds configured limit".to_string());
        }
        if let Some(index) = find_subslice(&bytes, b"\r\n\r\n") {
            header_end = index + 4;
            break;
        }
    }

    let headers = String::from_utf8_lossy(&bytes[..header_end]);
    let mut lines = headers.lines();
    let first = lines.next().ok_or_else(|| "missing request line".to_string())?;
    let mut request_line = first.split_whitespace();
    let method = request_line
        .next()
        .ok_or_else(|| "missing method".to_string())?
        .to_string();
    let raw_path = request_line
        .next()
        .ok_or_else(|| "missing path".to_string())?;
    let content_length = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim().parse::<usize>().ok())
        .unwrap_or(0);
    if content_length > max_body_bytes {
        return Err("request body exceeds configured limit".to_string());
    }
    while bytes.len() < header_end + content_length {
        let read = stream.read(&mut buffer).map_err(|error| error.to_string())?;
        if read == 0 {
            return Err("connection closed before request body".to_string());
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    let (path, query) = parse_path_and_query(raw_path);
    Ok(HttpRequest {
        method,
        path,
        query,
        body: bytes[header_end..header_end + content_length].to_vec(),
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
        405 => "Method Not Allowed",
        409 => "Conflict",
        413 => "Payload Too Large",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "OK",
    };
    let mut headers = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET,POST,PUT,DELETE,OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nX-NetCore-Security-Mode: open_lab\r\nConnection: close\r\n",
        response.status,
        reason,
        response.content_type,
        response.body.len()
    );
    if let Some(disposition) = response.disposition {
        headers.push_str(&format!("Content-Disposition: {disposition}\r\n"));
    }
    headers.push_str("\r\n");
    stream.write_all(headers.as_bytes())?;
    stream.write_all(&response.body)
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn parse_path_and_query(raw: &str) -> (String, HashMap<String, String>) {
    let (path, raw_query) = raw.split_once('?').unwrap_or((raw, ""));
    let query = raw_query
        .split('&')
        .filter(|value| !value.is_empty())
        .map(|item| item.split_once('=').unwrap_or((item, "")))
        .map(|(key, value)| (percent_decode(key), percent_decode(value)))
        .collect();
    (percent_decode(path), query)
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(hex) = u8::from_str_radix(&value[index + 1..index + 3], 16) {
                output.push(hex);
                index += 3;
                continue;
            }
        }
        output.push(if bytes[index] == b'+' {
            b' '
        } else {
            bytes[index]
        });
        index += 1;
    }
    String::from_utf8_lossy(&output).into_owned()
}

const INDEX_HTML: &str = r#"<!doctype html>
<html lang="de"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>NetCore SDS Router</title><style>
:root{font-family:Inter,system-ui,sans-serif;color:#e8edf4;background:#0b1018}*{box-sizing:border-box}body{margin:0}header{padding:20px 28px;background:#121a26;border-bottom:1px solid #293449;position:sticky;top:0;z-index:2}h1{margin:0;font-size:22px}main{padding:24px;max-width:1600px;margin:auto}.warn{padding:10px 14px;background:#4b3512;border:1px solid #8c6628;border-radius:8px;margin-top:12px}.cards{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:12px;margin:18px 0}.card,.panel{background:#121a26;border:1px solid #293449;border-radius:10px;padding:16px}.value{font-size:28px;font-weight:700}.muted{color:#99a7ba}.ok{color:#5dd39e}.bad{color:#ff7474}.toolbar{display:flex;gap:8px;flex-wrap:wrap;align-items:center;margin:12px 0}button,input,select,textarea{background:#0d1420;color:#e8edf4;border:1px solid #40506a;border-radius:6px;padding:9px}button{cursor:pointer}button.primary{background:#1769aa}button.danger{background:#7a2631}table{width:100%;border-collapse:collapse;font-size:13px}th,td{text-align:left;padding:9px;border-bottom:1px solid #293449;vertical-align:top}pre{max-height:330px;overflow:auto;white-space:pre-wrap}.grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(210px,1fr));gap:10px}.wide{grid-column:1/-1}dialog{background:#121a26;color:#e8edf4;border:1px solid #40506a;border-radius:10px;max-width:900px;width:94%}label{display:flex;flex-direction:column;gap:5px}@media(max-width:700px){main{padding:12px}.panel{overflow:auto}}
</style></head><body><header><h1>NetCore-Tetra · SDS Router</h1><div id="gateway" class="muted">verbinde …</div><div class="warn"><b>OPEN LAB:</b> keine Tokens, keine Anmeldung und kein TLS. Nur im isolierten Testnetz betreiben.</div></header><main>
<div id="cards" class="cards"></div>
<section class="panel"><div class="toolbar"><h2 style="margin-right:auto">SDS / Status</h2><button class="primary" onclick="messageDialog.showModal()">Nachricht senden</button><select id="stateFilter" onchange="refresh()"><option value="">alle Zustände</option><option>queued</option><option>offline</option><option>in_flight</option><option>delivered</option><option>failed</option><option>dead_letter</option><option>expired</option></select></div><table><thead><tr><th>Zeit</th><th>Quelle → Ziel</th><th>Typ</th><th>Inhalt</th><th>Zustand</th><th>Legs</th><th>Aktionen</th></tr></thead><tbody id="messageRows"></tbody></table></section>
<section class="panel"><div class="toolbar"><h2 style="margin-right:auto">Routingregeln</h2><button class="primary" onclick="openRoute()">Regel anlegen</button></div><table><thead><tr><th>Name</th><th>Match</th><th>Ziel</th><th>Modus</th><th>Aktiv</th><th>Aktionen</th></tr></thead><tbody id="routeRows"></tbody></table></section>
<section class="panel"><h2>Präsenz</h2><div class="grid"><div><h3>TBS</h3><pre id="nodes"></pre></div><div><h3>Teilnehmer</h3><pre id="subscribers"></pre></div><div><h3>Gruppen</h3><pre id="groups"></pre></div></div></section>
<section class="panel"><h2>Ereignisse</h2><pre id="events"></pre></section>
</main>
<dialog id="messageDialog"><form id="messageForm" class="grid" onsubmit="sendMessage(event)"><h2 class="wide">SDS / Status senden</h2><label>Quell-ISSI<input name="source_issi" type="number" min="1" max="16777215" required></label><label>Ziel ISSI/GSSI<input name="dest_issi" type="number" min="1" max="16777215" required></label><label>Adressart<select name="is_group"><option value="false">Einzel</option><option value="true">Gruppe</option></select></label><label>SDS-Typ<select name="sds_type" onchange="toggleStatus()"><option value="4">Type 4</option><option value="3">Type 3</option><option value="2">Type 2</option><option value="1">Type 1</option><option value="0">Status</option></select></label><label>Protocol-ID<input name="protocol_id" type="number" min="0" max="255" value="130"></label><label>Priorität<input name="priority" type="number" min="0" max="15" value="0"></label><label>TTL Sekunden<input name="ttl_secs" type="number" min="5" value="300"></label><label id="statusLabel">Statuscode<input name="status_code" type="number" min="0" max="65535"></label><label class="wide">Text<textarea name="text" rows="4"></textarea></label><label class="wide">Payload hex (alternativ zu Text)<textarea name="payload_hex" rows="3"></textarea></label><label class="wide">TBS erzwingen, kommasepariert<input name="force_nodes"></label><div class="wide toolbar"><button class="primary" type="submit">Einplanen</button><button type="button" onclick="messageDialog.close()">Abbrechen</button></div></form></dialog>
<dialog id="routeDialog"><form id="routeForm" class="grid" onsubmit="saveRoute(event)"><h2 class="wide">Routingregel</h2><label>Name<input name="name" required></label><label>Art<select name="kind"><option value="protocol">Protocol-ID</option><option value="individual">Einzelziel</option><option value="group">Gruppenziel</option></select></label><label>Matchwert<input name="match_value" type="number" min="0" max="16777215" required></label><label>Zielart<select name="target_kind"><option value="application">Anwendung</option><option value="node">TBS Node</option></select></label><label>Ziel<input name="target" required></label><label>Modus<select name="mode"><option value="route">Route</option><option value="tap">Tap</option><option value="intercept">Intercept</option></select></label><label><span>Aktiv</span><input name="enabled" type="checkbox" checked></label><label class="wide">Notizen<textarea name="notes"></textarea></label><div class="wide toolbar"><button class="primary" type="submit">Speichern</button><button type="button" onclick="routeDialog.close()">Abbrechen</button></div></form></dialog>
<script>
let messages=[],routes=[];const messageDialog=document.getElementById('messageDialog'),messageForm=document.getElementById('messageForm'),routeDialog=document.getElementById('routeDialog'),routeForm=document.getElementById('routeForm');
async function api(path,opt){const r=await fetch(path,opt);if(!r.ok&&r.status!==204){let e={};try{e=await r.json()}catch{}throw new Error(e.error||r.statusText)}return r.status===204?null:r.json()}
function esc(v){return String(v??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]))}
async function refresh(){try{const sf=document.getElementById('stateFilter').value;const [s,m,r,n,u,g,e]=await Promise.all([api('/api/v1/status'),api('/api/v1/messages?limit=300'+(sf?'&state='+sf:'')),api('/api/v1/routes'),api('/api/v1/nodes'),api('/api/v1/subscribers'),api('/api/v1/groups'),api('/api/v1/events?limit=80')]);messages=m;routes=r;document.getElementById('gateway').innerHTML=s.node_gateway_connected?'<span class="ok">● Node Gateway verbunden</span>':'<span class="bad">● Node Gateway getrennt</span>';document.getElementById('cards').innerHTML=[['Nachrichten',s.messages_total],['Wartend',s.queued],['Offline',s.offline],['In Flight',s.in_flight],['Zugestellt',s.delivered],['Dead Letter',s.dead_letter],['TBS online',s.nodes_connected],['Duplikate',s.duplicate_messages]].map(x=>`<div class="card"><div class="muted">${x[0]}</div><div class="value">${x[1]}</div></div>`).join('');renderMessages();renderRoutes();document.getElementById('nodes').textContent=n.map(x=>`${x.connected?'●':'○'} ${x.node_id} ${x.station_name}`).join('\n');document.getElementById('subscribers').textContent=u.map(x=>`${x.issi} → ${x.node_id}`).join('\n');document.getElementById('groups').textContent=g.map(x=>`${x.gssi} → ${x.nodes.join(', ')}`).join('\n');document.getElementById('events').textContent=e.map(x=>`${x.timestamp} #${x.seq} ${x.kind} ${x.node_id||''} ${x.message_id||''}`).join('\n')}catch(e){document.getElementById('gateway').innerHTML='<span class="bad">UI-Fehler: '+esc(e.message)+'</span>'}}
function renderMessages(){document.getElementById('messageRows').innerHTML=messages.map(m=>`<tr><td>${esc(m.created_at)}</td><td>${m.source_issi} → ${m.dest_issi}${m.is_group?' (G)':''}</td><td>${m.sds_type===0?'Status':'SDS-'+m.sds_type}<br><span class="muted">PID ${m.protocol_id}, P${m.priority}</span></td><td>${esc(m.text_preview)}<br><span class="muted">${esc(m.payload_hex)}</span></td><td>${esc(m.state)}</td><td>${m.delivered_legs}/${m.total_legs}</td><td><button onclick="showMessage('${m.id}')">Details</button> <button onclick="act('${m.id}','retry')">Retry</button> <button onclick="act('${m.id}','requeue')">Requeue</button> <button onclick="act('${m.id}','cancel')">Stop</button></td></tr>`).join('')}
function renderRoutes(){document.getElementById('routeRows').innerHTML=routes.map(r=>`<tr><td>${esc(r.name)}</td><td>${r.kind}: ${r.match_value}</td><td>${r.target_kind}: ${esc(r.target)}</td><td>${r.mode}</td><td>${r.enabled?'ja':'nein'}</td><td><button onclick="editRoute('${r.id}')">Bearbeiten</button> <button class="danger" onclick="deleteRoute('${r.id}')">Löschen</button></td></tr>`).join('')}
async function showMessage(id){try{const m=await api('/api/v1/messages/'+id);alert(JSON.stringify(m,null,2))}catch(e){alert(e.message)}}
async function act(id,action){try{await api(`/api/v1/messages/${id}/${action}`,{method:'POST'});refresh()}catch(e){alert(e.message)}}
function toggleStatus(){document.getElementById('statusLabel').style.display=Number(messageForm.sds_type.value)===0?'flex':'none'}toggleStatus();
async function sendMessage(e){e.preventDefault();const f=new FormData(messageForm),nodes=String(f.get('force_nodes')||'').split(',').map(x=>x.trim()).filter(Boolean);const p={source_issi:Number(f.get('source_issi')),dest_issi:Number(f.get('dest_issi')),is_group:f.get('is_group')==='true',sds_type:Number(f.get('sds_type')),protocol_id:Number(f.get('protocol_id')),status_code:f.get('status_code')?Number(f.get('status_code')):null,payload_hex:String(f.get('payload_hex')||''),text:String(f.get('text')||''),priority:Number(f.get('priority')),ttl_secs:Number(f.get('ttl_secs')),ingress:'webui',force_nodes:nodes};try{await api('/api/v1/messages',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(p)});messageDialog.close();messageForm.reset();refresh()}catch(e){alert(e.message)}}
function openRoute(){routeForm.reset();routeForm.dataset.id='';routeForm.enabled.checked=true;routeDialog.showModal()}
function editRoute(id){const r=routes.find(x=>x.id===id);if(!r)return;routeForm.dataset.id=id;for(const[k,v]of Object.entries(r)){const el=routeForm.elements[k];if(!el)continue;if(el.type==='checkbox')el.checked=!!v;else el.value=v??''}routeDialog.showModal()}
async function saveRoute(e){e.preventDefault();const f=new FormData(routeForm),p={name:String(f.get('name')||''),enabled:routeForm.enabled.checked,kind:f.get('kind'),match_value:Number(f.get('match_value')),target_kind:f.get('target_kind'),target:String(f.get('target')||''),mode:f.get('mode'),notes:String(f.get('notes')||'')},id=routeForm.dataset.id;try{await api(id?'/api/v1/routes/'+id:'/api/v1/routes',{method:id?'PUT':'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(p)});routeDialog.close();refresh()}catch(e){alert(e.message)}}
async function deleteRoute(id){if(!confirm('Routingregel löschen?'))return;try{await api('/api/v1/routes/'+id,{method:'DELETE'});refresh()}catch(e){alert(e.message)}}
refresh();setInterval(refresh,4000);
</script></body></html>"#;

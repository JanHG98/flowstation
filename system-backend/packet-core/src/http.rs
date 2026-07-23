use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::Sender;
use std::thread;

use serde::Serialize;
use serde_json::json;

use crate::config::PacketCoreConfig;
use crate::protocol::{
    ActionAckInput, BackendRequest, ContextActionInput, DownlinkNpduInput, EdgeEventInput,
};
use crate::state::SharedPacketCore;

pub fn spawn_http_server(
    config: PacketCoreConfig,
    core: SharedPacketCore,
    gateway_tx: Sender<BackendRequest>,
) -> std::io::Result<thread::JoinHandle<()>> {
    let listener = TcpListener::bind(config.server.bind)?;
    tracing::info!(
        "Packet Core WebUI/API listening on http://{}",
        config.server.bind
    );
    Ok(thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let core = core.clone();
                    let gateway_tx = gateway_tx.clone();
                    let config = config.clone();
                    thread::spawn(move || {
                        if let Err(error) = handle_connection(stream, core, gateway_tx, config) {
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
    core: SharedPacketCore,
    gateway_tx: Sender<BackendRequest>,
    config: PacketCoreConfig,
) -> Result<(), String> {
    let request = read_request(&mut stream, config.limits.max_body_bytes)?;
    let response = route(request, core, gateway_tx, config);
    write_response(&mut stream, response).map_err(|error| error.to_string())
}

fn route(
    request: HttpRequest,
    core: SharedPacketCore,
    gateway_tx: Sender<BackendRequest>,
    config: PacketCoreConfig,
) -> HttpResponse {
    if request.method == "OPTIONS" {
        return empty(204);
    }

    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") => html(INDEX_HTML),
        ("GET", "/health/live") => json_response(200, &json!({"status":"live"})),
        ("GET", "/health/ready") => {
            let status = core.status();
            let ready = status.authoritative || status.node_gateway_connected;
            json_response(if ready { 200 } else { 503 }, &status)
        }
        ("GET", "/api/v1/status") => json_response(200, &core.status()),
        ("GET", "/api/v1/config") => json_response(200, &config),
        ("GET", "/api/v1/nodes") => json_response(200, &core.nodes()),
        ("GET", "/api/v1/contexts") => json_response(200, &core.contexts()),
        ("GET", "/api/v1/bearers") => json_response(200, &core.bearers()),
        ("GET", "/api/v1/reassemblies") => json_response(200, &core.reassemblies()),
        ("GET", "/api/v1/actions") => {
            let node_id = request.query.get("node_id").map(String::as_str);
            let after = query_u64(&request, "after", 0);
            let limit = query_usize(&request, "limit", 500, 5_000);
            json_response(200, &core.actions(node_id, after, limit))
        }
        ("GET", "/api/v1/npdu-outbox") => {
            let limit = query_usize(&request, "limit", 250, 5_000);
            json_response(200, &core.npdu_outbox(limit))
        }
        ("GET", "/api/v1/events") => {
            let limit = query_usize(&request, "limit", 200, 5_000);
            json_response(200, &core.recent_events(limit))
        }
        ("GET", "/api/v1/export.json") => {
            download_json("netcore-packet-core-export.json", &core.export())
        }
        ("GET", "/metrics") => text(
            "text/plain; version=0.0.4; charset=utf-8",
            core.metrics(),
        ),
        ("GET", "/openapi.json") => json_response(200, &openapi()),
        ("POST", "/api/v1/edge/events") => {
            match parse_json::<EdgeEventInput>(&request.body)
                .and_then(|input| core.ingest_edge_event(input))
            {
                Ok(actions) => json_response(202, &json!({"accepted":true,"actions":actions})),
                Err(error) => json_response(409, &json!({"error":error})),
            }
        }
        ("POST", "/api/v1/downlink") => {
            match parse_json::<DownlinkNpduInput>(&request.body)
                .and_then(|input| core.queue_downlink(input))
            {
                Ok(actions) => json_response(202, &json!({"queued":true,"actions":actions})),
                Err(error) => json_response(409, &json!({"error":error})),
            }
        }
        _ => dynamic_route(request, core, gateway_tx),
    }
}

fn dynamic_route(
    request: HttpRequest,
    core: SharedPacketCore,
    gateway_tx: Sender<BackendRequest>,
) -> HttpResponse {
    let parts: Vec<&str> = request.path.trim_matches('/').split('/').collect();
    match (request.method.as_str(), parts.as_slice()) {
        ("GET", ["api", "v1", "contexts", id]) => match core.context(id) {
            Some(context) => json_response(200, &context),
            None => json_response(404, &json!({"error":"context not found"})),
        },
        ("POST", ["api", "v1", "contexts", id, action]) => {
            let input = if request.body.is_empty() {
                Ok(ContextActionInput {
                    reason: None,
                    available: None,
                    usage_active: None,
                    priority: None,
                    mtu: None,
                    nsapis: Vec::new(),
                })
            } else {
                parse_json::<ContextActionInput>(&request.body)
            };
            match input.and_then(|input| core.context_action(id, action, input)) {
                Ok((actions, commands)) => match dispatch_all(&gateway_tx, commands) {
                    Ok(()) => json_response(202, &json!({"accepted":true,"actions":actions})),
                    Err(error) => json_response(503, &json!({"error":error,"actions":actions})),
                },
                Err(error) => json_response(409, &json!({"error":error})),
            }
        }
        ("POST", ["api", "v1", "actions", id, "ack"]) => {
            match parse_json::<ActionAckInput>(&request.body)
                .and_then(|input| core.acknowledge_action(id, input))
            {
                Ok(action) => json_response(200, &action),
                Err(error) => json_response(409, &json!({"error":error})),
            }
        }
        ("DELETE", ["api", "v1", "npdu-outbox", id]) => match core.delete_npdu(id) {
            Ok(()) => empty(204),
            Err(error) => json_response(404, &json!({"error":error})),
        },
        _ => json_response(404, &json!({"error":"not found"})),
    }
}

fn dispatch_all(tx: &Sender<BackendRequest>, commands: Vec<BackendRequest>) -> Result<(), String> {
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

fn query_u64(request: &HttpRequest, key: &str, default: u64) -> u64 {
    request
        .query
        .get(key)
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
}

fn openapi() -> serde_json::Value {
    json!({
        "openapi":"3.0.3",
        "info":{
            "title":"NetCore Packet Core",
            "version":env!("CARGO_PKG_VERSION"),
            "description":"OPEN LAB SNDCP context and packet-flow API. No authentication, no token and no TLS."
        },
        "paths":{
            "/api/v1/status":{"get":{}},
            "/api/v1/nodes":{"get":{}},
            "/api/v1/contexts":{"get":{}},
            "/api/v1/contexts/{id}":{"get":{}},
            "/api/v1/contexts/{id}/{action}":{"post":{}},
            "/api/v1/bearers":{"get":{}},
            "/api/v1/reassemblies":{"get":{}},
            "/api/v1/actions":{"get":{}},
            "/api/v1/actions/{id}/ack":{"post":{}},
            "/api/v1/edge/events":{"post":{}},
            "/api/v1/downlink":{"post":{}},
            "/api/v1/npdu-outbox":{"get":{}},
            "/api/v1/npdu-outbox/{id}":{"delete":{}},
            "/api/v1/events":{"get":{}},
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
    haystack.windows(needle.len()).position(|window| window == needle)
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
        output.push(if bytes[index] == b'+' { b' ' } else { bytes[index] });
        index += 1;
    }
    String::from_utf8_lossy(&output).into_owned()
}

const INDEX_HTML: &str = r#"<!doctype html>
<html lang="de"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>NetCore Packet Core</title><style>
:root{font-family:Inter,system-ui,sans-serif;color:#e8edf4;background:#0b1018}*{box-sizing:border-box}body{margin:0}header{padding:18px 26px;background:#121a26;border-bottom:1px solid #293449;position:sticky;top:0;z-index:2}h1{margin:0;font-size:22px}main{padding:22px;max-width:1800px;margin:auto}.warn{padding:10px 14px;background:#4b3512;border:1px solid #8c6628;border-radius:8px;margin-top:10px}.mode{padding:7px 11px;border-radius:99px;background:#1d2b40;margin-left:10px}.cards{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:12px;margin:18px 0}.card,.panel{background:#121a26;border:1px solid #293449;border-radius:10px;padding:15px}.value{font-size:27px;font-weight:700}.muted{color:#99a7ba}.ok{color:#5dd39e}.bad{color:#ff7474}.standby{color:#ffd166}.toolbar{display:flex;gap:8px;flex-wrap:wrap;align-items:center;margin:10px 0}button,input,select,textarea{background:#0d1420;color:#e8edf4;border:1px solid #40506a;border-radius:6px;padding:8px}button{cursor:pointer}button.primary{background:#1769aa}button.danger{background:#7a2631}table{width:100%;border-collapse:collapse;font-size:13px}th,td{text-align:left;padding:8px;border-bottom:1px solid #293449;vertical-align:top}pre{max-height:360px;overflow:auto;white-space:pre-wrap}.grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(220px,1fr));gap:10px}.wide{grid-column:1/-1}.tabs{display:flex;gap:6px;flex-wrap:wrap;margin-bottom:14px}.tabs button.active{background:#1769aa}.view{display:none}.view.active{display:block}dialog{background:#121a26;color:#e8edf4;border:1px solid #40506a;border-radius:10px;max-width:900px;width:94%}label{display:flex;flex-direction:column;gap:5px}.pill{display:inline-block;padding:3px 7px;border-radius:99px;background:#26364e}.small{font-size:11px}@media(max-width:760px){main{padding:10px}.panel{overflow:auto}header{position:static}}
</style></head><body><header><div style="display:flex;align-items:center;flex-wrap:wrap"><h1>NetCore-Tetra · Packet Core</h1><span id="mode" class="mode">…</span></div><div id="gateway" class="muted">verbinde …</div><div class="warn"><b>OPEN LAB:</b> keine Tokens, keine Anmeldung, kein TLS. Jeder erreichbare Client kann PDP-Kontexte ändern oder trennen.</div></header><main>
<div class="tabs"><button class="active" onclick="show('overview',this)">Übersicht</button><button onclick="show('contexts',this)">PDP Contexts</button><button onclick="show('bearers',this)">PDCH & Flow</button><button onclick="show('edge',this)">Edge/API</button><button onclick="show('events',this)">Ereignisse</button><button onclick="show('config',this)">Konfiguration</button></div>
<section id="overview" class="view active"><div id="cards" class="cards"></div><div class="panel"><h2>Knoten</h2><table><thead><tr><th>TBS</th><th>Status</th><th>Packet Data</th><th>Gateway</th><th>Contexts</th><th>Bearer</th><th>Verkehr UL/DL</th></tr></thead><tbody id="nodeRows"></tbody></table></div><div class="panel"><h2>Aktive Reassembly</h2><table><thead><tr><th>ID</th><th>TBS</th><th>ISSI/NSAPI</th><th>Richtung</th><th>Fragmente</th><th>Bytes</th><th>Gesamt</th><th>Timeout</th></tr></thead><tbody id="reassemblyRows"></tbody></table></div></section>
<section id="contexts" class="view"><div class="panel"><div class="toolbar"><h2 style="margin-right:auto">PDP Contexts / NSAPI</h2><button class="primary" onclick="downlinkDialog.showModal()">N-PDU Downlink</button><button onclick="refresh()">Aktualisieren</button></div><table><thead><tr><th>ISSI / NSAPI</th><th>TBS / Anchor</th><th>IPv4</th><th>Zustand</th><th>SNEI</th><th>MTU / Prio</th><th>Queue</th><th>Traffic</th><th>Aktionen</th></tr></thead><tbody id="contextRows"></tbody></table></div></section>
<section id="bearers" class="view"><div class="panel"><h2>PDCH Bearer</h2><table><thead><tr><th>TBS</th><th>ISSI</th><th>Carrier / TS</th><th>NSAPI</th><th>Aktiv</th><th>Alter</th><th>Idle</th></tr></thead><tbody id="bearerRows"></tbody></table></div><div class="panel"><h2>Edge-Actions / Flow-Control</h2><table><thead><tr><th>Seq</th><th>TBS</th><th>Context</th><th>Typ</th><th>Status</th><th>Versuche</th><th>Erstellt</th><th>Fehler</th></tr></thead><tbody id="actionRows"></tbody></table></div></section>
<section id="edge" class="view"><div class="panel"><div class="toolbar"><h2 style="margin-right:auto">Edge-Schnittstelle</h2><button class="primary" onclick="edgeDialog.showModal()">Testevent einspeisen</button></div><p>Protokoll: <code>netcore-packet-edge-v1</code>. Der direkte Edge-Kanal kann Context-Aktivierung, READY/STANDBY, Modify, Reconnect, Deactivation, Bearer und Fragmente transportieren. Im <b>shadow</b>-Modus werden vorhandene TBS-Snapshots gespiegelt; im <b>authoritative</b>-Modus erzeugt der Core echte Edge-Actions.</p><pre>POST /api/v1/edge/events
GET  /api/v1/actions?node_id=tbs-a&amp;after=0
POST /api/v1/actions/{id}/ack
POST /api/v1/downlink</pre></div><div class="panel"><h2>Reassemblierte N-PDUs</h2><table><thead><tr><th>Zeit</th><th>TBS</th><th>ISSI/NSAPI</th><th>Richtung</th><th>Datagramm</th><th>Bytes</th><th>Aktion</th></tr></thead><tbody id="npduRows"></tbody></table></div></section>
<section id="events" class="view"><div class="panel"><h2>Ereignisse & Audit</h2><pre id="eventLog"></pre></div></section>
<section id="config" class="view"><div class="panel"><h2>Aktive Konfiguration</h2><pre id="configDump"></pre><p><a href="/openapi.json" target="_blank">OpenAPI</a> · <a href="/metrics" target="_blank">Prometheus Metrics</a> · <a href="/api/v1/export.json">JSON-Export</a></p></div></section>
</main>
<dialog id="downlinkDialog"><form id="downlinkForm" class="grid" onsubmit="sendDownlink(event)"><h2 class="wide">N-PDU für Teilnehmer einplanen</h2><label>ISSI<input name="issi" type="number" min="1" max="16777215" required></label><label>NSAPI<input name="nsapi" type="number" min="1" max="14" required></label><label>Service<select name="acknowledged"><option value="false">SN-UNITDATA</option><option value="true">SN-DATA</option></select></label><label>Priorität<input name="priority" type="number" min="0" max="7" value="4"></label><label class="wide">N-PDU als Hex<textarea name="payload_hex" rows="7" required placeholder="4500001c..."></textarea></label><div class="wide toolbar"><button class="primary" type="submit">Fragmentieren & einplanen</button><button type="button" onclick="downlinkDialog.close()">Abbrechen</button></div></form></dialog>
<dialog id="edgeDialog"><form id="edgeForm" onsubmit="sendEdge(event)"><h2>Edge-Testevent</h2><p class="muted">Beliebiges JSON gemäß <code>EdgeEventInput</code>. Beispiel: Context-Aktivierung.</p><textarea name="json" rows="18" style="width:100%">{
  "kind": "context_activated",
  "node_id": "tbs-lab-01",
  "issi": 4010001,
  "nsapi": 1,
  "ipv4": "10.0.0.2",
  "primary_nsapi": null,
  "snei": 1001,
  "mtu": 480,
  "priority": 4
}</textarea><div class="toolbar"><button class="primary" type="submit">Senden</button><button type="button" onclick="edgeDialog.close()">Abbrechen</button></div></form></dialog>
<script>
let contexts=[],nodes=[],bearers=[],actions=[],reassemblies=[],npdus=[],events=[];const downlinkDialog=document.getElementById('downlinkDialog'),edgeDialog=document.getElementById('edgeDialog');
function show(id,b){document.querySelectorAll('.view').forEach(x=>x.classList.remove('active'));document.querySelectorAll('.tabs button').forEach(x=>x.classList.remove('active'));document.getElementById(id).classList.add('active');b.classList.add('active')}
async function api(path,opt){const r=await fetch(path,opt);if(!r.ok&&r.status!==204){let e={};try{e=await r.json()}catch{}throw new Error(e.error||r.statusText)}return r.status===204?null:r.json()}
function esc(v){return String(v??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]))}
function actionKind(a){return a&&a.payload&&a.payload.kind||'?'}
async function refresh(){try{const [s,n,c,b,a,r,p,e,cfg]=await Promise.all([api('/api/v1/status'),api('/api/v1/nodes'),api('/api/v1/contexts'),api('/api/v1/bearers'),api('/api/v1/actions?limit=500'),api('/api/v1/reassemblies'),api('/api/v1/npdu-outbox?limit=200'),api('/api/v1/events?limit=160'),api('/api/v1/config')]);nodes=n;contexts=c;bearers=b;actions=a;reassemblies=r;npdus=p;events=e;document.getElementById('mode').textContent=s.mode.toUpperCase();document.getElementById('gateway').innerHTML=s.node_gateway_connected?'<span class="ok">● Node Gateway verbunden</span>':'<span class="bad">● Node Gateway getrennt</span>';document.getElementById('cards').innerHTML=[['Contexts',s.contexts_total],['READY',s.contexts_ready],['STANDBY',s.contexts_standby],['Suspended',s.contexts_suspended],['PDCH Bearer',s.bearers_active],['Actions offen',s.actions_pending],['Reassembly',s.reassemblies_active],['N-PDU Outbox',s.npdu_outbox],['Queue Pakete',s.queued_packets],['Queue Bytes',s.queued_bytes]].map(x=>`<div class="card"><div class="muted">${x[0]}</div><div class="value">${x[1]}</div></div>`).join('');render();document.getElementById('configDump').textContent=JSON.stringify(cfg,null,2)}catch(e){document.getElementById('gateway').innerHTML='<span class="bad">UI-Fehler: '+esc(e.message)+'</span>'}}
function render(){document.getElementById('nodeRows').innerHTML=nodes.map(n=>`<tr><td>${esc(n.station_name)}<br><span class="muted">${esc(n.node_id)}</span></td><td>${n.connected?'<span class="ok">online</span>':'<span class="bad">offline</span>'}</td><td>${n.packet_data_capable?'ja':'nein'} / Multi ${n.multi_pdch_capable?'ja':'nein'}</td><td>${n.gateway_running?'<span class="ok">UP</span>':'DOWN'} ${esc(n.interface_name||'')}<br>${esc(n.gateway_address||'')}</td><td>${n.active_contexts}</td><td>${n.active_bearers}/${n.bearer_capacity}</td><td>${n.packets_from_mobile}/${n.packets_to_mobile}<br><span class="muted">${n.bytes_from_mobile}/${n.bytes_to_mobile} B</span></td></tr>`).join('');document.getElementById('contextRows').innerHTML=contexts.map(c=>`<tr><td><b>${c.issi}</b> / ${c.nsapi}<br><span class="small muted">${esc(c.source)}</span></td><td>${esc(c.node_id)}<br><span class="muted">Anchor ${esc(c.anchor_node_id)}</span></td><td>${esc(c.ipv4)}</td><td><span class="pill">${esc(c.state)}</span><br>${c.available?'verfügbar':'gesperrt'} / ${c.usage_active?'aktiv':'inaktiv'}</td><td>${c.snei??'-'}</td><td>${c.mtu} / P${c.priority}</td><td>${c.queued_packets} / ${c.queued_bytes} B</td><td>UL ${c.packets_up}/${c.bytes_up} B<br>DL ${c.packets_down}/${c.bytes_down} B</td><td><button onclick="ctx('${c.id}','wake')">Wake</button> <button onclick="ctx('${c.id}','end-of-data')">EoD</button> <button onclick="ctx('${c.id}','suspend')">Sperren</button> <button onclick="ctx('${c.id}','resume')">Freigeben</button> <button onclick="ctx('${c.id}','flush')">Queue leeren</button> <button class="danger" onclick="ctx('${c.id}','deactivate')">Trennen</button></td></tr>`).join('');document.getElementById('bearerRows').innerHTML=bearers.map(b=>`<tr><td>${esc(b.node_id)}</td><td>${b.issi}</td><td>${b.carrier_num} / LTS ${b.logical_ts} / Air ${b.air_ts}</td><td>${b.nsapis.join(', ')}</td><td>${b.active?'ja':'nein'}</td><td>${b.age_secs}s</td><td>${b.idle_secs}s</td></tr>`).join('');document.getElementById('actionRows').innerHTML=actions.slice().reverse().map(a=>`<tr><td>${a.sequence}</td><td>${esc(a.node_id)}</td><td>${esc(a.context_id||'')}</td><td>${esc(actionKind(a))}</td><td>${esc(a.state)}</td><td>${a.attempts}/${a.max_attempts}</td><td>${esc(a.created_at)}</td><td>${esc(a.last_error||'')}</td></tr>`).join('');document.getElementById('reassemblyRows').innerHTML=reassemblies.map(r=>`<tr><td>${esc(r.datagram_id)}</td><td>${esc(r.node_id)}</td><td>${r.issi}/${r.nsapi}</td><td>${esc(r.direction)}</td><td>${r.fragment_count}</td><td>${r.received_bytes}</td><td>${r.total_len??'?'}</td><td>${esc(r.expires_at)}</td></tr>`).join('');document.getElementById('npduRows').innerHTML=npdus.map(n=>`<tr><td>${esc(n.created_at)}</td><td>${esc(n.node_id)}</td><td>${n.issi}/${n.nsapi}</td><td>${esc(n.direction)}</td><td>${esc(n.datagram_id)}</td><td>${n.payload.length}</td><td><button onclick="showNpdu('${n.id}')">Hex</button> <button onclick="dropNpdu('${n.id}')">Erledigt</button></td></tr>`).join('');document.getElementById('eventLog').textContent=events.map(e=>`${e.timestamp} #${e.sequence} ${e.kind} ${e.node_id||''} ${e.context_id||''} ${JSON.stringify(e.detail)}`).join('\n')}
async function ctx(id,action){if(action==='deactivate'&&!confirm('PDP Context wirklich trennen?'))return;try{await api(`/api/v1/contexts/${encodeURIComponent(id)}/${action}`,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({reason:'WebUI open-lab action'})});refresh()}catch(e){alert(e.message)}}
async function sendDownlink(e){e.preventDefault();const f=new FormData(e.target),p={issi:Number(f.get('issi')),nsapi:Number(f.get('nsapi')),payload_hex:String(f.get('payload_hex')),acknowledged:f.get('acknowledged')==='true',priority:Number(f.get('priority'))};try{await api('/api/v1/downlink',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(p)});downlinkDialog.close();e.target.reset();refresh()}catch(e){alert(e.message)}}
async function sendEdge(e){e.preventDefault();try{const p=JSON.parse(new FormData(e.target).get('json'));await api('/api/v1/edge/events',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(p)});edgeDialog.close();refresh()}catch(e){alert(e.message)}}
function showNpdu(id){const n=npdus.find(x=>x.id===id);if(n)alert(n.payload.map(x=>x.toString(16).padStart(2,'0')).join(''))}
async function dropNpdu(id){try{await api('/api/v1/npdu-outbox/'+id,{method:'DELETE'});refresh()}catch(e){alert(e.message)}}
refresh();setInterval(refresh,4000);
</script></body></html>"#;

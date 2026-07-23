use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::Sender;
use std::thread;

use serde::Serialize;
use serde_json::json;

use crate::config::MobilityCoreConfig;
use crate::protocol::BackendRequest;
use crate::state::{CreateTransferRequest, SharedMobility};

pub fn spawn_http_server(
    config: MobilityCoreConfig,
    mobility: SharedMobility,
    gateway_tx: Sender<BackendRequest>,
) -> std::io::Result<thread::JoinHandle<()>> {
    let listener = TcpListener::bind(config.server.bind)?;
    tracing::info!("Mobility Core WebUI/API listening on http://{}", config.server.bind);
    Ok(thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let mobility = mobility.clone();
                    let gateway_tx = gateway_tx.clone();
                    let config = config.clone();
                    thread::spawn(move || {
                        if let Err(error) = handle_connection(stream, mobility, gateway_tx, config) {
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
}

fn handle_connection(
    mut stream: TcpStream,
    mobility: SharedMobility,
    gateway_tx: Sender<BackendRequest>,
    config: MobilityCoreConfig,
) -> Result<(), String> {
    let request = read_request(&mut stream, config.limits.max_body_bytes)?;
    let response = route(request, mobility, gateway_tx, config);
    write_response(&mut stream, response).map_err(|error| error.to_string())
}

fn route(
    request: HttpRequest,
    mobility: SharedMobility,
    gateway_tx: Sender<BackendRequest>,
    config: MobilityCoreConfig,
) -> HttpResponse {
    if request.method == "OPTIONS" {
        return empty(204);
    }
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") => html(INDEX_HTML),
        ("GET", "/health/live") => json_response(200, &json!({ "status": "live" })),
        ("GET", "/health/ready") => {
            let status = mobility.status();
            let code = if status.node_gateway_connected { 200 } else { 503 };
            json_response(code, &status)
        }
        ("GET", "/api/v1/status") => json_response(200, &mobility.status()),
        ("GET", "/api/v1/nodes") => json_response(200, &mobility.nodes()),
        ("GET", "/api/v1/subscribers") => json_response(200, &mobility.subscribers()),
        ("GET", "/api/v1/transfers") => json_response(200, &mobility.transfers()),
        ("GET", "/api/v1/events") => {
            let limit = request.query.get("limit")
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(100)
                .min(1_000);
            json_response(200, &mobility.recent_events(limit))
        }
        ("GET", "/api/v1/config") => json_response(200, &config),
        ("GET", "/metrics") => HttpResponse {
            status: 200,
            content_type: "text/plain; version=0.0.4; charset=utf-8",
            body: mobility.metrics().into_bytes(),
        },
        ("GET", "/openapi.json") => json_response(200, &openapi()),
        ("POST", "/api/v1/transfers") => {
            let parsed = serde_json::from_slice::<CreateTransferRequest>(&request.body)
                .map_err(|error| format!("invalid JSON: {error}"));
            match parsed.and_then(|request| mobility.create_transfer(request)) {
                Ok((transfer, command)) => {
                    if gateway_tx.send(command).is_err() {
                        json_response(503, &json!({ "error": "node gateway worker is unavailable" }))
                    } else {
                        json_response(202, &transfer)
                    }
                }
                Err(error) => json_response(409, &json!({ "error": error })),
            }
        }
        _ if request.method == "POST" && request.path.starts_with("/api/v1/transfers/") => {
            let tail = request.path.trim_start_matches("/api/v1/transfers/");
            if let Some((id, action)) = tail.rsplit_once('/') {
                match action {
                    "cancel" => match mobility.cancel_transfer(id) {
                        Ok(transfer) => json_response(200, &transfer),
                        Err(error) => json_response(409, &json!({ "error": error })),
                    },
                    _ => json_response(404, &json!({ "error": "unknown transfer action" })),
                }
            } else {
                json_response(404, &json!({ "error": "missing transfer action" }))
            }
        }
        _ => json_response(404, &json!({ "error": "not found" })),
    }
}

fn openapi() -> serde_json::Value {
    json!({
        "openapi": "3.0.3",
        "info": {
            "title": "NetCore Mobility Core",
            "version": env!("CARGO_PKG_VERSION"),
            "description": "OPEN LAB API. No authentication, no token and no TLS."
        },
        "paths": {
            "/api/v1/status": { "get": {} },
            "/api/v1/nodes": { "get": {} },
            "/api/v1/subscribers": { "get": {} },
            "/api/v1/transfers": { "get": {}, "post": {} },
            "/api/v1/transfers/{id}/cancel": { "post": {} },
            "/api/v1/events": { "get": {} },
            "/health/live": { "get": {} },
            "/health/ready": { "get": {} },
            "/metrics": { "get": {} }
        }
    })
}

fn json_response<T: Serialize>(status: u16, value: &T) -> HttpResponse {
    match serde_json::to_vec_pretty(value) {
        Ok(body) => HttpResponse {
            status,
            content_type: "application/json; charset=utf-8",
            body,
        },
        Err(error) => HttpResponse {
            status: 500,
            content_type: "application/json; charset=utf-8",
            body: format!("{{\"error\":\"serialization failed: {error}\"}}").into_bytes(),
        },
    }
}

fn html(value: &str) -> HttpResponse {
    HttpResponse {
        status: 200,
        content_type: "text/html; charset=utf-8",
        body: value.as_bytes().to_vec(),
    }
}

fn empty(status: u16) -> HttpResponse {
    HttpResponse {
        status,
        content_type: "text/plain; charset=utf-8",
        body: Vec::new(),
    }
}

fn read_request(stream: &mut TcpStream, max_body_bytes: usize) -> Result<HttpRequest, String> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];
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

    let header_text = std::str::from_utf8(&buffer[..header_end])
        .map_err(|_| "request headers are not utf-8".to_string())?;
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
                content_length = value.trim().parse::<usize>()
                    .map_err(|_| "invalid content-length".to_string())?;
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

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
}

fn parse_path_and_query(raw: &str) -> (String, HashMap<String, String>) {
    let mut parts = raw.splitn(2, '?');
    let path = parts.next().unwrap_or(raw).to_string();
    let query = parts.next().map(|query| {
        query.split('&')
            .filter(|pair| !pair.is_empty())
            .map(|pair| {
                let mut fields = pair.splitn(2, '=');
                (
                    fields.next().unwrap_or_default().replace('+', " ").replace("%20", " "),
                    fields.next().unwrap_or("true").replace('+', " ").replace("%20", " "),
                )
            })
            .collect()
    }).unwrap_or_default();
    (path, query)
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
        503 => "Service Unavailable",
        _ => "OK",
    }
}

const INDEX_HTML: &str = r#"<!doctype html>
<html lang="de">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>NetCore Mobility Core</title>
<style>
:root{color-scheme:dark;--bg:#0b1220;--panel:#121d31;--line:#2a3b58;--text:#e9f0fb;--muted:#91a4c2;--ok:#4ade80;--warn:#facc15;--bad:#fb7185;--accent:#60a5fa}*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--text);font-family:Inter,system-ui,sans-serif}.wrap{max-width:1500px;margin:auto;padding:20px}.lab{background:#7f1d1d;border:2px solid var(--bad);padding:13px 18px;border-radius:12px;font-weight:800}.cards{display:grid;grid-template-columns:repeat(auto-fit,minmax(170px,1fr));gap:12px;margin:18px 0}.card,.panel{background:var(--panel);border:1px solid var(--line);border-radius:12px;padding:16px}.value{font-size:2rem;font-weight:800}.label,.small{color:var(--muted)}.panel{margin-top:14px;overflow:auto}table{width:100%;border-collapse:collapse;min-width:900px}th,td{text-align:left;padding:9px;border-bottom:1px solid var(--line)}button{border:0;border-radius:8px;padding:8px 11px;background:var(--accent);font-weight:800;cursor:pointer}.danger{background:var(--bad)}input,select{background:#0f1a2d;color:var(--text);border:1px solid var(--line);border-radius:8px;padding:9px;margin:3px}.form{display:flex;flex-wrap:wrap;gap:7px;align-items:center}.pill{padding:3px 8px;border-radius:99px;font-size:.8rem;font-weight:700}.online{background:#14532d}.offline{background:#4c0519}.phase{background:#1e3a5f}pre{white-space:pre-wrap;font-size:.8rem;color:#c9d7ed}</style>
</head>
<body><div class="wrap">
<div class="lab">⚠ OFFENER TESTMODUS: KEINE TOKENS, KEIN LOGIN, KEIN TLS. Jeder erreichbare Client darf Migrationen auslösen.</div>
<h1>NetCore Mobility Core</h1><div class="small">Zentrale Teilnehmerlage, Migrationen und Context Transfer zwischen TBS</div>
<div id="cards" class="cards"></div>
<div class="panel"><h2>Context Transfer starten</h2><div class="form">
<input id="issi" type="number" placeholder="ISSI">
<select id="source"></select><select id="target"></select>
<input id="local" type="number" placeholder="Ziel-ISSI (optional)">
<button onclick="startTransfer()">Transfer starten</button>
</div></div>
<div class="panel"><h2>Aktive und letzte Transfers</h2><table><thead><tr><th>Phase</th><th>ISSI</th><th>Quelle → Ziel</th><th>Ziel-ISSI</th><th>Zeit</th><th>Fehler</th><th>Aktion</th></tr></thead><tbody id="transfers"></tbody></table></div>
<div class="panel"><h2>Teilnehmer</h2><table><thead><tr><th>ISSI</th><th>Serving Node</th><th>Status</th><th>Gruppen</th><th>EE</th><th>RSSI</th><th>Letztes Ereignis</th></tr></thead><tbody id="subs"></tbody></table></div>
<div class="panel"><h2>Basisstationen</h2><table><thead><tr><th>Status</th><th>Node</th><th>Zelle</th><th>Carrier</th><th>Letzter Kontakt</th></tr></thead><tbody id="nodes"></tbody></table></div>
<div class="panel"><h2>Ereignisse</h2><pre id="events"></pre></div>
</div>
<script>
const esc=v=>String(v??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
async function getj(u){const r=await fetch(u);if(!r.ok)throw new Error(await r.text());return r.json()}
async function post(u,b){const r=await fetch(u,{method:'POST',headers:{'Content-Type':'application/json'},body:b?JSON.stringify(b):''});const t=await r.text();if(!r.ok)throw new Error(t);return t?JSON.parse(t):{}}
const card=(l,v)=>`<div class="card"><div class="value">${esc(v)}</div><div class="label">${esc(l)}</div></div>`;
async function startTransfer(){try{const issi=Number(document.querySelector('#issi').value);const source_node=document.querySelector('#source').value;const target_node=document.querySelector('#target').value;const lv=document.querySelector('#local').value;await post('/api/v1/transfers',{issi,source_node,target_node,target_local_issi:lv?Number(lv):null});refresh()}catch(e){alert(e.message)}}
async function cancelTransfer(id){if(!confirm('Transfer abbrechen?'))return;try{await post(`/api/v1/transfers/${id}/cancel`);refresh()}catch(e){alert(e.message)}}
async function refresh(){try{const [s,n,u,t,e]=await Promise.all([getj('/api/v1/status'),getj('/api/v1/nodes'),getj('/api/v1/subscribers'),getj('/api/v1/transfers'),getj('/api/v1/events?limit=60')]);document.querySelector('#cards').innerHTML=[card('Gateway',s.node_gateway_connected?'ONLINE':'OFFLINE'),card('TBS online',s.nodes_connected),card('Teilnehmer',s.subscribers_known),card('Transfers aktiv',s.transfers_active),card('Erfolgreich',s.transfers_completed),card('Fehlgeschlagen',s.transfers_failed)].join('');const opts=n.filter(x=>x.connected&&!x.stale).map(x=>`<option value="${esc(x.node_id)}">${esc(x.station_name)} (${esc(x.node_id)})</option>`).join('');document.querySelector('#source').innerHTML=opts;document.querySelector('#target').innerHTML=opts;document.querySelector('#nodes').innerHTML=n.map(x=>`<tr><td><span class="pill ${x.connected&&!x.stale?'online':'offline'}">${x.connected&&!x.stale?'ONLINE':'OFFLINE'}</span></td><td><b>${esc(x.station_name)}</b><br><span class="small">${esc(x.node_id)}</span></td><td>${x.mcc}/${x.mnc} LA ${x.location_area}, CC ${x.colour_code}</td><td>${x.main_carrier}${x.secondary_carrier?' / '+x.secondary_carrier:''}</td><td>${esc(x.last_seen)}</td></tr>`).join('')||'<tr><td colspan="5">Keine TBS.</td></tr>';document.querySelector('#subs').innerHTML=u.map(x=>`<tr><td><b>${x.issi}</b></td><td>${esc(x.serving_node||'-')}</td><td>${x.registered?'registriert':'offline'}</td><td>${[...x.groups].join(', ')||'-'}</td><td>${x.energy_saving_mode??'-'}</td><td>${x.last_rssi_dbfs==null?'-':x.last_rssi_dbfs.toFixed(1)+' dBFS'}</td><td>${esc(x.last_seen)}</td></tr>`).join('')||'<tr><td colspan="7">Noch keine Teilnehmertelemetrie.</td></tr>';document.querySelector('#transfers').innerHTML=t.map(x=>`<tr><td><span class="pill phase">${esc(x.phase)}</span></td><td>${x.issi}</td><td>${esc(x.source_node)} → ${esc(x.target_node)}</td><td>${x.target_local_issi}</td><td>${esc(x.updated_at)}</td><td>${esc(x.error||'-')}</td><td>${['completed','failed','timed_out','cancelled','source_cleanup_queued','source_cleanup_requested'].includes(x.phase)?'':`<button class="danger" onclick="cancelTransfer('${x.transfer_id}')">Abbrechen</button>`}</td></tr>`).join('')||'<tr><td colspan="7">Noch keine Transfers.</td></tr>';document.querySelector('#events').textContent=e.map(x=>`${x.timestamp} #${x.seq} ${x.kind}${x.issi?' ISSI '+x.issi:''}${x.transfer_id?' ['+x.transfer_id+']':''} ${JSON.stringify(x.detail)}`).join('\n')}catch(e){document.querySelector('#events').textContent='Fehler: '+e.message}}
refresh();setInterval(refresh,4000);
</script></body></html>"#;

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::Sender;
use std::thread;

use serde::Serialize;
use serde_json::json;

use crate::config::CallControlConfig;
use crate::protocol::BackendRequest;
use crate::state::{
    FloorInput, GroupCallInput, IndividualCallInput, RestoreInput, SharedCalls,
};

pub fn spawn_http_server(
    config: CallControlConfig,
    calls: SharedCalls,
    gateway_tx: Sender<BackendRequest>,
) -> std::io::Result<thread::JoinHandle<()>> {
    let listener = TcpListener::bind(config.server.bind)?;
    tracing::info!("Call Control WebUI/API listening on http://{}", config.server.bind);
    Ok(thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let config = config.clone();
                    let calls = calls.clone();
                    let gateway_tx = gateway_tx.clone();
                    thread::spawn(move || {
                        if let Err(error) = handle_connection(stream, config, calls, gateway_tx) {
                            tracing::warn!("Call Control HTTP connection failed: {}", error);
                        }
                    });
                }
                Err(error) => tracing::warn!("Call Control HTTP accept failed: {}", error),
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
    config: CallControlConfig,
    calls: SharedCalls,
    gateway_tx: Sender<BackendRequest>,
) -> Result<(), String> {
    let request = read_request(&mut stream, config.limits.max_body_bytes)?;
    let response = route(request, calls, gateway_tx);
    write_response(&mut stream, response).map_err(|error| error.to_string())
}

fn route(
    request: HttpRequest,
    calls: SharedCalls,
    gateway_tx: Sender<BackendRequest>,
) -> HttpResponse {
    if request.method == "OPTIONS" {
        return empty(204);
    }
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") => html(INDEX_HTML),
        ("GET", "/health/live") => json_response(200, &json!({"status":"live"})),
        ("GET", "/health/ready") => {
            let status = calls.status();
            json_response(if status.node_gateway_connected { 200 } else { 503 }, &status)
        }
        ("GET", "/api/v1/status") => json_response(200, &calls.status()),
        ("GET", "/api/v1/nodes") => json_response(200, &calls.nodes()),
        ("GET", "/api/v1/participants") => json_response(200, &calls.participants()),
        ("GET", "/api/v1/calls") => json_response(200, &calls.calls()),
        ("GET", "/api/v1/restores") => json_response(200, &calls.restores()),
        ("GET", "/api/v1/events") => {
            let limit = request
                .query
                .get("limit")
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(100)
                .min(1000);
            json_response(200, &calls.events(limit))
        }
        ("GET", "/api/v1/config") => json_response(200, &calls.config_view()),
        ("GET", "/metrics") => text("text/plain; version=0.0.4; charset=utf-8", calls.metrics()),
        ("GET", "/openapi.json") => json_response(200, &openapi()),
        ("POST", "/api/v1/calls/group") => {
            match parse_json::<GroupCallInput>(&request.body)
                .and_then(|input| calls.create_group_call(input))
            {
                Ok((call, commands)) => dispatch_response(&gateway_tx, commands, 202, &call),
                Err(error) => json_response(409, &json!({"error": error})),
            }
        }
        ("POST", "/api/v1/calls/individual") => {
            match parse_json::<IndividualCallInput>(&request.body)
                .and_then(|input| calls.create_individual_call(input))
            {
                Ok((call, commands)) => dispatch_response(&gateway_tx, commands, 202, &call),
                Err(error) => json_response(409, &json!({"error": error})),
            }
        }
        ("POST", "/api/v1/restores") => {
            match parse_json::<RestoreInput>(&request.body)
                .and_then(|input| calls.create_restore(input))
            {
                Ok((operation, commands)) => {
                    dispatch_response(&gateway_tx, commands, 202, &operation)
                }
                Err(error) => json_response(409, &json!({"error": error})),
            }
        }
        _ if request.path.starts_with("/api/v1/calls/") => {
            call_route(request, calls, gateway_tx)
        }
        _ if request.path.starts_with("/api/v1/restores/") => restore_route(request, calls),
        _ => json_response(404, &json!({"error":"not found"})),
    }
}

fn call_route(
    request: HttpRequest,
    calls: SharedCalls,
    gateway_tx: Sender<BackendRequest>,
) -> HttpResponse {
    let tail = request.path.trim_start_matches("/api/v1/calls/");
    if request.method == "GET" && !tail.contains('/') {
        return calls.call(tail).map_or_else(
            || json_response(404, &json!({"error":"logical call not found"})),
            |call| json_response(200, &call),
        );
    }
    let Some((logical_call_id, action)) = tail.split_once('/') else {
        return json_response(404, &json!({"error":"not found"}));
    };
    if request.method != "POST" {
        return json_response(405, &json!({"error":"method not allowed"}));
    }
    let result = match action {
        "release" => calls.release_call(logical_call_id),
        "floor" => parse_json::<FloorInput>(&request.body)
            .and_then(|input| calls.request_floor(logical_call_id, input)),
        "floor/release" => calls.release_floor(logical_call_id),
        _ => return json_response(404, &json!({"error":"not found"})),
    };
    match result {
        Ok(commands) => {
            let queued = commands.len();
            match dispatch_all(&gateway_tx, commands) {
                Ok(()) => json_response(202, &json!({"queued": queued})),
                Err(error) => json_response(503, &json!({"error": error})),
            }
        }
        Err(error) => json_response(409, &json!({"error": error})),
    }
}

fn restore_route(request: HttpRequest, calls: SharedCalls) -> HttpResponse {
    let tail = request.path.trim_start_matches("/api/v1/restores/");
    let Some(restore_id) = tail.strip_suffix("/cancel") else {
        return json_response(404, &json!({"error":"not found"}));
    };
    if request.method != "POST" {
        return json_response(405, &json!({"error":"method not allowed"}));
    }
    match calls.cancel_restore(restore_id) {
        Ok(()) => empty(204),
        Err(error) => json_response(409, &json!({"error": error})),
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
        Err(error) => json_response(503, &json!({"error": error})),
    }
}

fn dispatch_all(
    tx: &Sender<BackendRequest>,
    commands: Vec<BackendRequest>,
) -> Result<(), String> {
    for command in commands {
        tx.send(command)
            .map_err(|_| "Node Gateway worker is unavailable".to_string())?;
    }
    Ok(())
}

fn parse_json<T: serde::de::DeserializeOwned>(body: &[u8]) -> Result<T, String> {
    serde_json::from_slice(body).map_err(|error| format!("invalid JSON: {error}"))
}

fn openapi() -> serde_json::Value {
    json!({
        "openapi":"3.0.3",
        "info":{
            "title":"NetCore Call Control",
            "version":env!("CARGO_PKG_VERSION"),
            "description":"OPEN LAB API. No authentication, no token and no TLS."
        },
        "paths":{
            "/api/v1/status":{"get":{}},
            "/api/v1/nodes":{"get":{}},
            "/api/v1/participants":{"get":{}},
            "/api/v1/calls":{"get":{}},
            "/api/v1/calls/group":{"post":{}},
            "/api/v1/calls/individual":{"post":{}},
            "/api/v1/calls/{logical_call_id}":{"get":{}},
            "/api/v1/calls/{logical_call_id}/release":{"post":{}},
            "/api/v1/calls/{logical_call_id}/floor":{"post":{}},
            "/api/v1/calls/{logical_call_id}/floor/release":{"post":{}},
            "/api/v1/restores":{"get":{},"post":{}},
            "/api/v1/restores/{restore_id}/cancel":{"post":{}},
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
        },
        Err(error) => HttpResponse {
            status: 500,
            content_type: "application/json; charset=utf-8",
            body: format!("{{\"error\":\"serialization failed: {error}\"}}").into_bytes(),
        },
    }
}

fn text(content_type: &'static str, value: String) -> HttpResponse {
    HttpResponse {
        status: 200,
        content_type,
        body: value.into_bytes(),
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
        let read = stream
            .read(&mut chunk)
            .map_err(|error| format!("request read failed: {error}"))?;
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
        .map_err(|_| "request headers are not UTF-8".to_string())?;
    let mut lines = header_text.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| "missing request line".to_string())?;
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| "missing method".to_string())?
        .to_ascii_uppercase();
    let raw_path = parts
        .next()
        .ok_or_else(|| "missing path".to_string())?;
    let (path, query) = parse_path_and_query(raw_path);
    let mut content_length = 0usize;
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value
                    .trim()
                    .parse()
                    .map_err(|_| "invalid content-length".to_string())?;
            }
        }
    }
    if content_length > max_body_bytes {
        return Err("body too large".to_string());
    }
    let mut body = buffer[header_end..].to_vec();
    while body.len() < content_length {
        let read = stream
            .read(&mut chunk)
            .map_err(|error| format!("body read failed: {error}"))?;
        if read == 0 {
            return Err("connection closed before body was complete".to_string());
        }
        body.extend_from_slice(&chunk[..read]);
        if body.len() > max_body_bytes {
            return Err("body too large".to_string());
        }
    }
    body.truncate(content_length);
    Ok(HttpRequest {
        method,
        path,
        query,
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
        405 => "Method Not Allowed",
        409 => "Conflict",
        413 => "Payload Too Large",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "OK",
    };
    let headers = format!(
        concat!(
            "HTTP/1.1 {} {}\r\n",
            "Content-Type: {}\r\n",
            "Content-Length: {}\r\n",
            "Cache-Control: no-store\r\n",
            "Access-Control-Allow-Origin: *\r\n",
            "Access-Control-Allow-Methods: GET,POST,OPTIONS\r\n",
            "Access-Control-Allow-Headers: Content-Type\r\n",
            "X-NetCore-Security-Mode: open_lab\r\n",
            "Connection: close\r\n\r\n"
        ),
        response.status,
        reason,
        response.content_type,
        response.body.len(),
    );
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

const INDEX_HTML: &str = r#"<!doctype html><html lang="de"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>NetCore Call Control</title><style>:root{color-scheme:dark;--bg:#091117;--panel:#121e27;--line:#29404f;--text:#ecf4f8;--muted:#9eb0bc;--accent:#36a3ff;--ok:#55d68c;--warn:#ffca58;--bad:#ff6969}*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--text);font:14px system-ui}.lab{padding:10px 20px;background:#8e2020;color:#fff;text-align:center;font-weight:800}header{padding:20px 26px;background:#101a22;border-bottom:1px solid var(--line);display:flex;justify-content:space-between;gap:15px}h1,h2{margin:0 0 10px}.wrap{padding:20px;display:grid;gap:16px}.cards{display:grid;grid-template-columns:repeat(auto-fit,minmax(140px,1fr));gap:10px}.card,.panel{background:var(--panel);border:1px solid var(--line);border-radius:10px;padding:14px}.value{font-size:25px;font-weight:750}.muted{color:var(--muted)}.ok{color:var(--ok)}.bad{color:var(--bad)}.warn{color:var(--warn)}.toolbar{display:flex;gap:8px;flex-wrap:wrap;margin:10px 0}input,select,button{background:#192a35;color:var(--text);border:1px solid var(--line);border-radius:6px;padding:8px}button{cursor:pointer}.primary{background:#1268aa}.danger{background:#8a3038}table{width:100%;border-collapse:collapse}th,td{text-align:left;padding:8px;border-bottom:1px solid var(--line);vertical-align:top}.tablewrap{overflow:auto}pre{white-space:pre-wrap;max-height:320px;overflow:auto}@media(max-width:750px){header{display:block}.toolbar>*{flex:1 1 150px}}</style></head><body><div class="lab">⚠ OPEN LAB – keine Anmeldung, keine Tokens, kein TLS. Jeder erreichbare Client darf Rufe, Floor und Restore steuern.</div><header><div><h1>Call Control</h1><div class="muted">Netzweite logische Rufe, TBS-Legs, Floor Control und Call Restore</div></div><div id="gateway">Gateway …</div></header><main class="wrap"><section class="cards" id="cards"></section><section class="panel"><h2>Gruppenruf starten</h2><div class="toolbar"><input id="ggssi" type="number" placeholder="GSSI"><input id="gsource" type="number" placeholder="Quell-ISSI"><input id="gprio" type="number" min="0" max="15" value="0" placeholder="Priorität"><input id="gnodes" placeholder="Node-IDs, kommasepariert (optional)"><button class="primary" onclick="groupCall()">Starten</button></div></section><section class="panel"><h2>Individualruf starten</h2><div class="toolbar"><input id="icalling" type="number" placeholder="Calling ISSI"><input id="icalled" type="number" placeholder="Called ISSI"><select id="isimplex"><option value="true">Simplex</option><option value="false">Duplex</option></select><input id="iprio" type="number" min="0" max="15" value="0"><input id="inode" placeholder="Ziel-Node optional"><button class="primary" onclick="individualCall()">Starten</button></div></section><section class="panel"><h2>Logische Rufe</h2><div class="tablewrap"><table><thead><tr><th>Call</th><th>Typ / Ziel</th><th>Status</th><th>Floor</th><th>Legs</th><th>Aktionen</th></tr></thead><tbody id="callRows"></tbody></table></div></section><section class="panel"><h2>Call Restore</h2><div class="toolbar"><input id="rlogical" placeholder="Logical Call ID"><input id="rsource" placeholder="Quell-Node"><input id="rtarget" placeholder="Ziel-Node"><input id="rcall" type="number" placeholder="lokale Call-ID optional"><button onclick="restoreCall()">Restore vorbereiten</button></div><div class="tablewrap"><table><thead><tr><th>Zeit</th><th>Call</th><th>Quelle → Ziel</th><th>Status</th><th>Meldung</th><th>Aktion</th></tr></thead><tbody id="restoreRows"></tbody></table></div></section><section class="panel"><h2>Basisstationen</h2><div class="tablewrap"><table><thead><tr><th>Node</th><th>Zelle</th><th>Online</th><th>Call Control</th><th>Restore</th></tr></thead><tbody id="nodeRows"></tbody></table></div></section><section class="panel"><h2>Teilnehmerlage</h2><div class="tablewrap"><table><thead><tr><th>Node</th><th>ISSI</th><th>registriert</th><th>Gruppen</th><th>zuletzt</th></tr></thead><tbody id="participantRows"></tbody></table></div></section><section class="panel"><h2>Ereignisse</h2><pre id="events" class="muted"></pre></section></main><script>let calls=[],restores=[],nodes=[],participants=[];async function api(path,opt){const r=await fetch(path,opt);if(!r.ok&&r.status!==204){let e={};try{e=await r.json()}catch{}throw new Error(e.error||r.statusText)}return r.status===204?null:r.json()}function esc(v){return String(v??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]))}function post(path,body){return api(path,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body||{})})}async function refresh(){try{const[s,c,r,n,p,e]=await Promise.all([api('/api/v1/status'),api('/api/v1/calls'),api('/api/v1/restores'),api('/api/v1/nodes'),api('/api/v1/participants'),api('/api/v1/events?limit=40')]);calls=c;restores=r;nodes=n;participants=p;document.getElementById('gateway').innerHTML=s.node_gateway_connected?'<span class="ok">● Gateway verbunden</span>':'<span class="bad">● Gateway getrennt</span>';document.getElementById('cards').innerHTML=[['aktive Calls',s.calls_active],['aktive Legs',s.call_legs_active],['verwaltet',s.calls_managed],['TBS online',s.nodes_connected],['Teilnehmer',s.participants_registered],['Kommandos offen',s.pending_commands],['Restore offen',s.restores_pending],['Revision',s.database_revision]].map(x=>`<div class="card"><div class="muted">${x[0]}</div><div class="value">${x[1]}</div></div>`).join('');renderCalls();renderRestores();renderNodes();renderParticipants();document.getElementById('events').textContent=e.map(x=>`${x.timestamp} #${x.seq} ${x.kind} ${x.node_id||''} ${x.logical_call_id||''} ${x.local_call_id??''}`).join('\n')}catch(e){document.getElementById('gateway').innerHTML='<span class="bad">UI-Fehler: '+esc(e.message)+'</span>'}}function renderCalls(){document.getElementById('callRows').innerHTML=calls.map(c=>{const legs=Object.values(c.legs||{});return `<tr><td><b>${esc(c.logical_call_id)}</b><br><span class="muted">${esc(c.operation_id)}</span></td><td>${esc(c.kind)}<br>${c.gssi?'GSSI '+c.gssi:(c.calling_issi+' → '+c.called_issi)}</td><td>${esc(c.phase)}<br>Prio ${c.priority}${c.emergency?' / NOTRUF':''}</td><td>${c.floor_holder??'frei'}${c.floor_queue?.length?' / Queue '+c.floor_queue.join(', '):''}</td><td>${legs.map(l=>`${esc(l.node_id)}: ${esc(l.phase)} / ${l.local_call_id??'–'} / TS ${l.timeslot??'–'}`).join('<br>')}</td><td><button onclick="floorPrompt('${c.logical_call_id}')">Floor</button> <button onclick="releaseFloor('${c.logical_call_id}')">Floor frei</button> <button class="danger" onclick="releaseCall('${c.logical_call_id}')">Beenden</button></td></tr>`}).join('')}function renderRestores(){document.getElementById('restoreRows').innerHTML=restores.map(r=>`<tr><td>${esc(r.created_at)}</td><td>${esc(r.logical_call_id)}<br>${r.source_call_id} → ${r.target_call_id??'–'}</td><td>${esc(r.source_node)} → ${esc(r.target_node)}</td><td>${esc(r.phase)}</td><td>${esc(r.message)}</td><td>${['completed','cancelled','failed','timed_out'].includes(r.phase)?'':`<button class="danger" onclick="cancelRestore('${r.restore_id}')">Abbrechen</button>`}</td></tr>`).join('')}function renderNodes(){document.getElementById('nodeRows').innerHTML=nodes.map(n=>`<tr><td>${esc(n.node_id)}<br><span class="muted">${esc(n.station_name)}</span></td><td>${n.mcc}/${n.mnc} LA ${n.location_area} CC ${n.colour_code}</td><td>${n.connected&&!n.stale?'<span class="ok">online</span>':'<span class="bad">offline</span>'}</td><td>${n.call_control_capable?'ja':'nein'}</td><td>${n.call_restore_capable?'ja':'nein'}</td></tr>`).join('')}function renderParticipants(){document.getElementById('participantRows').innerHTML=participants.map(p=>`<tr><td>${esc(p.node_id)}</td><td>${p.issi}</td><td>${p.registered?'ja':'nein'}</td><td>${[...p.groups].join(', ')}</td><td>${esc(p.last_seen)}</td></tr>`).join('')}async function groupCall(){try{await post('/api/v1/calls/group',{gssi:Number(ggssi.value),source_issi:Number(gsource.value),priority:Number(gprio.value||0),target_nodes:gnodes.value.split(',').map(x=>x.trim()).filter(Boolean)});refresh()}catch(e){alert(e.message)}}async function individualCall(){try{await post('/api/v1/calls/individual',{calling_issi:Number(icalling.value),called_issi:Number(icalled.value),simplex:isimplex.value==='true',priority:Number(iprio.value||0),target_node:inode.value.trim()||null});refresh()}catch(e){alert(e.message)}}async function releaseCall(id){if(!confirm('Call beenden?'))return;try{await post(`/api/v1/calls/${id}/release`);refresh()}catch(e){alert(e.message)}}async function floorPrompt(id){const issi=Number(prompt('ISSI für Floor-Anforderung:'));if(!issi)return;const force=confirm('Floor notfalls erzwingen?');try{await post(`/api/v1/calls/${id}/floor`,{source_issi:issi,force});refresh()}catch(e){alert(e.message)}}async function releaseFloor(id){try{await post(`/api/v1/calls/${id}/floor/release`);refresh()}catch(e){alert(e.message)}}async function restoreCall(){try{await post('/api/v1/restores',{logical_call_id:rlogical.value.trim(),source_node:rsource.value.trim(),target_node:rtarget.value.trim(),source_call_id:rcall.value?Number(rcall.value):null});refresh()}catch(e){alert(e.message)}}async function cancelRestore(id){try{await post(`/api/v1/restores/${id}/cancel`);refresh()}catch(e){alert(e.message)}}refresh();setInterval(refresh,4000)</script></body></html>"#;

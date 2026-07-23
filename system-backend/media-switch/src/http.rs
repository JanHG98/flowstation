use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::Sender;
use std::thread;

use serde::Serialize;
use serde_json::json;

use crate::config::MediaSwitchConfig;
use crate::protocol::BackendRequest;
use crate::state::{InjectionInput, MuteInput, SharedMedia};

pub fn spawn_http_server(
    config: MediaSwitchConfig,
    media: SharedMedia,
    gateway_tx: Sender<BackendRequest>,
) -> std::io::Result<thread::JoinHandle<()>> {
    let listener = TcpListener::bind(config.server.bind)?;
    tracing::info!(
        "Media Switch WebUI/API listening on http://{}",
        config.server.bind
    );
    Ok(thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let config = config.clone();
                    let media = media.clone();
                    let gateway_tx = gateway_tx.clone();
                    thread::spawn(move || {
                        if let Err(error) =
                            handle_connection(stream, config, media, gateway_tx)
                        {
                            tracing::warn!("Media Switch HTTP connection failed: {}", error);
                        }
                    });
                }
                Err(error) => tracing::warn!("Media Switch HTTP accept failed: {}", error),
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
    config: MediaSwitchConfig,
    media: SharedMedia,
    gateway_tx: Sender<BackendRequest>,
) -> Result<(), String> {
    let request = read_request(&mut stream, config.limits.max_body_bytes)?;
    let response = route(request, media, gateway_tx);
    write_response(&mut stream, response).map_err(|error| error.to_string())
}

fn route(
    request: HttpRequest,
    media: SharedMedia,
    gateway_tx: Sender<BackendRequest>,
) -> HttpResponse {
    if request.method == "OPTIONS" {
        return empty(204);
    }
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") => html(INDEX_HTML),
        ("GET", "/health/live") => json_response(200, &json!({"status":"live"})),
        ("GET", "/health/ready") => {
            let status = media.status();
            json_response(
                if status.node_gateway_connected && status.call_control_connected {
                    200
                } else {
                    503
                },
                &status,
            )
        }
        ("GET", "/api/v1/status") => json_response(200, &media.status()),
        ("GET", "/api/v1/nodes") => json_response(200, &media.nodes()),
        ("GET", "/api/v1/sessions") => json_response(200, &media.sessions()),
        ("GET", "/api/v1/streams") => json_response(200, &media.streams()),
        ("GET", "/api/v1/buffers") => json_response(200, &media.buffers()),
        ("GET", "/api/v1/taps") => {
            let limit = query_limit(&request, 100);
            json_response(200, &media.taps(limit))
        }
        ("GET", "/api/v1/events") => {
            let limit = query_limit(&request, 100);
            json_response(200, &media.events(limit))
        }
        ("GET", "/api/v1/config") => json_response(200, &media.config_view()),
        ("POST", "/api/v1/gateway/ping") => {
            let request = BackendRequest::Ping {
                request_id: Some(format!("media-switch-{}", chrono::Utc::now().timestamp_millis())),
            };
            match gateway_tx.send(request) {
                Ok(()) => json_response(202, &json!({"queued":true})),
                Err(_) => json_response(503, &json!({"error":"Node Gateway worker unavailable"})),
            }
        }
        ("GET", "/metrics") => text(
            "text/plain; version=0.0.4; charset=utf-8",
            media.metrics(),
        ),
        ("GET", "/openapi.json") => json_response(200, &openapi()),
        _ if request.path.starts_with("/api/v1/sessions/") => {
            session_route(request, media)
        }
        _ => json_response(404, &json!({"error":"not found"})),
    }
}

fn session_route(request: HttpRequest, media: SharedMedia) -> HttpResponse {
    let tail = request.path.trim_start_matches("/api/v1/sessions/");
    if request.method == "GET" && !tail.contains('/') {
        return media.session(tail).map_or_else(
            || json_response(404, &json!({"error":"media session not found"})),
            |session| json_response(200, &session),
        );
    }
    let Some((session_id, action)) = tail.rsplit_once('/') else {
        return json_response(404, &json!({"error":"not found"}));
    };
    if request.method != "POST" {
        return json_response(405, &json!({"error":"method not allowed"}));
    }
    match action {
        "mute" => match parse_json::<MuteInput>(&request.body)
            .and_then(|input| media.mute_stream(session_id, input))
        {
            Ok(()) => empty(204),
            Err(error) => json_response(409, &json!({"error":error})),
        },
        "flush" => match media.flush_session(session_id) {
            Ok(frames) => json_response(200, &json!({"flushed_frames":frames})),
            Err(error) => json_response(409, &json!({"error":error})),
        },
        "inject" => match parse_json::<InjectionInput>(&request.body)
            .and_then(|input| media.inject(session_id, input))
        {
            Ok(targets) => json_response(202, &json!({"queued_targets":targets})),
            Err(error) => json_response(409, &json!({"error":error})),
        },
        _ => json_response(404, &json!({"error":"not found"})),
    }
}

fn query_limit(request: &HttpRequest, default: usize) -> usize {
    request
        .query
        .get("limit")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(default)
        .min(2_000)
}

fn parse_json<T: serde::de::DeserializeOwned>(body: &[u8]) -> Result<T, String> {
    serde_json::from_slice(body).map_err(|error| format!("invalid JSON: {error}"))
}

fn openapi() -> serde_json::Value {
    json!({
        "openapi":"3.0.3",
        "info":{
            "title":"NetCore Media Switch",
            "version":env!("CARGO_PKG_VERSION"),
            "description":"OPEN LAB API. No authentication, no token and no TLS. Routes packed TETRA ACELP frames between active TBS call legs."
        },
        "paths":{
            "/api/v1/status":{"get":{}},
            "/api/v1/nodes":{"get":{}},
            "/api/v1/sessions":{"get":{}},
            "/api/v1/sessions/{session_id}":{"get":{}},
            "/api/v1/sessions/{session_id}/mute":{"post":{}},
            "/api/v1/sessions/{session_id}/flush":{"post":{}},
            "/api/v1/sessions/{session_id}/inject":{"post":{}},
            "/api/v1/streams":{"get":{}},
            "/api/v1/buffers":{"get":{}},
            "/api/v1/taps":{"get":{}},
            "/api/v1/events":{"get":{}},
            "/api/v1/config":{"get":{}},
            "/api/v1/gateway/ping":{"post":{}},
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
            body: format!("{{\"error\":\"serialization failed: {error}\"}}")
                .into_bytes(),
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
        output.push(if bytes[index] == b'+' {
            b' '
        } else {
            bytes[index]
        });
        index += 1;
    }
    String::from_utf8_lossy(&output).into_owned()
}

const INDEX_HTML: &str = r#"<!doctype html><html lang="de"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>NetCore Media Switch</title><style>:root{color-scheme:dark;--bg:#071118;--panel:#111f29;--line:#294250;--text:#ecf5f8;--muted:#9fb2bd;--ok:#57d98e;--warn:#ffca58;--bad:#ff6d6d;--accent:#36a3ff}*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--text);font:14px system-ui}.lab{padding:10px 20px;background:#8e2020;color:#fff;text-align:center;font-weight:800}header{padding:20px 26px;background:#0e1a22;border-bottom:1px solid var(--line);display:flex;justify-content:space-between;gap:15px}h1,h2{margin:0 0 10px}.wrap{padding:20px;display:grid;gap:16px}.cards{display:grid;grid-template-columns:repeat(auto-fit,minmax(135px,1fr));gap:10px}.card,.panel{background:var(--panel);border:1px solid var(--line);border-radius:10px;padding:14px}.value{font-size:24px;font-weight:760}.muted{color:var(--muted)}.ok{color:var(--ok)}.bad{color:var(--bad)}.warn{color:var(--warn)}.toolbar{display:flex;gap:8px;flex-wrap:wrap;margin:10px 0}input,select,button,textarea{background:#192c37;color:var(--text);border:1px solid var(--line);border-radius:6px;padding:8px}button{cursor:pointer}.primary{background:#1268aa}.danger{background:#893039}table{width:100%;border-collapse:collapse}th,td{text-align:left;padding:8px;border-bottom:1px solid var(--line);vertical-align:top}.tablewrap{overflow:auto}pre{white-space:pre-wrap;max-height:300px;overflow:auto}@media(max-width:750px){header{display:block}.toolbar>*{flex:1 1 150px}}</style></head><body><div class="lab">⚠ OPEN LAB – keine Anmeldung, keine Tokens, kein TLS. Jeder erreichbare Client darf Medienströme stummschalten, puffern und Testframes einspeisen.</div><header><div><h1>Media Switch</h1><div class="muted">Netzweites Routing gepackter TETRA-Sprachframes, Jitter-Puffer und Media-Taps</div></div><div id="links">Verbindungen …</div></header><main class="wrap"><section class="cards" id="cards"></section><section class="panel"><h2>Media-Sessions</h2><div class="tablewrap"><table><thead><tr><th>Session</th><th>Typ / Zustand</th><th>Floor</th><th>Streams</th><th>Frames</th><th>Aktion</th></tr></thead><tbody id="sessionRows"></tbody></table></div></section><section class="panel"><h2>Streams / TBS-Legs</h2><div class="tablewrap"><table><thead><tr><th>Session</th><th>Node / TS</th><th>Status</th><th>RX / TX / Drop</th><th>Sequenz</th><th>Aktion</th></tr></thead><tbody id="streamRows"></tbody></table></div></section><section class="panel"><h2>Jitter-Puffer</h2><div class="tablewrap"><table><thead><tr><th>Session</th><th>Ziel</th><th>Frames</th><th>Fällig in</th></tr></thead><tbody id="bufferRows"></tbody></table></div></section><section class="panel"><h2>Basisstationen</h2><div class="tablewrap"><table><thead><tr><th>Node</th><th>Zelle</th><th>Online</th><th>Media Bridge</th><th>Frames</th></tr></thead><tbody id="nodeRows"></tbody></table></div></section><section class="panel"><h2>Testframe einspeisen</h2><div class="muted">35 gepackte Bytes, dezimal und kommasepariert. Diese offene Schnittstelle ist zugleich der vorbereitete Audio-Player-Eingang.</div><div class="toolbar"><input id="injectSession" placeholder="Session-ID"><input id="injectNode" placeholder="Ziel-Node optional"><input id="injectTs" type="number" min="1" max="7" placeholder="Ziel-TS optional"><textarea id="injectPayload" rows="3" style="min-width:420px" placeholder="0,0,0,... exakt 35 Werte"></textarea><button class="primary" onclick="injectFrame()">Einspeisen</button></div></section><section class="panel"><h2>Media-Taps</h2><pre id="taps" class="muted"></pre></section><section class="panel"><h2>Ereignisse</h2><pre id="events" class="muted"></pre></section></main><script>let sessions=[],streams=[];const el=id=>document.getElementById(id);async function api(path,opt){const r=await fetch(path,opt);if(!r.ok&&r.status!==204){let e={};try{e=await r.json()}catch{}throw new Error(e.error||r.statusText)}return r.status===204?null:r.json()}function esc(v){return String(v??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]))}function post(path,body){return api(path,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body||{})})}async function refresh(){try{const[s,se,st,b,n,t,e]=await Promise.all([api('/api/v1/status'),api('/api/v1/sessions'),api('/api/v1/streams'),api('/api/v1/buffers'),api('/api/v1/nodes'),api('/api/v1/taps?limit=30'),api('/api/v1/events?limit=40')]);sessions=se;streams=st;el('links').innerHTML=(s.node_gateway_connected?'<span class="ok">● Gateway</span>':'<span class="bad">● Gateway</span>')+' &nbsp; '+(s.call_control_connected?'<span class="ok">● Call Control</span>':'<span class="bad">● Call Control</span>');el('cards').innerHTML=[['Sessions',s.sessions_active],['Streams',s.streams_active],['Puffer',s.pending_frames],['RX',s.frames_received],['geroutet',s.frames_routed],['gesendet',s.frames_sent],['Drops',s.frames_dropped],['Duplikate',s.duplicate_frames],['Injection',s.frames_injected]].map(x=>`<div class="card"><div class="muted">${x[0]}</div><div class="value">${x[1]}</div></div>`).join('');renderSessions();renderStreams();el('bufferRows').innerHTML=b.map(x=>`<tr><td>${esc(x.session_id)}</td><td>${esc(x.target_node_id)} / TS ${x.target_logical_ts}</td><td>${x.queued_frames}</td><td>${x.oldest_due_in_ms} ms</td></tr>`).join('');el('nodeRows').innerHTML=n.map(x=>`<tr><td>${esc(x.node_id)}<br><span class="muted">${esc(x.station_name)}</span></td><td>${x.mcc}/${x.mnc} LA ${x.location_area} CC ${x.colour_code}</td><td>${x.connected&&!x.stale?'<span class="ok">online</span>':'<span class="bad">offline</span>'}</td><td>${x.media_bridge?'ja':'nein'}</td><td>${x.media_frame_count}</td></tr>`).join('');el('taps').textContent=t.map(x=>`${x.timestamp} #${x.seq} ${x.session_id} ${x.source_node_id}/TS${x.source_logical_ts} seq=${x.source_sequence} → ${x.target_count}${x.injected?' INJECT':''}`).join('\n');el('events').textContent=e.map(x=>`${x.timestamp} #${x.seq} ${x.kind} ${x.session_id||''} ${x.node_id||''}`).join('\n')}catch(e){el('links').innerHTML='<span class="bad">UI-Fehler: '+esc(e.message)+'</span>'}}function renderSessions(){el('sessionRows').innerHTML=sessions.map(s=>`<tr><td><b>${esc(s.logical_call_id)}</b></td><td>${esc(s.kind)} / ${esc(s.phase)}${s.emergency?' / NOTRUF':''}</td><td>${s.floor_holder??'frei'}</td><td>${Object.keys(s.legs||{}).length}</td><td>${s.frames_received} / ${s.frames_routed} / ${s.frames_dropped}</td><td><button class="danger" onclick="flushSession('${s.logical_call_id}')">Puffer leeren</button></td></tr>`).join('')}function renderStreams(){el('streamRows').innerHTML=streams.map(s=>`<tr><td>${esc(s.session_id)}</td><td>${esc(s.node_id)} / TS ${s.logical_ts}<br><span class="muted">Call ${s.local_call_id??'–'} Carrier ${s.carrier_num??'–'}</span></td><td>${esc(s.phase)} ${s.muted?'<span class="bad">STUMM</span>':''}</td><td>${s.rx_frames} / ${s.tx_frames} / ${s.dropped_frames}</td><td>${s.last_sequence??'–'}</td><td><button onclick="muteStream('${s.session_id}','${s.node_id}',${s.logical_ts},${!s.muted})">${s.muted?'Aktivieren':'Stumm'}</button></td></tr>`).join('')}async function muteStream(id,node,ts,muted){try{await post(`/api/v1/sessions/${id}/mute`,{node_id:node,logical_ts:ts,muted});refresh()}catch(e){alert(e.message)}}async function flushSession(id){if(!confirm('Jitter-Puffer dieser Session leeren?'))return;try{await post(`/api/v1/sessions/${id}/flush`,{});refresh()}catch(e){alert(e.message)}}async function injectFrame(){try{const payload=el('injectPayload').value.split(/[\s,;]+/).filter(Boolean).map(Number);await post(`/api/v1/sessions/${encodeURIComponent(el('injectSession').value.trim())}/inject`,{payload,target_node:el('injectNode').value.trim()||null,target_logical_ts:el('injectTs').value?Number(el('injectTs').value):null});refresh()}catch(e){alert(e.message)}}refresh();setInterval(refresh,2000)</script></body></html>"#;

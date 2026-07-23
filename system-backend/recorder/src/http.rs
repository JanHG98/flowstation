use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::thread;

use serde::Serialize;
use serde_json::json;

use crate::config::RecorderConfig;
use crate::state::{HoldInput, RecordingMetadata, RetentionInput, SharedRecorder};

pub fn spawn_http_server(
    config: RecorderConfig,
    recorder: SharedRecorder,
) -> std::io::Result<thread::JoinHandle<()>> {
    let listener = TcpListener::bind(config.server.bind)?;
    tracing::info!(
        "Recorder WebUI/API listening on http://{}",
        config.server.bind
    );
    Ok(thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let config = config.clone();
                    let recorder = recorder.clone();
                    thread::spawn(move || {
                        if let Err(error) = handle_connection(stream, config, recorder) {
                            tracing::warn!("Recorder HTTP connection failed: {}", error);
                        }
                    });
                }
                Err(error) => tracing::warn!("Recorder HTTP accept failed: {}", error),
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

enum ResponseBody {
    Bytes(Vec<u8>),
    File(PathBuf),
}

struct HttpResponse {
    status: u16,
    content_type: &'static str,
    headers: Vec<(String, String)>,
    body: ResponseBody,
}

fn handle_connection(
    mut stream: TcpStream,
    config: RecorderConfig,
    recorder: SharedRecorder,
) -> Result<(), String> {
    let request = read_request(&mut stream, config.limits.max_body_bytes)?;
    let response = route(request, recorder);
    write_response(&mut stream, response).map_err(|error| error.to_string())
}

fn route(request: HttpRequest, recorder: SharedRecorder) -> HttpResponse {
    if request.method == "OPTIONS" {
        return empty(204);
    }
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") => html(INDEX_HTML),
        ("GET", "/health/live") => json_response(200, &json!({"status":"live"})),
        ("GET", "/health/ready") => {
            let status = recorder.status();
            json_response(if status.ready { 200 } else { 503 }, &status)
        }
        ("GET", "/api/v1/status") => json_response(200, &recorder.status()),
        ("GET", "/api/v1/active") => json_response(200, &recorder.active_recordings()),
        ("GET", "/api/v1/recordings") => {
            let recordings = filter_recordings(recorder.recordings(), &request.query);
            json_response(200, &recordings)
        }
        ("GET", "/api/v1/events") => {
            let limit = query_limit(&request, 100);
            json_response(200, &recorder.events(limit))
        }
        ("GET", "/api/v1/config") => json_response(200, &recorder.config_view()),
        ("GET", "/metrics") => text(
            "text/plain; version=0.0.4; charset=utf-8",
            recorder.metrics(),
        ),
        ("GET", "/openapi.json") => json_response(200, &openapi()),
        _ if request.path.starts_with("/api/v1/recordings/") => {
            recording_route(request, recorder)
        }
        _ => json_response(404, &json!({"error":"not found"})),
    }
}

fn recording_route(request: HttpRequest, recorder: SharedRecorder) -> HttpResponse {
    let tail = request.path.trim_start_matches("/api/v1/recordings/");
    if request.method == "GET" && !tail.contains('/') {
        return recorder.recording(tail).map_or_else(
            || json_response(404, &json!({"error":"recording not found"})),
            |recording| json_response(200, &recording),
        );
    }
    let Some((id, action)) = tail.rsplit_once('/') else {
        return json_response(404, &json!({"error":"not found"}));
    };
    match (request.method.as_str(), action) {
        ("POST", "verify") => result_response(recorder.verify_recording(id)),
        ("POST", "retention") => parse_json::<RetentionInput>(&request)
            .and_then(|input| recorder.set_retention(id, input))
            .map_or_else(error_response, |value| json_response(200, &value)),
        ("POST", "hold") => parse_json::<HoldInput>(&request)
            .and_then(|input| recorder.set_hold(id, input))
            .map_or_else(error_response, |value| json_response(200, &value)),
        ("POST", "delete") => recorder.delete_recording(id).map_or_else(
            error_response,
            |()| json_response(200, &json!({"deleted":true,"id":id})),
        ),
        ("POST", "finalize") => result_response(recorder.finalize_active(id)),
        ("GET", "export") => match recorder.export_recording(id) {
            Ok(path) => file_response(
                path,
                "application/x-tar",
                vec![(
                    "Content-Disposition".to_string(),
                    format!("attachment; filename=\"netcore-recording-{id}.tar\""),
                )],
            ),
            Err(error) => error_response(error),
        },
        _ => json_response(404, &json!({"error":"not found"})),
    }
}

fn filter_recordings(
    mut recordings: Vec<RecordingMetadata>,
    query: &HashMap<String, String>,
) -> Vec<RecordingMetadata> {
    if let Some(q) = query.get("q").map(|value| value.trim().to_ascii_lowercase())
        && !q.is_empty()
    {
        recordings.retain(|recording| {
            let mut haystack = format!(
                "{} {} {} {:?} {:?} {:?} {:?}",
                recording.id,
                recording.session_id,
                recording.call_kind,
                recording.source_issi,
                recording.gssi,
                recording.calling_issi,
                recording.called_issi
            )
            .to_ascii_lowercase();
            for node in &recording.source_nodes {
                haystack.push(' ');
                haystack.push_str(&node.to_ascii_lowercase());
            }
            for speaker in &recording.speakers {
                haystack.push(' ');
                haystack.push_str(&speaker.to_string());
            }
            haystack.contains(&q)
        });
    }
    if let Some(gssi) = query.get("gssi").and_then(|value| value.parse::<u32>().ok()) {
        recordings.retain(|recording| recording.gssi == Some(gssi));
    }
    if let Some(issi) = query.get("issi").and_then(|value| value.parse::<u32>().ok()) {
        recordings.retain(|recording| {
            recording.source_issi == Some(issi)
                || recording.calling_issi == Some(issi)
                || recording.called_issi == Some(issi)
                || recording.speakers.contains(&issi)
        });
    }
    if let Some(emergency) = query.get("emergency").and_then(|value| value.parse::<bool>().ok()) {
        recordings.retain(|recording| recording.emergency == emergency);
    }
    let limit = query
        .get("limit")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(200)
        .clamp(1, 2_000);
    recordings.truncate(limit);
    recordings
}

fn result_response(result: Result<RecordingMetadata, String>) -> HttpResponse {
    result.map_or_else(error_response, |value| json_response(200, &value))
}

fn error_response(error: String) -> HttpResponse {
    let status = if error.contains("not found") {
        404
    } else if error.contains("disabled")
        || error.contains("legal hold")
        || error.contains("still active")
    {
        409
    } else {
        400
    };
    json_response(status, &json!({"error":error}))
}

fn parse_json<T: serde::de::DeserializeOwned>(request: &HttpRequest) -> Result<T, String> {
    serde_json::from_slice(&request.body).map_err(|error| format!("invalid JSON: {error}"))
}

fn query_limit(request: &HttpRequest, default: usize) -> usize {
    request
        .query
        .get("limit")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(default)
        .clamp(1, 2_000)
}

fn openapi() -> serde_json::Value {
    json!({
        "openapi":"3.0.3",
        "info":{
            "title":"NetCore Recorder OPEN LAB API",
            "version":env!("CARGO_PKG_VERSION"),
            "description":"OPEN LAB API. No authentication, no token and no TLS. Stores packed TETRA ACELP frames, metadata, hashes and retention state."
        },
        "paths":{
            "/health/live":{"get":{}},
            "/health/ready":{"get":{}},
            "/api/v1/status":{"get":{}},
            "/api/v1/active":{"get":{}},
            "/api/v1/recordings":{"get":{}},
            "/api/v1/recordings/{id}":{"get":{}},
            "/api/v1/recordings/{id}/verify":{"post":{}},
            "/api/v1/recordings/{id}/retention":{"post":{}},
            "/api/v1/recordings/{id}/hold":{"post":{}},
            "/api/v1/recordings/{id}/delete":{"post":{}},
            "/api/v1/recordings/{id}/finalize":{"post":{}},
            "/api/v1/recordings/{id}/export":{"get":{}},
            "/api/v1/events":{"get":{}},
            "/api/v1/config":{"get":{}},
            "/metrics":{"get":{}}
        }
    })
}

fn read_request(stream: &mut TcpStream, max_body_bytes: usize) -> Result<HttpRequest, String> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 8192];
    let header_end;
    loop {
        let read = stream
            .read(&mut chunk)
            .map_err(|error| format!("request read failed: {error}"))?;
        if read == 0 {
            return Err("connection closed before request headers".to_string());
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(position) = find_subslice(&buffer, b"\r\n\r\n") {
            header_end = position;
            break;
        }
        if buffer.len() > 64 * 1024 {
            return Err("request headers are too large".to_string());
        }
    }

    let header = std::str::from_utf8(&buffer[..header_end])
        .map_err(|_| "request headers are not UTF-8".to_string())?;
    let mut lines = header.lines();
    let request_line = lines
        .next()
        .ok_or_else(|| "missing request line".to_string())?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts
        .next()
        .ok_or_else(|| "missing method".to_string())?
        .to_string();
    let raw_path = request_parts
        .next()
        .ok_or_else(|| "missing path".to_string())?;
    let content_length = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim().parse::<usize>().ok())
        .unwrap_or(0);
    if content_length > max_body_bytes {
        return Err("request body is too large".to_string());
    }

    let body_start = header_end + 4;
    while buffer.len() < body_start + content_length {
        let read = stream
            .read(&mut chunk)
            .map_err(|error| format!("request body read failed: {error}"))?;
        if read == 0 {
            return Err("connection closed before request body".to_string());
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
    let (path, query) = parse_path_and_query(raw_path);
    Ok(HttpRequest {
        method,
        path,
        query,
        body: buffer[body_start..body_start + content_length].to_vec(),
    })
}

fn json_response<T: Serialize>(status: u16, value: &T) -> HttpResponse {
    match serde_json::to_vec(value) {
        Ok(body) => HttpResponse {
            status,
            content_type: "application/json; charset=utf-8",
            headers: Vec::new(),
            body: ResponseBody::Bytes(body),
        },
        Err(error) => HttpResponse {
            status: 500,
            content_type: "application/json; charset=utf-8",
            headers: Vec::new(),
            body: ResponseBody::Bytes(
                serde_json::to_vec(&json!({"error":error.to_string()})).unwrap_or_default(),
            ),
        },
    }
}

fn html(value: &str) -> HttpResponse {
    text("text/html; charset=utf-8", value.to_string())
}

fn text(content_type: &'static str, value: String) -> HttpResponse {
    HttpResponse {
        status: 200,
        content_type,
        headers: Vec::new(),
        body: ResponseBody::Bytes(value.into_bytes()),
    }
}

fn file_response(
    path: PathBuf,
    content_type: &'static str,
    headers: Vec<(String, String)>,
) -> HttpResponse {
    HttpResponse {
        status: 200,
        content_type,
        headers,
        body: ResponseBody::File(path),
    }
}

fn empty(status: u16) -> HttpResponse {
    HttpResponse {
        status,
        content_type: "text/plain; charset=utf-8",
        headers: Vec::new(),
        body: ResponseBody::Bytes(Vec::new()),
    }
}

fn write_response(stream: &mut TcpStream, response: HttpResponse) -> std::io::Result<()> {
    let reason = match response.status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        409 => "Conflict",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "OK",
    };
    let content_length = match &response.body {
        ResponseBody::Bytes(body) => body.len() as u64,
        ResponseBody::File(path) => std::fs::metadata(path)?.len(),
    };
    let mut headers = format!(
        concat!(
            "HTTP/1.1 {} {}\r\n",
            "Content-Type: {}\r\n",
            "Content-Length: {}\r\n",
            "Cache-Control: no-store\r\n",
            "Access-Control-Allow-Origin: *\r\n",
            "Access-Control-Allow-Methods: GET,POST,OPTIONS\r\n",
            "Access-Control-Allow-Headers: Content-Type\r\n",
            "X-NetCore-Security-Mode: open_lab\r\n"
        ),
        response.status, reason, response.content_type, content_length
    );
    for (name, value) in response.headers {
        headers.push_str(&format!("{name}: {value}\r\n"));
    }
    headers.push_str("Connection: close\r\n\r\n");
    stream.write_all(headers.as_bytes())?;
    match response.body {
        ResponseBody::Bytes(body) => stream.write_all(&body),
        ResponseBody::File(path) => {
            let mut file = File::open(path)?;
            std::io::copy(&mut file, stream)?;
            Ok(())
        }
    }
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
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let (Some(high), Some(low)) = (
                hex_nibble(bytes[index + 1]),
                hex_nibble(bytes[index + 2]),
            )
        {
            output.push((high << 4) | low);
            index += 3;
            continue;
        }
        output.push(if bytes[index] == b'+' { b' ' } else { bytes[index] });
        index += 1;
    }
    String::from_utf8_lossy(&output).into_owned()
}

fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

const INDEX_HTML: &str = r#"<!doctype html><html lang="de"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>NetCore Recorder</title><style>:root{color-scheme:dark;--bg:#081116;--panel:#111f28;--line:#2a4451;--text:#edf5f8;--muted:#9eb2bc;--ok:#57d98e;--warn:#ffca58;--bad:#ff6d6d;--accent:#38a5ff}*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--text);font:14px system-ui}.lab{padding:10px 20px;background:#8e2020;color:#fff;text-align:center;font-weight:800}header{padding:20px 26px;background:#0e1a22;border-bottom:1px solid var(--line);display:flex;justify-content:space-between;gap:15px}h1,h2{margin:0 0 10px}.wrap{padding:20px;display:grid;gap:16px}.cards{display:grid;grid-template-columns:repeat(auto-fit,minmax(135px,1fr));gap:10px}.card,.panel{background:var(--panel);border:1px solid var(--line);border-radius:10px;padding:14px}.value{font-size:24px;font-weight:760}.muted{color:var(--muted)}.ok{color:var(--ok)}.bad{color:var(--bad)}.warn{color:var(--warn)}.toolbar{display:flex;gap:8px;flex-wrap:wrap;margin:10px 0}input,select,button{background:#192d38;color:var(--text);border:1px solid var(--line);border-radius:6px;padding:8px}button{cursor:pointer}.primary{background:#1268aa}.danger{background:#893039}.hold{background:#6f5317}table{width:100%;border-collapse:collapse}th,td{text-align:left;padding:8px;border-bottom:1px solid var(--line);vertical-align:top}.tablewrap{overflow:auto}pre{white-space:pre-wrap;max-height:280px;overflow:auto}@media(max-width:750px){header{display:block}.toolbar>*{flex:1 1 150px}}</style></head><body><div class="lab">⚠ OPEN LAB – keine Anmeldung, keine Tokens, kein TLS. Jeder erreichbare Client darf Aufnahmen exportieren, Aufbewahrung ändern und – sofern erlaubt – löschen.</div><header><div><h1>Recorder</h1><div class="muted">Passive TETRA-Aufzeichnung, Metadaten, Integrität und Retention</div></div><div id="links">Verbindungen …</div></header><main class="wrap"><section class="cards" id="cards"></section><section class="panel"><h2>Aktive Aufnahmen</h2><div class="tablewrap"><table><thead><tr><th>Call</th><th>Ziel / Sprecher</th><th>Start / Frames</th><th>Status</th><th>Aktion</th></tr></thead><tbody id="activeRows"></tbody></table></div></section><section class="panel"><h2>Aufnahmen</h2><div class="toolbar"><input id="search" placeholder="Call, ISSI, GSSI, TBS …"><input id="gssi" type="number" placeholder="GSSI"><input id="issi" type="number" placeholder="ISSI"><select id="emergency"><option value="">alle</option><option value="true">nur Notruf</option><option value="false">ohne Notruf</option></select><button class="primary" onclick="refresh()">Suchen</button></div><div class="tablewrap"><table><thead><tr><th>Zeit / ID</th><th>Ruf</th><th>Frames / Dauer</th><th>Integrität / Retention</th><th>Aktionen</th></tr></thead><tbody id="recordingRows"></tbody></table></div></section><section class="panel"><h2>Ereignisse</h2><pre id="events" class="muted"></pre></section></main><script>const el=id=>document.getElementById(id);async function api(path,opt){const r=await fetch(path,opt);if(!r.ok&&r.status!==204){let e={};try{e=await r.json()}catch{}throw new Error(e.error||r.statusText)}return r.status===204?null:r.json()}function esc(v){return String(v??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]))}function post(path,body){return api(path,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body||{})})}function fmtBytes(n){if(n==null)return '–';const u=['B','KiB','MiB','GiB','TiB'];let i=0;while(n>=1024&&i<u.length-1){n/=1024;i++}return n.toFixed(i?1:0)+' '+u[i]}function callLabel(r){return r.gssi?'GSSI '+r.gssi:(r.calling_issi||'–')+' → '+(r.called_issi||'–')}async function refresh(){try{const params=new URLSearchParams();if(el('search').value)params.set('q',el('search').value);if(el('gssi').value)params.set('gssi',el('gssi').value);if(el('issi').value)params.set('issi',el('issi').value);if(el('emergency').value)params.set('emergency',el('emergency').value);params.set('limit','500');const[s,a,r,e]=await Promise.all([api('/api/v1/status'),api('/api/v1/active'),api('/api/v1/recordings?'+params),api('/api/v1/events?limit=50')]);el('links').innerHTML=(s.media_switch_connected?'<span class="ok">● Media Switch</span>':'<span class="bad">● Media Switch</span>')+' &nbsp; '+(s.storage_available?'<span class="ok">● Storage</span>':'<span class="bad">● Storage</span>');el('cards').innerHTML=[['aktiv',s.active_recordings],['abgeschlossen',s.completed_recordings],['Frames',s.frames_ingested],['vor Recorder verloren',s.frames_lost_before_recorder],['Speicher',fmtBytes(s.storage_used_bytes)],['frei',fmtBytes(s.storage_free_bytes)],['Cursor',s.media_cursor],['Recovered',s.recordings_recovered]].map(x=>`<div class="card"><div class="muted">${x[0]}</div><div class="value">${x[1]}</div></div>`).join('');el('activeRows').innerHTML=a.map(x=>{const r=x.metadata;return `<tr><td><b>${esc(r.session_id)}</b><br><span class="muted">${esc(r.id)}</span></td><td>${callLabel(r)}<br>Sprecher: ${[...r.speakers].join(', ')||'–'}</td><td>${esc(r.started_at)}<br>${r.frame_count} Frames / ${fmtBytes(r.audio_bytes)}</td><td>${x.session_missing?'<span class="warn">Session fehlt</span>':'<span class="ok">nimmt auf</span>'}<br>${x.seconds_since_last_frame}s ohne Frame</td><td><button class="danger" onclick="finalizeRec('${r.id}')">Finalisieren</button></td></tr>`}).join('');el('recordingRows').innerHTML=r.map(x=>`<tr><td>${esc(x.started_at)}<br><b>${esc(x.id)}</b>${x.emergency?'<br><span class="bad">NOTRUF</span>':''}</td><td>${esc(x.call_kind)} / ${callLabel(x)}<br><span class="muted">${[...x.source_nodes].join(', ')}</span></td><td>${x.frame_count} / ${Math.round(x.duration_ms/1000)}s<br>${fmtBytes(x.audio_bytes)}${x.lost_tap_frames?'<br><span class="warn">'+x.lost_tap_frames+' Frames fehlen</span>':''}</td><td>${esc(x.integrity_status)}${x.legal_hold?' / <span class="warn">HOLD</span>':''}<br>bis ${esc(x.retention_until)}</td><td><button onclick="verifyRec('${x.id}')">Prüfen</button> <button onclick="location.href='/api/v1/recordings/${x.id}/export'">Export</button> <button class="hold" onclick="holdRec('${x.id}',${!x.legal_hold})">${x.legal_hold?'Hold lösen':'Hold'}</button> <button onclick="retentionRec('${x.id}')">Retention</button> <button class="danger" onclick="deleteRec('${x.id}')">Löschen</button></td></tr>`).join('');el('events').textContent=e.map(x=>`${x.timestamp} #${x.seq} ${x.kind} ${x.recording_id||''} ${x.session_id||''}`).join('\n')}catch(e){el('links').innerHTML='<span class="bad">UI-Fehler: '+esc(e.message)+'</span>'}}async function verifyRec(id){try{await post(`/api/v1/recordings/${id}/verify`,{});refresh()}catch(e){alert(e.message);refresh()}}async function finalizeRec(id){if(!confirm('Aktive Aufnahme jetzt finalisieren?'))return;try{await post(`/api/v1/recordings/${id}/finalize`,{});refresh()}catch(e){alert(e.message)}}async function holdRec(id,legal_hold){try{await post(`/api/v1/recordings/${id}/hold`,{legal_hold});refresh()}catch(e){alert(e.message)}}async function retentionRec(id){const days=Number(prompt('Aufbewahrung in Tagen:',30));if(!Number.isFinite(days)||days<1)return;try{await post(`/api/v1/recordings/${id}/retention`,{days});refresh()}catch(e){alert(e.message)}}async function deleteRec(id){if(!confirm('Aufnahme endgültig löschen? Das ist absichtlich keine Papierkorb-Aktion.'))return;try{await post(`/api/v1/recordings/${id}/delete`,{});refresh()}catch(e){alert(e.message)}}refresh();setInterval(refresh,2500)</script></body></html>"#;

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::Sender;
use std::thread;

use serde::Serialize;
use serde_json::json;

use crate::config::GroupCoreConfig;
use crate::protocol::BackendRequest;
use crate::state::{DgnaInput, GroupInput, ImportRequest, MembershipInput, SharedGroups};

pub fn spawn_http_server(
    config: GroupCoreConfig,
    groups: SharedGroups,
    gateway_tx: Sender<BackendRequest>,
) -> std::io::Result<thread::JoinHandle<()>> {
    let listener = TcpListener::bind(config.server.bind)?;
    tracing::info!("Group Core WebUI/API listening on http://{}", config.server.bind);
    Ok(thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let groups = groups.clone();
                    let gateway_tx = gateway_tx.clone();
                    let config = config.clone();
                    thread::spawn(move || {
                        if let Err(error) = handle_connection(stream, groups, gateway_tx, config) {
                            tracing::warn!("HTTP connection failed: {}", error);
                        }
                    });
                }
                Err(error) => tracing::warn!("HTTP accept failed: {}", error),
            }
        }
    }))
}

struct HttpRequest { method: String, path: String, query: HashMap<String, String>, body: Vec<u8> }
struct HttpResponse { status: u16, content_type: &'static str, body: Vec<u8>, disposition: Option<String> }

fn handle_connection(mut stream: TcpStream, groups: SharedGroups, gateway_tx: Sender<BackendRequest>, config: GroupCoreConfig) -> Result<(), String> {
    let request = read_request(&mut stream, config.limits.max_body_bytes)?;
    let response = route(request, groups, gateway_tx, config);
    write_response(&mut stream, response).map_err(|error| error.to_string())
}

fn route(request: HttpRequest, groups: SharedGroups, gateway_tx: Sender<BackendRequest>, config: GroupCoreConfig) -> HttpResponse {
    if request.method == "OPTIONS" { return empty(204); }
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") => html(INDEX_HTML),
        ("GET", "/health/live") => json_response(200, &json!({"status":"live"})),
        ("GET", "/health/ready") => { let status = groups.status(); json_response(if status.node_gateway_connected {200} else {503}, &status) }
        ("GET", "/api/v1/status") => json_response(200, &groups.status()),
        ("GET", "/api/v1/nodes") => json_response(200, &groups.nodes()),
        ("GET", "/api/v1/groups") => json_response(200, &groups.groups()),
        ("GET", "/api/v1/memberships") => json_response(200, &groups.memberships()),
        ("GET", "/api/v1/affiliations") => json_response(200, &groups.affiliations()),
        ("GET", "/api/v1/syncs") => json_response(200, &groups.syncs()),
        ("GET", "/api/v1/dgna") => json_response(200, &groups.dgna_operations()),
        ("GET", "/api/v1/events") => { let limit = request.query.get("limit").and_then(|value| value.parse::<usize>().ok()).unwrap_or(100).min(1000); json_response(200, &groups.recent_events(limit)) }
        ("GET", "/api/v1/config") => json_response(200, &config),
        ("GET", "/api/v1/export.json") => download_json("netcore-groups.json", &groups.export_database()),
        ("GET", "/metrics") => text("text/plain; version=0.0.4; charset=utf-8", groups.metrics()),
        ("GET", "/openapi.json") => json_response(200, &openapi()),
        ("POST", "/api/v1/groups") => match parse_json::<GroupInput>(&request.body).and_then(|input| groups.create_group(input)) {
            Ok((profile, commands)) => dispatch_response(&gateway_tx, commands, 201, &profile),
            Err(error) => json_response(409, &json!({"error": error})),
        },
        ("POST", "/api/v1/memberships") => match parse_json::<MembershipInput>(&request.body).and_then(|input| groups.upsert_membership(input)) {
            Ok((membership, commands)) => dispatch_response(&gateway_tx, commands, 200, &membership),
            Err(error) => json_response(409, &json!({"error": error})),
        },
        ("POST", "/api/v1/sync") => { let commands = groups.sync_all(); let count = commands.len(); match dispatch_all(&gateway_tx, commands) { Ok(()) => json_response(202, &json!({"queued": count})), Err(error) => json_response(503, &json!({"error": error})) } }
        ("POST", "/api/v1/dgna") => match parse_json::<DgnaInput>(&request.body).and_then(|input| groups.request_dgna(input)) {
            Ok((operation, commands)) => dispatch_response(&gateway_tx, commands, 202, &operation),
            Err(error) => json_response(409, &json!({"error": error})),
        },
        ("POST", "/api/v1/import") => match parse_json::<ImportRequest>(&request.body).and_then(|input| groups.import_database(input)) {
            Ok((count, commands)) => match dispatch_all(&gateway_tx, commands) { Ok(()) => json_response(200, &json!({"records": count})), Err(error) => json_response(503, &json!({"error": error})) },
            Err(error) => json_response(409, &json!({"error": error})),
        },
        _ if request.path.starts_with("/api/v1/groups/") => group_route(request, groups, gateway_tx),
        _ if request.path.starts_with("/api/v1/memberships/") => membership_route(request, groups, gateway_tx),
        _ if request.path.starts_with("/api/v1/dgna/") => dgna_route(request, groups),
        _ => json_response(404, &json!({"error":"not found"})),
    }
}

fn group_route(request: HttpRequest, groups: SharedGroups, gateway_tx: Sender<BackendRequest>) -> HttpResponse {
    let tail = request.path.trim_start_matches("/api/v1/groups/");
    let Ok(gssi) = tail.parse::<u32>() else { return json_response(400, &json!({"error":"invalid GSSI"})); };
    match request.method.as_str() {
        "GET" => groups.group(gssi).map_or_else(|| json_response(404, &json!({"error":"not found"})), |profile| json_response(200, &profile)),
        "PUT" => match parse_json::<GroupInput>(&request.body).and_then(|input| groups.update_group(gssi, input)) {
            Ok((profile, commands)) => dispatch_response(&gateway_tx, commands, 200, &profile),
            Err(error) => json_response(409, &json!({"error": error})),
        },
        "DELETE" => match groups.delete_group(gssi) {
            Ok(commands) => match dispatch_all(&gateway_tx, commands) { Ok(()) => empty(204), Err(error) => json_response(503, &json!({"error": error})) },
            Err(error) => json_response(404, &json!({"error": error})),
        },
        _ => json_response(405, &json!({"error":"method not allowed"})),
    }
}

fn membership_route(request: HttpRequest, groups: SharedGroups, gateway_tx: Sender<BackendRequest>) -> HttpResponse {
    let parts: Vec<&str> = request.path.trim_start_matches("/api/v1/memberships/").split('/').collect();
    if parts.len() != 2 { return json_response(400, &json!({"error":"expected /memberships/{issi}/{gssi}"})); }
    let (Ok(issi), Ok(gssi)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) else { return json_response(400, &json!({"error":"invalid identity"})); };
    match request.method.as_str() {
        "DELETE" => match groups.delete_membership(issi, gssi) {
            Ok(commands) => match dispatch_all(&gateway_tx, commands) { Ok(()) => empty(204), Err(error) => json_response(503, &json!({"error": error})) },
            Err(error) => json_response(404, &json!({"error": error})),
        },
        _ => json_response(405, &json!({"error":"method not allowed"})),
    }
}

fn dgna_route(request: HttpRequest, groups: SharedGroups) -> HttpResponse {
    let tail = request.path.trim_start_matches("/api/v1/dgna/");
    let Some(id) = tail.strip_suffix("/cancel") else { return json_response(404, &json!({"error":"not found"})); };
    if request.method != "POST" { return json_response(405, &json!({"error":"method not allowed"})); }
    match groups.cancel_dgna(id) { Ok(()) => empty(204), Err(error) => json_response(409, &json!({"error": error})) }
}

fn dispatch_response<T: Serialize>(tx: &Sender<BackendRequest>, commands: Vec<BackendRequest>, status: u16, value: &T) -> HttpResponse {
    match dispatch_all(tx, commands) { Ok(()) => json_response(status, value), Err(error) => json_response(503, &json!({"error": error})) }
}
fn dispatch_all(tx: &Sender<BackendRequest>, commands: Vec<BackendRequest>) -> Result<(), String> { for command in commands { tx.send(command).map_err(|_| "node gateway worker is unavailable".to_string())?; } Ok(()) }
fn parse_json<T: serde::de::DeserializeOwned>(body: &[u8]) -> Result<T, String> { serde_json::from_slice(body).map_err(|error| format!("invalid JSON: {error}")) }

fn openapi() -> serde_json::Value { json!({"openapi":"3.0.3","info":{"title":"NetCore Group Core","version":env!("CARGO_PKG_VERSION"),"description":"OPEN LAB API. No authentication, no token and no TLS."},"paths":{"/api/v1/status":{"get":{}},"/api/v1/groups":{"get":{},"post":{}},"/api/v1/groups/{gssi}":{"get":{},"put":{},"delete":{}},"/api/v1/memberships":{"get":{},"post":{}},"/api/v1/memberships/{issi}/{gssi}":{"delete":{}},"/api/v1/affiliations":{"get":{}},"/api/v1/dgna":{"get":{},"post":{}},"/api/v1/sync":{"post":{}},"/api/v1/export.json":{"get":{}},"/health/live":{"get":{}},"/health/ready":{"get":{}},"/metrics":{"get":{}}}}) }

fn json_response<T: Serialize>(status: u16, value: &T) -> HttpResponse { match serde_json::to_vec_pretty(value) { Ok(body) => HttpResponse { status, content_type: "application/json; charset=utf-8", body, disposition: None }, Err(error) => HttpResponse { status: 500, content_type: "application/json; charset=utf-8", body: format!("{{\"error\":\"serialization failed: {error}\"}}").into_bytes(), disposition: None } } }
fn download_json<T: Serialize>(name: &str, value: &T) -> HttpResponse { match serde_json::to_vec_pretty(value) { Ok(body) => HttpResponse { status: 200, content_type: "application/json; charset=utf-8", body, disposition: Some(format!("attachment; filename=\"{name}\"")) }, Err(error) => json_response(500, &json!({"error": error.to_string()})) } }
fn text(content_type: &'static str, value: String) -> HttpResponse { HttpResponse { status: 200, content_type, body: value.into_bytes(), disposition: None } }
fn html(value: &str) -> HttpResponse { HttpResponse { status: 200, content_type: "text/html; charset=utf-8", body: value.as_bytes().to_vec(), disposition: None } }
fn empty(status: u16) -> HttpResponse { HttpResponse { status, content_type: "text/plain; charset=utf-8", body: Vec::new(), disposition: None } }

fn read_request(stream: &mut TcpStream, max_body_bytes: usize) -> Result<HttpRequest, String> {
    let mut buffer = Vec::new(); let mut chunk = [0u8; 4096];
    let header_end = loop { let read = stream.read(&mut chunk).map_err(|error| format!("request read failed: {error}"))?; if read == 0 { return Err("connection closed before request was complete".into()); } buffer.extend_from_slice(&chunk[..read]); if buffer.len() > max_body_bytes + 65_536 { return Err("request too large".into()); } if let Some(position) = find_subslice(&buffer, b"\r\n\r\n") { break position + 4; } };
    let header_text = std::str::from_utf8(&buffer[..header_end]).map_err(|_| "request headers are not utf-8".to_string())?;
    let mut lines = header_text.split("\r\n"); let request_line = lines.next().ok_or_else(|| "missing request line".to_string())?;
    let mut parts = request_line.split_whitespace(); let method = parts.next().ok_or_else(|| "missing method".to_string())?.to_ascii_uppercase(); let raw_path = parts.next().ok_or_else(|| "missing path".to_string())?; let (path, query) = parse_path_and_query(raw_path);
    let mut content_length = 0usize; for line in lines { if let Some((name, value)) = line.split_once(':') { if name.eq_ignore_ascii_case("content-length") { content_length = value.trim().parse().map_err(|_| "invalid content-length".to_string())?; } } }
    if content_length > max_body_bytes { return Err("body too large".into()); }
    let mut body = buffer[header_end..].to_vec(); while body.len() < content_length { let read = stream.read(&mut chunk).map_err(|error| format!("body read failed: {error}"))?; if read == 0 { return Err("connection closed before body was complete".into()); } body.extend_from_slice(&chunk[..read]); if body.len() > max_body_bytes { return Err("body too large".into()); } } body.truncate(content_length);
    Ok(HttpRequest { method, path, query, body })
}
fn write_response(stream: &mut TcpStream, response: HttpResponse) -> std::io::Result<()> { let reason = match response.status { 200=>"OK",201=>"Created",202=>"Accepted",204=>"No Content",400=>"Bad Request",404=>"Not Found",405=>"Method Not Allowed",409=>"Conflict",413=>"Payload Too Large",500=>"Internal Server Error",503=>"Service Unavailable",_=>"OK" }; let mut headers = format!("HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET,POST,PUT,DELETE,OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nX-NetCore-Security-Mode: open_lab\r\nConnection: close\r\n", response.status, reason, response.content_type, response.body.len()); if let Some(disposition) = response.disposition { headers.push_str(&format!("Content-Disposition: {disposition}\r\n")); } headers.push_str("\r\n"); stream.write_all(headers.as_bytes())?; stream.write_all(&response.body) }
fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> { haystack.windows(needle.len()).position(|window| window == needle) }
fn parse_path_and_query(raw: &str) -> (String, HashMap<String, String>) { let (path, raw_query) = raw.split_once('?').unwrap_or((raw, "")); let query = raw_query.split('&').filter(|value| !value.is_empty()).map(|item| item.split_once('=').unwrap_or((item, ""))).map(|(key, value)| (percent_decode(key), percent_decode(value))).collect(); (percent_decode(path), query) }
fn percent_decode(value: &str) -> String { let bytes = value.as_bytes(); let mut output = Vec::new(); let mut index = 0; while index < bytes.len() { if bytes[index] == b'%' && index + 2 < bytes.len() { if let Ok(hex) = u8::from_str_radix(&value[index + 1..index + 3], 16) { output.push(hex); index += 3; continue; } } output.push(if bytes[index] == b'+' { b' ' } else { bytes[index] }); index += 1; } String::from_utf8_lossy(&output).into_owned() }

const INDEX_HTML: &str = r#"<!doctype html><html lang="de"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>NetCore Group Core</title><style>:root{color-scheme:dark;--bg:#0b1218;--panel:#131e28;--line:#2a3b4a;--text:#e9f1f7;--muted:#9eb0bd;--accent:#36a3ff;--ok:#58d68d;--warn:#ffca58;--bad:#ff6b6b}*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--text);font:14px system-ui}header{display:flex;justify-content:space-between;gap:20px;padding:20px 28px;background:#101a23;border-bottom:1px solid var(--line)}h1,h2{margin:0 0 10px}.lab{padding:9px 20px;background:#8d1d1d;color:#fff;text-align:center;font-weight:700}.wrap{padding:22px;display:grid;gap:18px}.cards{display:grid;grid-template-columns:repeat(auto-fit,minmax(145px,1fr));gap:12px}.card,.panel{background:var(--panel);border:1px solid var(--line);border-radius:10px;padding:15px}.value{font-size:25px;font-weight:700}.muted{color:var(--muted)}.ok{color:var(--ok)}.bad{color:var(--bad)}.warn{color:var(--warn)}table{width:100%;border-collapse:collapse}th,td{text-align:left;padding:9px;border-bottom:1px solid var(--line);vertical-align:top}.toolbar{display:flex;gap:8px;flex-wrap:wrap;margin:10px 0}button,.btn,input,textarea,select{border:1px solid var(--line);background:#192733;color:var(--text);border-radius:6px;padding:8px}.primary{background:#1266a8}.danger{background:#8c2f39}.tablewrap{overflow:auto}dialog{background:var(--panel);color:var(--text);border:1px solid var(--line);border-radius:10px;max-width:780px;width:95%}form.grid{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:10px}label{display:grid;gap:4px}.wide{grid-column:1/-1}pre{white-space:pre-wrap}@media(max-width:720px){form.grid{grid-template-columns:1fr}.wide{grid-column:auto}}</style></head><body><div class="lab">⚠ OPEN LAB – keine Anmeldung, keine Tokens, kein TLS. Jeder erreichbare Client darf Gruppen, Mitgliedschaften und DGNA ändern.</div><header><div><h1>Group Core</h1><div class="muted">Zentrale GSSI-Verwaltung, Mitgliedschaften, Affiliationen und DGNA</div></div><div id="gateway">Gateway …</div></header><main class="wrap"><section class="cards" id="cards"></section><section class="panel"><h2>Gruppen</h2><div class="toolbar"><button class="primary" onclick="openGroup()">Gruppe anlegen</button><button onclick="syncNow()">Policy an alle TBS senden</button><a class="btn" href="/api/v1/export.json">JSON exportieren</a><input id="filter" placeholder="GSSI oder Name …" oninput="renderGroups()"></div><div class="tablewrap"><table><thead><tr><th>GSSI</th><th>Name</th><th>Status</th><th>Dienste</th><th>Prio</th><th>Bereich</th><th>Aktionen</th></tr></thead><tbody id="groupRows"></tbody></table></div></section><section class="panel"><h2>Mitgliedschaften</h2><div class="toolbar"><button class="primary" onclick="openMembership()">Mitgliedschaft hinzufügen</button></div><div class="tablewrap"><table><thead><tr><th>ISSI</th><th>GSSI</th><th>Erlaubt</th><th>Auto-Attach</th><th>Gesperrt</th><th>Aktion</th></tr></thead><tbody id="membershipRows"></tbody></table></div></section><section class="panel"><h2>DGNA</h2><div class="toolbar"><input id="dgnaNode" placeholder="Node-ID"><input id="dgnaIssi" type="number" placeholder="ISSI"><input id="dgnaGssi" type="number" placeholder="GSSI"><select id="dgnaAttach"><option value="true">Attach</option><option value="false">Detach</option></select><label><input id="dgnaForce" type="checkbox"> Force</label><button onclick="sendDgna()">Ausführen</button></div><div class="tablewrap"><table><thead><tr><th>Zeit</th><th>Node</th><th>ISSI</th><th>GSSI</th><th>Aktion</th><th>Status</th><th>Meldung</th></tr></thead><tbody id="dgnaRows"></tbody></table></div></section><section class="panel"><h2>Live-Affiliationen</h2><div class="tablewrap"><table><thead><tr><th>Node</th><th>ISSI</th><th>registriert</th><th>Gruppen</th><th>zuletzt</th></tr></thead><tbody id="affRows"></tbody></table></div></section><section class="panel"><h2>TBS-Synchronisation</h2><div class="tablewrap"><table><thead><tr><th>Node</th><th>Fähigkeit</th><th>Revision</th><th>Status</th><th>Meldung</th></tr></thead><tbody id="syncRows"></tbody></table></div></section><section class="panel"><h2>Ereignisse</h2><pre id="events" class="muted"></pre></section></main><dialog id="groupDialog"><form id="groupForm" class="grid" onsubmit="saveGroup(event)"><h2 class="wide" id="groupTitle">Gruppe</h2><label>GSSI<input name="gssi" type="number" min="1" max="16777215" required></label><label>Name<input name="name"></label><label class="wide">Beschreibung<textarea name="description"></textarea></label><label><span>Aktiv</span><input name="enabled" type="checkbox" checked></label><label><span>Attach erlaubt</span><input name="attach_allowed" type="checkbox" checked></label><label><span>DGNA erlaubt</span><input name="dgna_allowed" type="checkbox" checked></label><label><span>Ruf erlaubt</span><input name="call_allowed" type="checkbox" checked></label><label><span>SDS erlaubt</span><input name="sds_allowed" type="checkbox" checked></label><label><span>Notruf erlaubt</span><input name="emergency_allowed" type="checkbox"></label><label>Rufpriorität<input name="call_priority" type="number" min="0" max="15" value="0"></label><label>Class of Usage<input name="class_of_usage" type="number" min="0" max="15" value="4"></label><label class="wide">Node-Bereich, kommasepariert<input name="area_nodes"></label><label class="wide">Notizen<textarea name="notes"></textarea></label><div class="wide toolbar"><button class="primary" type="submit">Speichern</button><button type="button" onclick="groupDialog.close()">Abbrechen</button></div></form></dialog><dialog id="membershipDialog"><form id="membershipForm" class="grid" onsubmit="saveMembership(event)"><h2 class="wide">Mitgliedschaft</h2><label>ISSI<input name="issi" type="number" min="1" max="16777215" required></label><label>GSSI<input name="gssi" type="number" min="1" max="16777215" required></label><label><span>Erlaubt</span><input name="allowed" type="checkbox" checked></label><label><span>Auto-Attach</span><input name="auto_attach" type="checkbox"></label><label><span>Gesperrt</span><input name="locked" type="checkbox"></label><label class="wide">Notizen<textarea name="notes"></textarea></label><div class="wide toolbar"><button class="primary" type="submit">Speichern</button><button type="button" onclick="membershipDialog.close()">Abbrechen</button></div></form></dialog><script>let groups=[],memberships=[],nodes=[],syncs=[],affiliations=[],dgna=[];const groupDialog=document.getElementById('groupDialog'),groupForm=document.getElementById('groupForm'),membershipDialog=document.getElementById('membershipDialog'),membershipForm=document.getElementById('membershipForm');async function api(path,opt){const r=await fetch(path,opt);if(!r.ok&&r.status!==204){let e={};try{e=await r.json()}catch{}throw new Error(e.error||r.statusText)}return r.status===204?null:r.json()}function esc(v){return String(v??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]))}async function refresh(){try{const [s,g,m,n,y,a,d,e]=await Promise.all([api('/api/v1/status'),api('/api/v1/groups'),api('/api/v1/memberships'),api('/api/v1/nodes'),api('/api/v1/syncs'),api('/api/v1/affiliations'),api('/api/v1/dgna'),api('/api/v1/events?limit=35')]);groups=g;memberships=m;nodes=n;syncs=y;affiliations=a;dgna=d;document.getElementById('gateway').innerHTML=s.node_gateway_connected?'<span class="ok">● Gateway verbunden</span>':'<span class="bad">● Gateway getrennt</span>';document.getElementById('cards').innerHTML=[['Gruppen',s.groups_total],['aktiv',s.groups_enabled],['Mitgliedschaften',s.memberships_total],['Live-Affiliationen',s.observed_affiliations],['TBS verbunden',s.nodes_connected],['TBS synchron',s.nodes_synced],['Revision',s.database_revision],['DGNA offen',s.dgna_pending]].map(x=>`<div class="card"><div class="muted">${x[0]}</div><div class="value">${x[1]}</div></div>`).join('');renderGroups();renderMemberships();renderAffiliations();renderSyncs();renderDgna();document.getElementById('events').textContent=e.map(x=>`${x.timestamp} #${x.seq} ${x.kind} ${x.node_id||''} ${x.issi||''} ${x.gssi||''}`).join('\n')}catch(e){document.getElementById('gateway').innerHTML='<span class="bad">UI-Fehler: '+esc(e.message)+'</span>'}}function renderGroups(){const q=document.getElementById('filter').value.toLowerCase();document.getElementById('groupRows').innerHTML=groups.filter(g=>`${g.gssi} ${g.name}`.toLowerCase().includes(q)).map(g=>`<tr><td><b>${g.gssi}</b></td><td>${esc(g.name)}<br><span class="muted">${esc(g.description)}</span></td><td>${g.enabled?'<span class="ok">aktiv</span>':'<span class="bad">deaktiviert</span>'}</td><td>${g.attach_allowed?'Attach ':''}${g.dgna_allowed?'DGNA ':''}${g.call_allowed?'Ruf ':''}${g.sds_allowed?'SDS ':''}${g.emergency_allowed?'Notruf':''}</td><td>${g.call_priority}</td><td>${g.area_nodes.length?g.area_nodes.join(', '):'alle TBS'}</td><td><button onclick="editGroup(${g.gssi})">Bearbeiten</button> <button class="danger" onclick="deleteGroup(${g.gssi})">Löschen</button></td></tr>`).join('')}function renderMemberships(){document.getElementById('membershipRows').innerHTML=memberships.map(m=>`<tr><td>${m.issi}</td><td>${m.gssi}</td><td>${m.allowed?'ja':'nein'}</td><td>${m.auto_attach?'ja':'nein'}</td><td>${m.locked?'ja':'nein'}</td><td><button class="danger" onclick="deleteMembership(${m.issi},${m.gssi})">Löschen</button></td></tr>`).join('')}function renderAffiliations(){document.getElementById('affRows').innerHTML=affiliations.map(a=>`<tr><td>${esc(a.node_id)}</td><td>${a.issi}</td><td>${a.registered?'<span class="ok">ja</span>':'nein'}</td><td>${[...a.groups].join(', ')}</td><td>${esc(a.last_seen)}</td></tr>`).join('')}function renderSyncs(){const map=new Map(nodes.map(n=>[n.node_id,n]));document.getElementById('syncRows').innerHTML=[...new Set([...nodes.map(n=>n.node_id),...syncs.map(s=>s.node_id)])].map(id=>{const n=map.get(id),s=syncs.find(x=>x.node_id===id);return `<tr><td>${esc(id)}<br><span class="muted">${esc(n?.station_name||'')}</span></td><td>${n?.group_policy_capable?'<span class="ok">Group Policy</span>':'<span class="bad">nicht unterstützt</span>'}</td><td>${s?.applied_revision??'–'} / ${s?.desired_revision??'–'}</td><td>${esc(s?.phase||'noch nie')}</td><td>${esc(s?.message||'')}</td></tr>`}).join('')}function renderDgna(){document.getElementById('dgnaRows').innerHTML=dgna.slice(0,50).map(d=>`<tr><td>${esc(d.requested_at)}</td><td>${esc(d.node_id)}</td><td>${d.issi}</td><td>${d.gssi}</td><td>${d.attach?'Attach':'Detach'}${d.force?' / Force':''}</td><td>${esc(d.phase)}</td><td>${esc(d.message||'')}</td></tr>`).join('')}function openGroup(){groupForm.reset();groupForm.dataset.gssi='';groupForm.enabled.checked=true;groupForm.attach_allowed.checked=true;groupForm.dgna_allowed.checked=true;groupForm.call_allowed.checked=true;groupForm.sds_allowed.checked=true;groupForm.class_of_usage.value=4;document.getElementById('groupTitle').textContent='Gruppe anlegen';groupDialog.showModal()}function editGroup(gssi){const g=groups.find(x=>x.gssi===gssi);if(!g)return;groupForm.dataset.gssi=gssi;for(const[k,v]of Object.entries(g)){const el=groupForm.elements[k];if(!el)continue;if(el.type==='checkbox')el.checked=!!v;else if(k==='area_nodes')el.value=[...v].join(', ');else el.value=v??''}groupForm.elements.gssi.disabled=true;document.getElementById('groupTitle').textContent='GSSI '+gssi;groupDialog.showModal()}groupDialog.addEventListener('close',()=>groupForm.elements.gssi.disabled=false);function groupPayload(){const f=new FormData(groupForm),n=x=>Number(f.get(x)||0),b=x=>groupForm.elements[x].checked;return{gssi:n('gssi')||Number(groupForm.dataset.gssi),name:f.get('name')||'',description:f.get('description')||'',enabled:b('enabled'),attach_allowed:b('attach_allowed'),dgna_allowed:b('dgna_allowed'),call_allowed:b('call_allowed'),sds_allowed:b('sds_allowed'),emergency_allowed:b('emergency_allowed'),call_priority:n('call_priority'),class_of_usage:n('class_of_usage'),area_nodes:(f.get('area_nodes')||'').split(',').map(x=>x.trim()).filter(Boolean),notes:f.get('notes')||''}}async function saveGroup(e){e.preventDefault();try{const p=groupPayload(),existing=groupForm.dataset.gssi;await api(existing?'/api/v1/groups/'+existing:'/api/v1/groups',{method:existing?'PUT':'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(p)});groupDialog.close();refresh()}catch(e){alert(e.message)}}async function deleteGroup(gssi){if(!confirm('GSSI '+gssi+' samt Mitgliedschaften löschen?'))return;try{await api('/api/v1/groups/'+gssi,{method:'DELETE'});refresh()}catch(e){alert(e.message)}}function openMembership(){membershipForm.reset();membershipForm.allowed.checked=true;membershipDialog.showModal()}async function saveMembership(e){e.preventDefault();const f=new FormData(membershipForm),b=x=>membershipForm.elements[x].checked;try{await api('/api/v1/memberships',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({issi:Number(f.get('issi')),gssi:Number(f.get('gssi')),allowed:b('allowed'),auto_attach:b('auto_attach'),locked:b('locked'),notes:f.get('notes')||''})});membershipDialog.close();refresh()}catch(e){alert(e.message)}}async function deleteMembership(issi,gssi){if(!confirm('Mitgliedschaft löschen?'))return;try{await api(`/api/v1/memberships/${issi}/${gssi}`,{method:'DELETE'});refresh()}catch(e){alert(e.message)}}async function syncNow(){try{const r=await api('/api/v1/sync',{method:'POST'});alert(r.queued+' Synchronisation(en) eingeplant');refresh()}catch(e){alert(e.message)}}async function sendDgna(){try{await api('/api/v1/dgna',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({node_id:document.getElementById('dgnaNode').value,issi:Number(document.getElementById('dgnaIssi').value),gssi:Number(document.getElementById('dgnaGssi').value),attach:document.getElementById('dgnaAttach').value==='true',force:document.getElementById('dgnaForce').checked,update_membership:false,auto_attach:false})});refresh()}catch(e){alert(e.message)}}refresh();setInterval(refresh,5000)</script></body></html>"#;

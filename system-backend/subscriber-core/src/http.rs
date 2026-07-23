use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::Sender;
use std::thread;

use serde::Serialize;
use serde_json::json;

use crate::config::SubscriberCoreConfig;
use crate::protocol::BackendRequest;
use crate::state::{ImportRequest, SharedSubscribers, SubscriberInput};

pub fn spawn_http_server(
    config: SubscriberCoreConfig,
    subscribers: SharedSubscribers,
    gateway_tx: Sender<BackendRequest>,
) -> std::io::Result<thread::JoinHandle<()>> {
    let listener = TcpListener::bind(config.server.bind)?;
    tracing::info!("Subscriber Core WebUI/API listening on http://{}", config.server.bind);
    Ok(thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let subscribers = subscribers.clone();
                    let gateway_tx = gateway_tx.clone();
                    let config = config.clone();
                    thread::spawn(move || {
                        if let Err(error) = handle_connection(stream, subscribers, gateway_tx, config) {
                            tracing::warn!("HTTP connection failed: {}", error);
                        }
                    });
                }
                Err(error) => tracing::warn!("HTTP accept failed: {}", error),
            }
        }
    }))
}

struct HttpRequest { method: String, path: String, query: HashMap<String,String>, body: Vec<u8> }
struct HttpResponse { status: u16, content_type: &'static str, body: Vec<u8>, disposition: Option<String> }

fn handle_connection(mut stream: TcpStream, subscribers: SharedSubscribers, gateway_tx: Sender<BackendRequest>, config: SubscriberCoreConfig) -> Result<(), String> {
    let request = read_request(&mut stream, config.limits.max_body_bytes)?;
    let response = route(request, subscribers, gateway_tx, config);
    write_response(&mut stream, response).map_err(|error| error.to_string())
}

fn route(request: HttpRequest, subscribers: SharedSubscribers, gateway_tx: Sender<BackendRequest>, config: SubscriberCoreConfig) -> HttpResponse {
    if request.method == "OPTIONS" { return empty(204); }
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") => html(INDEX_HTML),
        ("GET", "/health/live") => json_response(200, &json!({"status":"live"})),
        ("GET", "/health/ready") => {
            let status = subscribers.status();
            let code = if status.node_gateway_connected {200} else {503}; json_response(code,&status)
        }
        ("GET", "/api/v1/status") => json_response(200,&subscribers.status()),
        ("GET", "/api/v1/nodes") => json_response(200,&subscribers.nodes()),
        ("GET", "/api/v1/subscribers") => json_response(200,&subscribers.subscribers()),
        ("GET", "/api/v1/observed") => json_response(200,&subscribers.observed()),
        ("GET", "/api/v1/syncs") => json_response(200,&subscribers.syncs()),
        ("GET", "/api/v1/events") => {
            let limit=request.query.get("limit").and_then(|v|v.parse::<usize>().ok()).unwrap_or(100).min(1000);
            json_response(200,&subscribers.recent_events(limit))
        }
        ("GET", "/api/v1/config") => json_response(200,&config),
        ("GET", "/api/v1/export.json") => download_json("netcore-subscribers.json",&subscribers.export_database()),
        ("GET", "/api/v1/export.csv") => download("text/csv; charset=utf-8","netcore-subscribers.csv",subscribers.export_csv().into_bytes()),
        ("GET", "/metrics") => text("text/plain; version=0.0.4; charset=utf-8",subscribers.metrics()),
        ("GET", "/openapi.json") => json_response(200,&openapi()),
        ("POST", "/api/v1/subscribers") => {
            match parse_json::<SubscriberInput>(&request.body).and_then(|input| subscribers.create_subscriber(input)) {
                Ok((profile,commands)) => match dispatch_all(&gateway_tx,commands) { Ok(())=>json_response(201,&profile), Err(e)=>json_response(503,&json!({"error":e})) },
                Err(e)=>json_response(409,&json!({"error":e})),
            }
        }
        ("POST", "/api/v1/import") => {
            match parse_json::<ImportRequest>(&request.body).and_then(|input| subscribers.import_subscribers(input)) {
                Ok((count,commands)) => match dispatch_all(&gateway_tx,commands) { Ok(())=>json_response(200,&json!({"imported":count})), Err(e)=>json_response(503,&json!({"error":e})) },
                Err(e)=>json_response(409,&json!({"error":e})),
            }
        }
        ("POST", "/api/v1/sync") => {
            let commands=subscribers.sync_all(); let count=commands.len();
            match dispatch_all(&gateway_tx,commands) { Ok(())=>json_response(202,&json!({"queued":count})), Err(e)=>json_response(503,&json!({"error":e})) }
        }
        _ if request.path.starts_with("/api/v1/subscribers/") => subscriber_route(request,subscribers,gateway_tx),
        _ => json_response(404,&json!({"error":"not found"})),
    }
}

fn subscriber_route(request: HttpRequest, subscribers: SharedSubscribers, gateway_tx: Sender<BackendRequest>) -> HttpResponse {
    let tail=request.path.trim_start_matches("/api/v1/subscribers/");
    let Ok(issi)=tail.parse::<u32>() else { return json_response(400,&json!({"error":"invalid ISSI"})); };
    match request.method.as_str() {
        "GET" => subscribers.subscriber(issi).map_or_else(||json_response(404,&json!({"error":"not found"})),|p|json_response(200,&p)),
        "PUT" => match parse_json::<SubscriberInput>(&request.body).and_then(|input|subscribers.update_subscriber(issi,input)) {
            Ok((profile,commands))=>match dispatch_all(&gateway_tx,commands){Ok(())=>json_response(200,&profile),Err(e)=>json_response(503,&json!({"error":e}))},
            Err(e)=>json_response(409,&json!({"error":e})),
        },
        "DELETE" => match subscribers.delete_subscriber(issi) {
            Ok(commands)=>match dispatch_all(&gateway_tx,commands){Ok(())=>empty(204),Err(e)=>json_response(503,&json!({"error":e}))},
            Err(e)=>json_response(404,&json!({"error":e})),
        },
        _=>json_response(405,&json!({"error":"method not allowed"})),
    }
}

fn dispatch_all(tx:&Sender<BackendRequest>,commands:Vec<BackendRequest>)->Result<(),String>{for command in commands{tx.send(command).map_err(|_|"node gateway worker is unavailable".to_string())?;}Ok(())}
fn parse_json<T:serde::de::DeserializeOwned>(body:&[u8])->Result<T,String>{serde_json::from_slice(body).map_err(|e|format!("invalid JSON: {e}"))}

fn openapi()->serde_json::Value{json!({"openapi":"3.0.3","info":{"title":"NetCore Subscriber Core","version":env!("CARGO_PKG_VERSION"),"description":"OPEN LAB API. No authentication, no token and no TLS."},"paths":{"/api/v1/status":{"get":{}},"/api/v1/subscribers":{"get":{},"post":{}},"/api/v1/subscribers/{issi}":{"get":{},"put":{},"delete":{}},"/api/v1/observed":{"get":{}},"/api/v1/nodes":{"get":{}},"/api/v1/syncs":{"get":{}},"/api/v1/sync":{"post":{}},"/api/v1/import":{"post":{}},"/api/v1/export.json":{"get":{}},"/api/v1/export.csv":{"get":{}},"/health/live":{"get":{}},"/health/ready":{"get":{}},"/metrics":{"get":{}}}})}

fn json_response<T:Serialize>(status:u16,value:&T)->HttpResponse{match serde_json::to_vec_pretty(value){Ok(body)=>HttpResponse{status,content_type:"application/json; charset=utf-8",body,disposition:None},Err(e)=>HttpResponse{status:500,content_type:"application/json; charset=utf-8",body:format!("{{\"error\":\"serialization failed: {e}\"}}").into_bytes(),disposition:None}}}
fn download_json<T:Serialize>(name:&str,value:&T)->HttpResponse{match serde_json::to_vec_pretty(value){Ok(body)=>download("application/json; charset=utf-8",name,body),Err(e)=>json_response(500,&json!({"error":e.to_string()}))}}
fn download(content_type:&'static str,name:&str,body:Vec<u8>)->HttpResponse{HttpResponse{status:200,content_type,body,disposition:Some(format!("attachment; filename=\"{name}\""))}}
fn text(content_type:&'static str,value:String)->HttpResponse{HttpResponse{status:200,content_type,body:value.into_bytes(),disposition:None}}
fn html(value:&str)->HttpResponse{HttpResponse{status:200,content_type:"text/html; charset=utf-8",body:value.as_bytes().to_vec(),disposition:None}}
fn empty(status:u16)->HttpResponse{HttpResponse{status,content_type:"text/plain; charset=utf-8",body:Vec::new(),disposition:None}}

fn read_request(stream:&mut TcpStream,max_body_bytes:usize)->Result<HttpRequest,String>{let mut buffer=Vec::new();let mut chunk=[0u8;4096];let header_end=loop{let read=stream.read(&mut chunk).map_err(|e|format!("request read failed: {e}"))?;if read==0{return Err("connection closed before request was complete".into())}buffer.extend_from_slice(&chunk[..read]);if buffer.len()>max_body_bytes+65536{return Err("request too large".into())}if let Some(pos)=find_subslice(&buffer,b"\r\n\r\n"){break pos+4}};let header_text=std::str::from_utf8(&buffer[..header_end]).map_err(|_|"request headers are not utf-8".to_string())?;let mut lines=header_text.split("\r\n");let request_line=lines.next().ok_or_else(||"missing request line".to_string())?;let mut parts=request_line.split_whitespace();let method=parts.next().ok_or_else(||"missing method".to_string())?.to_ascii_uppercase();let raw_path=parts.next().ok_or_else(||"missing path".to_string())?;let(path,query)=parse_path_and_query(raw_path);let mut content_length=0usize;for line in lines{if let Some((name,value))=line.split_once(':'){if name.eq_ignore_ascii_case("content-length"){content_length=value.trim().parse().map_err(|_|"invalid content-length".to_string())?;}}}if content_length>max_body_bytes{return Err("body too large".into())}let mut body=buffer[header_end..].to_vec();while body.len()<content_length{let read=stream.read(&mut chunk).map_err(|e|format!("body read failed: {e}"))?;if read==0{return Err("connection closed before body was complete".into())}body.extend_from_slice(&chunk[..read]);if body.len()>max_body_bytes{return Err("body too large".into())}}body.truncate(content_length);Ok(HttpRequest{method,path,query,body})}
fn write_response(stream:&mut TcpStream,response:HttpResponse)->std::io::Result<()>{let disposition=response.disposition.map(|v|format!("Content-Disposition: {v}\r\n")).unwrap_or_default();let header=format!("HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\n{}Access-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, PUT, DELETE, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nX-NetCore-Security-Mode: open-lab\r\nConnection: close\r\n\r\n",response.status,reason_phrase(response.status),response.content_type,response.body.len(),disposition);stream.write_all(header.as_bytes())?;stream.write_all(&response.body)?;stream.flush()}
fn find_subslice(h:&[u8],n:&[u8])->Option<usize>{h.windows(n.len()).position(|w|w==n)}
fn parse_path_and_query(raw:&str)->(String,HashMap<String,String>){let mut parts=raw.splitn(2,'?');let path=parts.next().unwrap_or(raw).to_string();let query=parts.next().map(|q|q.split('&').filter(|p|!p.is_empty()).filter_map(|p|{let mut v=p.splitn(2,'=');Some((v.next()?.to_string(),v.next().unwrap_or("").to_string()))}).collect()).unwrap_or_default();(path,query)}
fn reason_phrase(status:u16)->&'static str{match status{200=>"OK",201=>"Created",202=>"Accepted",204=>"No Content",400=>"Bad Request",404=>"Not Found",405=>"Method Not Allowed",409=>"Conflict",413=>"Payload Too Large",500=>"Internal Server Error",503=>"Service Unavailable",_=>"Response"}}

const INDEX_HTML:&str=r#"<!doctype html><html lang="de"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>NetCore Subscriber Core</title><style>
:root{color-scheme:dark;--bg:#0b1118;--card:#121b25;--line:#263545;--text:#ecf3fa;--muted:#96a9bb;--accent:#57b6ff;--bad:#ff646e;--ok:#49d18b;--warn:#ffbf57}*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--text);font:14px system-ui,sans-serif}.lab{background:#8d2029;padding:10px 18px;font-weight:700;text-align:center}header{padding:18px 24px;border-bottom:1px solid var(--line);display:flex;gap:16px;align-items:center;justify-content:space-between}h1,h2{margin:0}.wrap{padding:20px;display:grid;gap:18px}.cards{display:grid;grid-template-columns:repeat(auto-fit,minmax(180px,1fr));gap:12px}.card,.panel{background:var(--card);border:1px solid var(--line);border-radius:12px;padding:16px}.value{font-size:28px;font-weight:800;margin-top:8px}.muted{color:var(--muted)}button,.btn,input,textarea,select{background:#172432;color:var(--text);border:1px solid #365069;border-radius:7px;padding:8px 10px}button,.btn{cursor:pointer}button.primary{background:#1668a8}button.danger{background:#8d2029}.toolbar{display:flex;gap:8px;flex-wrap:wrap;margin:12px 0}table{width:100%;border-collapse:collapse}th,td{text-align:left;padding:9px;border-bottom:1px solid var(--line);vertical-align:top}th{color:#bcd0df}dialog{background:var(--card);color:var(--text);border:1px solid var(--line);border-radius:12px;max-width:760px;width:95%}form.grid{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:12px}label{display:grid;gap:5px}.wide{grid-column:1/-1}.tag{display:inline-block;border-radius:99px;padding:3px 8px;background:#233548}.ok{color:var(--ok)}.bad{color:var(--bad)}.warn{color:var(--warn)}@media(max-width:720px){form.grid{grid-template-columns:1fr}.wide{grid-column:auto}.tablewrap{overflow:auto}}
</style></head><body><div class="lab">⚠ OPEN LAB – keine Anmeldung, keine Tokens, kein TLS. Jeder erreichbare Client darf Teilnehmer und Zugangsregeln ändern.</div><header><div><h1>Subscriber Core</h1><div class="muted">Zentrale Teilnehmerdatenbank und TBS-Zugangsrichtlinie</div></div><div id="gateway">Gateway …</div></header><main class="wrap"><section class="cards" id="cards"></section><section class="panel"><h2>Teilnehmer</h2><div class="toolbar"><button class="primary" onclick="openNew()">Teilnehmer anlegen</button><button onclick="syncNow()">Richtlinie an alle TBS senden</button><a class="btn" href="/api/v1/export.json">JSON exportieren</a><a class="btn" href="/api/v1/export.csv">CSV exportieren</a><button onclick="document.getElementById('importFile').click()">JSON importieren</button><input id="importFile" type="file" accept="application/json" hidden onchange="importJson(this)"><input id="filter" placeholder="ISSI, Name, Organisation …" oninput="renderSubscribers()"></div><div class="tablewrap"><table><thead><tr><th>ISSI</th><th>Name / Organisation</th><th>Status</th><th>Profil</th><th>Gruppen</th><th>Aktionen</th></tr></thead><tbody id="subscriberRows"></tbody></table></div></section><section class="panel"><h2>TBS-Synchronisation</h2><div class="tablewrap"><table><thead><tr><th>Node</th><th>Verbindung</th><th>Revision</th><th>Status</th><th>Nachricht</th></tr></thead><tbody id="syncRows"></tbody></table></div></section><section class="panel"><h2>Beobachtete Funkgeräte</h2><div class="tablewrap"><table><thead><tr><th>ISSI</th><th>TBS</th><th>registriert</th><th>Profil</th><th>Gruppen</th><th>RSSI</th></tr></thead><tbody id="observedRows"></tbody></table></div></section><section class="panel"><h2>Ereignisse</h2><pre id="events" class="muted"></pre></section></main>
<dialog id="editor"><form id="form" class="grid" onsubmit="save(event)"><h2 class="wide" id="editorTitle">Teilnehmer</h2><label>ISSI<input name="issi" type="number" min="1" max="16777215" required></label><label>Anzeigename<input name="display_name"></label><label>Home MCC<input name="home_mcc" type="number" min="0" max="1023"></label><label>Home MNC<input name="home_mnc" type="number" min="0" max="16383"></label><label>Organisation<input name="organization"></label><label>Gerät / Rufname<input name="device_label"></label><label>TEI<input name="device_tei" type="number" min="0"></label><label>Rufpriorität<input name="call_priority" type="number" min="0" max="15" value="0"></label><label><span>Aktiv</span><input name="enabled" type="checkbox" checked></label><label><span>Registrierung erlaubt</span><input name="registration_allowed" type="checkbox" checked></label><label><span>Notruf erlaubt</span><input name="emergency_allowed" type="checkbox"></label><label><span>SDS erlaubt</span><input name="sds_allowed" type="checkbox" checked></label><label><span>Paketdaten erlaubt</span><input name="packet_data_allowed" type="checkbox"></label><label class="wide">Standardgruppen, kommasepariert<input name="default_groups"></label><label class="wide">Notizen<textarea name="notes" rows="4"></textarea></label><div class="wide toolbar"><button class="primary" type="submit">Speichern</button><button type="button" onclick="editor.close()">Abbrechen</button></div></form></dialog>
<script>let profiles=[],nodes=[],syncs=[],observed=[];const editor=document.getElementById('editor'),form=document.getElementById('form');async function api(path,opt){const r=await fetch(path,opt);if(!r.ok&&r.status!==204){let e={};try{e=await r.json()}catch{}throw new Error(e.error||r.statusText)}return r.status===204?null:r.json()}async function refresh(){try{const [s,p,n,y,o,e]=await Promise.all([api('/api/v1/status'),api('/api/v1/subscribers'),api('/api/v1/nodes'),api('/api/v1/syncs'),api('/api/v1/observed'),api('/api/v1/events?limit=30')]);profiles=p;nodes=n;syncs=y;observed=o;document.getElementById('gateway').innerHTML=s.node_gateway_connected?'<span class="ok">● Gateway verbunden</span>':'<span class="bad">● Gateway getrennt</span>';document.getElementById('cards').innerHTML=[['Profile',s.subscribers_total],['freigegeben',s.subscribers_authorized],['gesperrt',s.subscribers_blocked],['registriert',s.observed_registered],['TBS verbunden',s.nodes_connected],['TBS synchron',s.nodes_synced],['Revision',s.database_revision],['Policy',s.access_mode]].map(x=>`<div class="card"><div class="muted">${x[0]}</div><div class="value">${x[1]}</div></div>`).join('');renderSubscribers();renderSyncs();renderObserved();document.getElementById('events').textContent=e.map(x=>`${x.timestamp}  ${x.kind} ${x.issi||''} ${x.node_id||''}`).join('\n')}catch(e){document.getElementById('gateway').innerHTML='<span class="bad">UI-Fehler: '+esc(e.message)+'</span>'}}function esc(v){return String(v??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]))}function renderSubscribers(){const q=document.getElementById('filter').value.toLowerCase();document.getElementById('subscriberRows').innerHTML=profiles.filter(p=>`${p.issi} ${p.display_name} ${p.organization}`.toLowerCase().includes(q)).map(p=>`<tr><td><b>${p.issi}</b><br><span class="muted">${p.home_mcc}/${p.home_mnc}</span></td><td>${esc(p.display_name)||'<span class="muted">ohne Namen</span>'}<br><span class="muted">${esc(p.organization)}</span></td><td>${p.enabled&&p.registration_allowed?'<span class="ok">freigegeben</span>':'<span class="bad">gesperrt</span>'}</td><td>Prio ${p.call_priority}<br>${p.sds_allowed?'SDS ':''}${p.packet_data_allowed?'IP ':''}${p.emergency_allowed?'Notruf':''}</td><td>${[...p.default_groups].join(', ')}</td><td><button onclick="edit(${p.issi})">Bearbeiten</button> <button class="danger" onclick="del(${p.issi})">Löschen</button></td></tr>`).join('')}function renderSyncs(){const map=new Map(nodes.map(n=>[n.node_id,n]));document.getElementById('syncRows').innerHTML=[...new Set([...nodes.map(n=>n.node_id),...syncs.map(s=>s.node_id)])].map(id=>{const n=map.get(id),s=syncs.find(x=>x.node_id===id);return `<tr><td>${esc(id)}<br><span class="muted">${esc(n?.station_name||'')}</span></td><td>${n?.connected&&!n?.stale?'<span class="ok">verbunden</span>':'<span class="bad">offline</span>'}</td><td>${s?.applied_revision??'–'} / ${s?.desired_revision??'–'}</td><td>${esc(s?.phase||'noch nie synchronisiert')}</td><td>${esc(s?.message||'')}</td></tr>`}).join('')}function renderObserved(){document.getElementById('observedRows').innerHTML=observed.map(o=>`<tr><td>${o.issi}</td><td>${esc(o.serving_node||'–')}</td><td>${o.registered?'<span class="ok">ja</span>':'nein'}</td><td>${o.known_profile?(o.authorized?'<span class="ok">bekannt/frei</span>':'<span class="bad">bekannt/gesperrt</span>'):'<span class="warn">unbekannt</span>'}</td><td>${[...o.groups].join(', ')}</td><td>${o.last_rssi_dbfs==null?'–':o.last_rssi_dbfs.toFixed(1)+' dBFS'}</td></tr>`).join('')}function openNew(){form.reset();form.dataset.issi='';form.enabled.checked=true;form.registration_allowed.checked=true;form.sds_allowed.checked=true;document.getElementById('editorTitle').textContent='Teilnehmer anlegen';editor.showModal()}function edit(issi){const p=profiles.find(x=>x.issi===issi);if(!p)return;form.dataset.issi=issi;for(const [k,v] of Object.entries(p)){const el=form.elements[k];if(!el)continue;if(el.type==='checkbox')el.checked=!!v;else if(k==='default_groups')el.value=[...v].join(', ');else el.value=v??''}form.elements.issi.disabled=true;document.getElementById('editorTitle').textContent='Teilnehmer '+issi;editor.showModal()}editor.addEventListener('close',()=>form.elements.issi.disabled=false);function payload(){const f=new FormData(form);const n=x=>Number(f.get(x)||0),b=x=>form.elements[x].checked;return{issi:n('issi')||Number(form.dataset.issi),home_mcc:n('home_mcc'),home_mnc:n('home_mnc'),display_name:f.get('display_name')||'',organization:f.get('organization')||'',device_label:f.get('device_label')||'',device_tei:f.get('device_tei')?n('device_tei'):null,enabled:b('enabled'),registration_allowed:b('registration_allowed'),call_priority:n('call_priority'),emergency_allowed:b('emergency_allowed'),sds_allowed:b('sds_allowed'),packet_data_allowed:b('packet_data_allowed'),default_groups:(f.get('default_groups')||'').split(',').map(x=>Number(x.trim())).filter(Boolean),notes:f.get('notes')||''}}async function save(e){e.preventDefault();try{const p=payload(),existing=form.dataset.issi;await api(existing?'/api/v1/subscribers/'+existing:'/api/v1/subscribers',{method:existing?'PUT':'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(p)});editor.close();await refresh()}catch(e){alert(e.message)}}async function del(issi){if(!confirm('ISSI '+issi+' wirklich löschen? Die TBS-Richtlinie wird sofort aktualisiert.'))return;try{await api('/api/v1/subscribers/'+issi,{method:'DELETE'});refresh()}catch(e){alert(e.message)}}async function syncNow(){try{const r=await api('/api/v1/sync',{method:'POST'});alert(r.queued+' TBS-Synchronisation(en) eingeplant');refresh()}catch(e){alert(e.message)}}async function importJson(input){const file=input.files[0];if(!file)return;try{let data=JSON.parse(await file.text());if(Array.isArray(data))data={replace:false,subscribers:data};else if(data.schema_version&&data.subscribers)data={replace:confirm('Bestehende Datenbank durch Import ersetzen?'),subscribers:data.subscribers};await api('/api/v1/import',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(data)});refresh()}catch(e){alert(e.message)}finally{input.value=''}}refresh();setInterval(refresh,5000)</script></body></html>"#;

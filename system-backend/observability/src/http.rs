use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};

use crate::collector;
use crate::config::ObservabilityConfig;
use crate::protocol::{
    ActionInput, DiagnosticInput, LogIngestInput, MaintenanceInput, RuleInput, SilenceInput,
    TargetCreateInput, TraceIngestInput,
};
use crate::state::SharedObservability;

pub fn spawn_http_server(config: ObservabilityConfig, observability: SharedObservability) -> std::io::Result<thread::JoinHandle<()>> {
    let listener = TcpListener::bind(config.server.bind)?;
    tracing::info!("Observability WebUI/API listening on http://{}", config.server.bind);
    Ok(thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let config = config.clone(); let observability = observability.clone();
                    thread::spawn(move || {
                        if let Err(error) = handle_connection(stream, config, observability) { tracing::warn!("Observability HTTP connection failed: {}", error); }
                    });
                }
                Err(error) => tracing::warn!("Observability HTTP accept failed: {}", error),
            }
        }
    }))
}

struct HttpRequest { method: String, path: String, query: HashMap<String,String>, body: Vec<u8> }
struct HttpResponse { status: u16, content_type: &'static str, body: Vec<u8>, disposition: Option<String>, extra_headers: Vec<(String,String)> }

fn handle_connection(mut stream: TcpStream, config: ObservabilityConfig, observability: SharedObservability) -> Result<(),String> {
    let request = read_request(&mut stream, config.server.max_body_bytes)?; let response = route(request, config, observability);
    write_response(&mut stream, response).map_err(|error| error.to_string())
}

fn route(request: HttpRequest, config: ObservabilityConfig, observability: SharedObservability) -> HttpResponse {
    if request.method == "OPTIONS" { return empty(204); }
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") => html(INDEX_HTML),
        ("GET", "/health/live") => json_response(200, &json!({"status":"live"})),
        ("GET", "/health/ready") => { let status = observability.status(); json_response(if status.ready {200} else {503}, &status) }
        ("GET", "/api/v1/status") => json_response(200, &observability.status()),
        ("GET", "/api/v1/config") => json_response(200, &config),
        ("GET", "/api/v1/targets") => json_response(200, &observability.targets()),
        ("POST", "/api/v1/targets") => match parse_json::<TargetCreateInput>(&request.body).and_then(|input| observability.create_target(input)) { Ok(value) => json_response(201,&value), Err(error) => conflict(error) },
        ("GET", "/api/v1/stack") => json_response(200, &observability.stack()),
        ("GET", "/api/v1/metrics/catalog") => json_response(200, &observability.metric_catalog()),
        ("GET", "/api/v1/metrics/series") => {
            let limit = query_usize(&request,"limit",200,5000);
            json_response(200,&observability.series(request.query.get("metric").map(String::as_str),request.query.get("target_id").map(String::as_str),request.query.get("service").map(String::as_str),limit))
        }
        ("GET", "/api/v1/rules") => json_response(200,&observability.rules()),
        ("POST", "/api/v1/rules") => match parse_json::<RuleInput>(&request.body).and_then(|input| observability.create_rule(input)) { Ok(value)=>json_response(201,&value),Err(error)=>conflict(error) },
        ("GET", "/api/v1/alerts") => {
            let limit=query_usize(&request,"limit",500,5000);
            json_response(200,&observability.alerts(request.query.get("state").map(String::as_str),request.query.get("severity").map(String::as_str),request.query.get("service").map(String::as_str),limit))
        }
        ("GET", "/api/v1/silences") => json_response(200,&observability.silences()),
        ("POST", "/api/v1/silences") => match parse_json::<SilenceInput>(&request.body).and_then(|input| observability.create_silence(input)) { Ok(value)=>json_response(201,&value),Err(error)=>conflict(error) },
        ("GET", "/api/v1/logs") => {
            let limit=query_usize(&request,"limit",500,10000);
            json_response(200,&observability.logs(request.query.get("service").map(String::as_str),request.query.get("level").map(String::as_str),request.query.get("contains").map(String::as_str),request.query.get("trace_id").map(String::as_str),limit))
        }
        ("POST", "/api/v1/logs/ingest") => match parse_json::<LogIngestInput>(&request.body).and_then(|input| observability.ingest_logs(input)) { Ok(count)=>json_response(202,&json!({"accepted":count})),Err(error)=>conflict(error) },
        ("GET", "/api/v1/traces") => {
            let limit=query_usize(&request,"limit",500,10000);
            json_response(200,&observability.traces(request.query.get("service").map(String::as_str),request.query.get("trace_id").map(String::as_str),request.query.get("status").map(String::as_str),limit))
        }
        ("POST", "/api/v1/traces/ingest") => match parse_json::<TraceIngestInput>(&request.body).and_then(|input| observability.ingest_traces(input)) { Ok(count)=>json_response(202,&json!({"accepted":count})),Err(error)=>conflict(error) },
        ("GET", "/api/v1/audit") => json_response(200,&observability.audit(query_usize(&request,"limit",500,5000))),
        ("GET", "/api/v1/diagnostics") => json_response(200,&observability.diagnostics()),
        ("POST", "/api/v1/diagnostics") => match parse_json_or_default::<DiagnosticInput>(&request.body).and_then(|input| observability.create_diagnostic(input)) { Ok(value)=>json_response(201,&value),Err(error)=>json_response(500,&json!({"error":error})) },
        ("POST", "/api/v1/maintenance/tick") => {
            let input=parse_json_or_default::<MaintenanceInput>(&request.body);
            match input.and_then(|input| observability.maintenance(input.actor).map_err(|error|error.to_string())) { Ok(value)=>json_response(200,&value),Err(error)=>json_response(500,&json!({"error":error})) }
        }
        ("POST", "/api/v1/maintenance/scrape-now") => { collector::run_cycle(&config,&observability); json_response(200,&observability.status()) }
        ("POST", "/api/v1/maintenance/backup") => {
            let input=parse_json_or_default::<MaintenanceInput>(&request.body);
            match input.and_then(|input| observability.backup(input.actor)) { Ok(value)=>json_response(201,&value),Err(error)=>json_response(500,&json!({"error":error})) }
        }
        ("GET", "/api/v1/export.json") => download("netcore-observability-export.json","application/json",serde_json::to_vec_pretty(&observability.export()).unwrap_or_default()),
        ("GET", "/metrics") => text("text/plain; version=0.0.4; charset=utf-8",observability.metrics()),
        ("GET", "/openapi.json") => json_response(200,&openapi()),
        _ => dynamic_route(request,config,observability),
    }
}

fn dynamic_route(request: HttpRequest, config: ObservabilityConfig, observability: SharedObservability) -> HttpResponse {
    let parts: Vec<&str> = request.path.trim_matches('/').split('/').collect();
    match (request.method.as_str(),parts.as_slice()) {
        ("POST",["api","v1","targets",target_id,"test"]) => {
            let Some(target)=observability.target(target_id) else { return not_found(format!("target {target_id} not found")); };
            let result=collector::scrape_target(&config,&target); let snapshot=result.clone();
            match observability.record_scrape(result) { Ok(())=>json_response(200,&json!({"target":observability.target(target_id),"live":snapshot.live,"ready":snapshot.ready,"metrics_ok":snapshot.metrics_ok,"response_ms":snapshot.response_ms,"samples":snapshot.metrics.len(),"error":snapshot.error})),Err(error)=>json_response(500,&json!({"error":error.to_string()})) }
        }
        ("POST",["api","v1","targets",target_id,action]) => match parse_json_or_default::<ActionInput>(&request.body).and_then(|input|observability.target_action(target_id,action,input)) { Ok(value)=>json_response(200,&value),Err(error)=>conflict(error) },
        ("DELETE",["api","v1","targets",target_id]) => match parse_json_or_default::<ActionInput>(&request.body).and_then(|input|observability.delete_target(target_id,input)) { Ok(())=>empty(204),Err(error)=>not_found(error) },
        ("PUT",["api","v1","rules",rule_id]) => match parse_json::<RuleInput>(&request.body).and_then(|input|observability.update_rule(rule_id,input)) { Ok(value)=>json_response(200,&value),Err(error)=>conflict(error) },
        ("POST",["api","v1","rules",rule_id,action]) => match parse_json_or_default::<ActionInput>(&request.body).and_then(|input|observability.rule_action(rule_id,action,input)) { Ok(value)=>json_response(200,&value),Err(error)=>conflict(error) },
        ("DELETE",["api","v1","rules",rule_id]) => match parse_json_or_default::<ActionInput>(&request.body).and_then(|input|observability.delete_rule(rule_id,input)) { Ok(())=>empty(204),Err(error)=>not_found(error) },
        ("POST",["api","v1","alerts",alert_id,action]) => match parse_json_or_default::<ActionInput>(&request.body).and_then(|input|observability.alert_action(alert_id,action,input)) { Ok(value)=>json_response(200,&value),Err(error)=>conflict(error) },
        ("POST",["api","v1","silences",silence_id,"expire"]) => match parse_json_or_default::<ActionInput>(&request.body).and_then(|input|observability.expire_silence(silence_id,input)) { Ok(value)=>json_response(200,&value),Err(error)=>conflict(error) },
        ("GET",["api","v1","diagnostics",diagnostic_id,"download"]) => match observability.diagnostic_file(diagnostic_id) { Ok((name,bytes))=>download(&name,"application/gzip",bytes),Err(error)=>not_found(error) },
        _ => not_found("route not found".to_string()),
    }
}

fn openapi() -> Value { json!({
    "openapi":"3.0.3","info":{"title":"NetCore Observability API","version":"1.0.0","description":"OPEN LAB: no authentication, no tokens and no TLS"},
    "servers":[{"url":"/"}],"paths":{
        "/api/v1/status":{"get":{"summary":"NMS status"}},
        "/api/v1/targets":{"get":{"summary":"List scrape targets"},"post":{"summary":"Create target"}},
        "/api/v1/targets/{target_id}/test":{"post":{"summary":"Test and ingest one target"}},
        "/api/v1/metrics/catalog":{"get":{"summary":"Metric catalog"}},
        "/api/v1/metrics/series":{"get":{"summary":"Query bounded time series"}},
        "/api/v1/logs":{"get":{"summary":"Search logs"}},
        "/api/v1/logs/ingest":{"post":{"summary":"Ingest NetCore JSON logs"}},
        "/api/v1/traces":{"get":{"summary":"Search trace spans"}},
        "/api/v1/traces/ingest":{"post":{"summary":"Ingest NetCore JSON spans"}},
        "/api/v1/rules":{"get":{"summary":"List alert rules"},"post":{"summary":"Create alert rule"}},
        "/api/v1/alerts":{"get":{"summary":"List alert instances"}},
        "/api/v1/silences":{"get":{"summary":"List silences"},"post":{"summary":"Create silence"}},
        "/api/v1/diagnostics":{"get":{"summary":"List diagnostic bundles"},"post":{"summary":"Build diagnostic bundle"}},
        "/api/v1/maintenance/scrape-now":{"post":{"summary":"Run collection immediately"}},
        "/metrics":{"get":{"summary":"Prometheus metrics for the NMS"}}
    }
}) }

fn read_request(stream:&mut TcpStream,max_body_bytes:usize)->Result<HttpRequest,String>{
    stream.set_read_timeout(Some(std::time::Duration::from_secs(10))).map_err(|error|error.to_string())?;
    let mut raw=Vec::new(); let mut buffer=[0u8;8192]; let header_end;
    loop { let count=stream.read(&mut buffer).map_err(|error|error.to_string())?; if count==0{return Err("connection closed before headers".to_string());} raw.extend_from_slice(&buffer[..count]); if let Some(position)=raw.windows(4).position(|window|window==b"\r\n\r\n"){header_end=position;break;} if raw.len()>64*1024{return Err("request headers too large".to_string());} }
    let headers=String::from_utf8(raw[..header_end].to_vec()).map_err(|_|"headers are not UTF-8".to_string())?; let mut lines=headers.lines(); let request_line=lines.next().ok_or_else(||"missing request line".to_string())?; let mut parts=request_line.split_whitespace(); let method=parts.next().unwrap_or("").to_string(); let target=parts.next().unwrap_or("/");
    let content_length=lines.filter_map(|line|line.split_once(':')).find(|(key,_)|key.eq_ignore_ascii_case("content-length")).and_then(|(_,value)|value.trim().parse::<usize>().ok()).unwrap_or(0); if content_length>max_body_bytes{return Err(format!("request body exceeds {max_body_bytes} bytes"));}
    let body_start=header_end+4; while raw.len()<body_start+content_length { let count=stream.read(&mut buffer).map_err(|error|error.to_string())?; if count==0{break;} raw.extend_from_slice(&buffer[..count]); if raw.len()>body_start+max_body_bytes{return Err("request body too large".to_string());} }
    if raw.len()<body_start+content_length{return Err("truncated request body".to_string());}
    let (path,query)=parse_target(target); Ok(HttpRequest{method,path,query,body:raw[body_start..body_start+content_length].to_vec()})
}

fn parse_target(target:&str)->(String,HashMap<String,String>){let (path,query)=target.split_once('?').unwrap_or((target,""));let mut values=HashMap::new();for part in query.split('&').filter(|part|!part.is_empty()){let (key,value)=part.split_once('=').unwrap_or((part,""));values.insert(percent_decode(key),percent_decode(value));}(percent_decode(path),values)}
fn percent_decode(value:&str)->String{let bytes=value.as_bytes();let mut output=Vec::new();let mut index=0;while index<bytes.len(){match bytes[index]{b'%' if index+2<bytes.len()=>{if let Ok(text)=std::str::from_utf8(&bytes[index+1..index+3]){if let Ok(decoded)=u8::from_str_radix(text,16){output.push(decoded);index+=3;continue;}}output.push(bytes[index]);index+=1},b'+'=>{output.push(b' ');index+=1},value=>{output.push(value);index+=1}}}String::from_utf8_lossy(&output).to_string()}
fn write_response(stream:&mut TcpStream,response:HttpResponse)->std::io::Result<()>{let reason=match response.status{200=>"OK",201=>"Created",202=>"Accepted",204=>"No Content",400=>"Bad Request",404=>"Not Found",409=>"Conflict",413=>"Payload Too Large",500=>"Internal Server Error",503=>"Service Unavailable",_=>"OK"};let mut headers=format!("HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: Content-Type\r\nAccess-Control-Allow-Methods: GET,POST,PUT,DELETE,OPTIONS\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n",response.status,reason,response.content_type,response.body.len());if let Some(disposition)=response.disposition{headers.push_str(&format!("Content-Disposition: {disposition}\r\n"));}for(key,value)in response.extra_headers{headers.push_str(&format!("{key}: {value}\r\n"));}headers.push_str("\r\n");stream.write_all(headers.as_bytes())?;stream.write_all(&response.body)}
fn parse_json<T:DeserializeOwned>(body:&[u8])->Result<T,String>{serde_json::from_slice(body).map_err(|error|format!("invalid JSON: {error}"))}
fn parse_json_or_default<T:DeserializeOwned+Default>(body:&[u8])->Result<T,String>{if body.is_empty(){Ok(T::default())}else{parse_json(body)}}
fn query_usize(request:&HttpRequest,key:&str,default:usize,max:usize)->usize{request.query.get(key).and_then(|value|value.parse::<usize>().ok()).unwrap_or(default).clamp(1,max)}
fn json_response<T:Serialize>(status:u16,value:&T)->HttpResponse{HttpResponse{status,content_type:"application/json; charset=utf-8",body:serde_json::to_vec_pretty(value).unwrap_or_else(|_|b"{}".to_vec()),disposition:None,extra_headers:Vec::new()}}
fn html(value:&'static str)->HttpResponse{HttpResponse{status:200,content_type:"text/html; charset=utf-8",body:value.as_bytes().to_vec(),disposition:None,extra_headers:vec![("Content-Security-Policy".to_string(),"default-src 'self'; style-src 'self' 'unsafe-inline'; script-src 'self' 'unsafe-inline'; frame-src http:; connect-src 'self' http:".to_string())]}}
fn text(content_type:&'static str,value:String)->HttpResponse{HttpResponse{status:200,content_type,body:value.into_bytes(),disposition:None,extra_headers:Vec::new()}}
fn empty(status:u16)->HttpResponse{HttpResponse{status,content_type:"text/plain; charset=utf-8",body:Vec::new(),disposition:None,extra_headers:Vec::new()}}
fn conflict(error:String)->HttpResponse{json_response(409,&json!({"error":error}))}
fn not_found(error:String)->HttpResponse{json_response(404,&json!({"error":error}))}
fn download(name:&str,content_type:&'static str,body:Vec<u8>)->HttpResponse{HttpResponse{status:200,content_type,body,disposition:Some(format!("attachment; filename=\"{}\"",name.replace('"',""))),extra_headers:Vec::new()}}

const INDEX_HTML: &str = r##"<!doctype html>
<html lang="de"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>NetCore Observability</title>
<style>
:root{color-scheme:dark;--bg:#071018;--panel:#0d1b27;--line:#22384a;--muted:#91a6b8;--text:#e8f2f8;--accent:#32c6ff;--ok:#57d38c;--warn:#ffc857;--bad:#ff6b6b}*{box-sizing:border-box}body{margin:0;background:linear-gradient(135deg,#071018,#0b1622 55%,#08131c);color:var(--text);font-family:Inter,system-ui,sans-serif}header{position:sticky;top:0;z-index:2;background:#08131cf2;border-bottom:1px solid var(--line);padding:14px 22px;display:flex;gap:18px;align-items:center}.brand{font-weight:800;letter-spacing:.06em}.lab{background:#6b3810;color:#ffd8a1;border:1px solid #aa641d;padding:6px 10px;border-radius:999px;font-size:12px}nav{display:flex;gap:7px;flex-wrap:wrap}nav button{background:transparent;border:1px solid transparent;color:var(--muted);padding:8px 10px;border-radius:8px;cursor:pointer}nav button.active,nav button:hover{color:var(--text);border-color:var(--line);background:#122434}.right{margin-left:auto;color:var(--muted);font-size:13px}main{padding:20px;max-width:1680px;margin:auto}.page{display:none}.page.active{display:block}.grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(230px,1fr));gap:14px}.card{background:#0c1924e8;border:1px solid var(--line);border-radius:12px;padding:16px;box-shadow:0 12px 35px #0004;margin-bottom:14px}.kpi{font-size:30px;font-weight:800;margin-top:5px}.muted{color:var(--muted)}.ok{color:var(--ok)}.warn{color:var(--warn)}.bad{color:var(--bad)}table{width:100%;border-collapse:collapse;font-size:13px}th,td{text-align:left;padding:9px;border-bottom:1px solid #1a3040;vertical-align:top}th{color:var(--muted);position:sticky;top:0;background:#0c1924}button,.button{border:1px solid #2c4a5e;background:#112638;color:var(--text);padding:7px 10px;border-radius:8px;cursor:pointer;text-decoration:none}button.primary{background:#0c5270;border-color:#167da3}button.danger{background:#5b2020;border-color:#8e3636}button:disabled{opacity:.45;cursor:not-allowed}input,select,textarea{width:100%;background:#08131c;color:var(--text);border:1px solid #2a465a;border-radius:8px;padding:9px}label{display:block;font-size:12px;color:var(--muted);margin-bottom:4px}.form{display:grid;grid-template-columns:repeat(auto-fit,minmax(180px,1fr));gap:10px;align-items:end}.toolbar{display:flex;gap:8px;align-items:center;flex-wrap:wrap;margin-bottom:12px}.toolbar h2{margin-right:auto}.pill{display:inline-block;padding:3px 7px;border-radius:999px;background:#183044;font-size:11px}.pill.ok{background:#123e29}.pill.bad{background:#4a2020}.pill.warn{background:#4a3a16}pre{white-space:pre-wrap;word-break:break-word;background:#071018;border:1px solid var(--line);border-radius:8px;padding:12px;max-height:520px;overflow:auto}.links{display:flex;gap:10px;flex-wrap:wrap}.links a{color:var(--accent)}.split{display:grid;grid-template-columns:1fr 1fr;gap:14px}@media(max-width:900px){.split{grid-template-columns:1fr}.right{display:none}header{align-items:flex-start;flex-direction:column}nav{width:100%}}
</style></head><body>
<header><div class="brand">NETCORE · OBSERVABILITY</div><div class="lab">OPEN LAB · OHNE LOGIN/TOKEN/TLS</div><nav id="nav"></nav><div class="right" id="clock"></div></header>
<main>
<section class="page active" id="overview"><div class="grid" id="kpis"></div><div class="split"><div class="card"><h2>Dienste</h2><div style="max-height:480px;overflow:auto"><table><thead><tr><th>Dienst</th><th>Live</th><th>Ready</th><th>Antwort</th><th>Fehler</th></tr></thead><tbody id="overviewTargets"></tbody></table></div></div><div class="card"><h2>Aktive Alarme</h2><div id="overviewAlerts"></div></div></div><div class="card"><h2>Observability-Stack</h2><div class="grid" id="stackCards"></div></div></section>
<section class="page" id="targets"><div class="card"><div class="toolbar"><h2>Scrape Targets</h2><button class="primary" onclick="scrapeNow()">Alle jetzt scrapen</button></div><div style="overflow:auto"><table><thead><tr><th>Target</th><th>URL</th><th>Zustand</th><th>Serien</th><th>Letzter Lauf</th><th>Aktionen</th></tr></thead><tbody id="targetsTable"></tbody></table></div></div><div class="card"><h2>Target hinzufügen</h2><div class="form"><div><label>ID</label><input id="tId"></div><div><label>Name</label><input id="tName"></div><div><label>Service</label><input id="tService"></div><div><label>Base URL</label><input id="tUrl" value="http://127.0.0.1:8080"></div><button class="primary" onclick="addTarget()">Anlegen</button></div></div></section>
<section class="page" id="metricsPage"><div class="card"><div class="toolbar"><h2>Metriken</h2><select id="metricSelect" onchange="loadSeries()"><option value="">alle</option></select><select id="metricTarget" onchange="loadSeries()"><option value="">alle Targets</option></select><button onclick="loadSeries()">Aktualisieren</button></div><div style="overflow:auto"><table><thead><tr><th>Metrik</th><th>Target</th><th>Labels</th><th>Wert</th><th>Zeit</th><th>Samples</th></tr></thead><tbody id="seriesTable"></tbody></table></div></div></section>
<section class="page" id="alerts"><div class="split"><div class="card"><h2>Alarmzustände</h2><div id="alertsList"></div></div><div class="card"><h2>Stummschaltungen</h2><div id="silencesList"></div><hr><div class="form"><div><label>Kommentar</label><input id="sComment" value="Wartungsfenster"></div><div><label>Dauer Sekunden</label><input id="sDuration" type="number" value="3600"></div><div><label>Service (optional)</label><input id="sService"></div><div><label>Severity (optional)</label><input id="sSeverity"></div><button class="primary" onclick="addSilence()">Stummschalten</button></div></div></div><div class="card"><h2>Alarmregeln</h2><div style="overflow:auto"><table><thead><tr><th>Regel</th><th>Bedingung</th><th>For</th><th>Severity</th><th>Status</th><th>Aktion</th></tr></thead><tbody id="rulesTable"></tbody></table></div></div></section>
<section class="page" id="logs"><div class="card"><div class="toolbar"><h2>Logs</h2><input id="logService" placeholder="Service" style="max-width:180px"><select id="logLevel" style="max-width:160px"><option value="">alle Level</option><option>error</option><option>warn</option><option>info</option><option>debug</option></select><input id="logContains" placeholder="Textsuche" style="max-width:260px"><button onclick="loadLogs()">Suchen</button></div><div style="overflow:auto;max-height:650px"><table><thead><tr><th>Zeit</th><th>Service</th><th>Level</th><th>Meldung</th><th>Trace</th></tr></thead><tbody id="logsTable"></tbody></table></div></div></section>
<section class="page" id="traces"><div class="card"><div class="toolbar"><h2>Traces</h2><input id="traceService" placeholder="Service" style="max-width:180px"><input id="traceId" placeholder="Trace ID" style="max-width:280px"><button onclick="loadTraces()">Suchen</button></div><div style="overflow:auto"><table><thead><tr><th>Start</th><th>Trace / Span</th><th>Service</th><th>Operation</th><th>Dauer</th><th>Status</th></tr></thead><tbody id="tracesTable"></tbody></table></div></div></section>
<section class="page" id="audit"><div class="card"><h2>Audit und Ereignisse</h2><div style="overflow:auto;max-height:700px"><table><thead><tr><th>#</th><th>Zeit</th><th>Akteur</th><th>Aktion</th><th>Objekt</th><th>Ergebnis</th><th>Details</th></tr></thead><tbody id="auditTable"></tbody></table></div></div></section>
<section class="page" id="maintenance"><div class="split"><div class="card"><h2>Wartung</h2><div class="toolbar"><button class="primary" onclick="scrapeNow()">Scrape jetzt</button><button onclick="maintenance()">Retention ausführen</button><button onclick="backup()">Backup</button><button onclick="diagnostic()">Diagnosepaket</button></div><pre id="maintenanceOutput">Bereit.</pre></div><div class="card"><h2>Diagnosepakete</h2><div id="diagnosticsList"></div></div></div><div class="card"><h2>Externe Oberflächen</h2><div class="links"><a id="grafanaLink" href="#" target="_blank">Grafana :3000</a><a id="prometheusLink" href="#" target="_blank">Prometheus :9090</a><a id="alertmanagerLink" href="#" target="_blank">Alertmanager :9093</a><a id="lokiLink" href="#" target="_blank">Loki :3100</a></div></div></section>
<section class="page" id="config"><div class="card"><h2>Aktive Konfiguration</h2><pre id="configDump"></pre></div><div class="card"><h2>API und Über</h2><div class="links"><a href="/openapi.json" target="_blank">OpenAPI</a><a href="/metrics" target="_blank">Prometheus-Metriken</a><a href="/api/v1/export.json">JSON-Export</a><a href="/health/ready" target="_blank">Readiness</a></div><p class="muted">NetCore-Tetra Observability Management Plane · Protokoll/API v1 · Open-Lab-Paket. Prometheus, Grafana, Loki und Alertmanager bleiben eigenständige Prozesse; diese WebUI orchestriert Ziele, Regeln, Alarme, Logs, Traces, Retention und Diagnose.</p></div></section>
</main>
<script>
const pages=[['overview','Übersicht'],['targets','Targets'],['metricsPage','Metriken'],['alerts','Alarme'],['logs','Logs'],['traces','Traces'],['audit','Audit'],['maintenance','Wartung'],['config','Konfiguration']];
const esc=s=>String(s??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const fmt=t=>t?new Date(t).toLocaleString():'–'; const pill=(ok,text)=>`<span class="pill ${ok?'ok':'bad'}">${esc(text)}</span>`; const browserUrl=u=>{try{const x=new URL(u);if(x.hostname==='127.0.0.1'||x.hostname==='localhost')x.hostname=location.hostname;return x.toString()}catch{return u}};
async function api(path,options={}){const response=await fetch(path,{headers:{'Content-Type':'application/json'},...options});if(!response.ok){let msg=await response.text();throw new Error(msg)}if(response.status===204)return null;return response.json()}
function show(id){document.querySelectorAll('.page').forEach(x=>x.classList.toggle('active',x.id===id));document.querySelectorAll('nav button').forEach(x=>x.classList.toggle('active',x.dataset.page===id));if(id==='logs')loadLogs();if(id==='traces')loadTraces();if(id==='audit')loadAudit();if(id==='maintenance')loadDiagnostics();}
function initNav(){const nav=document.getElementById('nav');pages.forEach(([id,name],i)=>{const b=document.createElement('button');b.textContent=name;b.dataset.page=id;b.className=i===0?'active':'';b.onclick=()=>show(id);nav.appendChild(b)})}
async function refresh(){try{const [status,targets,stack,alerts,rules,silences,catalog,config]=await Promise.all([api('/api/v1/status'),api('/api/v1/targets'),api('/api/v1/stack'),api('/api/v1/alerts?limit=200'),api('/api/v1/rules'),api('/api/v1/silences'),api('/api/v1/metrics/catalog'),api('/api/v1/config')]);renderStatus(status);renderTargets(targets);renderStack(stack);renderAlerts(alerts);renderRules(rules);renderSilences(silences);renderCatalog(catalog,targets);document.getElementById('configDump').textContent=JSON.stringify(config,null,2);}catch(e){console.error(e)}}
function renderStatus(s){const values=[['Targets up',`${s.targets_up}/${s.targets_total}`,s.targets_up===s.targets_total],['Ready',`${s.targets_ready}/${s.targets_total}`,s.targets_ready===s.targets_total],['Metrikserien',s.series,true],['Logs',s.logs,true],['Traces',s.traces,true],['Alarme',s.alerts_firing,s.alerts_firing===0],['Unquittiert',s.alerts_unacknowledged,s.alerts_unacknowledged===0],['Stack',`${s.stack_ready}/${s.stack_total}`,s.stack_ready===s.stack_total]];document.getElementById('kpis').innerHTML=values.map(([n,v,ok])=>`<div class="card"><div class="muted">${n}</div><div class="kpi ${ok?'ok':'bad'}">${v}</div></div>`).join('')}
function renderTargets(ts){document.getElementById('overviewTargets').innerHTML=ts.map(t=>`<tr><td>${esc(t.display_name)}</td><td>${pill(t.live,t.live?'up':'down')}</td><td>${pill(t.ready,t.ready?'ready':'not ready')}</td><td>${t.response_ms?.toFixed(1)??'–'} ms</td><td>${esc(t.last_error??'')}</td></tr>`).join('');document.getElementById('targetsTable').innerHTML=ts.map(t=>`<tr><td><b>${esc(t.display_name)}</b><br><span class="muted">${esc(t.target_id)} · ${esc(t.service)}</span></td><td>${esc(t.base_url)}</td><td>${pill(t.live,'live')} ${pill(t.ready,'ready')} ${pill(t.metrics_ok,'metrics')}</td><td>${t.series_count}</td><td>${fmt(t.last_scrape_at)}<br><span class="bad">${esc(t.last_error??'')}</span></td><td><button onclick="targetTest('${esc(t.target_id)}')">Test</button> <button onclick="targetAction('${esc(t.target_id)}','${t.enabled?'disable':'enable'}')">${t.enabled?'Deaktivieren':'Aktivieren'}</button></td></tr>`).join('')}
function renderStack(items){document.getElementById('stackCards').innerHTML=items.map(x=>`<div class="card"><b>${esc(x.component)}</b><div class="kpi ${x.ready?'ok':'bad'}">${x.ready?'READY':'DOWN'}</div><div class="muted"><a href="${esc(browserUrl(x.endpoint))}" target="_blank">${esc(x.endpoint)}</a> · ${x.response_ms?.toFixed(1)??'–'} ms<br>${esc(x.last_error??'')}</div></div>`).join('')}
function renderAlerts(items){const active=items.filter(a=>a.state!=='resolved');document.getElementById('overviewAlerts').innerHTML=active.length?active.slice(0,8).map(a=>`<div class="card"><span class="pill ${a.severity==='critical'?'bad':'warn'}">${esc(a.severity)}</span> <b>${esc(a.rule_name)}</b><br>${esc(a.summary)}<br><span class="muted">${esc(a.service)} · ${esc(a.target_id)} · ${a.silenced?'silenced':''}</span></div>`).join(''):'<p class="ok">Keine aktiven Alarme.</p>';document.getElementById('alertsList').innerHTML=items.map(a=>`<div class="card"><div><span class="pill ${a.state==='firing'?'bad':a.state==='pending'?'warn':'ok'}">${esc(a.state)}</span> <span class="pill">${esc(a.severity)}</span> ${a.silenced?'<span class="pill warn">silenced</span>':''}</div><h3>${esc(a.rule_name)}</h3><p>${esc(a.summary)}</p><div class="muted">${esc(a.service)} / ${esc(a.target_id)} · seit ${fmt(a.first_seen_at)}</div><div class="toolbar"><button onclick="alertAction('${a.alert_id}','${a.acknowledged?'unacknowledge':'acknowledge'}')">${a.acknowledged?'Quittierung aufheben':'Quittieren'}</button>${a.state!=='resolved'?`<button onclick="alertAction('${a.alert_id}','resolve')">Manuell lösen</button>`:''}</div></div>`).join('')}
function renderRules(items){document.getElementById('rulesTable').innerHTML=items.map(r=>`<tr><td><b>${esc(r.name)}</b><br><span class="muted">${esc(r.rule_id)}</span></td><td><code>${esc(r.metric)} ${esc(r.comparator)} ${r.threshold}</code></td><td>${r.for_secs}s</td><td>${esc(r.severity)}</td><td>${pill(r.enabled,r.enabled?'enabled':'disabled')}</td><td><button onclick="ruleAction('${r.rule_id}','${r.enabled?'disable':'enable'}')">${r.enabled?'Disable':'Enable'}</button></td></tr>`).join('')}
function renderSilences(items){document.getElementById('silencesList').innerHTML=items.length?items.map(s=>`<div class="card"><b>${esc(s.comment)}</b> ${pill(s.active,s.active?'active':'expired')}<br><span class="muted">Service ${esc(s.service??'*')} · bis ${fmt(s.ends_at)} · ${esc(s.created_by)}</span>${s.active?`<br><button onclick="expireSilence('${s.silence_id}')">Beenden</button>`:''}</div>`).join(''):'<p class="muted">Keine Stummschaltungen.</p>'}
function renderCatalog(items,targets){const select=document.getElementById('metricSelect'),current=select.value;select.innerHTML='<option value="">alle Metriken</option>'+items.map(x=>`<option value="${esc(x.name)}">${esc(x.name)} (${x.series})</option>`).join('');select.value=current;const ts=document.getElementById('metricTarget'),tc=ts.value;ts.innerHTML='<option value="">alle Targets</option>'+targets.map(x=>`<option value="${esc(x.target_id)}">${esc(x.display_name)}</option>`).join('');ts.value=tc;loadSeries()}
async function loadSeries(){const q=new URLSearchParams({limit:'500'});if(metricSelect.value)q.set('metric',metricSelect.value);if(metricTarget.value)q.set('target_id',metricTarget.value);const rows=await api('/api/v1/metrics/series?'+q);document.getElementById('seriesTable').innerHTML=rows.map(x=>`<tr><td><code>${esc(x.name)}</code></td><td>${esc(x.target_id)}</td><td>${esc(JSON.stringify(x.labels))}</td><td>${Number(x.last_value).toPrecision(7)}</td><td>${fmt(x.last_at)}</td><td>${x.samples.length}</td></tr>`).join('')}
async function loadLogs(){const q=new URLSearchParams({limit:'1000'});if(logService.value)q.set('service',logService.value);if(logLevel.value)q.set('level',logLevel.value);if(logContains.value)q.set('contains',logContains.value);const rows=await api('/api/v1/logs?'+q);document.getElementById('logsTable').innerHTML=rows.map(x=>`<tr><td>${fmt(x.timestamp)}</td><td>${esc(x.service)}<br><span class="muted">${esc(x.node??'')}</span></td><td><span class="pill ${x.level==='error'?'bad':x.level==='warn'?'warn':''}">${esc(x.level)}</span></td><td>${esc(x.message)}<br><span class="muted">${esc(JSON.stringify(x.fields))}</span></td><td>${esc(x.trace_id??'')}</td></tr>`).join('')}
async function loadTraces(){const q=new URLSearchParams({limit:'1000'});if(traceService.value)q.set('service',traceService.value);if(traceId.value)q.set('trace_id',traceId.value);const rows=await api('/api/v1/traces?'+q);document.getElementById('tracesTable').innerHTML=rows.map(x=>`<tr><td>${fmt(x.started_at)}</td><td>${esc(x.trace_id)}<br><span class="muted">${esc(x.span_id)}</span></td><td>${esc(x.service)}</td><td>${esc(x.operation)}</td><td>${x.duration_ms.toFixed(2)} ms</td><td>${pill(x.status==='ok',x.status)}</td></tr>`).join('')}
async function loadAudit(){const rows=await api('/api/v1/audit?limit=2000');document.getElementById('auditTable').innerHTML=rows.map(x=>`<tr><td>${x.sequence}</td><td>${fmt(x.timestamp)}</td><td>${esc(x.actor)}</td><td>${esc(x.action)}</td><td>${esc(x.object_type)} / ${esc(x.object_id)}</td><td>${esc(x.result)}</td><td>${esc(JSON.stringify(x.detail))}</td></tr>`).join('')}
async function loadDiagnostics(){const rows=await api('/api/v1/diagnostics');document.getElementById('diagnosticsList').innerHTML=rows.map(x=>`<div class="card"><b>${esc(x.diagnostic_id)}</b> ${pill(x.state==='ready',x.state)}<br><span class="muted">${fmt(x.created_at)} · ${x.size_bytes??0} bytes · ${esc(x.sha256??'')}</span>${x.state==='ready'?`<br><a class="button" href="/api/v1/diagnostics/${x.diagnostic_id}/download">Download</a>`:''}</div>`).join('')}
async function targetTest(id){await api(`/api/v1/targets/${id}/test`,{method:'POST',body:'{}'});await refresh()} async function targetAction(id,action){await api(`/api/v1/targets/${id}/${action}`,{method:'POST',body:'{}'});await refresh()}
async function addTarget(){await api('/api/v1/targets',{method:'POST',body:JSON.stringify({target_id:tId.value,display_name:tName.value,service:tService.value,base_url:tUrl.value,labels:{environment:'open-lab'}})});tId.value=tName.value=tService.value='';await refresh()}
async function scrapeNow(){const out=await api('/api/v1/maintenance/scrape-now',{method:'POST',body:'{}'});document.getElementById('maintenanceOutput').textContent=JSON.stringify(out,null,2);await refresh()}
async function ruleAction(id,action){await api(`/api/v1/rules/${id}/${action}`,{method:'POST',body:'{}'});await refresh()} async function alertAction(id,action){await api(`/api/v1/alerts/${id}/${action}`,{method:'POST',body:JSON.stringify({actor:'webui'})});await refresh()}
async function addSilence(){await api('/api/v1/silences',{method:'POST',body:JSON.stringify({comment:sComment.value,duration_secs:Number(sDuration.value),service:sService.value||null,severity:sSeverity.value||null,created_by:'webui',match_labels:{}})});await refresh()} async function expireSilence(id){await api(`/api/v1/silences/${id}/expire`,{method:'POST',body:JSON.stringify({actor:'webui'})});await refresh()}
async function maintenance(){const out=await api('/api/v1/maintenance/tick',{method:'POST',body:JSON.stringify({actor:'webui'})});maintenanceOutput.textContent=JSON.stringify(out,null,2);await refresh()} async function backup(){const out=await api('/api/v1/maintenance/backup',{method:'POST',body:JSON.stringify({actor:'webui'})});maintenanceOutput.textContent=JSON.stringify(out,null,2)} async function diagnostic(){const out=await api('/api/v1/diagnostics',{method:'POST',body:JSON.stringify({actor:'webui',reason:'manual webui diagnostic',include_logs:true,include_traces:true,max_records:2000})});maintenanceOutput.textContent=JSON.stringify(out,null,2);await loadDiagnostics()}
initNav();grafanaLink.href=`http://${location.hostname}:3000/`;prometheusLink.href=`http://${location.hostname}:9090/`;alertmanagerLink.href=`http://${location.hostname}:9093/`;lokiLink.href=`http://${location.hostname}:3100/ready`;setInterval(()=>document.getElementById('clock').textContent=new Date().toLocaleString(),1000);refresh();setInterval(refresh,15000);
</script></body></html>"##;

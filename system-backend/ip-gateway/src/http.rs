use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

use serde::Serialize;
use serde_json::json;

use crate::config::IpGatewayConfig;
use crate::kernel;
use crate::protocol::{
    BlockAddressInput, CaptureStartInput, FirewallRuleInput, NatRuleInput, ReconcileInput,
    RouteRuleInput, StaticDnsInput,
};
use crate::runtime::RuntimeHandle;
use crate::state::SharedGateway;

pub fn spawn_http_server(
    config: IpGatewayConfig,
    gateway: SharedGateway,
    runtime: RuntimeHandle,
) -> std::io::Result<thread::JoinHandle<()>> {
    let listener = TcpListener::bind(config.server.bind)?;
    tracing::info!(
        "IP Gateway WebUI/API listening on http://{}",
        config.server.bind
    );
    Ok(thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let config = config.clone();
                    let gateway = gateway.clone();
                    let runtime = runtime.clone();
                    thread::spawn(move || {
                        if let Err(error) = handle_connection(stream, config, gateway, runtime) {
                            tracing::warn!("HTTP connection failed: {error}");
                        }
                    });
                }
                Err(error) => tracing::warn!("HTTP accept failed: {error}"),
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
    config: IpGatewayConfig,
    gateway: SharedGateway,
    runtime: RuntimeHandle,
) -> Result<(), String> {
    let request = read_request(&mut stream, config.limits.max_body_bytes)?;
    let response = route(request, config, gateway, runtime);
    write_response(&mut stream, response).map_err(|error| error.to_string())
}

fn route(
    request: HttpRequest,
    config: IpGatewayConfig,
    gateway: SharedGateway,
    runtime: RuntimeHandle,
) -> HttpResponse {
    if request.method == "OPTIONS" {
        return empty(204);
    }
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") => html(INDEX_HTML),
        ("GET", "/health/live") => json_response(200, &json!({"status":"live"})),
        ("GET", "/health/ready") => {
            let status = gateway.status();
            let ready = if status.authoritative {
                status.packet_core_connected && status.tun_open && status.kernel_last_error.is_none()
            } else {
                status.packet_core_connected
            };
            json_response(if ready { 200 } else { 503 }, &status)
        }
        ("GET", "/api/v1/status") => json_response(200, &gateway.status()),
        ("GET", "/api/v1/config") => json_response(200, &config),
        ("GET", "/api/v1/contexts") => json_response(200, &gateway.contexts()),
        ("GET", "/api/v1/routes") => json_response(200, &gateway.routes()),
        ("GET", "/api/v1/nat") => json_response(200, &gateway.nat_rules()),
        ("GET", "/api/v1/firewall") => json_response(200, &gateway.firewall_rules()),
        ("GET", "/api/v1/dns") => json_response(200, &gateway.dns_records()),
        ("GET", "/api/v1/blocked") => json_response(200, &gateway.blocked_addresses()),
        ("GET", "/api/v1/flows") => {
            let limit = query_usize(&request, "limit", 500, 10_000);
            json_response(200, &gateway.flows(limit))
        }
        ("GET", "/api/v1/captures") => json_response(200, &gateway.captures()),
        ("GET", "/api/v1/events") => {
            let limit = query_usize(&request, "limit", 250, 5_000);
            json_response(200, &gateway.recent_events(limit))
        }
        ("GET", "/api/v1/kernel/plan") => {
            json_response(200, &kernel::build_plan(&config, &gateway.kernel_snapshot()))
        }
        ("POST", "/api/v1/kernel/reconcile") => {
            let input = if request.body.is_empty() {
                Ok(ReconcileInput { force: false })
            } else {
                parse_json::<ReconcileInput>(&request.body)
            };
            match input {
                Ok(input) => match runtime.reconcile() {
                    Ok(plan) => json_response(202, &json!({"force":input.force,"plan":plan})),
                    Err(error) => json_response(503, &json!({"error":error})),
                },
                Err(error) => json_response(400, &json!({"error":error})),
            }
        }
        ("POST", "/api/v1/routes") => match parse_json::<RouteRuleInput>(&request.body)
            .and_then(|input| gateway.upsert_route(None, input))
        {
            Ok(record) => json_response(201, &record),
            Err(error) => json_response(409, &json!({"error":error})),
        },
        ("POST", "/api/v1/nat") => match parse_json::<NatRuleInput>(&request.body)
            .and_then(|input| gateway.upsert_nat(None, input))
        {
            Ok(record) => json_response(201, &record),
            Err(error) => json_response(409, &json!({"error":error})),
        },
        ("POST", "/api/v1/firewall") => {
            match parse_json::<FirewallRuleInput>(&request.body)
                .and_then(|input| gateway.upsert_firewall(None, input))
            {
                Ok(record) => json_response(201, &record),
                Err(error) => json_response(409, &json!({"error":error})),
            }
        }
        ("POST", "/api/v1/dns") => match parse_json::<StaticDnsInput>(&request.body)
            .and_then(|input| gateway.upsert_dns(None, input))
        {
            Ok(record) => json_response(201, &record),
            Err(error) => json_response(409, &json!({"error":error})),
        },
        ("POST", "/api/v1/blocked") => match parse_json::<BlockAddressInput>(&request.body)
            .and_then(|input| gateway.block_address(input))
        {
            Ok(record) => json_response(201, &record),
            Err(error) => json_response(409, &json!({"error":error})),
        },
        ("POST", "/api/v1/captures") => match parse_json::<CaptureStartInput>(&request.body)
            .and_then(|input| gateway.start_capture(input))
        {
            Ok(record) => json_response(201, &record),
            Err(error) => json_response(409, &json!({"error":error})),
        },
        ("GET", "/api/v1/export.json") => {
            download_json("netcore-ip-gateway-export.json", &gateway.export())
        }
        ("GET", "/metrics") => text(
            "text/plain; version=0.0.4; charset=utf-8",
            gateway.metrics(),
        ),
        ("GET", "/openapi.json") => json_response(200, &openapi()),
        _ => dynamic_route(request, gateway),
    }
}

fn dynamic_route(request: HttpRequest, gateway: SharedGateway) -> HttpResponse {
    let parts: Vec<&str> = request.path.trim_matches('/').split('/').collect();
    match (request.method.as_str(), parts.as_slice()) {
        ("PUT", ["api", "v1", "routes", id]) => match parse_json::<RouteRuleInput>(&request.body)
            .and_then(|input| gateway.upsert_route(Some(id), input))
        {
            Ok(record) => json_response(200, &record),
            Err(error) => json_response(409, &json!({"error":error})),
        },
        ("DELETE", ["api", "v1", "routes", id]) => result_empty(gateway.delete_route(id)),
        ("PUT", ["api", "v1", "nat", id]) => match parse_json::<NatRuleInput>(&request.body)
            .and_then(|input| gateway.upsert_nat(Some(id), input))
        {
            Ok(record) => json_response(200, &record),
            Err(error) => json_response(409, &json!({"error":error})),
        },
        ("DELETE", ["api", "v1", "nat", id]) => result_empty(gateway.delete_nat(id)),
        ("PUT", ["api", "v1", "firewall", id]) => {
            match parse_json::<FirewallRuleInput>(&request.body)
                .and_then(|input| gateway.upsert_firewall(Some(id), input))
            {
                Ok(record) => json_response(200, &record),
                Err(error) => json_response(409, &json!({"error":error})),
            }
        }
        ("DELETE", ["api", "v1", "firewall", id]) => {
            result_empty(gateway.delete_firewall(id))
        }
        ("PUT", ["api", "v1", "dns", id]) => match parse_json::<StaticDnsInput>(&request.body)
            .and_then(|input| gateway.upsert_dns(Some(id), input))
        {
            Ok(record) => json_response(200, &record),
            Err(error) => json_response(409, &json!({"error":error})),
        },
        ("DELETE", ["api", "v1", "dns", id]) => result_empty(gateway.delete_dns(id)),
        ("DELETE", ["api", "v1", "blocked", address]) => {
            result_empty(gateway.unblock_address(address))
        }
        ("POST", ["api", "v1", "captures", id, "stop"]) => {
            match gateway.stop_capture(id) {
                Ok(record) => json_response(200, &record),
                Err(error) => json_response(404, &json!({"error":error})),
            }
        }
        ("DELETE", ["api", "v1", "captures", id]) => {
            result_empty(gateway.delete_capture(id))
        }
        ("GET", ["api", "v1", "captures", id, "download"]) => {
            match gateway.capture(id) {
                Some(capture) => match fs::read(&capture.path) {
                    Ok(body) => HttpResponse {
                        status: 200,
                        content_type: "application/vnd.tcpdump.pcap",
                        body,
                        disposition: Some(format!(
                            "attachment; filename=\"{}.pcap\"",
                            safe_filename(&capture.name)
                        )),
                    },
                    Err(error) => json_response(404, &json!({"error":error.to_string()})),
                },
                None => json_response(404, &json!({"error":"capture not found"})),
            }
        }
        _ => json_response(404, &json!({"error":"not found"})),
    }
}

fn result_empty(result: Result<(), String>) -> HttpResponse {
    match result {
        Ok(()) => empty(204),
        Err(error) => json_response(404, &json!({"error":error})),
    }
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
            "title":"NetCore IP Gateway",
            "version":env!("CARGO_PKG_VERSION"),
            "description":"OPEN LAB Layer-3 TUN, routing, NAT, firewall, DNS, WAP/test and packet-capture API. No authentication, token or TLS."
        },
        "paths":{
            "/api/v1/status":{"get":{}},
            "/api/v1/contexts":{"get":{}},
            "/api/v1/routes":{"get":{},"post":{}},
            "/api/v1/routes/{id}":{"put":{},"delete":{}},
            "/api/v1/nat":{"get":{},"post":{}},
            "/api/v1/nat/{id}":{"put":{},"delete":{}},
            "/api/v1/firewall":{"get":{},"post":{}},
            "/api/v1/firewall/{id}":{"put":{},"delete":{}},
            "/api/v1/dns":{"get":{},"post":{}},
            "/api/v1/dns/{id}":{"put":{},"delete":{}},
            "/api/v1/blocked":{"get":{},"post":{}},
            "/api/v1/blocked/{address}":{"delete":{}},
            "/api/v1/flows":{"get":{}},
            "/api/v1/captures":{"get":{},"post":{}},
            "/api/v1/captures/{id}/stop":{"post":{}},
            "/api/v1/captures/{id}/download":{"get":{}},
            "/api/v1/kernel/plan":{"get":{}},
            "/api/v1/kernel/reconcile":{"post":{}},
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
        Err(error) => json_response(500, &json!({"error":error.to_string()})),
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
        409 => "Conflict",
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

fn safe_filename(value: &str) -> String {
    let value: String = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect();
    if value.is_empty() {
        "capture".to_string()
    } else {
        value
    }
}

const INDEX_HTML: &str = r#"<!doctype html>
<html lang="de"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>NetCore IP Gateway</title><style>
:root{font-family:Inter,system-ui,sans-serif;color:#e8edf4;background:#0b1018}*{box-sizing:border-box}body{margin:0}header{padding:18px 26px;background:#121a26;border-bottom:1px solid #293449;position:sticky;top:0;z-index:2}h1{margin:0;font-size:22px}main{padding:22px;max-width:1800px;margin:auto}.warn{padding:10px 14px;background:#4b3512;border:1px solid #8c6628;border-radius:8px;margin-top:10px}.cards{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:12px;margin:18px 0}.card,.panel{background:#121a26;border:1px solid #293449;border-radius:10px;padding:15px}.value{font-size:27px;font-weight:700}.muted{color:#99a7ba}.ok{color:#5dd39e}.bad{color:#ff7474}.toolbar{display:flex;gap:8px;flex-wrap:wrap;align-items:center;margin:10px 0}button,input,select,textarea{background:#0d1420;color:#e8edf4;border:1px solid #40506a;border-radius:6px;padding:8px}button{cursor:pointer}button.primary{background:#1769aa}button.danger{background:#7a2631}table{width:100%;border-collapse:collapse;font-size:13px}th,td{text-align:left;padding:8px;border-bottom:1px solid #293449;vertical-align:top}pre{max-height:420px;overflow:auto;white-space:pre-wrap}.tabs{display:flex;gap:6px;flex-wrap:wrap;margin:14px 0}.tabs button.active{background:#1769aa}.view{display:none}.view.active{display:block}.pill{padding:3px 7px;border-radius:99px;background:#1d2b40}.small{font-size:11px}a{color:#74b9ff}</style></head>
<body><header><h1>NetCore-Tetra · IP Gateway <span id="mode" class="pill">…</span></h1><div class="warn"><b>OPEN LAB:</b> keine Anmeldung, keine Token und kein TLS. Jeder erreichbare Client darf Routing, NAT, Firewall und Captures verändern.</div></header>
<main><div id="health" class="panel">Lade Status …</div><div id="cards" class="cards"></div>
<div class="tabs"><button class="active" onclick="show('overview',this)">Übersicht</button><button onclick="show('flows',this)">Flows</button><button onclick="show('policy',this)">Routing & Policy</button><button onclick="show('captures',this)">Captures</button><button onclick="show('diag',this)">Diagnose</button></div>
<section id="overview" class="view active"><div class="panel"><h2>PDP/IP-Leases</h2><table><thead><tr><th>IP</th><th>ISSI/NSAPI</th><th>TBS</th><th>Status</th><th>MTU/Priorität</th><th>Zähler Core</th></tr></thead><tbody id="contextRows"></tbody></table></div><div class="panel"><h2>Testdienste</h2><p>WAP/HTTP: <code>http://gateway:8088/wap/</code> · HTTP Echo: <code>/test/echo</code> · UDP Echo: <code>7007/udp</code> · DNS: <code>gateway:53/udp</code></p><p>Statische Namen: <code>netcore.test</code>, <code>wap.netcore.test</code>, <code>test.netcore.test</code>.</p></div></section>
<section id="flows" class="view"><div class="panel"><div class="toolbar"><h2 style="margin-right:auto">Aktive Flows</h2><button onclick="refresh()">Neu laden</button></div><table><thead><tr><th>Richtung</th><th>Protokoll</th><th>Quelle</th><th>Ziel</th><th>Teilnehmer</th><th>Pakete/Bytes</th><th>Block</th></tr></thead><tbody id="flowRows"></tbody></table></div></section>
<section id="policy" class="view"><div class="panel"><div class="toolbar"><h2 style="margin-right:auto">Routen</h2><button class="primary" onclick="addRoute()">Route hinzufügen</button></div><table><thead><tr><th>Name</th><th>Ziel</th><th>Gateway</th><th>Interface</th><th>Aktiv</th><th></th></tr></thead><tbody id="routeRows"></tbody></table></div><div class="panel"><div class="toolbar"><h2 style="margin-right:auto">Firewallregeln</h2><button class="primary" onclick="addFirewall()">Regel hinzufügen</button></div><table><thead><tr><th>Prio</th><th>Name</th><th>Chain</th><th>Match</th><th>Aktion</th><th></th></tr></thead><tbody id="firewallRows"></tbody></table></div><div class="panel"><div class="toolbar"><h2 style="margin-right:auto">NAT</h2><button class="primary" onclick="addNat()">NAT-Regel hinzufügen</button></div><table><thead><tr><th>Name</th><th>Typ</th><th>Quelle/Ziel</th><th>Übersetzung</th><th></th></tr></thead><tbody id="natRows"></tbody></table></div><div class="panel"><div class="toolbar"><h2 style="margin-right:auto">DNS</h2><button class="primary" onclick="addDns()">A-Record hinzufügen</button></div><table><thead><tr><th>Name</th><th>Adresse</th><th>Quelle</th><th></th></tr></thead><tbody id="dnsRows"></tbody></table></div></section>
<section id="captures" class="view"><div class="panel"><div class="toolbar"><h2 style="margin-right:auto">PCAP Captures</h2><button class="primary" onclick="addCapture()">Capture starten</button></div><table><thead><tr><th>Name</th><th>Status</th><th>Filter</th><th>Pakete</th><th>Bytes</th><th></th></tr></thead><tbody id="captureRows"></tbody></table></div></section>
<section id="diag" class="view"><div class="panel"><div class="toolbar"><h2 style="margin-right:auto">Kernel-Plan</h2><button class="primary" onclick="reconcile()">Jetzt abgleichen</button></div><pre id="kernelPlan"></pre></div><div class="panel"><h2>Ereignisse</h2><pre id="eventLog"></pre></div><div class="panel"><h2>Konfiguration</h2><pre id="configDump"></pre><p><a href="/openapi.json" target="_blank">OpenAPI</a> · <a href="/metrics" target="_blank">Prometheus</a> · <a href="/api/v1/export.json">JSON-Export</a></p></div></section></main>
<script>
let contexts=[],flows=[],routes=[],nat=[],firewall=[],dns=[],captures=[],events=[],plan={};
function show(id,b){document.querySelectorAll('.view').forEach(x=>x.classList.remove('active'));document.querySelectorAll('.tabs button').forEach(x=>x.classList.remove('active'));document.getElementById(id).classList.add('active');b.classList.add('active')}
async function api(path,opt){const r=await fetch(path,opt);if(!r.ok&&r.status!==204){let e={};try{e=await r.json()}catch{}throw new Error(e.error||r.statusText)}return r.status===204?null:r.json()}
function esc(v){return String(v??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]))}
async function refresh(){try{const [s,c,f,r,n,w,d,cap,e,p,cfg]=await Promise.all([api('/api/v1/status'),api('/api/v1/contexts'),api('/api/v1/flows?limit=1000'),api('/api/v1/routes'),api('/api/v1/nat'),api('/api/v1/firewall'),api('/api/v1/dns'),api('/api/v1/captures'),api('/api/v1/events?limit=200'),api('/api/v1/kernel/plan'),api('/api/v1/config')]);contexts=c;flows=f;routes=r;nat=n;firewall=w;dns=d;captures=cap;events=e;plan=p;document.getElementById('mode').textContent=s.mode.toUpperCase();document.getElementById('health').innerHTML=`Packet Core: ${s.packet_core_connected?'<span class="ok">verbunden</span>':'<span class="bad">getrennt</span>'} · TUN ${esc(s.tun_name)}: ${s.tun_open?'<span class="ok">offen</span>':'<span class="bad">geschlossen</span>'} · Kernel: ${s.kernel_last_error?'<span class="bad">'+esc(s.kernel_last_error)+'</span>':'<span class="ok">bereit</span>'}`;document.getElementById('cards').innerHTML=[['Contexts',s.contexts],['Flows',s.flows],['UL Pakete',s.packets_uplink],['DL Pakete',s.packets_downlink],['Drops',s.packets_dropped],['Captures',s.captures_active],['DNS Queries',s.dns_queries],['Testzugriffe',s.test_requests]].map(x=>`<div class="card"><div class="muted">${x[0]}</div><div class="value">${x[1]}</div></div>`).join('');render();document.getElementById('kernelPlan').textContent=JSON.stringify(plan,null,2);document.getElementById('eventLog').textContent=events.map(e=>`${e.timestamp} #${e.sequence} ${e.kind} ${JSON.stringify(e.detail)}`).join('\n');document.getElementById('configDump').textContent=JSON.stringify(cfg,null,2)}catch(e){document.getElementById('health').innerHTML='<span class="bad">UI-Fehler: '+esc(e.message)+'</span>'}}
function render(){document.getElementById('contextRows').innerHTML=contexts.map(c=>`<tr><td><b>${esc(c.ipv4)}</b></td><td>${c.issi}/${c.nsapi}</td><td>${esc(c.node_id)}</td><td>${esc(c.state)} · ${c.available?'verfügbar':'gesperrt'}</td><td>${c.mtu}/P${c.priority}</td><td>UL ${c.packets_up}/${c.bytes_up} B<br>DL ${c.packets_down}/${c.bytes_down} B</td></tr>`).join('');document.getElementById('flowRows').innerHTML=flows.map(f=>`<tr><td>${esc(f.direction)}</td><td>${esc(f.protocol)}</td><td>${esc(f.source)}${f.source_port?':'+f.source_port:''}</td><td>${esc(f.destination)}${f.destination_port?':'+f.destination_port:''}</td><td>${f.issi??'-'}/${f.nsapi??'-'}<br><span class="small muted">${esc(f.node_id||'')}</span></td><td>${f.packets}/${f.bytes} B</td><td>${f.blocked?'<span class="bad">blockiert</span>':'<button onclick="block(\''+(f.direction==='uplink'?f.source:f.destination)+'\')">Blocken</button>'}</td></tr>`).join('');document.getElementById('routeRows').innerHTML=routes.map(x=>`<tr><td>${esc(x.name)}</td><td>${esc(x.destination)}</td><td>${esc(x.gateway||'-')}</td><td>${esc(x.interface||'-')}</td><td>${x.enabled?'ja':'nein'}</td><td><button class="danger" onclick="del('/api/v1/routes/${x.id}')">Löschen</button></td></tr>`).join('');document.getElementById('firewallRows').innerHTML=firewall.sort((a,b)=>a.priority-b.priority).map(x=>`<tr><td>${x.priority}</td><td>${esc(x.name)}</td><td>${esc(x.chain)}</td><td>${esc(x.protocol)} ${esc(x.source_cidr||'')} → ${esc(x.destination_cidr||'')} ${x.destination_port?'port '+x.destination_port:''}</td><td>${esc(x.action)}</td><td><button class="danger" onclick="del('/api/v1/firewall/${x.id}')">Löschen</button></td></tr>`).join('');document.getElementById('natRows').innerHTML=nat.map(x=>`<tr><td>${esc(x.name)}</td><td>${esc(x.kind)}</td><td>${esc(x.source_cidr||'')} → ${esc(x.destination_cidr||'')}</td><td>${esc(x.to_address||'')}${x.to_port?':'+x.to_port:''}</td><td><button class="danger" onclick="del('/api/v1/nat/${x.id}')">Löschen</button></td></tr>`).join('');document.getElementById('dnsRows').innerHTML=dns.map(x=>`<tr><td>${esc(x.name)}</td><td>${esc(x.address)}</td><td>${esc(x.source)}</td><td>${x.source==='builtin'?'':'<button class="danger" onclick="del(\'/api/v1/dns/'+x.id+'\')">Löschen</button>'}</td></tr>`).join('');document.getElementById('captureRows').innerHTML=captures.map(x=>`<tr><td>${esc(x.name)}</td><td>${esc(x.state)}</td><td>${esc(x.direction)} ${esc(x.host||'')} ${esc(x.protocol||'')} ${x.port||''}</td><td>${x.packet_count}</td><td>${x.captured_bytes}/${x.original_bytes}</td><td><a href="/api/v1/captures/${x.id}/download">PCAP</a> ${x.state==='active'?'<button onclick="post(\'/api/v1/captures/'+x.id+'/stop\',{})">Stop</button>':''} <button class="danger" onclick="del('/api/v1/captures/${x.id}')">Löschen</button></td></tr>`).join('')}
async function post(path,p){try{await api(path,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(p)});refresh()}catch(e){alert(e.message)}}async function del(path){if(!confirm('Wirklich löschen?'))return;try{await api(path,{method:'DELETE'});refresh()}catch(e){alert(e.message)}}
function addRoute(){const destination=prompt('Zielnetz (CIDR)','192.168.50.0/24');if(!destination)return;const gateway=prompt('Gateway IPv4 (leer = direkt)','');const iface=prompt('Interface (leer = ntc-tun0)','');post('/api/v1/routes',{name:'WebUI route',destination,gateway:gateway||null,interface:iface||'ntc-tun0',metric:null,enabled:true})}
function addFirewall(){const chain=prompt('Chain: input, forward oder output','forward');if(!chain)return;const protocol=prompt('Protokoll: any, tcp, udp, icmp','tcp');const destination_port=protocol==='tcp'||protocol==='udp'?Number(prompt('Zielport (0 = keiner)','80'))||null:null;const action=prompt('Aktion: accept, drop, reject','accept');post('/api/v1/firewall',{name:'WebUI rule',chain,action,protocol,source_cidr:null,destination_cidr:null,source_port:null,destination_port,in_interface:'ntc-tun0',out_interface:null,priority:100,log:false,enabled:true})}
function addNat(){const kind=prompt('Typ: masquerade, snat, dnat','masquerade');if(!kind)return;const source_cidr=prompt('Quellnetz (optional)','10.0.0.0/24');const to_address=kind==='masquerade'?null:prompt('Zieladresse','');post('/api/v1/nat',{name:'WebUI NAT',kind,source_cidr:source_cidr||null,destination_cidr:null,protocol:null,destination_port:null,out_interface:null,to_address:to_address||null,to_port:null,enabled:true})}
function addDns(){const name=prompt('DNS-Name','service.netcore.test');if(!name)return;const address=prompt('IPv4-Adresse','10.0.0.1');if(address)post('/api/v1/dns',{name,address})}
function addCapture(){const name=prompt('Capture-Name','packet-data');if(name===null)return;const direction=prompt('Richtung: uplink, downlink, both','both');post('/api/v1/captures',{name,direction,host:null,protocol:null,port:null})}
function block(address){if(confirm(address+' blockieren?'))post('/api/v1/blocked',{address,reason:'WebUI flow block'})}async function reconcile(){try{await api('/api/v1/kernel/reconcile',{method:'POST',headers:{'Content-Type':'application/json'},body:'{}'});refresh()}catch(e){alert(e.message)}}
refresh();setInterval(refresh,4000);
</script></body></html>"#;

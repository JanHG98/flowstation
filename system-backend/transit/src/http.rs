use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

use serde::Serialize;
use serde_json::json;

use crate::config::TransitConfig;
use crate::protocol::{
    DeliveryAckInput, GroupReachabilityInput, MaintenanceInput, PeerActionInput, PeerCreateInput,
    PeerHeartbeatInput, RouteActionInput, RouteCreateInput, RouteResolveInput, SessionActionInput,
    SubscriberLocationInput, TransitEnvelopeInput, TransitSubmitInput,
};
use crate::state::SharedTransit;

pub fn spawn_http_server(
    config: TransitConfig,
    transit: SharedTransit,
) -> std::io::Result<thread::JoinHandle<()>> {
    let listener = TcpListener::bind(config.server.bind)?;
    tracing::info!(
        "Transit WebUI/API listening on http://{}",
        config.server.bind
    );
    Ok(thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let config = config.clone();
                    let transit = transit.clone();
                    thread::spawn(move || {
                        if let Err(error) = handle_connection(stream, config, transit) {
                            tracing::warn!("Transit HTTP connection failed: {}", error);
                        }
                    });
                }
                Err(error) => tracing::warn!("Transit HTTP accept failed: {}", error),
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
    config: TransitConfig,
    transit: SharedTransit,
) -> Result<(), String> {
    let request = read_request(&mut stream, config.limits.max_body_bytes)?;
    let response = route(request, config, transit);
    write_response(&mut stream, response).map_err(|error| error.to_string())
}

fn route(request: HttpRequest, config: TransitConfig, transit: SharedTransit) -> HttpResponse {
    if request.method == "OPTIONS" {
        return empty(204);
    }
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") => html(INDEX_HTML),
        ("GET", "/health/live") => json_response(200, &json!({"status":"live"})),
        ("GET", "/health/ready") => {
            let status = transit.status();
            json_response(if status.ready { 200 } else { 503 }, &status)
        }
        ("GET", "/api/v1/status") => json_response(200, &transit.status()),
        ("GET", "/api/v1/config") => json_response(200, &config),
        ("GET", "/api/v1/peers") => json_response(200, &transit.peers()),
        ("POST", "/api/v1/peers") => match parse_json::<PeerCreateInput>(&request.body)
            .and_then(|input| transit.create_peer(input))
        {
            Ok(peer) => json_response(201, &peer),
            Err(error) => json_response(409, &json!({"error":error})),
        },
        ("GET", "/api/v1/routes") => json_response(200, &transit.routes()),
        ("POST", "/api/v1/routes") => match parse_json::<RouteCreateInput>(&request.body)
            .and_then(|input| transit.create_route(input))
        {
            Ok(route) => json_response(201, &route),
            Err(error) => json_response(409, &json!({"error":error})),
        },
        ("GET", "/api/v1/locations/subscribers") => {
            json_response(200, &transit.subscriber_locations())
        }
        ("POST", "/api/v1/locations/subscribers") => {
            match parse_json::<SubscriberLocationInput>(&request.body)
                .and_then(|input| transit.update_subscriber_location(input))
            {
                Ok(location) => json_response(200, &location),
                Err(error) => json_response(409, &json!({"error":error})),
            }
        }
        ("GET", "/api/v1/locations/groups") => {
            json_response(200, &transit.group_reachability())
        }
        ("POST", "/api/v1/locations/groups") => {
            match parse_json::<GroupReachabilityInput>(&request.body)
                .and_then(|input| transit.update_group_reachability(input))
            {
                Ok(location) => json_response(200, &location),
                Err(error) => json_response(409, &json!({"error":error})),
            }
        }
        ("POST", "/api/v1/route/resolve") => {
            match parse_json::<RouteResolveInput>(&request.body)
                .and_then(|input| transit.resolve(input))
            {
                Ok(decision) => json_response(if decision.accepted { 200 } else { 409 }, &decision),
                Err(error) => json_response(409, &json!({"error":error})),
            }
        }
        ("GET", "/api/v1/sessions") => json_response(200, &transit.sessions()),
        ("GET", "/api/v1/outbound") => {
            let peer_id = request.query.get("peer_id").map(String::as_str);
            let limit = query_usize(&request, "limit", 500, 5_000);
            json_response(200, &transit.outbound(peer_id, limit))
        }
        ("GET", "/api/v1/local-deliveries") => {
            let service = request.query.get("service").map(String::as_str);
            let limit = query_usize(&request, "limit", 500, 5_000);
            json_response(200, &transit.local_deliveries(service, limit))
        }
        ("GET", "/api/v1/events") => {
            let limit = query_usize(&request, "limit", 500, 5_000);
            json_response(200, &transit.recent_events(limit))
        }
        ("POST", "/api/v1/transit/submit") => {
            match parse_json::<TransitSubmitInput>(&request.body)
                .and_then(|input| transit.submit(input))
            {
                Ok(result) => json_response(202, &result),
                Err(error) => json_response(409, &json!({"error":error})),
            }
        }
        ("POST", "/api/v1/peer/heartbeat") => {
            match parse_json::<PeerHeartbeatInput>(&request.body)
                .and_then(|input| transit.ingest_heartbeat(input))
            {
                Ok(peer) => json_response(200, &peer),
                Err(error) => json_response(409, &json!({"error":error})),
            }
        }
        ("POST", "/api/v1/peer/envelopes") => {
            match parse_json::<TransitEnvelopeInput>(&request.body)
                .and_then(|input| transit.ingest_envelope(input))
            {
                Ok(result) => json_response(202, &result),
                Err(error) => json_response(409, &json!({"error":error})),
            }
        }
        ("POST", "/api/v1/maintenance/tick") => {
            let input = parse_json_or_default::<MaintenanceInput>(&request.body);
            match input.and_then(|input| transit.maintenance_tick(input)) {
                Ok(status) => json_response(200, &status),
                Err(error) => json_response(409, &json!({"error":error})),
            }
        }
        ("POST", "/api/v1/maintenance/backup") => match transit.backup() {
            Ok(result) => json_response(201, &result),
            Err(error) => json_response(500, &json!({"error":error})),
        },
        ("GET", "/api/v1/export.json") => {
            download_json("netcore-transit-export.json", &transit.export())
        }
        ("GET", "/metrics") => text(
            "text/plain; version=0.0.4; charset=utf-8",
            transit.metrics(),
        ),
        ("GET", "/openapi.json") => json_response(200, &openapi()),
        _ => dynamic_route(request, transit),
    }
}

fn dynamic_route(request: HttpRequest, transit: SharedTransit) -> HttpResponse {
    let parts: Vec<&str> = request.path.trim_matches('/').split('/').collect();
    match (request.method.as_str(), parts.as_slice()) {
        ("POST", ["api", "v1", "peers", peer_id, action]) => {
            match parse_json_or_default::<PeerActionInput>(&request.body)
                .and_then(|input| transit.peer_action(peer_id, action, input))
            {
                Ok(peer) => json_response(200, &peer),
                Err(error) => json_response(409, &json!({"error":error})),
            }
        }
        ("POST", ["api", "v1", "routes", route_id, action]) => {
            match parse_json_or_default::<RouteActionInput>(&request.body)
                .and_then(|input| transit.route_action(route_id, action, input))
            {
                Ok(route) => json_response(200, &route),
                Err(error) => json_response(409, &json!({"error":error})),
            }
        }
        ("DELETE", ["api", "v1", "routes", route_id]) => {
            match transit.delete_route(route_id) {
                Ok(()) => empty(204),
                Err(error) => json_response(404, &json!({"error":error})),
            }
        }
        ("POST", ["api", "v1", "sessions", session_id, action]) => {
            match parse_json_or_default::<SessionActionInput>(&request.body)
                .and_then(|input| transit.session_action(session_id, action, input))
            {
                Ok(session) => json_response(200, &session),
                Err(error) => json_response(409, &json!({"error":error})),
            }
        }
        ("POST", ["api", "v1", "local-deliveries", delivery_id, "ack"]) => {
            match parse_json::<DeliveryAckInput>(&request.body)
                .and_then(|input| transit.acknowledge_local_delivery(delivery_id, input))
            {
                Ok(delivery) => json_response(200, &delivery),
                Err(error) => json_response(409, &json!({"error":error})),
            }
        }
        _ => json_response(404, &json!({"error":"not found"})),
    }
}

fn parse_json<T: serde::de::DeserializeOwned>(body: &[u8]) -> Result<T, String> {
    serde_json::from_slice(body).map_err(|error| format!("invalid JSON: {error}"))
}

fn parse_json_or_default<T: serde::de::DeserializeOwned + Default>(body: &[u8]) -> Result<T, String> {
    if body.is_empty() {
        Ok(T::default())
    } else {
        parse_json(body)
    }
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
            "title":"NetCore Transit",
            "version":env!("CARGO_PKG_VERSION"),
            "description":"OPEN LAB NetCore-native regional mobility, individual/group call, SDS, media and supplementary-service transit. No authentication, no token and no TLS. This is not yet ETSI ISI."
        },
        "paths":{
            "/api/v1/status":{"get":{}},
            "/api/v1/config":{"get":{}},
            "/api/v1/peers":{"get":{},"post":{}},
            "/api/v1/peers/{peer_id}/{action}":{"post":{}},
            "/api/v1/routes":{"get":{},"post":{}},
            "/api/v1/routes/{route_id}/{action}":{"post":{}},
            "/api/v1/routes/{route_id}":{"delete":{}},
            "/api/v1/route/resolve":{"post":{}},
            "/api/v1/locations/subscribers":{"get":{},"post":{}},
            "/api/v1/locations/groups":{"get":{},"post":{}},
            "/api/v1/transit/submit":{"post":{}},
            "/api/v1/peer/heartbeat":{"post":{}},
            "/api/v1/peer/envelopes":{"post":{}},
            "/api/v1/sessions":{"get":{}},
            "/api/v1/sessions/{session_id}/{action}":{"post":{}},
            "/api/v1/outbound":{"get":{}},
            "/api/v1/local-deliveries":{"get":{}},
            "/api/v1/local-deliveries/{delivery_id}/ack":{"post":{}},
            "/api/v1/events":{"get":{}},
            "/api/v1/maintenance/tick":{"post":{}},
            "/api/v1/maintenance/backup":{"post":{}},
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

fn download_json<T: Serialize>(filename: &str, value: &T) -> HttpResponse {
    match serde_json::to_vec_pretty(value) {
        Ok(body) => HttpResponse {
            status: 200,
            content_type: "application/json; charset=utf-8",
            body,
            disposition: Some(format!("attachment; filename=\"{filename}\"")),
        },
        Err(error) => json_response(500, &json!({"error":error.to_string()})),
    }
}

fn html(body: &'static str) -> HttpResponse {
    HttpResponse {
        status: 200,
        content_type: "text/html; charset=utf-8",
        body: body.as_bytes().to_vec(),
        disposition: None,
    }
}

fn text(content_type: &'static str, body: String) -> HttpResponse {
    HttpResponse {
        status: 200,
        content_type,
        body: body.into_bytes(),
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
    let mut buffer = Vec::with_capacity(8_192);
    let mut temporary = [0_u8; 4_096];
    let header_end;
    loop {
        let count = stream
            .read(&mut temporary)
            .map_err(|error| format!("read request: {error}"))?;
        if count == 0 {
            return Err("connection closed before request was complete".to_string());
        }
        buffer.extend_from_slice(&temporary[..count]);
        if let Some(position) = find_bytes(&buffer, b"\r\n\r\n") {
            header_end = position + 4;
            break;
        }
        if buffer.len() > 65_536 {
            return Err("request header is too large".to_string());
        }
    }
    let header_text = std::str::from_utf8(&buffer[..header_end])
        .map_err(|error| format!("request header is not UTF-8: {error}"))?;
    let mut lines = header_text.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| "missing request line".to_string())?;
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| "missing request method".to_string())?
        .to_string();
    let target = parts
        .next()
        .ok_or_else(|| "missing request target".to_string())?
        .to_string();
    let content_length = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .map(|(_, value)| value.trim().parse::<usize>())
        .transpose()
        .map_err(|error| format!("invalid Content-Length: {error}"))?
        .unwrap_or(0);
    if content_length > max_body_bytes {
        return Err(format!("request body exceeds {max_body_bytes} bytes"));
    }
    while buffer.len() < header_end + content_length {
        let count = stream
            .read(&mut temporary)
            .map_err(|error| format!("read request body: {error}"))?;
        if count == 0 {
            return Err("connection closed before request body was complete".to_string());
        }
        buffer.extend_from_slice(&temporary[..count]);
    }
    let body = buffer[header_end..header_end + content_length].to_vec();
    let (path, query) = split_target(&target);
    Ok(HttpRequest {
        method,
        path,
        query,
        body,
    })
}

fn split_target(target: &str) -> (String, HashMap<String, String>) {
    let (path, raw_query) = target.split_once('?').unwrap_or((target, ""));
    let mut query = HashMap::new();
    for pair in raw_query.split('&').filter(|value| !value.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        query.insert(percent_decode(key), percent_decode(value));
    }
    (percent_decode(path), query)
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() => {
                let high = hex_value(bytes[index + 1]);
                let low = hex_value(bytes[index + 2]);
                if let (Some(high), Some(low)) = (high, low) {
                    output.push((high << 4) | low);
                    index += 3;
                    continue;
                }
                output.push(bytes[index]);
                index += 1;
            }
            b'+' => {
                output.push(b' ');
                index += 1;
            }
            other => {
                output.push(other);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&output).into_owned()
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
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
        413 => "Payload Too Large",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Response",
    };
    let mut headers = format!(
        concat!(
            "HTTP/1.1 {} {}\r\n",
            "Content-Type: {}\r\n",
            "Content-Length: {}\r\n",
            "Access-Control-Allow-Origin: *\r\n",
            "Access-Control-Allow-Methods: GET, POST, DELETE, OPTIONS\r\n",
            "Access-Control-Allow-Headers: Content-Type, X-NetCore-Transit-Protocol\r\n",
            "X-NetCore-Security-Mode: open-lab\r\n",
            "X-NetCore-Transit-Protocol: netcore-transit-v1\r\n",
            "X-NetCore-ETSI-ISI: not-implemented\r\n",
            "X-Content-Type-Options: nosniff\r\n",
            "Content-Security-Policy: default-src 'self'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; connect-src 'self'\r\n"
        ),
        response.status,
        reason,
        response.content_type,
        response.body.len()
    );
    if let Some(disposition) = response.disposition {
        headers.push_str(&format!("Content-Disposition: {disposition}\r\n"));
    }
    headers.push_str("Connection: close\r\n\r\n");
    stream.write_all(headers.as_bytes())?;
    stream.write_all(&response.body)?;
    stream.flush()
}

const INDEX_HTML: &str = r#"<!doctype html>
<html lang="de">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>NetCore Transit</title>
<style>
:root{font-family:Inter,system-ui,sans-serif;color:#e8edf7;background:#0b1020;--card:#121a2e;--muted:#91a0ba;--accent:#6aa7ff;--danger:#ff6b7a;--ok:#48d597;--warn:#ffcc66;--line:#27324b}*{box-sizing:border-box}body{margin:0}.banner{background:#7c2d12;color:#fff;padding:10px 18px;font-weight:800;text-align:center}.layout{display:grid;grid-template-columns:240px 1fr;min-height:calc(100vh - 40px)}aside{padding:20px;background:#0e1527;border-right:1px solid var(--line)}main{padding:24px;overflow:auto}h1{font-size:20px;margin:0 0 6px}h2{margin-top:0}.muted{color:var(--muted)}nav{display:grid;gap:6px;margin-top:24px}nav button{border:0;text-align:left;padding:10px 12px;border-radius:8px;background:transparent;color:#dbe5f6;cursor:pointer}nav button.active,nav button:hover{background:#1a2742}.page{display:none}.page.active{display:block}.grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(190px,1fr));gap:14px}.card{background:var(--card);border:1px solid var(--line);border-radius:12px;padding:16px;margin-bottom:16px}.metric{font-size:30px;font-weight:800;margin:6px 0}.scroll{overflow:auto;max-height:60vh}table{border-collapse:collapse;width:100%;font-size:13px}th,td{text-align:left;padding:9px;border-bottom:1px solid var(--line);vertical-align:top}th{position:sticky;top:0;background:#16213a}input,select,textarea{width:100%;padding:9px;border-radius:7px;border:1px solid #33415f;background:#0b1223;color:#eef4ff;margin-top:4px}textarea{min-height:90px}.form{display:grid;grid-template-columns:repeat(auto-fit,minmax(180px,1fr));gap:12px}.action{border:0;border-radius:7px;padding:8px 11px;background:var(--accent);color:#071225;font-weight:800;cursor:pointer;margin:2px}.action.secondary{background:#31415f;color:#fff}.action.danger{background:var(--danger);color:#22050a}.pill{display:inline-block;border-radius:999px;padding:3px 8px;background:#293754}.pill.ok{background:#174c3a;color:#9affd7}.pill.warn{background:#5a4518;color:#ffe49b}.pill.danger{background:#5a1d29;color:#ffb7c1}.mono{font-family:ui-monospace,SFMono-Regular,monospace}.notice{border-left:4px solid var(--warn);padding:10px 12px;background:#2b2617;margin:12px 0}pre{white-space:pre-wrap;word-break:break-word;background:#080d18;padding:12px;border-radius:8px}@media(max-width:800px){.layout{grid-template-columns:1fr}aside{border-right:0;border-bottom:1px solid var(--line)}nav{grid-template-columns:repeat(2,1fr)}main{padding:14px}}
</style>
</head>
<body>
<div class="banner">OPEN LAB · keine Anmeldung · keine Tokens · kein TLS · nur isoliertes Testnetz</div>
<div class="layout"><aside><h1>NetCore Transit</h1><div class="muted">Regionaler Core-Transit</div><nav id="nav"></nav></aside><main>
<section id="overview" class="page active"><h2>Übersicht</h2><div id="metrics" class="grid"></div><div class="card"><div class="notice"><b>NetCore-native Transit v1</b> ist eine interne Edge/Core- und Region-zu-Region-Schnittstelle. ETSI ISI folgt als separater Interworking-Layer; wir kleben das Etikett nicht vorzeitig drauf.</div><pre id="statusJson"></pre></div></section>
<section id="peers" class="page"><h2>Regionen & Peers</h2><div class="card"><h3>Peer anlegen</h3><div class="form"><label>Peer-ID<input id="pId" value="region-b-primary"></label><label>Region-ID<input id="pRegion" value="region-b"></label><label>SwMI-ID<input id="pSwmi" value="netcore-swmi-b"></label><label>Name<input id="pName" value="NetCore Region B"></label><label>Endpoint<input id="pEndpoint" value="http://10.0.20.12:8200"></label><label>Priorität<input id="pPriority" type="number" value="100"></label><label>Capabilities<input id="pCaps" value="mobility,individual_call,group_call,sds,media,supplementary_service"></label></div><button class="action" onclick="createPeer()">Peer speichern</button></div><div class="card scroll"><table><thead><tr><th>Peer/Region</th><th>Endpoint</th><th>Admin</th><th>Operativ</th><th>Latenz</th><th>Protokoll</th><th>Aktionen</th></tr></thead><tbody id="peerRows"></tbody></table></div></section>
<section id="routes" class="page"><h2>Routing</h2><div class="card"><h3>Route anlegen</h3><div class="form"><label>Dienst<select id="rService"><option>individual_call</option><option>group_call</option><option>sds</option><option>media</option><option>mobility</option><option>supplementary_service</option><option>*</option></select></label><label>Selector<select id="rType"><option>default</option><option>region</option><option>issi</option><option>gssi</option><option>prefix</option></select></label><label>Selector-Wert<input id="rValue" value=""></label><label>Zielregion<input id="rRegion" value="region-b"></label><label>Peer-ID<input id="rPeer" value="region-b-primary"></label><label>Präferenz<input id="rPref" type="number" value="100"></label><label>Metrik<input id="rMetric" type="number" value="100"></label><label>Failover-Gruppe<input id="rFailover" value="region-b-links"></label></div><button class="action" onclick="createRoute()">Route anlegen</button></div><div class="card scroll"><table><thead><tr><th>Dienst</th><th>Selector</th><th>Ziel</th><th>Peer</th><th>Pref/Metric</th><th>Status</th><th>Aktionen</th></tr></thead><tbody id="routeRows"></tbody></table></div></section>
<section id="locations" class="page"><h2>Teilnehmer- & Gruppenregionen</h2><div class="grid"><div class="card"><h3>ISSI-Region</h3><label>ISSI<input id="lIssi" type="number" value="4010001"></label><label>Home<input id="lHome" value="region-a"></label><label>Aktuell<input id="lCurrent" value="region-b"></label><label>Serving Node<input id="lNode" value="tbs-b1"></label><button class="action" onclick="saveSubscriber()">Speichern</button></div><div class="card"><h3>GSSI-Reichweite</h3><label>GSSI<input id="lGssi" type="number" value="2000"></label><label>Regionen<input id="lRegions" value="region-a,region-b"></label><button class="action" onclick="saveGroup()">Speichern</button></div></div><div class="card scroll"><h3>Teilnehmer</h3><table><thead><tr><th>ISSI</th><th>Home</th><th>Aktuell</th><th>Node</th><th>Seq</th><th>Stand</th></tr></thead><tbody id="subscriberRows"></tbody></table></div><div class="card scroll"><h3>Gruppen</h3><table><thead><tr><th>GSSI</th><th>Regionen</th><th>Quelle</th><th>Stand</th></tr></thead><tbody id="groupRows"></tbody></table></div></section>
<section id="sessions" class="page"><h2>Transit-Sessions</h2><div class="card scroll"><table><thead><tr><th>Session</th><th>Dienst</th><th>Quelle → Ziel</th><th>Zustand</th><th>Legs</th><th>Frames/PDUs</th><th>Aktionen</th></tr></thead><tbody id="sessionRows"></tbody></table></div></section>
<section id="traffic" class="page"><h2>Traffic-Test & Queues</h2><div class="card"><h3>Lokalen Transitauftrag einspeisen</h3><div class="form"><label>Dienst<select id="tService"><option>sds</option><option>individual_call</option><option>group_call</option><option>media</option><option>mobility</option></select></label><label>Operation<input id="tOperation" value="unitdata"></label><label>Quelle<input id="tSource" value="4010001"></label><label>Zieltyp<select id="tKind"><option>issi</option><option>gssi</option><option>region</option></select></label><label>Ziel<input id="tDestination" value="4010002"></label><label>Zielregion optional<input id="tRegion" value="region-b"></label><label>Priorität<input id="tPriority" type="number" value="5"></label></div><label>Payload JSON<textarea id="tPayload">{"text":"Hallo aus Region A"}</textarea></label><button class="action" onclick="submitTransit()">Transitauftrag absenden</button></div><div class="card scroll"><h3>Outbound</h3><table><thead><tr><th>Envelope</th><th>Dienst</th><th>Zielregion</th><th>Peer</th><th>Status</th><th>Versuche</th><th>Fehler</th></tr></thead><tbody id="outboundRows"></tbody></table></div><div class="card scroll"><h3>Lokale Zustellungen</h3><table><thead><tr><th>Delivery</th><th>Dienst</th><th>Quelle</th><th>Ziel</th><th>Status</th><th>Payload</th><th>Aktion</th></tr></thead><tbody id="deliveryRows"></tbody></table></div></section>
<section id="events" class="page"><h2>Ereignisse & Audit</h2><div class="card scroll"><table><thead><tr><th>Seq</th><th>Zeit</th><th>Level</th><th>Kategorie</th><th>Aktion</th><th>Actor</th><th>Ziel</th><th>Details</th></tr></thead><tbody id="eventRows"></tbody></table></div></section>
<section id="maintenance" class="page"><h2>Wartung</h2><div class="card"><button class="action" onclick="post('/api/v1/maintenance/tick',{})">Routing, TTL & Peer-Timeout prüfen</button><button class="action secondary" onclick="post('/api/v1/maintenance/backup',{})">Backup schreiben</button><button class="action secondary" onclick="location.href='/api/v1/export.json'">JSON-Export</button><div class="notice">Shadow berechnet Pfade und Sessions, sendet aber nichts an andere Regionen. Authoritative aktiviert Heartbeats, HTTP-Transport, Retry und Failover.</div></div><div class="card"><pre id="configJson"></pre></div></section>
<section id="api" class="page"><h2>API</h2><div class="card"><p><a href="/openapi.json">OpenAPI JSON</a> · <a href="/metrics">Prometheus Metrics</a> · <a href="/health/ready">Readiness</a></p><pre>Peer-Ingress: POST /api/v1/peer/heartbeat
Peer-Ingress: POST /api/v1/peer/envelopes
Local-Core:   POST /api/v1/transit/submit
Local-Core:   GET  /api/v1/local-deliveries
Protocol:     netcore-transit-v1
ETSI ISI:     noch nicht implementiert</pre></div></section>
<section id="about" class="page"><h2>Über</h2><div class="card"><p><b>NetCore-Tetra Transit</b></p><p>Regionale Teilnehmerauflösung, Gruppenreichweite, Individual-/Gruppenruf-, SDS-, Media-, Mobility- und Supplementary-Service-Transit mit Path Vector, Hop Limit, Deduplizierung, redundanten Pfaden und kontrolliertem Failover.</p><p class="muted">Diese Phase bildet eine DXTT-ähnliche NetCore-Vermittlung. Standardisiertes ISI, Fremd-SwMI-Interoperabilität, ISI-Sicherheitsprofile und produktive mTLS/RBAC folgen später.</p></div></section>
</main></div>
<script>
const pages=[['overview','Übersicht'],['peers','Regionen & Peers'],['routes','Routing'],['locations','Regionenauflösung'],['sessions','Sessions'],['traffic','Traffic & Queues'],['events','Ereignisse'],['maintenance','Wartung'],['api','API'],['about','Über']];
const nav=document.getElementById('nav');pages.forEach(([id,label],i)=>{const b=document.createElement('button');b.textContent=label;b.className=i===0?'active':'';b.onclick=()=>show(id,b);nav.appendChild(b)});
function show(id,b){document.querySelectorAll('.page').forEach(x=>x.classList.remove('active'));document.getElementById(id).classList.add('active');document.querySelectorAll('nav button').forEach(x=>x.classList.remove('active'));b.classList.add('active');refresh()}
function esc(v){return String(v??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]))}
function pill(v){const s=String(v);const cls=/up|active|enabled|delivered|acknowledged|authoritative/i.test(s)?'ok':/down|failed|blocked|expired|cancelled|unroutable/i.test(s)?'danger':/degraded|maintenance|queued|retry|shadow|unknown|pending/i.test(s)?'warn':'';return `<span class="pill ${cls}">${esc(s)}</span>`}
async function api(path,opt={}){const r=await fetch(path,{headers:{'Content-Type':'application/json'},...opt});const t=await r.text();let d={};try{d=t?JSON.parse(t):{}}catch{d={raw:t}}if(!r.ok)throw new Error(d.error||`${r.status} ${r.statusText}`);return d}
async function post(path,data){try{const d=await api(path,{method:'POST',body:JSON.stringify(data)});await refresh();return d}catch(e){alert(e.message);throw e}}
function csv(v){return v.split(',').map(x=>x.trim()).filter(Boolean)}
function metric(label,value,detail=''){return `<div class="card"><div class="muted">${esc(label)}</div><div class="metric">${esc(value)}</div><div class="muted">${esc(detail)}</div></div>`}
async function refresh(){try{const paths=['/api/v1/status','/api/v1/config','/api/v1/peers','/api/v1/routes','/api/v1/locations/subscribers','/api/v1/locations/groups','/api/v1/sessions','/api/v1/outbound?limit=500','/api/v1/local-deliveries?limit=500','/api/v1/events?limit=500'];const [s,c,peers,routes,subs,groups,sessions,outbound,deliveries,events]=await Promise.all(paths.map(api));window.data={s,c,peers,routes,subs,groups,sessions,outbound,deliveries,events};render()}catch(e){console.error(e);statusJson.textContent=e.message}}
function render(){const {s,c,peers,routes,subs,groups,sessions,outbound,deliveries,events}=window.data;metrics.innerHTML=[metric('Region',s.region_id,s.swmi_id),metric('Modus',s.operating_mode,s.authoritative?'Peer-Transport aktiv':'nur Simulation'),metric('Peers UP',s.peers_up,`${s.peers_total} gesamt`),metric('Routen',s.routes_total),metric('Sessions',s.sessions_active),metric('Outbound',s.outbound_pending),metric('Local Delivery',s.local_deliveries_pending),metric('Loops verworfen',s.loop_rejections)].join('');statusJson.textContent=JSON.stringify(s,null,2);configJson.textContent=JSON.stringify(c,null,2);
peerRows.innerHTML=peers.map(p=>`<tr><td><b>${esc(p.display_name)}</b><br><span class="mono">${esc(p.peer_id)} / ${esc(p.region_id)}</span></td><td class="mono">${esc(p.endpoint)}</td><td>${pill(p.admin_state)}</td><td>${pill(p.oper_state)}<br>${esc(p.last_error||'')}</td><td>${p.latency_ms==null?'-':Number(p.latency_ms).toFixed(1)+' ms'}</td><td>${esc(p.protocol_version)}<br>${esc(p.capabilities.join(', '))}</td><td><button class="action secondary" onclick="peerAction('${p.peer_id}','enable')">Enable</button><button class="action secondary" onclick="peerAction('${p.peer_id}','maintenance')">Wartung</button><button class="action danger" onclick="peerAction('${p.peer_id}','block')">Sperren</button></td></tr>`).join('')||'<tr><td colspan="7" class="muted">Keine Peers</td></tr>';
routeRows.innerHTML=routes.map(r=>`<tr><td>${esc(r.service)}</td><td>${esc(r.selector_type)}: ${esc(r.selector_value||'*')}</td><td>${esc(r.destination_region)}</td><td class="mono">${esc(r.peer_id)}</td><td>${r.preference} / ${r.metric}</td><td>${pill(r.enabled?'enabled':'disabled')}<br>${esc(r.failover_group||'')}</td><td><button class="action secondary" onclick="routeAction('${r.route_id}',r.enabled?'disable':'enable')">${r.enabled?'Deaktivieren':'Aktivieren'}</button><button class="action" onclick="routeAction('${r.route_id}','prefer')">Bevorzugen</button><button class="action danger" onclick="deleteRoute('${r.route_id}')">Löschen</button></td></tr>`).join('')||'<tr><td colspan="7" class="muted">Keine Routen</td></tr>';
subscriberRows.innerHTML=subs.map(x=>`<tr><td>${x.issi}</td><td>${esc(x.home_region)}</td><td>${esc(x.current_region)}</td><td>${esc(x.serving_node||'-')}</td><td>${x.sequence}</td><td>${esc(x.updated_at)}</td></tr>`).join('')||'<tr><td colspan="6" class="muted">Keine Teilnehmerregionen</td></tr>';
groupRows.innerHTML=groups.map(x=>`<tr><td>${x.gssi}</td><td>${esc(x.regions.join(', '))}</td><td>${esc(x.source_peer||'local')}</td><td>${esc(x.updated_at)}</td></tr>`).join('')||'<tr><td colspan="4" class="muted">Keine Gruppenreichweiten</td></tr>';
sessionRows.innerHTML=sessions.map(x=>`<tr><td class="mono">${esc(x.session_id.slice(0,14))}</td><td>${esc(x.service)}</td><td>${esc(x.source)} → ${esc(x.destination)}</td><td>${pill(x.state)}<br>${esc(x.last_error||'')}</td><td>${x.legs.map(l=>`${esc(l.target_region)} via ${esc(l.selected_peer||'local')} ${pill(l.state)} F:${l.failover_count}`).join('<br>')}</td><td>${x.envelope_count}</td><td><button class="action secondary" onclick="sessionAction('${x.session_id}','failover')">Failover</button><button class="action danger" onclick="sessionAction('${x.session_id}','close')">Schließen</button></td></tr>`).join('')||'<tr><td colspan="7" class="muted">Keine Sessions</td></tr>';
outboundRows.innerHTML=outbound.map(x=>`<tr><td class="mono">${esc(x.envelope_id.slice(0,14))}</td><td>${esc(x.service)} / ${esc(x.operation)}</td><td>${esc(x.target_region)}</td><td>${esc(x.selected_peer||'-')}</td><td>${pill(x.state)}</td><td>${x.attempts}</td><td>${esc(x.last_error||'')}</td></tr>`).join('')||'<tr><td colspan="7" class="muted">Keine Outbound-Envelopes</td></tr>';
deliveryRows.innerHTML=deliveries.map(x=>`<tr><td class="mono">${esc(x.delivery_id.slice(0,14))}</td><td>${esc(x.service)} / ${esc(x.operation)}</td><td>${esc(x.source_region)}:${esc(x.source)}</td><td>${esc(x.destination)}</td><td>${pill(x.state)}</td><td class="mono">${esc(JSON.stringify(x.payload))}</td><td>${x.state==='pending'?`<button class="action" onclick="ackDelivery('${x.delivery_id}',true)">ACK</button><button class="action danger" onclick="ackDelivery('${x.delivery_id}',false)">NACK</button>`:''}</td></tr>`).join('')||'<tr><td colspan="7" class="muted">Keine lokalen Zustellungen</td></tr>';
eventRows.innerHTML=events.map(x=>`<tr><td>${x.sequence}</td><td>${esc(x.timestamp)}</td><td>${pill(x.severity)}</td><td>${esc(x.category)}</td><td>${esc(x.action)}</td><td>${esc(x.actor)}</td><td class="mono">${esc(x.target)}</td><td class="mono">${esc(JSON.stringify(x.detail))}</td></tr>`).join('')}
async function createPeer(){await post('/api/v1/peers',{peer_id:pId.value,region_id:pRegion.value,swmi_id:pSwmi.value,display_name:pName.value,endpoint:pEndpoint.value,priority:Number(pPriority.value),capabilities:csv(pCaps.value),protocol_version:'netcore-transit-v1',notes:'WebUI'})}
async function peerAction(id,action){await post(`/api/v1/peers/${encodeURIComponent(id)}/${action}`,{actor:'webui-operator',reason:`WebUI ${action}`})}
async function createRoute(){await post('/api/v1/routes',{service:rService.value,selector_type:rType.value,selector_value:rValue.value,destination_region:rRegion.value,peer_id:rPeer.value,preference:Number(rPref.value),metric:Number(rMetric.value),failover_group:rFailover.value||null,enabled:true,notes:'WebUI'})}
async function routeAction(id,action){await post(`/api/v1/routes/${id}/${action}`,{actor:'webui-operator',reason:`WebUI ${action}`})}
async function deleteRoute(id){if(confirm('Route wirklich löschen?')){await api(`/api/v1/routes/${id}`,{method:'DELETE'});await refresh()}}
async function saveSubscriber(){await post('/api/v1/locations/subscribers',{issi:Number(lIssi.value),home_region:lHome.value,current_region:lCurrent.value,serving_node:lNode.value||null})}
async function saveGroup(){await post('/api/v1/locations/groups',{gssi:Number(lGssi.value),regions:csv(lRegions.value),source_peer:null})}
async function submitTransit(){let payload;try{payload=JSON.parse(tPayload.value)}catch(e){alert('Payload JSON ungültig: '+e.message);return}await post('/api/v1/transit/submit',{service:tService.value,operation:tOperation.value,source_kind:'issi',source:tSource.value,destination_kind:tKind.value,destination:tDestination.value,target_region:tRegion.value||null,priority:Number(tPriority.value),payload})}
async function sessionAction(id,action){await post(`/api/v1/sessions/${id}/${action}`,{actor:'webui-operator',reason:`WebUI ${action}`})}
async function ackDelivery(id,success){await post(`/api/v1/local-deliveries/${id}/ack`,{success,error:success?null:'WebUI NACK',actor:'webui-operator'})}
refresh();setInterval(refresh,5000);
</script>
</body></html>"#;

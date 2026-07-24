use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

use serde::Serialize;
use serde_json::json;

use crate::config::{OPERATING_MODE_AUTHORITATIVE, SecurityCoreConfig};
use crate::protocol::{
    AlarmAckInput, AuthenticationResponseInput, AuthenticationStartInput, DisableInput,
    EdgeActionAckInput, EdgeClaimInput, PolicyInput, ProfileInput, RevokeInput,
};
use crate::state::SharedSecurityCore;

pub fn spawn_http_server(
    config: SecurityCoreConfig,
    core: SharedSecurityCore,
) -> std::io::Result<thread::JoinHandle<()>> {
    let listener = TcpListener::bind(config.server.bind)?;
    tracing::info!(
        "Security Core WebUI/API listening on http://{}",
        config.server.bind
    );
    Ok(thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let core = core.clone();
                    let config = config.clone();
                    thread::spawn(move || {
                        if let Err(error) = handle_connection(stream, core, config) {
                            tracing::warn!("Security Core HTTP connection failed: {}", error);
                        }
                    });
                }
                Err(error) => tracing::warn!("Security Core HTTP accept failed: {}", error),
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
    core: SharedSecurityCore,
    config: SecurityCoreConfig,
) -> Result<(), String> {
    let request = read_request(&mut stream, config.limits.max_body_bytes)?;
    let response = route(request, core, config);
    write_response(&mut stream, response).map_err(|error| error.to_string())
}

fn route(
    request: HttpRequest,
    core: SharedSecurityCore,
    config: SecurityCoreConfig,
) -> HttpResponse {
    if request.method == "OPTIONS" {
        return empty(204);
    }

    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") => html(INDEX_HTML),
        ("GET", "/health/live") => json_response(200, &json!({"status":"live"})),
        ("GET", "/health/ready") => {
            let status = core.status();
            let ready = status.operating_mode != OPERATING_MODE_AUTHORITATIVE
                || !config.node_gateway.observe_nodes
                || status.node_gateway_connected;
            json_response(if ready { 200 } else { 503 }, &status)
        }
        ("GET", "/api/v1/status") => json_response(200, &core.status()),
        ("GET", "/api/v1/config") => json_response(200, &core.redacted_config()),
        ("GET", "/api/v1/policy") => json_response(200, &core.policy()),
        ("POST", "/api/v1/policy") => match parse_json::<PolicyInput>(&request.body)
            .and_then(|input| core.update_policy(input, "open-lab-api"))
        {
            Ok(policy) => json_response(200, &policy),
            Err(error) => json_response(409, &json!({"error":error})),
        },
        ("GET", "/api/v1/profiles") => json_response(200, &core.profiles()),
        ("POST", "/api/v1/profiles") => match parse_json::<ProfileInput>(&request.body)
            .and_then(|input| core.upsert_profile(input, "open-lab-api"))
        {
            Ok(profile) => json_response(201, &profile),
            Err(error) => json_response(409, &json!({"error":error})),
        },
        ("GET", "/api/v1/subscribers") => json_response(200, &core.subscribers()),
        ("GET", "/api/v1/auth-contexts") => json_response(200, &core.auth_contexts()),
        ("POST", "/api/v1/auth/start") => {
            match parse_json::<AuthenticationStartInput>(&request.body)
                .and_then(|input| core.start_authentication(input))
            {
                Ok(context) => json_response(202, &context),
                Err(error) => json_response(409, &json!({"error":error})),
            }
        }
        ("GET", "/api/v1/dck-contexts") => json_response(200, &core.dck_contexts()),
        ("GET", "/api/v1/actions") => json_response(200, &core.actions()),
        ("POST", "/api/v1/edge/actions/claim") => {
            match parse_json::<EdgeClaimInput>(&request.body)
                .and_then(|input| core.claim_edge_actions(input))
            {
                Ok(actions) => json_response(200, &json!({
                    "protocol_version":crate::protocol::EDGE_PROTOCOL_VERSION,
                    "actions":actions,
                    "warning":"This edge-only endpoint may return ephemeral challenge or DCK material. Do not expose it to operator browsers."
                })),
                Err(error) => json_response(409, &json!({"error":error})),
            }
        }
        ("GET", "/api/v1/alarms") => json_response(200, &core.alarms()),
        ("GET", "/api/v1/nodes") => json_response(200, &core.nodes()),
        ("GET", "/api/v1/audit") => {
            let limit = query_usize(&request, "limit", 500, 10_000);
            json_response(200, &core.audit(limit))
        }
        ("POST", "/api/v1/maintenance/expire") => match core.maintenance_tick() {
            Ok(status) => json_response(200, &status),
            Err(error) => json_response(500, &json!({"error":error})),
        },
        ("POST", "/api/v1/maintenance/backup") => match core.backup() {
            Ok(path) => json_response(200, &json!({"backup_path":path})),
            Err(error) => json_response(500, &json!({"error":error})),
        },
        ("GET", "/api/v1/export.json") => {
            download_json("netcore-security-core-export.json", &core.export())
        }
        ("GET", "/metrics") => text(
            "text/plain; version=0.0.4; charset=utf-8",
            core.metrics(),
        ),
        ("GET", "/openapi.json") => json_response(200, &openapi()),
        _ => dynamic_route(request, core),
    }
}

fn dynamic_route(request: HttpRequest, core: SharedSecurityCore) -> HttpResponse {
    let parts: Vec<&str> = request.path.trim_matches('/').split('/').collect();
    match (request.method.as_str(), parts.as_slice()) {
        ("GET", ["api", "v1", "profiles", issi]) => match parse_issi(issi)
            .ok()
            .and_then(|value| core.profile(value))
        {
            Some(profile) => json_response(200, &profile),
            None => json_response(404, &json!({"error":"profile not found"})),
        },
        ("DELETE", ["api", "v1", "profiles", issi]) => match parse_issi(issi)
            .and_then(|value| core.delete_profile(value, "open-lab-api"))
        {
            Ok(()) => empty(204),
            Err(error) => json_response(404, &json!({"error":error})),
        },
        ("POST", ["api", "v1", "profiles", issi, "disable"]) => {
            profile_disable_route(&request.body, &core, issi, true)
        }
        ("POST", ["api", "v1", "profiles", issi, "enable"]) => {
            profile_disable_route(&request.body, &core, issi, false)
        }
        ("GET", ["api", "v1", "auth-contexts", id]) => match core.auth_context(id) {
            Some(context) => json_response(200, &context),
            None => json_response(404, &json!({"error":"authentication context not found"})),
        },
        ("POST", ["api", "v1", "auth-contexts", id, "response"]) => {
            match parse_json::<AuthenticationResponseInput>(&request.body)
                .and_then(|input| core.submit_authentication_response(id, input))
            {
                Ok(context) => json_response(200, &context),
                Err(error) => json_response(409, &json!({"error":error})),
            }
        }
        ("POST", ["api", "v1", "auth-contexts", id, "revoke"]) => {
            match parse_json_or_default::<RevokeInput>(&request.body)
                .and_then(|input| core.revoke_authentication(id, input))
            {
                Ok(context) => json_response(200, &context),
                Err(error) => json_response(409, &json!({"error":error})),
            }
        }
        ("POST", ["api", "v1", "dck-contexts", id, "revoke"]) => {
            match parse_json_or_default::<RevokeInput>(&request.body)
                .and_then(|input| core.revoke_dck(id, input))
            {
                Ok(context) => json_response(200, &context),
                Err(error) => json_response(409, &json!({"error":error})),
            }
        }
        ("POST", ["api", "v1", "edge", "actions", id, "ack"]) => {
            match parse_json_or_default::<EdgeActionAckInput>(&request.body)
                .and_then(|input| core.acknowledge_edge_action(id, input))
            {
                Ok(action) => json_response(200, &action),
                Err(error) => json_response(409, &json!({"error":error})),
            }
        }
        ("POST", ["api", "v1", "alarms", id, "ack"]) => {
            match parse_json_or_default::<AlarmAckInput>(&request.body)
                .and_then(|input| core.acknowledge_alarm(id, input))
            {
                Ok(alarm) => json_response(200, &alarm),
                Err(error) => json_response(409, &json!({"error":error})),
            }
        }
        _ => json_response(404, &json!({"error":"not found"})),
    }
}

fn profile_disable_route(
    body: &[u8],
    core: &SharedSecurityCore,
    issi: &str,
    disabled: bool,
) -> HttpResponse {
    match parse_issi(issi)
        .and_then(|value| parse_json_or_default::<DisableInput>(body).map(|input| (value, input)))
        .and_then(|(value, input)| core.set_disabled(value, disabled, input))
    {
        Ok(profile) => json_response(200, &profile),
        Err(error) => json_response(409, &json!({"error":error})),
    }
}

fn parse_issi(value: &str) -> Result<u32, String> {
    let issi = value
        .parse::<u32>()
        .map_err(|error| format!("invalid ISSI: {error}"))?;
    if issi > 0x00ff_ffff {
        return Err("ISSI must fit into 24 bits".to_string());
    }
    Ok(issi)
}

fn parse_json<T: serde::de::DeserializeOwned>(body: &[u8]) -> Result<T, String> {
    serde_json::from_slice(body).map_err(|error| format!("invalid JSON: {error}"))
}

fn parse_json_or_default<T>(body: &[u8]) -> Result<T, String>
where
    T: serde::de::DeserializeOwned + Default,
{
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
            "title":"NetCore Security Core",
            "version":env!("CARGO_PKG_VERSION"),
            "description":"OPEN LAB authentication orchestration, security policy, disable/enable, DCK metadata and security audit API. The lab_hmac_sha256 provider is not a TETRA TA algorithm. Raw secrets are absent from normal management responses."
        },
        "paths":{
            "/api/v1/status":{"get":{}},
            "/api/v1/config":{"get":{}},
            "/api/v1/policy":{"get":{},"post":{}},
            "/api/v1/profiles":{"get":{},"post":{}},
            "/api/v1/profiles/{issi}":{"get":{},"delete":{}},
            "/api/v1/profiles/{issi}/disable":{"post":{}},
            "/api/v1/profiles/{issi}/enable":{"post":{}},
            "/api/v1/subscribers":{"get":{}},
            "/api/v1/auth/start":{"post":{}},
            "/api/v1/auth-contexts":{"get":{}},
            "/api/v1/auth-contexts/{id}":{"get":{}},
            "/api/v1/auth-contexts/{id}/response":{"post":{}},
            "/api/v1/auth-contexts/{id}/revoke":{"post":{}},
            "/api/v1/dck-contexts":{"get":{}},
            "/api/v1/dck-contexts/{id}/revoke":{"post":{}},
            "/api/v1/actions":{"get":{}},
            "/api/v1/edge/actions/claim":{"post":{}},
            "/api/v1/edge/actions/{id}/ack":{"post":{}},
            "/api/v1/alarms":{"get":{}},
            "/api/v1/alarms/{id}/ack":{"post":{}},
            "/api/v1/audit":{"get":{}},
            "/api/v1/nodes":{"get":{}},
            "/api/v1/maintenance/expire":{"post":{}},
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
    let request_line = lines.next().ok_or_else(|| "missing request line".to_string())?;
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
    haystack.windows(needle.len()).position(|window| window == needle)
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
            "Access-Control-Allow-Headers: Content-Type\r\n",
            "X-NetCore-Security-Mode: open-lab\r\n",
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
<title>NetCore Security Core</title>
<style>
:root{font-family:Inter,system-ui,sans-serif;color:#e8edf7;background:#0b1020;--card:#121a2e;--muted:#91a0ba;--accent:#6aa7ff;--danger:#ff6b7a;--ok:#48d597;--warn:#ffcc66;--line:#27324b}
*{box-sizing:border-box}body{margin:0}.banner{background:#7c2d12;color:#fff;padding:10px 18px;font-weight:800;text-align:center}.layout{display:grid;grid-template-columns:240px 1fr;min-height:calc(100vh - 44px)}aside{background:#0d1425;border-right:1px solid var(--line);padding:22px 14px;position:sticky;top:0;height:calc(100vh - 44px)}h1{font-size:20px;margin:0 0 4px}.sub{color:var(--muted);font-size:12px;margin-bottom:20px}nav button{width:100%;display:block;text-align:left;background:transparent;border:0;color:#dbe5f6;padding:11px 12px;border-radius:9px;margin:3px 0;cursor:pointer}nav button.active,nav button:hover{background:#1a2742;color:#fff}main{padding:24px;min-width:0}.page{display:none}.page.active{display:block}.grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(190px,1fr));gap:14px}.card{background:var(--card);border:1px solid var(--line);border-radius:14px;padding:16px;box-shadow:0 8px 28px #0003}.metric{font-size:30px;font-weight:800;margin-top:6px}.muted{color:var(--muted)}.ok{color:var(--ok)}.danger{color:var(--danger)}.warn{color:var(--warn)}table{width:100%;border-collapse:collapse;font-size:13px}th,td{padding:10px 8px;border-bottom:1px solid var(--line);vertical-align:top;text-align:left}th{color:#b8c5db;font-weight:700;position:sticky;top:0;background:var(--card)}.scroll{overflow:auto;max-height:65vh}.toolbar{display:flex;gap:8px;flex-wrap:wrap;margin:12px 0}button.action{background:#1e66d0;color:white;border:0;border-radius:8px;padding:9px 12px;cursor:pointer}button.danger{background:#b42335;color:white}button.secondary{background:#263553;color:white}.pill{display:inline-block;border-radius:999px;padding:3px 8px;background:#263553;font-size:11px}.pill.ok{background:#123f31}.pill.warn{background:#4a3814}.pill.danger{background:#4b1921}input,select,textarea{width:100%;background:#0b1324;color:#eef4ff;border:1px solid #34415d;border-radius:8px;padding:9px}label{display:block;font-size:12px;color:#b8c5db;margin:8px 0 4px}.formgrid{display:grid;grid-template-columns:repeat(auto-fit,minmax(170px,1fr));gap:10px}.mono{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;word-break:break-all}pre{background:#080d18;padding:14px;border-radius:10px;overflow:auto;white-space:pre-wrap}.notice{border-left:4px solid var(--warn);background:#352b16;padding:12px;border-radius:8px;margin:12px 0}.secret{border-left-color:var(--danger);background:#32181e}@media(max-width:850px){.layout{grid-template-columns:1fr}aside{position:static;height:auto}nav{display:flex;overflow:auto}nav button{min-width:max-content}main{padding:14px}}
</style>
</head>
<body>
<div class="banner">OPEN LAB – keine Anmeldung, keine Tokens, kein TLS. Nur im isolierten Managementnetz betreiben.</div>
<div class="layout">
<aside><h1>Security Core</h1><div class="sub">Authentication • Policy • DCK • Audit</div><nav id="nav"></nav></aside>
<main>
<section id="overview" class="page active"><h2>Übersicht</h2><div class="grid" id="metrics"></div><div class="notice secret"><b>Kein Schlüssel-Schaufenster:</b> Die normale WebUI und Management-API zeigen weder Lab-Seed, Challenge-Rohwerte noch DCK-Material. Ephemeres Material läuft ausschließlich über den Edge-Claim-Endpunkt.</div><div class="card"><h3>Aktive Alarme</h3><div class="scroll"><table><thead><tr><th>Schwere</th><th>Art</th><th>Ziel</th><th>Meldung</th><th>Zeit</th></tr></thead><tbody id="overviewAlarms"></tbody></table></div></div></section>
<section id="profiles" class="page"><h2>Security-Profile</h2><div class="card"><div class="formgrid"><div><label>ISSI</label><input id="pIssi" type="number" min="0" max="16777215"></div><div><label>Anzeigename</label><input id="pName"></div><div><label>Authentisierung</label><select id="pAuth"><option value="true">erforderlich</option><option value="false">optional</option></select></div><div><label>Mindestklasse</label><select id="pMin"><option>1</option><option>2</option><option>3</option></select></div><div><label>Bevorzugte Klasse</label><select id="pPref"><option>1</option><option>2</option><option>3</option></select></div><div><label>Max. Fehlversuche</label><input id="pFailures" type="number" min="1" value="3"></div><div><label>Erlaubte Nodes (Komma)</label><input id="pNodes"></div><div><label>Notiz</label><input id="pNotes"></div></div><div class="toolbar"><button class="action" onclick="saveProfile()">Profil speichern</button></div></div><div class="card"><div class="scroll"><table><thead><tr><th>ISSI</th><th>Name</th><th>Auth</th><th>Klassen</th><th>Nodes</th><th>Status</th><th>Aktion</th></tr></thead><tbody id="profileRows"></tbody></table></div></div></section>
<section id="authentication" class="page"><h2>Authentisierung</h2><div class="card"><div class="formgrid"><div><label>Node-ID</label><input id="aNode" value="tbs-lab-01"></div><div><label>ISSI</label><input id="aIssi" type="number" value="4010001"></div><div><label>Angeforderte Klasse</label><select id="aClass"><option>1</option><option>2</option><option>3</option></select></div><div><label>Unterstützte Klassen</label><input id="aSupported" value="1,3"></div><div><label>Equipment-ID/TEI</label><input id="aEquipment"></div></div><div class="toolbar"><button class="action" onclick="startAuth()">Challenge starten</button></div><div class="notice">Die WebUI zeigt absichtlich keine Challenge. Der TBS-Adapter holt sie über <span class="mono">POST /api/v1/edge/actions/claim</span>.</div></div><div class="card"><div class="scroll"><table><thead><tr><th>ID</th><th>ISSI/Node</th><th>Klasse</th><th>Status</th><th>Versuche</th><th>Fingerprints</th><th>Aktion</th></tr></thead><tbody id="authRows"></tbody></table></div></div></section>
<section id="dck" class="page"><h2>DCK-Kontexte</h2><div class="notice secret">Nur Referenz, Fingerprint, Klasse und Lebenszyklus werden angezeigt. DCK-Rohmaterial bleibt ephemer im Prozess und wird nach Neustart verworfen.</div><div class="card"><div class="scroll"><table><thead><tr><th>Referenz</th><th>ISSI/Node</th><th>Klasse</th><th>Status</th><th>Fingerprint</th><th>Ablauf</th><th>Aktion</th></tr></thead><tbody id="dckRows"></tbody></table></div></div></section>
<section id="actions" class="page"><h2>Edge-Aktionen</h2><div class="card"><div class="scroll"><table><thead><tr><th>Seq</th><th>Node</th><th>Art</th><th>Ziel</th><th>Status</th><th>Secret?</th><th>Fehler</th></tr></thead><tbody id="actionRows"></tbody></table></div></div></section>
<section id="alarms" class="page"><h2>Alarme</h2><div class="card"><div class="scroll"><table><thead><tr><th>Schwere</th><th>Art</th><th>Ziel</th><th>Status</th><th>Meldung</th><th>Aktion</th></tr></thead><tbody id="alarmRows"></tbody></table></div></div></section>
<section id="policy" class="page"><h2>Policy</h2><div class="card"><div class="formgrid"><div><label>Betriebsmodus</label><select id="polMode"><option value="shadow">shadow</option><option value="authoritative">authoritative</option></select></div><div><label>Standardklasse</label><select id="polDefault"><option>1</option><option>2</option><option>3</option></select></div><div><label>Mindestklasse</label><select id="polMin"><option>1</option><option>2</option><option>3</option></select></div><div><label>Auth standardmäßig</label><select id="polAuth"><option value="true">ja</option><option value="false">nein</option></select></div><div><label>Class-1-Fallback</label><select id="polFallback"><option value="true">erlaubt</option><option value="false">verboten</option></select></div><div><label>Unbekannte Teilnehmer</label><select id="polUnknown"><option value="false">beobachten/zulassen</option><option value="true">abweisen</option></select></div><div><label>Nach Fehlern deaktivieren</label><select id="polDisable"><option value="false">nein</option><option value="true">ja</option></select></div></div><div class="toolbar"><button class="action" onclick="savePolicy()">Policy speichern</button></div></div><div class="card"><pre id="policyJson"></pre></div></section>
<section id="dependencies" class="page"><h2>Zustand & Abhängigkeiten</h2><div class="card"><pre id="statusJson"></pre></div><div class="card"><h3>Nodes</h3><div class="scroll"><table><thead><tr><th>Node</th><th>Station</th><th>Netz</th><th>Zustand</th><th>Zuletzt</th><th>Fehler</th></tr></thead><tbody id="nodeRows"></tbody></table></div></div></section>
<section id="audit" class="page"><h2>Audit</h2><div class="card"><div class="scroll"><table><thead><tr><th>Seq</th><th>Zeit</th><th>Akteur</th><th>Aktion</th><th>Ziel</th><th>Ergebnis</th><th>Details</th></tr></thead><tbody id="auditRows"></tbody></table></div></div></section>
<section id="maintenance" class="page"><h2>Wartung</h2><div class="card"><div class="toolbar"><button class="action" onclick="post('/api/v1/maintenance/expire',{})">Abläufe jetzt prüfen</button><button class="action secondary" onclick="post('/api/v1/maintenance/backup',{})">Backup erzeugen</button><button class="action secondary" onclick="location.href='/api/v1/export.json'">Redacted Export</button></div><div class="notice">Ein Neustart verwirft absichtlich Challenge-, Verifier- und DCK-Rohmaterial. Offene Authentisierungen werden danach als abgelaufen, aktive DCKs als widerrufen markiert.</div></div></section>
<section id="api" class="page"><h2>API</h2><div class="card"><p><a href="/openapi.json" style="color:var(--accent)">OpenAPI JSON</a> · <a href="/metrics" style="color:var(--accent)">Prometheus Metrics</a> · <a href="/health/ready" style="color:var(--accent)">Readiness</a></p><pre>Management: /api/v1/*\nEdge-only secret transport: POST /api/v1/edge/actions/claim\nProtocol: netcore-security-edge-v1</pre></div></section>
<section id="about" class="page"><h2>Über</h2><div class="card"><p><b>NetCore-Tetra Security Core</b></p><p>Authentication Centre, Security Policies, DCK-Lifecycle, Disable/Enable, Alarme und Audit.</p><p class="muted">Der eingebaute lab_hmac_sha256-Provider dient nur zur Ende-zu-Ende-Integration. Er ist weder TA11/TA12 noch ein Ersatz für die nächste KMF-Phase.</p></div></section>
</main></div>
<script>
const pages=[['overview','Übersicht'],['profiles','Profile'],['authentication','Authentisierung'],['dck','DCK'],['actions','Edge-Aktionen'],['alarms','Alarme'],['policy','Policy'],['dependencies','Abhängigkeiten'],['audit','Audit'],['maintenance','Wartung'],['api','API'],['about','Über']];
const nav=document.getElementById('nav');pages.forEach(([id,label],i)=>{const b=document.createElement('button');b.textContent=label;b.className=i===0?'active':'';b.onclick=()=>show(id,b);nav.appendChild(b)});
function show(id,b){document.querySelectorAll('.page').forEach(x=>x.classList.remove('active'));document.getElementById(id).classList.add('active');document.querySelectorAll('nav button').forEach(x=>x.classList.remove('active'));b.classList.add('active');refresh()}
function esc(v){return String(v??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]))}
function pill(v){const s=String(v);const cls=/active|authenticated|applied|connected|success/i.test(s)?'ok':/failed|rejected|revoked|disabled|critical/i.test(s)?'danger':/pending|warning|in_flight|awaiting/i.test(s)?'warn':'';return `<span class="pill ${cls}">${esc(s)}</span>`}
async function api(path,opt={}){const r=await fetch(path,{headers:{'Content-Type':'application/json'},...opt});const t=await r.text();let d={};try{d=t?JSON.parse(t):{}}catch{d={raw:t}}if(!r.ok)throw new Error(d.error||`${r.status} ${r.statusText}`);return d}
async function post(path,data){try{const d=await api(path,{method:'POST',body:JSON.stringify(data)});await refresh();return d}catch(e){alert(e.message);throw e}}
async function del(path){if(!confirm('Wirklich löschen?'))return;try{await api(path,{method:'DELETE'});await refresh()}catch(e){alert(e.message)}}
function metric(label,value,detail=''){return `<div class="card"><div class="muted">${esc(label)}</div><div class="metric">${esc(value)}</div><div class="muted">${esc(detail)}</div></div>`}
async function refresh(){try{const [s,p,subs,auth,dcks,actions,alarms,nodes,audit]=await Promise.all(['/api/v1/status','/api/v1/profiles','/api/v1/subscribers','/api/v1/auth-contexts','/api/v1/dck-contexts','/api/v1/actions','/api/v1/alarms','/api/v1/nodes','/api/v1/audit?limit=500'].map(x=>api(x)));window.data={s,p,subs,auth,dcks,actions,alarms,nodes,audit};render();await loadPolicy()}catch(e){console.error(e);document.getElementById('statusJson').textContent=e.message}}
function render(){const {s,p,auth,dcks,actions,alarms,nodes,audit}=window.data;document.getElementById('metrics').innerHTML=[metric('Betriebsmodus',s.operating_mode,s.authoritative?'verbindlich':'beobachtend'),metric('Profile',s.profiles),metric('Aktive Auth-Kontexte',s.active_auth_contexts),metric('Aktive DCK',s.active_dck_contexts),metric('Offene Alarme',s.open_alarms),metric('Node Gateway',s.node_gateway_connected?'verbunden':'getrennt',s.node_gateway_last_error||'')].join('');
const open=alarms.filter(x=>x.state==='open');document.getElementById('overviewAlarms').innerHTML=open.map(x=>`<tr><td>${pill(x.severity)}</td><td>${esc(x.kind)}</td><td>${esc(x.issi??x.node_id??'-')}</td><td>${esc(x.message)}</td><td>${esc(x.created_at)}</td></tr>`).join('')||'<tr><td colspan="5" class="muted">Keine offenen Alarme</td></tr>';
document.getElementById('profileRows').innerHTML=p.map(x=>`<tr><td class="mono">${x.issi}</td><td>${esc(x.display_name)}</td><td>${x.authentication_required?'ja':'optional'}</td><td>min ${x.minimum_security_class} / pref ${x.preferred_security_class}</td><td>${esc(x.allowed_nodes.join(', ')||'alle')}</td><td>${x.disabled||x.equipment_disabled?pill('disabled'):pill('enabled')}</td><td><button class="action secondary" onclick="toggleDisable(${x.issi},${!x.disabled})">${x.disabled?'Freigeben':'Sperren'}</button> <button class="action danger" onclick="del('/api/v1/profiles/${x.issi}')">Löschen</button></td></tr>`).join('');
document.getElementById('authRows').innerHTML=auth.map(x=>`<tr><td class="mono">${esc(x.id.slice(0,12))}</td><td>${x.issi}<br>${esc(x.node_id)}</td><td>${x.requested_security_class} → ${x.negotiated_security_class}</td><td>${pill(x.state)}</td><td>${x.attempts}/${x.max_attempts}</td><td class="mono">C ${esc(x.challenge_fingerprint)}<br>R ${esc(x.response_fingerprint||'-')}</td><td><button class="action danger" onclick="post('/api/v1/auth-contexts/${x.id}/revoke',{reason:'WebUI revoke'})">Widerrufen</button></td></tr>`).join('');
document.getElementById('dckRows').innerHTML=dcks.map(x=>`<tr><td class="mono">${esc(x.key_reference)}</td><td>${x.issi}<br>${esc(x.node_id)}</td><td>${x.security_class}</td><td>${pill(x.state)}</td><td class="mono">${esc(x.key_fingerprint)}</td><td>${esc(x.expires_at)}</td><td><button class="action danger" onclick="post('/api/v1/dck-contexts/${x.id}/revoke',{reason:'WebUI revoke'})">Widerrufen</button></td></tr>`).join('');
document.getElementById('actionRows').innerHTML=actions.map(x=>`<tr><td>${x.sequence}</td><td>${esc(x.node_id)}</td><td>${esc(x.kind)}</td><td>${esc(x.issi??'-')}<br><span class="mono">${esc((x.context_id||'').slice(0,12))}</span></td><td>${pill(x.state)}</td><td>${x.secret_bearing?'ja':'nein'}</td><td>${esc(x.last_error||'')}</td></tr>`).join('');
document.getElementById('alarmRows').innerHTML=alarms.map(x=>`<tr><td>${pill(x.severity)}</td><td>${esc(x.kind)}</td><td>${esc(x.issi??x.node_id??'-')}</td><td>${pill(x.state)}</td><td>${esc(x.message)}</td><td>${x.state==='open'?`<button class="action" onclick="post('/api/v1/alarms/${x.id}/ack',{note:'WebUI'})">Quittieren</button>`:''}</td></tr>`).join('');
document.getElementById('nodeRows').innerHTML=nodes.map(x=>`<tr><td class="mono">${esc(x.node_id)}</td><td>${esc(x.station_name)}</td><td>${esc(x.mcc??'-')}/${esc(x.mnc??'-')} LA ${esc(x.location_area??'-')}</td><td>${pill(x.connected?'connected':x.stale?'stale':'disconnected')}</td><td>${esc(x.last_seen)}</td><td>${esc(x.last_error||'')}</td></tr>`).join('');
document.getElementById('auditRows').innerHTML=audit.map(x=>`<tr><td>${x.sequence}</td><td>${esc(x.timestamp)}</td><td>${esc(x.actor)}</td><td>${esc(x.action)}</td><td class="mono">${esc(x.target)}</td><td>${pill(x.outcome)}</td><td class="mono">${esc(JSON.stringify(x.detail))}</td></tr>`).join('');document.getElementById('statusJson').textContent=JSON.stringify(s,null,2)}
async function loadPolicy(){const p=await api('/api/v1/policy');document.getElementById('policyJson').textContent=JSON.stringify(p,null,2);polMode.value=p.operating_mode;polDefault.value=p.default_security_class;polMin.value=p.minimum_security_class;polAuth.value=String(p.authentication_required);polFallback.value=String(p.allow_class1_fallback);polUnknown.value=String(p.reject_unknown_subscribers);polDisable.value=String(p.disable_after_failures)}
async function saveProfile(){await post('/api/v1/profiles',{issi:Number(pIssi.value),display_name:pName.value,authentication_required:pAuth.value==='true',minimum_security_class:Number(pMin.value),preferred_security_class:Number(pPref.value),allow_class1_fallback:true,allowed_nodes:pNodes.value.split(',').map(x=>x.trim()).filter(Boolean),max_failures:Number(pFailures.value),notes:pNotes.value})}
async function toggleDisable(issi,disabled){await post(`/api/v1/profiles/${issi}/${disabled?'disable':'enable'}`,{reason:'WebUI action'})}
async function startAuth(){await post('/api/v1/auth/start',{node_id:aNode.value,issi:Number(aIssi.value),requested_security_class:Number(aClass.value),supported_security_classes:aSupported.value.split(',').map(Number).filter(Boolean),equipment_id:aEquipment.value||null,source:'webui-simulator'})}
async function savePolicy(){await post('/api/v1/policy',{operating_mode:polMode.value,default_security_class:Number(polDefault.value),minimum_security_class:Number(polMin.value),authentication_required:polAuth.value==='true',allow_class1_fallback:polFallback.value==='true',reject_unknown_subscribers:polUnknown.value==='true',disable_after_failures:polDisable.value==='true'})}
refresh();setInterval(refresh,5000);
</script>
</body></html>"#;

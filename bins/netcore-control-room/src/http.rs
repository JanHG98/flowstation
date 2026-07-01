use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};
use tetra_entities::net_control::ControlCommand;
use tetra_entities::net_control_room::ControlCommandEnvelope;

use crate::auth::{AuthError, AuthIdentity, AuthRole, AuthState, AuthTokenListResponse, CreateAuthTokenRequest, UpdateAuthTokenRequest};
use crate::state::{SharedControlRoom, now_iso};

const MAX_HTTP_REQUEST_BYTES: usize = 1024 * 1024;

#[derive(Debug)]
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub query: HashMap<String, String>,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

#[derive(Debug)]
pub struct HttpResponse {
    pub status: u16,
    pub reason: &'static str,
    pub content_type: &'static str,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn json<T: serde::Serialize>(status: u16, value: &T) -> Self {
        let reason = reason_phrase(status);
        let body = serde_json::to_vec_pretty(value).unwrap_or_else(|_| b"{\"error\":\"json serialisation failed\"}".to_vec());
        Self {
            status,
            reason,
            content_type: "application/json; charset=utf-8",
            body,
        }
    }

    pub fn text(status: u16, text: impl Into<String>) -> Self {
        Self {
            status,
            reason: reason_phrase(status),
            content_type: "text/plain; charset=utf-8",
            body: text.into().into_bytes(),
        }
    }

    pub fn html(status: u16, html: impl Into<String>) -> Self {
        Self {
            status,
            reason: reason_phrase(status),
            content_type: "text/html; charset=utf-8",
            body: html.into().into_bytes(),
        }
    }
}

pub fn handle_http_stream(mut stream: TcpStream, state: SharedControlRoom, node_path: &str, ui_path: &str, auth: AuthState) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let request = match read_http_request(&mut stream) {
        Ok(req) => req,
        Err(err) => {
            let _ = write_response(&mut stream, &HttpResponse::text(400, format!("bad request: {}\n", err)));
            return;
        }
    };

    tracing::debug!(method = %request.method, path = %request.path, "http request");
    let response = route_http(request, state, node_path, ui_path, &auth);
    let _ = write_response(&mut stream, &response);
}

fn route_http(request: HttpRequest, state: SharedControlRoom, node_path: &str, ui_path: &str, auth: &AuthState) -> HttpResponse {
    if request.method == "OPTIONS" {
        return HttpResponse::text(204, "");
    }

    let health_public = request.method == "GET" && request.path == "/health" && auth.allow_health_unauthenticated();
    let identity = if health_public {
        None
    } else {
        match auth.authorize_http_role(&request.headers, &request.query, required_role_for_request(&request)) {
            Ok(identity) => Some(identity),
            Err(err) => {
                tracing::warn!(method = %request.method, path = %request.path, required = %required_role_for_request(&request), "http request rejected by RBAC");
                return auth_error_response(err, required_role_for_request(&request));
            }
        }
    };

    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") => HttpResponse::html(200, index_html(node_path, ui_path)),
        ("GET", "/health") => HttpResponse::json(200, &json!({
            "ok": true,
            "service": "netcore-control-room",
            "timestamp": now_iso(),
        })),
        ("GET", "/api/overview") => HttpResponse::json(200, &state.overview()),
        ("GET", "/api/state") => HttpResponse::json(200, &state.snapshot()),
        ("GET", "/api/rf") => HttpResponse::json(200, &state.rf_snapshot()),
        ("GET", "/api/health/full") => HttpResponse::json(200, &state.health_snapshot()),
        ("GET", "/api/subscribers") => {
            let online_only = query_bool(&request, "online", false) || query_bool(&request, "online_only", false);
            HttpResponse::json(200, &state.subscribers_snapshot(None, online_only).expect("global subscribers snapshot exists"))
        }
        ("GET", "/api/groups") => HttpResponse::json(200, &state.groups_snapshot(None).expect("global groups snapshot exists")),
        ("GET", "/api/calls") => HttpResponse::json(200, &state.calls_snapshot(None).expect("global calls snapshot exists")),
        ("GET", "/api/sds") => {
            let limit = query_usize(&request, "limit", 100, 500);
            HttpResponse::json(200, &state.sds_snapshot(None, limit).expect("global sds snapshot exists"))
        }
        ("GET", "/api/emergencies") => {
            let active_only = query_bool(&request, "active", false) || query_bool(&request, "active_only", false);
            HttpResponse::json(200, &state.emergencies_snapshot(None, active_only).expect("global emergencies snapshot exists"))
        }
        ("GET", "/api/locations") => HttpResponse::json(200, &state.locations_snapshot(None).expect("global locations snapshot exists")),
        ("GET", "/api/nodes") => {
            let snapshot = state.snapshot();
            HttpResponse::json(200, &snapshot.nodes)
        }
        ("GET", "/api/commands") => HttpResponse::json(200, &state.recent_commands(query_usize(&request, "limit", 100, 1000))),
        ("GET", "/api/events") => {
            let limit = query_usize(&request, "limit", 200, 1000);
            let quiet = query_bool(&request, "quiet", false);
            let event_type = request
                .query
                .get("event_type")
                .or_else(|| request.query.get("type"))
                .map(String::as_str);
            HttpResponse::json(200, &state.recent_events_filtered(limit, event_type, quiet))
        }
        ("GET", "/api/admin/tokens") => admin_list_tokens(auth),
        ("POST", "/api/admin/tokens") => admin_create_token(&request.body, auth, identity.as_ref()),
        ("POST", "/api/commands") => submit_command_from_body(&request.body, state, None),
        _ if request.path.starts_with("/api/admin/tokens/") => {
            match parse_admin_token_route(&request.path) {
                Some(token_id) if request.method == "PATCH" || request.method == "POST" => admin_update_token(&token_id, &request.body, auth),
                Some(token_id) if request.method == "DELETE" => admin_delete_token(&token_id, auth),
                _ => HttpResponse::json(404, &json!({ "error": "unknown admin token route" })),
            }
        }
        _ if request.method == "GET" => {
            if let Some(route) = parse_node_detail_route(&request.path) {
                match route.collection.as_deref() {
                    None => node_or_404(state.node_detail(&route.node_id)),
                    Some("subscribers") => {
                        let online_only = query_bool(&request, "online", false) || query_bool(&request, "online_only", false);
                        node_or_404(state.subscribers_snapshot(Some(&route.node_id), online_only))
                    }
                    Some("groups") => node_or_404(state.groups_snapshot(Some(&route.node_id))),
                    Some("calls") => node_or_404(state.calls_snapshot(Some(&route.node_id))),
                    Some("sds") => {
                        let limit = query_usize(&request, "limit", 100, 500);
                        node_or_404(state.sds_snapshot(Some(&route.node_id), limit))
                    }
                    Some("emergencies") => {
                        let active_only = query_bool(&request, "active", false) || query_bool(&request, "active_only", false);
                        node_or_404(state.emergencies_snapshot(Some(&route.node_id), active_only))
                    }
                    Some("locations") => node_or_404(state.locations_snapshot(Some(&route.node_id))),
                    Some(other) => HttpResponse::json(404, &json!({
                        "error": format!("unknown node detail collection '{}'", other),
                        "available_collections": ["subscribers", "groups", "calls", "sds", "emergencies", "locations"]
                    })),
                }
            } else {
                not_found(node_path, ui_path)
            }
        }
        _ if request.method == "POST" => {
            if let Some(route) = parse_node_command_route(&request.path) {
                match route.shortcut.as_deref() {
                    None => submit_command_from_body(&request.body, state, Some(route.node_id)),
                    Some("kick") => submit_kick_command(&request.body, state, route.node_id),
                    Some("dgna") => submit_dgna_command(&request.body, state, route.node_id),
                    Some("clear-emergency") => submit_clear_emergency_command(&request.body, state, route.node_id),
                    Some("restart-service") => submit_service_command(&request.body, state, route.node_id, ServiceAction::Restart),
                    Some("shutdown-service") => submit_service_command(&request.body, state, route.node_id, ServiceAction::Shutdown),
                    Some(other) => HttpResponse::json(404, &json!({
                        "error": format!("unknown command shortcut '{}'", other),
                        "available_shortcuts": ["kick", "dgna", "clear-emergency", "restart-service", "shutdown-service"]
                    })),
                }
            } else {
                not_found(node_path, ui_path)
            }
        }
        _ => not_found(node_path, ui_path),
    }
}

fn required_role_for_request(request: &HttpRequest) -> AuthRole {
    if request.path.starts_with("/api/admin/tokens") {
        return AuthRole::Admin;
    }
    if request.method == "POST" {
        if let Some(route) = parse_node_command_route(&request.path) {
            return match route.shortcut.as_deref() {
                Some("restart-service") | Some("shutdown-service") => AuthRole::Admin,
                _ => AuthRole::Operator,
            };
        }
        if request.path == "/api/commands" {
            return AuthRole::Operator;
        }
    }
    AuthRole::Viewer
}

fn admin_list_tokens(auth: &AuthState) -> HttpResponse {
    match auth.list_tokens() {
        Ok(tokens) => HttpResponse::json(200, &AuthTokenListResponse { now: now_iso(), count: tokens.len(), tokens }),
        Err(err) => HttpResponse::json(500, &json!({ "error": err })),
    }
}

fn admin_create_token(body: &[u8], auth: &AuthState, identity: Option<&AuthIdentity>) -> HttpResponse {
    let req: CreateAuthTokenRequest = match parse_json_body(body) {
        Ok(req) => req,
        Err(response) => return response,
    };
    match auth.create_token(req, identity.map(|i| i.label.as_str())) {
        Ok(created) => HttpResponse::json(201, &created),
        Err(err) => HttpResponse::json(400, &json!({ "error": err })),
    }
}

fn admin_update_token(token_id: &str, body: &[u8], auth: &AuthState) -> HttpResponse {
    let req: UpdateAuthTokenRequest = match parse_json_body(body) {
        Ok(req) => req,
        Err(response) => return response,
    };
    match auth.update_token(token_id, req) {
        Ok(updated) => HttpResponse::json(200, &updated),
        Err(err) if err == "token not found" => HttpResponse::json(404, &json!({ "error": err })),
        Err(err) => HttpResponse::json(400, &json!({ "error": err })),
    }
}

fn admin_delete_token(token_id: &str, auth: &AuthState) -> HttpResponse {
    match auth.delete_token(token_id) {
        Ok(true) => HttpResponse::json(200, &json!({ "deleted": true, "id": token_id })),
        Ok(false) => HttpResponse::json(404, &json!({ "error": "token not found", "id": token_id })),
        Err(err) => HttpResponse::json(500, &json!({ "error": err })),
    }
}

fn parse_admin_token_route(path: &str) -> Option<String> {
    let rest = path.strip_prefix("/api/admin/tokens/")?;
    let mut parts = rest.split('/').filter(|part| !part.is_empty());
    let id = parts.next()?.to_string();
    if parts.next().is_some() {
        return None;
    }
    Some(id)
}


#[derive(Debug)]
struct NodeCommandRoute {
    node_id: String,
    shortcut: Option<String>,
}

#[derive(Debug)]
struct NodeDetailRoute {
    node_id: String,
    collection: Option<String>,
}

fn parse_node_detail_route(path: &str) -> Option<NodeDetailRoute> {
    let rest = path.strip_prefix("/api/nodes/")?;
    let mut parts = rest.split('/').filter(|part| !part.is_empty());
    let node_id = parts.next()?.to_string();
    let collection = parts.next().map(ToString::to_string);
    if parts.next().is_some() {
        return None;
    }
    Some(NodeDetailRoute { node_id, collection })
}

fn node_or_404<T: serde::Serialize>(value: Option<T>) -> HttpResponse {
    match value {
        Some(value) => HttpResponse::json(200, &value),
        None => HttpResponse::json(404, &json!({ "error": "node not found" })),
    }
}

fn parse_node_command_route(path: &str) -> Option<NodeCommandRoute> {
    let rest = path.strip_prefix("/api/nodes/")?;
    let mut parts = rest.split('/').filter(|part| !part.is_empty());
    let node_id = parts.next()?.to_string();
    if parts.next()? != "commands" {
        return None;
    }
    let shortcut = parts.next().map(ToString::to_string);
    if parts.next().is_some() {
        return None;
    }
    Some(NodeCommandRoute { node_id, shortcut })
}

#[derive(Debug, Deserialize)]
struct OperatorOnlyRequest {
    operator_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct KickCommandRequest {
    operator_id: Option<String>,
    issi: u32,
}

#[derive(Debug, Deserialize)]
struct DgnaCommandRequest {
    operator_id: Option<String>,
    issi: u32,
    gssi: u32,
    attach: bool,
}

#[derive(Debug, Deserialize)]
struct ClearEmergencyCommandRequest {
    operator_id: Option<String>,
    #[serde(default)]
    issi: u32,
}

#[derive(Debug, Clone, Copy)]
enum ServiceAction {
    Restart,
    Shutdown,
}

fn submit_kick_command(body: &[u8], state: SharedControlRoom, node_id: String) -> HttpResponse {
    let req: KickCommandRequest = match parse_json_body(body) {
        Ok(req) => req,
        Err(response) => return response,
    };
    let envelope = state.make_envelope(node_id, req.operator_id, ControlCommand::KickMs { issi: req.issi });
    submit_envelope(envelope, state)
}

fn submit_dgna_command(body: &[u8], state: SharedControlRoom, node_id: String) -> HttpResponse {
    let req: DgnaCommandRequest = match parse_json_body(body) {
        Ok(req) => req,
        Err(response) => return response,
    };
    let envelope = state.make_envelope(
        node_id,
        req.operator_id,
        ControlCommand::Dgna {
            issi: req.issi,
            gssi: req.gssi,
            attach: req.attach,
        },
    );
    submit_envelope(envelope, state)
}

fn submit_clear_emergency_command(body: &[u8], state: SharedControlRoom, node_id: String) -> HttpResponse {
    let req: ClearEmergencyCommandRequest = match parse_json_body(body) {
        Ok(req) => req,
        Err(response) => return response,
    };
    let envelope = state.make_envelope(node_id, req.operator_id, ControlCommand::ClearEmergency { issi: req.issi });
    submit_envelope(envelope, state)
}

fn submit_service_command(body: &[u8], state: SharedControlRoom, node_id: String, action: ServiceAction) -> HttpResponse {
    let req: OperatorOnlyRequest = match parse_json_body(body) {
        Ok(req) => req,
        Err(response) => return response,
    };
    let command = match action {
        ServiceAction::Restart => ControlCommand::RestartService,
        ServiceAction::Shutdown => ControlCommand::ShutdownService,
    };
    let envelope = state.make_envelope(node_id, req.operator_id, command);
    submit_envelope(envelope, state)
}

fn parse_json_body<T: for<'de> Deserialize<'de>>(body: &[u8]) -> Result<T, HttpResponse> {
    serde_json::from_slice(body).map_err(|e| HttpResponse::json(400, &json!({ "error": format!("invalid json: {}", e) })))
}

fn submit_envelope(envelope: ControlCommandEnvelope, state: SharedControlRoom) -> HttpResponse {
    match state.submit_command(envelope) {
        Ok(queued) => HttpResponse::json(202, &queued),
        Err(e) => HttpResponse::json(409, &json!({ "error": e })),
    }
}

fn submit_command_from_body(body: &[u8], state: SharedControlRoom, node_from_path: Option<String>) -> HttpResponse {
    let value: Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(e) => return HttpResponse::json(400, &json!({ "error": format!("invalid json: {}", e) })),
    };

    let envelope = match parse_command_request(value, &state, node_from_path) {
        Ok(envelope) => envelope,
        Err(e) => return HttpResponse::json(400, &json!({ "error": e })),
    };

    submit_envelope(envelope, state)
}

fn parse_command_request(value: Value, state: &SharedControlRoom, node_from_path: Option<String>) -> Result<ControlCommandEnvelope, String> {
    if value.get("target_node_id").is_some() && value.get("command_id").is_some() && value.get("command").is_some() {
        let mut envelope: ControlCommandEnvelope = serde_json::from_value(value).map_err(|e| format!("invalid ControlCommandEnvelope: {}", e))?;
        if let Some(node_id) = node_from_path {
            envelope.target_node_id = node_id;
        }
        return Ok(envelope);
    }

    #[derive(Debug, Deserialize)]
    struct SubmitCommandRequest {
        operator_id: Option<String>,
        command_id: Option<String>,
        issued_at: Option<String>,
        command: ControlCommand,
    }

    let req: SubmitCommandRequest = serde_json::from_value(value).map_err(|e| format!("invalid command request: {}", e))?;
    let target_node_id = node_from_path.ok_or_else(|| {
        "target_node_id is required here; use POST /api/nodes/{node_id}/commands or send a full ControlCommandEnvelope".to_string()
    })?;

    let mut envelope = state.make_envelope(target_node_id, req.operator_id, req.command);
    if let Some(command_id) = req.command_id {
        envelope.command_id = command_id;
    }
    if let Some(issued_at) = req.issued_at {
        envelope.issued_at = issued_at;
    }
    Ok(envelope)
}

pub fn read_http_request(stream: &mut TcpStream) -> Result<HttpRequest, String> {
    let mut buf = Vec::with_capacity(4096);
    let mut chunk = [0u8; 4096];
    let mut header_end = None;

    while header_end.is_none() {
        let n = stream.read(&mut chunk).map_err(|e| format!("read failed: {}", e))?;
        if n == 0 {
            return Err("connection closed before headers".to_string());
        }
        buf.extend_from_slice(&chunk[..n]);
        header_end = find_subslice(&buf, b"\r\n\r\n");
        if buf.len() > MAX_HTTP_REQUEST_BYTES {
            return Err("request too large".to_string());
        }
    }

    let header_end = header_end.expect("header_end checked") + 4;
    let header_text = String::from_utf8_lossy(&buf[..header_end]);
    let mut lines = header_text.lines();
    let request_line = lines.next().ok_or_else(|| "missing request line".to_string())?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().ok_or_else(|| "missing method".to_string())?.to_string();
    let raw_path = parts.next().ok_or_else(|| "missing path".to_string())?;
    let (path, query) = parse_path_and_query(raw_path);

    let mut headers = HashMap::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }

    let content_length = headers
        .get("content-length")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);
    if content_length > MAX_HTTP_REQUEST_BYTES {
        return Err("body too large".to_string());
    }

    let mut body = buf[header_end..].to_vec();
    while body.len() < content_length {
        let n = stream.read(&mut chunk).map_err(|e| format!("body read failed: {}", e))?;
        if n == 0 {
            return Err("connection closed before body complete".to_string());
        }
        body.extend_from_slice(&chunk[..n]);
        if body.len() > MAX_HTTP_REQUEST_BYTES {
            return Err("body too large".to_string());
        }
    }
    body.truncate(content_length);

    Ok(HttpRequest { method, path, query, headers, body })
}

pub fn write_response(stream: &mut TcpStream, response: &HttpResponse) -> std::io::Result<()> {
    let header = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, PATCH, DELETE, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type, Authorization, X-Control-Room-Token, X-NetCore-Token, X-Operator-Token\r\nConnection: close\r\n\r\n",
        response.status,
        response.reason,
        response.content_type,
        response.body.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(&response.body)?;
    stream.flush()
}

pub fn looks_like_websocket_upgrade(peek: &[u8]) -> bool {
    let text = String::from_utf8_lossy(peek).to_ascii_lowercase();
    text.contains("upgrade: websocket") && text.contains("sec-websocket-key:")
}

pub fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    hay.windows(needle.len()).position(|w| w == needle)
}

fn parse_path_and_query(raw_path: &str) -> (String, HashMap<String, String>) {
    let mut split = raw_path.splitn(2, '?');
    let path = split.next().unwrap_or(raw_path).to_string();
    let query = split.next().map(parse_query_string).unwrap_or_default();
    (path, query)
}

fn parse_query_string(query: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let mut split = pair.splitn(2, '=');
        let key = split.next().unwrap_or_default();
        let value = split.next().unwrap_or("true");
        out.insert(percentish_decode(key), percentish_decode(value));
    }
    out
}

fn percentish_decode(value: &str) -> String {
    value.replace('+', " ")
}

fn query_usize(request: &HttpRequest, key: &str, default: usize, max: usize) -> usize {
    request
        .query
        .get(key)
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(default)
        .min(max)
}

fn query_bool(request: &HttpRequest, key: &str, default: bool) -> bool {
    request
        .query
        .get(key)
        .map(|value| matches!(value.as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(default)
}

fn not_found(node_path: &str, ui_path: &str) -> HttpResponse {
    HttpResponse::json(
        404,
        &json!({
            "error": "not found",
            "available": [
                "GET /",
                "GET /health",
                "GET /api/overview",
                "GET /api/subscribers?online=true",
                "GET /api/groups",
                "GET /api/calls",
                "GET /api/sds?limit=100",
                "GET /api/emergencies?active=true",
                "GET /api/locations",
                "GET /api/nodes",
                "GET /api/nodes/{node_id}",
                "GET /api/nodes/{node_id}/subscribers",
                "GET /api/nodes/{node_id}/groups",
                "GET /api/nodes/{node_id}/calls",
                "GET /api/nodes/{node_id}/sds",
                "GET /api/nodes/{node_id}/emergencies",
                "GET /api/nodes/{node_id}/locations",
                "GET /api/state",
                "GET /api/rf",
                "GET /api/health/full",
                "GET /api/events?limit=50&quiet=true",
                "GET /api/commands?limit=50",
                "GET /api/admin/tokens",
                "POST /api/admin/tokens",
                "POST/PATCH /api/admin/tokens/{token_id}",
                "DELETE /api/admin/tokens/{token_id}",
                "POST /api/commands",
                "POST /api/nodes/{node_id}/commands",
                "POST /api/nodes/{node_id}/commands/kick",
                "POST /api/nodes/{node_id}/commands/dgna",
                "POST /api/nodes/{node_id}/commands/clear-emergency",
                format!("WS {}", node_path),
                format!("WS {}", ui_path)
            ]
        }),
    )
}

fn auth_error_response(error: AuthError, required: AuthRole) -> HttpResponse {
    match error {
        AuthError::Missing | AuthError::Invalid => HttpResponse::json(401, &json!({
            "error": "unauthorized",
            "required_role": required.as_str(),
            "hint": "send Authorization: Bearer <token> or X-Control-Room-Token"
        })),
        AuthError::Insufficient => HttpResponse::json(403, &json!({
            "error": "forbidden",
            "required_role": required.as_str()
        })),
    }
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        409 => "Conflict",
        500 => "Internal Server Error",
        _ => "OK",
    }
}

fn index_html(node_path: &str, ui_path: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="de">
<head>
  <meta charset="utf-8">
  <title>NetCore Control Room</title>
  <style>
    body {{ font-family: system-ui, sans-serif; margin: 2rem; max-width: 980px; }}
    code, pre {{ background: #f3f3f3; padding: .2rem .35rem; border-radius: .35rem; }}
    pre {{ padding: 1rem; overflow: auto; }}
    .ok {{ color: #087f23; font-weight: 700; }}
  </style>
</head>
<body>
  <h1>NetCore Control Room Core</h1>
  <p class="ok">Server läuft.</p>
  <p>Base-Station WebSocket: <code>{node_path}</code></p>
  <p>UI/Event WebSocket: <code>{ui_path}</code></p>
  <h2>HTTP API</h2>
  <ul>
    <li><code>GET /health</code></li>
    <li><code>GET /api/overview</code> — schlanker Leitstellenstatus</li>
    <li><code>GET /api/subscribers?online=true</code> — Teilnehmerliste</li>
    <li><code>GET /api/groups</code> — Gruppen und Mitglieder</li>
    <li><code>GET /api/calls</code> — aktive Rufe</li>
    <li><code>GET /api/sds?limit=100</code> — SDS-Log</li>
    <li><code>GET /api/emergencies?active=true</code> — Notrufe</li>
    <li><code>GET /api/locations</code> — zuletzt bekannte LIP-/Positionsdaten</li>
    <li><code>GET /api/nodes</code></li>
    <li><code>GET /api/nodes/&lt;node_id&gt;</code> — Node-Detailansicht</li>
    <li><code>GET /api/nodes/&lt;node_id&gt;/subscribers</code></li>
    <li><code>GET /api/nodes/&lt;node_id&gt;/groups</code></li>
    <li><code>GET /api/nodes/&lt;node_id&gt;/calls</code></li>
    <li><code>GET /api/nodes/&lt;node_id&gt;/sds</code></li>
    <li><code>GET /api/nodes/&lt;node_id&gt;/emergencies</code></li>
    <li><code>GET /api/nodes/&lt;node_id&gt;/locations</code></li>
    <li><code>GET /api/state</code> — kompletter Debug-State</li>
    <li><code>GET /api/rf</code> — RF/SDR-Snapshot</li>
    <li><code>GET /api/health/full</code> — technische Health-Daten</li>
    <li><code>GET /api/events?limit=50&amp;quiet=true</code></li>
    <li><code>GET /api/commands?limit=50</code></li>
    <li><code>GET /api/admin/tokens</code> — Admin: Tokenliste</li>
    <li><code>POST /api/admin/tokens</code> — Admin: Token erzeugen</li>
    <li><code>PATCH /api/admin/tokens/&lt;token_id&gt;</code> — Admin: Token ändern/deaktivieren</li>
    <li><code>DELETE /api/admin/tokens/&lt;token_id&gt;</code> — Admin: Token löschen</li>
    <li><code>POST /api/nodes/&lt;node_id&gt;/commands</code></li>
    <li><code>POST /api/nodes/&lt;node_id&gt;/commands/kick</code></li>
    <li><code>POST /api/nodes/&lt;node_id&gt;/commands/dgna</code></li>
    <li><code>POST /api/nodes/&lt;node_id&gt;/commands/clear-emergency</code></li>
  </ul>
  <h2>Beispiel: Kick MS</h2>
  <pre>curl -X POST http://127.0.0.1:9010/api/nodes/tbs-04010001/commands/kick \
  -H 'Content-Type: application/json' \
  -d '{{"operator_id":"jan","issi":2010001}}'</pre>

  <h2>Beispiel: DGNA Attach</h2>
  <pre>curl -X POST http://127.0.0.1:9010/api/nodes/tbs-04010001/commands/dgna \
  -H 'Content-Type: application/json' \
  -d '{{"operator_id":"jan","issi":2010001,"gssi":1001,"attach":true}}'</pre>
</body>
</html>"#
    )
}

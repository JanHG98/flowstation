use std::collections::HashMap;
use std::env;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};
use tetra_entities::net_control::ControlCommand;
use tetra_entities::net_control_room::ControlCommandEnvelope;

use crate::auth::{
    AuthError, AuthIdentity, AuthRole, AuthState, ChangePasswordRequest, CreateUserRequest, LoginRequest,
    LoginResponse, UpdateUserRequest, UserListResponse,
};
use crate::operations::{
    CreateIncidentRequest, IncidentActionRequest, IncidentNoteRequest, SharedOperations,
    ShiftLogRequest,
};
use crate::state::{SharedControlRoom, now_iso};

const MAX_HTTP_REQUEST_BYTES: usize = 1024 * 1024;
const V5_14_2_NO_RESOLVED_LEN_MARKER: &str = "v5.14.2-no-resolved-len";

pub type SharedDirectory = Arc<Mutex<Value>>;

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

    pub fn with_content_type(status: u16, content_type: &'static str, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            reason: reason_phrase(status),
            content_type,
            body: body.into(),
        }
    }
}

pub fn handle_http_stream(
    mut stream: TcpStream,
    state: SharedControlRoom,
    node_path: &str,
    ui_path: &str,
    auth: AuthState,
    directory: SharedDirectory,
    operations: SharedOperations,
) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let request = match read_http_request(&mut stream) {
        Ok(req) => req,
        Err(err) => {
            let _ = write_response(&mut stream, &HttpResponse::text(400, format!("bad request: {}\n", err)));
            return;
        }
    };

    tracing::debug!(method = %request.method, path = %request.path, "http request");
    let response = route_http(request, state, node_path, ui_path, &auth, &directory, &operations);
    let _ = write_response(&mut stream, &response);
}

fn route_http(
    request: HttpRequest,
    state: SharedControlRoom,
    node_path: &str,
    ui_path: &str,
    auth: &AuthState,
    directory: &SharedDirectory,
    operations: &SharedOperations,
) -> HttpResponse {
    if request.method == "OPTIONS" {
        return HttpResponse::text(204, "");
    }

    let health_public = request.method == "GET"
        && matches!(request.path.as_str(), "/health" | "/health/live" | "/health/ready")
        && auth.allow_health_unauthenticated();
    let login_public = request.method == "POST" && request.path == "/api/login";
    let identity = if health_public || login_public {
        None
    } else {
        match auth.authorize_http_role(&request.headers, required_role_for_request(&request)) {
            Ok(identity) => Some(identity),
            Err(err) => {
                tracing::warn!(method = %request.method, path = %request.path, required = %required_role_for_request(&request), "http request rejected by RBAC");
                return auth_error_response(err, required_role_for_request(&request));
            }
        }
    };

    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") => HttpResponse::html(200, index_html(node_path, ui_path)),
        ("GET", "/health/live") => HttpResponse::json(200, &json!({
            "ok": true,
            "service": "netcore-control-room",
            "status": "live",
            "security_mode": "open_lab",
            "timestamp": now_iso(),
        })),
        ("GET", "/health/ready") => {
            let overview = operations.overview();
            HttpResponse::json(200, &json!({
                "ok": true,
                "service": "netcore-control-room",
                "status": if overview.critical_services_offline == 0 { "ready" } else { "degraded" },
                "critical_services_offline": overview.critical_services_offline,
                "operator_plane_available": true,
                "timestamp": now_iso(),
            }))
        }
        ("GET", "/metrics") => HttpResponse::with_content_type(
            200,
            "text/plain; version=0.0.4; charset=utf-8",
            operations.metrics().into_bytes(),
        ),
        ("GET", "/api/v1/openapi.json") => HttpResponse::json(200, &control_room_openapi()),
        ("GET", "/api/v1/config") => HttpResponse::json(200, &operations.config_snapshot()),
        ("GET", "/api/v1/dependencies") => HttpResponse::json(200, &operations.dependencies()),
        ("GET", "/api/v1/export") => HttpResponse::json(200, &operations.export()),
        ("GET", "/api/v1/control-room/overview") => HttpResponse::json(200, &json!({
            "service": "netcore-control-room",
            "security_mode": "open_lab",
            "authoritative_state": false,
            "legacy": state.overview(),
            "operations": operations.overview(),
            "federated": operations.federated_domain_overview(),
            "timestamp": now_iso(),
        })),
        ("GET", "/api/v1/services") => HttpResponse::json(200, &json!({
            "services": operations.services(),
            "overview": operations.overview(),
            "timestamp": now_iso(),
        })),
        ("POST", "/api/v1/services/poll") => HttpResponse::json(
            202,
            &json!({
                "accepted": operations.trigger_poll(),
                "message": "service poll requested",
                "timestamp": now_iso(),
            }),
        ),
        ("GET", "/api/v1/incidents") => {
            let limit = query_usize(&request, "limit", 200, 5000);
            let status = request.query.get("status").map(String::as_str);
            HttpResponse::json(200, &json!({
                "incidents": operations.incidents(status, limit),
                "timestamp": now_iso(),
            }))
        }
        ("POST", "/api/v1/incidents") => operations_create_incident(&request.body, operations),
        ("GET", "/api/v1/shift-log") => HttpResponse::json(200, &json!({
            "entries": operations.shift_log(query_usize(&request, "limit", 200, 5000)),
            "timestamp": now_iso(),
        })),
        ("POST", "/api/v1/shift-log") => operations_add_shift_log(&request.body, operations),
        ("GET", "/health") => HttpResponse::json(200, &json!({
            "ok": true,
            "service": "netcore-control-room",
            "build_fix": V5_14_2_NO_RESOLVED_LEN_MARKER,
            "control_room_ws": "marker-ping-v1",
            "timestamp": now_iso(),
        })),
        ("POST", "/api/login") => api_login(&request.body, auth),
        ("GET", "/api/me") => HttpResponse::json(200, &json!({
            "ok": true,
            "user": identity.as_ref(),
            "auth_mode": if auth.enabled() { "user_password" } else { "open_lab" }
        })),
        ("GET", "/api/overview") => HttpResponse::json(200, &state.overview()),
        ("GET", "/api/directory") => HttpResponse::json(200, &directory_snapshot(directory)),
        ("GET", "/api/directory/resolved") => HttpResponse::json(200, &directory_resolved_response(directory)),
        ("GET", "/api/directory/upstream") => HttpResponse::json(200, &directory_upstream_debug()),
        ("POST", "/api/directory/import") | ("POST", "/api/directory/merge") => api_directory_import(&request.body, directory),
        ("POST", "/api/directory/refresh") => api_directory_refresh(directory),
        ("GET", "/api/state") => HttpResponse::json(200, &state.snapshot()),
        ("GET", "/api/rf") => HttpResponse::json(200, &state.rf_snapshot()),
        ("GET", "/api/health/full") => HttpResponse::json(200, &state.health_snapshot()),
        ("GET", "/api/packet-data") => HttpResponse::json(
            200,
            &state.packet_data_snapshot(None).expect("global packet-data snapshot exists"),
        ),
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
        ("GET", "/api/admin/users") => admin_list_users(auth),
        ("POST", "/api/admin/users") => admin_create_user(&request.body, auth, identity.as_ref()),
        ("POST", "/api/commands") => submit_command_from_body(&request.body, state, None),
        _ if request.path.starts_with("/api/v1/services/") => {
            route_service_operation(&request, operations)
        }
        _ if request.path.starts_with("/api/v1/incidents/") => {
            route_incident_operation(&request, operations)
        }
        _ if request.path.starts_with("/api/admin/users/") => {
            match parse_admin_user_route(&request.path) {
                Some((username, Some("password"))) if request.method == "POST" => admin_change_user_password(&username, &request.body, auth),
                Some((username, None)) if request.method == "PATCH" || request.method == "POST" => admin_update_user(&username, &request.body, auth),
                Some((username, None)) if request.method == "DELETE" => admin_delete_user(&username, auth),
                _ => HttpResponse::json(404, &json!({ "error": "unknown admin user route" })),
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
                    Some("packet-data") => node_or_404(state.packet_data_snapshot(Some(&route.node_id))),
                    Some(other) => HttpResponse::json(404, &json!({
                        "error": format!("unknown node detail collection '{}'", other),
                        "available_collections": ["subscribers", "groups", "calls", "sds", "emergencies", "locations", "packet-data"]
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
                    Some("legacy-wap") => submit_legacy_wap_command(&request.body, state, route.node_id),
                    Some("restart-service") => submit_service_command(&request.body, state, route.node_id, ServiceAction::Restart),
                    Some("shutdown-service") => submit_service_command(&request.body, state, route.node_id, ServiceAction::Shutdown),
                    Some(other) => HttpResponse::json(404, &json!({
                        "error": format!("unknown command shortcut '{}'", other),
                        "available_shortcuts": ["kick", "dgna", "clear-emergency", "legacy-wap", "restart-service", "shutdown-service"]
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
    if request.path.starts_with("/api/admin/users")
        || (request.method == "POST"
            && request.path.starts_with("/api/v1/services/")
            && (request.path.ends_with("/enable") || request.path.ends_with("/disable")))
        || request.path == "/api/directory/import"
        || request.path == "/api/directory/merge"
        || request.path == "/api/directory/refresh"
    {
        return AuthRole::Admin;
    }
    if request.method == "POST" {
        if request.path == "/api/v1/services/poll"
            || request.path == "/api/v1/incidents"
            || request.path.starts_with("/api/v1/incidents/")
            || request.path == "/api/v1/shift-log"
        {
            return AuthRole::Operator;
        }
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


fn directory_snapshot(directory: &SharedDirectory) -> Value {
    let mut guard = match directory.lock() {
        Ok(guard) => guard,
        Err(_) => return json!({ "error": "directory lock poisoned" }),
    };
    let upstream_status = sync_directory_upstream(&mut guard);
    let mut value = guard.clone();
    if let Some(status) = upstream_status {
        value["upstream_status"] = json!(status);
    }
    value
}

fn api_directory_import(body: &[u8], directory: &SharedDirectory) -> HttpResponse {
    let incoming: Value = match serde_json::from_slice(body) {
        Ok(value) => value,
        Err(error) => return HttpResponse::json(400, &json!({ "error": format!("invalid json: {error}") })),
    };

    let mut guard = match directory.lock() {
        Ok(guard) => guard,
        Err(_) => return HttpResponse::json(500, &json!({ "error": "directory lock poisoned" })),
    };

    merge_directory_value(&mut guard, incoming);
    let resolved = directory_resolved_from_value(&guard);
    HttpResponse::json(200, &json!({
        "ok": true,
        "message": "directory imported",
        "resolved_subscriber_count": resolved.as_object().map(|object| object.len()).unwrap_or(0),
        "directory": guard.clone(),
        "resolved": {
            "subscribers": resolved,
        },
        "timestamp": now_iso(),
    }))
}

fn api_directory_refresh(directory: &SharedDirectory) -> HttpResponse {
    let mut guard = match directory.lock() {
        Ok(guard) => guard,
        Err(_) => return HttpResponse::json(500, &json!({ "error": "directory lock poisoned" })),
    };
    let upstream_status = sync_directory_upstream(&mut guard).unwrap_or_else(|| "no upstream data".to_string());
    let resolved = directory_resolved_from_value(&guard);
    HttpResponse::json(200, &json!({
        "ok": true,
        "message": "directory refreshed",
        "upstream_status": upstream_status,
        "resolved_subscriber_count": resolved.as_object().map(|object| object.len()).unwrap_or(0),
        "directory": guard.clone(),
        "resolved": {
            "subscribers": resolved,
        },
        "timestamp": now_iso(),
    }))
}

fn directory_resolved_response(directory: &SharedDirectory) -> Value {
    let mut guard = match directory.lock() {
        Ok(guard) => guard,
        Err(_) => return json!({ "error": "directory lock poisoned" }),
    };
    let upstream_status = sync_directory_upstream(&mut guard);
    let subscribers = directory_resolved_from_value(&guard);
    json!({
        "directory": guard.clone(),
        "resolved": {
            "subscribers": subscribers,
        },
        "resolved_subscriber_count": subscribers.as_object().map(|object| object.len()).unwrap_or(0),
        "upstream_status": upstream_status.unwrap_or_else(|| "no upstream data".to_string()),
        "timestamp": now_iso(),
    })
}

fn directory_upstream_debug() -> Value {
    let base = directory_upstream_base();
    match fetch_netcore_directory_upstream() {
        Some(value) => json!({
            "ok": true,
            "base": base,
            "directory": value,
            "resolved_subscriber_count": directory_resolved_from_value(&value).as_object().map(|object| object.len()).unwrap_or(0),
            "timestamp": now_iso(),
        }),
        None => json!({
            "ok": false,
            "base": base,
            "message": "no upstream directory data fetched",
            "hint": "Set NETCORE_DIRECTORY_API, NETCORE_DIRECTORY_URL or NETCORE_DIRECTORY_BASE_URL, or run NetCore Directory on http://127.0.0.1:8095",
            "timestamp": now_iso(),
        }),
    }
}

fn sync_directory_upstream(target: &mut Value) -> Option<String> {
    let upstream = fetch_netcore_directory_upstream()?;
    let count = directory_resolved_from_value(&upstream).as_object().map(|object| object.len()).unwrap_or(0);
    merge_directory_value(target, upstream);
    Some(format!("NetCore Directory upstream synced: {count} subscriber name(s)"))
}


fn directory_upstream_base() -> String {
    env::var("NETCORE_DIRECTORY_API")
        .or_else(|_| env::var("NETCORE_DIRECTORY_URL"))
        .or_else(|_| env::var("NETCORE_DIRECTORY_BASE_URL"))
        .unwrap_or_else(|_| "http://127.0.0.1:8095".to_string())
        .trim_end_matches('/')
        .to_string()
}

fn fetch_netcore_directory_upstream() -> Option<Value> {
    let base = directory_upstream_base();

    let devices = http_get_json(&format!("{base}/api/devices")).unwrap_or_else(|| json!([]));
    let basestations = http_get_json(&format!("{base}/api/basestations")).unwrap_or_else(|| json!([]));
    let groups = http_get_json(&format!("{base}/api/groups")).unwrap_or_else(|| json!([]));
    let device_groups = http_get_json(&format!("{base}/api/device-groups")).unwrap_or_else(|| json!([]));
    let statuses = http_get_json(&format!("{base}/api/status")).unwrap_or_else(|| json!([]));

    let any_data = devices.as_array().map(|items| !items.is_empty()).unwrap_or(false)
        || basestations.as_array().map(|items| !items.is_empty()).unwrap_or(false)
        || groups.as_array().map(|items| !items.is_empty()).unwrap_or(false)
        || device_groups.as_array().map(|items| !items.is_empty()).unwrap_or(false)
        || statuses.as_array().map(|items| !items.is_empty()).unwrap_or(false);

    if !any_data {
        return None;
    }

    Some(netcore_directory_api_to_control_room(devices, basestations, groups, device_groups, statuses, &base))
}

fn http_get_json(url: &str) -> Option<Value> {
    let (host, port, path) = parse_http_url(url)?;
    let mut stream = TcpStream::connect((host.as_str(), port)).ok()?;
    let _ = stream.set_read_timeout(Some(Duration::from_millis(900)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(900)));

    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nAccept: application/json\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).ok()?;

    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;

    if !response.starts_with("HTTP/1.1 200") && !response.starts_with("HTTP/1.0 200") {
        return None;
    }

    let body = response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .unwrap_or(response.as_str());

    serde_json::from_str(body.trim()).ok()
}

fn parse_http_url(url: &str) -> Option<(String, u16, String)> {
    let without_scheme = url.strip_prefix("http://")?;
    let (authority, path) = without_scheme
        .split_once('/')
        .map(|(authority, path)| (authority, format!("/{path}")))
        .unwrap_or((without_scheme, "/".to_string()));

    let (host, port) = authority
        .split_once(':')
        .map(|(host, port)| (host.to_string(), port.parse::<u16>().ok()))
        .unwrap_or((authority.to_string(), Some(80)));

    Some((host, port?, path))
}

fn netcore_directory_api_to_control_room(
    devices: Value,
    basestations: Value,
    groups: Value,
    device_groups: Value,
    statuses: Value,
    base: &str,
) -> Value {
    let mut subscribers = serde_json::Map::new();
    let mut group_map = serde_json::Map::new();
    let mut status_groups = serde_json::Map::new();
    let mut status_map = serde_json::Map::new();

    if let Some(items) = devices.as_array() {
        for item in items {
            let Some(issi) = value_field_u64(item, &["issi", "id"]) else { continue; };
            let name = value_field_string(item, &["name", "short", "label"]).unwrap_or_else(|| issi.to_string());
            let short = value_field_string(item, &["short", "label"]);
            let mut entry = json!({
                "name": name,
                "device_class": value_field_string(item, &["type", "kind", "device_class"]).unwrap_or_else(|| "HRT".to_string()),
                "source": "netcore-directory",
            });
            if let Some(short) = short { entry["label"] = json!(short); }
            copy_string_field(item, &mut entry, "owner", &["owner"]);
            copy_string_field(item, &mut entry, "role", &["role"]);
            copy_string_field(item, &mut entry, "icon", &["icon"]);
            copy_string_field(item, &mut entry, "color", &["color"]);
            copy_bool_field(item, &mut entry, "visible", &["visible"]);
            subscribers.insert(issi.to_string(), entry);
        }
    }

    if let Some(items) = basestations.as_array() {
        for item in items {
            let Some(issi) = value_field_u64(item, &["issi", "id"]) else { continue; };
            let name = value_field_string(item, &["name", "short", "label"]).unwrap_or_else(|| issi.to_string());
            let mut entry = json!({
                "name": name,
                "device_class": "Infrastruktur",
                "hidden": true,
                "hide_in_subscribers": true,
                "source": "netcore-directory-basestation",
            });
            copy_string_field(item, &mut entry, "label", &["short", "label"]);
            copy_string_field(item, &mut entry, "location", &["location"]);
            copy_string_field(item, &mut entry, "color", &["color"]);
            subscribers.insert(issi.to_string(), entry);
        }
    }

    if let Some(items) = groups.as_array() {
        for item in items {
            let Some(gssi) = value_field_u64(item, &["gssi", "id"]) else { continue; };
            let name = value_field_string(item, &["name", "short", "label"]).unwrap_or_else(|| gssi.to_string());
            let mut entry = json!({ "name": name, "source": "netcore-directory" });
            copy_string_field(item, &mut entry, "label", &["short", "label"]);
            copy_string_field(item, &mut entry, "kind", &["type", "kind"]);
            copy_string_field(item, &mut entry, "color", &["color"]);
            group_map.insert(gssi.to_string(), entry);
        }
    }

    if let Some(items) = device_groups.as_array() {
        for item in items {
            let Some(group_id) = value_field_u64(item, &["group_id", "id"]) else { continue; };
            let group_key = group_id.to_string();
            let name = value_field_string(item, &["name", "short", "opta", "label"]).unwrap_or_else(|| group_key.clone());
            let mut entry = json!({ "name": name, "source": "netcore-directory-device-group" });
            copy_string_field(item, &mut entry, "label", &["short", "opta", "label"]);
            copy_string_field(item, &mut entry, "kind", &["type", "kind"]);
            copy_string_field(item, &mut entry, "color", &["color"]);
            status_groups.insert(group_key.clone(), entry);

            if let Some(members) = item.get("members").and_then(Value::as_array) {
                for member in members {
                    if let Some(issi) = value_as_u64(member) {
                        let subscriber = subscribers
                            .entry(issi.to_string())
                            .or_insert_with(|| json!({ "name": issi.to_string(), "source": "netcore-directory-device-group" }));
                        subscriber["status_group"] = json!(group_key);
                    }
                }
            }
            if let Some(member_devices) = item.get("member_devices").and_then(Value::as_array) {
                for member in member_devices {
                    let Some(issi) = value_field_u64(member, &["issi", "id"]) else { continue; };
                    let subscriber = subscribers.entry(issi.to_string()).or_insert_with(|| {
                        json!({
                            "name": value_field_string(member, &["name", "short", "label"]).unwrap_or_else(|| issi.to_string()),
                            "device_class": value_field_string(member, &["type", "kind"]).unwrap_or_else(|| "HRT".to_string()),
                            "source": "netcore-directory-device-group-member",
                        })
                    });
                    subscriber["status_group"] = json!(group_key);
                }
            }
        }
    }

    if let Some(items) = statuses.as_array() {
        for item in items {
            let Some(code) = value_field_u64(item, &["code", "status", "id"]) else { continue; };
            let label = value_field_string(item, &["label", "name", "description"]).unwrap_or_else(|| format!("Status {code}"));
            let mut entry = json!({ "label": label, "source": "netcore-directory" });
            copy_string_field(item, &mut entry, "description", &["description"]);
            copy_string_field(item, &mut entry, "color", &["color"]);
            copy_string_field(item, &mut entry, "severity", &["severity"]);
            status_map.insert(code.to_string(), entry);
        }
    }

    json!({
        "subscribers": subscribers,
        "groups": group_map,
        "status_groups": status_groups,
        "statuses": status_map,
        "hide_infrastructure": true,
        "upstream": {
            "kind": "netcore-directory",
            "base": base,
            "timestamp": now_iso(),
        }
    })
}

fn value_field_string(value: &Value, keys: &[&str]) -> Option<String> {
    let object = value.as_object()?;
    for key in keys {
        let Some(value) = object.get(*key) else { continue; };
        if let Some(text) = value.as_str().map(str::trim).filter(|text| !text.is_empty()) {
            return Some(text.to_string());
        }
        if let Some(number) = value.as_u64() {
            return Some(number.to_string());
        }
        if let Some(number) = value.as_i64() {
            return Some(number.to_string());
        }
    }
    None
}

fn value_field_u64(value: &Value, keys: &[&str]) -> Option<u64> {
    let object = value.as_object()?;
    for key in keys {
        let Some(value) = object.get(*key) else { continue; };
        if let Some(number) = value_as_u64(value) {
            return Some(number);
        }
    }
    None
}

fn value_as_u64(value: &Value) -> Option<u64> {
    if let Some(number) = value.as_u64() {
        return Some(number);
    }
    if let Some(number) = value.as_i64().and_then(|number| u64::try_from(number).ok()) {
        return Some(number);
    }
    value.as_str()?.trim().parse::<u64>().ok()
}

fn copy_string_field(source: &Value, target: &mut Value, target_key: &str, source_keys: &[&str]) {
    if let Some(value) = value_field_string(source, source_keys) {
        target[target_key] = json!(value);
    }
}

fn copy_bool_field(source: &Value, target: &mut Value, target_key: &str, source_keys: &[&str]) {
    let Some(object) = source.as_object() else { return; };
    for key in source_keys {
        let Some(value) = object.get(*key) else { continue; };
        if let Some(flag) = value.as_bool() {
            target[target_key] = json!(flag);
            return;
        }
        if let Some(number) = value.as_i64() {
            target[target_key] = json!(number != 0);
            return;
        }
    }
}

fn merge_directory_value(target: &mut Value, incoming: Value) {
    let incoming = canonical_directory_value(incoming);
    if !target.is_object() {
        *target = json!({});
    }
    merge_json_objects(target, &incoming);
    if target.get("hide_infrastructure").is_none() {
        target["hide_infrastructure"] = json!(true);
    }
}

fn canonical_directory_value(value: Value) -> Value {
    if let Some(directory) = value.get("directory").cloned() {
        return canonical_directory_value(directory);
    }

    let mut value = value;
    if let Some(object) = value.as_object_mut() {
        canonicalize_directory_collection(object, "subscribers", &["issi", "individual_issi", "source_issi", "address", "id", "subscriber_id", "terminal_id", "radio_id"]);
        canonicalize_directory_collection(object, "groups", &["gssi", "group", "id", "address"]);
        canonicalize_directory_collection(object, "status_groups", &["id", "key", "name", "label"]);
        canonicalize_directory_collection(object, "statuses", &["code", "status", "id", "number"]);
    }
    value
}

fn canonicalize_directory_collection(object: &mut serde_json::Map<String, Value>, field: &str, key_fields: &[&str]) {
    let Some(value) = object.get_mut(field) else { return; };
    let Some(array) = value.as_array() else { return; };

    let mut map = serde_json::Map::new();
    for item in array {
        if let Some(key) = directory_item_key(item, key_fields) {
            map.insert(key, item.clone());
        }
    }
    *value = Value::Object(map);
}

fn directory_item_key(item: &Value, key_fields: &[&str]) -> Option<String> {
    for key in key_fields {
        if let Some(value) = item.get(*key) {
            if let Some(text) = value.as_str().map(str::trim).filter(|text| !text.is_empty()) {
                return Some(text.to_string());
            }
            if let Some(number) = value.as_u64() {
                return Some(number.to_string());
            }
            if let Some(number) = value.as_i64().and_then(|number| u64::try_from(number).ok()) {
                return Some(number.to_string());
            }
        }
    }
    None
}

fn merge_json_objects(target: &mut Value, incoming: &Value) {
    match (target, incoming) {
        (Value::Object(target), Value::Object(incoming)) => {
            for (key, incoming_value) in incoming {
                match target.get_mut(key) {
                    Some(target_value) => merge_json_objects(target_value, incoming_value),
                    None => {
                        target.insert(key.clone(), incoming_value.clone());
                    }
                }
            }
        }
        (target, incoming) => *target = incoming.clone(),
    }
}

fn directory_resolved_from_value(directory: &Value) -> Value {
    let mut resolved: serde_json::Map<String, Value> = serde_json::Map::new();
    collect_resolved_subscribers(directory, &mut resolved);
    Value::Object(resolved)
}

fn collect_resolved_subscribers(value: &Value, resolved: &mut serde_json::Map<String, Value>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_resolved_subscribers(item, resolved);
            }
        }
        Value::Object(object) => {
            if let Some(issi) = directory_object_issi(object) {
                if let Some(name) = directory_object_name(object, issi) {
                    resolved.entry(issi.to_string()).or_insert_with(|| json!({
                        "issi": issi,
                        "name": name,
                        "source": "directory",
                    }));
                }
            }

            for (key, child) in object {
                if let Ok(issi) = key.trim().parse::<u64>() {
                    if let Some(child_object) = child.as_object() {
                        if let Some(name) = directory_object_name(child_object, issi) {
                            let mut entry = json!({
                                "issi": issi,
                                "name": name,
                                "source": "directory_map",
                            });
                            if let Some(device_class) = directory_object_text(child_object, &["device_class", "class", "kind", "type"]) {
                                entry["device_class"] = json!(device_class);
                            }
                            if let Some(status_group) = directory_object_text(child_object, &["status_group", "statusGroup", "statusgroup"]) {
                                entry["status_group"] = json!(status_group);
                            }
                            resolved.entry(issi.to_string()).or_insert(entry);
                        }
                    } else if let Some(text) = child.as_str().map(str::trim).filter(|text| !text.is_empty()) {
                        if text != issi.to_string() {
                            resolved.entry(issi.to_string()).or_insert_with(|| json!({
                                "issi": issi,
                                "name": text,
                                "source": "directory_map_string",
                            }));
                        }
                    }
                }
                collect_resolved_subscribers(child, resolved);
            }
        }
        _ => {}
    }
}

fn directory_object_issi(object: &serde_json::Map<String, Value>) -> Option<u64> {
    for key in ["issi", "individual_issi", "source_issi", "address", "id", "subscriber_id", "subscriberId", "terminal_id", "terminalId", "radio_id", "radioId"] {
        let Some(value) = object.get(key) else { continue; };
        if let Some(number) = value.as_u64() {
            return Some(number);
        }
        if let Some(number) = value.as_i64().and_then(|number| u64::try_from(number).ok()) {
            return Some(number);
        }
        if let Some(text) = value.as_str().map(str::trim) {
            if let Ok(number) = text.parse::<u64>() {
                return Some(number);
            }
        }
    }
    None
}

fn directory_object_name(object: &serde_json::Map<String, Value>, issi: u64) -> Option<String> {
    directory_object_text(object, &["name", "display_name", "displayName", "label", "alias", "rufname", "callsign", "call_sign", "radio_alias", "radioAlias", "short_name", "shortName", "shortLabel", "terminal_name", "terminalName", "bezeichnung", "description", "title"])
        .filter(|text| {
            let trimmed = text.trim();
            !trimmed.is_empty() && trimmed != "-" && trimmed != issi.to_string()
        })
}

fn directory_object_text(object: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    for key in keys {
        let Some(value) = object.get(*key) else { continue; };
        match value {
            Value::String(text) => {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
            Value::Number(number) => return Some(number.to_string()),
            _ => {}
        }
    }
    None
}

fn api_login(body: &[u8], auth: &AuthState) -> HttpResponse {
    let req: LoginRequest = match parse_json_body(body) {
        Ok(req) => req,
        Err(response) => return response,
    };
    match auth.login(&req.username, &req.password) {
        Ok(user) => HttpResponse::json(200, &LoginResponse { ok: true, user, auth_mode: "user_password" }),
        Err(err) => auth_error_response(err, AuthRole::Viewer),
    }
}

fn admin_list_users(auth: &AuthState) -> HttpResponse {
    match auth.list_users() {
        Ok(users) => HttpResponse::json(200, &UserListResponse { now: now_iso(), count: users.len(), users }),
        Err(err) => HttpResponse::json(500, &json!({ "error": err })),
    }
}

fn admin_create_user(body: &[u8], auth: &AuthState, identity: Option<&AuthIdentity>) -> HttpResponse {
    let req: CreateUserRequest = match parse_json_body(body) {
        Ok(req) => req,
        Err(response) => return response,
    };
    match auth.create_user(req, identity.map(|i| i.username.as_str())) {
        Ok(created) => HttpResponse::json(201, &created),
        Err(err) => HttpResponse::json(400, &json!({ "error": err })),
    }
}

fn admin_update_user(username: &str, body: &[u8], auth: &AuthState) -> HttpResponse {
    let req: UpdateUserRequest = match parse_json_body(body) {
        Ok(req) => req,
        Err(response) => return response,
    };
    match auth.update_user(username, req) {
        Ok(updated) => HttpResponse::json(200, &updated),
        Err(err) if err == "user not found" => HttpResponse::json(404, &json!({ "error": err })),
        Err(err) => HttpResponse::json(400, &json!({ "error": err })),
    }
}

fn admin_change_user_password(username: &str, body: &[u8], auth: &AuthState) -> HttpResponse {
    let req: ChangePasswordRequest = match parse_json_body(body) {
        Ok(req) => req,
        Err(response) => return response,
    };
    match auth.change_user_password(username, req) {
        Ok(updated) => HttpResponse::json(200, &updated),
        Err(err) if err == "user not found" => HttpResponse::json(404, &json!({ "error": err })),
        Err(err) => HttpResponse::json(400, &json!({ "error": err })),
    }
}

fn admin_delete_user(username: &str, auth: &AuthState) -> HttpResponse {
    match auth.delete_user(username) {
        Ok(true) => HttpResponse::json(200, &json!({ "deleted": true, "username": username })),
        Ok(false) => HttpResponse::json(404, &json!({ "error": "user not found", "username": username })),
        Err(err) => HttpResponse::json(500, &json!({ "error": err })),
    }
}

fn parse_admin_user_route(path: &str) -> Option<(String, Option<&'static str>)> {
    let rest = path.strip_prefix("/api/admin/users/")?;
    let mut parts = rest.split('/').filter(|part| !part.is_empty());
    let username = parts.next()?.to_string();
    match parts.next() {
        None => Some((username, None)),
        Some("password") if parts.next().is_none() => Some((username, Some("password"))),
        _ => None,
    }
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

#[derive(Debug, Deserialize)]
struct LegacyWapCommandRequest {
    operator_id: Option<String>,
    dest_issi: u32,
    #[serde(default = "default_legacy_wap_source_issi")]
    source_issi: u32,
    #[serde(default)]
    dest_is_group: bool,
    #[serde(default = "default_legacy_wap_title")]
    title: String,
    message: String,
    url: Option<String>,
    #[serde(default)]
    transport: Option<String>,
    #[serde(default = "default_legacy_wap_message_reference")]
    message_reference: u8,
}

fn default_legacy_wap_source_issi() -> u32 {
    4_010_001
}

fn default_legacy_wap_title() -> String {
    "NetCore".to_string()
}

fn default_legacy_wap_message_reference() -> u8 {
    1
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

fn submit_legacy_wap_command(body: &[u8], state: SharedControlRoom, node_id: String) -> HttpResponse {
    let req: LegacyWapCommandRequest = match parse_json_body(body) {
        Ok(req) => req,
        Err(response) => return response,
    };
    if req.dest_issi == 0 {
        return HttpResponse::json(400, &json!({ "error": "dest_issi must be non-zero" }));
    }
    if req.message.trim().is_empty() {
        return HttpResponse::json(400, &json!({ "error": "message must not be empty" }));
    }
    let transport = match req.transport.as_deref().map(str::trim) {
        Some("sds_tl") | Some("sds-tl") | Some("84") | Some("0x84") => {
            tetra_entities::legacy_wap::LegacyWapTransport::SdsTl
        }
        Some("") | None | Some("wdp") | Some("04") | Some("0x04") => {
            tetra_entities::legacy_wap::LegacyWapTransport::Wdp
        }
        Some(other) => {
            return HttpResponse::json(
                400,
                &json!({ "error": format!("unsupported legacy WAP transport '{}'", other) }),
            );
        }
    };
    let payload = match tetra_entities::legacy_wap::build_compact_wml_type4(
        &req.title,
        &req.message,
        req.url.as_deref(),
        transport,
        req.message_reference,
    ) {
        Ok(payload) => payload,
        Err(error) => {
            return HttpResponse::json(
                400,
                &json!({ "error": format!("legacy WAP payload rejected: {:?}", error) }),
            );
        }
    };
    let len_bits = (payload.len() * 8) as u16;
    let envelope = state.make_envelope(
        node_id,
        req.operator_id,
        ControlCommand::SendRawSdsType4 {
            handle: 0,
            source_ssi: req.source_issi,
            dest_ssi: req.dest_issi,
            dest_is_group: req.dest_is_group,
            len_bits,
            payload,
        },
    );
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
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, PATCH, DELETE, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type, Authorization\r\nConnection: close\r\n\r\n",
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


fn operations_create_incident(body: &[u8], operations: &SharedOperations) -> HttpResponse {
    let request: CreateIncidentRequest = match parse_json_body(body) {
        Ok(request) => request,
        Err(response) => return response,
    };
    match operations.create_incident(request) {
        Ok(incident) => HttpResponse::json(201, &json!({ "incident": incident })),
        Err(error) => HttpResponse::json(400, &json!({ "error": error })),
    }
}

fn operations_add_shift_log(body: &[u8], operations: &SharedOperations) -> HttpResponse {
    let request: ShiftLogRequest = match parse_json_body(body) {
        Ok(request) => request,
        Err(response) => return response,
    };
    match operations.add_shift_log(request) {
        Ok(entry) => HttpResponse::json(201, &json!({ "entry": entry })),
        Err(error) => HttpResponse::json(400, &json!({ "error": error })),
    }
}

fn route_service_operation(request: &HttpRequest, operations: &SharedOperations) -> HttpResponse {
    let suffix = request.path.trim_start_matches("/api/v1/services/");
    let mut parts = suffix.split('/').filter(|part| !part.is_empty());
    let Some(name) = parts.next() else {
        return HttpResponse::json(404, &json!({ "error": "service name missing" }));
    };
    let action = parts.next();
    if parts.next().is_some() {
        return HttpResponse::json(404, &json!({ "error": "unknown service route" }));
    }
    match (request.method.as_str(), action) {
        ("GET", None) => match operations.service(name) {
            Some(service) => HttpResponse::json(200, &service),
            None => HttpResponse::json(404, &json!({ "error": format!("unknown service '{name}'") })),
        },
        ("POST", Some("enable")) | ("POST", Some("disable")) => {
            let body: Value = if request.body.is_empty() {
                json!({})
            } else {
                match serde_json::from_slice(&request.body) {
                    Ok(value) => value,
                    Err(error) => return HttpResponse::json(400, &json!({ "error": format!("invalid json: {error}") })),
                }
            };
            let operator = body
                .get("operator_id")
                .and_then(Value::as_str)
                .unwrap_or("operator");
            let enabled = action == Some("enable");
            match operations.set_service_enabled(name, enabled, operator) {
                Ok(service) => HttpResponse::json(200, &json!({ "service": service })),
                Err(error) => HttpResponse::json(404, &json!({ "error": error })),
            }
        }
        _ => HttpResponse::json(404, &json!({
            "error": "unknown service route",
            "available": ["GET /api/v1/services/{name}", "POST /api/v1/services/{name}/enable", "POST /api/v1/services/{name}/disable"]
        })),
    }
}

fn route_incident_operation(request: &HttpRequest, operations: &SharedOperations) -> HttpResponse {
    let suffix = request.path.trim_start_matches("/api/v1/incidents/");
    let mut parts = suffix.split('/').filter(|part| !part.is_empty());
    let Some(id) = parts.next() else {
        return HttpResponse::json(404, &json!({ "error": "incident id missing" }));
    };
    let action = parts.next();
    if parts.next().is_some() {
        return HttpResponse::json(404, &json!({ "error": "unknown incident route" }));
    }
    match (request.method.as_str(), action) {
        ("GET", None) => {
            let incident = operations
                .incidents(None, 5000)
                .into_iter()
                .find(|incident| incident.id == id);
            match incident {
                Some(incident) => HttpResponse::json(200, &incident),
                None => HttpResponse::json(404, &json!({ "error": format!("unknown incident '{id}'") })),
            }
        }
        ("POST", Some("ack")) => {
            let action: IncidentActionRequest = match parse_json_body_or_default(&request.body) {
                Ok(action) => action,
                Err(response) => return response,
            };
            match operations.acknowledge_incident(id, action) {
                Ok(incident) => HttpResponse::json(200, &json!({ "incident": incident })),
                Err(error) => HttpResponse::json(400, &json!({ "error": error })),
            }
        }
        ("POST", Some("resolve")) => {
            let action: IncidentActionRequest = match parse_json_body_or_default(&request.body) {
                Ok(action) => action,
                Err(response) => return response,
            };
            match operations.resolve_incident(id, action) {
                Ok(incident) => HttpResponse::json(200, &json!({ "incident": incident })),
                Err(error) => HttpResponse::json(400, &json!({ "error": error })),
            }
        }
        ("POST", Some("notes")) => {
            let note: IncidentNoteRequest = match parse_json_body(&request.body) {
                Ok(note) => note,
                Err(response) => return response,
            };
            match operations.add_incident_note(id, note) {
                Ok(incident) => HttpResponse::json(200, &json!({ "incident": incident })),
                Err(error) => HttpResponse::json(400, &json!({ "error": error })),
            }
        }
        _ => HttpResponse::json(404, &json!({
            "error": "unknown incident route",
            "available": ["GET /api/v1/incidents/{id}", "POST /api/v1/incidents/{id}/ack", "POST /api/v1/incidents/{id}/resolve", "POST /api/v1/incidents/{id}/notes"]
        })),
    }
}

fn parse_json_body_or_default<T>(body: &[u8]) -> Result<T, HttpResponse>
where
    T: for<'de> Deserialize<'de> + Default,
{
    if body.is_empty() {
        Ok(T::default())
    } else {
        parse_json_body(body)
    }
}

fn control_room_openapi() -> Value {
    json!({
        "openapi": "3.0.3",
        "info": {
            "title": "NetCore Control Room API",
            "version": "1.0.0-open-lab",
            "description": "Operator and presentation API. Domain state remains authoritative in the individual core services."
        },
        "servers": [{ "url": "/" }],
        "paths": {
            "/health/live": { "get": { "summary": "Liveness" } },
            "/health/ready": { "get": { "summary": "Operator-plane readiness and dependency degradation" } },
            "/metrics": { "get": { "summary": "Prometheus metrics" } },
            "/api/v1/control-room/overview": { "get": { "summary": "Combined operator overview" } },
            "/api/v1/services": { "get": { "summary": "Core-service health matrix" } },
            "/api/v1/services/poll": { "post": { "summary": "Trigger an immediate service poll" } },
            "/api/v1/services/{name}": { "get": { "summary": "Service detail" } },
            "/api/v1/incidents": {
                "get": { "summary": "Incident journal" },
                "post": { "summary": "Create a manual incident" }
            },
            "/api/v1/shift-log": {
                "get": { "summary": "Shift log" },
                "post": { "summary": "Add a shift log entry" }
            },
            "/api/v1/dependencies": { "get": { "summary": "Architecture and dependency view" } },
            "/api/v1/config": { "get": { "summary": "Sanitized configuration" } },
            "/api/v1/export": { "get": { "summary": "Operational-state export" } }
        },
        "security": [],
        "x-netcore-security-mode": "open_lab"
    })
}

fn not_found(node_path: &str, ui_path: &str) -> HttpResponse {
    HttpResponse::json(
        404,
        &json!({
            "error": "not found",
            "available": [
                "GET /",
                "GET /health",
                "GET /health/live",
                "GET /health/ready",
                "GET /metrics",
                "GET /api/v1/control-room/overview",
                "GET /api/v1/services",
                "POST /api/v1/services/poll",
                "GET /api/v1/incidents",
                "POST /api/v1/incidents",
                "GET /api/v1/shift-log",
                "POST /api/v1/shift-log",
                "GET /api/v1/dependencies",
                "GET /api/v1/config",
                "GET /api/v1/export",
                "GET /api/v1/openapi.json",
                "GET /api/overview",
                "GET /api/directory",
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
                "GET /api/packet-data",
                "GET /api/nodes/{node_id}/packet-data",
                "GET /api/events?limit=50&quiet=true",
                "GET /api/commands?limit=50",
                "POST /api/login",
                "GET /api/me",
                "GET /api/admin/users",
                "POST /api/admin/users",
                "PATCH /api/admin/users/{username}",
                "POST /api/admin/users/{username}/password",
                "DELETE /api/admin/users/{username}",
                "POST /api/commands",
                "POST /api/nodes/{node_id}/commands",
                "POST /api/nodes/{node_id}/commands/kick",
                "POST /api/nodes/{node_id}/commands/dgna",
                "POST /api/nodes/{node_id}/commands/clear-emergency",
                "POST /api/nodes/{node_id}/commands/legacy-wap",
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
            "hint": "login with username/password or send HTTP Basic auth"
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
    crate::webui::index_html(node_path, ui_path)
}

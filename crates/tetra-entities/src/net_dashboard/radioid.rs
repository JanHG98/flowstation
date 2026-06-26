//! NetCore Directory client for FlowStation dashboard labels.
//!
//! This module intentionally keeps the historic filename `radioid.rs` so the
//! existing module wiring does not need to move yet. It no longer talks to
//! radioid.net. Instead it queries the local/LAN NetCore Directory Server.
//!
//! Supported local endpoints:
//! - `/api/devices`
//! - `/api/basestations`
//! - `/api/groups`
//! - `/api/status`
//! - `/api/dmr/user/?id=...`
//! - `/api/dmr/repeater/?id=...`
//!
//! Configuration is read directly from the active `config.toml` so the dashboard
//! can use it without plumbing a new field through the whole StackConfig yet:
//!
//! ```toml
//! [netcore_directory]
//! enabled = true
//! base_url = "http://127.0.0.1:8095"
//! timeout_ms = 2000
//! ```
//!
//! Environment overrides are also accepted:
//! - `NETCORE_DIRECTORY_ENABLED=true|false`
//! - `NETCORE_DIRECTORY_URL=http://x.x.x.x:8095`
//! - `NETCORE_DIRECTORY_TIMEOUT_MS=2000`

use serde_json::{Map, Value};
use std::time::Duration;

const DEFAULT_BASE_URL: &str = "http://127.0.0.1:8095";
const DEFAULT_TIMEOUT_MS: u64 = 2_000;

/// Runtime config for the local NetCore Directory client.
#[derive(Debug, Clone)]
pub struct NetCoreDirectoryConfig {
    pub enabled: bool,
    pub base_url: String,
    pub timeout_ms: u64,
}

impl Default for NetCoreDirectoryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: DEFAULT_BASE_URL.to_string(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
        }
    }
}

impl NetCoreDirectoryConfig {
    pub fn normalized_base_url(&self) -> String {
        self.base_url.trim().trim_end_matches('/').to_string()
    }
}

fn parse_netcore_directory_section(text: &str) -> std::collections::HashMap<String, String> {
    let mut out = std::collections::HashMap::new();
    let mut in_section = false;

    for raw_line in text.lines() {
        let line = strip_toml_comment(raw_line).trim().to_string();
        if line.is_empty() {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            in_section = line.trim_matches(|c| c == '[' || c == ']').trim() == "netcore_directory";
            continue;
        }

        if !in_section {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        out.insert(key.trim().to_string(), unquote_toml_value(value.trim()));
    }

    out
}

fn strip_toml_comment(line: &str) -> &str {
    let mut in_string = false;
    let mut escaped = false;
    for (idx, ch) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            '#' if !in_string => return &line[..idx],
            _ => {}
        }
    }
    line
}

fn unquote_toml_value(value: &str) -> String {
    let v = value.trim();
    if v.len() >= 2 && v.starts_with('"') && v.ends_with('"') {
        v[1..v.len() - 1].replace("\\\"", "\"")
    } else {
        v.to_string()
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// Read `[netcore_directory]` from the active FlowStation config file.
///
/// This is deliberately tolerant: unknown/missing/broken config falls back to disabled
/// so a directory outage can never prevent the BS from starting.
pub fn load_config(config_path: &str) -> NetCoreDirectoryConfig {
    let mut cfg = NetCoreDirectoryConfig::default();

    if let Ok(text) = std::fs::read_to_string(config_path) {
        let section = parse_netcore_directory_section(&text);
        if let Some(v) = section.get("enabled").and_then(|v| parse_bool(v)) {
            cfg.enabled = v;
        }
        if let Some(v) = section.get("base_url") {
            let v = v.trim();
            if !v.is_empty() {
                cfg.base_url = v.to_string();
            }
        }
        if let Some(v) = section.get("timeout_ms") {
            if let Ok(n) = v.trim().parse::<u64>() {
                cfg.timeout_ms = n.clamp(250, 30_000);
            }
        }
    }

    // Environment wins over file settings. Handy for systemd drop-ins.
    if let Ok(v) = std::env::var("NETCORE_DIRECTORY_URL") {
        let v = v.trim();
        if !v.is_empty() {
            cfg.base_url = v.to_string();
            cfg.enabled = true;
        }
    }
    if let Ok(v) = std::env::var("NETCORE_DIRECTORY_TIMEOUT_MS") {
        if let Ok(n) = v.trim().parse::<u64>() {
            cfg.timeout_ms = n.clamp(250, 30_000);
        }
    }
    if let Ok(v) = std::env::var("NETCORE_DIRECTORY_ENABLED") {
        match v.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => cfg.enabled = true,
            "0" | "false" | "no" | "off" => cfg.enabled = false,
            _ => {}
        }
    }

    cfg.base_url = cfg.normalized_base_url();
    cfg
}

/// Fetch a raw path from the configured NetCore Directory Server.
///
/// `path_and_query` must start with `/`, e.g. `/api/devices`.
pub fn fetch_path(cfg: &NetCoreDirectoryConfig, path_and_query: &str) -> Result<Option<String>, String> {
    if !cfg.enabled {
        return Ok(None);
    }
    let path = if path_and_query.starts_with('/') {
        path_and_query.to_string()
    } else {
        format!("/{path_and_query}")
    };
    let url = format!("{}{}", cfg.normalized_base_url(), path);

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(cfg.timeout_ms))
        .user_agent("netcore-flowstation-directory")
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client.get(&url).send().map_err(|e| e.to_string())?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("{} returned HTTP {}", url, status));
    }
    resp.text().map(Some).map_err(|e| e.to_string())
}

/// Fetch a local Directory path using config loaded from `config_path`.
pub fn fetch_path_from_config(config_path: &str, path_and_query: &str) -> Result<Option<String>, String> {
    let cfg = load_config(config_path);
    fetch_path(&cfg, path_and_query)
}

/// Convert a Directory `/api/devices` response into the dashboard's compact
/// ISSI-keyed object form.
///
/// Accepted input formats:
/// - Native Directory list: `[{"issi":2020001,"name":"Motorola MTP",...}]`
/// - Export format: `{"devices":[...]}`
/// - Existing local format: `{"2020001":{"name":"Motorola MTP",...}}`
///
/// Output format:
/// `{ "2020001": { "name": "...", "short": "...", "type": "...", ... } }`
pub fn normalize_devices_json(raw: &str) -> Result<String, String> {
    let json: Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    let mut out = Map::new();

    match json {
        Value::Array(arr) => collect_device_array(&mut out, arr),
        Value::Object(mut map) => {
            if let Some(Value::Array(arr)) = map.remove("devices") {
                collect_device_array(&mut out, arr);
            } else {
                // Already an ISSI-keyed object. Keep only numeric keys and visible-ish entries.
                for (key, value) in map {
                    let id = key.trim();
                    if id.parse::<u32>().is_err() {
                        continue;
                    }
                    if !value_visible(&value) {
                        continue;
                    }
                    out.insert(id.to_string(), normalize_device_value(id, value));
                }
            }
        }
        _ => {}
    }

    serde_json::to_string(&Value::Object(out)).map_err(|e| e.to_string())
}

fn collect_device_array(out: &mut Map<String, Value>, arr: Vec<Value>) {
    for value in arr {
        let Some(id) = id_from_value(&value, &["issi", "id", "ssi"]) else {
            continue;
        };
        if !value_visible(&value) {
            continue;
        }
        out.insert(id.to_string(), normalize_device_value(&id.to_string(), value));
    }
}

fn id_from_value(value: &Value, keys: &[&str]) -> Option<u32> {
    let obj = value.as_object()?;
    for key in keys {
        if let Some(v) = obj.get(*key) {
            if let Some(n) = v.as_u64() {
                if n <= u32::MAX as u64 {
                    return Some(n as u32);
                }
            }
            if let Some(s) = v.as_str() {
                if let Ok(n) = s.trim().parse::<u32>() {
                    return Some(n);
                }
            }
        }
    }
    None
}

fn value_visible(value: &Value) -> bool {
    let Some(obj) = value.as_object() else {
        return true;
    };
    match obj.get("visible") {
        Some(Value::Bool(v)) => *v,
        Some(Value::Number(n)) => n.as_i64().unwrap_or(1) != 0,
        Some(Value::String(s)) => !matches!(s.trim().to_ascii_lowercase().as_str(), "0" | "false" | "no" | "off" | "hidden"),
        _ => true,
    }
}

fn normalize_device_value(id: &str, value: Value) -> Value {
    match value {
        Value::String(name) => {
            let mut obj = Map::new();
            obj.insert("issi".to_string(), Value::String(id.to_string()));
            obj.insert("name".to_string(), Value::String(name));
            Value::Object(obj)
        }
        Value::Object(mut obj) => {
            obj.entry("issi".to_string()).or_insert_with(|| Value::String(id.to_string()));

            // Normalize common aliases so older/newer frontends both work.
            if !obj.contains_key("name") {
                if let Some(v) = obj.get("label").cloned().or_else(|| obj.get("title").cloned()) {
                    obj.insert("name".to_string(), v);
                }
            }
            if !obj.contains_key("note") {
                if let Some(v) = obj.get("notes").cloned().or_else(|| obj.get("description").cloned()) {
                    obj.insert("note".to_string(), v);
                }
            }
            Value::Object(obj)
        }
        _ => {
            let mut obj = Map::new();
            obj.insert("issi".to_string(), Value::String(id.to_string()));
            Value::Object(obj)
        }
    }
}

/// Generic pass-through JSON sanity check.
pub fn validate_json_or_empty(raw: &str, empty: &str) -> String {
    if serde_json::from_str::<Value>(raw).is_ok() {
        raw.to_string()
    } else {
        empty.to_string()
    }
}

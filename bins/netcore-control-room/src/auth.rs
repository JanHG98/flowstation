use std::borrow::Cow;
use std::collections::HashMap;
use std::env;

use base64::Engine as _;
use tungstenite::handshake::server::Request;

use crate::config::AuthConfig;

#[derive(Debug, Clone, Copy)]
pub enum AuthRole {
    Node,
    Operator,
}

#[derive(Debug, Clone)]
pub struct AuthState {
    enabled: bool,
    allow_health_unauthenticated: bool,
    node_token: Option<String>,
    operator_token: Option<String>,
}

impl AuthState {
    pub fn from_config(config: &AuthConfig) -> Result<Self, String> {
        let node_token = resolve_secret(config.node_token.as_deref(), config.node_token_env.as_deref());
        let operator_token = resolve_secret(config.operator_token.as_deref(), config.operator_token_env.as_deref());

        if config.enabled {
            if node_token.is_none() {
                return Err(
                    "auth.enabled=true but no node token is configured; set auth.node_token or NETCORE_CONTROL_ROOM_NODE_TOKEN"
                        .to_string(),
                );
            }
            if operator_token.is_none() {
                return Err(
                    "auth.enabled=true but no operator token is configured; set auth.operator_token or NETCORE_CONTROL_ROOM_OPERATOR_TOKEN"
                        .to_string(),
                );
            }
        }

        Ok(Self {
            enabled: config.enabled,
            allow_health_unauthenticated: config.allow_health_unauthenticated,
            node_token,
            operator_token,
        })
    }

    pub fn disabled() -> Self {
        Self {
            enabled: false,
            allow_health_unauthenticated: true,
            node_token: None,
            operator_token: None,
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn allow_health_unauthenticated(&self) -> bool {
        !self.enabled || self.allow_health_unauthenticated
    }

    pub fn authorize_http_operator(&self, headers: &HashMap<String, String>, query: &HashMap<String, String>) -> bool {
        if !self.enabled {
            return true;
        }
        let Some(expected) = self.operator_token.as_deref() else {
            return false;
        };

        header_token(headers, &["authorization"])
            .or_else(|| header_token(headers, &["x-control-room-token", "x-netcore-token", "x-operator-token"]))
            .or_else(|| query_token(query, &["token", "operator_token", "api_token"]))
            .map(|presented| secret_eq(presented.as_bytes(), expected.as_bytes()))
            .unwrap_or(false)
    }

    pub fn authorize_ws_request(&self, role: AuthRole, request: &Request) -> bool {
        if !self.enabled {
            return true;
        }

        let expected = match role {
            AuthRole::Node => self.node_token.as_deref(),
            AuthRole::Operator => self.operator_token.as_deref(),
        };
        let Some(expected) = expected else {
            return false;
        };

        request_header_token(request, &["authorization"])
            .or_else(|| request_header_token(request, &["x-control-room-token", "x-netcore-token", "x-node-token", "x-operator-token"]))
            .or_else(|| request.uri().query().and_then(|q| query_token_from_str(q, &["token", "node_token", "operator_token", "api_token"])))
            .map(|presented| secret_eq(presented.as_bytes(), expected.as_bytes()))
            .unwrap_or(false)
    }
}

fn resolve_secret(config_value: Option<&str>, env_name: Option<&str>) -> Option<String> {
    if let Some(value) = config_value.and_then(non_empty) {
        return Some(value.to_string());
    }
    let Some(env_name) = env_name.and_then(non_empty) else {
        return None;
    };
    env::var(env_name).ok().and_then(|value| non_empty(&value).map(ToString::to_string))
}

fn non_empty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn header_token(headers: &HashMap<String, String>, names: &[&str]) -> Option<String> {
    for name in names {
        if let Some(value) = headers.get(*name).and_then(|v| extract_auth_value(v)) {
            return Some(value);
        }
    }
    None
}

fn request_header_token(request: &Request, names: &[&str]) -> Option<String> {
    for name in names {
        if let Some(value) = request.headers().get(*name).and_then(|v| v.to_str().ok()).and_then(extract_auth_value) {
            return Some(value);
        }
    }
    None
}

fn extract_auth_value(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if let Some(token) = trimmed.strip_prefix("Bearer ").or_else(|| trimmed.strip_prefix("bearer ")) {
        return non_empty(token).map(ToString::to_string);
    }
    if let Some(encoded) = trimmed.strip_prefix("Basic ").or_else(|| trimmed.strip_prefix("basic ")) {
        return decode_basic_password(encoded);
    }
    non_empty(trimmed).map(ToString::to_string)
}

fn decode_basic_password(encoded: &str) -> Option<String> {
    let decoded = base64::engine::general_purpose::STANDARD.decode(encoded.trim()).ok()?;
    let decoded = String::from_utf8(decoded).ok()?;
    let (_user, password) = decoded.split_once(':')?;
    non_empty(password).map(ToString::to_string)
}

fn query_token(query: &HashMap<String, String>, names: &[&str]) -> Option<String> {
    for name in names {
        if let Some(value) = query.get(*name).and_then(|v| non_empty(v)) {
            return Some(value.to_string());
        }
    }
    None
}

fn query_token_from_str(query: &str, names: &[&str]) -> Option<String> {
    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let mut split = pair.splitn(2, '=');
        let key = split.next().unwrap_or_default();
        let value = split.next().unwrap_or_default();
        if names.iter().any(|name| *name == key) {
            return non_empty(value).map(|v| percentish_decode(v).into_owned());
        }
    }
    None
}

fn percentish_decode(value: &str) -> Cow<'_, str> {
    if value.contains('+') || value.contains("%20") {
        Cow::Owned(value.replace('+', " ").replace("%20", " "))
    } else {
        Cow::Borrowed(value)
    }
}

fn secret_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

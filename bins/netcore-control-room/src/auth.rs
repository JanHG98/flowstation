use std::borrow::Cow;
use std::collections::HashMap;
use std::env;

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tungstenite::handshake::server::Request;

use crate::config::AuthConfig;
use crate::persistence::PersistenceHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthRole {
    Node,
    Viewer,
    Operator,
    Admin,
}

impl AuthRole {
    pub fn as_str(self) -> &'static str {
        match self {
            AuthRole::Node => "node",
            AuthRole::Viewer => "viewer",
            AuthRole::Operator => "operator",
            AuthRole::Admin => "admin",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "node" => Some(AuthRole::Node),
            "viewer" | "read" | "readonly" => Some(AuthRole::Viewer),
            "operator" | "ops" => Some(AuthRole::Operator),
            "admin" | "administrator" => Some(AuthRole::Admin),
            _ => None,
        }
    }

    pub fn allows(self, required: AuthRole) -> bool {
        match (self, required) {
            (AuthRole::Admin, AuthRole::Admin | AuthRole::Operator | AuthRole::Viewer) => true,
            (AuthRole::Operator, AuthRole::Operator | AuthRole::Viewer) => true,
            (AuthRole::Viewer, AuthRole::Viewer) => true,
            (AuthRole::Node, AuthRole::Node) => true,
            _ => false,
        }
    }
}

impl std::fmt::Display for AuthRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthIdentity {
    pub token_id: Option<String>,
    pub label: String,
    pub role: AuthRole,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthTokenRecord {
    pub id: String,
    pub label: String,
    pub role: AuthRole,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
    pub last_used_at: Option<String>,
    pub expires_at: Option<String>,
    pub created_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthTokenCreateResponse {
    pub id: String,
    pub label: String,
    pub role: AuthRole,
    pub enabled: bool,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub created_by: Option<String>,
    /// Plain token is shown exactly once. It is only stored as SHA-256 hash in SQLite.
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthTokenListResponse {
    pub now: String,
    pub count: usize,
    pub tokens: Vec<AuthTokenRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAuthTokenRequest {
    pub label: String,
    pub role: String,
    pub expires_at: Option<String>,
    pub created_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAuthTokenRequest {
    pub enabled: Option<bool>,
    pub label: Option<String>,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthError {
    Missing,
    Invalid,
    Insufficient,
}

#[derive(Clone)]
pub struct AuthState {
    enabled: bool,
    allow_health_unauthenticated: bool,
    node_token: Option<String>,
    operator_token: Option<String>,
    operator_token_role: AuthRole,
    persistence: Option<PersistenceHandle>,
}

impl AuthState {
    pub fn from_config(config: &AuthConfig, persistence: Option<PersistenceHandle>) -> Result<Self, String> {
        let node_token = resolve_secret(config.node_token.as_deref(), config.node_token_env.as_deref());
        let operator_token = resolve_secret(config.operator_token.as_deref(), config.operator_token_env.as_deref());
        let operator_token_role = AuthRole::from_str(&config.operator_token_role).unwrap_or(AuthRole::Admin);

        if config.enabled {
            if node_token.is_none() {
                return Err(
                    "auth.enabled=true but no node token is configured; set auth.node_token or NETCORE_CONTROL_ROOM_NODE_TOKEN"
                        .to_string(),
                );
            }
            if operator_token.is_none() && persistence.is_none() {
                return Err(
                    "auth.enabled=true but no operator token or persistence token registry is configured".to_string(),
                );
            }
        }

        Ok(Self {
            enabled: config.enabled,
            allow_health_unauthenticated: config.allow_health_unauthenticated,
            node_token,
            operator_token,
            operator_token_role,
            persistence,
        })
    }

    pub fn disabled() -> Self {
        Self {
            enabled: false,
            allow_health_unauthenticated: true,
            node_token: None,
            operator_token: None,
            operator_token_role: AuthRole::Admin,
            persistence: None,
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn allow_health_unauthenticated(&self) -> bool {
        !self.enabled || self.allow_health_unauthenticated
    }

    pub fn authorize_http_role(
        &self,
        headers: &HashMap<String, String>,
        query: &HashMap<String, String>,
        required: AuthRole,
    ) -> Result<AuthIdentity, AuthError> {
        if !self.enabled {
            return Ok(AuthIdentity {
                token_id: None,
                label: "auth-disabled".to_string(),
                role: AuthRole::Admin,
                source: "disabled".to_string(),
            });
        }

        let presented = header_token(headers, &["authorization"])
            .or_else(|| header_token(headers, &["x-control-room-token", "x-netcore-token", "x-operator-token"]))
            .or_else(|| query_token(query, &["token", "operator_token", "api_token"]));
        let Some(presented) = presented else {
            return Err(AuthError::Missing);
        };

        if let Some(expected) = self.operator_token.as_deref() {
            if secret_eq(presented.as_bytes(), expected.as_bytes()) {
                let identity = AuthIdentity {
                    token_id: None,
                    label: "bootstrap-operator-token".to_string(),
                    role: self.operator_token_role,
                    source: "bootstrap".to_string(),
                };
                return if identity.role.allows(required) { Ok(identity) } else { Err(AuthError::Insufficient) };
            }
        }

        if let Some(persistence) = &self.persistence {
            let hash = token_hash(&presented);
            match persistence.find_auth_token_by_hash(&hash) {
                Ok(Some(record)) => {
                    if is_expired(record.expires_at.as_deref()) {
                        return Err(AuthError::Invalid);
                    }
                    if !record.role.allows(required) {
                        return Err(AuthError::Insufficient);
                    }
                    persistence.update_auth_token_last_used(&record.id, &crate::state::now_iso());
                    return Ok(AuthIdentity {
                        token_id: Some(record.id),
                        label: record.label,
                        role: record.role,
                        source: "registry".to_string(),
                    });
                }
                Ok(None) => {}
                Err(err) => {
                    tracing::warn!("auth token registry lookup failed: {}", err);
                }
            }
        }

        Err(AuthError::Invalid)
    }

    pub fn authorize_ws_request(&self, required: AuthRole, request: &Request) -> Result<AuthIdentity, AuthError> {
        if !self.enabled {
            return Ok(AuthIdentity {
                token_id: None,
                label: "auth-disabled".to_string(),
                role: AuthRole::Admin,
                source: "disabled".to_string(),
            });
        }

        let presented = request_header_token(request, &["authorization"])
            .or_else(|| request_header_token(request, &["x-control-room-token", "x-netcore-token", "x-node-token", "x-operator-token"]))
            .or_else(|| request.uri().query().and_then(|q| query_token_from_str(q, &["token", "node_token", "operator_token", "api_token"])));
        let Some(presented) = presented else {
            return Err(AuthError::Missing);
        };

        if required == AuthRole::Node {
            if let Some(expected) = self.node_token.as_deref() {
                if secret_eq(presented.as_bytes(), expected.as_bytes()) {
                    return Ok(AuthIdentity {
                        token_id: None,
                        label: "bootstrap-node-token".to_string(),
                        role: AuthRole::Node,
                        source: "bootstrap".to_string(),
                    });
                }
            }
        }

        if required != AuthRole::Node {
            if let Some(expected) = self.operator_token.as_deref() {
                if secret_eq(presented.as_bytes(), expected.as_bytes()) {
                    let identity = AuthIdentity {
                        token_id: None,
                        label: "bootstrap-operator-token".to_string(),
                        role: self.operator_token_role,
                        source: "bootstrap".to_string(),
                    };
                    return if identity.role.allows(required) { Ok(identity) } else { Err(AuthError::Insufficient) };
                }
            }
        }

        if let Some(persistence) = &self.persistence {
            let hash = token_hash(&presented);
            match persistence.find_auth_token_by_hash(&hash) {
                Ok(Some(record)) => {
                    if is_expired(record.expires_at.as_deref()) {
                        return Err(AuthError::Invalid);
                    }
                    if !record.role.allows(required) {
                        return Err(AuthError::Insufficient);
                    }
                    persistence.update_auth_token_last_used(&record.id, &crate::state::now_iso());
                    return Ok(AuthIdentity {
                        token_id: Some(record.id),
                        label: record.label,
                        role: record.role,
                        source: "registry".to_string(),
                    });
                }
                Ok(None) => {}
                Err(err) => tracing::warn!("auth token registry lookup failed: {}", err),
            }
        }

        Err(AuthError::Invalid)
    }

    pub fn list_tokens(&self) -> Result<Vec<AuthTokenRecord>, String> {
        let persistence = self.persistence.as_ref().ok_or_else(|| "token registry requires SQLite persistence".to_string())?;
        persistence.list_auth_tokens().map_err(|e| e.to_string())
    }

    pub fn create_token(&self, request: CreateAuthTokenRequest, created_by_fallback: Option<&str>) -> Result<AuthTokenCreateResponse, String> {
        let label = request.label.trim().to_string();
        if label.is_empty() {
            return Err("label is required".to_string());
        }
        let role = AuthRole::from_str(&request.role).ok_or_else(|| "role must be one of: node, viewer, operator, admin".to_string())?;
        let token = generate_token();
        let hash = token_hash(&token);
        let now = crate::state::now_iso();
        let id = format!("tok_{}", uuid::Uuid::new_v4().as_simple());
        let created_by = request.created_by.clone().or_else(|| created_by_fallback.map(ToString::to_string));
        let persistence = self.persistence.as_ref().ok_or_else(|| "token registry requires SQLite persistence".to_string())?;
        let record = persistence
            .insert_auth_token(&id, &label, role, &hash, &now, request.expires_at.as_deref(), created_by.as_deref())
            .map_err(|e| e.to_string())?;
        Ok(AuthTokenCreateResponse {
            id: record.id,
            label: record.label,
            role: record.role,
            enabled: record.enabled,
            created_at: record.created_at,
            expires_at: record.expires_at,
            created_by: record.created_by,
            token,
        })
    }

    pub fn update_token(&self, id: &str, request: UpdateAuthTokenRequest) -> Result<AuthTokenRecord, String> {
        let persistence = self.persistence.as_ref().ok_or_else(|| "token registry requires SQLite persistence".to_string())?;
        persistence
            .update_auth_token(id, request.enabled, request.label.as_deref(), request.expires_at.as_deref(), &crate::state::now_iso())
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "token not found".to_string())
    }

    pub fn delete_token(&self, id: &str) -> Result<bool, String> {
        let persistence = self.persistence.as_ref().ok_or_else(|| "token registry requires SQLite persistence".to_string())?;
        persistence.delete_auth_token(id).map_err(|e| e.to_string())
    }
}

pub fn token_hash(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let digest = hasher.finalize();
    format!("sha256:{}", to_hex(&digest))
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn generate_token() -> String {
    let a = uuid::Uuid::new_v4().as_simple().to_string();
    let b = uuid::Uuid::new_v4().as_simple().to_string();
    format!("ncr_{}{}", a, b)
}

fn is_expired(expires_at: Option<&str>) -> bool {
    let Some(expires_at) = expires_at else {
        return false;
    };
    !expires_at.trim().is_empty() && expires_at <= crate::state::now_iso().as_str()
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

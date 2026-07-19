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
    pub user_id: Option<String>,
    pub username: String,
    pub display_name: String,
    pub role: AuthRole,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRecord {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub role: AuthRole,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
    pub last_login_at: Option<String>,
    pub created_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserListResponse {
    pub now: String,
    pub count: usize,
    pub users: Vec<UserRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub ok: bool,
    pub user: AuthIdentity,
    pub auth_mode: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub display_name: Option<String>,
    pub role: String,
    pub enabled: Option<bool>,
    pub created_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateUserRequest {
    pub display_name: Option<String>,
    pub role: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangePasswordRequest {
    pub password: String,
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
    bootstrap_username: Option<String>,
    bootstrap_password: Option<String>,
    bootstrap_role: AuthRole,
    persistence: Option<PersistenceHandle>,
}

impl AuthState {
    pub fn from_config(config: &AuthConfig, persistence: Option<PersistenceHandle>) -> Result<Self, String> {
        let node_token = resolve_secret(config.node_token.as_deref(), config.node_token_env.as_deref());
        let bootstrap_username = resolve_secret(config.bootstrap_username.as_deref(), config.bootstrap_username_env.as_deref());
        let bootstrap_password = resolve_secret(config.bootstrap_password.as_deref(), config.bootstrap_password_env.as_deref());
        let bootstrap_role = AuthRole::from_str(&config.bootstrap_role).unwrap_or(AuthRole::Admin);

        if config.enabled && node_token.is_none() {
            return Err(
                "auth.enabled=true but no node token is configured; set auth.node_token or NETCORE_CONTROL_ROOM_NODE_TOKEN"
                    .to_string(),
            );
        }

        if config.enabled && persistence.is_none() && (bootstrap_username.is_none() || bootstrap_password.is_none()) {
            return Err(
                "auth.enabled=true but no SQLite user store and no bootstrap username/password are configured".to_string(),
            );
        }

        Ok(Self {
            enabled: config.enabled,
            allow_health_unauthenticated: config.allow_health_unauthenticated,
            node_token,
            bootstrap_username,
            bootstrap_password,
            bootstrap_role,
            persistence,
        })
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn allow_health_unauthenticated(&self) -> bool {
        !self.enabled || self.allow_health_unauthenticated
    }

    pub fn login(&self, username: &str, password: &str) -> Result<AuthIdentity, AuthError> {
        if !self.enabled {
            return Ok(AuthIdentity {
                user_id: None,
                username: "auth-disabled".to_string(),
                display_name: "Auth Disabled".to_string(),
                role: AuthRole::Admin,
                source: "disabled".to_string(),
            });
        }
        let username = username.trim();
        if username.is_empty() || password.is_empty() {
            return Err(AuthError::Missing);
        }

        if let (Some(bootstrap_username), Some(bootstrap_password)) = (self.bootstrap_username.as_deref(), self.bootstrap_password.as_deref()) {
            if username.eq_ignore_ascii_case(bootstrap_username) && secret_eq(password.as_bytes(), bootstrap_password.as_bytes()) {
                return Ok(AuthIdentity {
                    user_id: None,
                    username: bootstrap_username.to_string(),
                    display_name: "Bootstrap Admin".to_string(),
                    role: self.bootstrap_role,
                    source: "bootstrap".to_string(),
                });
            }
        }

        if let Some(persistence) = &self.persistence {
            match persistence.find_user_by_username(username) {
                Ok(Some(record)) => {
                    if !record.enabled {
                        return Err(AuthError::Invalid);
                    }
                    if !verify_password(password, &record.password_salt, &record.password_hash) {
                        return Err(AuthError::Invalid);
                    }
                    persistence.update_user_last_login(&record.username, &crate::state::now_iso());
                    return Ok(AuthIdentity {
                        user_id: Some(record.id),
                        username: record.username,
                        display_name: record.display_name,
                        role: record.role,
                        source: "user-db".to_string(),
                    });
                }
                Ok(None) => {}
                Err(err) => tracing::warn!("auth user lookup failed: {}", err),
            }
        }

        Err(AuthError::Invalid)
    }

    pub fn authorize_http_role(
        &self,
        headers: &HashMap<String, String>,
        required: AuthRole,
    ) -> Result<AuthIdentity, AuthError> {
        if !self.enabled {
            return Ok(AuthIdentity {
                user_id: None,
                username: "auth-disabled".to_string(),
                display_name: "Auth Disabled".to_string(),
                role: AuthRole::Admin,
                source: "disabled".to_string(),
            });
        }

        let Some((username, password)) = header_basic_credentials(headers) else {
            return Err(AuthError::Missing);
        };

        let identity = self.login(&username, &password)?;
        if identity.role.allows(required) {
            Ok(identity)
        } else {
            Err(AuthError::Insufficient)
        }
    }

    pub fn authorize_ws_request(&self, required: AuthRole, request: &Request) -> Result<AuthIdentity, AuthError> {
        if !self.enabled {
            return Ok(AuthIdentity {
                user_id: None,
                username: "auth-disabled".to_string(),
                display_name: "Auth Disabled".to_string(),
                role: AuthRole::Admin,
                source: "disabled".to_string(),
            });
        }

        if required == AuthRole::Node {
            let presented = request_node_token(request);
            let Some(presented) = presented else { return Err(AuthError::Missing); };
            if let Some(expected) = self.node_token.as_deref() {
                if secret_eq(presented.as_bytes(), expected.as_bytes()) {
                    return Ok(AuthIdentity {
                        user_id: None,
                        username: "node".to_string(),
                        display_name: "TBS Node".to_string(),
                        role: AuthRole::Node,
                        source: "node-token".to_string(),
                    });
                }
            }
            return Err(AuthError::Invalid);
        }

        let Some((username, password)) = request_basic_credentials(request) else { return Err(AuthError::Missing); };
        let identity = self.login(&username, &password)?;
        if identity.role.allows(required) { Ok(identity) } else { Err(AuthError::Insufficient) }
    }

    pub fn list_users(&self) -> Result<Vec<UserRecord>, String> {
        let persistence = self.persistence.as_ref().ok_or_else(|| "user registry requires SQLite persistence".to_string())?;
        persistence.list_users().map_err(|e| e.to_string())
    }

    pub fn create_user(&self, request: CreateUserRequest, created_by_fallback: Option<&str>) -> Result<UserRecord, String> {
        let username = normalise_username(&request.username).ok_or_else(|| "username is required".to_string())?;
        let display_name = request.display_name.as_deref().and_then(non_empty).unwrap_or(&username).to_string();
        let role = AuthRole::from_str(&request.role).ok_or_else(|| "role must be one of: viewer, operator, admin".to_string())?;
        if role == AuthRole::Node {
            return Err("node is not a user role; the TBS still uses the node token".to_string());
        }
        if request.password.trim().len() < 6 {
            return Err("password must have at least 6 characters".to_string());
        }
        let now = crate::state::now_iso();
        let id = format!("usr_{}", uuid::Uuid::new_v4().as_simple());
        let salt = generate_salt();
        let hash = password_hash(&salt, &request.password);
        let created_by = request.created_by.clone().or_else(|| created_by_fallback.map(ToString::to_string));
        let enabled = request.enabled.unwrap_or(true);
        let persistence = self.persistence.as_ref().ok_or_else(|| "user registry requires SQLite persistence".to_string())?;
        persistence
            .insert_user(&id, &username, &display_name, role, enabled, &salt, &hash, &now, created_by.as_deref())
            .map_err(|e| e.to_string())
    }

    pub fn update_user(&self, username: &str, request: UpdateUserRequest) -> Result<UserRecord, String> {
        let role = match request.role.as_deref() {
            Some(role) if !role.trim().is_empty() => {
                let role = AuthRole::from_str(role).ok_or_else(|| "role must be one of: viewer, operator, admin".to_string())?;
                if role == AuthRole::Node { return Err("node is not a user role".to_string()); }
                Some(role)
            }
            _ => None,
        };
        let persistence = self.persistence.as_ref().ok_or_else(|| "user registry requires SQLite persistence".to_string())?;
        persistence
            .update_user(username, request.display_name.as_deref(), role, request.enabled, &crate::state::now_iso())
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "user not found".to_string())
    }

    pub fn change_user_password(&self, username: &str, request: ChangePasswordRequest) -> Result<UserRecord, String> {
        if request.password.trim().len() < 6 {
            return Err("password must have at least 6 characters".to_string());
        }
        let salt = generate_salt();
        let hash = password_hash(&salt, &request.password);
        let persistence = self.persistence.as_ref().ok_or_else(|| "user registry requires SQLite persistence".to_string())?;
        persistence
            .update_user_password(username, &salt, &hash, &crate::state::now_iso())
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "user not found".to_string())
    }

    pub fn delete_user(&self, username: &str) -> Result<bool, String> {
        let persistence = self.persistence.as_ref().ok_or_else(|| "user registry requires SQLite persistence".to_string())?;
        persistence.delete_user(username).map_err(|e| e.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct StoredUserRecord {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub role: AuthRole,
    pub enabled: bool,
    pub password_salt: String,
    pub password_hash: String,
}

pub fn password_hash(salt: &str, password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update(b":");
    hasher.update(password.as_bytes());
    format!("sha256:{}", to_hex(&hasher.finalize()))
}

fn verify_password(password: &str, salt: &str, expected_hash: &str) -> bool {
    let calculated = password_hash(salt, password);
    secret_eq(calculated.as_bytes(), expected_hash.as_bytes())
}

fn generate_salt() -> String {
    uuid::Uuid::new_v4().as_simple().to_string()
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

fn resolve_secret(config_value: Option<&str>, env_name: Option<&str>) -> Option<String> {
    if let Some(value) = config_value.and_then(non_empty) {
        return Some(value.to_string());
    }
    let Some(env_name) = env_name.and_then(non_empty) else { return None; };
    env::var(env_name).ok().and_then(|value| non_empty(&value).map(ToString::to_string))
}

fn normalise_username(value: &str) -> Option<String> {
    let trimmed = value.trim().to_ascii_lowercase();
    if trimmed.is_empty() { None } else { Some(trimmed) }
}

fn non_empty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() { None } else { Some(trimmed) }
}

fn header_basic_credentials(headers: &HashMap<String, String>) -> Option<(String, String)> {
    headers.get("authorization").and_then(|value| decode_basic_credentials(value))
}

fn request_basic_credentials(request: &Request) -> Option<(String, String)> {
    request.headers().get("authorization").and_then(|v| v.to_str().ok()).and_then(decode_basic_credentials)
}

fn request_node_token(request: &Request) -> Option<String> {
    // TBS/node authentication is intentionally separate from human User+Password RBAC.
    // Supported formats, in compatibility order:
    //   Authorization: Bearer <node-token>
    //   X-Control-Room-Token / X-NetCore-Token / X-Node-Token: <node-token>
    //   ?token=<node-token> or ?node_token=<node-token>
    //   Authorization: Basic node:<node-token>
    //
    // The existing BS WebSocket transport already supports HTTP Basic auth. Earlier
    // configs map `[control_room] token = "..."` to Basic username "node" with the
    // token as password, so accepting that here prevents the v5 human-login change
    // from breaking the machine link.
    if let Some(token) = request_header_token(request, &["authorization"])
        .or_else(|| request_header_token(request, &["x-control-room-token", "x-netcore-token", "x-node-token"]))
        .or_else(|| request.uri().query().and_then(|q| query_token_from_str(q, &["token", "node_token"])))
    {
        return Some(token);
    }

    let (username, password) = request_basic_credentials(request)?;
    let user = username.trim().to_ascii_lowercase();
    if matches!(user.as_str(), "node" | "tbs" | "basisstation" | "control-room-node") {
        non_empty(&password).map(ToString::to_string)
    } else {
        None
    }
}

fn decode_basic_credentials(value: &str) -> Option<(String, String)> {
    let trimmed = value.trim();
    let encoded = trimmed.strip_prefix("Basic ").or_else(|| trimmed.strip_prefix("basic "))?;
    let decoded = base64::engine::general_purpose::STANDARD.decode(encoded.trim()).ok()?;
    let decoded = String::from_utf8(decoded).ok()?;
    let (username, password) = decoded.split_once(':')?;
    Some((username.trim().to_string(), password.to_string()))
}

fn request_header_token(request: &Request, names: &[&str]) -> Option<String> {
    for name in names {
        if let Some(value) = request.headers().get(*name).and_then(|v| v.to_str().ok()).and_then(extract_token_value) {
            return Some(value);
        }
    }
    None
}

fn extract_token_value(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if let Some(token) = trimmed.strip_prefix("Bearer ").or_else(|| trimmed.strip_prefix("bearer ")) {
        return non_empty(token).map(ToString::to_string);
    }
    if trimmed.starts_with("Basic ") || trimmed.starts_with("basic ") {
        return None;
    }
    non_empty(trimmed).map(ToString::to_string)
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
    if a.len() != b.len() { return false; }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) { diff |= x ^ y; }
    diff == 0
}

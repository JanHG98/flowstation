use clap::{Parser, Subcommand};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

const DEFAULT_API: &str = "http://127.0.0.1:9010";
const DEFAULT_NODE: &str = "SRV-M_TBS-01";
const DEFAULT_OPERATOR: &str = "jan";
const DEFAULT_PROFILE: &str = "default";

type AppResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Debug, Parser)]
#[command(name = "netcore-control-room-operator")]
#[command(about = "Native NetCore Control Room operator console", long_about = None)]
struct Cli {
    #[arg(long)]
    api: Option<String>,
    #[arg(long)]
    username: Option<String>,
    #[arg(long)]
    password: Option<String>,
    #[arg(long)]
    password_file: Option<PathBuf>,
    #[arg(long, default_value = DEFAULT_PROFILE)]
    profile: String,
    #[arg(long)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    Dashboard { #[arg(long, default_value_t = 2)] refresh: u64 },
    Overview,
    Subscribers { #[arg(long)] online: bool },
    Groups,
    Calls,
    Locations,
    Sds { #[arg(long, default_value_t = 20)] limit: usize },
    Commands { #[arg(long, default_value_t = 20)] limit: usize },
    Directory,
    Me,
    Kick { #[arg(long)] node: Option<String>, #[arg(long)] issi: u32, #[arg(long)] operator: Option<String> },
    Dgna { #[arg(long)] node: Option<String>, #[arg(long)] issi: u32, #[arg(long)] gssi: u32, #[arg(long)] detach: bool, #[arg(long)] operator: Option<String> },
    ClearEmergency { #[arg(long)] node: Option<String>, #[arg(long, default_value_t = 0)] issi: u32, #[arg(long)] operator: Option<String> },
    Users { #[command(subcommand)] command: UserCommand },
    Profiles { #[command(subcommand)] command: ProfileCommand },
    Health,
}

#[derive(Debug, Subcommand)]
enum UserCommand {
    List,
    Create { #[arg(long)] username: String, #[arg(long)] password: String, #[arg(long)] role: String, #[arg(long)] display_name: Option<String>, #[arg(long)] disabled: bool },
    Enable { #[arg(long)] username: String },
    Disable { #[arg(long)] username: String },
    Password { #[arg(long)] username: String, #[arg(long)] password: String },
    Delete { #[arg(long)] username: String },
}

#[derive(Debug, Subcommand)]
enum ProfileCommand {
    Show,
    Init {
        #[arg(long, default_value = DEFAULT_PROFILE)] profile: String,
        #[arg(long)] system: bool,
        #[arg(long)] path: Option<PathBuf>,
        #[arg(long)] api: Option<String>,
        #[arg(long)] username: Option<String>,
        #[arg(long)] default_node: Option<String>,
        #[arg(long)] operator_id: Option<String>,
        #[arg(long)] force: bool,
    },
}

#[derive(Debug, Default, Clone)]
struct OperatorConfig { profiles: HashMap<String, ProfileConfig> }

#[derive(Debug, Default, Clone)]
struct ProfileConfig { api: Option<String>, username: Option<String>, default_node: Option<String>, operator_id: Option<String> }

#[derive(Debug, Serialize)]
struct ResolvedSettingsView {
    config_path: Option<String>, profile: String, api: String, username: Option<String>, username_source: Option<String>, password_present: bool, password_source: Option<String>, default_node: String, operator_id: String,
}

#[derive(Debug, Clone)]
struct ResolvedSettings { config_path: Option<PathBuf>, profile: String, api: String, username: Option<String>, username_source: Option<String>, password: Option<String>, password_source: Option<String>, default_node: String, operator_id: String }

struct ApiClient { base: String, username: Option<String>, password: Option<String>, http: reqwest::blocking::Client }

#[derive(Debug)]
struct ApiStatusError { status: reqwest::StatusCode, url: String, body: String }

impl fmt::Display for ApiStatusError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let body = self.body.trim();
        if body.is_empty() { write!(formatter, "Control Room API returned {} for {}", self.status, self.url) } else { write!(formatter, "Control Room API returned {} for {}: {}", self.status, self.url, body) }
    }
}
impl Error for ApiStatusError {}

impl ApiClient {
    fn new(base: &str, username: Option<String>, password: Option<String>) -> Self { Self { base: base.trim_end_matches('/').to_string(), username, password, http: reqwest::blocking::Client::new() } }
    fn get_json(&self, path: &str) -> AppResult<Value> { let url = self.url(path); let request = self.with_auth(self.http.get(&url)); self.read_response(url, request.send()?) }
    fn post_json<T: Serialize + ?Sized>(&self, path: &str, body: &T) -> AppResult<Value> { let url = self.url(path); let request = self.with_auth(self.http.post(&url).json(body)); self.read_response(url, request.send()?) }
    fn patch_json<T: Serialize + ?Sized>(&self, path: &str, body: &T) -> AppResult<Value> { let url = self.url(path); let request = self.with_auth(self.http.patch(&url).json(body)); self.read_response(url, request.send()?) }
    fn delete_json(&self, path: &str) -> AppResult<Value> { let url = self.url(path); let request = self.with_auth(self.http.delete(&url)); self.read_response(url, request.send()?) }
    fn with_auth(&self, request: reqwest::blocking::RequestBuilder) -> reqwest::blocking::RequestBuilder { match (self.username.as_ref(), self.password.as_ref()) { (Some(u), Some(p)) => request.basic_auth(u, Some(p)), _ => request } }
    fn read_response(&self, url: String, response: reqwest::blocking::Response) -> AppResult<Value> { let status = response.status(); let body = response.text()?; if !status.is_success() { return Err(Box::new(ApiStatusError { status, url, body })); } if body.trim().is_empty() { Ok(json!({})) } else { Ok(serde_json::from_str(&body).unwrap_or_else(|_| json!({"raw": body}))) } }
    fn url(&self, path: &str) -> String { if path.starts_with('/') { format!("{}{}", self.base, path) } else { format!("{}/{}", self.base, path) } }
}

fn main() -> AppResult<()> {
    let mut cli = Cli::parse();
    let command = cli.command.take().unwrap_or(Command::Dashboard { refresh: 2 });
    if let Command::Profiles { command: ProfileCommand::Init { profile, system, path, api, username, default_node, operator_id, force } } = &command { return write_profile_config(&cli, profile, *system, path.as_ref(), api.as_deref(), username.as_deref(), default_node.as_deref(), operator_id.as_deref(), *force); }
    let (config_path, config) = load_operator_config(cli.config.as_deref())?;
    let settings = resolve_settings(&cli, config_path, &config)?;
    if let Command::Profiles { command: ProfileCommand::Show } = &command { return print_json(serde_json::to_value(settings.view())?); }
    let api = ApiClient::new(&settings.api, settings.username.clone(), settings.password.clone());
    match command {
        Command::Dashboard { refresh } => run_dashboard(&api, refresh),
        Command::Overview => print_json(api.get_json("/api/overview")?),
        Command::Subscribers { online } => print_json(api.get_json(if online { "/api/subscribers?online=true" } else { "/api/subscribers" })?),
        Command::Groups => print_json(api.get_json("/api/groups")?),
        Command::Calls => print_json(api.get_json("/api/calls")?),
        Command::Locations => print_json(api.get_json("/api/locations")?),
        Command::Sds { limit } => print_json(api.get_json(&format!("/api/sds?limit={limit}"))?),
        Command::Commands { limit } => print_json(api.get_json(&format!("/api/commands?limit={limit}"))?),
        Command::Directory => print_json(api.get_json("/api/directory")?),
        Command::Me => print_json(api.get_json("/api/me")?),
        Command::Kick { node, issi, operator } => { let node = node.unwrap_or_else(|| settings.default_node.clone()); let operator = operator.unwrap_or_else(|| settings.operator_id.clone()); print_json(api.post_json(&format!("/api/nodes/{node}/commands/kick"), &json!({"operator_id": operator, "issi": issi}))?) }
        Command::Dgna { node, issi, gssi, detach, operator } => { let node = node.unwrap_or_else(|| settings.default_node.clone()); let operator = operator.unwrap_or_else(|| settings.operator_id.clone()); print_json(api.post_json(&format!("/api/nodes/{node}/commands/dgna"), &json!({"operator_id": operator, "issi": issi, "gssi": gssi, "attach": !detach}))?) }
        Command::ClearEmergency { node, issi, operator } => { let node = node.unwrap_or_else(|| settings.default_node.clone()); let operator = operator.unwrap_or_else(|| settings.operator_id.clone()); print_json(api.post_json(&format!("/api/nodes/{node}/commands/clear-emergency"), &json!({"operator_id": operator, "issi": issi}))?) }
        Command::Users { command } => handle_user_command(&api, command),
        Command::Profiles { .. } => Ok(()),
        Command::Health => print_json(api.get_json("/health")?),
    }
}

impl ResolvedSettings { fn view(&self) -> ResolvedSettingsView { ResolvedSettingsView { config_path: self.config_path.as_ref().map(|p| p.display().to_string()), profile: self.profile.clone(), api: self.api.clone(), username: self.username.clone(), username_source: self.username_source.clone(), password_present: self.password.as_ref().map(|p| !p.is_empty()).unwrap_or(false), password_source: self.password_source.clone(), default_node: self.default_node.clone(), operator_id: self.operator_id.clone() } } }

fn resolve_settings(cli: &Cli, config_path: Option<PathBuf>, config: &OperatorConfig) -> AppResult<ResolvedSettings> {
    let profile = config.profiles.get(&cli.profile).or_else(|| config.profiles.get(DEFAULT_PROFILE));
    let api = cli.api.clone().or_else(|| env_nonempty("NETCORE_CONTROL_ROOM_API")).or_else(|| profile.and_then(|p| p.api.clone())).unwrap_or_else(|| DEFAULT_API.to_string());
    let mut username_source = None;
    let username = if let Some(u) = cli.username.clone().filter(|u| !u.trim().is_empty()) { username_source = Some("cli --username".to_string()); Some(u) } else if let Some(u) = env_nonempty("NETCORE_CONTROL_ROOM_USER") { username_source = Some("env NETCORE_CONTROL_ROOM_USER".to_string()); Some(u) } else if let Some(u) = profile.and_then(|p| p.username.clone()).filter(|u| !u.trim().is_empty()) { username_source = Some(format!("profile {} username", cli.profile)); Some(u) } else { None };
    let mut password_source = None;
    let password = if let Some(p) = cli.password.clone().filter(|p| !p.trim().is_empty()) { password_source = Some("cli --password".to_string()); Some(p) } else if let Some(path) = cli.password_file.as_ref() { password_source = Some(format!("cli --password-file {}", path.display())); read_secret_file(path)? } else if let Some(p) = env_nonempty("NETCORE_CONTROL_ROOM_PASSWORD") { password_source = Some("env NETCORE_CONTROL_ROOM_PASSWORD".to_string()); Some(p) } else { None };
    let default_node = env_nonempty("NETCORE_CONTROL_ROOM_NODE_ID").or_else(|| profile.and_then(|p| p.default_node.clone())).unwrap_or_else(|| DEFAULT_NODE.to_string());
    let operator_id = env_nonempty("NETCORE_CONTROL_ROOM_OPERATOR_ID").or_else(|| profile.and_then(|p| p.operator_id.clone())).unwrap_or_else(|| DEFAULT_OPERATOR.to_string());
    Ok(ResolvedSettings { config_path, profile: cli.profile.clone(), api, username, username_source, password, password_source, default_node, operator_id })
}

fn handle_user_command(api: &ApiClient, command: UserCommand) -> AppResult<()> {
    match command {
        UserCommand::List => print_json(api.get_json("/api/admin/users")?),
        UserCommand::Create { username, password, role, display_name, disabled } => print_json(api.post_json("/api/admin/users", &json!({"username": username, "password": password, "role": role, "display_name": display_name, "enabled": !disabled}))?),
        UserCommand::Enable { username } => print_json(api.patch_json(&format!("/api/admin/users/{username}"), &json!({"enabled": true}))?),
        UserCommand::Disable { username } => print_json(api.patch_json(&format!("/api/admin/users/{username}"), &json!({"enabled": false}))?),
        UserCommand::Password { username, password } => print_json(api.post_json(&format!("/api/admin/users/{username}/password"), &json!({"password": password}))?),
        UserCommand::Delete { username } => print_json(api.delete_json(&format!("/api/admin/users/{username}"))?),
    }
}

fn write_profile_config(cli: &Cli, profile: &str, system: bool, explicit_path: Option<&PathBuf>, api: Option<&str>, username: Option<&str>, default_node: Option<&str>, operator_id: Option<&str>, force: bool) -> AppResult<()> {
    let path = explicit_path.cloned().or_else(|| cli.config.clone()).unwrap_or_else(|| if system { system_config_path() } else { default_user_config_path() });
    if path.exists() && !force { return Err(format!("config already exists: {}. Re-run with --force to overwrite.", path.display()).into()); }
    if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
    let api = api.map(ToOwned::to_owned).or_else(|| cli.api.clone()).or_else(|| env_nonempty("NETCORE_CONTROL_ROOM_API")).unwrap_or_else(|| "http://10.0.1.25:9010".to_string());
    let default_node = default_node.unwrap_or(DEFAULT_NODE);
    let operator_id = operator_id.unwrap_or(DEFAULT_OPERATOR);
    let mut content = String::new();
    content.push_str("# NetCore Control Room Operator profile config\n");
    content.push_str("# Username only. Passwords are entered in the UI or supplied via CLI env/--password-file.\n\n");
    content.push_str(&format!("[profiles.{}]\n", toml_bare_key(profile)));
    content.push_str(&format!("api = \"{}\"\n", escape_toml_string(&api)));
    content.push_str(&format!("default_node = \"{}\"\n", escape_toml_string(default_node)));
    content.push_str(&format!("operator_id = \"{}\"\n", escape_toml_string(operator_id)));
    if let Some(username) = username.filter(|u| !u.trim().is_empty()) { content.push_str(&format!("username = \"{}\"\n", escape_toml_string(username.trim()))); } else { content.push_str("# username = \"jan\"\n"); }
    fs::write(&path, content)?; set_private_permissions(&path)?;
    println!("created operator profile config: {}", path.display());
    Ok(())
}

fn load_operator_config(explicit_path: Option<&Path>) -> AppResult<(Option<PathBuf>, OperatorConfig)> {
    let mut candidates = Vec::new();
    if let Some(path) = explicit_path { candidates.push(path.to_path_buf()); } else if let Some(path) = env_nonempty("NETCORE_CONTROL_ROOM_OPERATOR_CONFIG") { candidates.push(PathBuf::from(path)); } else { candidates.push(default_user_config_path()); candidates.push(system_config_path()); }
    for path in candidates { if path.exists() { let text = fs::read_to_string(&path)?; return Ok((Some(path), parse_operator_config(&text))); } }
    Ok((None, OperatorConfig::default()))
}

fn parse_operator_config(text: &str) -> OperatorConfig {
    let mut config = OperatorConfig::default(); let mut current_profile = DEFAULT_PROFILE.to_string(); config.profiles.entry(current_profile.clone()).or_default();
    for raw_line in text.lines() {
        let line = strip_comment(raw_line).trim(); if line.is_empty() { continue; }
        if line.starts_with('[') && line.ends_with(']') { let section = line.trim_start_matches('[').trim_end_matches(']').trim(); current_profile = match section.strip_prefix("profiles.") { Some(name) => parse_section_name(name), None if section == "default" => DEFAULT_PROFILE.to_string(), None => section.to_string() }; config.profiles.entry(current_profile.clone()).or_default(); continue; }
        let Some((key, value)) = line.split_once('=') else { continue; }; let key = key.trim(); let value = parse_toml_string(value.trim()); let profile = config.profiles.entry(current_profile.clone()).or_default();
        match key { "api" => profile.api = value, "username" | "user" => profile.username = value, "default_node" | "node" | "node_id" => profile.default_node = value, "operator_id" | "operator" => profile.operator_id = value, _ => {} }
    }
    config
}

fn strip_comment(line: &str) -> &str { let mut in_string = false; let mut escaped = false; for (idx, ch) in line.char_indices() { match ch { '\\' if in_string => escaped = !escaped, '"' if !escaped => in_string = !in_string, '#' if !in_string => return &line[..idx], _ => escaped = false } } line }
fn parse_section_name(value: &str) -> String { parse_toml_string(value.trim()).unwrap_or_else(|| value.trim().to_string()) }
fn parse_toml_string(value: &str) -> Option<String> { let value = value.trim(); if value.is_empty() { return None; } if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 { let inner = &value[1..value.len()-1]; Some(inner.replace("\\\"", "\"").replace("\\\\", "\\")) } else if value.starts_with('\'') && value.ends_with('\'') && value.len() >= 2 { Some(value[1..value.len()-1].to_string()) } else { Some(value.to_string()) } }
fn default_user_config_path() -> PathBuf { #[cfg(windows)] { env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(".")).join("netcore").join("control-room").join("operator.toml") } #[cfg(not(windows))] { env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|| env::var_os("HOME").map(|h| PathBuf::from(h).join(".config"))).unwrap_or_else(|| PathBuf::from(".")).join("netcore").join("control-room").join("operator.toml") } }
fn system_config_path() -> PathBuf { PathBuf::from("/etc/netcore-control-room/operator.toml") }
fn read_secret_file(path: &Path) -> AppResult<Option<String>> { let secret = fs::read_to_string(path)?.trim().to_string(); if secret.is_empty() { Ok(None) } else { Ok(Some(secret)) } }
fn env_nonempty(name: &str) -> Option<String> { env::var(name).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty()) }
fn set_private_permissions(path: &Path) -> std::io::Result<()> { #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; fs::set_permissions(path, fs::Permissions::from_mode(0o600))?; } #[cfg(not(unix))] { let _ = path; } Ok(()) }
fn escape_toml_string(value: &str) -> String { value.replace('\\', "\\\\").replace('"', "\\\"") }
fn toml_bare_key(value: &str) -> String { if value.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') { value.to_string() } else { format!("\"{}\"", escape_toml_string(value)) } }
fn print_json(value: Value) -> AppResult<()> { println!("{}", serde_json::to_string_pretty(&value)?); Ok(()) }

fn run_dashboard(api: &ApiClient, refresh: u64) -> AppResult<()> { loop { let overview = api.get_json("/api/overview")?; print!("\x1B[2J\x1B[1;1H"); println!("NetCore Control Room Dashboard\n"); println!("{}", serde_json::to_string_pretty(&overview)?); thread::sleep(Duration::from_secs(refresh.max(1))); } }

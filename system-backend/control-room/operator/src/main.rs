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
const DEFAULT_NODE: &str = "tbs-04010001";
const DEFAULT_OPERATOR: &str = "operator";
const DEFAULT_PROFILE: &str = "default";

type AppResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Debug, Parser)]
#[command(name = "netcore-control-room-operator")]
#[command(about = "Native NetCore Control Room operator console", long_about = None)]
struct Cli {
    /// Control Room Core base URL, for example http://10.0.1.25:9010.
    /// Resolution order: CLI > NETCORE_CONTROL_ROOM_API > profile config > default.
    #[arg(long)]
    api: Option<String>,

    /// Operator/API bearer token. Prefer profile config or NETCORE_CONTROL_ROOM_TOKEN for daily use.
    /// Resolution order: CLI > CLI token file > env > profile token > profile token file.
    #[arg(long)]
    token: Option<String>,

    /// Read the API token from a file. Useful for shell history hygiene.
    #[arg(long)]
    token_file: Option<PathBuf>,

    /// Operator profile name from operator.toml.
    #[arg(long, default_value = DEFAULT_PROFILE)]
    profile: String,

    /// Operator config path. Can also be set with NETCORE_CONTROL_ROOM_OPERATOR_CONFIG.
    #[arg(long)]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the native terminal dashboard. This polls the Control Room API.
    Dashboard {
        /// Refresh interval in seconds.
        #[arg(long, default_value_t = 2)]
        refresh: u64,
    },

    /// Show the compact overview JSON.
    Overview,

    /// Show subscribers.
    Subscribers {
        /// Only show online subscribers.
        #[arg(long)]
        online: bool,
    },

    /// Show groups.
    Groups,

    /// Show active calls.
    Calls,

    /// Show last known locations.
    Locations,

    /// Show SDS log.
    Sds {
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },

    /// Show command audit log.
    Commands {
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },

    /// Kick a subscriber from the TBS.
    Kick {
        #[arg(long)]
        node: Option<String>,
        #[arg(long)]
        issi: u32,
        #[arg(long)]
        operator: Option<String>,
    },

    /// Attach or detach a subscriber to/from a group by DGNA.
    Dgna {
        #[arg(long)]
        node: Option<String>,
        #[arg(long)]
        issi: u32,
        #[arg(long)]
        gssi: u32,
        /// Detach instead of attach.
        #[arg(long)]
        detach: bool,
        #[arg(long)]
        operator: Option<String>,
    },

    /// Clear emergency state. Omit ISSI or pass 0 to clear all.
    ClearEmergency {
        #[arg(long)]
        node: Option<String>,
        #[arg(long, default_value_t = 0)]
        issi: u32,
        #[arg(long)]
        operator: Option<String>,
    },

    /// Manage RBAC/API tokens. Requires admin role.
    Tokens {
        #[command(subcommand)]
        command: TokenCommand,
    },

    /// Manage local operator profiles. This never contacts the Control Room, except when another command is used.
    Profiles {
        #[command(subcommand)]
        command: ProfileCommand,
    },

    /// Check /health.
    Health,
}

#[derive(Debug, Subcommand)]
enum TokenCommand {
    /// List configured tokens. Plain token values are never shown here.
    List,

    /// Create a new token. The plain token is shown exactly once.
    Create {
        #[arg(long)]
        label: String,
        #[arg(long)]
        role: String,
        #[arg(long)]
        expires_at: Option<String>,
        #[arg(long)]
        created_by: Option<String>,
    },

    /// Enable a token.
    Enable {
        #[arg(long)]
        id: String,
    },

    /// Disable a token without deleting its audit metadata.
    Disable {
        #[arg(long)]
        id: String,
    },

    /// Delete a token.
    Delete {
        #[arg(long)]
        id: String,
    },
}

#[derive(Debug, Subcommand)]
enum ProfileCommand {
    /// Show resolved local operator settings without printing the token.
    Show,

    /// Create an operator profile config file.
    Init {
        /// Profile name to write.
        #[arg(long, default_value = DEFAULT_PROFILE)]
        profile: String,
        /// Write /etc/netcore-control-room/operator.toml.
        #[arg(long)]
        system: bool,
        /// Explicit config path to write.
        #[arg(long)]
        path: Option<PathBuf>,
        /// Control Room API URL.
        #[arg(long)]
        api: Option<String>,
        /// Token to store in the config file. For less shell history exposure, use --token-file.
        #[arg(long)]
        token: Option<String>,
        /// Token file path to store in the config file.
        #[arg(long)]
        token_file: Option<PathBuf>,
        /// Default node ID for commands like kick/dgna/clear-emergency.
        #[arg(long)]
        default_node: Option<String>,
        /// Default operator ID for audit log entries.
        #[arg(long)]
        operator_id: Option<String>,
        /// Overwrite an existing config file.
        #[arg(long)]
        force: bool,
    },
}

#[derive(Debug, Default, Clone)]
struct OperatorConfig {
    profiles: HashMap<String, ProfileConfig>,
}

#[derive(Debug, Default, Clone)]
struct ProfileConfig {
    api: Option<String>,
    token: Option<String>,
    token_file: Option<PathBuf>,
    default_node: Option<String>,
    operator_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct ResolvedSettingsView {
    config_path: Option<String>,
    profile: String,
    api: String,
    token_present: bool,
    token_source: Option<String>,
    default_node: String,
    operator_id: String,
}

#[derive(Debug, Clone)]
struct ResolvedSettings {
    config_path: Option<PathBuf>,
    profile: String,
    api: String,
    token: Option<String>,
    token_source: Option<String>,
    default_node: String,
    operator_id: String,
}

struct ApiClient {
    base: String,
    token: Option<String>,
    http: reqwest::blocking::Client,
}

#[derive(Debug)]
struct ApiStatusError {
    status: reqwest::StatusCode,
    url: String,
    body: String,
}

impl fmt::Display for ApiStatusError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let body = self.body.trim();
        if body.is_empty() {
            write!(formatter, "Control Room API returned {} for {}", self.status, self.url)
        } else {
            write!(formatter, "Control Room API returned {} for {}: {}", self.status, self.url, body)
        }
    }
}

impl Error for ApiStatusError {}

impl ApiClient {
    fn new(base: &str, token: Option<String>) -> Self {
        Self {
            base: base.trim_end_matches('/').to_string(),
            token,
            http: reqwest::blocking::Client::new(),
        }
    }

    fn get_json(&self, path: &str) -> AppResult<Value> {
        let url = self.url(path);
        let mut request = self.http.get(&url);
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        self.read_response(url, request.send()?)
    }

    fn post_json<T: Serialize + ?Sized>(&self, path: &str, body: &T) -> AppResult<Value> {
        let url = self.url(path);
        let mut request = self.http.post(&url).json(body);
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        self.read_response(url, request.send()?)
    }

    fn patch_json<T: Serialize + ?Sized>(&self, path: &str, body: &T) -> AppResult<Value> {
        let url = self.url(path);
        let mut request = self.http.patch(&url).json(body);
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        self.read_response(url, request.send()?)
    }

    fn delete_json(&self, path: &str) -> AppResult<Value> {
        let url = self.url(path);
        let mut request = self.http.delete(&url);
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        self.read_response(url, request.send()?)
    }

    fn read_response(&self, url: String, response: reqwest::blocking::Response) -> AppResult<Value> {
        let status = response.status();
        let body = response.text()?;
        if !status.is_success() {
            return Err(Box::new(ApiStatusError { status, url, body }));
        }
        if body.trim().is_empty() {
            return Ok(json!({}));
        }
        match serde_json::from_str(&body) {
            Ok(value) => Ok(value),
            Err(_) => Ok(json!({ "raw": body })),
        }
    }

    fn url(&self, path: &str) -> String {
        if path.starts_with('/') {
            format!("{}{}", self.base, path)
        } else {
            format!("{}/{}", self.base, path)
        }
    }
}

fn main() -> AppResult<()> {
    let mut cli = Cli::parse();
    let command = cli.command.take().unwrap_or(Command::Dashboard { refresh: 2 });

    if let Command::Profiles { command: ProfileCommand::Init { profile, system, path, api, token, token_file, default_node, operator_id, force } } = &command {
        return init_profile_config(
            &cli,
            profile,
            *system,
            path.as_ref(),
            api.as_deref(),
            token.as_deref(),
            token_file.as_ref(),
            default_node.as_deref(),
            operator_id.as_deref(),
            *force,
        );
    }

    let (config_path, config) = load_operator_config(cli.config.as_deref())?;
    let settings = resolve_settings(&cli, config_path, &config)?;

    if matches!(&command, Command::Profiles { command: ProfileCommand::Show }) {
        return print_json(serde_json::to_value(settings.view())?);
    }

    let api = ApiClient::new(&settings.api, settings.token.clone());

    match command {
        Command::Dashboard { refresh } => run_dashboard(&api, refresh),
        Command::Overview => print_json(api.get_json("/api/overview")?),
        Command::Subscribers { online } => {
            let path = if online { "/api/subscribers?online=true" } else { "/api/subscribers" };
            print_json(api.get_json(path)?)
        }
        Command::Groups => print_json(api.get_json("/api/groups")?),
        Command::Calls => print_json(api.get_json("/api/calls")?),
        Command::Locations => print_json(api.get_json("/api/locations")?),
        Command::Sds { limit } => print_json(api.get_json(&format!("/api/sds?limit={limit}"))?),
        Command::Commands { limit } => print_json(api.get_json(&format!("/api/commands?limit={limit}"))?),
        Command::Kick { node, issi, operator } => {
            let node = node.unwrap_or_else(|| settings.default_node.clone());
            let operator = operator.unwrap_or_else(|| settings.operator_id.clone());
            let body = json!({ "operator_id": operator, "issi": issi });
            print_json(api.post_json(&format!("/api/nodes/{node}/commands/kick"), &body)?)
        }
        Command::Dgna { node, issi, gssi, detach, operator } => {
            let node = node.unwrap_or_else(|| settings.default_node.clone());
            let operator = operator.unwrap_or_else(|| settings.operator_id.clone());
            let body = json!({ "operator_id": operator, "issi": issi, "gssi": gssi, "attach": !detach });
            print_json(api.post_json(&format!("/api/nodes/{node}/commands/dgna"), &body)?)
        }
        Command::ClearEmergency { node, issi, operator } => {
            let node = node.unwrap_or_else(|| settings.default_node.clone());
            let operator = operator.unwrap_or_else(|| settings.operator_id.clone());
            let body = json!({ "operator_id": operator, "issi": issi });
            print_json(api.post_json(&format!("/api/nodes/{node}/commands/clear-emergency"), &body)?)
        }
        Command::Tokens { command } => handle_token_command(&api, command),
        Command::Profiles { .. } => Ok(()),
        Command::Health => print_json(api.get_json("/health")?),
    }
}

impl ResolvedSettings {
    fn view(&self) -> ResolvedSettingsView {
        ResolvedSettingsView {
            config_path: self.config_path.as_ref().map(|path| path.display().to_string()),
            profile: self.profile.clone(),
            api: self.api.clone(),
            token_present: self.token.as_ref().map(|token| !token.trim().is_empty()).unwrap_or(false),
            token_source: self.token_source.clone(),
            default_node: self.default_node.clone(),
            operator_id: self.operator_id.clone(),
        }
    }
}

fn resolve_settings(cli: &Cli, config_path: Option<PathBuf>, config: &OperatorConfig) -> AppResult<ResolvedSettings> {
    let profile = config.profiles.get(&cli.profile).or_else(|| config.profiles.get(DEFAULT_PROFILE));

    let api = cli.api.clone()
        .or_else(|| env_nonempty("NETCORE_CONTROL_ROOM_API"))
        .or_else(|| profile.and_then(|profile| profile.api.clone()))
        .unwrap_or_else(|| DEFAULT_API.to_string());

    let mut token_source = None;
    let token = if let Some(token) = cli.token.clone().filter(|token| !token.trim().is_empty()) {
        token_source = Some("cli --token".to_string());
        Some(token)
    } else if let Some(path) = cli.token_file.as_ref() {
        token_source = Some(format!("cli --token-file {}", path.display()));
        read_token_file(path)?
    } else if let Some(token) = env_nonempty("NETCORE_CONTROL_ROOM_TOKEN") {
        token_source = Some("env NETCORE_CONTROL_ROOM_TOKEN".to_string());
        Some(token)
    } else if let Some(token) = env_nonempty("NETCORE_CONTROL_ROOM_OPERATOR_TOKEN") {
        token_source = Some("env NETCORE_CONTROL_ROOM_OPERATOR_TOKEN".to_string());
        Some(token)
    } else if let Some(token) = profile.and_then(|profile| profile.token.clone()).filter(|token| !token.trim().is_empty()) {
        token_source = Some(format!("profile {} token", cli.profile));
        Some(token)
    } else if let Some(path) = profile.and_then(|profile| profile.token_file.as_ref()) {
        token_source = Some(format!("profile {} token_file {}", cli.profile, path.display()));
        read_token_file(path)?
    } else {
        None
    };

    let default_node = env_nonempty("NETCORE_CONTROL_ROOM_NODE_ID")
        .or_else(|| profile.and_then(|profile| profile.default_node.clone()))
        .unwrap_or_else(|| DEFAULT_NODE.to_string());

    let operator_id = env_nonempty("NETCORE_CONTROL_ROOM_OPERATOR_ID")
        .or_else(|| profile.and_then(|profile| profile.operator_id.clone()))
        .unwrap_or_else(|| DEFAULT_OPERATOR.to_string());

    Ok(ResolvedSettings {
        config_path,
        profile: cli.profile.clone(),
        api,
        token,
        token_source,
        default_node,
        operator_id,
    })
}

fn handle_token_command(api: &ApiClient, command: TokenCommand) -> AppResult<()> {
    match command {
        TokenCommand::List => print_json(api.get_json("/api/admin/tokens")?),
        TokenCommand::Create { label, role, expires_at, created_by } => {
            let body = json!({
                "label": label,
                "role": role,
                "expires_at": expires_at,
                "created_by": created_by,
            });
            print_json(api.post_json("/api/admin/tokens", &body)?)
        }
        TokenCommand::Enable { id } => {
            let body = json!({ "enabled": true });
            print_json(api.patch_json(&format!("/api/admin/tokens/{id}"), &body)?)
        }
        TokenCommand::Disable { id } => {
            let body = json!({ "enabled": false });
            print_json(api.patch_json(&format!("/api/admin/tokens/{id}"), &body)?)
        }
        TokenCommand::Delete { id } => print_json(api.delete_json(&format!("/api/admin/tokens/{id}"))?),
    }
}

fn init_profile_config(
    cli: &Cli,
    profile: &str,
    system: bool,
    explicit_path: Option<&PathBuf>,
    api: Option<&str>,
    token: Option<&str>,
    token_file: Option<&PathBuf>,
    default_node: Option<&str>,
    operator_id: Option<&str>,
    force: bool,
) -> AppResult<()> {
    let path = explicit_path
        .cloned()
        .or_else(|| cli.config.clone())
        .unwrap_or_else(|| if system { system_config_path() } else { default_user_config_path() });

    if path.exists() && !force {
        return Err(format!("config already exists: {}. Re-run with --force to overwrite.", path.display()).into());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let api = api
        .map(ToOwned::to_owned)
        .or_else(|| cli.api.clone())
        .or_else(|| env_nonempty("NETCORE_CONTROL_ROOM_API"))
        .unwrap_or_else(|| "http://10.0.1.25:9010".to_string());

    let default_node = default_node.unwrap_or("SRV-M_TBS-01");
    let operator_id = operator_id.unwrap_or("jan");

    let mut content = String::new();
    content.push_str("# NetCore Control Room Operator profile config\n");
    content.push_str("# Keep this file private if it contains token = ...\n\n");
    content.push_str(&format!("[profiles.{}]\n", toml_bare_key(profile)));
    content.push_str(&format!("api = \"{}\"\n", escape_toml_string(&api)));
    content.push_str(&format!("default_node = \"{}\"\n", escape_toml_string(default_node)));
    content.push_str(&format!("operator_id = \"{}\"\n", escape_toml_string(operator_id)));

    if let Some(token) = token.filter(|token| !token.trim().is_empty()) {
        content.push_str(&format!("token = \"{}\"\n", escape_toml_string(token.trim())));
    } else if let Some(token_file) = token_file {
        content.push_str(&format!("token_file = \"{}\"\n", escape_toml_string(&token_file.display().to_string())));
    } else {
        content.push_str("# token = \"paste-token-here\"\n");
        content.push_str("# token_file = \"/etc/netcore-control-room/operator.token\"\n");
    }

    fs::write(&path, content)?;
    set_private_permissions(&path)?;

    println!("created operator profile config: {}", path.display());
    println!("profile: {profile}");
    println!("api: {api}");
    println!("default_node: {default_node}");
    println!("operator_id: {operator_id}");
    if token.is_none() && token_file.is_none() {
        println!("token: not configured yet — edit the file or add token_file before using protected API commands");
    }
    Ok(())
}

fn load_operator_config(explicit_path: Option<&Path>) -> AppResult<(Option<PathBuf>, OperatorConfig)> {
    let mut candidates = Vec::new();

    if let Some(path) = explicit_path {
        candidates.push(path.to_path_buf());
    } else if let Some(path) = env_nonempty("NETCORE_CONTROL_ROOM_OPERATOR_CONFIG") {
        candidates.push(PathBuf::from(path));
    } else {
        candidates.push(default_user_config_path());
        candidates.push(system_config_path());
    }

    for path in candidates {
        if path.exists() {
            let text = fs::read_to_string(&path)?;
            return Ok((Some(path), parse_operator_config(&text)));
        }
    }

    Ok((None, OperatorConfig::default()))
}

fn parse_operator_config(text: &str) -> OperatorConfig {
    let mut config = OperatorConfig::default();
    let mut current_profile = DEFAULT_PROFILE.to_string();
    config.profiles.entry(current_profile.clone()).or_default();

    for raw_line in text.lines() {
        let line = strip_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            let section = line.trim_start_matches('[').trim_end_matches(']').trim();
            current_profile = match section.strip_prefix("profiles.") {
                Some(name) => parse_section_name(name),
                None if section == "default" => DEFAULT_PROFILE.to_string(),
                None => section.to_string(),
            };
            config.profiles.entry(current_profile.clone()).or_default();
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = parse_toml_string(value.trim());
        let profile = config.profiles.entry(current_profile.clone()).or_default();
        match key {
            "api" => profile.api = value,
            "token" => profile.token = value,
            "token_file" => profile.token_file = value.map(PathBuf::from),
            "default_node" | "node" | "node_id" => profile.default_node = value,
            "operator_id" | "operator" => profile.operator_id = value,
            _ => {}
        }
    }

    config
}

fn strip_comment(line: &str) -> &str {
    let mut in_string = false;
    let mut escaped = false;
    for (idx, ch) in line.char_indices() {
        match ch {
            '\\' if in_string => escaped = !escaped,
            '"' if !escaped => in_string = !in_string,
            '#' if !in_string => return &line[..idx],
            _ => escaped = false,
        }
    }
    line
}

fn parse_section_name(value: &str) -> String {
    parse_toml_string(value.trim()).unwrap_or_else(|| value.trim().to_string())
}

fn parse_toml_string(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
        let inner = &value[1..value.len() - 1];
        Some(inner.replace("\\\"", "\"").replace("\\\\", "\\"))
    } else {
        Some(value.to_string())
    }
}

fn env_nonempty(name: &str) -> Option<String> {
    env::var(name).ok().map(|value| value.trim().to_string()).filter(|value| !value.is_empty())
}

fn read_token_file(path: &Path) -> AppResult<Option<String>> {
    let token = fs::read_to_string(path)?.trim().to_string();
    if token.is_empty() {
        Ok(None)
    } else {
        Ok(Some(token))
    }
}

fn default_user_config_path() -> PathBuf {
    if let Some(xdg) = env_nonempty("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("netcore/control-room/operator.toml");
    }
    if let Some(home) = env_nonempty("HOME") {
        return PathBuf::from(home).join(".config/netcore/control-room/operator.toml");
    }
    system_config_path()
}

fn system_config_path() -> PathBuf {
    PathBuf::from("/etc/netcore-control-room/operator.toml")
}

fn set_private_permissions(path: &Path) -> AppResult<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)?.permissions();
        permissions.set_mode(0o600);
        fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

fn escape_toml_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn toml_bare_key(value: &str) -> String {
    if value.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-') {
        value.to_string()
    } else {
        format!("\"{}\"", escape_toml_string(value))
    }
}

fn run_dashboard(api: &ApiClient, refresh: u64) -> AppResult<()> {
    loop {
        let frame = DashboardFrame {
            overview: api.get_json("/api/overview").unwrap_or_else(error_value),
            subscribers: api.get_json("/api/subscribers?online=true").unwrap_or_else(error_value),
            groups: api.get_json("/api/groups").unwrap_or_else(error_value),
            calls: api.get_json("/api/calls").unwrap_or_else(error_value),
            locations: api.get_json("/api/locations").unwrap_or_else(error_value),
            sds: api.get_json("/api/sds?limit=8").unwrap_or_else(error_value),
            commands: api.get_json("/api/commands?limit=5").unwrap_or_else(error_value),
        };

        clear_screen();
        render_dashboard(&frame);
        thread::sleep(Duration::from_secs(refresh.max(1)));
    }
}

struct DashboardFrame {
    overview: Value,
    subscribers: Value,
    groups: Value,
    calls: Value,
    locations: Value,
    sds: Value,
    commands: Value,
}

fn render_dashboard(frame: &DashboardFrame) {
    let now = str_at(&frame.overview, &["now"]).unwrap_or("unknown");
    println!("NetCore Control Room Operator   {}", now);
    println!("{}", "═".repeat(96));

    if let Some(error) = str_at(&frame.overview, &["error"]) {
        println!("API ERROR: {error}");
        return;
    }

    let nodes_connected = u64_at(&frame.overview, &["nodes_connected"]).unwrap_or(0);
    let node_count = u64_at(&frame.overview, &["node_count"]).unwrap_or(0);
    let subscribers_online = u64_at(&frame.overview, &["subscribers_online"]).unwrap_or(0);
    let subscribers_total = u64_at(&frame.overview, &["subscribers_total"]).unwrap_or(0);
    let groups_total = u64_at(&frame.overview, &["groups_total"]).unwrap_or(0);
    let active_calls_total = u64_at(&frame.overview, &["active_calls_total"]).unwrap_or(0);
    let emergencies_active = u64_at(&frame.overview, &["emergencies_active"]).unwrap_or(0);

    println!(
        "Nodes: {nodes_connected}/{node_count}   Radios: {subscribers_online}/{subscribers_total}   Groups: {groups_total}   Calls: {active_calls_total}   Emergencies: {emergencies_active}"
    );
    println!();

    render_nodes(&frame.overview);
    render_calls(&frame.calls);
    render_subscribers(&frame.subscribers);
    render_groups(&frame.groups);
    render_locations(&frame.locations);
    render_sds(&frame.sds);
    render_commands(&frame.commands);

    println!();
    println!("Ctrl+C beendet die Operator-Konsole. Commands: kick, dgna, clear-emergency. Profile: profiles show/init.");
}

fn render_nodes(overview: &Value) {
    println!("NODES");
    println!("{:<18} {:<8} {:<8} {:<10} {:<8} {:<9} {:<9}", "Node", "Conn", "Health", "Subs", "Calls", "RF peak", "RF rms");
    println!("{}", "-".repeat(96));

    for node in arr_at(overview, &["nodes"]).into_iter().flatten() {
        println!(
            "{:<18} {:<8} {:<8} {:<10} {:<8} {:<9} {:<9}",
            str_at(node, &["node_id"]).unwrap_or("?"),
            bool_word(bool_at(node, &["connected"]).unwrap_or(false)),
            str_at(node, &["health_overall"]).unwrap_or("?"),
            format!("{}/{}", u64_at(node, &["subscribers_online"]).unwrap_or(0), u64_at(node, &["subscribers_total"]).unwrap_or(0)),
            u64_at(node, &["active_calls_total"]).unwrap_or(0),
            fmt_f64(f64_at(node, &["rf_peak_dbfs"])),
            fmt_f64(f64_at(node, &["rf_rms_dbfs"])),
        );
    }
    println!();
}

fn render_calls(calls: &Value) {
    println!("ACTIVE CALLS");
    println!("{:<12} {:<8} {:<8} {:<10} {:<10} {:<8} {:<4} {:<24}", "Key", "GSSI", "Call", "Caller", "Speaker", "Carrier", "TS", "Started");
    println!("{}", "-".repeat(96));

    let mut any = false;
    for call in arr_at(calls, &["calls"]).into_iter().flatten() {
        any = true;
        println!(
            "{:<12} {:<8} {:<8} {:<10} {:<10} {:<8} {:<4} {:<24}",
            str_at(call, &["key"]).unwrap_or("?"),
            u64_at(call, &["gssi"]).map_or("-".into(), |v| v.to_string()),
            u64_at(call, &["call_id"]).unwrap_or(0),
            u64_at(call, &["caller_issi"]).map_or("-".into(), |v| v.to_string()),
            u64_at(call, &["speaker_issi"]).map_or("-".into(), |v| v.to_string()),
            u64_at(call, &["carrier_num"]).map_or("-".into(), |v| v.to_string()),
            u64_at(call, &["ts"]).map_or("-".into(), |v| v.to_string()),
            str_at(call, &["started_at"]).unwrap_or("?"),
        );
    }
    if !any {
        println!("keine aktiven Rufe");
    }
    println!();
}

fn render_subscribers(subscribers: &Value) {
    println!("ONLINE SUBSCRIBERS");
    println!("{:<10} {:<8} {:<24} {:<10} {:<20} {:<20}", "ISSI", "RSSI", "Groups", "ESM", "Active call", "Last seen");
    println!("{}", "-".repeat(96));

    for sub in arr_at(subscribers, &["subscribers"]).into_iter().flatten().take(16) {
        println!(
            "{:<10} {:<8} {:<24} {:<10} {:<20} {:<20}",
            u64_at(sub, &["issi"]).unwrap_or(0),
            fmt_f64(f64_at(sub, &["rssi_dbfs"])),
            join_u64_array(sub, &["groups"]),
            u64_at(sub, &["energy_saving_mode"]).map_or("-".into(), |v| v.to_string()),
            join_string_array(sub, &["active_call_keys"]),
            str_at(sub, &["last_seen"]).unwrap_or("?"),
        );
    }
    println!();
}

fn render_groups(groups: &Value) {
    println!("GROUPS");
    println!("{:<8} {:<8} {:<32} {:<12}", "GSSI", "Online", "Members", "Active call");
    println!("{}", "-".repeat(96));

    for group in arr_at(groups, &["groups"]).into_iter().flatten().take(12) {
        println!(
            "{:<8} {:<8} {:<32} {:<12}",
            u64_at(group, &["gssi"]).unwrap_or(0),
            u64_at(group, &["members_online"]).unwrap_or(0),
            join_u64_array(group, &["members"]),
            str_at(group, &["active_call_key"]).unwrap_or("-"),
        );
    }
    println!();
}

fn render_locations(locations: &Value) {
    println!("LOCATIONS");
    println!("{:<10} {:<12} {:<12} {:<24}", "ISSI", "Latitude", "Longitude", "Updated");
    println!("{}", "-".repeat(96));

    let mut any = false;
    for location in arr_at(locations, &["locations"]).into_iter().flatten().take(8) {
        any = true;
        println!(
            "{:<10} {:<12} {:<12} {:<24}",
            u64_at(location, &["issi"]).unwrap_or(0),
            fmt_f64(f64_at(location, &["latitude"])),
            fmt_f64(f64_at(location, &["longitude"])),
            str_at(location, &["updated_at"]).unwrap_or("?"),
        );
    }
    if !any {
        println!("keine Standorte");
    }
    println!();
}

fn render_sds(sds: &Value) {
    println!("SDS / MESSAGES");
    println!("{:<8} {:<10} {:<10} {:<8} {}", "Dir", "Source", "Dest", "Proto", "Text");
    println!("{}", "-".repeat(96));

    let mut any = false;
    for msg in arr_at(sds, &["sds"]).into_iter().flatten().take(8) {
        any = true;
        println!(
            "{:<8} {:<10} {:<10} {:<8} {}",
            str_at(msg, &["direction"]).unwrap_or("?"),
            u64_at(msg, &["source_issi"]).map_or("-".into(), |v| v.to_string()),
            u64_at(msg, &["dest_issi"]).map_or("-".into(), |v| v.to_string()),
            u64_at(msg, &["protocol_id"]).map_or("-".into(), |v| v.to_string()),
            str_at(msg, &["text"]).unwrap_or(""),
        );
    }
    if !any {
        println!("keine SDS");
    }
    println!();
}

fn render_commands(commands: &Value) {
    println!("COMMAND LOG");
    println!("{:<36} {:<12} {:<10} {}", "Command", "Status", "Operator", "Message");
    println!("{}", "-".repeat(96));

    let mut any = false;
    let command_values = commands.as_array().or_else(|| arr_at(commands, &["commands"]));
    for cmd in command_values.into_iter().flatten().take(5) {
        any = true;
        println!(
            "{:<36} {:<12} {:<10} {}",
            str_at(cmd, &["command_id"]).unwrap_or("?"),
            str_at(cmd, &["status"]).unwrap_or("?"),
            str_at(cmd, &["operator_id"]).unwrap_or("?"),
            str_at(cmd, &["message"]).unwrap_or(""),
        );
    }
    if !any {
        println!("keine Commands");
    }
}

fn clear_screen() {
    print!("\x1B[2J\x1B[H");
}

fn print_json(value: Value) -> AppResult<()> {
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}

fn error_value(error: impl std::fmt::Display) -> Value {
    json!({ "error": error.to_string() })
}

fn bool_word(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn fmt_f64(value: Option<f64>) -> String {
    value.map_or_else(|| "-".to_string(), |v| format!("{v:.2}"))
}

fn arr_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Vec<Value>> {
    get_at(value, path)?.as_array()
}

fn str_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    get_at(value, path)?.as_str()
}

fn u64_at(value: &Value, path: &[&str]) -> Option<u64> {
    get_at(value, path)?.as_u64()
}

fn f64_at(value: &Value, path: &[&str]) -> Option<f64> {
    get_at(value, path)?.as_f64()
}

fn bool_at(value: &Value, path: &[&str]) -> Option<bool> {
    get_at(value, path)?.as_bool()
}

fn get_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut cursor = value;
    for segment in path {
        cursor = cursor.get(*segment)?;
    }
    Some(cursor)
}

fn join_u64_array(value: &Value, path: &[&str]) -> String {
    let values = match arr_at(value, path) {
        Some(values) => values,
        None => return "-".into(),
    };
    if values.is_empty() {
        return "-".into();
    }
    let joined = values
        .iter()
        .filter_map(Value::as_u64)
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",");
    truncate(&joined, 30)
}

fn join_string_array(value: &Value, path: &[&str]) -> String {
    let values = match arr_at(value, path) {
        Some(values) => values,
        None => return "-".into(),
    };
    if values.is_empty() {
        return "-".into();
    }
    let joined = values
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join(",");
    truncate(&joined, 18)
}

fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let mut out = value.chars().take(max_chars.saturating_sub(1)).collect::<String>();
    out.push('…');
    out
}

use eframe::egui;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

const DEFAULT_API: &str = "http://127.0.0.1:9010";
const DEFAULT_PROFILE: &str = "default";
const DEFAULT_NODE: &str = "SRV-M_TBS-01";
const DEFAULT_OPERATOR: &str = "jan";
const UI_VERSION_LABEL: &str = "Native UI v5.14.1 · Directory-API Pull Buildfix";
const DEFAULT_TILE_URL: &str = "https://tile.openstreetmap.org/{z}/{x}/{y}.png";
const DEFAULT_TILE_ATTRIBUTION: &str = "© OpenStreetMap contributors";
const TILE_SIZE: f64 = 256.0;

fn main() -> eframe::Result<()> {
    let (settings, startup_warning) = ResolvedSettings::load();
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 920.0])
            .with_min_inner_size([1024.0, 700.0]),
        ..Default::default()
    };

    eframe::run_native(
        "NetCore Control Room Operator",
        native_options,
        Box::new(move |_cc| Box::new(ControlRoomApp::new(settings.clone(), startup_warning.clone()))),
    )
}

#[derive(Debug, Clone)]
struct ResolvedSettings {
    profile: String,
    api: String,
    username: Option<String>,
    default_node: String,
    operator_id: String,
    map: MapSettings,
    directory: DirectorySettings,
}

#[derive(Debug, Default, Deserialize)]
struct OperatorConfig {
    profiles: HashMap<String, ProfileConfig>,
    ui: Option<UiConfig>,
    directory: Option<DirectoryConfig>,
}

#[derive(Debug, Default, Clone, Deserialize)]
struct UiConfig {
    map: Option<MapSettingsConfig>,
}

#[derive(Debug, Default, Clone, Deserialize)]
struct MapSettingsConfig {
    online_tiles: Option<bool>,
    tile_url: Option<String>,
    tile_attribution: Option<String>,
    default_lat: Option<f64>,
    default_lon: Option<f64>,
    default_zoom: Option<u8>,
    min_zoom: Option<u8>,
    max_zoom: Option<u8>,
    cache_dir: Option<PathBuf>,
}


#[derive(Debug, Default, Clone, Deserialize)]
struct DirectoryConfig {
    #[serde(default)]
    subscribers: HashMap<String, DirectorySubscriberConfig>,
    #[serde(default)]
    groups: HashMap<String, DirectoryLabelConfig>,
    #[serde(default)]
    status_groups: HashMap<String, DirectoryLabelConfig>,
    #[serde(default)]
    statuses: HashMap<String, DirectoryStatusConfig>,
    hide_infrastructure: Option<bool>,
}

#[derive(Debug, Default, Clone, Deserialize)]
struct DirectorySubscriberConfig {
    name: Option<String>,
    label: Option<String>,
    display_name: Option<String>,
    alias: Option<String>,
    device_class: Option<String>,
    class: Option<String>,
    kind: Option<String>,
    status: Option<String>,
    status_group: Option<String>,
    #[serde(default)]
    groups: Vec<u64>,
    #[serde(default)]
    static_groups: Vec<u64>,
    hidden: Option<bool>,
    hide_in_subscribers: Option<bool>,
    #[serde(flatten)]
    extra: HashMap<String, Value>,
}

#[derive(Debug, Default, Clone, Deserialize)]
struct DirectoryLabelConfig {
    name: Option<String>,
    label: Option<String>,
    kind: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Default, Clone, Deserialize)]
struct DirectoryStatusConfig {
    name: Option<String>,
    label: Option<String>,
    group: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Default, Clone)]
struct DirectorySettings {
    subscribers: HashMap<String, DirectorySubscriberConfig>,
    groups: HashMap<String, DirectoryLabelConfig>,
    status_groups: HashMap<String, DirectoryLabelConfig>,
    statuses: HashMap<String, DirectoryStatusConfig>,
    hide_infrastructure: bool,
}

impl DirectorySettings {
    fn from_config(config: Option<&DirectoryConfig>) -> Self {
        let Some(config) = config else {
            return Self { hide_infrastructure: true, ..Default::default() };
        };
        Self {
            subscribers: config.subscribers.clone(),
            groups: config.groups.clone(),
            status_groups: config.status_groups.clone(),
            statuses: config.statuses.clone(),
            hide_infrastructure: config.hide_infrastructure.unwrap_or(true),
        }
    }

    fn subscriber(&self, issi: u64) -> Option<&DirectorySubscriberConfig> {
        for key in id_key_variants(issi) {
            if let Some(entry) = self.subscribers.get(&key) {
                return Some(entry);
            }
        }
        None
    }

    fn group(&self, gssi: u64) -> Option<&DirectoryLabelConfig> {
        for key in id_key_variants(gssi) {
            if let Some(entry) = self.groups.get(&key) {
                return Some(entry);
            }
        }
        None
    }

    fn status(&self, code: u64) -> Option<&DirectoryStatusConfig> {
        for key in id_key_variants(code) {
            if let Some(entry) = self.statuses.get(&key) {
                return Some(entry);
            }
        }
        None
    }


    fn merge_from(&mut self, other: &Self) {
        for (key, other_entry) in &other.subscribers {
            self.subscribers
                .entry(key.clone())
                .and_modify(|entry| merge_subscriber_missing(entry, other_entry))
                .or_insert_with(|| other_entry.clone());
        }
        for (key, other_entry) in &other.groups {
            self.groups
                .entry(key.clone())
                .and_modify(|entry| merge_label_missing(entry, other_entry))
                .or_insert_with(|| other_entry.clone());
        }
        for (key, other_entry) in &other.status_groups {
            self.status_groups
                .entry(key.clone())
                .and_modify(|entry| merge_label_missing(entry, other_entry))
                .or_insert_with(|| other_entry.clone());
        }
        for (key, other_entry) in &other.statuses {
            self.statuses
                .entry(key.clone())
                .and_modify(|entry| merge_status_missing(entry, other_entry))
                .or_insert_with(|| other_entry.clone());
        }
        self.hide_infrastructure = self.hide_infrastructure && other.hide_infrastructure;
    }

    fn fill_missing_from(&mut self, other: &Self) {
        for (key, value) in &other.subscribers {
            self.subscribers.entry(key.clone()).or_insert_with(|| value.clone());
        }
        for (key, value) in &other.groups {
            self.groups.entry(key.clone()).or_insert_with(|| value.clone());
        }
        for (key, value) in &other.status_groups {
            self.status_groups.entry(key.clone()).or_insert_with(|| value.clone());
        }
        for (key, value) in &other.statuses {
            self.statuses.entry(key.clone()).or_insert_with(|| value.clone());
        }
    }

    fn subscriber_issis(&self) -> Vec<u64> {
        let mut ids = self.subscribers
            .keys()
            .filter_map(|key| key.trim().parse::<u64>().ok())
            .collect::<Vec<_>>();
        ids.sort();
        ids.dedup();
        ids
    }

    fn group_gssis(&self) -> Vec<u64> {
        let mut ids = self.groups
            .keys()
            .filter_map(|key| key.trim().parse::<u64>().ok())
            .collect::<Vec<_>>();
        ids.sort();
        ids.dedup();
        ids
    }

    fn status_group(&self, id_or_name: &str) -> Option<&DirectoryLabelConfig> {
        let trimmed = id_or_name.trim();
        if trimmed.is_empty() {
            return None;
        }
        if let Some(entry) = self.status_groups.get(trimmed) {
            return Some(entry);
        }
        if let Ok(id) = trimmed.parse::<u64>() {
            for key in id_key_variants(id) {
                if let Some(entry) = self.status_groups.get(&key) {
                    return Some(entry);
                }
            }
        }
        None
    }
}

#[derive(Debug, Clone)]
struct MapSettings {
    online_tiles: bool,
    tile_url: String,
    tile_attribution: String,
    default_lat: f64,
    default_lon: f64,
    default_zoom: u8,
    min_zoom: u8,
    max_zoom: u8,
    cache_dir: PathBuf,
}

impl Default for MapSettings {
    fn default() -> Self {
        Self {
            online_tiles: true,
            tile_url: DEFAULT_TILE_URL.to_string(),
            tile_attribution: DEFAULT_TILE_ATTRIBUTION.to_string(),
            default_lat: 52.3759,
            default_lon: 9.7320,
            default_zoom: 13,
            min_zoom: 3,
            max_zoom: 18,
            cache_dir: default_tile_cache_dir(),
        }
    }
}

impl MapSettings {
    fn from_config(config: Option<&MapSettingsConfig>) -> Self {
        let mut settings = Self::default();
        if let Some(config) = config {
            if let Some(value) = config.online_tiles { settings.online_tiles = value; }
            if let Some(value) = config.tile_url.clone().filter(|v| !v.trim().is_empty()) { settings.tile_url = value; }
            if let Some(value) = config.tile_attribution.clone() { settings.tile_attribution = value; }
            if let Some(value) = config.default_lat { settings.default_lat = value; }
            if let Some(value) = config.default_lon { settings.default_lon = value; }
            if let Some(value) = config.default_zoom { settings.default_zoom = value.clamp(1, 19); }
            if let Some(value) = config.min_zoom { settings.min_zoom = value.clamp(1, 19); }
            if let Some(value) = config.max_zoom { settings.max_zoom = value.clamp(1, 19); }
            if settings.min_zoom > settings.max_zoom {
                std::mem::swap(&mut settings.min_zoom, &mut settings.max_zoom);
            }
            settings.default_zoom = settings.default_zoom.clamp(settings.min_zoom, settings.max_zoom);
            if let Some(path) = config.cache_dir.clone() { settings.cache_dir = path; }
        }
        settings
    }
}

fn default_tile_cache_dir() -> PathBuf {
    dirs_next::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("netcore")
        .join("control-room")
        .join("tiles")
}

#[derive(Debug, Default, Clone, Deserialize)]
struct ProfileConfig {
    api: Option<String>,
    username: Option<String>,
    default_node: Option<String>,
    operator_id: Option<String>,
}

#[derive(Debug, Default)]
struct CliArgs {
    api: Option<String>,
    username: Option<String>,
    config: Option<PathBuf>,
    profile: String,
}

impl ResolvedSettings {
    fn load() -> (Self, Option<String>) {
        let cli = parse_args();
        let (_config_path, config, warning) = load_operator_config(cli.config.as_deref());
        let profile = config
            .profiles
            .get(&cli.profile)
            .or_else(|| config.profiles.get(DEFAULT_PROFILE));

        let api = cli
            .api
            .clone()
            .or_else(|| env_nonempty("NETCORE_CONTROL_ROOM_API"))
            .or_else(|| profile.and_then(|profile| profile.api.clone()))
            .unwrap_or_else(|| DEFAULT_API.to_string());

        let username = if let Some(username) = cli.username.clone().filter(|username| !username.trim().is_empty()) {
            Some(username)
        } else if let Some(username) = env_nonempty("NETCORE_CONTROL_ROOM_USER") {
            Some(username)
        } else if let Some(username) = profile.and_then(|profile| profile.username.clone()).filter(|username| !username.trim().is_empty()) {
            Some(username)
        } else {
            None
        };

        let default_node = env_nonempty("NETCORE_CONTROL_ROOM_NODE_ID")
            .or_else(|| profile.and_then(|profile| profile.default_node.clone()))
            .unwrap_or_else(|| DEFAULT_NODE.to_string());

        let operator_id = env_nonempty("NETCORE_CONTROL_ROOM_OPERATOR_ID")
            .or_else(|| profile.and_then(|profile| profile.operator_id.clone()))
            .unwrap_or_else(|| DEFAULT_OPERATOR.to_string());

        let map = MapSettings::from_config(config.ui.as_ref().and_then(|ui| ui.map.as_ref()));
        let directory = DirectorySettings::from_config(config.directory.as_ref());

        (
            Self {
                profile: cli.profile,
                api,
                username,
                default_node,
                operator_id,
                map,
                directory,
            },
            warning,
        )
    }
}

fn parse_args() -> CliArgs {
    let mut args = CliArgs {
        profile: DEFAULT_PROFILE.to_string(),
        ..Default::default()
    };
    let mut iter = env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--api" => args.api = iter.next(),
            "--username" => args.username = iter.next(),
            "--config" => args.config = iter.next().map(PathBuf::from),
            "--profile" => args.profile = iter.next().unwrap_or_else(|| DEFAULT_PROFILE.to_string()),
            "--help" | "-h" => {
                println!("NetCore Control Room Operator UI");
                println!("  --api <url>");
                println!("  --username <user>");
                println!("  --config <operator.toml>");
                println!("  --profile <name>");
                std::process::exit(0);
            }
            _ => {}
        }
    }
    args
}

fn load_operator_config(explicit_path: Option<&Path>) -> (Option<PathBuf>, OperatorConfig, Option<String>) {
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
            match fs::read_to_string(&path) {
                Ok(text) => match toml::from_str::<OperatorConfig>(&text) {
                    Ok(config) => return (Some(path), config, None),
                    Err(error) => {
                        return (
                            Some(path.clone()),
                            OperatorConfig::default(),
                            Some(format!("operator config konnte nicht gelesen werden: {}: {error}", path.display())),
                        )
                    }
                },
                Err(error) => {
                    return (
                        Some(path.clone()),
                        OperatorConfig::default(),
                        Some(format!("operator config konnte nicht geöffnet werden: {}: {error}", path.display())),
                    )
                }
            }
        }
    }

    (None, OperatorConfig::default(), Some("kein operator.toml gefunden; UI nutzt Defaults oder CLI/env Werte".to_string()))
}

fn default_user_config_path() -> PathBuf {
    dirs_next::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("netcore")
        .join("control-room")
        .join("operator.toml")
}

fn system_config_path() -> PathBuf {
    PathBuf::from("/etc/netcore-control-room/operator.toml")
}

fn env_nonempty(name: &str) -> Option<String> {
    env::var(name).ok().map(|value| value.trim().to_string()).filter(|value| !value.is_empty())
}


#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
enum Tab {
    Overview,
    Subscribers,
    Groups,
    Calls,
    Sds,
    Locations,
    Map,
    StatusTableau,
    Commands,
    AdminUsers,
    Raw,
}

impl Tab {
    const ALL: [Tab; 11] = [
        Tab::Overview,
        Tab::Subscribers,
        Tab::Groups,
        Tab::Calls,
        Tab::Sds,
        Tab::Locations,
        Tab::Map,
        Tab::StatusTableau,
        Tab::Commands,
        Tab::AdminUsers,
        Tab::Raw,
    ];

    fn label(self) -> &'static str {
        match self {
            Tab::Overview => "Übersicht",
            Tab::Subscribers => "Teilnehmer",
            Tab::Groups => "Gruppen",
            Tab::Calls => "Rufe",
            Tab::Sds => "SDS",
            Tab::Locations => "Standorte",
            Tab::Map => "Karte",
            Tab::StatusTableau => "Status",
            Tab::Commands => "Commands",
            Tab::AdminUsers => "Admin/User",
            Tab::Raw => "Raw JSON",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Tab::Overview => "⌂",
            Tab::Subscribers => "●",
            Tab::Groups => "▦",
            Tab::Calls => "☎",
            Tab::Sds => "✉",
            Tab::Locations => "⌖",
            Tab::Map => "◎",
            Tab::StatusTableau => "▦",
            Tab::Commands => "⚡",
            Tab::AdminUsers => "⚙",
            Tab::Raw => "{}",
        }
    }
}

struct ApiClient {
    base: String,
    username: Option<String>,
    password: Option<String>,
    http: reqwest::blocking::Client,
}

impl ApiClient {
    fn new(settings: &ResolvedSettings) -> Self {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(4))
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());
        Self {
            base: settings.api.trim_end_matches('/').to_string(),
            username: None,
            password: None,
            http,
        }
    }

    fn get(&self, path: &str) -> Result<Value, String> {
        let url = self.url(path);
        let request = self.with_auth(self.http.get(&url));
        self.read(url, request.send().map_err(|error| error.to_string())?)
    }

    fn post<T: Serialize + ?Sized>(&self, path: &str, body: &T) -> Result<Value, String> {
        let url = self.url(path);
        let request = self.with_auth(self.http.post(&url).json(body));
        self.read(url, request.send().map_err(|error| error.to_string())?)
    }

    fn patch<T: Serialize + ?Sized>(&self, path: &str, body: &T) -> Result<Value, String> {
        let url = self.url(path);
        let request = self.with_auth(self.http.patch(&url).json(body));
        self.read(url, request.send().map_err(|error| error.to_string())?)
    }

    fn delete(&self, path: &str) -> Result<Value, String> {
        let url = self.url(path);
        let request = self.with_auth(self.http.delete(&url));
        self.read(url, request.send().map_err(|error| error.to_string())?)
    }

    fn set_login(&mut self, username: String, password: String) {
        self.username = Some(username);
        self.password = Some(password);
    }

    fn clear_login(&mut self) {
        self.username = None;
        self.password = None;
    }

    fn login(&self, username: &str, password: &str) -> Result<Value, String> {
        let url = self.url("/api/login");
        let body = json!({ "username": username, "password": password });
        self.read(url.clone(), self.http.post(&url).json(&body).send().map_err(|error| error.to_string())?)
    }

    fn with_auth(&self, request: reqwest::blocking::RequestBuilder) -> reqwest::blocking::RequestBuilder {
        match (self.username.as_ref(), self.password.as_ref()) {
            (Some(username), Some(password)) => request.basic_auth(username, Some(password)),
            _ => request,
        }
    }

    fn read(&self, url: String, response: reqwest::blocking::Response) -> Result<Value, String> {
        let status = response.status();
        let body = response.text().map_err(|error| error.to_string())?;
        if !status.is_success() {
            let trimmed = body.trim();
            if trimmed.is_empty() {
                return Err(format!("{} für {}", status, url));
            }
            return Err(format!("{} für {}: {}", status, url, trimmed));
        }
        if body.trim().is_empty() {
            return Ok(json!({}));
        }
        serde_json::from_str(&body).map_err(|error| format!("JSON Fehler von {url}: {error}; body={body}"))
    }

    fn url(&self, path: &str) -> String {
        if path.starts_with('/') {
            format!("{}{}", self.base, path)
        } else {
            format!("{}/{}", self.base, path)
        }
    }
}

struct ControlRoomApp {
    settings: ResolvedSettings,
    api: ApiClient,
    tab: Tab,
    auto_refresh: bool,
    refresh_seconds: f32,
    last_refresh: Option<Instant>,
    last_ok: Option<String>,
    last_error: Option<String>,
    startup_warning: Option<String>,

    overview: Option<Value>,
    subscribers: Option<Value>,
    groups: Option<Value>,
    calls: Option<Value>,
    sds: Option<Value>,
    locations: Option<Value>,
    commands: Option<Value>,
    emergencies: Option<Value>,
    admin_users: Option<Value>,
    current_user: Option<Value>,
    logged_in: bool,
    login_username: String,
    login_password: String,
    login_result: Option<String>,

    kick_issi: String,
    dgna_issi: String,
    dgna_gssi: String,
    dgna_detach: bool,
    clear_issi: String,
    command_result: Option<String>,

    new_user_username: String,
    new_user_display_name: String,
    new_user_password: String,
    new_user_role: String,
    user_result: Option<String>,

    detached_windows: HashMap<Tab, bool>,
    window_mode: bool,
    map_follow_latest: bool,
    map_tiles_enabled: bool,
    map_zoom_adjust: i32,
    map_wheel_zoom_accum: f32,
    map_manual_center: Option<(f64, f64)>,
    selected_location_issi: Option<u64>,
    status_card_offsets: HashMap<String, egui::Vec2>,
    map_cache: MapTileCache,
    local_directory: DirectorySettings,
    raw_directory: Option<Value>,
    directory_source: String,
    directory_name_index: HashMap<u64, String>,
}


impl ControlRoomApp {
    fn new(settings: ResolvedSettings, startup_warning: Option<String>) -> Self {
        let api = ApiClient::new(&settings);
        let local_directory = settings.directory.clone();
        let login_username = settings
            .username
            .clone()
            .unwrap_or_else(|| settings.operator_id.clone());
        Self {
            kick_issi: String::new(),
            dgna_issi: String::new(),
            dgna_gssi: String::new(),
            dgna_detach: false,
            clear_issi: String::new(),
            command_result: None,
            new_user_username: String::new(),
            new_user_display_name: String::new(),
            new_user_password: String::new(),
            new_user_role: "viewer".to_string(),
            user_result: None,
            map_tiles_enabled: settings.map.online_tiles,
            map_zoom_adjust: 0,
            map_wheel_zoom_accum: 0.0,
            map_manual_center: None,
            selected_location_issi: None,
            status_card_offsets: HashMap::new(),
            map_cache: MapTileCache::new(settings.map.clone()),
            local_directory,
            raw_directory: None,
            directory_source: "operator.toml".to_string(),
            directory_name_index: HashMap::new(),
            settings,
            api,
            tab: Tab::Overview,
            auto_refresh: true,
            refresh_seconds: 1.0,
            last_refresh: None,
            last_ok: None,
            last_error: None,
            startup_warning,
            overview: None,
            subscribers: None,
            groups: None,
            calls: None,
            sds: None,
            locations: None,
            commands: None,
            emergencies: None,
            admin_users: None,
            current_user: None,
            logged_in: false,
            login_username,
            login_password: String::new(),
            login_result: None,
            detached_windows: HashMap::new(),
            window_mode: false,
            map_follow_latest: true,
        }
    }

    fn current_role(&self) -> &str {
        self.current_user
            .as_ref()
            .and_then(|value| str_at(value, &["user", "role"]).or_else(|| str_at(value, &["role"])))
            .unwrap_or("viewer")
    }

    fn role_label(&self) -> String {
        match self.current_role() {
            "admin" => "Admin".to_string(),
            "operator" => "Operator".to_string(),
            "viewer" => "Viewer".to_string(),
            other => other.to_string(),
        }
    }

    fn is_admin(&self) -> bool {
        self.current_role() == "admin"
    }

    fn can_operate(&self) -> bool {
        matches!(self.current_role(), "admin" | "operator")
    }

    fn can_access_tab(&self, tab: Tab) -> bool {
        match tab {
            Tab::AdminUsers | Tab::Raw => self.is_admin(),
            Tab::Commands => self.can_operate(),
            _ => true,
        }
    }

    fn visible_tabs(&self) -> Vec<Tab> {
        Tab::ALL
            .iter()
            .copied()
            .filter(|tab| self.can_access_tab(*tab))
            .collect()
    }

    fn enforce_rbac_view(&mut self) {
        if !self.can_access_tab(self.tab) {
            self.tab = Tab::Overview;
        }
        let denied = self
            .detached_windows
            .keys()
            .copied()
            .filter(|tab| !self.can_access_tab(*tab))
            .collect::<Vec<_>>();
        for tab in denied {
            self.detached_windows.insert(tab, false);
        }
    }

    fn refresh_all(&mut self) {
        let mut errors = Vec::new();

        match self.api.get("/api/me") {
            Ok(value) => self.current_user = Some(value),
            Err(error) => self.current_user = Some(json!({ "error": error })),
        }
        self.enforce_rbac_view();

        self.refresh_directory(&mut errors);
        self.get_into("/api/overview", DataSlot::Overview, &mut errors);
        self.get_into("/api/subscribers", DataSlot::Subscribers, &mut errors);
        self.get_into("/api/groups", DataSlot::Groups, &mut errors);
        self.get_into("/api/calls", DataSlot::Calls, &mut errors);
        self.get_into("/api/sds?limit=50", DataSlot::Sds, &mut errors);
        self.get_into("/api/locations", DataSlot::Locations, &mut errors);
        self.get_into("/api/emergencies", DataSlot::Emergencies, &mut errors);

        if self.can_operate() {
            self.get_into("/api/commands?limit=50", DataSlot::Commands, &mut errors);
        } else {
            self.commands = None;
            self.command_result = None;
        }

        if self.is_admin() {
            match self.api.get("/api/admin/users") {
                Ok(value) => self.admin_users = Some(value),
                Err(error) => self.admin_users = Some(json!({ "error": error })),
            }
        } else {
            self.admin_users = None;
            self.user_result = None;
        }

        self.last_refresh = Some(Instant::now());
        if errors.is_empty() {
            self.last_error = None;
            self.last_ok = Some(now_label());
        } else {
            self.last_error = Some(errors.join("\n"));
        }
    }


    fn refresh_directory(&mut self, errors: &mut Vec<String>) {
        match self.api.get("/api/directory/resolved") {
            Ok(value) => {
                self.raw_directory = value.get("directory").cloned().or_else(|| Some(value.clone()));
                self.directory_name_index = build_directory_name_index(&value);
                let directory_value = value.get("directory").cloned().unwrap_or_else(|| value.clone());
                let normalized = normalize_directory_value(directory_value);

                match serde_json::from_value::<DirectoryConfig>(normalized) {
                    Ok(config) => {
                        let mut directory = DirectorySettings::from_config(Some(&config));
                        directory.fill_missing_from(&self.local_directory);
                        self.settings.directory = directory;
                        self.directory_source = format!(
                            "LXC /api/directory/resolved · {} Namen",
                            self.directory_name_index.len()
                        );
                    }
                    Err(error) => {
                        self.settings.directory = self.local_directory.clone();
                        self.directory_source = format!(
                            "LXC /api/directory/resolved raw · {} Namen",
                            self.directory_name_index.len()
                        );
                        errors.push(format!("/api/directory/resolved: Directory konnte nicht gelesen werden: {error}"));
                    }
                }
                return;
            }
            Err(error) => {
                if !error.contains("404") {
                    errors.push(format!("/api/directory/resolved: {error}"));
                }
            }
        }

        match self.api.get("/api/directory") {
            Ok(raw_value) => {
                let deep_index = build_directory_name_index(&raw_value);
                let normalized = normalize_directory_value(raw_value);
                self.raw_directory = Some(normalized.clone());

                match serde_json::from_value::<DirectoryConfig>(normalized) {
                    Ok(config) => {
                        let mut directory = DirectorySettings::from_config(Some(&config));
                        directory.fill_missing_from(&self.local_directory);
                        self.settings.directory = directory;
                        self.directory_name_index = deep_index;
                        self.directory_source = format!("LXC /api/directory · {} Namen", self.directory_name_index.len());
                    }
                    Err(error) => {
                        self.directory_name_index = deep_index;
                        errors.push(format!("/api/directory: Directory konnte nicht gelesen werden: {error}"));
                        self.settings.directory = self.local_directory.clone();
                        self.directory_source = format!("LXC /api/directory raw · {} Namen", self.directory_name_index.len());
                    }
                }
            }
            Err(error) => {
                self.raw_directory = None;
                self.settings.directory = self.local_directory.clone();
                self.directory_name_index.clear();
                self.directory_source = "operator.toml lokal".to_string();
                if !error.contains("404") {
                    errors.push(format!("/api/directory: {error}"));
                }
            }
        }
    }


    fn get_into(&mut self, path: &str, slot: DataSlot, errors: &mut Vec<String>) {
        match self.api.get(path) {
            Ok(value) => self.set_slot(slot, value),
            Err(error) => errors.push(format!("{path}: {error}")),
        }
    }

    fn set_slot(&mut self, slot: DataSlot, value: Value) {
        match slot {
            DataSlot::Overview => self.overview = Some(value),
            DataSlot::Subscribers => self.subscribers = Some(value),
            DataSlot::Groups => self.groups = Some(value),
            DataSlot::Calls => self.calls = Some(value),
            DataSlot::Sds => self.sds = Some(value),
            DataSlot::Locations => self.locations = Some(value),
            DataSlot::Commands => self.commands = Some(value),
            DataSlot::Emergencies => self.emergencies = Some(value),
        }
    }

    fn send_kick(&mut self) {
        if !self.can_operate() {
            self.command_result = Some("Kein Zugriff: deine Rolle darf keine Befehle senden".to_string());
            return;
        }
        let issi = match parse_u32(&self.kick_issi, "ISSI") {
            Ok(value) => value,
            Err(error) => {
                self.command_result = Some(error);
                return;
            }
        };
        let body = json!({ "operator_id": self.settings.operator_id.clone(), "issi": issi });
        self.command_result = Some(match self.api.post(&format!("/api/nodes/{}/commands/kick", self.settings.default_node), &body) {
            Ok(value) => pretty(&value),
            Err(error) => error,
        });
        self.refresh_all();
    }

    fn send_dgna(&mut self) {
        if !self.can_operate() {
            self.command_result = Some("Kein Zugriff: deine Rolle darf keine Befehle senden".to_string());
            return;
        }
        let issi = match parse_u32(&self.dgna_issi, "ISSI") {
            Ok(value) => value,
            Err(error) => {
                self.command_result = Some(error);
                return;
            }
        };
        let gssi = match parse_u32(&self.dgna_gssi, "GSSI") {
            Ok(value) => value,
            Err(error) => {
                self.command_result = Some(error);
                return;
            }
        };
        let body = json!({
            "operator_id": self.settings.operator_id.clone(),
            "issi": issi,
            "gssi": gssi,
            "attach": !self.dgna_detach,
        });
        self.command_result = Some(match self.api.post(&format!("/api/nodes/{}/commands/dgna", self.settings.default_node), &body) {
            Ok(value) => pretty(&value),
            Err(error) => error,
        });
        self.refresh_all();
    }

    fn send_clear_emergency(&mut self) {
        if !self.can_operate() {
            self.command_result = Some("Kein Zugriff: deine Rolle darf keine Befehle senden".to_string());
            return;
        }
        let issi = if self.clear_issi.trim().is_empty() {
            0
        } else {
            match parse_u32(&self.clear_issi, "ISSI") {
                Ok(value) => value,
                Err(error) => {
                    self.command_result = Some(error);
                    return;
                }
            }
        };
        let body = json!({ "operator_id": self.settings.operator_id.clone(), "issi": issi });
        self.command_result = Some(match self.api.post(&format!("/api/nodes/{}/commands/clear-emergency", self.settings.default_node), &body) {
            Ok(value) => pretty(&value),
            Err(error) => error,
        });
        self.refresh_all();
    }

    fn login(&mut self) {
        let username = self.login_username.trim().to_string();
        let password = self.login_password.clone();
        if username.is_empty() || password.is_empty() {
            self.login_result = Some("Benutzername und Passwort sind erforderlich".to_string());
            return;
        }
        match self.api.login(&username, &password) {
            Ok(value) => {
                self.api.set_login(username, password);
                self.current_user = Some(value);
                self.logged_in = true;
                self.login_result = None;
                self.refresh_all();
            }
            Err(error) => self.login_result = Some(error),
        }
    }

    fn logout(&mut self) {
        self.api.clear_login();
        self.logged_in = false;
        self.login_password.clear();
        self.current_user = None;
        self.overview = None;
        self.subscribers = None;
        self.groups = None;
        self.calls = None;
        self.sds = None;
        self.locations = None;
        self.commands = None;
        self.emergencies = None;
        self.admin_users = None;
    }

    fn create_user(&mut self) {
        if !self.is_admin() {
            self.user_result = Some("Kein Zugriff: nur Admins dürfen Benutzer anlegen".to_string());
            return;
        }
        let username = self.new_user_username.trim();
        if username.is_empty() {
            self.user_result = Some("Benutzername fehlt".to_string());
            return;
        }
        if self.new_user_password.trim().len() < 6 {
            self.user_result = Some("Passwort muss mindestens 6 Zeichen haben".to_string());
            return;
        }
        let body = json!({
            "username": username,
            "display_name": self.new_user_display_name.trim(),
            "password": self.new_user_password.clone(),
            "role": self.new_user_role.trim(),
            "enabled": true,
            "created_by": self.login_username.trim(),
        });
        self.user_result = Some(match self.api.post("/api/admin/users", &body) {
            Ok(value) => pretty(&value),
            Err(error) => error,
        });
        self.new_user_password.clear();
        self.refresh_all();
    }

    fn set_user_enabled(&mut self, username: &str, enabled: bool) {
        if !self.is_admin() {
            self.user_result = Some("Kein Zugriff: nur Admins dürfen Benutzer ändern".to_string());
            return;
        }
        let body = json!({ "enabled": enabled });
        self.user_result = Some(match self.api.patch(&format!("/api/admin/users/{username}"), &body) {
            Ok(value) => pretty(&value),
            Err(error) => error,
        });
        self.refresh_all();
    }

    fn delete_user(&mut self, username: &str) {
        if !self.is_admin() {
            self.user_result = Some("Kein Zugriff: nur Admins dürfen Benutzer löschen".to_string());
            return;
        }
        self.user_result = Some(match self.api.delete(&format!("/api/admin/users/{username}")) {
            Ok(value) => pretty(&value),
            Err(error) => error,
        });
        self.refresh_all();
    }
}

#[derive(Debug, Copy, Clone)]
enum DataSlot {
    Overview,
    Subscribers,
    Groups,
    Calls,
    Sds,
    Locations,
    Commands,
    Emergencies,
}

impl eframe::App for ControlRoomApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.style_mut(|style| {
            style.spacing.item_spacing = egui::vec2(8.0, 7.0);
            style.spacing.button_padding = egui::vec2(12.0, 7.0);
            style.spacing.text_edit_width = 260.0;
            style.visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(242, 245, 248);
            style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(235, 239, 244);
            style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(220, 232, 246);
            style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(199, 220, 244);
            style.visuals.selection.bg_fill = egui::Color32::from_rgb(0, 118, 214);
        });

        if !self.logged_in {
            self.render_login_screen(ctx);
            return;
        }
        if self.overview.is_none() {
            self.refresh_all();
        }
        self.auto_refresh = true;
        self.refresh_seconds = 1.0;

        if self.auto_refresh {
            let due = self
                .last_refresh
                .map(|instant| instant.elapsed() >= Duration::from_secs_f32(self.refresh_seconds.max(1.0)))
                .unwrap_or(true);
            if due {
                self.refresh_all();
            }
            ctx.request_repaint_after(Duration::from_millis(250));
        }

        let screen_width = ctx.input(|input| input.screen_rect().width());
        let compact_layout = screen_width < 1280.0;
        let side_width = if screen_width < 1180.0 {
            185.0
        } else if screen_width < 1450.0 {
            205.0
        } else {
            225.0
        };

        egui::TopBottomPanel::top("top_bar")
            .resizable(false)
            .show(ctx, |ui| {
                self.render_elz_header(ui, screen_width, compact_layout);
            });

        egui::SidePanel::left("leitstelle_nav")
            .resizable(true)
            .default_width(side_width)
            .min_width(180.0)
            .max_width(320.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    ui.add_space(4.0);
                    ui.heading("Module");
                    ui.small(format!("angemeldet: {} / {}", self.login_username, self.role_label()));
                    ui.separator();
                    for tab in self.visible_tabs() {
                        self.render_nav_button(ui, tab);
                    }
                    ui.separator();
                    ui.checkbox(&mut self.window_mode, "OS-Fenster");
                    ui.small("Module auf andere Monitore ziehen");
                    if ui.add_sized([ui.available_width(), 30.0], egui::Button::new("Alle erlaubten Fenster öffnen")).clicked() {
                        self.window_mode = true;
                        for tab in self.visible_tabs() {
                            if tab != Tab::Raw {
                                self.detached_windows.insert(tab, true);
                            }
                        }
                    }
                    if ui.add_sized([ui.available_width(), 30.0], egui::Button::new("Fenster schließen")).clicked() {
                        self.detached_windows.clear();
                    }
                    ui.separator();
                    if self.can_operate() {
                        self.render_command_box(ui);
                    } else {
                        ui.heading("Lesezugriff");
                        ui.small("Keine Funkbefehle mit dieser Rolle.");
                    }
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                self.render_module_content(ui, self.tab);
            });
        });

        self.render_detached_windows(ctx);
    }
}

impl ControlRoomApp {
    fn render_elz_header(&mut self, ui: &mut egui::Ui, screen_width: f32, compact_layout: bool) {
        let compact = compact_layout || screen_width < 1350.0;

        egui::Frame::none()
            .fill(egui::Color32::from_rgb(0, 72, 128))
            .inner_margin(egui::Margin::symmetric(8.0, 5.0))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.colored_label(
                        egui::Color32::WHITE,
                        egui::RichText::new("NetCore Control Room").strong().size(if compact { 18.0 } else { 20.0 }),
                    );
                    ui.separator();
                    ui.colored_label(egui::Color32::from_rgb(220, 238, 255), UI_VERSION_LABEL);
                    ui.separator();
                    ui.colored_label(egui::Color32::WHITE, format!("{} · {}", self.login_username, self.role_label()));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add_sized([82.0, 26.0], egui::Button::new("Logout")).clicked() {
                            self.logout();
                        }
                        ui.small("Live · 1s");
                    });
                });
            });

        egui::Frame::none()
            .fill(egui::Color32::from_rgb(232, 238, 246))
            .inner_margin(egui::Margin::symmetric(8.0, 4.0))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal_wrapped(|ui| {
                    if ui.add_sized([92.0, 28.0], egui::Button::new("Übersicht")).clicked() {
                        self.tab = Tab::Overview;
                    }
                    if ui.add_sized([100.0, 28.0], egui::Button::new("Teilnehmer")).clicked() {
                        self.tab = Tab::Subscribers;
                    }
                    if ui.add_sized([84.0, 28.0], egui::Button::new("Gruppen")).clicked() {
                        self.tab = Tab::Groups;
                    }
                    if ui.add_sized([72.0, 28.0], egui::Button::new("Rufe")).clicked() {
                        self.tab = Tab::Calls;
                    }
                    if ui.add_sized([64.0, 28.0], egui::Button::new("SDS")).clicked() {
                        self.tab = Tab::Sds;
                    }
                    if ui.add_sized([72.0, 28.0], egui::Button::new("Karte")).clicked() {
                        self.tab = Tab::Map;
                    }
                    if ui.add_sized([78.0, 28.0], egui::Button::new("Status")).clicked() {
                        self.tab = Tab::StatusTableau;
                    }
                    if self.can_operate() {
                        if ui.add_sized([92.0, 28.0], egui::Button::new("Befehle")).clicked() {
                            self.tab = Tab::Commands;
                        }
                    }
                    if self.is_admin() {
                        if ui.add_sized([96.0, 28.0], egui::Button::new("Benutzer")).clicked() {
                            self.tab = Tab::AdminUsers;
                        }
                    }
                    ui.separator();
                    if ui.add_sized([96.0, 28.0], egui::Button::new("Maske leer")).clicked() {
                        self.clear_command_inputs();
                    }
                    if ui.add_sized([92.0, 28.0], egui::Button::new("Aktualisieren")).clicked() {
                        self.refresh_all();
                    }
                });
            });

        egui::Frame::none()
            .fill(egui::Color32::from_rgb(248, 250, 252))
            .inner_margin(egui::Margin::symmetric(8.0, 4.0))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal_wrapped(|ui| {
                    if let Some(overview) = &self.overview {
                        let connected = u64_at(overview, &["nodes_connected"]).unwrap_or(0);
                        let total = u64_at(overview, &["node_count"]).unwrap_or(0);
                        status_pill(ui, "TBS", &format!("{connected}/{total}"), connected > 0);
                        status_pill(ui, "Teilnehmer", &format!("{}/{}", u64_at(overview, &["subscribers_online"]).unwrap_or(0), u64_at(overview, &["subscribers_total"]).unwrap_or(0)), true);
                        status_pill(ui, "Rufe", &u64_at(overview, &["active_calls_total"]).unwrap_or(0).to_string(), true);
                        status_pill(ui, "Notrufe", &u64_at(overview, &["emergencies_active"]).unwrap_or(0).to_string(), u64_at(overview, &["emergencies_active"]).unwrap_or(0) == 0);
                    } else {
                        status_pill(ui, "Status", "keine Daten", false);
                    }

                    ui.separator();

                    if let Some(error) = &self.last_error {
                        ui.colored_label(egui::Color32::RED, error);
                    } else if let Some(ok) = &self.last_ok {
                        ui.small(format!("Stand: {ok}"));
                    } else {
                        ui.small("Live-Aktualisierung aktiv");
                    }
                });
            });
    }

    fn render_nav_button(&mut self, ui: &mut egui::Ui, tab: Tab) {
        let selected = self.tab == tab;
        let row_height = 32.0;
        let row_width = ui.available_width().max(160.0);

        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(row_width, row_height),
            egui::Sense::click(),
        );

        let arrow_size = egui::vec2(28.0, 24.0);
        let arrow_rect = egui::Rect::from_center_size(
            egui::pos2(rect.right() - arrow_size.x * 0.5 - 4.0, rect.center().y),
            arrow_size,
        );

        let label_rect = egui::Rect::from_min_max(
            rect.min,
            egui::pos2(arrow_rect.left() - 8.0, rect.max.y),
        );

        let pointer_pos = ui.input(|input| input.pointer.interact_pos());
        let clicked_arrow = response.clicked()
            && pointer_pos.map(|pos| arrow_rect.contains(pos)).unwrap_or(false);
        let clicked_label = response.clicked() && !clicked_arrow;

        if clicked_label {
            self.tab = tab;
        }

        if clicked_arrow {
            let is_open = *self.detached_windows.get(&tab).unwrap_or(&false);
            self.detached_windows.insert(tab, !is_open);
            self.window_mode = true;
        }

        let fill = if selected {
            egui::Color32::from_rgb(0, 118, 214)
        } else if response.hovered() {
            egui::Color32::from_rgb(220, 232, 246)
        } else {
            egui::Color32::TRANSPARENT
        };

        if selected || response.hovered() {
            ui.painter().rect_filled(rect.shrink2(egui::vec2(2.0, 2.0)), egui::Rounding::same(4.0), fill);
        }

        let text_color = if selected {
            egui::Color32::WHITE
        } else {
            ui.visuals().text_color()
        };

        ui.painter().text(
            label_rect.left_center() + egui::vec2(10.0, 0.0),
            egui::Align2::LEFT_CENTER,
            format!("{}  {}", tab.icon(), tab.label()),
            egui::FontId::proportional(14.0),
            text_color,
        );

        let is_open = *self.detached_windows.get(&tab).unwrap_or(&false);
        let arrow_fill = if arrow_rect.contains(pointer_pos.unwrap_or(egui::pos2(-1.0, -1.0))) {
            egui::Color32::from_rgb(210, 220, 232)
        } else {
            egui::Color32::from_rgb(230, 234, 240)
        };
        ui.painter().rect_filled(arrow_rect, egui::Rounding::same(4.0), arrow_fill);
        ui.painter().text(
            arrow_rect.center(),
            egui::Align2::CENTER_CENTER,
            if is_open { "▣" } else { "↗" },
            egui::FontId::proportional(13.0),
            ui.visuals().text_color(),
        );

        response.on_hover_text("Modul öffnen · Pfeil: als OS-Fenster öffnen/schließen");
    }

    fn clear_command_inputs(&mut self) {
        self.kick_issi.clear();
        self.dgna_issi.clear();
        self.dgna_gssi.clear();
        self.clear_issi.clear();
        self.dgna_detach = false;
        self.command_result = None;
        self.last_ok = Some("Maske geleert".to_string());
    }

    fn render_module_content(&mut self, ui: &mut egui::Ui, tab: Tab) {
        if !self.can_access_tab(tab) {
            ui.heading("Kein Zugriff");
            ui.label(format!("Deine Rolle '{}' darf dieses Modul nicht öffnen.", self.current_role()));
            return;
        }
        match tab {
            Tab::Overview => self.render_overview(ui),
            Tab::Subscribers => self.render_subscribers(ui),
            Tab::Groups => self.render_groups(ui),
            Tab::Calls => self.render_calls(ui),
            Tab::Sds => self.render_sds(ui),
            Tab::Locations => self.render_locations(ui),
            Tab::Map => self.render_map(ui),
            Tab::StatusTableau => self.render_status_tableau(ui),
            Tab::Commands => self.render_commands(ui),
            Tab::AdminUsers => self.render_admin_users(ui),
            Tab::Raw => self.render_raw(ui),
        }
    }

    fn render_detached_windows(&mut self, ctx: &egui::Context) {
        if !self.window_mode {
            return;
        }

        let open_tabs = Tab::ALL
            .iter()
            .copied()
            .filter(|tab| *self.detached_windows.get(tab).unwrap_or(&false))
            .filter(|tab| self.can_access_tab(*tab))
            .collect::<Vec<_>>();

        for tab in open_tabs {
            let mut close_requested = false;
            let title = format!("{} – NetCore Control Room", tab.label());
            let default_size = match tab {
                Tab::Map => [1100.0, 760.0],
                Tab::Overview => [1180.0, 760.0],
                Tab::AdminUsers => [1050.0, 720.0],
                Tab::Raw => [980.0, 720.0],
                _ => [900.0, 640.0],
            };
            let min_size = match tab {
                Tab::Map => [760.0, 520.0],
                Tab::Overview => [820.0, 520.0],
                _ => [640.0, 440.0],
            };

            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of(format!("netcore_module_os_window_{:?}", tab)),
                egui::ViewportBuilder::default()
                    .with_title(title)
                    .with_inner_size(default_size)
                    .with_min_inner_size(min_size),
                |viewport_ctx, _class| {
                    if viewport_ctx.input(|input| input.viewport().close_requested()) {
                        close_requested = true;
                    }

                    egui::TopBottomPanel::top(format!("module_top_{:?}", tab)).show(viewport_ctx, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.heading(tab.label());
                            ui.separator();
                            ui.small(UI_VERSION_LABEL);
                            ui.separator();
                            ui.small(format!("API: {}", self.settings.api));
                            if ui.button("Refresh").clicked() {
                                self.refresh_all();
                            }
                            if ui.button("Fenster schließen").clicked() {
                                close_requested = true;
                            }
                        });
                        if let Some(error) = &self.last_error {
                            ui.colored_label(egui::Color32::RED, error);
                        }
                    });

                    egui::CentralPanel::default().show(viewport_ctx, |ui| {
                        self.render_module_content(ui, tab);
                    });
                },
            );

            if close_requested {
                self.detached_windows.insert(tab, false);
            }
        }
    }

    fn render_command_box(&mut self, ui: &mut egui::Ui) {
        if !self.can_operate() {
            ui.heading("Lesezugriff");
            ui.small("Deine Rolle darf keine Funkbefehle senden.");
            return;
        }
        ui.heading("Befehle");
        ui.small("nutzt default_node/operator_id aus dem Profil");
        ui.separator();

        ui.label("Kick ISSI");
        ui.add_sized([ui.available_width(), 26.0], egui::TextEdit::singleline(&mut self.kick_issi));
        if ui.add_sized([ui.available_width(), 28.0], egui::Button::new("Kick senden")).clicked() {
            self.send_kick();
        }

        ui.separator();
        ui.label("DGNA");
        ui.label("ISSI");
        ui.add_sized([ui.available_width(), 26.0], egui::TextEdit::singleline(&mut self.dgna_issi));
        ui.label("GSSI");
        ui.add_sized([ui.available_width(), 26.0], egui::TextEdit::singleline(&mut self.dgna_gssi));
        ui.checkbox(&mut self.dgna_detach, "Detach statt Attach");
        if ui.add_sized([ui.available_width(), 28.0], egui::Button::new("DGNA senden")).clicked() {
            self.send_dgna();
        }

        ui.separator();
        ui.label("Emergency Clear");
        ui.add_sized([ui.available_width(), 26.0], egui::TextEdit::singleline(&mut self.clear_issi));
        ui.small("leer/0 = alle");
        if ui.add_sized([ui.available_width(), 28.0], egui::Button::new("Emergency löschen")).clicked() {
            self.send_clear_emergency();
        }

        if let Some(result) = &self.command_result {
            ui.separator();
            ui.label("Letztes Ergebnis:");
            egui::ScrollArea::vertical().max_height(180.0).auto_shrink([false, false]).show(ui, |ui| {
                ui.monospace(result);
            });
        }
    }

    fn render_overview(&self, ui: &mut egui::Ui) {
        ui.heading("Einsatzleitplatz / Funklage");
        let Some(overview) = &self.overview else {
            ui.label("Noch keine Daten");
            return;
        };

        ui.horizontal_wrapped(|ui| {
            metric(ui, "TBS online", format!("{}/{}", u64_at(overview, &["nodes_connected"]).unwrap_or(0), u64_at(overview, &["node_count"]).unwrap_or(0)));
            metric(ui, "Teilnehmer", format!("{}/{}", u64_at(overview, &["subscribers_online"]).unwrap_or(0), u64_at(overview, &["subscribers_total"]).unwrap_or(0)));
            metric(ui, "Gruppen", u64_at(overview, &["groups_total"]).unwrap_or(0).to_string());
            metric(ui, "Aktive Rufe", u64_at(overview, &["active_calls_total"]).unwrap_or(0).to_string());
            metric(ui, "Notrufe", u64_at(overview, &["emergencies_active"]).unwrap_or(0).to_string());
        });

        ui.add_space(8.0);
        ui.columns(2, |columns| {
            columns[0].group(|ui| {
                ui.heading("Basisstationen");
                ui.small("Netzstatus, Carrier, RF-Werte und letzte Meldung");
                ui.separator();
                egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                    egui::Grid::new("nodes_grid_elz").striped(true).min_col_width(72.0).show(ui, |ui| {
                        header_row(ui, &["Node", "Online", "Health", "Carrier", "Subs", "Calls", "Brew", "RF", "Seen"]);
                        for node in array_at(overview, &["nodes"]) {
                            ui.monospace(str_at(node, &["node_id"]).unwrap_or("?"));
                            bool_label(ui, bool_at(node, &["connected"]).unwrap_or(false));
                            ui.label(str_at(node, &["health_overall"]).unwrap_or("?"));
                            ui.label(format!("{} / {}", display_u64(node, &["main_carrier"]), display_u64(node, &["secondary_carrier"])));
                            ui.label(format!("{}/{}", display_u64(node, &["subscribers_online"]), display_u64(node, &["subscribers_total"])));
                            ui.label(display_u64(node, &["active_calls_total"]));
                            tri_label(ui, node.get("brew_connected"));
                            ui.label(format!("{} / {}", display_f64(node, &["rf_peak_dbfs"]), display_f64(node, &["rf_rms_dbfs"])));
                            ui.small(str_at(node, &["last_seen"]).unwrap_or("?"));
                            ui.end_row();
                        }
                    });
                });
            });
            columns[1].group(|ui| {
                ui.heading("Aktuelle Meldungen / Audit");
                ui.small("letzte Commands und Systemreaktionen");
                ui.separator();
                let recent = array_at(overview, &["recent_commands"]);
                if recent.is_empty() {
                    ui.label("Keine aktuellen Commands");
                } else {
                    egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                        for row in recent.iter().take(10) {
                            ui.horizontal_wrapped(|ui| {
                                ui.monospace(str_at(row, &["status"]).unwrap_or("?"));
                                ui.label(str_at(row, &["message"]).unwrap_or(""));
                                ui.small(str_at(row, &["updated_at"]).unwrap_or("?"));
                            });
                            ui.separator();
                        }
                    });
                }
            });
        });
    }

    fn render_subscribers(&self, ui: &mut egui::Ui) {
        ui.heading("Teilnehmer");
        let live_rows = self.subscribers
            .as_ref()
            .map(|value| array_at(value, &["subscribers"]))
            .unwrap_or_default();
        let clean_live_rows = self.clean_subscriber_rows(live_rows.clone());
        let mut display_values: Vec<Value> = clean_live_rows.iter().map(|row| (*row).clone()).collect();
        let mut seen: std::collections::HashSet<u64> = display_values
            .iter()
            .filter_map(|row| u64_at(row, &["issi"]).or_else(|| u64_at(row, &["individual_issi"])))
            .collect();

        let mut directory_only_count = 0usize;
        for issi in self.settings.directory.subscriber_issis() {
            if seen.contains(&issi) {
                continue;
            }
            let pseudo = json!({ "issi": issi, "online": false, "directory_only": true });
            if self.subscriber_is_hidden(&pseudo, issi) {
                continue;
            }
            seen.insert(issi);
            directory_only_count += 1;
            display_values.push(pseudo);
        }

        display_values.sort_by(|left, right| {
            subscriber_online(right).cmp(&subscriber_online(left))
                .then_with(|| u64_at(left, &["issi"]).unwrap_or(0).cmp(&u64_at(right, &["issi"]).unwrap_or(0)))
        });
        let hidden_count = live_rows.len().saturating_sub(clean_live_rows.len());

        ui.horizontal_wrapped(|ui| {
            metric(ui, "sichtbare Geräte", display_values.len().to_string());
            metric(ui, "live", clean_live_rows.len().to_string());
            if directory_only_count > 0 {
                metric(ui, "nur Directory", directory_only_count.to_string());
            }
            if hidden_count > 0 {
                metric(ui, "ausgeblendete/alte Einträge", hidden_count.to_string());
            }
            metric(ui, "Directory", format!("{} Teilnehmer / {} Gruppen / {} Status", self.settings.directory.subscribers.len(), self.settings.directory.groups.len(), self.settings.directory.statuses.len()));
        });
        ui.separator();
        ui.small("Directory-first: bekannte Endgeräte aus dem LXC-/Operator-Directory werden vollständig als Stammdaten genutzt. Live-Daten überschreiben nur Online-/Zeit-/Funkzustände; Infrastruktur und Zombie-Duplikate bleiben ausgeblendet.");
        ui.separator();

        let rows: Vec<&Value> = display_values.iter().collect();
        table(ui, "subscribers_table_directory_first", &["Name", "ISSI", "Typ", "Online", "Status", "Statusgruppe", "Gruppen", "Quelle", "Letztes Signal"], rows, |ui, row| {
            let issi = u64_at(row, &["issi"]).unwrap_or(0);
            ui.label(self.subscriber_display_name(row));
            ui.monospace(if issi > 0 { issi.to_string() } else { "-".to_string() });
            ui.label(self.subscriber_type_label(row));
            bool_label(ui, subscriber_online(row));
            ui.label(self.subscriber_status_label(row));
            ui.label(self.subscriber_status_group_label(row));
            ui.label(self.subscriber_groups_label(row));
            ui.label(if bool_at(row, &["directory_only"]).unwrap_or(false) { "Directory" } else { "Live" });
            ui.small(str_at(row, &["last_seen"]).or_else(|| str_at(row, &["updated_at"])).unwrap_or("-"));
        });
    }

    fn render_groups(&self, ui: &mut egui::Ui) {
        ui.heading("Gruppen");
        let live_rows = self.groups
            .as_ref()
            .map(|value| array_at(value, &["groups"]))
            .unwrap_or_default();
        let mut display_values: Vec<Value> = live_rows.iter().map(|row| (*row).clone()).collect();
        let mut seen: std::collections::HashSet<u64> = display_values
            .iter()
            .filter_map(|row| u64_at(row, &["gssi"]).or_else(|| u64_at(row, &["group"])))
            .collect();
        let mut directory_only_count = 0usize;
        for gssi in self.settings.directory.group_gssis() {
            if seen.contains(&gssi) {
                continue;
            }
            seen.insert(gssi);
            directory_only_count += 1;
            display_values.push(json!({ "gssi": gssi, "directory_only": true }));
        }
        display_values.sort_by(|left, right| u64_at(left, &["gssi"]).unwrap_or(0).cmp(&u64_at(right, &["gssi"]).unwrap_or(0)));
        ui.horizontal_wrapped(|ui| {
            metric(ui, "sichtbare Gruppen", display_values.len().to_string());
            metric(ui, "live", live_rows.len().to_string());
            if directory_only_count > 0 { metric(ui, "nur Directory", directory_only_count.to_string()); }
        });
        ui.separator();
        let rows: Vec<&Value> = display_values.iter().collect();
        table(ui, "groups_table_directory_first", &["Name", "GSSI", "Typ", "Quelle", "Members online", "Members", "Active Call", "Last Update"], rows, |ui, row| {
            let gssi = u64_at(row, &["gssi"]).unwrap_or(0);
            ui.label(self.group_display_name(gssi));
            ui.monospace(if gssi > 0 { gssi.to_string() } else { "-".to_string() });
            ui.label(self.group_type_label(gssi));
            ui.label(if bool_at(row, &["directory_only"]).unwrap_or(false) { "Directory" } else { "Live" });
            ui.label(display_u64(row, &["members_online"]));
            ui.label(self.group_members_label(row));
            ui.label(str_at(row, &["active_call_key"]).unwrap_or("-"));
            ui.small(str_at(row, &["updated_at"]).unwrap_or("-"));
        });
    }

    fn render_calls(&self, ui: &mut egui::Ui) {
        ui.heading("Aktive Rufe");
        let Some(value) = &self.calls else { ui.label("Noch keine Daten"); return; };
        table(ui, "calls_table", &["Key", "Gruppe", "GSSI", "Call ID", "Rufer", "Sprecher", "Carrier", "TS", "Started"], array_at(value, &["calls"]), |ui, row| {
            let gssi = u64_at(row, &["gssi"]).unwrap_or(0);
            let caller = u64_at(row, &["caller_issi"]).unwrap_or(0);
            let speaker = u64_at(row, &["speaker_issi"]).unwrap_or(0);
            ui.monospace(str_at(row, &["key"]).unwrap_or("?"));
            ui.label(self.group_display_name(gssi));
            ui.label(if gssi > 0 { gssi.to_string() } else { "-".to_string() });
            ui.label(display_u64(row, &["call_id"]));
            ui.label(self.format_issi_with_name(caller));
            ui.label(self.format_issi_with_name(speaker));
            ui.label(display_u64(row, &["carrier_num"]));
            ui.label(display_u64(row, &["ts"]));
            ui.small(str_at(row, &["started_at"]).unwrap_or("?"));
        });
    }

    fn render_sds(&self, ui: &mut egui::Ui) {
        ui.heading("SDS / Nachrichten");
        let Some(value) = &self.sds else { ui.label("Noch keine Daten"); return; };
        table(ui, "sds_table", &["Zeit", "Richtung", "Source", "Dest", "Proto", "Text"], array_at(value, &["sds"]), |ui, row| {
            ui.small(str_at(row, &["timestamp"]).or_else(|| str_at(row, &["created_at"])).unwrap_or("?"));
            ui.label(str_at(row, &["direction"]).unwrap_or("?"));
            ui.label(sds_source_issi(row).map(|value| value.to_string()).unwrap_or_else(|| "-".to_string()));
            ui.label(u64_any_at(row, &["dest_issi"]).or_else(|| u64_any_at(row, &["destination_issi"])).or_else(|| u64_any_at(row, &["dest"])).map(|value| value.to_string()).unwrap_or_else(|| "-".to_string()));
            ui.label(sds_protocol(row).map(|value| value.to_string()).unwrap_or_else(|| "-".to_string()));
            ui.label(str_at(row, &["text"]).unwrap_or(""));
        });
    }

    fn render_locations(&mut self, ui: &mut egui::Ui) {
        ui.heading("Standorte");
        let Some(value) = self.locations.clone() else { ui.label("Noch keine Daten"); return; };
        let all_rows = array_at(&value, &["locations"]);
        let rows = latest_location_rows(&all_rows);
        let filtered_count = all_rows.len().saturating_sub(rows.len());
        ui.horizontal_wrapped(|ui| {
            metric(ui, "Aktuelle Geräte", rows.len().to_string());
            if filtered_count > 0 {
                metric(ui, "alte Zombie-Positionen ausgeblendet", filtered_count.to_string());
            }
            let latest = rows.iter().filter_map(|row| str_at(row, &["updated_at"])).max().unwrap_or("-");
            metric(ui, "Letztes Update", latest.to_string());
        });
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            ui.small("Die Live-Karte liegt jetzt nur noch im Tab ‚Karte‘, damit Standorte eine klare Tabellen-/Listenansicht bleibt.");
            if ui.button("Zur Karte wechseln").clicked() {
                self.tab = Tab::Map;
            }
        });
        ui.separator();
        table(ui, "locations_table", &["Gerät", "ISSI", "Typ", "Status", "Latitude", "Longitude", "Source", "Updated"], rows, |ui, row| {
            let issi = u64_at(row, &["issi"]).unwrap_or(0);
            if let Some(subscriber) = self.subscriber_for_issi(issi) {
                ui.label(self.subscriber_display_name(subscriber));
                ui.monospace(issi.to_string());
                ui.label(self.subscriber_type_label(subscriber));
                ui.label(self.subscriber_status_label(subscriber));
            } else {
                let pseudo = json!({ "issi": issi, "online": false, "directory_only": true });
                ui.label(self.subscriber_display_name(&pseudo));
                ui.monospace(if issi > 0 { issi.to_string() } else { "-".to_string() });
                ui.label(self.subscriber_type_label(&pseudo));
                ui.label(self.subscriber_status_label(&pseudo));
            }
            ui.label(display_f64(row, &["latitude"]));
            ui.label(display_f64(row, &["longitude"]));
            ui.label(str_at(row, &["source"]).unwrap_or("-"));
            ui.small(str_at(row, &["updated_at"]).unwrap_or("?"));
        });
    }


    fn render_status_tableau(&mut self, ui: &mut egui::Ui) {
        ui.heading("Status-Tableau");
        ui.small("Statusgruppen, Namen, Statuslabels und Gruppen kommen aus dem zentralen NetCore Directory-Server (/api/directory). Falls Live-Teilnehmer noch keinen Statuscode liefern, nutzt das Tableau den letzten passenden SDS-Status (z. B. Proto 218) als Fallback.");
        ui.small(format!("Directory: {}", self.directory_source));
        ui.separator();

        let cards = self.build_status_tableau_cards();

        if cards.is_empty() {
            ui.label("Keine Statusdaten vorhanden.");
            ui.small("Lege im LXC-Directory z. B. directory.subscribers.<ISSI>.status_group und directory.status_groups an oder warte auf Live-Teilnehmerdaten.");
            return;
        }

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            ui.label("Legende:");
            status_badge(ui, "1", "Frei", status_colour_for_code(1));
            status_badge(ui, "2", "Bereit", status_colour_for_code(2));
            status_badge(ui, "3", "Sprechwunsch", status_colour_for_code(3));
            status_badge(ui, "4", "Einsatz", status_colour_for_code(4));
            status_badge(ui, "5", "Am Ziel", status_colour_for_code(5));
            status_badge(ui, "6", "Nicht bereit", status_colour_for_code(6));
            status_badge(ui, "7", "Transport", status_colour_for_code(7));
            status_badge(ui, "8", "Sonderstatus", status_colour_for_code(8));
        });
        ui.separator();
        ui.small(format!("Directory-Index: {}", self.directory_source));
        if let Some((sds_rows, status_rows)) = self.sds_status_tableau_counts() {
            ui.small(format!("SDS-Fallback: {status_rows}/{sds_rows} SDS-Zeilen als Statuskandidaten erkannt"));
        }
        ui.separator();

        ui.horizontal(|ui| {
            if ui.button("Tableau-Positionen zurücksetzen").clicked() {
                self.status_card_offsets.clear();
            }
            ui.small("Karten per Maus ziehen und frei anordnen.");
        });

        let available_width = ui.available_width().max(480.0);
        let column_count = if available_width > 1500.0 {
            4_usize
        } else if available_width > 1120.0 {
            3_usize
        } else if available_width > 760.0 {
            2_usize
        } else {
            1_usize
        };
        let gap = 10.0;
        let column_width = ((available_width - ((column_count.saturating_sub(1)) as f32 * gap)) / column_count as f32).max(320.0);

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let mut column_heights = vec![0.0_f32; column_count];
            let mut placements: Vec<(StatusTableauCard, egui::Pos2, f32)> = Vec::new();

            for card in cards {
                let height = estimate_status_card_height(&card);
                let column = column_heights
                    .iter()
                    .enumerate()
                    .min_by(|left, right| left.1.partial_cmp(right.1).unwrap_or(std::cmp::Ordering::Equal))
                    .map(|(index, _)| index)
                    .unwrap_or(0);
                let pos = egui::pos2(column as f32 * (column_width + gap), column_heights[column]);
                column_heights[column] += height + gap;
                placements.push((card, pos, height));
            }

            let canvas_height = column_heights
                .into_iter()
                .fold(420.0_f32, |acc, value| acc.max(value + 80.0));
            let (canvas_rect, _) = ui.allocate_exact_size(egui::vec2(available_width, canvas_height), egui::Sense::hover());

            for (card, base_pos, height) in placements {
                let offset = *self.status_card_offsets.entry(card.id.clone()).or_insert(egui::Vec2::ZERO);
                let rect = egui::Rect::from_min_size(
                    canvas_rect.min + base_pos.to_vec2() + offset,
                    egui::vec2(column_width, height),
                );
                self.render_status_card(ui, &card, rect);
            }
        });
    }

    fn sds_status_tableau_counts(&self) -> Option<(usize, usize)> {
        let value = self.sds.as_ref()?;
        let rows = array_at(value, &["sds"]);
        let total = rows.len();
        let status = rows
            .iter()
            .filter(|row| {
                let proto = sds_protocol(row);
                let text = first_string(row, &["text", "message", "body", "payload_text", "payload"]).unwrap_or_default();
                (proto == Some(218) || looks_like_status_text(&text)) && sds_source_issi(row).is_some()
            })
            .count();
        Some((total, status))
    }

    fn build_status_tableau_cards(&self) -> Vec<StatusTableauCard> {
        let mut ids = self.settings.directory.subscriber_issis();

        if let Some(value) = &self.subscribers {
            let live_rows = array_at(value, &["subscribers"]);
            for row in live_rows {
                if let Some(issi) = u64_any_at(row, &["issi"]).or_else(|| u64_any_at(row, &["individual_issi"])).or_else(|| u64_any_at(row, &["source_issi"])) {
                    ids.push(issi);
                }
            }
        }

        if let Some(value) = &self.sds {
            for row in array_at(value, &["sds"]) {
                let proto = sds_protocol(row);
                let text = first_string(row, &["text", "message", "body", "payload_text", "payload"]).unwrap_or_default();
                if proto == Some(218) || looks_like_status_text(&text) {
                    if let Some(source) = sds_source_issi(row) {
                        ids.push(source);
                    }
                }
            }
        }

        ids.sort();
        ids.dedup();

        let mut by_group: std::collections::BTreeMap<String, Vec<StatusTableauDevice>> = std::collections::BTreeMap::new();
        let mut single_cards: Vec<StatusTableauCard> = Vec::new();

        for issi in ids {
            let pseudo = json!({ "issi": issi, "directory_only": true });
            let live_row = self.find_live_subscriber_row(issi);
            let hide_probe = live_row.unwrap_or(&pseudo);
            if self.subscriber_is_hidden(hide_probe, issi) {
                continue;
            }

            let device = self.status_tableau_device(issi, live_row);
            let raw_group = self.status_group_key_for_issi_or_row(issi, live_row);

            if let Some(group_key) = raw_group.filter(|value| !value.trim().is_empty()) {
                by_group.entry(group_key).or_default().push(device);
            } else {
                let title = device.name.clone();
                let subtitle = format!("ISSI {} · {}", device.issi, device.device_class);
                single_cards.push(StatusTableauCard {
                    id: format!("issi:{issi}"),
                    title,
                    subtitle,
                    devices: vec![device],
                    is_single_bucket: true,
                });
            }
        }

        let mut cards = Vec::new();

        for (group_id, mut devices) in by_group {
            devices.sort_by(|left, right| left.name.cmp(&right.name).then(left.issi.cmp(&right.issi)));
            let title = self.status_group_display_name(&group_id);
            let subtitle = format!("{} Gerät(e)", devices.len());
            cards.push(StatusTableauCard {
                id: format!("group:{group_id}"),
                title,
                subtitle,
                devices,
                is_single_bucket: false,
            });
        }

        single_cards.sort_by(|left, right| left.title.cmp(&right.title).then(left.subtitle.cmp(&right.subtitle)));
        cards.extend(single_cards);
        cards.sort_by(|left, right| left.title.cmp(&right.title).then(left.subtitle.cmp(&right.subtitle)));
        cards
    }

    fn find_live_subscriber_row(&self, issi: u64) -> Option<&Value> {
        let value = self.subscribers.as_ref()?;
        let live_rows = array_at(value, &["subscribers"]);
        let clean_rows = self.clean_subscriber_rows(live_rows);
        for row in clean_rows {
            if u64_any_at(row, &["issi"]).or_else(|| u64_any_at(row, &["individual_issi"])) == Some(issi) {
                return Some(row);
            }
        }
        None
    }

    fn status_group_key_for_issi_or_row(&self, issi: u64, row: Option<&Value>) -> Option<String> {
        self.settings.directory
            .subscriber(issi)
            .and_then(|entry| entry.status_group.clone())
            .or_else(|| row.and_then(|row| first_string(row, &["status_group", "status_group_id", "statusgroup"])))
            .or_else(|| {
                let code = self.status_code_for_issi_or_row(issi, row)?;
                self.settings.directory.status(code).and_then(|entry| entry.group.clone())
            })
    }

    fn status_group_display_name(&self, raw: &str) -> String {
        if let Some(entry) = self.settings.directory.status_group(raw) {
            if let Some(label) = first_config_text(&[entry.label.as_ref(), entry.name.as_ref()]) {
                return label;
            }
        }
        raw.to_string()
    }

    fn status_code_for_issi_or_row(&self, issi: u64, row: Option<&Value>) -> Option<u64> {
        row.and_then(|row| u64_any_at(row, &["status"]).or_else(|| u64_any_at(row, &["status_code"])))
            .or_else(|| {
                row.and_then(|row| first_string(row, &["status_label", "status_text", "state", "registration_state"]))
                    .and_then(|text| self.infer_status_code_from_text(&text))
            })
            .or_else(|| {
                self.settings.directory
                    .subscriber(issi)
                    .and_then(|entry| entry.status.as_deref())
                    .and_then(|value| parse_status_code(value).or_else(|| self.infer_status_code_from_text(value)))
            })
            .or_else(|| self.latest_sds_status_for_issi(issi).and_then(|(code, _)| code))
    }

    fn latest_sds_status_for_issi(&self, issi: u64) -> Option<(Option<u64>, String)> {
        let value = self.sds.as_ref()?;
        let rows = array_at(value, &["sds"]);
        let mut latest: Option<&Value> = None;

        for row in rows {
            let source = sds_source_issi(row);
            if source != Some(issi) {
                continue;
            }

            let proto = sds_protocol(row);
            let text = first_string(row, &["text", "message", "body", "payload_text", "payload"]).unwrap_or_default();
            if proto != Some(218) && !looks_like_status_text(&text) {
                continue;
            }

            match latest {
                Some(current) if !sds_row_is_newer(row, current) => {}
                _ => latest = Some(row),
            }
        }

        let row = latest?;
        let raw_text = first_string(row, &["text", "message", "body", "payload_text", "payload"]).unwrap_or_default();
        let text = extract_status_label_from_text(&raw_text).unwrap_or_else(|| raw_text.trim().to_string());
        let code = u64_any_at(row, &["status"])
            .or_else(|| u64_any_at(row, &["status_code"]))
            .or_else(|| u64_any_at(row, &["status_id"]))
            .or_else(|| u64_any_at(row, &["status_number"]))
            .or_else(|| self.infer_status_code_from_text(&text));
        Some((code, text))
    }

    fn infer_status_code_from_text(&self, text: &str) -> Option<u64> {
        let normalized = normalize_status_text(text);
        if normalized.is_empty() {
            return None;
        }

        for (key, entry) in &self.settings.directory.statuses {
            let candidates = [
                entry.label.as_deref(),
                entry.name.as_deref(),
                entry.description.as_deref(),
            ];
            for candidate in candidates.into_iter().flatten() {
                if normalize_status_text(candidate) == normalized {
                    return parse_status_code(key);
                }
            }
        }

        let checks: &[(u64, &[&str])] = &[
            (6, &["nicht bereit", "nicht einsatzbereit", "abgemeldet", "ausser dienst", "außer dienst"]),
            (3, &["sprechwunsch", "sprech wunsch"]),
            (7, &["transport"]),
            (8, &["sonderstatus", "sonder status"]),
            (5, &["am ziel", "eingetroffen", "ziel"]),
            (4, &["einsatz", "alarm", "unterwegs"]),
            (2, &["bereit"]),
            (1, &["frei auf wache", "frei auf funk", "auf wache", "auf funk", "frei"]),
        ];
        for (code, patterns) in checks {
            if patterns.iter().any(|pattern| normalized.contains(*pattern)) {
                return Some(*code);
            }
        }
        None
    }

    fn status_tableau_device(&self, issi: u64, live_row: Option<&Value>) -> StatusTableauDevice {
        let pseudo = json!({ "issi": issi, "directory_only": true });
        let row = live_row.unwrap_or(&pseudo);

        let name = self.best_device_name_for_issi_or_row(issi, live_row)
            .unwrap_or_else(|| issi.to_string());
        let device_class = self.subscriber_type_label(row);
        let sds_status = self.latest_sds_status_for_issi(issi);
        let explicit_code = self.status_code_for_issi_or_row(issi, live_row);
        let mut status_text = if let Some((_, text)) = sds_status.clone() {
            if !text.trim().is_empty() {
                text
            } else if let Some(code) = explicit_code {
                self.settings.directory
                    .status(code)
                    .and_then(|entry| first_config_text(&[entry.label.as_ref(), entry.name.as_ref()]))
                    .unwrap_or_else(|| status_label_default(code).to_string())
            } else {
                self.subscriber_status_label(row)
            }
        } else if let Some(code) = explicit_code {
            self.settings.directory
                .status(code)
                .and_then(|entry| first_config_text(&[entry.label.as_ref(), entry.name.as_ref()]))
                .unwrap_or_else(|| status_label_default(code).to_string())
        } else {
            self.subscriber_status_label(row)
        };
        if status_text.trim().is_empty() {
            status_text = "Status unbekannt".to_string();
        }

        let status_code = explicit_code
            .or_else(|| self.infer_status_code_from_text(&status_text))
            .unwrap_or(0);

        let status_group = self.status_group_key_for_issi_or_row(issi, live_row)
            .map(|raw| self.status_group_display_name(&raw))
            .unwrap_or_else(|| "-".to_string());

        let mut groups = group_ids_from_row(row);
        if groups.is_empty() {
            if let Some(entry) = self.settings.directory.subscriber(issi) {
                groups.extend(entry.groups.iter().copied());
                groups.extend(entry.static_groups.iter().copied());
            }
        }
        groups.sort();
        groups.dedup();

        StatusTableauDevice {
            issi,
            name,
            device_class,
            status_code,
            status_text,
            status_group,
            groups,
            last_seen: str_at(row, &["last_seen"]).or_else(|| str_at(row, &["updated_at"])).unwrap_or("-").to_string(),
            online: subscriber_online(row),
        }
    }


    fn render_status_card(&mut self, ui: &mut egui::Ui, card: &StatusTableauCard, rect: egui::Rect) {
        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
        if response.dragged() {
            let delta = ui.ctx().input(|input| input.pointer.delta());
            if delta != egui::Vec2::ZERO {
                *self.status_card_offsets.entry(card.id.clone()).or_insert(egui::Vec2::ZERO) += delta;
            }
        }

        let frame_fill = if card.is_single_bucket {
            egui::Color32::from_rgb(174, 174, 166)
        } else {
            egui::Color32::from_rgb(166, 166, 158)
        };

        ui.allocate_ui_at_rect(rect, |ui| {
            egui::Frame::none()
                .fill(frame_fill)
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(64, 64, 64)))
                .rounding(egui::Rounding::same(5.0))
                .inner_margin(egui::Margin::same(5.0))
                .show(ui, |ui| {
                    ui.set_width((rect.width() - 8.0).max(280.0));

                    egui::Frame::none()
                        .fill(egui::Color32::from_rgb(54, 60, 64))
                        .rounding(egui::Rounding::same(3.0))
                        .inner_margin(egui::Margin::symmetric(6.0, 4.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.colored_label(egui::Color32::WHITE, egui::RichText::new(&card.title).strong());
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.small(egui::RichText::new(&card.subtitle).color(egui::Color32::LIGHT_GRAY));
                                });
                            });
                        });

                    ui.add_space(5.0);
                    for device in &card.devices {
                        self.render_status_device_row(ui, device, rect.width() - 18.0, card.is_single_bucket);
                        ui.add_space(3.0);
                    }
                });
        });
    }


    fn render_status_device_row(&self, ui: &mut egui::Ui, device: &StatusTableauDevice, width: f32, single_bucket: bool) {
        let row_height = 46.0;
        let status_width = 52.0;
        let (rect, response) = ui.allocate_exact_size(egui::vec2(width.max(260.0), row_height), egui::Sense::click());
        let painter = ui.painter();

        let status_colour = status_colour_for_code(device.status_code);
        let name_rect = egui::Rect::from_min_max(rect.min, egui::pos2(rect.right() - status_width, rect.bottom()));
        let status_rect = egui::Rect::from_min_max(egui::pos2(rect.right() - status_width, rect.top()), rect.max);

        let base_colour = if single_bucket {
            egui::Color32::from_rgb(245, 245, 239)
        } else {
            egui::Color32::from_rgb(42, 157, 202)
        };

        painter.rect_filled(name_rect, egui::Rounding::same(4.0), base_colour);
        painter.rect_filled(status_rect, egui::Rounding::same(4.0), status_colour);
        painter.rect_stroke(rect, egui::Rounding::same(4.0), egui::Stroke::new(1.0, egui::Color32::from_rgb(55, 55, 55)));

        let heading = if device.name.trim() == device.issi.to_string() {
            device.issi.to_string()
        } else {
            device.name.clone()
        };
        painter.text(
            name_rect.center_top() + egui::vec2(0.0, 6.0),
            egui::Align2::CENTER_TOP,
            heading,
            egui::FontId::proportional(15.0),
            egui::Color32::BLACK,
        );

        let detail_text = if single_bucket {
            format!("ISSI {} · [{}] {}", device.issi, device.device_class, device.status_text)
        } else {
            format!("[{}] {}", device.device_class, device.status_text)
        };
        painter.text(
            name_rect.center_bottom() - egui::vec2(0.0, 6.0),
            egui::Align2::CENTER_BOTTOM,
            detail_text,
            egui::FontId::proportional(11.0),
            egui::Color32::from_rgb(30, 30, 30),
        );

        painter.text(
            status_rect.center(),
            egui::Align2::CENTER_CENTER,
            if device.status_code > 0 { device.status_code.to_string() } else { "–".to_string() },
            egui::FontId::proportional(24.0),
            egui::Color32::BLACK,
        );

        let groups = if device.groups.is_empty() {
            "-".to_string()
        } else {
            device.groups.iter().map(|gssi| self.format_group(*gssi)).collect::<Vec<_>>().join(", ")
        };

        response.on_hover_text(format!(
            "{}\nISSI: {}\nTyp: {}\nStatus: {}\nStatusgruppe: {}\nGruppen: {}\nOnline: {}\nLetzte Meldung: {}",
            device.name,
            device.issi,
            device.device_class,
            device.status_text,
            device.status_group,
            groups,
            if device.online { "ja" } else { "nein/unbekannt" },
            device.last_seen,
        ));
    }

    fn render_map(&mut self, ui: &mut egui::Ui) {
        ui.heading("Live-Karte / LIP-Standorte");
        ui.horizontal_wrapped(|ui| {
            if ui.checkbox(&mut self.map_follow_latest, "Positionen folgen").changed() && self.map_follow_latest {
                self.map_manual_center = None;
            }
            ui.checkbox(&mut self.map_tiles_enabled, "Online-Kartenkacheln laden");
            if ui.button("− Zoom").clicked() {
                self.map_zoom_adjust -= 1;
                self.map_follow_latest = false;
            }
            if ui.button("+ Zoom").clicked() {
                self.map_zoom_adjust += 1;
                self.map_follow_latest = false;
            }
            if ui.button("Ansicht reset").clicked() {
                self.map_zoom_adjust = 0;
                self.map_manual_center = None;
                self.map_follow_latest = true;
            }
            ui.small("Maus: ziehen = Karte verschieben · Mausrad = fein dosiert zoomen · Klick auf GPS-Punkt = Gerätedetails · Doppelklick = zentrieren.");
        });
        let Some(value) = self.locations.clone() else { ui.label("Noch keine Standortdaten"); return; };
        let all_rows = array_at(&value, &["locations"]);
        let rows = latest_location_rows(&all_rows);
        self.render_location_map(ui, &rows);
    }

    fn render_location_map(&mut self, ui: &mut egui::Ui, rows: &[&Value]) {
        let points = collect_points(rows);
        let desired_size = egui::vec2(ui.available_width().max(620.0), ui.available_height().clamp(430.0, 760.0));
        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        let visuals = ui.visuals();

        painter.rect_filled(rect, egui::Rounding::same(8.0), egui::Color32::from_rgb(22, 28, 34));
        painter.rect_stroke(rect, egui::Rounding::same(8.0), egui::Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color));

        let map_rect = rect.shrink(10.0);
        let viewport = MapViewport::for_state(
            &points,
            map_rect,
            &self.settings.map,
            self.map_follow_latest,
            self.map_manual_center,
            self.map_zoom_adjust,
        );

        self.handle_map_interaction(ui, &response, map_rect, &viewport);
        self.handle_marker_selection(ui, &response, map_rect, &viewport, &points);

        let viewport = MapViewport::for_state(
            &points,
            map_rect,
            &self.settings.map,
            self.map_follow_latest,
            self.map_manual_center,
            self.map_zoom_adjust,
        );

        if self.map_tiles_enabled {
            self.draw_tiles(ui, &painter, map_rect, &viewport);
        } else {
            self.draw_fallback_map_grid(&painter, map_rect, &viewport);
        }

        self.draw_map_overlay(&painter, rect, map_rect, &viewport, &points, self.selected_location_issi);

        if points.is_empty() {
            painter.text(
                map_rect.center(),
                egui::Align2::CENTER_CENTER,
                "Noch keine LIP-/Standortdaten – Karte zeigt Fallback-Zentrum",
                egui::FontId::proportional(18.0),
                egui::Color32::WHITE,
            );
        }

        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(if response.dragged() { egui::CursorIcon::Grabbing } else { egui::CursorIcon::Grab });
            if let Some(pos) = ui.input(|input| input.pointer.hover_pos()) {
                if map_rect.contains(pos) {
                    if let Some(text) = self.map_hover_text(pos, &points, map_rect, &viewport) {
                        response.on_hover_text(text);
                    } else {
                        let (lat, lon) = viewport.screen_to_lat_lon(pos, map_rect);
                        response.on_hover_text(format!(
                            "Lat {:.6}
Lon {:.6}
Zoom {}
Ziehen = verschieben
Mausrad = fein dosiert zoomen
Cluster anklicken = auffächern",
                            lat,
                            lon,
                            viewport.zoom
                        ));
                    }
                }
            }
        }
    }


    fn handle_map_interaction(&mut self, ui: &egui::Ui, response: &egui::Response, map_rect: egui::Rect, viewport: &MapViewport) {
        let pointer_pos = ui.input(|input| input.pointer.hover_pos());
        let pointer_in_map = pointer_pos.map(|pos| map_rect.contains(pos)).unwrap_or(false);

        if response.dragged_by(egui::PointerButton::Primary) && pointer_in_map {
            let delta = ui.input(|input| input.pointer.delta());
            if delta.length_sq() > 0.0 {
                let center_world = lat_lon_to_world(viewport.center_lat, viewport.center_lon, viewport.zoom);
                let new_center_world = WorldPoint {
                    x: center_world.x - delta.x as f64,
                    y: center_world.y - delta.y as f64,
                };
                let (lat, lon) = world_to_lat_lon(new_center_world, viewport.zoom);
                self.map_manual_center = Some((lat.clamp(-85.0, 85.0), lon));
                self.map_zoom_adjust = viewport.zoom as i32 - self.settings.map.default_zoom as i32;
                self.map_follow_latest = false;
                ui.ctx().request_repaint();
            }
        }

        if response.double_clicked() && pointer_in_map {
            if let Some(pos) = pointer_pos {
                let (lat, lon) = viewport.screen_to_lat_lon(pos, map_rect);
                self.map_manual_center = Some((lat.clamp(-85.0, 85.0), lon));
                self.map_zoom_adjust = viewport.zoom as i32 - self.settings.map.default_zoom as i32;
                self.map_follow_latest = false;
                ui.ctx().request_repaint();
            }
        }

        if response.hovered() && pointer_in_map {
            // Use the raw wheel delta instead of smooth_scroll_delta.
            // smooth_scroll_delta is intentionally animated by egui and caused one physical wheel notch
            // to be applied over several frames, which felt like jumping through many zoom levels.
            let scroll_y = ui.input(|input| input.raw_scroll_delta.y);
            if scroll_y.abs() > 0.0 {
                const WHEEL_ZOOM_THRESHOLD: f32 = 120.0;
                self.map_wheel_zoom_accum = (self.map_wheel_zoom_accum + scroll_y).clamp(-WHEEL_ZOOM_THRESHOLD, WHEEL_ZOOM_THRESHOLD);

                if self.map_wheel_zoom_accum.abs() >= WHEEL_ZOOM_THRESHOLD {
                    let zoom_step = if self.map_wheel_zoom_accum > 0.0 { 1 } else { -1 };
                    self.map_wheel_zoom_accum = 0.0;

                    let old_zoom = viewport.zoom;
                    let new_zoom = ((old_zoom as i32 + zoom_step)
                        .clamp(self.settings.map.min_zoom as i32, self.settings.map.max_zoom as i32)) as u8;

                    if new_zoom != old_zoom {
                        let anchor_pos = pointer_pos.unwrap_or(map_rect.center());
                        let (anchor_lat, anchor_lon) = viewport.screen_to_lat_lon(anchor_pos, map_rect);
                        let anchor_world = lat_lon_to_world(anchor_lat, anchor_lon, new_zoom);
                        let offset_from_center = anchor_pos - map_rect.center();
                        let new_center_world = WorldPoint {
                            x: anchor_world.x - offset_from_center.x as f64,
                            y: anchor_world.y - offset_from_center.y as f64,
                        };
                        let (center_lat, center_lon) = world_to_lat_lon(new_center_world, new_zoom);
                        self.map_manual_center = Some((center_lat.clamp(-85.0, 85.0), center_lon));
                        self.map_zoom_adjust = new_zoom as i32 - self.settings.map.default_zoom as i32;
                        self.map_follow_latest = false;
                        ui.ctx().request_repaint();
                    }
                }
            }
        }
    }

    fn handle_marker_selection(&mut self, ui: &egui::Ui, response: &egui::Response, map_rect: egui::Rect, viewport: &MapViewport, points: &[LocationPoint]) {
        if !response.clicked() {
            return;
        }
        let Some(pos) = ui.input(|input| input.pointer.interact_pos()) else {
            return;
        };
        if !map_rect.contains(pos) {
            return;
        }

        let clusters = build_map_clusters(points, map_rect, viewport, 32.0);

        if let Some(selected_issi) = self.selected_location_issi {
            for cluster in &clusters {
                if cluster.members.len() > 1 && cluster_contains_issi(cluster, points, selected_issi) {
                    for (member_index, point_index) in cluster.members.iter().enumerate() {
                        let spider_pos = spider_position(cluster.center, member_index, cluster.members.len());
                        if (spider_pos - pos).length_sq() <= 18.0 * 18.0 {
                            self.selected_location_issi = Some(points[*point_index].issi);
                            return;
                        }
                    }
                }
            }
        }

        let mut best: Option<(f32, u64)> = None;
        for cluster in &clusters {
            let distance_sq = (cluster.center - pos).length_sq();
            let radius = if cluster.members.len() > 1 { cluster_radius(cluster.members.len()) + 8.0 } else { 18.0 };
            if distance_sq <= radius * radius {
                let issi = points[cluster.members[0]].issi;
                if best.map(|(best_distance, _)| distance_sq < best_distance).unwrap_or(true) {
                    best = Some((distance_sq, issi));
                }
            }
        }

        self.selected_location_issi = best.map(|(_, issi)| issi);
    }

    fn draw_tiles(&mut self, ui: &egui::Ui, painter: &egui::Painter, map_rect: egui::Rect, viewport: &MapViewport) {
        painter.rect_filled(map_rect, egui::Rounding::same(6.0), egui::Color32::from_rgb(210, 218, 222));

        let ctx = ui.ctx();
        let zoom_scale = 2.0_f64.powi(viewport.zoom as i32);
        let min_tile_x = (viewport.top_left_world.x / TILE_SIZE).floor() as i32 - 1;
        let max_tile_x = ((viewport.top_left_world.x + map_rect.width() as f64) / TILE_SIZE).ceil() as i32 + 1;
        let min_tile_y = (viewport.top_left_world.y / TILE_SIZE).floor() as i32 - 1;
        let max_tile_y = ((viewport.top_left_world.y + map_rect.height() as f64) / TILE_SIZE).ceil() as i32 + 1;
        let tile_limit = zoom_scale as i32;
        let mut tile_errors = Vec::new();

        for tile_y in min_tile_y..=max_tile_y {
            if tile_y < 0 || tile_y >= tile_limit {
                continue;
            }
            for tile_x_raw in min_tile_x..=max_tile_x {
                let tile_x = tile_x_raw.rem_euclid(tile_limit);
                let screen_x = map_rect.left() + ((tile_x_raw as f64 * TILE_SIZE - viewport.top_left_world.x) as f32);
                let screen_y = map_rect.top() + ((tile_y as f64 * TILE_SIZE - viewport.top_left_world.y) as f32);
                let tile_rect = egui::Rect::from_min_size(
                    egui::pos2(screen_x, screen_y),
                    egui::vec2(TILE_SIZE as f32 + 1.0, TILE_SIZE as f32 + 1.0),
                );
                if !tile_rect.intersects(map_rect) {
                    continue;
                }

                match self.map_cache.texture_id(ctx, viewport.zoom, tile_x as u32, tile_y as u32, self.map_tiles_enabled) {
                    Ok(Some(texture_id)) => {
                        let clipped = tile_rect.intersect(map_rect);
                        if clipped.is_positive() {
                            let uv_min = egui::pos2(
                                ((clipped.left() - tile_rect.left()) / tile_rect.width()).clamp(0.0, 1.0),
                                ((clipped.top() - tile_rect.top()) / tile_rect.height()).clamp(0.0, 1.0),
                            );
                            let uv_max = egui::pos2(
                                ((clipped.right() - tile_rect.left()) / tile_rect.width()).clamp(0.0, 1.0),
                                ((clipped.bottom() - tile_rect.top()) / tile_rect.height()).clamp(0.0, 1.0),
                            );
                            painter.image(texture_id, clipped, egui::Rect::from_min_max(uv_min, uv_max), egui::Color32::WHITE);
                        }
                    }
                    Ok(None) => {
                        painter.rect_filled(tile_rect.intersect(map_rect), egui::Rounding::ZERO, egui::Color32::from_rgb(198, 207, 211));
                        ui.ctx().request_repaint_after(Duration::from_millis(40));
                    }
                    Err(error) => {
                        if tile_errors.len() < 2 {
                            tile_errors.push(error);
                        }
                        painter.rect_filled(tile_rect.intersect(map_rect), egui::Rounding::ZERO, egui::Color32::from_rgb(185, 195, 200));
                    }
                }
            }
        }

        if !tile_errors.is_empty() {
            let text = format!("Tile-Fehler: {}", tile_errors.join(" | "));
            painter.text(
                map_rect.left_top() + egui::vec2(12.0, 34.0),
                egui::Align2::LEFT_TOP,
                text,
                egui::FontId::monospace(11.0),
                egui::Color32::from_rgb(140, 30, 30),
            );
        }
    }

    fn draw_fallback_map_grid(&self, painter: &egui::Painter, map_rect: egui::Rect, viewport: &MapViewport) {
        painter.rect_filled(map_rect, egui::Rounding::same(6.0), egui::Color32::from_rgb(24, 36, 44));
        for i in 0..=8 {
            let t = i as f32 / 8.0;
            let x = egui::lerp(map_rect.left()..=map_rect.right(), t);
            let y = egui::lerp(map_rect.top()..=map_rect.bottom(), t);
            painter.line_segment(
                [egui::pos2(x, map_rect.top()), egui::pos2(x, map_rect.bottom())],
                egui::Stroke::new(0.6, egui::Color32::from_gray(70)),
            );
            painter.line_segment(
                [egui::pos2(map_rect.left(), y), egui::pos2(map_rect.right(), y)],
                egui::Stroke::new(0.6, egui::Color32::from_gray(70)),
            );
        }
        painter.text(
            map_rect.center_top() + egui::vec2(0.0, 16.0),
            egui::Align2::CENTER_TOP,
            format!("Kartenkacheln deaktiviert · Zentrum {:.5}, {:.5} · Zoom {}", viewport.center_lat, viewport.center_lon, viewport.zoom),
            egui::FontId::proportional(13.0),
            egui::Color32::LIGHT_GRAY,
        );
    }

    fn draw_map_overlay(&self, painter: &egui::Painter, rect: egui::Rect, map_rect: egui::Rect, viewport: &MapViewport, points: &[LocationPoint], selected_issi: Option<u64>) {
        painter.rect_stroke(map_rect, egui::Rounding::same(6.0), egui::Stroke::new(1.0, egui::Color32::from_black_alpha(140)));

        let clusters = build_map_clusters(points, map_rect, viewport, 32.0);
        let selected_point = selected_issi.and_then(|issi| points.iter().find(|point| point.issi == issi));
        let selected_cluster = selected_issi.and_then(|issi| clusters.iter().find(|cluster| cluster.members.len() > 1 && cluster_contains_issi(cluster, points, issi)));

        for cluster in &clusters {
            if cluster.members.len() <= 1 {
                let point = &points[cluster.members[0]];
                if !map_rect.expand(16.0).contains(cluster.center) {
                    continue;
                }
                let selected = selected_issi == Some(point.issi);
                self.draw_device_marker(painter, cluster.center, point, selected, true);
                continue;
            }

            if !map_rect.expand(42.0).contains(cluster.center) {
                continue;
            }

            let expanded = selected_issi.map(|issi| cluster_contains_issi(cluster, points, issi)).unwrap_or(false);
            if expanded {
                painter.circle_filled(cluster.center, 12.0, egui::Color32::from_rgb(35, 82, 122));
                painter.circle_stroke(cluster.center, 15.0, egui::Stroke::new(2.0, egui::Color32::WHITE));
                painter.text(
                    cluster.center,
                    egui::Align2::CENTER_CENTER,
                    cluster.members.len().to_string(),
                    egui::FontId::monospace(13.0),
                    egui::Color32::WHITE,
                );

                for (member_index, point_index) in cluster.members.iter().enumerate() {
                    let point = &points[*point_index];
                    let pos = spider_position(cluster.center, member_index, cluster.members.len());
                    painter.line_segment([cluster.center, pos], egui::Stroke::new(1.0, egui::Color32::from_white_alpha(170)));
                    let selected = selected_issi == Some(point.issi);
                    self.draw_device_marker(painter, pos, point, selected, true);
                }
            } else {
                self.draw_cluster_marker(painter, cluster.center, cluster.members.len());
            }
        }

        if let Some(cluster) = selected_cluster {
            self.draw_cluster_device_card(painter, map_rect, cluster, points, selected_issi);
        } else if let Some(point) = selected_point {
            self.draw_selected_location_card(painter, map_rect, point);
        }

        let clustered_count = clusters.iter().filter(|cluster| cluster.members.len() > 1).count();
        let title = if points.is_empty() {
            "Live-Karte · keine Positionen".to_string()
        } else if clustered_count > 0 {
            format!("Live-Karte · {} Geräte · {} Cluster · Zoom {}", points.len(), clustered_count, viewport.zoom)
        } else {
            format!("Live-Karte · {} Position(en) · Zoom {}", points.len(), viewport.zoom)
        };
        painter.rect_filled(
            egui::Rect::from_min_size(rect.left_top() + egui::vec2(16.0, 14.0), egui::vec2(360.0, 52.0)),
            egui::Rounding::same(6.0),
            egui::Color32::from_black_alpha(145),
        );
        painter.text(
            rect.left_top() + egui::vec2(24.0, 20.0),
            egui::Align2::LEFT_TOP,
            title,
            egui::FontId::proportional(15.0),
            egui::Color32::WHITE,
        );
        painter.text(
            rect.left_top() + egui::vec2(24.0, 39.0),
            egui::Align2::LEFT_TOP,
            format!("Zentrum {:.5}, {:.5} · Cluster anklicken = auffächern", viewport.center_lat, viewport.center_lon),
            egui::FontId::monospace(11.0),
            egui::Color32::LIGHT_GRAY,
        );

        painter.rect_filled(
            egui::Rect::from_min_size(rect.right_bottom() - egui::vec2(430.0, 34.0), egui::vec2(418.0, 22.0)),
            egui::Rounding::same(5.0),
            egui::Color32::from_black_alpha(135),
        );
        painter.text(
            rect.right_bottom() - egui::vec2(20.0, 28.0),
            egui::Align2::RIGHT_TOP,
            format!("{} · Cache: {}", self.settings.map.tile_attribution, self.settings.map.cache_dir.display()),
            egui::FontId::proportional(11.0),
            egui::Color32::WHITE,
        );
    }

    fn draw_device_marker(&self, painter: &egui::Painter, pos: egui::Pos2, point: &LocationPoint, selected: bool, show_label: bool) {
        let fill = if selected { egui::Color32::from_rgb(255, 191, 0) } else { egui::Color32::from_rgb(0, 210, 80) };
        let radius = if selected { 9.0 } else { 7.0 };
        painter.circle_filled(pos, radius, fill);
        painter.circle_stroke(pos, if selected { 13.0 } else { 10.0 }, egui::Stroke::new(2.0, egui::Color32::WHITE));
        if show_label {
            let label = self.directory_name_for_issi(point.issi).unwrap_or_else(|| point.issi.to_string());
            let short_label = compact_marker_label(&label);
            painter.text(
                pos + egui::vec2(12.0, -10.0),
                egui::Align2::LEFT_BOTTOM,
                &short_label,
                egui::FontId::monospace(13.0),
                egui::Color32::BLACK,
            );
            painter.text(
                pos + egui::vec2(13.0, -9.0),
                egui::Align2::LEFT_BOTTOM,
                short_label,
                egui::FontId::monospace(13.0),
                egui::Color32::WHITE,
            );
        }
    }

    fn draw_cluster_marker(&self, painter: &egui::Painter, pos: egui::Pos2, count: usize) {
        let radius = cluster_radius(count);
        painter.circle_filled(pos, radius + 4.0, egui::Color32::from_white_alpha(215));
        painter.circle_filled(pos, radius, egui::Color32::from_rgb(0, 118, 214));
        painter.circle_stroke(pos, radius + 4.0, egui::Stroke::new(2.0, egui::Color32::from_rgb(20, 52, 84)));
        painter.text(
            pos,
            egui::Align2::CENTER_CENTER,
            count.to_string(),
            egui::FontId::proportional(17.0),
            egui::Color32::WHITE,
        );
    }

    fn map_hover_text(&self, pos: egui::Pos2, points: &[LocationPoint], map_rect: egui::Rect, viewport: &MapViewport) -> Option<String> {
        let clusters = build_map_clusters(points, map_rect, viewport, 32.0);

        if let Some(selected_issi) = self.selected_location_issi {
            for cluster in &clusters {
                if cluster.members.len() > 1 && cluster_contains_issi(cluster, points, selected_issi) {
                    for (member_index, point_index) in cluster.members.iter().enumerate() {
                        let spider_pos = spider_position(cluster.center, member_index, cluster.members.len());
                        if (spider_pos - pos).length_sq() <= 18.0 * 18.0 {
                            let point = &points[*point_index];
                            return Some(self.device_hover_text(point));
                        }
                    }
                }
            }
        }

        let mut best_cluster: Option<(f32, &MapCluster)> = None;
        for cluster in &clusters {
            let distance_sq = (cluster.center - pos).length_sq();
            let radius = if cluster.members.len() > 1 { cluster_radius(cluster.members.len()) + 8.0 } else { 18.0 };
            if distance_sq <= radius * radius && best_cluster.map(|(best, _)| distance_sq < best).unwrap_or(true) {
                best_cluster = Some((distance_sq, cluster));
            }
        }

        let (_, cluster) = best_cluster?;
        if cluster.members.len() > 1 {
            let mut lines = vec![format!("{} Geräte an diesem Punkt", cluster.members.len()), "Klick = auffächern".to_string()];
            for point_index in cluster.members.iter().take(8) {
                let point = &points[*point_index];
                let name = self.directory_name_for_issi(point.issi).unwrap_or_else(|| point.issi.to_string());
                lines.push(format!("• {} ({})", compact_marker_label(&name), point.issi));
            }
            if cluster.members.len() > 8 {
                lines.push(format!("… und {} weitere", cluster.members.len() - 8));
            }
            Some(lines.join("
"))
        } else {
            Some(self.device_hover_text(&points[cluster.members[0]]))
        }
    }

    fn device_hover_text(&self, point: &LocationPoint) -> String {
        let name = self.directory_name_for_issi(point.issi).unwrap_or_else(|| point.issi.to_string());
        format!(
            "{}
ISSI {}
Lat {:.6}
Lon {:.6}
Quelle {}
Update {}
Klick = Gerätedetails",
            name,
            point.issi,
            point.lat,
            point.lon,
            point.source,
            point.updated_at,
        )
    }

    fn draw_cluster_device_card(&self, painter: &egui::Painter, map_rect: egui::Rect, cluster: &MapCluster, points: &[LocationPoint], selected_issi: Option<u64>) {
        let width = 380.0;
        let max_rows = cluster.members.len().min(9);
        let height = 96.0 + max_rows as f32 * 20.0;
        let card = egui::Rect::from_min_size(
            map_rect.right_top() + egui::vec2(-width - 14.0, 14.0),
            egui::vec2(width, height),
        );
        painter.rect_filled(card, egui::Rounding::same(8.0), egui::Color32::from_black_alpha(210));
        painter.rect_stroke(card, egui::Rounding::same(8.0), egui::Stroke::new(1.0, egui::Color32::WHITE));

        let mut y = card.top() + 12.0;
        let x = card.left() + 14.0;
        painter.text(
            egui::pos2(x, y),
            egui::Align2::LEFT_TOP,
            format!("Gerätecluster · {} Geräte", cluster.members.len()),
            egui::FontId::proportional(17.0),
            egui::Color32::WHITE,
        );
        y += 24.0;
        painter.text(
            egui::pos2(x, y),
            egui::Align2::LEFT_TOP,
            "Klick auf aufgefächerten Punkt wählt Gerät",
            egui::FontId::proportional(12.0),
            egui::Color32::LIGHT_GRAY,
        );
        y += 24.0;

        for point_index in cluster.members.iter().take(max_rows) {
            let point = &points[*point_index];
            let selected = selected_issi == Some(point.issi);
            let name = self.directory_name_for_issi(point.issi).unwrap_or_else(|| point.issi.to_string());
            let color = if selected { egui::Color32::from_rgb(255, 220, 80) } else { egui::Color32::from_rgb(230, 235, 240) };
            painter.text(
                egui::pos2(x, y),
                egui::Align2::LEFT_TOP,
                format!("{}{} · ISSI {} · {}", if selected { "➤ " } else { "• " }, compact_marker_label(&name), point.issi, point.updated_at),
                egui::FontId::monospace(12.0),
                color,
            );
            y += 20.0;
        }
        if cluster.members.len() > max_rows {
            painter.text(
                egui::pos2(x, y),
                egui::Align2::LEFT_TOP,
                format!("… {} weitere Geräte", cluster.members.len() - max_rows),
                egui::FontId::monospace(12.0),
                egui::Color32::LIGHT_GRAY,
            );
        }
    }

    fn draw_selected_location_card(&self, painter: &egui::Painter, map_rect: egui::Rect, point: &LocationPoint) {
        let width = 350.0;
        let height = 178.0;
        let card = egui::Rect::from_min_size(
            map_rect.right_top() + egui::vec2(-width - 14.0, 14.0),
            egui::vec2(width, height),
        );
        painter.rect_filled(card, egui::Rounding::same(8.0), egui::Color32::from_black_alpha(205));
        painter.rect_stroke(card, egui::Rounding::same(8.0), egui::Stroke::new(1.0, egui::Color32::WHITE));

        let mut y = card.top() + 12.0;
        let x = card.left() + 14.0;
        painter.text(
            egui::pos2(x, y),
            egui::Align2::LEFT_TOP,
            format!("Gerät / ISSI {}", point.issi),
            egui::FontId::proportional(17.0),
            egui::Color32::WHITE,
        );
        y += 28.0;
        for (label, value) in self.location_detail_lines(point) {
            painter.text(
                egui::pos2(x, y),
                egui::Align2::LEFT_TOP,
                format!("{label}: {value}"),
                egui::FontId::monospace(12.0),
                egui::Color32::from_rgb(230, 235, 240),
            );
            y += 18.0;
        }
    }

    fn location_detail_lines(&self, point: &LocationPoint) -> Vec<(String, String)> {
        let mut lines = Vec::new();
        if let Some(name) = self.directory_name_for_issi(point.issi) {
            lines.push(("Name".to_string(), name));
        }
        lines.push(("Position".to_string(), format!("{:.6}, {:.6}", point.lat, point.lon)));
        lines.push(("Quelle".to_string(), point.source.clone()));
        lines.push(("Update".to_string(), point.updated_at.clone()));
        if let Some(subscriber) = self.subscriber_for_issi(point.issi) {
            lines.push(("Typ".to_string(), self.subscriber_type_label(subscriber)));
            lines.push(("Status".to_string(), self.subscriber_status_label(subscriber)));
            lines.push(("Statusgruppe".to_string(), self.subscriber_status_group_label(subscriber)));
            lines.push(("Gruppen".to_string(), self.subscriber_groups_label(subscriber)));
            if let Some(last_seen) = str_at(subscriber, &["last_seen"]).or_else(|| str_at(subscriber, &["updated_at"])) {
                lines.push(("Teiln. gesehen".to_string(), last_seen.to_string()));
            }
            lines.push(("Online".to_string(), if subscriber_online(subscriber) { "ja" } else { "nein" }.to_string()));
        } else if self.settings.directory.subscriber(point.issi).is_some() {
            let pseudo = json!({ "issi": point.issi, "online": false, "directory_only": true });
            lines.push(("Typ".to_string(), self.subscriber_type_label(&pseudo)));
            lines.push(("Status".to_string(), self.subscriber_status_label(&pseudo)));
            lines.push(("Statusgruppe".to_string(), self.subscriber_status_group_label(&pseudo)));
            lines.push(("Gruppen".to_string(), self.subscriber_groups_label(&pseudo)));
        } else if let Some(class) = issi_class_label(point.issi) {
            lines.push(("Typ".to_string(), class.to_string()));
        }
        lines.truncate(10);
        lines
    }

    fn subscriber_for_issi(&self, issi: u64) -> Option<&Value> {
        let value = self.subscribers.as_ref()?;
        for row in array_at(value, &["subscribers"]) {
            if u64_at(row, &["issi"]) == Some(issi)
                || u64_at(row, &["individual_issi"]) == Some(issi)
                || u64_at(row, &["address"]) == Some(issi)
            {
                return Some(row);
            }
        }
        None
    }



    fn clean_subscriber_rows<'a>(&self, rows: Vec<&'a Value>) -> Vec<&'a Value> {
        let mut latest_by_issi: HashMap<u64, &'a Value> = HashMap::new();
        let mut out_without_issi = Vec::new();

        for row in rows {
            let Some(issi) = u64_at(row, &["issi"]).or_else(|| u64_at(row, &["individual_issi"])) else {
                out_without_issi.push(row);
                continue;
            };
            if issi == 0 || self.subscriber_is_hidden(row, issi) {
                continue;
            }
            let replace = latest_by_issi
                .get(&issi)
                .map(|current| subscriber_row_is_newer(row, current))
                .unwrap_or(true);
            if replace {
                latest_by_issi.insert(issi, row);
            }
        }

        let mut rows: Vec<&'a Value> = latest_by_issi.into_values().collect();
        rows.extend(out_without_issi.into_iter().filter(|row| !self.subscriber_is_hidden(row, 0)));
        rows
    }

    fn subscriber_is_hidden(&self, row: &Value, issi: u64) -> bool {
        if let Some(entry) = self.settings.directory.subscriber(issi) {
            if entry.hidden.unwrap_or(false) || entry.hide_in_subscribers.unwrap_or(false) {
                return true;
            }
            if entry.hide_in_subscribers == Some(false) {
                return false;
            }
        }

        if self.settings.directory.hide_infrastructure {
            if let Some(class) = issi_class_label(issi) {
                if class == "Infrastruktur" || class == "Gateway" {
                    return true;
                }
            }
            for field in ["type", "device_type", "kind", "role", "class", "device_class"] {
                if let Some(text) = str_at(row, &[field]) {
                    let lower = text.to_ascii_lowercase();
                    if lower.contains("basis")
                        || lower.contains("base")
                        || lower.contains("station")
                        || lower.contains("infrastruktur")
                        || lower.contains("infrastructure")
                        || lower.contains("gateway")
                        || lower.contains("node")
                        || lower.contains("tbs")
                    {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn subscriber_display_name(&self, row: &Value) -> String {
        let issi = u64_at(row, &["issi"]).or_else(|| u64_at(row, &["individual_issi"])).unwrap_or(0);
        self.best_device_name_for_issi_or_row(issi, Some(row)).unwrap_or_else(|| "-".to_string())
    }

    fn directory_name_for_issi(&self, issi: u64) -> Option<String> {
        if let Some(value) = self.directory_name_index.get(&issi) {
            let value = value.trim();
            if !value.is_empty() && value != "-" && value != issi.to_string() {
                return Some(value.to_string());
            }
        }

        let entry = self.settings.directory.subscriber(issi)?;
        first_config_text(&[entry.display_name.as_ref(), entry.name.as_ref(), entry.label.as_ref(), entry.alias.as_ref()])
            .or_else(|| first_extra_string(&entry.extra, &[
                "rufname",
                "callsign",
                "call_sign",
                "radio_alias",
                "radioAlias",
                "short_name",
                "shortName",
                "shortLabel",
                "terminal_name",
                "terminalName",
                "bezeichnung",
                "description",
                "display",
            ]))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty() && value != "-" && value != &issi.to_string())
    }


    fn best_device_name_for_issi_or_row(&self, issi: u64, row: Option<&Value>) -> Option<String> {
        let directory_name = self.directory_name_for_issi(issi)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty() && value != "-" && value != &issi.to_string());

        if directory_name.is_some() {
            return directory_name;
        }

        row.and_then(|row| first_string(row, &[
                "display_name",
                "displayName",
                "label",
                "name",
                "alias",
                "radio_alias",
                "radioAlias",
                "short_name",
                "shortName",
                "shortLabel",
                "callsign",
                "call_sign",
                "rufname",
                "title",
            ]))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty() && value != "-" && value != &issi.to_string())
    }

    fn subscriber_type_label(&self, row: &Value) -> String {
        let issi = u64_at(row, &["issi"]).or_else(|| u64_at(row, &["individual_issi"])).unwrap_or(0);
        if let Some(entry) = self.settings.directory.subscriber(issi) {
            if let Some(value) = first_config_text(&[entry.device_class.as_ref(), entry.class.as_ref(), entry.kind.as_ref()]) {
                return value;
            }
        }
        first_string(row, &["device_class", "class", "kind", "type", "radio_type", "terminal_type"])
            .or_else(|| issi_class_label(issi).map(|value| value.to_string()))
            .unwrap_or_else(|| "-".to_string())
    }

    fn subscriber_status_label(&self, row: &Value) -> String {
        let issi = u64_at(row, &["issi"]).or_else(|| u64_at(row, &["individual_issi"])).unwrap_or(0);
        if let Some(entry) = self.settings.directory.subscriber(issi) {
            if let Some(status) = entry.status.as_ref().filter(|value| !value.trim().is_empty()) {
                return status.trim().to_string();
            }
        }
        if let Some(text) = first_string(row, &["status_label", "status_text", "state", "registration_state"]) {
            return text;
        }
        if let Some(code) = u64_at(row, &["status"]).or_else(|| u64_at(row, &["status_code"])) {
            if let Some(entry) = self.settings.directory.status(code) {
                if let Some(label) = first_config_text(&[entry.label.as_ref(), entry.name.as_ref()]) {
                    return label;
                }
            }
            return "Status unbekannt".to_string();
        }
        if let Some(esm) = u64_at(row, &["energy_saving_mode"]) {
            return match esm {
                0 => "ESM aus".to_string(),
                1 => "ESM aktiv".to_string(),
                other => format!("ESM {other}"),
            };
        }
        if subscriber_online(row) { "online".to_string() } else { "offline".to_string() }
    }

    fn subscriber_status_group_label(&self, row: &Value) -> String {
        let issi = u64_at(row, &["issi"]).or_else(|| u64_at(row, &["individual_issi"])).unwrap_or(0);
        let raw = self.settings.directory.subscriber(issi)
            .and_then(|entry| entry.status_group.clone())
            .or_else(|| first_string(row, &["status_group", "status_group_id", "statusgroup"]));
        let raw = raw.or_else(|| {
            let code = u64_at(row, &["status"]).or_else(|| u64_at(row, &["status_code"]))?;
            self.settings.directory.status(code).and_then(|entry| entry.group.clone())
        });
        let Some(raw) = raw else { return "-".to_string(); };
        if let Some(entry) = self.settings.directory.status_group(&raw) {
            if let Some(label) = first_config_text(&[entry.label.as_ref(), entry.name.as_ref()]) {
                return label;
            }
        }
        raw
    }

    fn subscriber_groups_label(&self, row: &Value) -> String {
        let issi = u64_at(row, &["issi"]).or_else(|| u64_at(row, &["individual_issi"])).unwrap_or(0);
        let mut groups = group_ids_from_row(row);
        if groups.is_empty() {
            if let Some(entry) = self.settings.directory.subscriber(issi) {
                groups.extend(entry.groups.iter().copied());
                groups.extend(entry.static_groups.iter().copied());
            }
        }
        if groups.is_empty() {
            return "-".to_string();
        }
        let joined = groups
            .iter()
            .map(|gssi| self.format_group(*gssi))
            .collect::<Vec<_>>()
            .join(", ");
        truncate(&joined, 110)
    }

    fn group_display_name(&self, gssi: u64) -> String {
        if let Some(entry) = self.settings.directory.group(gssi) {
            if let Some(name) = first_config_text(&[entry.name.as_ref(), entry.label.as_ref()]) {
                return name;
            }
        }
        if gssi > 0 { format!("GSSI {gssi}") } else { "-".to_string() }
    }

    fn group_type_label(&self, gssi: u64) -> String {
        self.settings.directory.group(gssi)
            .and_then(|entry| first_config_text(&[entry.kind.as_ref(), entry.description.as_ref()]))
            .unwrap_or_else(|| "-".to_string())
    }

    fn format_group(&self, gssi: u64) -> String {
        let name = self.group_display_name(gssi);
        if name == format!("GSSI {gssi}") {
            name
        } else {
            format!("{name} ({gssi})")
        }
    }

    fn group_members_label(&self, row: &Value) -> String {
        let members = group_ids_from_path(row, &["members"]);
        if members.is_empty() {
            return join_array(row, &["members"]);
        }
        let joined = members
            .iter()
            .map(|issi| self.format_issi_with_name(*issi))
            .collect::<Vec<_>>()
            .join(", ");
        truncate(&joined, 120)
    }

    fn format_issi_with_name(&self, issi: u64) -> String {
        if issi == 0 {
            return "-".to_string();
        }
        match self.directory_name_for_issi(issi) {
            Some(name) => format!("{name} ({issi})"),
            None => issi.to_string(),
        }
    }


    fn render_commands(&self, ui: &mut egui::Ui) {
        ui.heading("Command-/Audit-Log");
        let Some(value) = &self.commands else { ui.label("Noch keine Daten"); return; };
        let rows = value.as_array().cloned().or_else(|| value.get("commands").and_then(Value::as_array).cloned()).unwrap_or_default();
        table(ui, "commands_table", &["Command ID", "Node", "Operator", "Status", "Message", "Issued", "Updated"], rows.iter().collect(), |ui, row| {
            ui.monospace(str_at(row, &["command_id"]).unwrap_or("?"));
            ui.label(str_at(row, &["target_node_id"]).unwrap_or("?"));
            ui.label(str_at(row, &["operator_id"]).unwrap_or("?"));
            ui.label(str_at(row, &["status"]).unwrap_or("?"));
            ui.label(str_at(row, &["message"]).unwrap_or(""));
            ui.small(str_at(row, &["issued_at"]).unwrap_or("?"));
            ui.small(str_at(row, &["updated_at"]).unwrap_or("?"));
        });
    }

    fn render_login_screen(&mut self, ctx: &egui::Context) {
        ctx.style_mut(|style| {
            style.spacing.item_spacing = egui::vec2(10.0, 9.0);
            style.spacing.button_padding = egui::vec2(14.0, 9.0);
            style.spacing.text_edit_width = 360.0;
            style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(238, 242, 247);
            style.visuals.selection.bg_fill = egui::Color32::from_rgb(0, 118, 214);
        });

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::from_rgb(235, 240, 246)))
            .show(ctx, |ui| {
                let available = ui.available_size();
                let card_width = available.x.clamp(420.0, 580.0);
                let top_space = ((available.y - 420.0) * 0.35).clamp(32.0, 160.0);
                ui.add_space(top_space);

                ui.vertical_centered(|ui| {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.set_width(card_width);
                        ui.add_space(10.0);
                        ui.vertical_centered(|ui| {
                            ui.heading(egui::RichText::new("NetCore Control Room").size(26.0));
                            ui.colored_label(egui::Color32::from_rgb(0, 118, 214), UI_VERSION_LABEL);
                            ui.add_space(6.0);
                            ui.label("Einsatzleitplatz / Operator Login");
                        });
                        ui.separator();
                        ui.horizontal_wrapped(|ui| {
                            ui.small(format!("API: {}", self.settings.api));
                            ui.separator();
                            ui.small(format!("Profil: {}", self.settings.profile));
                        });
                        ui.add_space(12.0);
                        ui.label("Benutzername");
                        ui.add_sized([ui.available_width(), 34.0], egui::TextEdit::singleline(&mut self.login_username));
                        ui.label("Passwort");
                        let response = ui.add_sized(
                            [ui.available_width(), 34.0],
                            egui::TextEdit::singleline(&mut self.login_password).password(true),
                        );
                        if response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                            self.login();
                        }
                        ui.add_space(12.0);
                        if ui.add_sized([ui.available_width(), 38.0], egui::Button::new("Anmelden")).clicked() {
                            self.login();
                        }
                        if let Some(result) = &self.login_result {
                            ui.add_space(8.0);
                            ui.colored_label(egui::Color32::RED, result);
                        }
                        ui.add_space(8.0);
                        ui.small("User+Passwort + RBAC. Der TBS-Node-Token bleibt reine Maschinen-Auth für /node.");
                        if let Some(warning) = &self.startup_warning {
                            ui.colored_label(egui::Color32::YELLOW, warning);
                        }
                    });
                });
            });
    }

    fn render_admin_users(&mut self, ui: &mut egui::Ui) {
        if !self.is_admin() {
            ui.heading("Kein Zugriff");
            ui.label("Benutzerverwaltung ist nur für Admins sichtbar.");
            return;
        }
        ui.heading("Admin / Benutzer & RBAC");
        ui.label("Klassische Benutzerverwaltung: Username + Passwort + Rolle. Keine Operator-Tokens mehr.");
        ui.separator();

        ui.group(|ui| {
            ui.heading("Neuen Benutzer erstellen");
            egui::Grid::new("new_user_grid").show(ui, |ui| {
                ui.label("Username");
                ui.add_sized([260.0, 26.0], egui::TextEdit::singleline(&mut self.new_user_username));
                ui.end_row();
                ui.label("Anzeigename");
                ui.add_sized([260.0, 26.0], egui::TextEdit::singleline(&mut self.new_user_display_name));
                ui.end_row();
                ui.label("Passwort");
                ui.add_sized([260.0, 26.0], egui::TextEdit::singleline(&mut self.new_user_password).password(true));
                ui.end_row();
                ui.label("Role");
                egui::ComboBox::from_id_source("user_role_combo")
                    .selected_text(&self.new_user_role)
                    .show_ui(ui, |ui| {
                        for role in ["viewer", "operator", "admin"] {
                            ui.selectable_value(&mut self.new_user_role, role.to_string(), role);
                        }
                    });
                ui.end_row();
            });
            if ui.button("Benutzer erstellen").clicked() {
                self.create_user();
            }
            if let Some(result) = &self.user_result {
                ui.separator();
                ui.label("Ergebnis:");
                egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| ui.monospace(result));
            }
        });

        ui.separator();
        let Some(value) = &self.admin_users else { ui.label("Noch keine Daten"); return; };
        if let Some(error) = str_at(value, &["error"]) {
            ui.colored_label(egui::Color32::YELLOW, format!("Benutzerliste nicht verfügbar: {error}"));
            ui.label("Das ist normal, wenn du nur operator statt admin bist.");
            return;
        }

        let mut action: Option<UserAction> = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("users_grid").striped(true).show(ui, |ui| {
                header_row(ui, &["Username", "Name", "Role", "Enabled", "Created", "Last login", "Aktion"]);
                for user in array_at(value, &["users"]) {
                    let username = str_at(user, &["username"]).unwrap_or("?").to_string();
                    let enabled = bool_at(user, &["enabled"]).unwrap_or(false);
                    ui.monospace(&username);
                    ui.label(str_at(user, &["display_name"]).unwrap_or("?"));
                    ui.label(str_at(user, &["role"]).unwrap_or("?"));
                    bool_label(ui, enabled);
                    ui.small(str_at(user, &["created_at"]).unwrap_or("?"));
                    ui.small(str_at(user, &["last_login_at"]).unwrap_or("-"));
                    ui.horizontal(|ui| {
                        if enabled {
                            if ui.button("Disable").clicked() {
                                action = Some(UserAction::SetEnabled(username.clone(), false));
                            }
                        } else if ui.button("Enable").clicked() {
                            action = Some(UserAction::SetEnabled(username.clone(), true));
                        }
                        if ui.button("Delete").clicked() {
                            action = Some(UserAction::Delete(username.clone()));
                        }
                    });
                    ui.end_row();
                }
            });
        });
        if let Some(action) = action {
            match action {
                UserAction::SetEnabled(username, enabled) => self.set_user_enabled(&username, enabled),
                UserAction::Delete(username) => self.delete_user(&username),
            }
        }
    }

    fn render_raw(&self, ui: &mut egui::Ui) {
        ui.heading("Raw JSON");
        egui::ScrollArea::vertical().show(ui, |ui| {
            raw_block(ui, "overview", &self.overview);
            raw_block(ui, "subscribers", &self.subscribers);
            raw_block(ui, "groups", &self.groups);
            raw_block(ui, "calls", &self.calls);
            raw_block(ui, "sds", &self.sds);
            raw_block(ui, "locations", &self.locations);
            raw_block(ui, "commands", &self.commands);
            raw_block(ui, "admin_users", &self.admin_users);
        });
    }
}

#[derive(Debug, Clone)]
struct StatusTableauCard {
    id: String,
    title: String,
    subtitle: String,
    devices: Vec<StatusTableauDevice>,
    is_single_bucket: bool,
}

#[derive(Debug, Clone)]
struct StatusTableauDevice {
    issi: u64,
    name: String,
    device_class: String,
    status_code: u64,
    status_text: String,
    status_group: String,
    groups: Vec<u64>,
    last_seen: String,
    online: bool,
}

struct LocationPoint {
    issi: u64,
    lat: f64,
    lon: f64,
    source: String,
    updated_at: String,
}

#[derive(Debug, Clone)]
struct MapCluster {
    center: egui::Pos2,
    members: Vec<usize>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
struct TileKey {
    z: u8,
    x: u32,
    y: u32,
}

enum TileEntry {
    Ready(egui::TextureHandle),
    Loading(Receiver<Result<Vec<u8>, String>>),
    Failed { message: String, last_try: Instant },
}

struct MapTileCache {
    settings: MapSettings,
    entries: HashMap<TileKey, TileEntry>,
    http: reqwest::blocking::Client,
}

impl MapTileCache {
    fn new(settings: MapSettings) -> Self {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(8))
            .user_agent("NetCore-Control-Room-UI/1.3 map-client")
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());
        Self {
            settings,
            entries: HashMap::new(),
            http,
        }
    }

    fn texture_id(&mut self, ctx: &egui::Context, z: u8, x: u32, y: u32, allow_online: bool) -> Result<Option<egui::TextureId>, String> {
        let key = TileKey { z, x, y };
        let entry = match self.entries.remove(&key) {
            Some(entry) => entry,
            None => TileEntry::Loading(self.spawn_loading(key, allow_online)),
        };

        match entry {
            TileEntry::Ready(texture) => {
                let id = texture.id();
                self.entries.insert(key, TileEntry::Ready(texture));
                Ok(Some(id))
            }
            TileEntry::Loading(receiver) => match receiver.try_recv() {
                Ok(Ok(bytes)) => {
                    let image = decode_tile(&bytes).map_err(|error| format!("Tile {z}/{x}/{y}: {error}"))?;
                    let texture = ctx.load_texture(
                        format!("netcore_map_tile_{z}_{x}_{y}"),
                        image,
                        egui::TextureOptions::LINEAR,
                    );
                    let id = texture.id();
                    self.entries.insert(key, TileEntry::Ready(texture));
                    Ok(Some(id))
                }
                Ok(Err(message)) => {
                    self.entries.insert(key, TileEntry::Failed { message: message.clone(), last_try: Instant::now() });
                    Err(message)
                }
                Err(TryRecvError::Empty) => {
                    self.entries.insert(key, TileEntry::Loading(receiver));
                    ctx.request_repaint_after(Duration::from_millis(40));
                    Ok(None)
                }
                Err(TryRecvError::Disconnected) => {
                    let message = format!("Tile {z}/{x}/{y}: Lade-Thread beendet");
                    self.entries.insert(key, TileEntry::Failed { message: message.clone(), last_try: Instant::now() });
                    Err(message)
                }
            },
            TileEntry::Failed { message, last_try } => {
                if allow_online && last_try.elapsed() >= Duration::from_secs(20) {
                    self.entries.insert(key, TileEntry::Loading(self.spawn_loading(key, allow_online)));
                    ctx.request_repaint_after(Duration::from_millis(40));
                    Ok(None)
                } else {
                    self.entries.insert(key, TileEntry::Failed { message: message.clone(), last_try });
                    Err(message)
                }
            }
        }
    }

    fn spawn_loading(&self, key: TileKey, allow_online: bool) -> Receiver<Result<Vec<u8>, String>> {
        let path = self.tile_path(key);
        let url = self.tile_url(key);
        let online_tiles = self.settings.online_tiles;
        let client = self.http.clone();
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let result = load_tile_bytes(client, path, url, key, allow_online && online_tiles);
            let _ = sender.send(result);
        });
        receiver
    }

    fn tile_url(&self, key: TileKey) -> String {
        self.settings
            .tile_url
            .replace("{z}", &key.z.to_string())
            .replace("{x}", &key.x.to_string())
            .replace("{y}", &key.y.to_string())
    }

    fn tile_path(&self, key: TileKey) -> PathBuf {
        self.settings
            .cache_dir
            .join(key.z.to_string())
            .join(key.x.to_string())
            .join(format!("{}.png", key.y))
    }
}

fn load_tile_bytes(client: reqwest::blocking::Client, path: PathBuf, url: String, key: TileKey, allow_online: bool) -> Result<Vec<u8>, String> {
    if let Ok(bytes) = fs::read(&path) {
        return Ok(bytes);
    }
    if !allow_online {
        return Err(format!("nicht im Cache: {}/{}/{}", key.z, key.x, key.y));
    }
    let response = client.get(&url).send().map_err(|error| error.to_string())?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("{} {}", status, url));
    }
    let bytes = response.bytes().map_err(|error| error.to_string())?.to_vec();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(&path, &bytes);
    Ok(bytes)
}


fn decode_tile(bytes: &[u8]) -> Result<egui::ColorImage, image::ImageError> {
    let image = image::load_from_memory(bytes)?.to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    Ok(egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw()))
}

#[derive(Debug, Copy, Clone)]
struct WorldPoint {
    x: f64,
    y: f64,
}

#[derive(Debug, Copy, Clone)]
struct MapViewport {
    center_lat: f64,
    center_lon: f64,
    zoom: u8,
    top_left_world: WorldPoint,
}

impl MapViewport {
    fn for_state(
        points: &[LocationPoint],
        map_rect: egui::Rect,
        settings: &MapSettings,
        follow_latest: bool,
        manual_center: Option<(f64, f64)>,
        zoom_adjust: i32,
    ) -> Self {
        let mut center_lat = settings.default_lat;
        let mut center_lon = settings.default_lon;
        let mut zoom = settings.default_zoom;

        if !follow_latest {
            if let Some((lat, lon)) = manual_center {
                center_lat = lat.clamp(-85.0, 85.0);
                center_lon = normalize_lon(lon);
            } else if let Some(point) = points.last() {
                center_lat = point.lat.clamp(-85.0, 85.0);
                center_lon = normalize_lon(point.lon);
            }
        } else if !points.is_empty() {
            let mut min_lat = f64::INFINITY;
            let mut max_lat = f64::NEG_INFINITY;
            let mut min_lon = f64::INFINITY;
            let mut max_lon = f64::NEG_INFINITY;
            for point in points {
                min_lat = min_lat.min(point.lat);
                max_lat = max_lat.max(point.lat);
                min_lon = min_lon.min(point.lon);
                max_lon = max_lon.max(point.lon);
            }
            center_lat = ((min_lat + max_lat) / 2.0).clamp(-85.0, 85.0);
            center_lon = normalize_lon((min_lon + max_lon) / 2.0);
            zoom = choose_zoom(min_lat, max_lat, min_lon, max_lon, map_rect, settings);
        }

        let adjusted_zoom = (zoom as i32 + zoom_adjust).clamp(settings.min_zoom as i32, settings.max_zoom as i32) as u8;
        let center_world = lat_lon_to_world(center_lat, center_lon, adjusted_zoom);
        let top_left_world = WorldPoint {
            x: center_world.x - map_rect.width() as f64 / 2.0,
            y: center_world.y - map_rect.height() as f64 / 2.0,
        };

        let (center_lat, center_lon) = world_to_lat_lon(center_world, adjusted_zoom);
        Self {
            center_lat: center_lat.clamp(-85.0, 85.0),
            center_lon,
            zoom: adjusted_zoom,
            top_left_world,
        }
    }

    fn lat_lon_to_screen(&self, lat: f64, lon: f64, map_rect: egui::Rect) -> egui::Pos2 {
        let world = lat_lon_to_world(lat, lon, self.zoom);
        egui::pos2(
            map_rect.left() + (world.x - self.top_left_world.x) as f32,
            map_rect.top() + (world.y - self.top_left_world.y) as f32,
        )
    }

    fn screen_to_lat_lon(&self, pos: egui::Pos2, map_rect: egui::Rect) -> (f64, f64) {
        let world = WorldPoint {
            x: self.top_left_world.x + (pos.x - map_rect.left()) as f64,
            y: self.top_left_world.y + (pos.y - map_rect.top()) as f64,
        };
        world_to_lat_lon(world, self.zoom)
    }
}

fn choose_zoom(min_lat: f64, max_lat: f64, min_lon: f64, max_lon: f64, map_rect: egui::Rect, settings: &MapSettings) -> u8 {
    if (max_lat - min_lat).abs() < 0.000_1 && (max_lon - min_lon).abs() < 0.000_1 {
        return settings.default_zoom.clamp(settings.min_zoom, settings.max_zoom);
    }
    let padding = 0.82;
    for zoom in (settings.min_zoom..=settings.max_zoom).rev() {
        let p1 = lat_lon_to_world(min_lat, min_lon, zoom);
        let p2 = lat_lon_to_world(max_lat, max_lon, zoom);
        let width = (p2.x - p1.x).abs().max(1.0);
        let height = (p2.y - p1.y).abs().max(1.0);
        if width <= map_rect.width() as f64 * padding && height <= map_rect.height() as f64 * padding {
            return zoom;
        }
    }
    settings.min_zoom
}

fn lat_lon_to_world(lat: f64, lon: f64, zoom: u8) -> WorldPoint {
    let lat = lat.clamp(-85.051_128_78, 85.051_128_78);
    let lon = normalize_lon(lon);
    let scale = TILE_SIZE * 2.0_f64.powi(zoom as i32);
    let x = (lon + 180.0) / 360.0 * scale;
    let lat_rad = lat.to_radians();
    let y = (1.0 - ((lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / std::f64::consts::PI)) / 2.0 * scale;
    WorldPoint { x, y }
}

fn world_to_lat_lon(world: WorldPoint, zoom: u8) -> (f64, f64) {
    let scale = TILE_SIZE * 2.0_f64.powi(zoom as i32);
    let lon = world.x / scale * 360.0 - 180.0;
    let n = std::f64::consts::PI - 2.0 * std::f64::consts::PI * world.y / scale;
    let lat = n.sinh().atan().to_degrees();
    (lat, normalize_lon(lon))
}

fn normalize_lon(lon: f64) -> f64 {
    let mut lon = lon;
    while lon < -180.0 { lon += 360.0; }
    while lon > 180.0 { lon -= 360.0; }
    lon
}

fn collect_points(rows: &[&Value]) -> Vec<LocationPoint> {
    rows.iter()
        .filter_map(|row| {
            let lat = f64_at(row, &["latitude"])?;
            let lon = f64_at(row, &["longitude"])?;
            if !lat.is_finite() || !lon.is_finite() {
                return None;
            }
            Some(LocationPoint {
                issi: u64_any_at(row, &["issi"]).or_else(|| u64_any_at(row, &["individual_issi"])).or_else(|| u64_any_at(row, &["source_issi"])).or_else(|| u64_any_at(row, &["address"])).unwrap_or(0),
                lat,
                lon,
                source: str_at(row, &["source"]).unwrap_or("-").to_string(),
                updated_at: str_at(row, &["updated_at"]).unwrap_or("?").to_string(),
            })
        })
        .collect()
}


fn latest_location_rows<'a>(rows: &[&'a Value]) -> Vec<&'a Value> {
    let mut latest_by_issi: HashMap<u64, &'a Value> = HashMap::new();
    let mut unkeyed_rows: Vec<&'a Value> = Vec::new();

    for &row in rows {
        if let Some(issi) = u64_at(row, &["issi"]) {
            let replace = latest_by_issi
                .get(&issi)
                .map(|current| location_row_is_newer(row, current))
                .unwrap_or(true);
            if replace {
                latest_by_issi.insert(issi, row);
            }
        } else {
            // Keep rows without ISSI instead of collapsing them into one pseudo-device.
            unkeyed_rows.push(row);
        }
    }

    let mut latest: Vec<&'a Value> = latest_by_issi.into_values().collect();
    latest.extend(unkeyed_rows);
    latest.sort_by(|left, right| {
        location_timestamp(right)
            .cmp(location_timestamp(left))
            .then_with(|| u64_at(left, &["issi"]).unwrap_or(0).cmp(&u64_at(right, &["issi"]).unwrap_or(0)))
    });
    latest
}

fn location_row_is_newer(candidate: &Value, current: &Value) -> bool {
    let candidate_time = location_timestamp(candidate);
    let current_time = location_timestamp(current);
    if candidate_time != current_time {
        return candidate_time > current_time;
    }

    // Same timestamp: prefer the later row from the API response. This avoids
    // sticky stale details if the backend sends a corrected position with the
    // same update time.
    true
}

fn location_timestamp(row: &Value) -> &str {
    str_at(row, &["updated_at"])
        .or_else(|| str_at(row, &["timestamp"]))
        .or_else(|| str_at(row, &["created_at"]))
        .unwrap_or("")
}



fn build_map_clusters(points: &[LocationPoint], map_rect: egui::Rect, viewport: &MapViewport, threshold: f32) -> Vec<MapCluster> {
    let mut clusters: Vec<MapCluster> = Vec::new();
    let threshold_sq = threshold * threshold;

    for (index, point) in points.iter().enumerate() {
        let pos = viewport.lat_lon_to_screen(point.lat, point.lon, map_rect);
        if !map_rect.expand(48.0).contains(pos) {
            continue;
        }

        if let Some(cluster) = clusters.iter_mut().find(|cluster| (cluster.center - pos).length_sq() <= threshold_sq) {
            let old_len = cluster.members.len() as f32;
            cluster.center = egui::pos2(
                (cluster.center.x * old_len + pos.x) / (old_len + 1.0),
                (cluster.center.y * old_len + pos.y) / (old_len + 1.0),
            );
            cluster.members.push(index);
        } else {
            clusters.push(MapCluster { center: pos, members: vec![index] });
        }
    }

    clusters.sort_by(|left, right| right.members.len().cmp(&left.members.len()));
    clusters
}

fn cluster_contains_issi(cluster: &MapCluster, points: &[LocationPoint], issi: u64) -> bool {
    cluster.members.iter().any(|index| points[*index].issi == issi)
}

fn cluster_radius(count: usize) -> f32 {
    (15.0 + (count as f32).sqrt() * 3.5).clamp(18.0, 34.0)
}

fn spider_position(center: egui::Pos2, index: usize, total: usize) -> egui::Pos2 {
    if total <= 1 {
        return center;
    }
    let radius = (30.0 + total as f32 * 3.0).clamp(34.0, 76.0);
    let angle = std::f32::consts::TAU * index as f32 / total as f32 - std::f32::consts::FRAC_PI_2;
    center + egui::vec2(angle.cos() * radius, angle.sin() * radius)
}

fn compact_marker_label(label: &str) -> String {
    let label = label.trim();
    if label.is_empty() {
        return "?".to_string();
    }
    let mut chars = label.chars();
    let mut out = String::new();
    for _ in 0..18 {
        if let Some(ch) = chars.next() {
            out.push(ch);
        } else {
            return out;
        }
    }
    out.push('…');
    out
}


fn merge_subscriber_missing(entry: &mut DirectorySubscriberConfig, other: &DirectorySubscriberConfig) {
    if entry.name.is_none() { entry.name = other.name.clone(); }
    if entry.label.is_none() { entry.label = other.label.clone(); }
    if entry.display_name.is_none() { entry.display_name = other.display_name.clone(); }
    if entry.alias.is_none() { entry.alias = other.alias.clone(); }
    if entry.device_class.is_none() { entry.device_class = other.device_class.clone(); }
    if entry.class.is_none() { entry.class = other.class.clone(); }
    if entry.kind.is_none() { entry.kind = other.kind.clone(); }
    if entry.status.is_none() { entry.status = other.status.clone(); }
    if entry.status_group.is_none() { entry.status_group = other.status_group.clone(); }
    if entry.groups.is_empty() { entry.groups = other.groups.clone(); }
    if entry.static_groups.is_empty() { entry.static_groups = other.static_groups.clone(); }
    if entry.hidden.is_none() { entry.hidden = other.hidden; }
    if entry.hide_in_subscribers.is_none() { entry.hide_in_subscribers = other.hide_in_subscribers; }
    for (key, value) in &other.extra {
        entry.extra.entry(key.clone()).or_insert_with(|| value.clone());
    }
}

fn merge_label_missing(entry: &mut DirectoryLabelConfig, other: &DirectoryLabelConfig) {
    if entry.name.is_none() { entry.name = other.name.clone(); }
    if entry.label.is_none() { entry.label = other.label.clone(); }
    if entry.kind.is_none() { entry.kind = other.kind.clone(); }
    if entry.description.is_none() { entry.description = other.description.clone(); }
}

fn merge_status_missing(entry: &mut DirectoryStatusConfig, other: &DirectoryStatusConfig) {
    if entry.name.is_none() { entry.name = other.name.clone(); }
    if entry.label.is_none() { entry.label = other.label.clone(); }
    if entry.group.is_none() { entry.group = other.group.clone(); }
    if entry.description.is_none() { entry.description = other.description.clone(); }
}

fn directory_entry_has_name(entry: &DirectorySubscriberConfig) -> bool {
    directory_name_from_entry(entry).is_some()
}

fn directory_name_from_entry(entry: &DirectorySubscriberConfig) -> Option<String> {
    first_config_text(&[entry.display_name.as_ref(), entry.name.as_ref(), entry.label.as_ref(), entry.alias.as_ref()])
        .or_else(|| first_extra_string(&entry.extra, &[
            "rufname", "Rufname", "callsign", "callSign", "call_sign", "radio_alias",
            "radioAlias", "short_name", "shortName", "shortLabel", "terminal_name",
            "terminalName", "description", "bezeichnung", "Bezeichnung", "title",
        ]))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && value != "-")
}

fn raw_directory_name_for_id(value: &Value, id: u64) -> Option<String> {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                if text_matches_directory_id(key, id) {
                    if let Some(name) = raw_directory_name_from_value(child, id) {
                        return Some(name);
                    }
                }
            }
            if object_matches_directory_id(object, id) {
                if let Some(name) = raw_directory_name_from_object(object, id) {
                    return Some(name);
                }
            }
            for child in object.values() {
                if let Some(name) = raw_directory_name_for_id(child, id) {
                    return Some(name);
                }
            }
            None
        }
        Value::Array(items) => {
            for item in items {
                if let Some(name) = raw_directory_name_for_id(item, id) {
                    return Some(name);
                }
            }
            None
        }
        _ => None,
    }
}

fn raw_directory_name_from_value(value: &Value, id: u64) -> Option<String> {
    match value {
        Value::Object(object) => raw_directory_name_from_object(object, id),
        Value::String(text) => clean_directory_name_candidate(text, id),
        _ => None,
    }
}

fn raw_directory_name_from_object(object: &serde_json::Map<String, Value>, id: u64) -> Option<String> {
    let direct_keys = [
        "display_name", "displayName", "name", "label", "alias", "rufname", "Rufname",
        "callsign", "callSign", "call_sign", "radio_alias", "radioAlias", "short_name",
        "shortName", "shortLabel", "terminal_name", "terminalName", "bezeichnung", "Bezeichnung",
        "description", "title",
    ];
    for key in direct_keys {
        if let Some(value) = object.get(key) {
            if let Some(text) = value.as_str().and_then(|text| clean_directory_name_candidate(text, id)) {
                return Some(text);
            }
            if let Some(number) = value.as_u64() {
                let text = number.to_string();
                if !text_matches_directory_id(&text, id) {
                    return Some(text);
                }
            }
        }
    }
    for nested_key in ["subscriber", "terminal", "radio", "device", "meta", "profile"] {
        if let Some(Value::Object(nested)) = object.get(nested_key) {
            if let Some(name) = raw_directory_name_from_object(nested, id) {
                return Some(name);
            }
        }
    }
    None
}

fn object_matches_directory_id(object: &serde_json::Map<String, Value>, id: u64) -> bool {
    let id_keys = [
        "issi", "individual_issi", "source_issi", "subscriber_issi", "address", "id",
        "terminal_id", "terminalId", "radio_id", "radioId", "number", "issi_number",
    ];
    id_keys.iter().any(|key| object.get(*key).and_then(value_as_directory_id) == Some(id))
}

fn value_as_directory_id(value: &Value) -> Option<u64> {
    if let Some(number) = value.as_u64() { return Some(number); }
    if let Some(number) = value.as_i64() { return u64::try_from(number).ok(); }
    value.as_str().and_then(parse_directory_id_text)
}

fn parse_directory_id_text(text: &str) -> Option<u64> {
    let trimmed = text.trim();
    if trimmed.is_empty() { return None; }
    trimmed.parse::<u64>().ok().or_else(|| trimmed.trim_start_matches('0').parse::<u64>().ok())
}

fn text_matches_directory_id(text: &str, id: u64) -> bool {
    parse_directory_id_text(text) == Some(id) || id_key_variants(id).iter().any(|key| key == text.trim())
}

fn clean_directory_name_candidate(text: &str, id: u64) -> Option<String> {
    let candidate = text.trim();
    if candidate.is_empty() || candidate == "-" { return None; }
    if id > 0 && text_matches_directory_id(candidate, id) { return None; }
    Some(candidate.to_string())
}

fn build_directory_name_index(value: &Value) -> HashMap<u64, String> {
    let mut index = HashMap::new();
    collect_directory_names(value, &mut index);
    index.retain(|issi, name| {
        let trimmed = name.trim();
        !trimmed.is_empty() && trimmed != "-" && trimmed != issi.to_string()
    });
    index
}

fn collect_directory_names(value: &Value, index: &mut HashMap<u64, String>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_directory_names(item, index);
            }
        }
        Value::Object(map) => {
            if let Some(issi) = directory_object_issi(map) {
                if let Some(name) = directory_object_name(map, issi) {
                    index.entry(issi).or_insert(name);
                }
            }

            for (key, child) in map {
                if let Ok(issi) = key.trim().parse::<u64>() {
                    if let Some(child_map) = child.as_object() {
                        if let Some(name) = directory_object_name(child_map, issi) {
                            index.entry(issi).or_insert(name);
                        }
                    } else if let Some(text) = child.as_str().map(str::trim).filter(|text| !text.is_empty()) {
                        if text != issi.to_string() {
                            index.entry(issi).or_insert(text.to_string());
                        }
                    }
                }
                collect_directory_names(child, index);
            }
        }
        _ => {}
    }
}

fn directory_object_issi(map: &serde_json::Map<String, Value>) -> Option<u64> {
    for key in [
        "issi",
        "individual_issi",
        "source_issi",
        "address",
        "id",
        "subscriber_id",
        "subscriberId",
        "terminal_id",
        "terminalId",
        "radio_id",
        "radioId",
    ] {
        let Some(value) = map.get(key) else { continue; };
        if let Some(number) = value.as_u64() {
            return Some(number);
        }
        if let Some(number) = value.as_i64().and_then(|value| u64::try_from(value).ok()) {
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

fn directory_object_name(map: &serde_json::Map<String, Value>, issi: u64) -> Option<String> {
    for key in [
        "name",
        "display_name",
        "displayName",
        "label",
        "alias",
        "rufname",
        "callsign",
        "call_sign",
        "radio_alias",
        "radioAlias",
        "short_name",
        "shortName",
        "shortLabel",
        "terminal_name",
        "terminalName",
        "bezeichnung",
        "description",
        "title",
    ] {
        let Some(value) = map.get(key) else { continue; };
        let text = match value {
            Value::String(text) => text.trim().to_string(),
            Value::Number(number) => number.to_string(),
            _ => continue,
        };
        if !text.is_empty() && text != "-" && text != issi.to_string() {
            return Some(text);
        }
    }
    None
}

fn estimate_status_card_height(card: &StatusTableauCard) -> f32 {
    let row_count = card.devices.len().max(1) as f32;
    42.0 + row_count * 49.0 + 12.0
}

fn normalize_directory_value(mut value: Value) -> Value {
    if let Some(inner) = value.get("directory").cloned() {
        value = inner;
    }

    let Some(object) = value.as_object_mut() else {
        return value;
    };

    normalize_directory_collection(object, "subscribers", &["issi", "individual_issi", "source_issi", "id", "address"]);
    normalize_directory_collection(object, "groups", &["gssi", "group", "id", "address"]);
    normalize_directory_collection(object, "status_groups", &["id", "key", "name", "label"]);
    normalize_directory_collection(object, "statuses", &["code", "status", "id", "number"]);

    value
}

fn normalize_directory_collection(object: &mut serde_json::Map<String, Value>, field: &str, key_fields: &[&str]) {
    let Some(value) = object.get_mut(field) else {
        return;
    };
    let Some(items) = value.as_array() else {
        return;
    };

    let mut map = serde_json::Map::new();
    for item in items {
        let Some(key) = directory_item_key(item, key_fields) else {
            continue;
        };
        map.insert(key, item.clone());
    }
    *value = Value::Object(map);
}

fn directory_item_key(item: &Value, key_fields: &[&str]) -> Option<String> {
    for key in key_fields {
        if let Some(text) = str_at(item, &[*key]).map(str::trim).filter(|text| !text.is_empty()) {
            return Some(text.to_string());
        }
        if let Some(number) = u64_any_at(item, &[*key]) {
            return Some(number.to_string());
        }
    }
    None
}

fn id_key_variants(id: u64) -> Vec<String> {
    let mut keys = vec![id.to_string(), format!("{id:07}"), format!("{id:08}")];
    keys.sort();
    keys.dedup();
    keys
}

fn first_config_text(values: &[Option<&String>]) -> Option<String> {
    values
        .iter()
        .filter_map(|value| value.as_ref())
        .map(|value| value.trim())
        .find(|value| !value.is_empty())
        .map(|value| value.to_string())
}

fn first_extra_string(values: &HashMap<String, Value>, keys: &[&str]) -> Option<String> {
    for key in keys {
        let Some(value) = values.get(*key) else { continue; };
        if let Some(text) = value.as_str().map(str::trim).filter(|text| !text.is_empty()) {
            return Some(text.to_string());
        }
        if let Some(number) = value.as_u64() {
            return Some(number.to_string());
        }
    }
    None
}

fn first_string(value: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(text) = str_at(value, &[*key]).map(str::trim).filter(|text| !text.is_empty()) {
            return Some(text.to_string());
        }
        if let Some(number) = u64_any_at(value, &[*key]) {
            return Some(number.to_string());
        }
    }
    None
}

fn issi_class_label(issi: u64) -> Option<&'static str> {
    if issi == 0 {
        return None;
    }
    match (issi / 1_000_000) % 10 {
        1 => Some("LST"),
        2 => Some("HRT"),
        3 => Some("MRT"),
        4 => Some("Infrastruktur"),
        5 => Some("Gateway"),
        _ => None,
    }
}

fn subscriber_online(row: &Value) -> bool {
    if let Some(value) = bool_at(row, &["online"]) {
        return value;
    }
    for key in ["state", "status", "registration_state"] {
        if let Some(text) = str_at(row, &[key]) {
            let lower = text.to_ascii_lowercase();
            return lower.contains("online") || lower.contains("registered") || lower.contains("attached");
        }
    }
    false
}

fn subscriber_row_is_newer(candidate: &Value, current: &Value) -> bool {
    let candidate_time = subscriber_timestamp(candidate);
    let current_time = subscriber_timestamp(current);
    if candidate_time != current_time {
        return candidate_time > current_time;
    }
    true
}

fn subscriber_timestamp(row: &Value) -> &str {
    str_at(row, &["last_seen"])
        .or_else(|| str_at(row, &["updated_at"]))
        .or_else(|| str_at(row, &["timestamp"]))
        .or_else(|| str_at(row, &["created_at"]))
        .unwrap_or("")
}

fn group_ids_from_row(row: &Value) -> Vec<u64> {
    let mut ids = group_ids_from_path(row, &["groups"]);
    ids.extend(group_ids_from_path(row, &["static_groups"]));
    ids.extend(group_ids_from_path(row, &["group_ids"]));
    ids.extend(group_ids_from_path(row, &["attached_groups"]));
    ids.sort_unstable();
    ids.dedup();
    ids
}

fn group_ids_from_path(value: &Value, path: &[&str]) -> Vec<u64> {
    let Some(values) = get_at(value, path).and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut ids = Vec::new();
    for value in values {
        if let Some(id) = value.as_u64() {
            ids.push(id);
        } else if let Some(text) = value.as_str() {
            if let Ok(id) = text.trim().parse::<u64>() {
                ids.push(id);
            }
        } else if let Some(id) = value.get("gssi").and_then(Value::as_u64).or_else(|| value.get("id").and_then(Value::as_u64)) {
            ids.push(id);
        }
    }
    ids.sort_unstable();
    ids.dedup();
    ids
}

fn sds_source_issi(row: &Value) -> Option<u64> {
    u64_any_at(row, &["source_issi"])
        .or_else(|| u64_any_at(row, &["source"]))
        .or_else(|| u64_any_at(row, &["src_issi"]))
        .or_else(|| u64_any_at(row, &["src"]))
        .or_else(|| u64_any_at(row, &["from_issi"]))
        .or_else(|| u64_any_at(row, &["from"]))
        .or_else(|| u64_any_at(row, &["sender_issi"]))
        .or_else(|| u64_any_at(row, &["sender"]))
        .or_else(|| u64_any_at(row, &["issi"]))
}

fn sds_protocol(row: &Value) -> Option<u64> {
    u64_any_at(row, &["protocol_id"])
        .or_else(|| u64_any_at(row, &["proto"]))
        .or_else(|| u64_any_at(row, &["protocol"]))
        .or_else(|| u64_any_at(row, &["pid"]))
}

fn sds_row_is_newer(candidate: &Value, current: &Value) -> bool {
    let candidate_time = sds_timestamp(candidate);
    let current_time = sds_timestamp(current);
    if candidate_time != current_time {
        return candidate_time > current_time;
    }
    true
}

fn sds_timestamp(row: &Value) -> &str {
    str_at(row, &["timestamp"])
        .or_else(|| str_at(row, &["time"]))
        .or_else(|| str_at(row, &["updated_at"]))
        .or_else(|| str_at(row, &["created_at"]))
        .unwrap_or("")
}

fn looks_like_status_text(text: &str) -> bool {
    let normalized = normalize_status_text(text);
    normalized.starts_with("status")
        || normalized.contains("frei")
        || normalized.contains("bereit")
        || normalized.contains("einsatz")
        || normalized.contains("transport")
        || normalized.contains("sonderstatus")
}

fn extract_status_label_from_text(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut value = trimmed;
    if let Some(rest) = trimmed.strip_prefix("Status:") {
        value = rest.trim();
    } else if let Some(rest) = trimmed.strip_prefix("status:") {
        value = rest.trim();
    }

    for separator in ["—", "–", " - ", " -- ", " => "] {
        if let Some((first, _)) = value.split_once(separator) {
            let candidate = first.trim();
            if !candidate.is_empty() {
                return Some(candidate.to_string());
            }
        }
    }

    Some(value.to_string())
}

fn normalize_status_text(text: &str) -> String {
    text.to_ascii_lowercase()
        .replace('—', " ")
        .replace('–', " ")
        .replace(':', " ")
        .replace('_', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

enum UserAction {
    SetEnabled(String, bool),
    Delete(String),
}

fn metric(ui: &mut egui::Ui, label: &str, value: String) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_min_width(110.0);
        ui.vertical_centered(|ui| {
            ui.heading(egui::RichText::new(value).size(24.0));
            ui.small(label);
        });
    });
}



fn status_pill(ui: &mut egui::Ui, label: &str, value: &str, ok: bool) {
    let color = if ok { egui::Color32::from_rgb(0, 130, 70) } else { egui::Color32::from_rgb(185, 40, 40) };
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.colored_label(color, "●");
            ui.strong(label);
            ui.monospace(value);
        });
    });
}


fn parse_status_code(value: &str) -> Option<u64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    trimmed.parse::<u64>().ok()
}

fn status_label_default(code: u64) -> &'static str {
    match code {
        1 => "Frei",
        2 => "Bereit",
        3 => "Sprechwunsch",
        4 => "Im Einsatz",
        5 => "Am Ziel",
        6 => "Nicht bereit",
        7 => "Transport",
        8 => "Sonderstatus",
        _ => "Status",
    }
}

fn status_colour_for_code(code: u64) -> egui::Color32 {
    match code {
        1 => egui::Color32::from_rgb(0, 230, 0),
        2 => egui::Color32::from_rgb(170, 235, 30),
        3 => egui::Color32::from_rgb(80, 210, 245),
        4 => egui::Color32::from_rgb(255, 130, 35),
        5 => egui::Color32::from_rgb(0, 230, 0),
        6 => egui::Color32::from_rgb(255, 0, 0),
        7 => egui::Color32::from_rgb(235, 95, 230),
        8 => egui::Color32::from_rgb(235, 160, 245),
        _ => egui::Color32::from_rgb(205, 205, 205),
    }
}

fn status_badge(ui: &mut egui::Ui, code: &str, label: &str, colour: egui::Color32) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(28.0, 22.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, egui::Rounding::same(4.0), colour);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            code,
            egui::FontId::proportional(14.0),
            egui::Color32::BLACK,
        );
        ui.small(label);
    });
}

fn table<F>(ui: &mut egui::Ui, id: &str, headers: &[&str], rows: Vec<&Value>, mut row_fn: F)
where
    F: FnMut(&mut egui::Ui, &Value),
{
    if rows.is_empty() {
        ui.label("Keine Einträge");
        return;
    }
    egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
        egui::Grid::new(id).striped(true).min_col_width(110.0).spacing(egui::vec2(12.0, 6.0)).show(ui, |ui| {
            header_row(ui, headers);
            for row in rows {
                row_fn(ui, row);
                ui.end_row();
            }
        });
    });
}

fn header_row(ui: &mut egui::Ui, headers: &[&str]) {
    for header in headers {
        ui.strong(*header);
    }
    ui.end_row();
}

fn bool_label(ui: &mut egui::Ui, value: bool) {
    if value {
        ui.colored_label(egui::Color32::GREEN, "ja");
    } else {
        ui.colored_label(egui::Color32::RED, "nein");
    }
}

fn tri_label(ui: &mut egui::Ui, value: Option<&Value>) {
    match value.and_then(Value::as_bool) {
        Some(true) => ui.colored_label(egui::Color32::GREEN, "ja"),
        Some(false) => ui.colored_label(egui::Color32::RED, "nein"),
        None => ui.label("-"),
    };
}

fn raw_block(ui: &mut egui::Ui, name: &str, value: &Option<Value>) {
    ui.collapsing(name, |ui| {
        if let Some(value) = value {
            ui.monospace(pretty(value));
        } else {
            ui.label("Keine Daten");
        }
    });
}

fn pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

fn parse_u32(value: &str, label: &str) -> Result<u32, String> {
    value.trim().parse::<u32>().map_err(|_| format!("{label} ist keine gültige Zahl: {value}"))
}

fn now_label() -> String {
    format!("{:?}", std::time::SystemTime::now())
}

fn get_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut cursor = value;
    for key in path {
        cursor = cursor.get(*key)?;
    }
    Some(cursor)
}

fn str_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    get_at(value, path)?.as_str()
}

fn u64_at(value: &Value, path: &[&str]) -> Option<u64> {
    get_at(value, path)?.as_u64()
}

fn u64_any_at(value: &Value, path: &[&str]) -> Option<u64> {
    let value = get_at(value, path)?;
    if let Some(number) = value.as_u64() {
        return Some(number);
    }
    if let Some(number) = value.as_i64() {
        return u64::try_from(number).ok();
    }
    if let Some(text) = value.as_str() {
        return text.trim().parse::<u64>().ok();
    }
    None
}

fn f64_at(value: &Value, path: &[&str]) -> Option<f64> {
    get_at(value, path)?.as_f64()
}

fn bool_at(value: &Value, path: &[&str]) -> Option<bool> {
    get_at(value, path)?.as_bool()
}

fn array_at<'a>(value: &'a Value, path: &[&str]) -> Vec<&'a Value> {
    get_at(value, path)
        .and_then(Value::as_array)
        .map(|values| values.iter().collect())
        .unwrap_or_default()
}

fn display_u64(value: &Value, path: &[&str]) -> String {
    u64_at(value, path).map(|v| v.to_string()).unwrap_or_else(|| "-".to_string())
}

fn display_f64(value: &Value, path: &[&str]) -> String {
    f64_at(value, path).map(|v| format!("{v:.2}")).unwrap_or_else(|| "-".to_string())
}

fn join_array(value: &Value, path: &[&str]) -> String {
    let Some(values) = get_at(value, path).and_then(Value::as_array) else {
        return "-".to_string();
    };
    if values.is_empty() {
        return "-".to_string();
    }
    let joined = values
        .iter()
        .map(|value| match value {
            Value::String(text) => text.clone(),
            other => other.to_string(),
        })
        .collect::<Vec<_>>()
        .join(",");
    truncate(&joined, 80)
}

fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let mut out = value.chars().take(max_chars.saturating_sub(1)).collect::<String>();
    out.push('…');
    out
}

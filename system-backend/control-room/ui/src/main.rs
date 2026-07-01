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
const UI_VERSION_LABEL: &str = "Native UI v4.7 · Directory-first · Zombie-frei";
const DEFAULT_TILE_URL: &str = "https://tile.openstreetmap.org/{z}/{x}/{y}.png";
const DEFAULT_TILE_ATTRIBUTION: &str = "© OpenStreetMap contributors";
const TILE_SIZE: f64 = 256.0;

fn main() -> eframe::Result<()> {
    let (settings, startup_warning) = ResolvedSettings::load();
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1320.0, 860.0])
            .with_min_inner_size([1080.0, 720.0]),
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
    config_path: Option<PathBuf>,
    profile: String,
    api: String,
    token: Option<String>,
    token_source: Option<String>,
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
    owner: Option<String>,
    status: Option<String>,
    status_group: Option<String>,
    #[serde(default)]
    groups: Vec<u64>,
    #[serde(default)]
    static_groups: Vec<u64>,
    hidden: Option<bool>,
    hide_in_subscribers: Option<bool>,
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
        self.subscribers.extend(other.subscribers.clone());
        self.groups.extend(other.groups.clone());
        self.status_groups.extend(other.status_groups.clone());
        self.statuses.extend(other.statuses.clone());
        self.hide_infrastructure = other.hide_infrastructure;
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
    token: Option<String>,
    token_file: Option<PathBuf>,
    default_node: Option<String>,
    operator_id: Option<String>,
}

#[derive(Debug, Default)]
struct CliArgs {
    api: Option<String>,
    token: Option<String>,
    token_file: Option<PathBuf>,
    config: Option<PathBuf>,
    profile: String,
}

impl ResolvedSettings {
    fn load() -> (Self, Option<String>) {
        let cli = parse_args();
        let (config_path, config, warning) = load_operator_config(cli.config.as_deref());
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

        let mut token_source = None;
        let token = if let Some(token) = cli.token.clone().filter(|token| !token.trim().is_empty()) {
            token_source = Some("CLI --token".to_string());
            Some(token)
        } else if let Some(path) = cli.token_file.as_ref() {
            token_source = Some(format!("CLI --token-file {}", path.display()));
            read_token_file(path).ok().flatten()
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
            read_token_file(path).ok().flatten()
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
                config_path,
                profile: cli.profile,
                api,
                token,
                token_source,
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
            "--token" => args.token = iter.next(),
            "--token-file" => args.token_file = iter.next().map(PathBuf::from),
            "--config" => args.config = iter.next().map(PathBuf::from),
            "--profile" => args.profile = iter.next().unwrap_or_else(|| DEFAULT_PROFILE.to_string()),
            "--help" | "-h" => {
                println!("NetCore Control Room Operator UI");
                println!("  --api <url>");
                println!("  --token <token>");
                println!("  --token-file <path>");
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

fn read_token_file(path: &Path) -> Result<Option<String>, std::io::Error> {
    let token = fs::read_to_string(path)?.trim().to_string();
    if token.is_empty() { Ok(None) } else { Ok(Some(token)) }
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
    Commands,
    AdminTokens,
    Raw,
}

impl Tab {
    const ALL: [Tab; 10] = [
        Tab::Overview,
        Tab::Subscribers,
        Tab::Groups,
        Tab::Calls,
        Tab::Sds,
        Tab::Locations,
        Tab::Map,
        Tab::Commands,
        Tab::AdminTokens,
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
            Tab::Commands => "Commands",
            Tab::AdminTokens => "Admin/Tokens",
            Tab::Raw => "Raw JSON",
        }
    }
}

struct ApiClient {
    base: String,
    token: Option<String>,
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
            token: settings.token.clone(),
            http,
        }
    }

    fn get(&self, path: &str) -> Result<Value, String> {
        let url = self.url(path);
        let mut request = self.http.get(&url);
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        self.read(url, request.send().map_err(|error| error.to_string())?)
    }

    fn post<T: Serialize + ?Sized>(&self, path: &str, body: &T) -> Result<Value, String> {
        let url = self.url(path);
        let mut request = self.http.post(&url).json(body);
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        self.read(url, request.send().map_err(|error| error.to_string())?)
    }

    fn patch<T: Serialize + ?Sized>(&self, path: &str, body: &T) -> Result<Value, String> {
        let url = self.url(path);
        let mut request = self.http.patch(&url).json(body);
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        self.read(url, request.send().map_err(|error| error.to_string())?)
    }

    fn delete(&self, path: &str) -> Result<Value, String> {
        let url = self.url(path);
        let mut request = self.http.delete(&url);
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        self.read(url, request.send().map_err(|error| error.to_string())?)
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
    admin_tokens: Option<Value>,

    kick_issi: String,
    dgna_issi: String,
    dgna_gssi: String,
    dgna_detach: bool,
    clear_issi: String,
    command_result: Option<String>,

    new_token_label: String,
    new_token_role: String,
    new_token_expires: String,
    new_token_created_by: String,
    token_result: Option<String>,

    detached_windows: HashMap<Tab, bool>,
    window_mode: bool,
    map_follow_latest: bool,
    map_tiles_enabled: bool,
    map_zoom_adjust: i32,
    map_wheel_zoom_accum: f32,
    map_manual_center: Option<(f64, f64)>,
    selected_location_issi: Option<u64>,
    map_cache: MapTileCache,
    local_directory: DirectorySettings,
    directory_source: String,
}


impl ControlRoomApp {
    fn new(settings: ResolvedSettings, startup_warning: Option<String>) -> Self {
        let api = ApiClient::new(&settings);
        let local_directory = settings.directory.clone();
        Self {
            kick_issi: String::new(),
            dgna_issi: String::new(),
            dgna_gssi: String::new(),
            dgna_detach: false,
            clear_issi: String::new(),
            command_result: None,
            new_token_label: String::new(),
            new_token_role: "viewer".to_string(),
            new_token_expires: String::new(),
            new_token_created_by: settings.operator_id.clone(),
            token_result: None,
            map_tiles_enabled: settings.map.online_tiles,
            map_zoom_adjust: 0,
            map_wheel_zoom_accum: 0.0,
            map_manual_center: None,
            selected_location_issi: None,
            map_cache: MapTileCache::new(settings.map.clone()),
            local_directory,
            directory_source: "operator.toml".to_string(),
            settings,
            api,
            tab: Tab::Overview,
            auto_refresh: true,
            refresh_seconds: 2.0,
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
            admin_tokens: None,
            detached_windows: HashMap::new(),
            window_mode: false,
            map_follow_latest: true,
        }
    }

    fn refresh_all(&mut self) {
        let mut errors = Vec::new();
        self.refresh_directory(&mut errors);
        self.get_into("/api/overview", DataSlot::Overview, &mut errors);
        self.get_into("/api/subscribers", DataSlot::Subscribers, &mut errors);
        self.get_into("/api/groups", DataSlot::Groups, &mut errors);
        self.get_into("/api/calls", DataSlot::Calls, &mut errors);
        self.get_into("/api/sds?limit=50", DataSlot::Sds, &mut errors);
        self.get_into("/api/locations", DataSlot::Locations, &mut errors);
        self.get_into("/api/commands?limit=50", DataSlot::Commands, &mut errors);
        self.get_into("/api/emergencies", DataSlot::Emergencies, &mut errors);

        match self.api.get("/api/admin/tokens") {
            Ok(value) => self.admin_tokens = Some(value),
            Err(error) => self.admin_tokens = Some(json!({ "error": error })),
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
        match self.api.get("/api/directory") {
            Ok(value) => match serde_json::from_value::<DirectoryConfig>(value) {
                Ok(config) => {
                    let mut directory = DirectorySettings::from_config(Some(&config));
                    // Local operator.toml deliberately wins, so a Windows operator can
                    // override labels while the LXC remains the central source.
                    directory.merge_from(&self.local_directory);
                    self.settings.directory = directory;
                    self.directory_source = "LXC /api/directory + operator.toml overrides".to_string();
                }
                Err(error) => errors.push(format!("/api/directory: Directory konnte nicht gelesen werden: {error}")),
            },
            Err(error) => {
                // Older cores do not have /api/directory. Keep the UI usable with local config.
                self.settings.directory = self.local_directory.clone();
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

    fn create_token(&mut self) {
        let label = self.new_token_label.trim();
        if label.is_empty() {
            self.token_result = Some("Label fehlt".to_string());
            return;
        }
        let expires_at = self.new_token_expires.trim();
        let body = json!({
            "label": label,
            "role": self.new_token_role.trim(),
            "expires_at": if expires_at.is_empty() { Value::Null } else { Value::String(expires_at.to_string()) },
            "created_by": self.new_token_created_by.trim(),
        });
        self.token_result = Some(match self.api.post("/api/admin/tokens", &body) {
            Ok(value) => pretty(&value),
            Err(error) => error,
        });
        self.refresh_all();
    }

    fn set_token_enabled(&mut self, id: &str, enabled: bool) {
        let body = json!({ "enabled": enabled });
        self.token_result = Some(match self.api.patch(&format!("/api/admin/tokens/{id}"), &body) {
            Ok(value) => pretty(&value),
            Err(error) => error,
        });
        self.refresh_all();
    }

    fn delete_token(&mut self, id: &str) {
        self.token_result = Some(match self.api.delete(&format!("/api/admin/tokens/{id}")) {
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
        if self.overview.is_none() {
            self.refresh_all();
        }
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

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("NetCore Control Room");
                ui.colored_label(egui::Color32::LIGHT_BLUE, UI_VERSION_LABEL);
                ui.separator();
                ui.label(format!("API: {}", self.settings.api));
                ui.label(format!("Profil: {}", self.settings.profile));
                ui.label(format!("Node: {}", self.settings.default_node));
                ui.label(format!("Operator: {}", self.settings.operator_id));
                if self.settings.token.is_some() {
                    ui.label("Token: geladen");
                } else {
                    ui.colored_label(egui::Color32::YELLOW, "Token: fehlt");
                }
                ui.label(format!("Directory: {}", self.directory_source));
                if ui.button("Refresh").clicked() {
                    self.refresh_all();
                }
                ui.checkbox(&mut self.auto_refresh, "Auto");
                ui.add(egui::Slider::new(&mut self.refresh_seconds, 1.0..=15.0).text("s"));
            });
            if let Some(path) = &self.settings.config_path {
                ui.small(format!("Config: {}", path.display()));
            }
            if let Some(source) = &self.settings.token_source {
                ui.small(format!("Token-Quelle: {source}"));
            }
            if let Some(warning) = &self.startup_warning {
                ui.colored_label(egui::Color32::YELLOW, warning);
            }
            if let Some(error) = &self.last_error {
                ui.colored_label(egui::Color32::RED, error);
            } else if let Some(ok) = &self.last_ok {
                ui.small(format!("Letzter erfolgreicher Refresh: {ok}"));
            }
        });

        egui::SidePanel::left("tabs").resizable(false).default_width(160.0).show(ctx, |ui| {
            ui.vertical_centered(|ui| ui.heading("Module"));
            ui.separator();
            ui.checkbox(&mut self.window_mode, "OS-Fenster-Modus");
            ui.small("öffnet Module als echte Betriebssystem-Fenster");
            if ui.button("Alle Module als OS-Fenster öffnen").clicked() {
                self.window_mode = true;
                for tab in Tab::ALL {
                    if tab != Tab::Raw {
                        self.detached_windows.insert(tab, true);
                    }
                }
            }
            if ui.button("Alle OS-Fenster schließen").clicked() {
                self.detached_windows.clear();
            }
            ui.separator();
            for tab in Tab::ALL {
                ui.horizontal(|ui| {
                    if ui.selectable_label(self.tab == tab, tab.label()).clicked() {
                        self.tab = tab;
                    }
                    let is_open = *self.detached_windows.get(&tab).unwrap_or(&false);
                    let button_label = if is_open { "▣" } else { "↗" };
                    if ui.small_button(button_label).on_hover_text("Modul als echtes OS-Fenster öffnen/schließen").clicked() {
                        self.detached_windows.insert(tab, !is_open);
                        self.window_mode = true;
                    }
                });
            }
            ui.separator();
            self.render_command_box(ui);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.render_module_content(ui, self.tab);
        });

        self.render_detached_windows(ctx);
    }
}

impl ControlRoomApp {
    fn render_module_content(&mut self, ui: &mut egui::Ui, tab: Tab) {
        match tab {
            Tab::Overview => self.render_overview(ui),
            Tab::Subscribers => self.render_subscribers(ui),
            Tab::Groups => self.render_groups(ui),
            Tab::Calls => self.render_calls(ui),
            Tab::Sds => self.render_sds(ui),
            Tab::Locations => self.render_locations(ui),
            Tab::Map => self.render_map(ui),
            Tab::Commands => self.render_commands(ui),
            Tab::AdminTokens => self.render_admin_tokens(ui),
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
            .collect::<Vec<_>>();

        for tab in open_tabs {
            let mut close_requested = false;
            let title = format!("{} – NetCore Control Room", tab.label());
            let default_size = match tab {
                Tab::Map => [1100.0, 760.0],
                Tab::Overview => [1180.0, 760.0],
                Tab::AdminTokens => [1050.0, 720.0],
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
        ui.heading("Befehle");
        ui.small("nutzt default_node/operator_id aus dem Profil");
        ui.separator();

        ui.label("Kick ISSI");
        ui.text_edit_singleline(&mut self.kick_issi);
        if ui.button("Kick senden").clicked() {
            self.send_kick();
        }

        ui.separator();
        ui.label("DGNA");
        ui.horizontal(|ui| {
            ui.label("ISSI");
            ui.text_edit_singleline(&mut self.dgna_issi);
        });
        ui.horizontal(|ui| {
            ui.label("GSSI");
            ui.text_edit_singleline(&mut self.dgna_gssi);
        });
        ui.checkbox(&mut self.dgna_detach, "Detach statt Attach");
        if ui.button("DGNA senden").clicked() {
            self.send_dgna();
        }

        ui.separator();
        ui.label("Emergency Clear");
        ui.text_edit_singleline(&mut self.clear_issi);
        ui.small("leer/0 = alle");
        if ui.button("Emergency löschen").clicked() {
            self.send_clear_emergency();
        }

        if let Some(result) = &self.command_result {
            ui.separator();
            ui.label("Letztes Ergebnis:");
            egui::ScrollArea::vertical().max_height(140.0).show(ui, |ui| {
                ui.monospace(result);
            });
        }
    }

    fn render_overview(&self, ui: &mut egui::Ui) {
        ui.heading("Übersicht");
        let Some(overview) = &self.overview else {
            ui.label("Noch keine Daten");
            return;
        };

        ui.horizontal_wrapped(|ui| {
            metric(ui, "Nodes", format!("{}/{}", u64_at(overview, &["nodes_connected"]).unwrap_or(0), u64_at(overview, &["node_count"]).unwrap_or(0)));
            metric(ui, "Teilnehmer", format!("{}/{}", u64_at(overview, &["subscribers_online"]).unwrap_or(0), u64_at(overview, &["subscribers_total"]).unwrap_or(0)));
            metric(ui, "Gruppen", u64_at(overview, &["groups_total"]).unwrap_or(0).to_string());
            metric(ui, "Aktive Rufe", u64_at(overview, &["active_calls_total"]).unwrap_or(0).to_string());
            metric(ui, "Notrufe", u64_at(overview, &["emergencies_active"]).unwrap_or(0).to_string());
        });

        ui.separator();
        ui.heading("Basisstationen");
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("nodes_grid").striped(true).min_col_width(70.0).show(ui, |ui| {
                header_row(ui, &["Node", "Station", "Site", "Online", "Health", "Carrier", "MCC/MNC", "LA", "CC", "Subs", "Calls", "Brew", "RF Peak", "RF RMS", "Seen"]);
                for node in array_at(overview, &["nodes"]) {
                    ui.monospace(str_at(node, &["node_id"]).unwrap_or("?"));
                    ui.label(str_at(node, &["station_name"]).unwrap_or("?"));
                    ui.label(str_at(node, &["site"]).unwrap_or("-"));
                    bool_label(ui, bool_at(node, &["connected"]).unwrap_or(false));
                    ui.label(str_at(node, &["health_overall"]).unwrap_or("?"));
                    ui.label(format!("{} / {}", display_u64(node, &["main_carrier"]), display_u64(node, &["secondary_carrier"])));
                    ui.label(format!("{} / {}", display_u64(node, &["mcc"]), display_u64(node, &["mnc"])));
                    ui.label(display_u64(node, &["location_area"]));
                    ui.label(display_u64(node, &["colour_code"]));
                    ui.label(format!("{}/{}", display_u64(node, &["subscribers_online"]), display_u64(node, &["subscribers_total"])));
                    ui.label(display_u64(node, &["active_calls_total"]));
                    tri_label(ui, node.get("brew_connected"));
                    ui.label(display_f64(node, &["rf_peak_dbfs"]));
                    ui.label(display_f64(node, &["rf_rms_dbfs"]));
                    ui.small(str_at(node, &["last_seen"]).unwrap_or("?"));
                    ui.end_row();
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
            ui.label(display_u64(row, &["source_issi"]));
            ui.label(display_u64(row, &["dest_issi"]));
            ui.label(display_u64(row, &["protocol_id"]));
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
        ui.separator();
        table(ui, "map_locations_table", &["Gerät", "ISSI", "Koordinaten", "Quelle", "Update"], rows, |ui, row| {
            let issi = u64_at(row, &["issi"]).unwrap_or(0);
            ui.label(self.device_label_for_location(row));
            ui.monospace(if issi > 0 { issi.to_string() } else { "-".to_string() });
            ui.label(format!("{}, {}", display_f64(row, &["latitude"]), display_f64(row, &["longitude"])));
            ui.label(str_at(row, &["source"]).unwrap_or("-"));
            ui.small(str_at(row, &["updated_at"]).unwrap_or("?"));
        });
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
                    if let Some(point) = nearest_marker(pos, &points, map_rect, &viewport, 16.0) {
                        response.on_hover_text(format!(
                            "ISSI {}
Lat {:.6}
Lon {:.6}
Quelle {}
Update {}
Klick = Gerätedetails",
                            point.issi,
                            point.lat,
                            point.lon,
                            point.source,
                            point.updated_at,
                        ));
                    } else {
                        let (lat, lon) = viewport.screen_to_lat_lon(pos, map_rect);
                        response.on_hover_text(format!(
                            "Lat {:.6}
Lon {:.6}
Zoom {}
Ziehen = verschieben
Mausrad = fein dosiert zoomen
Klick auf GPS-Punkt = Details",
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
        self.selected_location_issi = nearest_marker(pos, points, map_rect, viewport, 18.0).map(|point| point.issi);
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

        let selected_point = selected_issi.and_then(|issi| points.iter().find(|point| point.issi == issi));

        for point in points {
            let pos = viewport.lat_lon_to_screen(point.lat, point.lon, map_rect);
            if !map_rect.expand(14.0).contains(pos) {
                continue;
            }
            let selected = selected_issi == Some(point.issi);
            let fill = if selected { egui::Color32::from_rgb(255, 191, 0) } else { egui::Color32::from_rgb(0, 210, 80) };
            let radius = if selected { 9.0 } else { 7.0 };
            painter.circle_filled(pos, radius, fill);
            painter.circle_stroke(pos, if selected { 13.0 } else { 10.0 }, egui::Stroke::new(2.0, egui::Color32::WHITE));
            painter.text(
                pos + egui::vec2(12.0, -10.0),
                egui::Align2::LEFT_BOTTOM,
                format!("{}", point.issi),
                egui::FontId::monospace(13.0),
                egui::Color32::BLACK,
            );
            painter.text(
                pos + egui::vec2(13.0, -9.0),
                egui::Align2::LEFT_BOTTOM,
                format!("{}", point.issi),
                egui::FontId::monospace(13.0),
                egui::Color32::WHITE,
            );
        }

        if let Some(point) = selected_point {
            self.draw_selected_location_card(painter, map_rect, point);
        }

        let title = if points.is_empty() {
            "Live-Karte · keine Positionen".to_string()
        } else {
            format!("Live-Karte · {} Position(en) · Zoom {}", points.len(), viewport.zoom)
        };
        painter.rect_filled(
            egui::Rect::from_min_size(rect.left_top() + egui::vec2(16.0, 14.0), egui::vec2(310.0, 52.0)),
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
            format!("Zentrum {:.5}, {:.5}", viewport.center_lat, viewport.center_lon),
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
        self.directory_name_for_issi(issi)
            .or_else(|| first_string(row, &["display_name", "label", "name", "alias", "radio_alias"]))
            .unwrap_or_else(|| "-".to_string())
    }

    fn directory_name_for_issi(&self, issi: u64) -> Option<String> {
        let entry = self.settings.directory.subscriber(issi)?;
        first_config_text(&[entry.display_name.as_ref(), entry.name.as_ref(), entry.label.as_ref(), entry.alias.as_ref()])
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

    fn device_label_for_location(&self, row: &Value) -> String {
        let issi = u64_at(row, &["issi"]).unwrap_or(0);
        if let Some(subscriber) = self.subscriber_for_issi(issi) {
            return self.subscriber_display_name(subscriber);
        }
        self.directory_name_for_issi(issi).unwrap_or_else(|| "-".to_string())
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

    fn render_admin_tokens(&mut self, ui: &mut egui::Ui) {
        ui.heading("Admin / Tokenverwaltung");
        ui.label("Tokenwerte werden nur beim Erstellen einmal angezeigt. Die Liste enthält keine Klartext-Tokens.");
        ui.separator();

        ui.group(|ui| {
            ui.heading("Neuen Token erstellen");
            egui::Grid::new("new_token_grid").show(ui, |ui| {
                ui.label("Label");
                ui.text_edit_singleline(&mut self.new_token_label);
                ui.end_row();
                ui.label("Role");
                egui::ComboBox::from_id_source("role_combo")
                    .selected_text(&self.new_token_role)
                    .show_ui(ui, |ui| {
                        for role in ["viewer", "operator", "admin", "node"] {
                            ui.selectable_value(&mut self.new_token_role, role.to_string(), role);
                        }
                    });
                ui.end_row();
                ui.label("Expires at");
                ui.text_edit_singleline(&mut self.new_token_expires);
                ui.small("optional, ISO-Zeit");
                ui.end_row();
                ui.label("Created by");
                ui.text_edit_singleline(&mut self.new_token_created_by);
                ui.end_row();
            });
            if ui.button("Token erstellen").clicked() {
                self.create_token();
            }
            if let Some(result) = &self.token_result {
                ui.separator();
                ui.label("Ergebnis:");
                egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| ui.monospace(result));
            }
        });

        ui.separator();
        let Some(value) = &self.admin_tokens else { ui.label("Noch keine Daten"); return; };
        if let Some(error) = str_at(value, &["error"]) {
            ui.colored_label(egui::Color32::YELLOW, format!("Tokenliste nicht verfügbar: {error}"));
            ui.label("Das ist normal, wenn du nur operator statt admin bist.");
            return;
        }

        let mut action: Option<TokenAction> = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("tokens_grid").striped(true).show(ui, |ui| {
                header_row(ui, &["ID", "Label", "Role", "Enabled", "Created", "Last used", "Aktion"]);
                for token in array_at(value, &["tokens"]) {
                    let id = str_at(token, &["id"]).unwrap_or("?").to_string();
                    let enabled = bool_at(token, &["enabled"]).unwrap_or(false);
                    ui.monospace(&id);
                    ui.label(str_at(token, &["label"]).unwrap_or("?"));
                    ui.label(str_at(token, &["role"]).unwrap_or("?"));
                    bool_label(ui, enabled);
                    ui.small(str_at(token, &["created_at"]).unwrap_or("?"));
                    ui.small(str_at(token, &["last_used_at"]).unwrap_or("-"));
                    ui.horizontal(|ui| {
                        if enabled {
                            if ui.button("Disable").clicked() {
                                action = Some(TokenAction::SetEnabled(id.clone(), false));
                            }
                        } else if ui.button("Enable").clicked() {
                            action = Some(TokenAction::SetEnabled(id.clone(), true));
                        }
                        if ui.button("Delete").clicked() {
                            action = Some(TokenAction::Delete(id.clone()));
                        }
                    });
                    ui.end_row();
                }
            });
        });
        if let Some(action) = action {
            match action {
                TokenAction::SetEnabled(id, enabled) => self.set_token_enabled(&id, enabled),
                TokenAction::Delete(id) => self.delete_token(&id),
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
            raw_block(ui, "admin_tokens", &self.admin_tokens);
        });
    }
}

#[derive(Debug, Clone)]
struct LocationPoint {
    issi: u64,
    lat: f64,
    lon: f64,
    source: String,
    updated_at: String,
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
                issi: u64_at(row, &["issi"]).unwrap_or(0),
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


fn nearest_marker<'a>(pos: egui::Pos2, points: &'a [LocationPoint], map_rect: egui::Rect, viewport: &MapViewport, radius: f32) -> Option<&'a LocationPoint> {
    let max_distance_sq = radius * radius;
    points
        .iter()
        .filter_map(|point| {
            let marker_pos = viewport.lat_lon_to_screen(point.lat, point.lon, map_rect);
            let distance_sq = (marker_pos - pos).length_sq();
            if distance_sq <= max_distance_sq {
                Some((distance_sq, point))
            } else {
                None
            }
        })
        .min_by(|left, right| left.0.partial_cmp(&right.0).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(_, point)| point)
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

fn first_string(value: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(text) = str_at(value, &[*key]).map(str::trim).filter(|text| !text.is_empty()) {
            return Some(text.to_string());
        }
        if let Some(number) = u64_at(value, &[*key]) {
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

enum TokenAction {
    SetEnabled(String, bool),
    Delete(String),
}

fn metric(ui: &mut egui::Ui, label: &str, value: String) {
    ui.group(|ui| {
        ui.heading(value);
        ui.label(label);
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
    egui::ScrollArea::both().show(ui, |ui| {
        egui::Grid::new(id).striped(true).min_col_width(70.0).show(ui, |ui| {
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

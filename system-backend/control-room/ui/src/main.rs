use eframe::egui;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const DEFAULT_API: &str = "http://127.0.0.1:9010";
const DEFAULT_PROFILE: &str = "default";
const DEFAULT_NODE: &str = "SRV-M_TBS-01";
const DEFAULT_OPERATOR: &str = "jan";

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
}

#[derive(Debug, Default, Deserialize)]
struct OperatorConfig {
    profiles: HashMap<String, ProfileConfig>,
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

        (
            Self {
                config_path,
                profile: cli.profile,
                api,
                token,
                token_source,
                default_node,
                operator_id,
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

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum Tab {
    Overview,
    Subscribers,
    Groups,
    Calls,
    Sds,
    Locations,
    Commands,
    AdminTokens,
    Raw,
}

impl Tab {
    const ALL: [Tab; 9] = [
        Tab::Overview,
        Tab::Subscribers,
        Tab::Groups,
        Tab::Calls,
        Tab::Sds,
        Tab::Locations,
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
}

impl ControlRoomApp {
    fn new(settings: ResolvedSettings, startup_warning: Option<String>) -> Self {
        let api = ApiClient::new(&settings);
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
        }
    }

    fn refresh_all(&mut self) {
        let mut errors = Vec::new();
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
            for tab in Tab::ALL {
                if ui.selectable_label(self.tab == tab, tab.label()).clicked() {
                    self.tab = tab;
                }
            }
            ui.separator();
            self.render_command_box(ui);
        });

        egui::CentralPanel::default().show(ctx, |ui| match self.tab {
            Tab::Overview => self.render_overview(ui),
            Tab::Subscribers => self.render_subscribers(ui),
            Tab::Groups => self.render_groups(ui),
            Tab::Calls => self.render_calls(ui),
            Tab::Sds => self.render_sds(ui),
            Tab::Locations => self.render_locations(ui),
            Tab::Commands => self.render_commands(ui),
            Tab::AdminTokens => self.render_admin_tokens(ui),
            Tab::Raw => self.render_raw(ui),
        });
    }
}

impl ControlRoomApp {
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
        let Some(value) = &self.subscribers else { ui.label("Noch keine Daten"); return; };
        table(ui, "subscribers_table", &["ISSI", "Online", "RSSI", "Groups", "ESM", "Active Call", "Last Seen"], array_at(value, &["subscribers"]), |ui, row| {
            ui.monospace(display_u64(row, &["issi"]));
            bool_label(ui, bool_at(row, &["online"]).unwrap_or(false));
            ui.label(display_f64(row, &["rssi_dbfs"]));
            ui.label(join_array(row, &["groups"]));
            ui.label(display_u64(row, &["energy_saving_mode"]));
            ui.label(join_array(row, &["active_call_keys"]));
            ui.small(str_at(row, &["last_seen"]).unwrap_or("?"));
        });
    }

    fn render_groups(&self, ui: &mut egui::Ui) {
        ui.heading("Gruppen");
        let Some(value) = &self.groups else { ui.label("Noch keine Daten"); return; };
        table(ui, "groups_table", &["GSSI", "Members online", "Members", "Active Call", "Last Update"], array_at(value, &["groups"]), |ui, row| {
            ui.monospace(display_u64(row, &["gssi"]));
            ui.label(display_u64(row, &["members_online"]));
            ui.label(join_array(row, &["members"]));
            ui.label(str_at(row, &["active_call_key"]).unwrap_or("-"));
            ui.small(str_at(row, &["updated_at"]).unwrap_or("-"));
        });
    }

    fn render_calls(&self, ui: &mut egui::Ui) {
        ui.heading("Aktive Rufe");
        let Some(value) = &self.calls else { ui.label("Noch keine Daten"); return; };
        table(ui, "calls_table", &["Key", "GSSI", "Call ID", "Caller", "Speaker", "Carrier", "TS", "Started"], array_at(value, &["calls"]), |ui, row| {
            ui.monospace(str_at(row, &["key"]).unwrap_or("?"));
            ui.label(display_u64(row, &["gssi"]));
            ui.label(display_u64(row, &["call_id"]));
            ui.label(display_u64(row, &["caller_issi"]));
            ui.label(display_u64(row, &["speaker_issi"]));
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

    fn render_locations(&self, ui: &mut egui::Ui) {
        ui.heading("Standorte");
        let Some(value) = &self.locations else { ui.label("Noch keine Daten"); return; };
        table(ui, "locations_table", &["ISSI", "Latitude", "Longitude", "Source", "Updated"], array_at(value, &["locations"]), |ui, row| {
            ui.monospace(display_u64(row, &["issi"]));
            ui.label(display_f64(row, &["latitude"]));
            ui.label(display_f64(row, &["longitude"]));
            ui.label(str_at(row, &["source"]).unwrap_or("-"));
            ui.small(str_at(row, &["updated_at"]).unwrap_or("?"));
        });
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

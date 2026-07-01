use clap::{Parser, Subcommand};
use serde::Serialize;
use serde_json::{json, Value};
use std::env;
use std::error::Error;
use std::thread;
use std::time::Duration;

const DEFAULT_API: &str = "http://127.0.0.1:9010";
const DEFAULT_NODE: &str = "tbs-04010001";
const DEFAULT_OPERATOR: &str = "operator";

type AppResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Debug, Parser)]
#[command(name = "netcore-control-room-operator")]
#[command(about = "Native NetCore Control Room operator console", long_about = None)]
struct Cli {
    /// Control Room Core base URL, for example http://10.10.40.20:9010.
    /// Can also be set with NETCORE_CONTROL_ROOM_API.
    #[arg(long, default_value = DEFAULT_API)]
    api: String,

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
        #[arg(long, default_value = DEFAULT_NODE)]
        node: String,
        #[arg(long)]
        issi: u32,
        #[arg(long, default_value = DEFAULT_OPERATOR)]
        operator: String,
    },

    /// Attach or detach a subscriber to/from a group by DGNA.
    Dgna {
        #[arg(long, default_value = DEFAULT_NODE)]
        node: String,
        #[arg(long)]
        issi: u32,
        #[arg(long)]
        gssi: u32,
        /// Detach instead of attach.
        #[arg(long)]
        detach: bool,
        #[arg(long, default_value = DEFAULT_OPERATOR)]
        operator: String,
    },

    /// Clear emergency state. Omit ISSI or pass 0 to clear all.
    ClearEmergency {
        #[arg(long, default_value = DEFAULT_NODE)]
        node: String,
        #[arg(long, default_value_t = 0)]
        issi: u32,
        #[arg(long, default_value = DEFAULT_OPERATOR)]
        operator: String,
    },

    /// Check /health.
    Health,
}

struct ApiClient {
    base: String,
    http: reqwest::blocking::Client,
}

impl ApiClient {
    fn new(base: &str) -> Self {
        Self {
            base: base.trim_end_matches('/').to_string(),
            http: reqwest::blocking::Client::new(),
        }
    }

    fn get_json(&self, path: &str) -> AppResult<Value> {
        let url = self.url(path);
        let response = self.http.get(url).send()?.error_for_status()?;
        Ok(response.json()?)
    }

    fn post_json<T: Serialize + ?Sized>(&self, path: &str, body: &T) -> AppResult<Value> {
        let url = self.url(path);
        let response = self.http.post(url).json(body).send()?.error_for_status()?;
        Ok(response.json()?)
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

    if cli.api == DEFAULT_API {
        if let Ok(api) = env::var("NETCORE_CONTROL_ROOM_API") {
            if !api.trim().is_empty() {
                cli.api = api;
            }
        }
    }

    let api = ApiClient::new(&cli.api);

    match cli.command.unwrap_or(Command::Dashboard { refresh: 2 }) {
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
            let body = json!({ "operator_id": operator, "issi": issi });
            print_json(api.post_json(&format!("/api/nodes/{node}/commands/kick"), &body)?)
        }
        Command::Dgna { node, issi, gssi, detach, operator } => {
            let body = json!({ "operator_id": operator, "issi": issi, "gssi": gssi, "attach": !detach });
            print_json(api.post_json(&format!("/api/nodes/{node}/commands/dgna"), &body)?)
        }
        Command::ClearEmergency { node, issi, operator } => {
            let body = json!({ "operator_id": operator, "issi": issi });
            print_json(api.post_json(&format!("/api/nodes/{node}/commands/clear-emergency"), &body)?)
        }
        Command::Health => print_json(api.get_json("/health")?),
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
    println!("Ctrl+C beendet die Operator-Konsole. Commands aktuell per CLI: kick, dgna, clear-emergency.");
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
    for cmd in commands.as_array().into_iter().flatten().take(5) {
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

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const OPEN_LAB_MODE: &str = "open_lab";
pub const SHADOW_MODE: &str = "shadow";
pub const AUTHORITATIVE_MODE: &str = "authoritative";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ApplicationGatewayConfig {
    pub server: ServerConfig,
    pub storage: StorageConfig,
    pub security: SecurityConfig,
    pub runtime: RuntimeConfig,
    pub connectors: Vec<ConnectorSeed>,
    pub rules: Vec<RouteRuleSeed>,
    pub templates: Vec<TemplateSeed>,
}

impl Default for ApplicationGatewayConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            storage: StorageConfig::default(),
            security: SecurityConfig::default(),
            runtime: RuntimeConfig::default(),
            connectors: default_connectors(),
            rules: default_rules(),
            templates: default_templates(),
        }
    }
}

impl ApplicationGatewayConfig {
    pub fn load(path: Option<&Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let mut config = match path {
            Some(path) => toml::from_str::<Self>(&fs::read_to_string(path)?)?,
            None => Self::default(),
        };
        config
            .normalise()
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;
        Ok(config)
    }

    pub fn apply_bind_override(&mut self, bind: Option<SocketAddr>) -> Result<(), String> {
        if let Some(bind) = bind {
            self.server.bind = bind;
        }
        self.normalise()
    }

    fn normalise(&mut self) -> Result<(), String> {
        if self.security.mode != OPEN_LAB_MODE {
            return Err(format!(
                "unsupported security.mode={}; this package intentionally implements open_lab management only",
                self.security.mode
            ));
        }
        if self.security.management_token_auth || self.security.management_tls {
            return Err("management_token_auth and management_tls must remain false in open_lab".into());
        }
        if !self.security.allow_remote_management && !self.server.bind.ip().is_loopback() {
            return Err("server.bind must be loopback when allow_remote_management=false".into());
        }
        if !matches!(self.runtime.operating_mode.as_str(), SHADOW_MODE | AUTHORITATIVE_MODE) {
            return Err("runtime.operating_mode must be shadow or authoritative".into());
        }
        if !self.server.public_base_url.starts_with("http://") {
            return Err("server.public_base_url must use http:// in the current open-lab package".into());
        }
        self.server.public_base_url = self.server.public_base_url.trim_end_matches('/').to_string();
        self.server.max_body_bytes = self.server.max_body_bytes.max(65_536);
        self.server.history_limit = self.server.history_limit.max(100);
        self.runtime.worker_interval_ms = self.runtime.worker_interval_ms.max(100);
        self.runtime.probe_interval_secs = self.runtime.probe_interval_secs.max(5);
        self.runtime.default_ttl_secs = self.runtime.default_ttl_secs.max(30);
        self.runtime.max_attempts = self.runtime.max_attempts.max(1);
        self.runtime.base_backoff_secs = self.runtime.base_backoff_secs.max(1);
        self.runtime.max_backoff_secs = self.runtime.max_backoff_secs.max(self.runtime.base_backoff_secs);
        self.runtime.max_response_bytes = self.runtime.max_response_bytes.max(4_096);
        self.runtime.max_artifact_bytes = self.runtime.max_artifact_bytes.max(65_536);
        self.runtime.max_events = self.runtime.max_events.max(100);
        self.runtime.max_deliveries = self.runtime.max_deliveries.max(100);
        self.runtime.max_audit_records = self.runtime.max_audit_records.max(100);
        self.runtime.max_tts_jobs = self.runtime.max_tts_jobs.max(20);
        self.runtime.event_retention_secs = self.runtime.event_retention_secs.max(300);
        self.runtime.delivery_retention_secs = self.runtime.delivery_retention_secs.max(300);
        self.runtime.audit_retention_secs = self.runtime.audit_retention_secs.max(300);
        self.runtime.dedupe_window_secs = self.runtime.dedupe_window_secs.max(10);

        let mut connector_ids = HashSet::new();
        for connector in &mut self.connectors {
            connector.connector_id = slug(&connector.connector_id);
            connector.kind = connector.kind.trim().to_ascii_lowercase();
            connector.direction = connector.direction.trim().to_ascii_lowercase();
            connector.endpoint = connector.endpoint.trim().trim_end_matches('/').to_string();
            connector.health_endpoint = connector
                .health_endpoint
                .as_ref()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
            connector.timeout_ms = connector.timeout_ms.max(100);
            connector.rate_limit_per_minute = connector.rate_limit_per_minute.max(1);
            connector.circuit_failure_threshold = connector.circuit_failure_threshold.max(1);
            connector.circuit_open_secs = connector.circuit_open_secs.max(5);
            connector.required_secrets = connector
                .required_secrets
                .iter()
                .map(|value| slug(value))
                .filter(|value| !value.is_empty())
                .collect();
            if connector.connector_id.is_empty() || !connector_ids.insert(connector.connector_id.clone()) {
                return Err(format!("connector_id must be non-empty and unique: {}", connector.connector_id));
            }
            if !matches!(connector.direction.as_str(), "inbound" | "outbound" | "bidirectional") {
                return Err(format!("connector {} has invalid direction", connector.connector_id));
            }
            if connector.endpoint.is_empty() && connector.direction != "inbound" {
                return Err(format!("connector {} requires an endpoint", connector.connector_id));
            }
            if !connector.endpoint.is_empty()
                && !(connector.endpoint.starts_with("http://") || connector.endpoint.starts_with("https://"))
            {
                return Err(format!("connector {} endpoint must use http:// or https://", connector.connector_id));
            }
        }

        let mut rule_ids = HashSet::new();
        for rule in &mut self.rules {
            rule.rule_id = slug(&rule.rule_id);
            rule.source_connector = rule.source_connector.trim().to_ascii_lowercase();
            rule.event_type = rule.event_type.trim().to_ascii_lowercase();
            rule.target_connector = slug(&rule.target_connector);
            if rule.rule_id.is_empty() || !rule_ids.insert(rule.rule_id.clone()) {
                return Err(format!("rule_id must be non-empty and unique: {}", rule.rule_id));
            }
            if !connector_ids.contains(&rule.target_connector) {
                return Err(format!("rule {} references unknown connector {}", rule.rule_id, rule.target_connector));
            }
        }

        let mut template_ids = HashSet::new();
        for template in &mut self.templates {
            template.template_id = slug(&template.template_id);
            template.kind = template.kind.trim().to_ascii_lowercase();
            template.target_connector = template.target_connector.as_ref().map(|value| slug(value));
            if template.template_id.is_empty() || !template_ids.insert(template.template_id.clone()) {
                return Err(format!("template_id must be non-empty and unique: {}", template.template_id));
            }
            if !matches!(template.kind.as_str(), "text" | "json" | "tts") {
                return Err(format!("template {} has unsupported kind {}", template.template_id, template.kind));
            }
            if let Some(connector) = &template.target_connector
                && !connector_ids.contains(connector)
            {
                return Err(format!("template {} references unknown connector {}", template.template_id, connector));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub bind: SocketAddr,
    pub public_base_url: String,
    pub max_body_bytes: usize,
    pub history_limit: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: "0.0.0.0:8220".parse().expect("valid default bind"),
            public_base_url: "http://127.0.0.1:8220".to_string(),
            max_body_bytes: 4 * 1024 * 1024,
            history_limit: 5_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StorageConfig {
    pub state_path: PathBuf,
    pub state_backup_path: PathBuf,
    pub secrets_path: PathBuf,
    pub spool_dir: PathBuf,
    pub backup_dir: PathBuf,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            state_path: "/var/lib/netcore-application-gateway/state.json".into(),
            state_backup_path: "/var/lib/netcore-application-gateway/state.json.bak".into(),
            secrets_path: "/var/lib/netcore-application-gateway/secrets.json".into(),
            spool_dir: "/var/lib/netcore-application-gateway/spool".into(),
            backup_dir: "/var/lib/netcore-application-gateway/backups".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SecurityConfig {
    pub mode: String,
    pub management_token_auth: bool,
    pub management_tls: bool,
    pub allow_remote_management: bool,
    pub connector_secrets_allowed: bool,
    pub warning_banner: String,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            mode: OPEN_LAB_MODE.to_string(),
            management_token_auth: false,
            management_tls: false,
            allow_remote_management: true,
            connector_secrets_allowed: true,
            warning_banner: "OPEN LAB: no login, no management tokens and no TLS. Isolated management network only.".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RuntimeConfig {
    pub operating_mode: String,
    pub worker_interval_ms: u64,
    pub probe_interval_secs: u64,
    pub default_ttl_secs: u64,
    pub max_attempts: u32,
    pub base_backoff_secs: u64,
    pub max_backoff_secs: u64,
    pub dedupe_window_secs: u64,
    pub max_response_bytes: usize,
    pub max_artifact_bytes: usize,
    pub max_events: usize,
    pub max_deliveries: usize,
    pub max_tts_jobs: usize,
    pub max_audit_records: usize,
    pub event_retention_secs: u64,
    pub delivery_retention_secs: u64,
    pub audit_retention_secs: u64,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            operating_mode: SHADOW_MODE.to_string(),
            worker_interval_ms: 1_000,
            probe_interval_secs: 30,
            default_ttl_secs: 300,
            max_attempts: 6,
            base_backoff_secs: 2,
            max_backoff_secs: 120,
            dedupe_window_secs: 600,
            max_response_bytes: 64 * 1024,
            max_artifact_bytes: 32 * 1024 * 1024,
            max_events: 20_000,
            max_deliveries: 50_000,
            max_tts_jobs: 5_000,
            max_audit_records: 50_000,
            event_retention_secs: 7 * 24 * 3600,
            delivery_retention_secs: 14 * 24 * 3600,
            audit_retention_secs: 30 * 24 * 3600,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ConnectorSeed {
    pub connector_id: String,
    pub display_name: String,
    pub kind: String,
    pub direction: String,
    pub endpoint: String,
    pub health_endpoint: Option<String>,
    pub enabled: bool,
    pub timeout_ms: u64,
    pub rate_limit_per_minute: u32,
    pub circuit_failure_threshold: u32,
    pub circuit_open_secs: u64,
    pub required_secrets: Vec<String>,
    pub settings: BTreeMap<String, String>,
}

impl Default for ConnectorSeed {
    fn default() -> Self {
        Self {
            connector_id: "webhook".to_string(),
            display_name: "Webhook".to_string(),
            kind: "generic_webhook".to_string(),
            direction: "outbound".to_string(),
            endpoint: "http://127.0.0.1:9000/webhook".to_string(),
            health_endpoint: None,
            enabled: false,
            timeout_ms: 5_000,
            rate_limit_per_minute: 60,
            circuit_failure_threshold: 5,
            circuit_open_secs: 60,
            required_secrets: Vec::new(),
            settings: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RouteRuleSeed {
    pub rule_id: String,
    pub name: String,
    pub enabled: bool,
    pub priority: i32,
    pub source_connector: String,
    pub event_type: String,
    pub text_contains: Option<String>,
    pub target_connector: String,
    pub template_id: Option<String>,
    pub destination: Option<String>,
    pub stop_processing: bool,
}

impl Default for RouteRuleSeed {
    fn default() -> Self {
        Self {
            rule_id: "manual-to-sds".to_string(),
            name: "Manual messages to SDS Router".to_string(),
            enabled: true,
            priority: 100,
            source_connector: "manual".to_string(),
            event_type: "sds.message".to_string(),
            text_contains: None,
            target_connector: "sds-router".to_string(),
            template_id: Some("sds-standard".to_string()),
            destination: None,
            stop_processing: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TemplateSeed {
    pub template_id: String,
    pub name: String,
    pub kind: String,
    pub body: String,
    pub content_type: String,
    pub enabled: bool,
    pub target_connector: Option<String>,
    pub default_destination: Option<String>,
    pub description: String,
}

impl Default for TemplateSeed {
    fn default() -> Self {
        Self {
            template_id: "plain-text".to_string(),
            name: "Plain text".to_string(),
            kind: "text".to_string(),
            body: "{{text}}".to_string(),
            content_type: "text/plain; charset=utf-8".to_string(),
            enabled: true,
            target_connector: None,
            default_destination: None,
            description: String::new(),
        }
    }
}

fn connector(id: &str, name: &str, kind: &str, direction: &str, endpoint: &str, enabled: bool) -> ConnectorSeed {
    ConnectorSeed {
        connector_id: id.to_string(),
        display_name: name.to_string(),
        kind: kind.to_string(),
        direction: direction.to_string(),
        endpoint: endpoint.to_string(),
        enabled,
        ..ConnectorSeed::default()
    }
}

fn default_connectors() -> Vec<ConnectorSeed> {
    let mut sds = connector(
        "sds-router",
        "SDS Router",
        "sds_router",
        "outbound",
        "http://127.0.0.1:8150/api/v1/messages",
        true,
    );
    sds.health_endpoint = Some("http://127.0.0.1:8150/health/ready".to_string());
    sds.rate_limit_per_minute = 600;
    sds.settings.insert("source_issi".to_string(), "9999".to_string());
    sds.settings.insert("sds_type".to_string(), "4".to_string());
    sds.settings.insert("protocol_id".to_string(), "0".to_string());
    sds.settings.insert("priority".to_string(), "3".to_string());

    let mut piper = connector(
        "piper-tts",
        "Piper TTS",
        "piper_tts",
        "outbound",
        "http://127.0.0.1:5005/synthesize",
        true,
    );
    piper.health_endpoint = Some("http://127.0.0.1:5005/voices".to_string());
    piper.timeout_ms = 30_000;
    piper.rate_limit_per_minute = 30;
    piper.circuit_failure_threshold = 3;

    let mut media = connector(
        "media-library",
        "Media Library",
        "media_library",
        "outbound",
        "http://127.0.0.1:8230/api/v1/assets/import-url",
        false,
    );
    media.health_endpoint = Some("http://127.0.0.1:8230/health/ready".to_string());

    let mut telegram = connector(
        "telegram",
        "Telegram Bot",
        "telegram_bot",
        "bidirectional",
        "https://api.telegram.org/bot{bot_token}/sendMessage",
        false,
    );
    telegram.required_secrets = vec!["bot_token".to_string()];
    telegram.settings.insert("chat_id".to_string(), String::new());

    let mut dapnet = connector(
        "dapnet",
        "DAPNET Relay",
        "dapnet_http",
        "bidirectional",
        "http://127.0.0.1:8225/api/v1/messages",
        false,
    );
    dapnet.required_secrets = vec!["auth_key".to_string()];
    dapnet.settings.insert("callsign".to_string(), String::new());

    let mut meshcom = connector(
        "meshcom",
        "MeshCom",
        "meshcom_http",
        "bidirectional",
        "http://127.0.0.1:1799/api/message",
        false,
    );
    meshcom.required_secrets = vec!["api_key".to_string()];

    let snom = connector(
        "snom",
        "Snom Notify",
        "snom_notify",
        "outbound",
        "http://127.0.0.1:8089/notify",
        false,
    );

    let mut geoalarm = connector(
        "geoalarm",
        "GeoAlarm",
        "geoalarm_http",
        "bidirectional",
        "http://127.0.0.1:8099/api/alarms",
        false,
    );
    geoalarm.required_secrets = vec!["api_key".to_string()];

    let weather = connector(
        "weather",
        "WX / METAR",
        "weather_http",
        "bidirectional",
        "https://aviationweather.gov/api/data/metar",
        false,
    );

    let mut tpg = connector(
        "tpg2200",
        "TPG2200 Bridge",
        "tpg2200_bridge",
        "outbound",
        "http://127.0.0.1:8150/api/v1/messages",
        false,
    );
    tpg.settings.insert("source_issi".to_string(), "9999".to_string());
    tpg.settings.insert("protocol_id".to_string(), "130".to_string());
    tpg.settings.insert("priority".to_string(), "3".to_string());

    let directory = connector(
        "directory",
        "Status Directory",
        "directory_http",
        "bidirectional",
        "http://127.0.0.1:8060/api/v1/events",
        false,
    );

    let generic = connector(
        "generic-webhook",
        "Generic Webhook",
        "generic_webhook",
        "bidirectional",
        "http://127.0.0.1:9000/webhook",
        false,
    );

    vec![sds, piper, media, telegram, dapnet, meshcom, snom, geoalarm, weather, tpg, directory, generic]
}

fn default_rules() -> Vec<RouteRuleSeed> {
    vec![
        RouteRuleSeed::default(),
        RouteRuleSeed {
            rule_id: "manual-webhook".to_string(),
            name: "Manual webhook dispatch".to_string(),
            enabled: false,
            priority: 50,
            source_connector: "manual".to_string(),
            event_type: "webhook.message".to_string(),
            target_connector: "generic-webhook".to_string(),
            template_id: Some("generic-event-json".to_string()),
            ..RouteRuleSeed::default()
        },
    ]
}

fn default_templates() -> Vec<TemplateSeed> {
    vec![
        TemplateSeed {
            template_id: "sds-standard".to_string(),
            name: "SDS Standard".to_string(),
            kind: "text".to_string(),
            body: "{{text}}".to_string(),
            content_type: "text/plain; charset=utf-8".to_string(),
            enabled: true,
            target_connector: Some("sds-router".to_string()),
            default_destination: None,
            description: "Plain SDS text; SDS addressing is supplied by the event destination and connector settings".to_string(),
        },
        TemplateSeed {
            template_id: "generic-event-json".to_string(),
            name: "Generic event JSON".to_string(),
            kind: "json".to_string(),
            body: r#"{"source":"{{source}}","event_type":"{{event_type}}","destination":"{{destination}}","text":"{{text}}"}"#.to_string(),
            content_type: "application/json".to_string(),
            enabled: true,
            target_connector: Some("generic-webhook".to_string()),
            default_destination: None,
            description: "Small interoperable webhook envelope".to_string(),
        },
        TemplateSeed {
            template_id: "tts-announcement".to_string(),
            name: "TTS Announcement".to_string(),
            kind: "tts".to_string(),
            body: "Achtung. {{text}}".to_string(),
            content_type: "text/plain; charset=utf-8".to_string(),
            enabled: true,
            target_connector: Some("piper-tts".to_string()),
            default_destination: None,
            description: "Reusable TTS announcement prefix".to_string(),
        },
    ]
}

pub fn slug(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|character| if character.is_ascii_alphanumeric() || character == '-' || character == '_' { character } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

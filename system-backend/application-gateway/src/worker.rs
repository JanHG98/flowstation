use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use reqwest::blocking::{Client, RequestBuilder, Response};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, USER_AGENT};
use reqwest::redirect::Policy;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::config::ApplicationGatewayConfig;
use crate::model::{
    ClaimedDelivery, ConnectorProbeOutcome, ConnectorRecord, DeliveryOutcome, DeliveryRecord,
};
use crate::state::SharedGateway;

pub fn spawn_worker(
    config: ApplicationGatewayConfig,
    gateway: SharedGateway,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .redirect(Policy::none())
            .user_agent("NetCore-Tetra Application-Gateway/1")
            .build();
        let Ok(client) = client else {
            tracing::error!("Application Gateway cannot construct HTTP client");
            return;
        };
        loop {
            run_cycle(&config, &gateway, &client, 32);
            thread::sleep(Duration::from_millis(config.runtime.worker_interval_ms));
        }
    })
}

pub fn run_cycle(
    config: &ApplicationGatewayConfig,
    gateway: &SharedGateway,
    client: &Client,
    max_deliveries: usize,
) {
    for connector in gateway.connectors_due_probe() {
        let outcome = probe_connector(client, &connector);
        if let Err(error) = gateway.record_probe(outcome) {
            tracing::warn!("Application Gateway probe result could not be persisted: {}", error);
        }
    }

    for _ in 0..max_deliveries {
        let claimed = match gateway.claim_due_delivery() {
            Ok(Some(claimed)) => claimed,
            Ok(None) => break,
            Err(error) => {
                tracing::warn!("Application Gateway cannot claim delivery: {}", error);
                break;
            }
        };
        let delivery_id = claimed.delivery.delivery_id.clone();
        let outcome = send_delivery(config, client, claimed);
        if let Err(error) = gateway.finish_delivery(&delivery_id, outcome) {
            tracing::warn!("Application Gateway delivery result could not be persisted: {}", error);
        }
    }
}

pub fn test_connector(client: &Client, connector: &ConnectorRecord) -> ConnectorProbeOutcome {
    probe_connector(client, connector)
}

pub fn build_client() -> Result<Client, String> {
    Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .redirect(Policy::none())
        .user_agent("NetCore-Tetra Application-Gateway/1")
        .build()
        .map_err(|error| error.to_string())
}

fn probe_connector(client: &Client, connector: &ConnectorRecord) -> ConnectorProbeOutcome {
    let started = Instant::now();
    let Some(endpoint) = connector.health_endpoint.as_ref() else {
        return ConnectorProbeOutcome {
            connector_id: connector.connector_id.clone(),
            success: true,
            status: None,
            response_ms: 0.0,
            error: None,
        };
    };
    let response = client
        .get(endpoint)
        .timeout(Duration::from_millis(connector.timeout_ms))
        .header(USER_AGENT, "NetCore-Tetra Application-Gateway-Probe/1")
        .send();
    match response {
        Ok(response) => ConnectorProbeOutcome {
            connector_id: connector.connector_id.clone(),
            success: response.status().is_success(),
            status: Some(response.status().as_u16()),
            response_ms: started.elapsed().as_secs_f64() * 1_000.0,
            error: if response.status().is_success() {
                None
            } else {
                Some(format!("health endpoint returned HTTP {}", response.status()))
            },
        },
        Err(error) => ConnectorProbeOutcome {
            connector_id: connector.connector_id.clone(),
            success: false,
            status: None,
            response_ms: started.elapsed().as_secs_f64() * 1_000.0,
            error: Some(error.to_string()),
        },
    }
}

fn send_delivery(
    config: &ApplicationGatewayConfig,
    client: &Client,
    claimed: ClaimedDelivery,
) -> DeliveryOutcome {
    let result = match claimed.connector.kind.as_str() {
        "piper_tts" => send_piper(config, client, &claimed),
        "telegram_bot" => send_telegram(config, client, &claimed),
        "sds_router" => send_sds(config, client, &claimed, false),
        "tpg2200_bridge" => send_sds(config, client, &claimed, true),
        "snom_notify" => send_snom(config, client, &claimed),
        "weather_http" => send_weather(config, client, &claimed),
        "media_library" | "directory_http" | "dapnet_http" | "meshcom_http"
        | "geoalarm_http" | "generic_webhook" => send_generic(config, client, &claimed),
        other => Err(format!("unsupported connector kind {other}")),
    };
    match result {
        Ok(outcome) => outcome,
        Err(error) => DeliveryOutcome {
            success: false,
            status: None,
            response_excerpt: None,
            error: Some(error),
            artifact_path: None,
            artifact_sha256: None,
            artifact_size_bytes: None,
        },
    }
}

fn send_generic(
    config: &ApplicationGatewayConfig,
    client: &Client,
    claimed: &ClaimedDelivery,
) -> Result<DeliveryOutcome, String> {
    let endpoint = endpoint_with_secrets(&claimed.connector.endpoint, &claimed.secrets);
    let method = claimed
        .connector
        .settings
        .get("method")
        .map(|value| value.to_ascii_uppercase())
        .unwrap_or_else(|| "POST".to_string());
    let envelope = json!({
        "schema": "netcore-application-event-v1",
        "delivery_id": claimed.delivery.delivery_id,
        "event_id": claimed.delivery.event_id,
        "event_type": claimed.delivery.event_type,
        "destination": claimed.delivery.destination,
        "text": claimed.delivery.text,
        "payload": claimed.delivery.payload,
        "correlation_id": claimed.delivery.correlation_id,
        "priority": claimed.delivery.priority,
        "connector_kind": claimed.connector.kind,
    });
    let request = match method.as_str() {
        "PUT" => client.put(&endpoint),
        "PATCH" => client.patch(&endpoint),
        "GET" => client.get(&endpoint),
        "DELETE" => client.delete(&endpoint),
        _ => client.post(&endpoint),
    };
    let request = apply_auth(request, &claimed.connector, &claimed.secrets)
        .timeout(Duration::from_millis(claimed.connector.timeout_ms));
    let response = if method == "GET" || method == "DELETE" {
        request.send()
    } else {
        request.json(&envelope).send()
    }
    .map_err(|error| format!("{} request failed: {error}", claimed.connector.display_name))?;
    response_outcome(config, response)
}

fn send_telegram(
    config: &ApplicationGatewayConfig,
    client: &Client,
    claimed: &ClaimedDelivery,
) -> Result<DeliveryOutcome, String> {
    let token = claimed
        .secrets
        .get("bot_token")
        .ok_or_else(|| "Telegram bot_token secret is missing".to_string())?;
    let endpoint = claimed.connector.endpoint.replace("{bot_token}", token);
    let chat_id = claimed
        .delivery
        .destination
        .as_deref()
        .filter(|value| !value.is_empty())
        .or_else(|| claimed.connector.settings.get("chat_id").map(String::as_str))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Telegram chat_id is missing in destination or connector settings".to_string())?;
    let text = delivery_text(&claimed.delivery);
    let mut payload = json!({"chat_id": chat_id, "text": text});
    if let Some(mode) = claimed.connector.settings.get("parse_mode")
        && !mode.is_empty()
    {
        payload["parse_mode"] = json!(mode);
    }
    let response = client
        .post(endpoint)
        .timeout(Duration::from_millis(claimed.connector.timeout_ms))
        .json(&payload)
        .send()
        .map_err(|error| format!("Telegram request failed: {error}"))?;
    response_outcome(config, response)
}

fn send_sds(
    config: &ApplicationGatewayConfig,
    client: &Client,
    claimed: &ClaimedDelivery,
    tpg2200: bool,
) -> Result<DeliveryOutcome, String> {
    let destination = claimed
        .delivery
        .destination
        .as_deref()
        .or_else(|| claimed.delivery.payload.get("dest_issi").and_then(Value::as_u64).map(|_| ""))
        .unwrap_or("");
    let dest_issi = claimed
        .delivery
        .payload
        .get("dest_issi")
        .and_then(Value::as_u64)
        .map(|value| value as u32)
        .or_else(|| destination.parse::<u32>().ok())
        .ok_or_else(|| "SDS destination must be a numeric ISSI/GSSI".to_string())?;
    let source_issi = value_u32(
        &claimed.delivery.payload,
        "source_issi",
        setting_u32(&claimed.connector.settings, "source_issi", 9999),
    );
    let protocol_id = value_u32(
        &claimed.delivery.payload,
        "protocol_id",
        setting_u32(&claimed.connector.settings, "protocol_id", if tpg2200 { 130 } else { 0 }),
    );
    let priority = value_u32(
        &claimed.delivery.payload,
        "priority",
        setting_u32(&claimed.connector.settings, "priority", 3),
    );
    let sds_type = value_u32(
        &claimed.delivery.payload,
        "sds_type",
        setting_u32(&claimed.connector.settings, "sds_type", 4),
    );
    let is_group = claimed
        .delivery
        .payload
        .get("is_group")
        .and_then(Value::as_bool)
        .unwrap_or_else(|| setting_bool(&claimed.connector.settings, "is_group", false));
    let text = delivery_text(&claimed.delivery);
    let payload = json!({
        "source_issi": source_issi,
        "dest_issi": dest_issi,
        "is_group": is_group,
        "sds_type": sds_type,
        "protocol_id": protocol_id,
        "status_code": claimed.delivery.payload.get("status_code").cloned().unwrap_or(Value::Null),
        "payload_hex": claimed.delivery.payload.get("payload_hex").and_then(Value::as_str).unwrap_or(""),
        "text": text,
        "priority": priority,
        "ttl_secs": 300,
        "ingress": if tpg2200 { "application-gateway:tpg2200" } else { "application-gateway" },
        "force_nodes": claimed.delivery.payload.get("force_nodes").cloned().unwrap_or_else(|| json!([])),
        "application_metadata": {
            "delivery_id": claimed.delivery.delivery_id,
            "correlation_id": claimed.delivery.correlation_id,
            "tpg2200_bridge": tpg2200,
        }
    });
    let response = apply_auth(client.post(&claimed.connector.endpoint), &claimed.connector, &claimed.secrets)
        .timeout(Duration::from_millis(claimed.connector.timeout_ms))
        .json(&payload)
        .send()
        .map_err(|error| format!("SDS Router request failed: {error}"))?;
    response_outcome(config, response)
}

fn send_snom(
    config: &ApplicationGatewayConfig,
    client: &Client,
    claimed: &ClaimedDelivery,
) -> Result<DeliveryOutcome, String> {
    let body = claimed
        .delivery
        .payload
        .get("xml")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .unwrap_or_else(|| {
            let text = xml_escape(&delivery_text(&claimed.delivery));
            format!("<?xml version=\"1.0\"?><SnomIPPhoneText><Title>NetCore-Tetra</Title><Text>{text}</Text></SnomIPPhoneText>")
        });
    let response = apply_auth(client.post(&claimed.connector.endpoint), &claimed.connector, &claimed.secrets)
        .timeout(Duration::from_millis(claimed.connector.timeout_ms))
        .header(CONTENT_TYPE, "application/xml; charset=utf-8")
        .body(body)
        .send()
        .map_err(|error| format!("Snom notify request failed: {error}"))?;
    response_outcome(config, response)
}

fn send_weather(
    config: &ApplicationGatewayConfig,
    client: &Client,
    claimed: &ClaimedDelivery,
) -> Result<DeliveryOutcome, String> {
    let station = claimed
        .delivery
        .destination
        .as_deref()
        .or_else(|| claimed.delivery.payload.get("station").and_then(Value::as_str))
        .or_else(|| claimed.connector.settings.get("station").map(String::as_str))
        .unwrap_or("EDDV");
    let response = apply_auth(
        client.get(&claimed.connector.endpoint).query(&[("ids", station), ("format", "json")]),
        &claimed.connector,
        &claimed.secrets,
    )
    .timeout(Duration::from_millis(claimed.connector.timeout_ms))
    .send()
    .map_err(|error| format!("WX/METAR request failed: {error}"))?;
    response_outcome(config, response)
}

fn send_piper(
    config: &ApplicationGatewayConfig,
    client: &Client,
    claimed: &ClaimedDelivery,
) -> Result<DeliveryOutcome, String> {
    let job_id = claimed
        .delivery
        .payload
        .get("tts_job_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "Piper delivery has no tts_job_id".to_string())?;
    let text = claimed
        .delivery
        .payload
        .get("text")
        .and_then(Value::as_str)
        .ok_or_else(|| "Piper delivery has no text".to_string())?;
    let voice = claimed
        .delivery
        .payload
        .get("voice")
        .and_then(Value::as_str)
        .unwrap_or("de_DE-thorsten-medium");
    let length_scale = claimed
        .delivery
        .payload
        .get("length_scale")
        .and_then(Value::as_f64)
        .unwrap_or(1.0);
    let mut payload = json!({"text": text, "voice": voice, "length_scale": length_scale});
    if let Some(speaker_id) = claimed.delivery.payload.get("speaker_id").and_then(Value::as_u64) {
        payload["speaker_id"] = json!(speaker_id);
    }
    let mut response = client
        .post(&claimed.connector.endpoint)
        .timeout(Duration::from_millis(claimed.connector.timeout_ms))
        .json(&payload)
        .send()
        .map_err(|error| format!("Piper request failed: {error}"))?;
    let status = response.status();
    if !status.is_success() {
        return response_outcome(config, response);
    }
    let tts_dir = config.storage.spool_dir.join("tts");
    fs::create_dir_all(&tts_dir).map_err(|error| format!("cannot create {}: {error}", tts_dir.display()))?;
    let part_path = tts_dir.join(format!("{job_id}.part.wav"));
    let final_path = tts_dir.join(format!("{job_id}.wav"));
    let mut output = fs::File::create(&part_path)
        .map_err(|error| format!("cannot create {}: {error}", part_path.display()))?;
    let copied = std::io::copy(
        &mut response.by_ref().take(config.runtime.max_artifact_bytes as u64 + 1),
        &mut output,
    )
    .map_err(|error| format!("cannot store Piper WAV: {error}"))?;
    output.sync_all().map_err(|error| format!("cannot sync Piper WAV: {error}"))?;
    drop(output);
    if copied > config.runtime.max_artifact_bytes as u64 {
        let _ = fs::remove_file(&part_path);
        return Err(format!("Piper WAV exceeds {} bytes", config.runtime.max_artifact_bytes));
    }
    validate_wav(&part_path)?;
    fs::rename(&part_path, &final_path)
        .map_err(|error| format!("cannot finalize {}: {error}", final_path.display()))?;
    let bytes = fs::read(&final_path).map_err(|error| error.to_string())?;
    Ok(DeliveryOutcome {
        success: true,
        status: Some(status.as_u16()),
        response_excerpt: Some(format!("Piper WAV stored: {}", final_path.display())),
        error: None,
        artifact_path: Some(final_path),
        artifact_sha256: Some(sha256_hex(&bytes)),
        artifact_size_bytes: Some(bytes.len() as u64),
    })
}

fn response_outcome(
    config: &ApplicationGatewayConfig,
    mut response: Response,
) -> Result<DeliveryOutcome, String> {
    let status = response.status();
    let mut bytes = Vec::new();
    response
        .by_ref()
        .take(config.runtime.max_response_bytes as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read connector response: {error}"))?;
    let truncated = bytes.len() > config.runtime.max_response_bytes;
    bytes.truncate(config.runtime.max_response_bytes);
    let mut excerpt = String::from_utf8_lossy(&bytes).trim().to_string();
    if truncated {
        excerpt.push_str(" … [truncated]");
    }
    if excerpt.is_empty() {
        excerpt = format!("HTTP {status}");
    }
    Ok(DeliveryOutcome {
        success: status.is_success(),
        status: Some(status.as_u16()),
        response_excerpt: Some(excerpt.clone()),
        error: if status.is_success() {
            None
        } else {
            Some(format!("connector returned HTTP {}: {}", status, excerpt))
        },
        artifact_path: None,
        artifact_sha256: None,
        artifact_size_bytes: None,
    })
}

fn apply_auth(
    mut request: RequestBuilder,
    connector: &ConnectorRecord,
    secrets: &BTreeMap<String, String>,
) -> RequestBuilder {
    if let Some(token) = secrets.get("bearer_token") {
        request = request.header(AUTHORIZATION, format!("Bearer {token}"));
    }
    if let Some(api_key) = secrets.get("api_key") {
        request = request.header("X-API-Key", api_key);
    }
    if let Some(auth_key) = secrets.get("auth_key") {
        request = request.header("X-Auth-Key", auth_key);
    }
    if let Some(password) = secrets.get("basic_password") {
        let username = connector.settings.get("basic_username").cloned().unwrap_or_default();
        request = request.basic_auth(username, Some(password));
    }
    request
}

fn endpoint_with_secrets(endpoint: &str, secrets: &BTreeMap<String, String>) -> String {
    let mut endpoint = endpoint.to_string();
    for (name, value) in secrets {
        endpoint = endpoint.replace(&format!("{{{name}}}"), value);
    }
    endpoint
}

fn delivery_text(delivery: &DeliveryRecord) -> String {
    delivery
        .payload
        .get("text")
        .and_then(Value::as_str)
        .or(delivery.text.as_deref())
        .map(ToString::to_string)
        .unwrap_or_else(|| delivery.payload.to_string())
}

fn setting_u32(settings: &BTreeMap<String, String>, key: &str, fallback: u32) -> u32 {
    settings.get(key).and_then(|value| value.parse().ok()).unwrap_or(fallback)
}

fn setting_bool(settings: &BTreeMap<String, String>, key: &str, fallback: bool) -> bool {
    settings
        .get(key)
        .and_then(|value| match value.to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Some(true),
            "false" | "0" | "no" | "off" => Some(false),
            _ => None,
        })
        .unwrap_or(fallback)
}

fn value_u32(payload: &Value, key: &str, fallback: u32) -> u32 {
    payload.get(key).and_then(Value::as_u64).map(|value| value as u32).unwrap_or(fallback)
}

fn validate_wav(path: &Path) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("Piper response is not a RIFF/WAVE file".into());
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

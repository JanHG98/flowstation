use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Duration, Utc};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::config::{
    slug, ApplicationGatewayConfig, ConnectorSeed, RouteRuleSeed, TemplateSeed, AUTHORITATIVE_MODE,
};
use crate::model::{
    ActionInput, AuditRecord, BackupInput, BackupRecord, ClaimedDelivery, ConnectorInput,
    ConnectorProbeOutcome, ConnectorRecord, DeliveryOutcome, DeliveryRecord, DispatchInput,
    EventRecord, GatewayStatus, PersistedGateway, RouteRuleInput, RouteRuleRecord, SecretEntry,
    SecretSetInput, SecretStatus, SecretVault, TemplateInput, TemplateRecord, TemplateRenderInput,
    TtsJobInput, TtsJobRecord, TtsPublishInput,
};

#[derive(Clone)]
pub struct SharedGateway {
    inner: Arc<Mutex<GatewayInner>>,
}

struct GatewayInner {
    config: ApplicationGatewayConfig,
    state: PersistedGateway,
    secrets: SecretVault,
}

impl SharedGateway {
    pub fn load(config: ApplicationGatewayConfig) -> Result<Self, Box<dyn std::error::Error>> {
        ensure_parent(&config.storage.state_path)?;
        ensure_parent(&config.storage.secrets_path)?;
        fs::create_dir_all(&config.storage.spool_dir)?;
        fs::create_dir_all(&config.storage.backup_dir)?;

        let now = Utc::now();
        let mut state = if config.storage.state_path.is_file() {
            serde_json::from_slice::<PersistedGateway>(&fs::read(&config.storage.state_path)?)?
        } else {
            empty_state(now)
        };
        recover_inflight(&mut state, now);
        seed_state(&config, &mut state, now)?;
        let secrets = if config.storage.secrets_path.is_file() {
            serde_json::from_slice::<SecretVault>(&fs::read(&config.storage.secrets_path)?)?
        } else {
            SecretVault::default()
        };

        let gateway = Self {
            inner: Arc::new(Mutex::new(GatewayInner {
                config,
                state,
                secrets,
            })),
        };
        gateway.persist_all()?;
        Ok(gateway)
    }

    pub fn status(&self) -> GatewayStatus {
        let inner = lock(&self.inner);
        status_locked(&inner)
    }

    pub fn redacted_config(&self) -> Value {
        let inner = lock(&self.inner);
        json!({
            "server": inner.config.server,
            "storage": {
                "state_path": inner.config.storage.state_path,
                "state_backup_path": inner.config.storage.state_backup_path,
                "secrets_path": inner.config.storage.secrets_path,
                "spool_dir": inner.config.storage.spool_dir,
                "backup_dir": inner.config.storage.backup_dir,
            },
            "security": inner.config.security,
            "runtime": inner.config.runtime,
            "note": "Connector secret values are intentionally excluded from management responses",
        })
    }

    pub fn connectors(&self) -> Vec<Value> {
        let inner = lock(&self.inner);
        inner
            .state
            .connectors
            .values()
            .map(|connector| connector_view(&inner, connector))
            .collect()
    }

    pub fn connector(&self, connector_id: &str) -> Option<Value> {
        let inner = lock(&self.inner);
        inner
            .state
            .connectors
            .get(&slug(connector_id))
            .map(|connector| connector_view(&inner, connector))
    }

    pub fn create_connector(&self, input: ConnectorInput) -> Result<Value, String> {
        let mut inner = lock(&self.inner);
        let id = slug(&input.connector_id);
        if id.is_empty() {
            return Err("connector_id is required".into());
        }
        if inner.state.connectors.contains_key(&id) {
            return Err(format!("connector {id} already exists"));
        }
        validate_connector_input(&input)?;
        let now = Utc::now();
        let record = ConnectorRecord {
            connector_id: id.clone(),
            display_name: non_empty(&input.display_name, &id),
            kind: input.kind.trim().to_ascii_lowercase(),
            direction: input.direction.trim().to_ascii_lowercase(),
            endpoint: input.endpoint.trim().trim_end_matches('/').to_string(),
            health_endpoint: clean_optional(input.health_endpoint),
            enabled: input.enabled.unwrap_or(false),
            timeout_ms: input.timeout_ms.unwrap_or(5_000).max(100),
            rate_limit_per_minute: input.rate_limit_per_minute.unwrap_or(60).max(1),
            circuit_failure_threshold: input.circuit_failure_threshold.unwrap_or(5).max(1),
            circuit_open_secs: input.circuit_open_secs.unwrap_or(60).max(5),
            required_secrets: input.required_secrets.iter().map(|value| slug(value)).collect(),
            settings: input.settings,
            health: "unknown".to_string(),
            circuit_state: "closed".to_string(),
            circuit_open_until: None,
            consecutive_failures: 0,
            sent_total: 0,
            failed_total: 0,
            received_total: 0,
            rate_window_started_at: now,
            rate_window_count: 0,
            last_probe_at: None,
            last_success_at: None,
            last_failure_at: None,
            last_error: None,
            created_at: now,
            updated_at: now,
        };
        inner.state.connectors.insert(id.clone(), record);
        audit_locked(
            &mut inner,
            actor(input.actor.as_deref()),
            "connector",
            "create",
            "connector",
            &id,
            "ok",
            json!({"kind": input.kind}),
        );
        persist_locked(&mut inner).map_err(|error| error.to_string())?;
        Ok(connector_view(&inner, inner.state.connectors.get(&id).expect("inserted connector")))
    }

    pub fn update_connector(&self, connector_id: &str, input: ConnectorInput) -> Result<Value, String> {
        let mut inner = lock(&self.inner);
        let id = slug(connector_id);
        validate_connector_input(&input)?;
        let now = Utc::now();
        let record = inner
            .state
            .connectors
            .get_mut(&id)
            .ok_or_else(|| format!("connector {id} not found"))?;
        record.display_name = non_empty(&input.display_name, &id);
        record.kind = input.kind.trim().to_ascii_lowercase();
        record.direction = input.direction.trim().to_ascii_lowercase();
        record.endpoint = input.endpoint.trim().trim_end_matches('/').to_string();
        record.health_endpoint = clean_optional(input.health_endpoint);
        if let Some(enabled) = input.enabled {
            record.enabled = enabled;
        }
        if let Some(value) = input.timeout_ms {
            record.timeout_ms = value.max(100);
        }
        if let Some(value) = input.rate_limit_per_minute {
            record.rate_limit_per_minute = value.max(1);
        }
        if let Some(value) = input.circuit_failure_threshold {
            record.circuit_failure_threshold = value.max(1);
        }
        if let Some(value) = input.circuit_open_secs {
            record.circuit_open_secs = value.max(5);
        }
        record.required_secrets = input.required_secrets.iter().map(|value| slug(value)).collect();
        record.settings = input.settings;
        record.updated_at = now;
        audit_locked(
            &mut inner,
            actor(input.actor.as_deref()),
            "connector",
            "update",
            "connector",
            &id,
            "ok",
            json!({}),
        );
        persist_locked(&mut inner).map_err(|error| error.to_string())?;
        Ok(connector_view(&inner, inner.state.connectors.get(&id).expect("updated connector")))
    }

    pub fn connector_action(&self, connector_id: &str, action: &str, input: ActionInput) -> Result<Value, String> {
        let mut inner = lock(&self.inner);
        let id = slug(connector_id);
        let now = Utc::now();
        let record = inner
            .state
            .connectors
            .get_mut(&id)
            .ok_or_else(|| format!("connector {id} not found"))?;
        match action {
            "enable" => {
                record.enabled = true;
                record.health = "unknown".to_string();
            }
            "disable" => {
                record.enabled = false;
                record.health = "disabled".to_string();
            }
            "reset-circuit" => {
                record.circuit_state = "closed".to_string();
                record.circuit_open_until = None;
                record.consecutive_failures = 0;
                record.last_error = None;
            }
            _ => return Err(format!("unsupported connector action {action}")),
        }
        record.updated_at = now;
        audit_locked(
            &mut inner,
            actor(input.actor.as_deref()),
            "connector",
            action,
            "connector",
            &id,
            "ok",
            json!({"reason": input.reason}),
        );
        persist_locked(&mut inner).map_err(|error| error.to_string())?;
        Ok(connector_view(&inner, inner.state.connectors.get(&id).expect("connector exists")))
    }

    pub fn delete_connector(&self, connector_id: &str, input: ActionInput) -> Result<(), String> {
        let mut inner = lock(&self.inner);
        let id = slug(connector_id);
        if inner
            .state
            .rules
            .values()
            .any(|rule| rule.target_connector == id)
        {
            return Err("connector is referenced by a routing rule".into());
        }
        inner
            .state
            .connectors
            .remove(&id)
            .ok_or_else(|| format!("connector {id} not found"))?;
        inner.secrets.connectors.remove(&id);
        audit_locked(
            &mut inner,
            actor(input.actor.as_deref()),
            "connector",
            "delete",
            "connector",
            &id,
            "ok",
            json!({"reason": input.reason}),
        );
        persist_locked(&mut inner).map_err(|error| error.to_string())?;
        persist_secrets_locked(&inner).map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn secret_statuses(&self, connector_id: Option<&str>) -> Vec<SecretStatus> {
        let inner = lock(&self.inner);
        secret_statuses_locked(&inner, connector_id)
    }

    pub fn set_secret(&self, connector_id: &str, input: SecretSetInput) -> Result<SecretStatus, String> {
        let mut inner = lock(&self.inner);
        if !inner.config.security.connector_secrets_allowed {
            return Err("connector secrets are disabled by configuration".into());
        }
        let id = slug(connector_id);
        let name = slug(&input.name);
        if name.is_empty() || input.value.is_empty() {
            return Err("secret name and value are required".into());
        }
        let connector = inner
            .state
            .connectors
            .get(&id)
            .ok_or_else(|| format!("connector {id} not found"))?;
        let required = connector.required_secrets.contains(&name);
        let now = Utc::now();
        let entry = SecretEntry {
            fingerprint: sha256_hex(input.value.as_bytes())[..16].to_string(),
            value: input.value,
            updated_at: now,
        };
        let status = SecretStatus {
            connector_id: id.clone(),
            name: name.clone(),
            present: true,
            fingerprint: Some(entry.fingerprint.clone()),
            updated_at: Some(now),
            required,
        };
        inner
            .secrets
            .connectors
            .entry(id.clone())
            .or_default()
            .insert(name.clone(), entry);
        audit_locked(
            &mut inner,
            actor(input.actor.as_deref()),
            "secret",
            "set",
            "connector-secret",
            &format!("{id}/{name}"),
            "ok",
            json!({"fingerprint": status.fingerprint}),
        );
        persist_secrets_locked(&inner).map_err(|error| error.to_string())?;
        persist_locked(&mut inner).map_err(|error| error.to_string())?;
        Ok(status)
    }

    pub fn delete_secret(&self, connector_id: &str, name: &str, input: ActionInput) -> Result<(), String> {
        let mut inner = lock(&self.inner);
        let id = slug(connector_id);
        let name = slug(name);
        let removed = inner
            .secrets
            .connectors
            .get_mut(&id)
            .and_then(|values| values.remove(&name));
        if removed.is_none() {
            return Err(format!("secret {id}/{name} not found"));
        }
        audit_locked(
            &mut inner,
            actor(input.actor.as_deref()),
            "secret",
            "delete",
            "connector-secret",
            &format!("{id}/{name}"),
            "ok",
            json!({"reason": input.reason}),
        );
        persist_secrets_locked(&inner).map_err(|error| error.to_string())?;
        persist_locked(&mut inner).map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn rules(&self) -> Vec<RouteRuleRecord> {
        let inner = lock(&self.inner);
        let mut rows: Vec<_> = inner.state.rules.values().cloned().collect();
        rows.sort_by(|left, right| right.priority.cmp(&left.priority).then(left.rule_id.cmp(&right.rule_id)));
        rows
    }

    pub fn create_rule(&self, input: RouteRuleInput) -> Result<RouteRuleRecord, String> {
        let mut inner = lock(&self.inner);
        let id = slug(&input.rule_id);
        if inner.state.rules.contains_key(&id) {
            return Err(format!("rule {id} already exists"));
        }
        validate_rule_input(&inner, &input)?;
        let now = Utc::now();
        let record = rule_from_input(id.clone(), input.clone(), now, 0);
        inner.state.rules.insert(id.clone(), record.clone());
        audit_locked(
            &mut inner,
            actor(input.actor.as_deref()),
            "routing",
            "create",
            "rule",
            &id,
            "ok",
            json!({"target_connector": record.target_connector}),
        );
        persist_locked(&mut inner).map_err(|error| error.to_string())?;
        Ok(record)
    }

    pub fn update_rule(&self, rule_id: &str, input: RouteRuleInput) -> Result<RouteRuleRecord, String> {
        let mut inner = lock(&self.inner);
        let id = slug(rule_id);
        validate_rule_input(&inner, &input)?;
        let old = inner
            .state
            .rules
            .get(&id)
            .ok_or_else(|| format!("rule {id} not found"))?;
        let record = rule_from_input(id.clone(), input.clone(), old.created_at, old.matched_total);
        inner.state.rules.insert(id.clone(), record.clone());
        audit_locked(
            &mut inner,
            actor(input.actor.as_deref()),
            "routing",
            "update",
            "rule",
            &id,
            "ok",
            json!({}),
        );
        persist_locked(&mut inner).map_err(|error| error.to_string())?;
        Ok(record)
    }

    pub fn rule_action(&self, rule_id: &str, action: &str, input: ActionInput) -> Result<RouteRuleRecord, String> {
        let mut inner = lock(&self.inner);
        let id = slug(rule_id);
        let record = inner
            .state
            .rules
            .get_mut(&id)
            .ok_or_else(|| format!("rule {id} not found"))?;
        match action {
            "enable" => record.enabled = true,
            "disable" => record.enabled = false,
            _ => return Err(format!("unsupported rule action {action}")),
        }
        record.updated_at = Utc::now();
        let result = record.clone();
        audit_locked(
            &mut inner,
            actor(input.actor.as_deref()),
            "routing",
            action,
            "rule",
            &id,
            "ok",
            json!({"reason": input.reason}),
        );
        persist_locked(&mut inner).map_err(|error| error.to_string())?;
        Ok(result)
    }

    pub fn delete_rule(&self, rule_id: &str, input: ActionInput) -> Result<(), String> {
        let mut inner = lock(&self.inner);
        let id = slug(rule_id);
        inner
            .state
            .rules
            .remove(&id)
            .ok_or_else(|| format!("rule {id} not found"))?;
        audit_locked(
            &mut inner,
            actor(input.actor.as_deref()),
            "routing",
            "delete",
            "rule",
            &id,
            "ok",
            json!({"reason": input.reason}),
        );
        persist_locked(&mut inner).map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn templates(&self) -> Vec<TemplateRecord> {
        let inner = lock(&self.inner);
        inner.state.templates.values().cloned().collect()
    }

    pub fn create_template(&self, input: TemplateInput) -> Result<TemplateRecord, String> {
        let mut inner = lock(&self.inner);
        let id = slug(&input.template_id);
        if inner.state.templates.contains_key(&id) {
            return Err(format!("template {id} already exists"));
        }
        validate_template_input(&inner, &input)?;
        let now = Utc::now();
        let record = template_from_input(id.clone(), input.clone(), now, 0);
        inner.state.templates.insert(id.clone(), record.clone());
        audit_locked(
            &mut inner,
            actor(input.actor.as_deref()),
            "template",
            "create",
            "template",
            &id,
            "ok",
            json!({"kind": record.kind}),
        );
        persist_locked(&mut inner).map_err(|error| error.to_string())?;
        Ok(record)
    }

    pub fn update_template(&self, template_id: &str, input: TemplateInput) -> Result<TemplateRecord, String> {
        let mut inner = lock(&self.inner);
        let id = slug(template_id);
        validate_template_input(&inner, &input)?;
        let old = inner
            .state
            .templates
            .get(&id)
            .ok_or_else(|| format!("template {id} not found"))?;
        let record = template_from_input(id.clone(), input.clone(), old.created_at, old.render_total);
        inner.state.templates.insert(id.clone(), record.clone());
        audit_locked(
            &mut inner,
            actor(input.actor.as_deref()),
            "template",
            "update",
            "template",
            &id,
            "ok",
            json!({}),
        );
        persist_locked(&mut inner).map_err(|error| error.to_string())?;
        Ok(record)
    }

    pub fn delete_template(&self, template_id: &str, input: ActionInput) -> Result<(), String> {
        let mut inner = lock(&self.inner);
        let id = slug(template_id);
        if inner
            .state
            .rules
            .values()
            .any(|rule| rule.template_id.as_deref() == Some(id.as_str()))
        {
            return Err("template is referenced by a routing rule".into());
        }
        inner
            .state
            .templates
            .remove(&id)
            .ok_or_else(|| format!("template {id} not found"))?;
        audit_locked(
            &mut inner,
            actor(input.actor.as_deref()),
            "template",
            "delete",
            "template",
            &id,
            "ok",
            json!({"reason": input.reason}),
        );
        persist_locked(&mut inner).map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn render_template(&self, template_id: &str, input: TemplateRenderInput) -> Result<Value, String> {
        let mut inner = lock(&self.inner);
        let id = slug(template_id);
        let template = inner
            .state
            .templates
            .get_mut(&id)
            .ok_or_else(|| format!("template {id} not found"))?;
        let event = RenderContext {
            source: input.source.unwrap_or_else(|| "preview".to_string()),
            event_type: input.event_type.unwrap_or_else(|| "preview".to_string()),
            destination: input.destination,
            text: input.text,
            payload: input.payload,
        };
        let value = render_template_value(template, &event)?;
        template.render_total = template.render_total.saturating_add(1);
        template.updated_at = Utc::now();
        persist_locked(&mut inner).map_err(|error| error.to_string())?;
        Ok(value)
    }

    pub fn dispatch(&self, input: DispatchInput) -> Result<EventRecord, String> {
        let mut inner = lock(&self.inner);
        let event = dispatch_locked(&mut inner, input)?;
        persist_locked(&mut inner).map_err(|error| error.to_string())?;
        Ok(event)
    }

    pub fn ingest_webhook(&self, connector_id: &str, mut input: DispatchInput) -> Result<EventRecord, String> {
        let mut inner = lock(&self.inner);
        let id = slug(connector_id);
        let connector = inner
            .state
            .connectors
            .get_mut(&id)
            .ok_or_else(|| format!("connector {id} not found"))?;
        if !matches!(connector.direction.as_str(), "inbound" | "bidirectional") {
            return Err(format!("connector {id} is not configured for inbound traffic"));
        }
        connector.received_total = connector.received_total.saturating_add(1);
        connector.updated_at = Utc::now();
        input.source_connector = Some(id.clone());
        if input.actor.is_none() {
            input.actor = Some(format!("webhook:{id}"));
        }
        let event = dispatch_locked(&mut inner, input)?;
        persist_locked(&mut inner).map_err(|error| error.to_string())?;
        Ok(event)
    }

    pub fn events(&self, state: Option<&str>, source: Option<&str>, limit: usize) -> Vec<EventRecord> {
        let inner = lock(&self.inner);
        inner
            .state
            .events
            .iter()
            .rev()
            .filter(|event| state.is_none_or(|value| event.state == value))
            .filter(|event| source.is_none_or(|value| event.source_connector == value))
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn deliveries(&self, state: Option<&str>, connector: Option<&str>, limit: usize) -> Vec<DeliveryRecord> {
        let inner = lock(&self.inner);
        inner
            .state
            .deliveries
            .iter()
            .rev()
            .filter(|delivery| state.is_none_or(|value| delivery.state == value))
            .filter(|delivery| connector.is_none_or(|value| delivery.connector_id == value))
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn delivery(&self, delivery_id: &str) -> Option<DeliveryRecord> {
        let inner = lock(&self.inner);
        inner
            .state
            .deliveries
            .iter()
            .find(|delivery| delivery.delivery_id == delivery_id)
            .cloned()
    }

    pub fn delivery_action(&self, delivery_id: &str, action: &str, input: ActionInput) -> Result<DeliveryRecord, String> {
        let mut inner = lock(&self.inner);
        let now = Utc::now();
        let delivery = inner
            .state
            .deliveries
            .iter_mut()
            .find(|delivery| delivery.delivery_id == delivery_id)
            .ok_or_else(|| format!("delivery {delivery_id} not found"))?;
        match action {
            "retry" | "requeue" => {
                delivery.state = "queued".to_string();
                delivery.next_attempt_at = now;
                delivery.last_error = None;
                if action == "requeue" {
                    delivery.attempts = 0;
                }
            }
            "cancel" => {
                if matches!(delivery.state.as_str(), "delivered" | "cancelled") {
                    return Err(format!("delivery is already {}", delivery.state));
                }
                delivery.state = "cancelled".to_string();
            }
            _ => return Err(format!("unsupported delivery action {action}")),
        }
        delivery.updated_at = now;
        let result = delivery.clone();
        audit_locked(
            &mut inner,
            actor(input.actor.as_deref()),
            "delivery",
            action,
            "delivery",
            delivery_id,
            "ok",
            json!({"reason": input.reason}),
        );
        persist_locked(&mut inner).map_err(|error| error.to_string())?;
        Ok(result)
    }

    pub fn tts_jobs(&self, state: Option<&str>, limit: usize) -> Vec<TtsJobRecord> {
        let inner = lock(&self.inner);
        inner
            .state
            .tts_jobs
            .iter()
            .rev()
            .filter(|job| state.is_none_or(|value| job.state == value))
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn create_tts_job(&self, input: TtsJobInput) -> Result<TtsJobRecord, String> {
        let mut inner = lock(&self.inner);
        if input.text.trim().is_empty() {
            return Err("TTS text is required".into());
        }
        let speed = input.speed.unwrap_or(0.95);
        if !(0.5..=2.0).contains(&speed) {
            return Err("TTS speed must be between 0.5 and 2.0".into());
        }
        if !inner.state.connectors.contains_key("piper-tts") {
            return Err("piper-tts connector is not configured".into());
        }
        let now = Utc::now();
        let job_id = Uuid::new_v4().to_string();
        let rendered_text = if let Some(template_id) = &input.template_id {
            let template = inner
                .state
                .templates
                .get(template_id)
                .ok_or_else(|| format!("template {template_id} not found"))?;
            let context = RenderContext {
                source: "tts".to_string(),
                event_type: "tts.request".to_string(),
                destination: input.destination_id.map(|value| value.to_string()),
                text: Some(input.text.clone()),
                payload: json!({}),
            };
            value_text(render_template_value(template, &context)?)
        } else {
            input.text.clone()
        };
        let event_id = Uuid::new_v4().to_string();
        let delivery_id = Uuid::new_v4().to_string();
        let voice = input.voice.unwrap_or_else(|| "de_DE-thorsten-medium".to_string());
        let payload = json!({
            "tts_job_id": job_id,
            "text": rendered_text,
            "voice": voice,
            "length_scale": 1.0_f32 / speed,
            "speaker_id": input.speaker_id,
        });
        let expires_at = now + Duration::seconds(inner.config.runtime.default_ttl_secs as i64);
        inner.state.events.push(EventRecord {
            event_id: event_id.clone(),
            source_connector: "tts".to_string(),
            event_type: "tts.request".to_string(),
            destination: input.destination_id.map(|value| value.to_string()),
            text: Some(rendered_text.clone()),
            payload: payload.clone(),
            idempotency_key: None,
            correlation_id: job_id.clone(),
            priority: input.priority.unwrap_or(3) as i32,
            state: "routed".to_string(),
            matched_rules: Vec::new(),
            delivery_ids: vec![delivery_id.clone()],
            received_at: now,
            updated_at: now,
            expires_at,
        });
        inner.state.deliveries.push(DeliveryRecord {
            delivery_id: delivery_id.clone(),
            event_id,
            connector_id: "piper-tts".to_string(),
            template_id: input.template_id.clone(),
            event_type: "tts.request".to_string(),
            destination: input.destination_id.map(|value| value.to_string()),
            text: Some(rendered_text.clone()),
            payload,
            content_type: "application/json".to_string(),
            correlation_id: job_id.clone(),
            priority: input.priority.unwrap_or(3) as i32,
            state: "queued".to_string(),
            attempts: 0,
            max_attempts: inner.config.runtime.max_attempts,
            next_attempt_at: now,
            expires_at,
            last_attempt_at: None,
            delivered_at: None,
            response_status: None,
            response_excerpt: None,
            last_error: None,
            artifact_path: None,
            artifact_sha256: None,
            artifact_size_bytes: None,
            created_at: now,
            updated_at: now,
        });
        let job = TtsJobRecord {
            job_id: job_id.clone(),
            name: non_empty(&input.name, "TTS announcement"),
            template_id: input.template_id,
            text: input.text,
            rendered_text,
            voice,
            speed,
            speaker_id: input.speaker_id,
            state: "queued".to_string(),
            synthesis_delivery_id: delivery_id,
            publish_delivery_id: None,
            destination_kind: input.destination_kind,
            destination_id: input.destination_id,
            priority: input.priority.unwrap_or(3).min(15),
            artifact_path: None,
            artifact_url: None,
            artifact_sha256: None,
            artifact_size_bytes: None,
            created_at: now,
            updated_at: now,
            completed_at: None,
            last_error: None,
        };
        inner.state.tts_jobs.push(job.clone());
        audit_locked(
            &mut inner,
            actor(input.actor.as_deref()),
            "tts",
            "create",
            "tts-job",
            &job_id,
            "ok",
            json!({"voice": job.voice, "chars": job.rendered_text.chars().count()}),
        );
        trim_locked(&mut inner);
        persist_locked(&mut inner).map_err(|error| error.to_string())?;
        Ok(job)
    }

    pub fn publish_tts_job(&self, job_id: &str, input: TtsPublishInput) -> Result<TtsJobRecord, String> {
        let mut inner = lock(&self.inner);
        if !inner.state.connectors.contains_key("media-library") {
            return Err("media-library connector is not configured".into());
        }
        let now = Utc::now();
        let job_index = inner
            .state
            .tts_jobs
            .iter()
            .position(|job| job.job_id == job_id)
            .ok_or_else(|| format!("TTS job {job_id} not found"))?;
        let job = inner.state.tts_jobs[job_index].clone();
        if job.artifact_path.as_ref().is_none_or(|path| !path.is_file()) {
            return Err("TTS job has no synthesized WAV artifact".into());
        }
        if job.publish_delivery_id.is_some() {
            return Err("TTS job already has a publish delivery".into());
        }
        let event_id = Uuid::new_v4().to_string();
        let delivery_id = Uuid::new_v4().to_string();
        let artifact_url = format!(
            "{}/api/v1/tts/jobs/{}/artifact",
            inner.config.server.public_base_url, job_id
        );
        let destination_kind = input.destination_kind.or(job.destination_kind.clone());
        let destination_id = input.destination_id.or(job.destination_id);
        let priority = input.priority.unwrap_or(job.priority).min(15);
        let payload = json!({
            "schema": "netcore-media-import-v1",
            "source": "application-gateway",
            "source_url": artifact_url,
            "name": job.name,
            "sha256": job.artifact_sha256,
            "size_bytes": job.artifact_size_bytes,
            "media_type": "audio/wav",
            "kind": "tts",
            "voice": job.voice,
            "text": job.rendered_text,
            "broadcast": {
                "destination_kind": destination_kind,
                "destination_id": destination_id,
                "priority": priority,
            }
        });
        let expires_at = now + Duration::seconds(inner.config.runtime.default_ttl_secs as i64);
        inner.state.events.push(EventRecord {
            event_id: event_id.clone(),
            source_connector: "tts".to_string(),
            event_type: "tts.ready".to_string(),
            destination: destination_id.map(|value| value.to_string()),
            text: Some(job.rendered_text.clone()),
            payload: payload.clone(),
            idempotency_key: Some(format!("tts-publish:{job_id}")),
            correlation_id: job_id.to_string(),
            priority: priority as i32,
            state: "routed".to_string(),
            matched_rules: Vec::new(),
            delivery_ids: vec![delivery_id.clone()],
            received_at: now,
            updated_at: now,
            expires_at,
        });
        inner.state.deliveries.push(DeliveryRecord {
            delivery_id: delivery_id.clone(),
            event_id,
            connector_id: "media-library".to_string(),
            template_id: None,
            event_type: "tts.ready".to_string(),
            destination: destination_id.map(|value| value.to_string()),
            text: Some(job.rendered_text.clone()),
            payload,
            content_type: "application/json".to_string(),
            correlation_id: job_id.to_string(),
            priority: priority as i32,
            state: "queued".to_string(),
            attempts: 0,
            max_attempts: inner.config.runtime.max_attempts,
            next_attempt_at: now,
            expires_at,
            last_attempt_at: None,
            delivered_at: None,
            response_status: None,
            response_excerpt: None,
            last_error: None,
            artifact_path: None,
            artifact_sha256: None,
            artifact_size_bytes: None,
            created_at: now,
            updated_at: now,
        });
        let updated = &mut inner.state.tts_jobs[job_index];
        updated.publish_delivery_id = Some(delivery_id);
        updated.artifact_url = Some(artifact_url);
        updated.destination_kind = destination_kind;
        updated.destination_id = destination_id;
        updated.priority = priority;
        updated.state = "publish_queued".to_string();
        updated.updated_at = now;
        let result = updated.clone();
        audit_locked(
            &mut inner,
            actor(input.actor.as_deref()),
            "tts",
            "publish",
            "tts-job",
            job_id,
            "ok",
            json!({"destination_kind": result.destination_kind, "destination_id": result.destination_id}),
        );
        persist_locked(&mut inner).map_err(|error| error.to_string())?;
        Ok(result)
    }

    pub fn tts_artifact(&self, job_id: &str) -> Result<(String, Vec<u8>), String> {
        let inner = lock(&self.inner);
        let job = inner
            .state
            .tts_jobs
            .iter()
            .find(|job| job.job_id == job_id)
            .ok_or_else(|| format!("TTS job {job_id} not found"))?;
        let path = job
            .artifact_path
            .as_ref()
            .ok_or_else(|| "TTS job has no artifact".to_string())?;
        let bytes = fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        Ok((format!("{}.wav", safe_filename(&job.name)), bytes))
    }

    pub fn claim_due_delivery(&self) -> Result<Option<ClaimedDelivery>, String> {
        let mut inner = lock(&self.inner);
        let now = Utc::now();
        maintenance_locked(&mut inner, now);

        if inner.config.runtime.operating_mode != AUTHORITATIVE_MODE {
            let mut shadowed_delivery_ids = Vec::new();
            for delivery in inner
                .state
                .deliveries
                .iter_mut()
                .filter(|delivery| matches!(delivery.state.as_str(), "queued" | "retry") && delivery.next_attempt_at <= now)
                .take(100)
            {
                delivery.state = "shadowed".to_string();
                delivery.delivered_at = Some(now);
                delivery.updated_at = now;
                delivery.response_excerpt = Some("OPEN LAB shadow mode: external side effect suppressed".to_string());
                shadowed_delivery_ids.push(delivery.delivery_id.clone());
            }
            if !shadowed_delivery_ids.is_empty() {
                let shadowed: HashSet<&str> = shadowed_delivery_ids.iter().map(String::as_str).collect();
                for job in &mut inner.state.tts_jobs {
                    if shadowed.contains(job.synthesis_delivery_id.as_str()) {
                        job.state = "shadowed".to_string();
                        job.updated_at = now;
                    }
                    if job
                        .publish_delivery_id
                        .as_deref()
                        .is_some_and(|delivery_id| shadowed.contains(delivery_id))
                    {
                        job.state = "publish_shadowed".to_string();
                        job.updated_at = now;
                    }
                }
                persist_locked(&mut inner).map_err(|error| error.to_string())?;
            }
            return Ok(None);
        }

        let candidate_index = inner.state.deliveries.iter().position(|delivery| {
            if !matches!(delivery.state.as_str(), "queued" | "retry")
                || delivery.next_attempt_at > now
                || delivery.expires_at <= now
            {
                return false;
            }
            let Some(connector) = inner.state.connectors.get(&delivery.connector_id) else {
                return false;
            };
            if !connector.enabled {
                return false;
            }
            if connector.circuit_state == "open"
                && connector.circuit_open_until.is_some_and(|until| until > now)
            {
                return false;
            }
            required_secrets_present(&inner, connector)
        });

        let Some(index) = candidate_index else {
            persist_locked(&mut inner).map_err(|error| error.to_string())?;
            return Ok(None);
        };
        let connector_id = inner.state.deliveries[index].connector_id.clone();
        let rate_limited_until = {
            let connector = inner
                .state
                .connectors
                .get_mut(&connector_id)
                .expect("candidate connector exists");
            if now - connector.rate_window_started_at >= Duration::seconds(60) {
                connector.rate_window_started_at = now;
                connector.rate_window_count = 0;
            }
            if connector.rate_window_count >= connector.rate_limit_per_minute {
                Some(connector.rate_window_started_at + Duration::seconds(60))
            } else {
                if connector.circuit_state == "open" {
                    connector.circuit_state = "half_open".to_string();
                    connector.circuit_open_until = None;
                }
                connector.rate_window_count = connector.rate_window_count.saturating_add(1);
                connector.updated_at = now;
                None
            }
        };
        if let Some(next_attempt_at) = rate_limited_until {
            inner.state.deliveries[index].next_attempt_at = next_attempt_at;
            persist_locked(&mut inner).map_err(|error| error.to_string())?;
            return Ok(None);
        }
        let delivery = &mut inner.state.deliveries[index];
        delivery.state = "in_flight".to_string();
        delivery.attempts = delivery.attempts.saturating_add(1);
        delivery.last_attempt_at = Some(now);
        delivery.updated_at = now;
        let claimed_delivery = delivery.clone();
        let connector = inner
            .state
            .connectors
            .get(&connector_id)
            .expect("candidate connector exists")
            .clone();
        let secrets = inner
            .secrets
            .connectors
            .get(&connector_id)
            .map(|values| {
                values
                    .iter()
                    .map(|(name, entry)| (name.clone(), entry.value.clone()))
                    .collect()
            })
            .unwrap_or_default();
        if let Some(job) = inner
            .state
            .tts_jobs
            .iter_mut()
            .find(|job| job.synthesis_delivery_id == claimed_delivery.delivery_id)
        {
            job.state = "synthesizing".to_string();
            job.updated_at = now;
        }
        persist_locked(&mut inner).map_err(|error| error.to_string())?;
        Ok(Some(ClaimedDelivery {
            delivery: claimed_delivery,
            connector,
            secrets,
        }))
    }

    pub fn finish_delivery(&self, delivery_id: &str, outcome: DeliveryOutcome) -> Result<DeliveryRecord, String> {
        let mut inner = lock(&self.inner);
        let now = Utc::now();
        let index = inner
            .state
            .deliveries
            .iter()
            .position(|delivery| delivery.delivery_id == delivery_id)
            .ok_or_else(|| format!("delivery {delivery_id} not found"))?;
        let connector_id = inner.state.deliveries[index].connector_id.clone();
        let attempts = inner.state.deliveries[index].attempts;
        let max_attempts = inner.state.deliveries[index].max_attempts;
        let expires_at = inner.state.deliveries[index].expires_at;
        let base_backoff = inner.config.runtime.base_backoff_secs;
        let max_backoff = inner.config.runtime.max_backoff_secs;

        {
            let delivery = &mut inner.state.deliveries[index];
            delivery.response_status = outcome.status;
            delivery.response_excerpt = outcome.response_excerpt.clone();
            delivery.last_error = outcome.error.clone();
            delivery.artifact_path = outcome.artifact_path.clone();
            delivery.artifact_sha256 = outcome.artifact_sha256.clone();
            delivery.artifact_size_bytes = outcome.artifact_size_bytes;
            delivery.updated_at = now;
            if outcome.success {
                delivery.state = "delivered".to_string();
                delivery.delivered_at = Some(now);
            } else if attempts >= max_attempts || expires_at <= now {
                delivery.state = "dead_letter".to_string();
            } else {
                delivery.state = "retry".to_string();
                let exponent = attempts.saturating_sub(1).min(20);
                let backoff = base_backoff.saturating_mul(1_u64 << exponent).min(max_backoff);
                delivery.next_attempt_at = now + Duration::seconds(backoff as i64);
            }
        }

        if let Some(connector) = inner.state.connectors.get_mut(&connector_id) {
            if outcome.success {
                connector.health = "healthy".to_string();
                connector.circuit_state = "closed".to_string();
                connector.circuit_open_until = None;
                connector.consecutive_failures = 0;
                connector.sent_total = connector.sent_total.saturating_add(1);
                connector.last_success_at = Some(now);
                connector.last_error = None;
            } else {
                connector.health = "degraded".to_string();
                connector.failed_total = connector.failed_total.saturating_add(1);
                connector.consecutive_failures = connector.consecutive_failures.saturating_add(1);
                connector.last_failure_at = Some(now);
                connector.last_error = outcome.error.clone();
                if connector.consecutive_failures >= connector.circuit_failure_threshold {
                    connector.circuit_state = "open".to_string();
                    connector.circuit_open_until = Some(now + Duration::seconds(connector.circuit_open_secs as i64));
                }
            }
            connector.updated_at = now;
        }

        let delivery = inner.state.deliveries[index].clone();
        let public_base_url = inner.config.server.public_base_url.clone();
        for job in &mut inner.state.tts_jobs {
            if job.synthesis_delivery_id == delivery_id {
                if outcome.success {
                    job.state = "ready".to_string();
                    job.artifact_path = outcome.artifact_path.clone();
                    job.artifact_sha256 = outcome.artifact_sha256.clone();
                    job.artifact_size_bytes = outcome.artifact_size_bytes;
                    job.artifact_url = Some(format!(
                        "{}/api/v1/tts/jobs/{}/artifact",
                        public_base_url, job.job_id
                    ));
                    job.completed_at = Some(now);
                    job.last_error = None;
                } else if delivery.state == "dead_letter" {
                    job.state = "failed".to_string();
                    job.last_error = outcome.error.clone();
                } else {
                    job.state = "retry".to_string();
                    job.last_error = outcome.error.clone();
                }
                job.updated_at = now;
            }
            if job.publish_delivery_id.as_deref() == Some(delivery_id) {
                if outcome.success {
                    job.state = "published".to_string();
                    job.completed_at = Some(now);
                    job.last_error = None;
                } else if delivery.state == "dead_letter" {
                    job.state = "publish_failed".to_string();
                    job.last_error = outcome.error.clone();
                } else {
                    job.state = "publish_retry".to_string();
                    job.last_error = outcome.error.clone();
                }
                job.updated_at = now;
            }
        }
        audit_locked(
            &mut inner,
            "worker",
            "delivery",
            if outcome.success { "delivered" } else { "failed" },
            "delivery",
            delivery_id,
            if outcome.success { "ok" } else { "error" },
            json!({"connector_id": connector_id, "status": outcome.status, "error": outcome.error}),
        );
        trim_locked(&mut inner);
        persist_locked(&mut inner).map_err(|error| error.to_string())?;
        Ok(delivery)
    }

    pub fn connectors_due_probe(&self) -> Vec<ConnectorRecord> {
        let inner = lock(&self.inner);
        let now = Utc::now();
        let interval = Duration::seconds(inner.config.runtime.probe_interval_secs as i64);
        inner
            .state
            .connectors
            .values()
            .filter(|connector| connector.enabled && connector.health_endpoint.is_some())
            .filter(|connector| connector.last_probe_at.is_none_or(|last| now - last >= interval))
            .cloned()
            .collect()
    }

    pub fn connector_for_probe(&self, connector_id: &str) -> Option<ConnectorRecord> {
        let inner = lock(&self.inner);
        inner.state.connectors.get(&slug(connector_id)).cloned()
    }

    pub fn record_probe(&self, outcome: ConnectorProbeOutcome) -> Result<Value, String> {
        let mut inner = lock(&self.inner);
        let now = Utc::now();
        let connector_snapshot = {
            let connector = inner
                .state
                .connectors
                .get_mut(&outcome.connector_id)
                .ok_or_else(|| format!("connector {} not found", outcome.connector_id))?;
            connector.last_probe_at = Some(now);
            if outcome.success {
                connector.health = "healthy".to_string();
                connector.last_success_at = Some(now);
                if connector.circuit_state != "open" {
                    connector.last_error = None;
                }
            } else {
                connector.health = "degraded".to_string();
                connector.last_failure_at = Some(now);
                connector.last_error = outcome.error.clone();
            }
            connector.updated_at = now;
            connector.clone()
        };
        let view = connector_view(&inner, &connector_snapshot);
        persist_locked(&mut inner).map_err(|error| error.to_string())?;
        Ok(json!({
            "connector": view,
            "success": outcome.success,
            "status": outcome.status,
            "response_ms": outcome.response_ms,
            "error": outcome.error,
        }))
    }

    pub fn maintenance(&self, actor_name: Option<String>) -> Result<Value, String> {
        let mut inner = lock(&self.inner);
        let now = Utc::now();
        let before_events = inner.state.events.len();
        let before_deliveries = inner.state.deliveries.len();
        let before_audit = inner.state.audit.len();
        maintenance_locked(&mut inner, now);
        audit_locked(
            &mut inner,
            actor(actor_name.as_deref()),
            "maintenance",
            "tick",
            "gateway",
            "application-gateway",
            "ok",
            json!({}),
        );
        persist_locked(&mut inner).map_err(|error| error.to_string())?;
        Ok(json!({
            "events_removed": before_events.saturating_sub(inner.state.events.len()),
            "deliveries_removed": before_deliveries.saturating_sub(inner.state.deliveries.len()),
            "audit_removed": before_audit.saturating_sub(inner.state.audit.len()),
            "status": status_locked(&inner),
        }))
    }

    pub fn backup(&self, input: BackupInput) -> Result<BackupRecord, String> {
        let mut inner = lock(&self.inner);
        let now = Utc::now();
        let backup_id = format!("{}-{}", now.format("%Y%m%dT%H%M%SZ"), &Uuid::new_v4().to_string()[..8]);
        let directory = inner.config.storage.backup_dir.join(&backup_id);
        fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        let state_bytes = serde_json::to_vec_pretty(&inner.state).map_err(|error| error.to_string())?;
        let state_path = directory.join("state.json");
        atomic_write(&state_path, &state_bytes).map_err(|error| error.to_string())?;
        let state_sha256 = sha256_hex(&state_bytes);
        let manifest = json!({
            "backup_id": backup_id,
            "created_at": now,
            "created_by": actor(input.actor.as_deref()),
            "note": input.note,
            "state_sha256": state_sha256,
            "includes_secrets": false,
            "warning": "Connector secrets are deliberately excluded. Back up secrets.json separately under an approved secret-management procedure.",
        });
        atomic_write(
            &directory.join("manifest.json"),
            &serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        let record = BackupRecord {
            backup_id: backup_id.clone(),
            path: directory,
            state_sha256,
            created_at: now,
            created_by: actor(input.actor.as_deref()).to_string(),
            note: input.note,
            includes_secrets: false,
        };
        inner.state.backups.push(record.clone());
        audit_locked(
            &mut inner,
            &record.created_by,
            "backup",
            "create",
            "backup",
            &backup_id,
            "ok",
            json!({"includes_secrets": false}),
        );
        persist_locked(&mut inner).map_err(|error| error.to_string())?;
        Ok(record)
    }

    pub fn backups(&self) -> Vec<BackupRecord> {
        let inner = lock(&self.inner);
        inner.state.backups.iter().rev().cloned().collect()
    }

    pub fn audit(&self, limit: usize) -> Vec<AuditRecord> {
        let inner = lock(&self.inner);
        inner.state.audit.iter().rev().take(limit).cloned().collect()
    }

    pub fn export(&self) -> Value {
        let inner = lock(&self.inner);
        json!({
            "schema": "netcore-application-gateway-export-v1",
            "generated_at": Utc::now(),
            "status": status_locked(&inner),
            "connectors": inner.state.connectors.values().map(|connector| connector_view(&inner, connector)).collect::<Vec<_>>(),
            "rules": inner.state.rules,
            "templates": inner.state.templates,
            "events": inner.state.events,
            "deliveries": inner.state.deliveries,
            "tts_jobs": inner.state.tts_jobs,
            "audit": inner.state.audit,
            "backups": inner.state.backups,
            "secrets": secret_statuses_locked(&inner, None),
            "warning": "Secret values are never included in exports",
        })
    }

    pub fn metrics(&self) -> String {
        let inner = lock(&self.inner);
        let status = status_locked(&inner);
        let mut out = String::new();
        metric(&mut out, "netcore_application_gateway_ready", if status.ready { 1 } else { 0 });
        metric(&mut out, "netcore_application_gateway_connectors_total", status.connectors_total as u64);
        metric(&mut out, "netcore_application_gateway_connectors_enabled", status.connectors_enabled as u64);
        metric(&mut out, "netcore_application_gateway_connectors_healthy", status.connectors_healthy as u64);
        metric(&mut out, "netcore_application_gateway_circuits_open", status.circuits_open as u64);
        metric(&mut out, "netcore_application_gateway_events_total", status.events_total as u64);
        metric(&mut out, "netcore_application_gateway_events_unrouted", status.events_unrouted as u64);
        metric(&mut out, "netcore_application_gateway_deliveries_queued", status.deliveries_queued as u64);
        metric(&mut out, "netcore_application_gateway_deliveries_retry", status.deliveries_retry as u64);
        metric(&mut out, "netcore_application_gateway_deliveries_delivered", status.deliveries_delivered as u64);
        metric(&mut out, "netcore_application_gateway_deliveries_shadowed", status.deliveries_shadowed as u64);
        metric(&mut out, "netcore_application_gateway_deliveries_dead_letter", status.deliveries_dead_letter as u64);
        metric(&mut out, "netcore_application_gateway_tts_jobs_total", status.tts_jobs_total as u64);
        metric(&mut out, "netcore_application_gateway_tts_jobs_ready", status.tts_jobs_ready as u64);
        metric(&mut out, "netcore_application_gateway_missing_required_secrets", status.missing_required_secrets as u64);
        for connector in inner.state.connectors.values() {
            let labels = format!(
                "connector=\"{}\",kind=\"{}\"",
                prom_escape(&connector.connector_id),
                prom_escape(&connector.kind)
            );
            labelled_metric(&mut out, "netcore_application_gateway_connector_enabled", &labels, if connector.enabled { 1 } else { 0 });
            labelled_metric(&mut out, "netcore_application_gateway_connector_healthy", &labels, if connector.health == "healthy" { 1 } else { 0 });
            labelled_metric(&mut out, "netcore_application_gateway_connector_sent_total", &labels, connector.sent_total);
            labelled_metric(&mut out, "netcore_application_gateway_connector_failed_total", &labels, connector.failed_total);
            labelled_metric(&mut out, "netcore_application_gateway_connector_received_total", &labels, connector.received_total);
        }
        out
    }

    fn persist_all(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut inner = lock(&self.inner);
        persist_locked(&mut inner)?;
        persist_secrets_locked(&inner)?;
        Ok(())
    }
}

fn empty_state(now: DateTime<Utc>) -> PersistedGateway {
    PersistedGateway {
        schema_version: 1,
        connectors: BTreeMap::new(),
        rules: BTreeMap::new(),
        templates: BTreeMap::new(),
        events: Vec::new(),
        deliveries: Vec::new(),
        tts_jobs: Vec::new(),
        audit: Vec::new(),
        backups: Vec::new(),
        audit_sequence: 0,
        started_at: now,
        updated_at: now,
    }
}

fn seed_state(config: &ApplicationGatewayConfig, state: &mut PersistedGateway, now: DateTime<Utc>) -> Result<(), String> {
    for seed in &config.connectors {
        state
            .connectors
            .entry(seed.connector_id.clone())
            .or_insert_with(|| connector_from_seed(seed, now));
    }
    for seed in &config.rules {
        state
            .rules
            .entry(seed.rule_id.clone())
            .or_insert_with(|| rule_from_seed(seed, now));
    }
    for seed in &config.templates {
        state
            .templates
            .entry(seed.template_id.clone())
            .or_insert_with(|| template_from_seed(seed, now));
    }
    for rule in state.rules.values() {
        if !state.connectors.contains_key(&rule.target_connector) {
            return Err(format!("persisted rule {} references missing connector {}", rule.rule_id, rule.target_connector));
        }
    }
    state.updated_at = now;
    Ok(())
}

fn connector_from_seed(seed: &ConnectorSeed, now: DateTime<Utc>) -> ConnectorRecord {
    ConnectorRecord {
        connector_id: seed.connector_id.clone(),
        display_name: seed.display_name.clone(),
        kind: seed.kind.clone(),
        direction: seed.direction.clone(),
        endpoint: seed.endpoint.clone(),
        health_endpoint: seed.health_endpoint.clone(),
        enabled: seed.enabled,
        timeout_ms: seed.timeout_ms,
        rate_limit_per_minute: seed.rate_limit_per_minute,
        circuit_failure_threshold: seed.circuit_failure_threshold,
        circuit_open_secs: seed.circuit_open_secs,
        required_secrets: seed.required_secrets.clone(),
        settings: seed.settings.clone(),
        health: if seed.enabled { "unknown" } else { "disabled" }.to_string(),
        circuit_state: "closed".to_string(),
        circuit_open_until: None,
        consecutive_failures: 0,
        sent_total: 0,
        failed_total: 0,
        received_total: 0,
        rate_window_started_at: now,
        rate_window_count: 0,
        last_probe_at: None,
        last_success_at: None,
        last_failure_at: None,
        last_error: None,
        created_at: now,
        updated_at: now,
    }
}

fn rule_from_seed(seed: &RouteRuleSeed, now: DateTime<Utc>) -> RouteRuleRecord {
    RouteRuleRecord {
        rule_id: seed.rule_id.clone(),
        name: seed.name.clone(),
        enabled: seed.enabled,
        priority: seed.priority,
        source_connector: seed.source_connector.clone(),
        event_type: seed.event_type.clone(),
        text_contains: seed.text_contains.clone(),
        target_connector: seed.target_connector.clone(),
        template_id: seed.template_id.clone(),
        destination: seed.destination.clone(),
        stop_processing: seed.stop_processing,
        matched_total: 0,
        created_at: now,
        updated_at: now,
    }
}

fn template_from_seed(seed: &TemplateSeed, now: DateTime<Utc>) -> TemplateRecord {
    TemplateRecord {
        template_id: seed.template_id.clone(),
        name: seed.name.clone(),
        kind: seed.kind.clone(),
        body: seed.body.clone(),
        content_type: seed.content_type.clone(),
        enabled: seed.enabled,
        target_connector: seed.target_connector.clone(),
        default_destination: seed.default_destination.clone(),
        description: seed.description.clone(),
        render_total: 0,
        created_at: now,
        updated_at: now,
    }
}

fn recover_inflight(state: &mut PersistedGateway, now: DateTime<Utc>) {
    for delivery in &mut state.deliveries {
        if delivery.state == "in_flight" {
            delivery.state = "retry".to_string();
            delivery.next_attempt_at = now;
            delivery.last_error = Some("recovered after application-gateway restart".to_string());
            delivery.updated_at = now;
        }
    }
    for job in &mut state.tts_jobs {
        if job.state == "synthesizing" {
            job.state = "retry".to_string();
            job.last_error = Some("recovered after application-gateway restart".to_string());
            job.updated_at = now;
        }
    }
}

fn dispatch_locked(inner: &mut GatewayInner, input: DispatchInput) -> Result<EventRecord, String> {
    if input.event_type.trim().is_empty() {
        return Err("event_type is required".into());
    }
    let source = slug(input.source_connector.as_deref().unwrap_or("manual"));
    let event_type = input.event_type.trim().to_ascii_lowercase();
    let now = Utc::now();
    if let Some(key) = input.idempotency_key.as_ref() {
        if let Some(existing) = inner.state.events.iter().rev().find(|event| {
            event.source_connector == source
                && event.idempotency_key.as_deref() == Some(key.as_str())
                && now - event.received_at <= Duration::seconds(inner.config.runtime.dedupe_window_secs as i64)
        }) {
            return Ok(existing.clone());
        }
    }
    let event_id = Uuid::new_v4().to_string();
    let correlation_id = input.correlation_id.clone().unwrap_or_else(|| event_id.clone());
    let expires_at = now + Duration::seconds(input.ttl_secs.unwrap_or(inner.config.runtime.default_ttl_secs) as i64);
    let mut event = EventRecord {
        event_id: event_id.clone(),
        source_connector: source.clone(),
        event_type: event_type.clone(),
        destination: clean_optional(input.destination.clone()),
        text: clean_optional(input.text.clone()),
        payload: input.payload.clone(),
        idempotency_key: clean_optional(input.idempotency_key.clone()),
        correlation_id,
        priority: input.priority.unwrap_or(0),
        state: "received".to_string(),
        matched_rules: Vec::new(),
        delivery_ids: Vec::new(),
        received_at: now,
        updated_at: now,
        expires_at,
    };

    if !input.target_connectors.is_empty() {
        for connector_id in input.target_connectors.iter().map(|value| slug(value)) {
            create_delivery_locked(
                inner,
                &mut event,
                &connector_id,
                input.template_id.as_deref(),
                event.destination.clone(),
                now,
            )?;
        }
    } else {
        let mut rule_ids: Vec<String> = inner.state.rules.keys().cloned().collect();
        rule_ids.sort_by(|left, right| {
            inner.state.rules[right]
                .priority
                .cmp(&inner.state.rules[left].priority)
                .then(left.cmp(right))
        });
        for rule_id in rule_ids {
            let Some(rule) = inner.state.rules.get(&rule_id).cloned() else {
                continue;
            };
            if !rule.enabled || !rule_matches(&rule, &event) {
                continue;
            }
            if let Some(record) = inner.state.rules.get_mut(&rule_id) {
                record.matched_total = record.matched_total.saturating_add(1);
                record.updated_at = now;
            }
            event.matched_rules.push(rule_id);
            create_delivery_locked(
                inner,
                &mut event,
                &rule.target_connector,
                input.template_id.as_deref().or(rule.template_id.as_deref()),
                rule.destination.clone().or(event.destination.clone()),
                now,
            )?;
            if rule.stop_processing {
                break;
            }
        }
    }
    event.state = if event.delivery_ids.is_empty() { "unrouted" } else { "routed" }.to_string();
    event.updated_at = now;
    inner.state.events.push(event.clone());
    audit_locked(
        inner,
        actor(input.actor.as_deref()),
        "event",
        "ingest",
        "event",
        &event_id,
        "ok",
        json!({"source": source, "event_type": event_type, "deliveries": event.delivery_ids.len()}),
    );
    trim_locked(inner);
    Ok(event)
}

fn create_delivery_locked(
    inner: &mut GatewayInner,
    event: &mut EventRecord,
    connector_id: &str,
    template_id: Option<&str>,
    destination: Option<String>,
    now: DateTime<Utc>,
) -> Result<(), String> {
    let connector_id = slug(connector_id);
    if !inner.state.connectors.contains_key(&connector_id) {
        return Err(format!("connector {connector_id} not found"));
    }
    let context = RenderContext {
        source: event.source_connector.clone(),
        event_type: event.event_type.clone(),
        destination: destination.clone(),
        text: event.text.clone(),
        payload: event.payload.clone(),
    };
    let (payload, content_type, chosen_template) = if let Some(template_id) = template_id {
        let template_id = slug(template_id);
        let template = inner
            .state
            .templates
            .get_mut(&template_id)
            .ok_or_else(|| format!("template {template_id} not found"))?;
        if !template.enabled {
            return Err(format!("template {template_id} is disabled"));
        }
        let value = render_template_value(template, &context)?;
        template.render_total = template.render_total.saturating_add(1);
        template.updated_at = now;
        (value, template.content_type.clone(), Some(template_id))
    } else {
        let payload = if event.payload.is_null() {
            json!({"text": event.text})
        } else {
            event.payload.clone()
        };
        (payload, "application/json".to_string(), None)
    };
    let delivery_id = Uuid::new_v4().to_string();
    inner.state.deliveries.push(DeliveryRecord {
        delivery_id: delivery_id.clone(),
        event_id: event.event_id.clone(),
        connector_id,
        template_id: chosen_template,
        event_type: event.event_type.clone(),
        destination,
        text: event.text.clone(),
        payload,
        content_type,
        correlation_id: event.correlation_id.clone(),
        priority: event.priority,
        state: "queued".to_string(),
        attempts: 0,
        max_attempts: inner.config.runtime.max_attempts,
        next_attempt_at: now,
        expires_at: event.expires_at,
        last_attempt_at: None,
        delivered_at: None,
        response_status: None,
        response_excerpt: None,
        last_error: None,
        artifact_path: None,
        artifact_sha256: None,
        artifact_size_bytes: None,
        created_at: now,
        updated_at: now,
    });
    event.delivery_ids.push(delivery_id);
    Ok(())
}

fn rule_matches(rule: &RouteRuleRecord, event: &EventRecord) -> bool {
    let source_match = rule.source_connector == "*" || rule.source_connector == event.source_connector;
    let type_match = rule.event_type == "*" || rule.event_type == event.event_type;
    let text_match = rule.text_contains.as_ref().is_none_or(|needle| {
        event
            .text
            .as_ref()
            .is_some_and(|text| text.to_ascii_lowercase().contains(&needle.to_ascii_lowercase()))
    });
    source_match && type_match && text_match
}

#[derive(Debug, Clone)]
struct RenderContext {
    source: String,
    event_type: String,
    destination: Option<String>,
    text: Option<String>,
    payload: Value,
}

fn render_template_value(template: &TemplateRecord, context: &RenderContext) -> Result<Value, String> {
    let rendered = render_string(&template.body, context, template.kind == "json");
    if template.kind == "json" {
        serde_json::from_str(&rendered).map_err(|error| format!("rendered template is invalid JSON: {error}"))
    } else {
        Ok(json!({"text": rendered}))
    }
}

fn render_string(body: &str, context: &RenderContext, json_mode: bool) -> String {
    let value = |raw: &str| {
        if json_mode {
            json_fragment(raw)
        } else {
            raw.to_string()
        }
    };
    let mut rendered = body
        .replace("{{source}}", &value(&context.source))
        .replace("{{event_type}}", &value(&context.event_type))
        .replace("{{destination}}", &value(context.destination.as_deref().unwrap_or("")))
        .replace("{{text}}", &value(context.text.as_deref().unwrap_or("")));
    if let Some(object) = context.payload.as_object() {
        for (key, item) in object {
            let raw = match item {
                Value::String(value) => value.clone(),
                Value::Null => String::new(),
                other => other.to_string(),
            };
            rendered = rendered.replace(&format!("{{{{payload.{key}}}}}"), &value(&raw));
        }
    }
    rendered
}

fn json_fragment(value: &str) -> String {
    serde_json::to_string(value)
        .unwrap_or_else(|_| "\"\"".to_string())
        .trim_matches('"')
        .to_string()
}

fn value_text(value: Value) -> String {
    value
        .get("text")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .unwrap_or_else(|| value.to_string())
}

fn validate_connector_input(input: &ConnectorInput) -> Result<(), String> {
    if input.kind.trim().is_empty() {
        return Err("connector kind is required".into());
    }
    if !matches!(input.direction.trim(), "inbound" | "outbound" | "bidirectional") {
        return Err("direction must be inbound, outbound or bidirectional".into());
    }
    if input.endpoint.trim().is_empty() && input.direction.trim() != "inbound" {
        return Err("outbound connectors require an endpoint".into());
    }
    if !input.endpoint.trim().is_empty()
        && !(input.endpoint.starts_with("http://") || input.endpoint.starts_with("https://"))
    {
        return Err("connector endpoint must use http:// or https://".into());
    }
    Ok(())
}

fn validate_rule_input(inner: &GatewayInner, input: &RouteRuleInput) -> Result<(), String> {
    let connector_id = slug(&input.target_connector);
    if !inner.state.connectors.contains_key(&connector_id) {
        return Err(format!("target connector {connector_id} not found"));
    }
    if let Some(template_id) = &input.template_id
        && !inner.state.templates.contains_key(&slug(template_id))
    {
        return Err(format!("template {} not found", slug(template_id)));
    }
    if input.event_type.trim().is_empty() || input.source_connector.trim().is_empty() {
        return Err("source_connector and event_type are required".into());
    }
    Ok(())
}

fn rule_from_input(id: String, input: RouteRuleInput, created_at: DateTime<Utc>, matched_total: u64) -> RouteRuleRecord {
    RouteRuleRecord {
        rule_id: id,
        name: non_empty(&input.name, "Routing rule"),
        enabled: input.enabled.unwrap_or(true),
        priority: input.priority.unwrap_or(0),
        source_connector: input.source_connector.trim().to_ascii_lowercase(),
        event_type: input.event_type.trim().to_ascii_lowercase(),
        text_contains: clean_optional(input.text_contains),
        target_connector: slug(&input.target_connector),
        template_id: input.template_id.map(|value| slug(&value)),
        destination: clean_optional(input.destination),
        stop_processing: input.stop_processing.unwrap_or(false),
        matched_total,
        created_at,
        updated_at: Utc::now(),
    }
}

fn validate_template_input(inner: &GatewayInner, input: &TemplateInput) -> Result<(), String> {
    if !matches!(input.kind.trim(), "text" | "json" | "tts") {
        return Err("template kind must be text, json or tts".into());
    }
    if input.body.is_empty() {
        return Err("template body is required".into());
    }
    if let Some(connector_id) = &input.target_connector
        && !inner.state.connectors.contains_key(&slug(connector_id))
    {
        return Err(format!("connector {} not found", slug(connector_id)));
    }
    Ok(())
}

fn template_from_input(id: String, input: TemplateInput, created_at: DateTime<Utc>, render_total: u64) -> TemplateRecord {
    let kind = input.kind.trim().to_ascii_lowercase();
    TemplateRecord {
        template_id: id,
        name: non_empty(&input.name, "Template"),
        kind: kind.clone(),
        body: input.body,
        content_type: input.content_type.unwrap_or_else(|| {
            if kind == "json" {
                "application/json".to_string()
            } else {
                "text/plain; charset=utf-8".to_string()
            }
        }),
        enabled: input.enabled.unwrap_or(true),
        target_connector: input.target_connector.map(|value| slug(&value)),
        default_destination: clean_optional(input.default_destination),
        description: input.description.unwrap_or_default(),
        render_total,
        created_at,
        updated_at: Utc::now(),
    }
}

fn status_locked(inner: &GatewayInner) -> GatewayStatus {
    let connectors_total = inner.state.connectors.len();
    let connectors_enabled = inner.state.connectors.values().filter(|connector| connector.enabled).count();
    let connectors_healthy = inner.state.connectors.values().filter(|connector| connector.health == "healthy").count();
    let connectors_degraded = inner
        .state
        .connectors
        .values()
        .filter(|connector| matches!(connector.health.as_str(), "degraded" | "down"))
        .count();
    let circuits_open = inner.state.connectors.values().filter(|connector| connector.circuit_state == "open").count();
    let missing_required_secrets = inner
        .state
        .connectors
        .values()
        .filter(|connector| connector.enabled)
        .map(|connector| {
            connector
                .required_secrets
                .iter()
                .filter(|name| {
                    inner
                        .secrets
                        .connectors
                        .get(&connector.connector_id)
                        .and_then(|values| values.get(*name))
                        .is_none()
                })
                .count()
        })
        .sum();
    GatewayStatus {
        ready: inner.config.storage.state_path.parent().is_some_and(Path::is_dir)
            && inner.config.storage.spool_dir.is_dir(),
        security_mode: inner.config.security.mode.clone(),
        operating_mode: inner.config.runtime.operating_mode.clone(),
        management_token_auth: inner.config.security.management_token_auth,
        management_tls: inner.config.security.management_tls,
        connectors_total,
        connectors_enabled,
        connectors_healthy,
        connectors_degraded,
        circuits_open,
        events_total: inner.state.events.len(),
        events_unrouted: inner.state.events.iter().filter(|event| event.state == "unrouted").count(),
        deliveries_queued: inner.state.deliveries.iter().filter(|delivery| delivery.state == "queued").count(),
        deliveries_retry: inner.state.deliveries.iter().filter(|delivery| delivery.state == "retry").count(),
        deliveries_delivered: inner.state.deliveries.iter().filter(|delivery| delivery.state == "delivered").count(),
        deliveries_shadowed: inner.state.deliveries.iter().filter(|delivery| delivery.state == "shadowed").count(),
        deliveries_dead_letter: inner.state.deliveries.iter().filter(|delivery| delivery.state == "dead_letter").count(),
        tts_jobs_total: inner.state.tts_jobs.len(),
        tts_jobs_ready: inner.state.tts_jobs.iter().filter(|job| job.state == "ready").count(),
        missing_required_secrets,
        state_path: inner.config.storage.state_path.display().to_string(),
        secrets_path: inner.config.storage.secrets_path.display().to_string(),
        spool_dir: inner.config.storage.spool_dir.display().to_string(),
        started_at: inner.state.started_at,
        updated_at: inner.state.updated_at,
    }
}

fn connector_view(inner: &GatewayInner, connector: &ConnectorRecord) -> Value {
    let statuses: Vec<_> = connector
        .required_secrets
        .iter()
        .map(|name| {
            let entry = inner
                .secrets
                .connectors
                .get(&connector.connector_id)
                .and_then(|values| values.get(name));
            json!({
                "name": name,
                "present": entry.is_some(),
                "fingerprint": entry.map(|entry| entry.fingerprint.clone()),
                "updated_at": entry.map(|entry| entry.updated_at),
            })
        })
        .collect();
    json!({
        "connector_id": connector.connector_id,
        "display_name": connector.display_name,
        "kind": connector.kind,
        "direction": connector.direction,
        "endpoint": connector.endpoint,
        "health_endpoint": connector.health_endpoint,
        "enabled": connector.enabled,
        "timeout_ms": connector.timeout_ms,
        "rate_limit_per_minute": connector.rate_limit_per_minute,
        "circuit_failure_threshold": connector.circuit_failure_threshold,
        "circuit_open_secs": connector.circuit_open_secs,
        "settings": connector.settings,
        "required_secrets": statuses,
        "health": connector.health,
        "circuit_state": connector.circuit_state,
        "circuit_open_until": connector.circuit_open_until,
        "consecutive_failures": connector.consecutive_failures,
        "sent_total": connector.sent_total,
        "failed_total": connector.failed_total,
        "received_total": connector.received_total,
        "last_probe_at": connector.last_probe_at,
        "last_success_at": connector.last_success_at,
        "last_failure_at": connector.last_failure_at,
        "last_error": connector.last_error,
        "created_at": connector.created_at,
        "updated_at": connector.updated_at,
    })
}

fn secret_statuses_locked(inner: &GatewayInner, connector_id: Option<&str>) -> Vec<SecretStatus> {
    let requested = connector_id.map(slug);
    let mut rows = Vec::new();
    for connector in inner.state.connectors.values() {
        if requested.as_ref().is_some_and(|value| value != &connector.connector_id) {
            continue;
        }
        let required: HashSet<&str> = connector.required_secrets.iter().map(String::as_str).collect();
        let stored = inner.secrets.connectors.get(&connector.connector_id);
        let mut names: HashSet<String> = connector.required_secrets.iter().cloned().collect();
        if let Some(stored) = stored {
            names.extend(stored.keys().cloned());
        }
        for name in names {
            let entry = stored.and_then(|values| values.get(&name));
            rows.push(SecretStatus {
                connector_id: connector.connector_id.clone(),
                name: name.clone(),
                present: entry.is_some(),
                fingerprint: entry.map(|entry| entry.fingerprint.clone()),
                updated_at: entry.map(|entry| entry.updated_at),
                required: required.contains(name.as_str()),
            });
        }
    }
    rows.sort_by(|left, right| {
        left.connector_id
            .cmp(&right.connector_id)
            .then(left.name.cmp(&right.name))
    });
    rows
}

fn required_secrets_present(inner: &GatewayInner, connector: &ConnectorRecord) -> bool {
    connector.required_secrets.iter().all(|name| {
        inner
            .secrets
            .connectors
            .get(&connector.connector_id)
            .and_then(|values| values.get(name))
            .is_some()
    })
}

fn maintenance_locked(inner: &mut GatewayInner, now: DateTime<Utc>) {
    for connector in inner.state.connectors.values_mut() {
        if connector.circuit_state == "open"
            && connector.circuit_open_until.is_some_and(|until| until <= now)
        {
            connector.circuit_state = "half_open".to_string();
            connector.circuit_open_until = None;
            connector.updated_at = now;
        }
    }
    for delivery in &mut inner.state.deliveries {
        if matches!(delivery.state.as_str(), "queued" | "retry" | "in_flight") && delivery.expires_at <= now {
            delivery.state = "dead_letter".to_string();
            delivery.last_error = Some("delivery TTL expired".to_string());
            delivery.updated_at = now;
        }
    }
    let event_cutoff = now - Duration::seconds(inner.config.runtime.event_retention_secs as i64);
    let delivery_cutoff = now - Duration::seconds(inner.config.runtime.delivery_retention_secs as i64);
    let audit_cutoff = now - Duration::seconds(inner.config.runtime.audit_retention_secs as i64);
    inner.state.events.retain(|event| event.received_at >= event_cutoff || event.state == "routed");
    inner.state.deliveries.retain(|delivery| {
        delivery.created_at >= delivery_cutoff
            || matches!(delivery.state.as_str(), "queued" | "retry" | "in_flight" | "dead_letter")
    });
    inner.state.audit.retain(|record| record.timestamp >= audit_cutoff);
    trim_locked(inner);
    inner.state.updated_at = now;
}

fn trim_locked(inner: &mut GatewayInner) {
    trim_front(&mut inner.state.events, inner.config.runtime.max_events);
    trim_front(&mut inner.state.deliveries, inner.config.runtime.max_deliveries);
    trim_front(&mut inner.state.tts_jobs, inner.config.runtime.max_tts_jobs);
    trim_front(&mut inner.state.audit, inner.config.runtime.max_audit_records);
    inner.state.updated_at = Utc::now();
}

fn trim_front<T>(rows: &mut Vec<T>, limit: usize) {
    if rows.len() > limit {
        rows.drain(0..rows.len().saturating_sub(limit));
    }
}

fn audit_locked(
    inner: &mut GatewayInner,
    actor_name: &str,
    category: &str,
    action: &str,
    object_type: &str,
    object_id: &str,
    result: &str,
    detail: Value,
) {
    inner.state.audit_sequence = inner.state.audit_sequence.saturating_add(1);
    inner.state.audit.push(AuditRecord {
        sequence: inner.state.audit_sequence,
        timestamp: Utc::now(),
        actor: actor_name.to_string(),
        category: category.to_string(),
        action: action.to_string(),
        object_type: object_type.to_string(),
        object_id: object_id.to_string(),
        result: result.to_string(),
        detail,
    });
}

fn persist_locked(inner: &mut GatewayInner) -> Result<(), Box<dyn std::error::Error>> {
    inner.state.updated_at = Utc::now();
    let bytes = serde_json::to_vec_pretty(&inner.state)?;
    if inner.config.storage.state_path.is_file() {
        let _ = fs::copy(&inner.config.storage.state_path, &inner.config.storage.state_backup_path);
    }
    atomic_write(&inner.config.storage.state_path, &bytes)?;
    Ok(())
}

fn persist_secrets_locked(inner: &GatewayInner) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = serde_json::to_vec_pretty(&inner.secrets)?;
    atomic_write(&inner.config.storage.secrets_path, &bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&inner.config.storage.secrets_path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    ensure_parent(path)?;
    let temp = path.with_extension(format!("tmp-{}", Uuid::new_v4()));
    let mut file = fs::File::create(&temp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(temp, path)?;
    Ok(())
}

fn ensure_parent(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn metric(out: &mut String, name: &str, value: impl std::fmt::Display) {
    out.push_str(name);
    out.push(' ');
    out.push_str(&value.to_string());
    out.push('\n');
}

fn labelled_metric(out: &mut String, name: &str, labels: &str, value: impl std::fmt::Display) {
    out.push_str(name);
    out.push('{');
    out.push_str(labels);
    out.push_str("} ");
    out.push_str(&value.to_string());
    out.push('\n');
}

fn prom_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n")
}

fn actor(value: Option<&str>) -> &str {
    value.filter(|value| !value.trim().is_empty()).unwrap_or("webui")
}

fn non_empty(value: &str, fallback: &str) -> String {
    let value = value.trim();
    if value.is_empty() { fallback.to_string() } else { value.to_string() }
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value.map(|value| value.trim().to_string()).filter(|value| !value.is_empty())
}

fn safe_filename(value: &str) -> String {
    let value = value
        .chars()
        .map(|character| if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') { character } else { '_' })
        .collect::<String>();
    if value.is_empty() { "tts".to_string() } else { value }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

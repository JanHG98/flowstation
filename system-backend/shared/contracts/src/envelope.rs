use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ApiVersion(String);

impl ApiVersion {
    pub const V1: &'static str = "netcore.v1";

    pub fn v1() -> Self {
        Self(Self::V1.to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for ApiVersion {
    fn default() -> Self {
        Self::v1()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageKind {
    Command,
    Event,
    Query,
    Reply,
    Snapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliverySemantics {
    AtMostOnce,
    AtLeastOnce,
    IdempotentAtLeastOnce,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TraceContext {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvelopeMeta {
    pub message_id: Uuid,
    #[serde(default)]
    pub api_version: ApiVersion,
    pub kind: MessageKind,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destination: Option<String>,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub causation_id: Option<Uuid>,
    #[serde(default)]
    pub trace: TraceContext,
    pub delivery: DeliverySemantics,
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

impl EnvelopeMeta {
    pub fn new(kind: MessageKind, source: impl Into<String>) -> Self {
        Self {
            message_id: Uuid::new_v4(),
            api_version: ApiVersion::v1(),
            kind,
            source: source.into(),
            destination: None,
            created_at: Utc::now(),
            expires_at: None,
            correlation_id: None,
            causation_id: None,
            trace: TraceContext::default(),
            delivery: DeliverySemantics::IdempotentAtLeastOnce,
            idempotency_key: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Envelope<T> {
    pub meta: EnvelopeMeta,
    pub payload_type: String,
    pub payload: T,
}

impl<T> Envelope<T> {
    pub fn new(
        kind: MessageKind,
        source: impl Into<String>,
        payload_type: impl Into<String>,
        payload: T,
    ) -> Self {
        Self {
            meta: EnvelopeMeta::new(kind, source),
            payload_type: payload_type.into(),
            payload,
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn envelope_uses_v1_contract_by_default() {
        let envelope = Envelope::new(MessageKind::Event, "group-core", "group.affiliation", json!({"gssi": 2000}));
        assert_eq!(envelope.meta.api_version.as_str(), ApiVersion::V1);
        assert_eq!(envelope.meta.delivery, DeliverySemantics::IdempotentAtLeastOnce);
    }
}

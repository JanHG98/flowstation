use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProblemDetails {
    #[serde(rename = "type")]
    pub problem_type: String,
    pub title: String,
    pub status: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    #[serde(flatten, default)]
    pub extensions: BTreeMap<String, Value>,
}

impl ProblemDetails {
    pub fn new(status: u16, code: impl Into<String>, title: impl Into<String>) -> Self {
        let code = code.into();
        Self {
            problem_type: format!("urn:netcore:problem:{code}"),
            title: title.into(),
            status,
            detail: None,
            instance: None,
            code: Some(code),
            correlation_id: None,
            extensions: BTreeMap::new(),
        }
    }
}

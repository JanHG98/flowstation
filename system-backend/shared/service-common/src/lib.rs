//! Process-level helpers shared by independently deployable NetCore services.

use netcore_contracts::{ApiVersion, BuildInfo, OperatingMode, SecurityMode, ServiceCapability, ServiceDescriptor};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagementPolicy {
    pub security_mode: SecurityMode,
    pub token_auth: bool,
    pub tls: bool,
    pub warning: String,
}

impl ManagementPolicy {
    pub fn open_lab(warning: impl Into<String>) -> Self {
        Self {
            security_mode: SecurityMode::OpenLab,
            token_auth: false,
            tls: false,
            warning: warning.into(),
        }
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.security_mode == SecurityMode::OpenLab && (self.token_auth || self.tls) {
            return Err("open_lab policy must not pretend token or TLS enforcement is active");
        }
        if self.security_mode != SecurityMode::OpenLab && !self.token_auth && !self.tls {
            return Err("non-lab policy requires at least one management protection mechanism");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceIdentity {
    pub name: String,
    pub instance: String,
    pub api_base: String,
    pub operating_mode: OperatingMode,
    pub management: ManagementPolicy,
    pub capabilities: Vec<ServiceCapability>,
}

impl ServiceIdentity {
    pub fn descriptor(&self) -> ServiceDescriptor {
        ServiceDescriptor {
            name: self.name.clone(),
            instance: self.instance.clone(),
            service_version: env!("CARGO_PKG_VERSION").to_owned(),
            contract_version: ApiVersion::V1.to_owned(),
            security_mode: self.management.security_mode,
            operating_mode: self.operating_mode,
            api_base: self.api_base.clone(),
            health_live: "/health/live".to_owned(),
            health_ready: "/health/ready".to_owned(),
            metrics: "/metrics".to_owned(),
            capabilities: self.capabilities.clone(),
        }
    }
}

pub fn build_info(service: impl Into<String>) -> BuildInfo {
    BuildInfo {
        service: service.into(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        git_commit: option_env!("NETCORE_GIT_COMMIT").map(str::to_owned),
        build_timestamp: option_env!("NETCORE_BUILD_TIMESTAMP").map(str::to_owned),
        contract_version: ApiVersion::V1.to_owned(),
    }
}

pub fn request_id(provided: Option<&str>) -> String {
    provided
        .filter(|value| is_safe_request_id(value))
        .map(str::to_owned)
        .unwrap_or_else(|| Uuid::new_v4().to_string())
}

fn is_safe_request_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_fake_open_lab_security_flags() {
        let policy = ManagementPolicy {
            security_mode: SecurityMode::OpenLab,
            token_auth: true,
            tls: false,
            warning: String::new(),
        };
        assert!(policy.validate().is_err());
    }

    #[test]
    fn preserves_safe_request_id_only() {
        assert_eq!(request_id(Some("abc-123")), "abc-123");
        assert_ne!(request_id(Some("bad request id")), "bad request id");
    }
}

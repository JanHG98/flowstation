use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityMode {
    OpenLab,
    Authenticated,
    MutualTls,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperatingMode {
    Shadow,
    Authoritative,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceCapability {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub optional: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceDescriptor {
    pub name: String,
    pub instance: String,
    pub service_version: String,
    pub contract_version: String,
    pub security_mode: SecurityMode,
    pub operating_mode: OperatingMode,
    pub api_base: String,
    pub health_live: String,
    pub health_ready: String,
    pub metrics: String,
    #[serde(default)]
    pub capabilities: Vec<ServiceCapability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Compatibility {
    pub compatible: bool,
    pub local_contract: String,
    pub remote_contract: String,
    #[serde(default)]
    pub missing_required_capabilities: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

impl ServiceDescriptor {
    pub fn compatibility_with(&self, remote: &Self, required: &[&str]) -> Compatibility {
        let local_major = contract_major(&self.contract_version);
        let remote_major = contract_major(&remote.contract_version);
        let missing = required
            .iter()
            .filter(|required_name| {
                !remote.capabilities.iter().any(|capability| capability.name == **required_name)
            })
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>();
        let mut warnings = Vec::new();
        if self.security_mode != remote.security_mode {
            warnings.push("security_mode_mismatch".to_owned());
        }
        Compatibility {
            compatible: local_major.is_some() && local_major == remote_major && missing.is_empty(),
            local_contract: self.contract_version.clone(),
            remote_contract: remote.contract_version.clone(),
            missing_required_capabilities: missing,
            warnings,
        }
    }
}

fn contract_major(value: &str) -> Option<u64> {
    value.strip_prefix("netcore.v")?.split('.').next()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor(version: &str, capabilities: &[&str]) -> ServiceDescriptor {
        ServiceDescriptor {
            name: "test".into(),
            instance: "test-1".into(),
            service_version: "1.3.0".into(),
            contract_version: version.into(),
            security_mode: SecurityMode::OpenLab,
            operating_mode: OperatingMode::Shadow,
            api_base: "/api/v1".into(),
            health_live: "/health/live".into(),
            health_ready: "/health/ready".into(),
            metrics: "/metrics".into(),
            capabilities: capabilities
                .iter()
                .map(|name| ServiceCapability { name: (*name).into(), version: "1".into(), optional: false })
                .collect(),
        }
    }

    #[test]
    fn checks_major_version_and_capabilities() {
        let local = descriptor("netcore.v1", &[]);
        let remote = descriptor("netcore.v1.2", &["calls"]);
        assert!(local.compatibility_with(&remote, &["calls"]).compatible);
        assert!(!local.compatibility_with(&remote, &["sds"]).compatible);
        assert!(!local.compatibility_with(&descriptor("netcore.v2", &["calls"]), &["calls"]).compatible);
    }
}

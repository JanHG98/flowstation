//! Lite stack-health for FlowStation.
//!
//! The serialisable health data types are always available because telemetry
//! protocol consumers need to deserialize `TelemetryEvent::HealthSnapshot`.
//! The live registry/supervisor are base-station runtime concerns and are only
//! compiled with the `runtime` feature.

pub mod types;

#[cfg(feature = "runtime")]
pub mod registry;
#[cfg(feature = "runtime")]
pub mod supervisor;

#[cfg(feature = "runtime")]
pub use registry::{HealthRegistry, HealthThresholds, registry};
#[cfg(feature = "runtime")]
pub use supervisor::{HealthMonitorConfig, spawn_health_monitor};
pub use types::{DomainHealth, HealthDomain, HealthLevel, HealthSnapshot};

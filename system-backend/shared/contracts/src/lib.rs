//! Versioned, transport-neutral contracts shared by NetCore-Tetra backend services.
//!
//! The crate deliberately contains no networking, storage, authentication or service-owned
//! state. It defines stable wire shapes and validated identifiers so backend services do not
//! silently drift into incompatible private JSON dialects.

mod address;
mod envelope;
mod event;
mod health;
mod pagination;
mod problem;
mod service;

pub use address::{AddressError, Gssi, Issi, Ssi};
pub use envelope::{ApiVersion, DeliverySemantics, Envelope, EnvelopeMeta, MessageKind, TraceContext};
pub use event::{AuditRecord, EventRecord, Severity};
pub use health::{BuildInfo, DependencyHealth, HealthDocument, HealthStatus};
pub use pagination::{Page, PageRequest};
pub use problem::ProblemDetails;
pub use service::{Compatibility, OperatingMode, SecurityMode, ServiceCapability, ServiceDescriptor};

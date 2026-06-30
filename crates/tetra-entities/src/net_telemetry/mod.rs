//! External networked telemetry component.
//!
//! `TelemetryEvent` is a protocol type and is available without `runtime` so a
//! Control Room Core can deserialize base-station events without linking RF or
//! audio libraries. Channels/workers/codecs are base-station runtime pieces.

#[cfg(feature = "runtime")]
pub mod channel;
#[cfg(feature = "runtime")]
pub mod codec;
pub mod events;
#[cfg(feature = "runtime")]
pub mod worker;

use std::time::Duration;

#[cfg(feature = "runtime")]
pub use self::channel::{TelemetrySink, TelemetrySource, telemetry_channel};
pub use self::events::{TelemetryEvent, telemetry_source_for_entity};
#[cfg(feature = "runtime")]
pub use self::worker::TelemetryWorker;

/// Sent as subprotocol in WebSocket handshake.
pub const TELEMETRY_PROTOCOL_VERSION: &str = "bluestation-telemetry-v1";
pub const TELEMETRY_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(10);
pub const TELEMETRY_HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(30);

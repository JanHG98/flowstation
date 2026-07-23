//! External networked command component.
//!
//! `ControlCommand` and `ControlResponse` are protocol types and are available
//! without the `runtime` feature.  Channels/workers/codecs are only needed on
//! the base-station side and stay runtime-gated.

#[cfg(feature = "runtime")]
pub mod channel;
#[cfg(feature = "runtime")]
pub mod codec;
pub mod commands;
#[cfg(feature = "runtime")]
pub mod worker;

use std::time::Duration;

#[cfg(feature = "runtime")]
pub use self::channel::{CommandDispatcher, ControlEndpoint, make_control_link};
pub use self::commands::{
    ControlCommand, ControlResponse, ManagedCallKind, ManagedCallRestoreContextPayload,
    ManagedNetworkCircuitCallPayload,
};
#[cfg(feature = "runtime")]
pub use self::worker::ControlWorker;

/// Sent as subprotocol in WebSocket handshake.
pub const CONTROL_PROTOCOL_VERSION: &str = "bluestation-control-v1";
pub const CONTROL_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(10);
pub const CONTROL_HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(30);

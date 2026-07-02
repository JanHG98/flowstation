//! NetCore Control-Room node side / shared protocol.
//!
//! Protocol structs are always available.  The base-station worker is gated
//! behind `runtime` so the Control Room Core can use the protocol without SDR,
//! Brew, dashboard or voice-codec dependencies.

pub mod codec;
pub mod protocol;
#[cfg(feature = "runtime")]
pub mod worker;

use std::time::Duration;

pub use self::codec::{ControlRoomCodecError, ControlRoomCodecJson};
pub use self::protocol::*;
#[cfg(feature = "runtime")]
pub use self::worker::ControlRoomWorker;

/// Sent as WebSocket subprotocol in the node <-> control-room handshake.
pub const CONTROL_ROOM_PROTOCOL_VERSION: &str = "netcore-control-room-node-v1";

/// Node heartbeat cadence.  The WebSocket transport also has its own ping/pong;
/// this application heartbeat is visible to the Leitstelle state model.
pub const CONTROL_ROOM_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);

/// Transport heartbeat timeout.  Keep this a little wider than the heartbeat
/// interval so brief RF/CPU spikes do not flap the Leitstelle connection.
pub const CONTROL_ROOM_HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(30);

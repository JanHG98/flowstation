//! NetCore Control-Room node side.
//!
//! This module is the base-station side of the future Leitstelle/Control-Room
//! connection.  It intentionally lives next to the legacy `net_telemetry` and
//! `net_control` modules instead of replacing them: dashboard, standalone
//! telemetry and old command endpoints can keep working while a new Leitstelle
//! uses one clean bidirectional node protocol.

pub mod codec;
pub mod protocol;
pub mod worker;

use std::time::Duration;

pub use self::codec::ControlRoomCodecJson;
pub use self::protocol::*;
pub use self::worker::ControlRoomWorker;

/// Sent as WebSocket subprotocol in the node <-> control-room handshake.
pub const CONTROL_ROOM_PROTOCOL_VERSION: &str = "netcore-control-room-node-v1";

/// Node heartbeat cadence.  The WebSocket transport also has its own ping/pong;
/// this application heartbeat is visible to the Leitstelle state model.
pub const CONTROL_ROOM_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);

/// Transport heartbeat timeout.  Keep this a little wider than the heartbeat
/// interval so brief RF/CPU spikes do not flap the Leitstelle connection.
pub const CONTROL_ROOM_HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(30);

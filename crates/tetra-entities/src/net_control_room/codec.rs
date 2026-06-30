//! JSON codec for the NetCore Control-Room node protocol.
//!
//! JSON is deliberately used for v1: it is easy to inspect on the wire, easy for
//! a Rust/Python/TypeScript Leitstelle backend to consume, and stable enough for
//! early protocol evolution.  This codec intentionally has no dependency on the
//! runtime transport layer so `netcore-control-room` can build without SDR/audio
//! libraries.

use crate::net_control_room::protocol::{ControlRoomToNodeMessage, NodeToControlRoomMessage};

#[derive(Debug, Clone)]
pub struct ControlRoomCodecError(pub String);

impl std::fmt::Display for ControlRoomCodecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ControlRoomCodecError {}

#[derive(Default)]
pub struct ControlRoomCodecJson;

impl ControlRoomCodecJson {
    pub fn encode_uplink(&self, message: &NodeToControlRoomMessage) -> Vec<u8> {
        serde_json::to_vec(message).unwrap_or_default()
    }

    pub fn decode_uplink(&self, payload: &[u8]) -> Result<NodeToControlRoomMessage, ControlRoomCodecError> {
        serde_json::from_slice(payload).map_err(|e| ControlRoomCodecError(format!("control-room uplink decode: {}", e)))
    }

    pub fn encode_downlink(&self, message: &ControlRoomToNodeMessage) -> Vec<u8> {
        serde_json::to_vec(message).unwrap_or_default()
    }

    pub fn decode_downlink(&self, payload: &[u8]) -> Result<ControlRoomToNodeMessage, ControlRoomCodecError> {
        serde_json::from_slice(payload).map_err(|e| ControlRoomCodecError(format!("control-room downlink decode: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net_control_room::protocol::{ControlRoomNodeHeartbeat, NodeToControlRoomMessage};

    #[test]
    fn json_roundtrip_heartbeat() {
        let codec = ControlRoomCodecJson;
        let msg = NodeToControlRoomMessage::Heartbeat {
            heartbeat: ControlRoomNodeHeartbeat {
                node_id: "tbs-test".to_string(),
                seq: 1,
                timestamp: "2026-06-30T19:00:00Z".to_string(),
                connected: true,
            },
        };
        let bytes = codec.encode_uplink(&msg);
        let decoded = codec.decode_uplink(&bytes).unwrap();
        assert!(matches!(decoded, NodeToControlRoomMessage::Heartbeat { .. }));
    }
}

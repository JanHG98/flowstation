//! Command codec — bitcode-based and JSON-based serialization of
//! [`Command`]s and [`CommandResponse`]s.

use crate::{
    net_control::commands::{ControlCommand, ControlResponse},
    network::transports::NetworkError,
};

// ---------------------------------------------------------------------------
// Codecs
// ---------------------------------------------------------------------------

/// Codec for commands using bitcode for serialization.
#[derive(Default)]
pub struct ControlCodecBitcode;

impl ControlCodecBitcode {
    /// Encode a [`Command`] to bitcode bytes.
    pub fn encode_command(&self, cmd: &ControlCommand) -> Vec<u8> {
        bitcode::encode(cmd)
    }

    /// Decode bitcode bytes into a [`Command`].
    pub fn decode_command(&self, payload: &[u8]) -> Result<ControlCommand, NetworkError> {
        bitcode::decode(payload).map_err(|e| NetworkError::SerializationError(format!("command decode: {}", e)))
    }

    /// Encode a [`CommandResponse`] to bitcode bytes.
    pub fn encode_response(&self, resp: &ControlResponse) -> Vec<u8> {
        bitcode::encode(resp)
    }

    /// Decode bitcode bytes into a [`CommandResponse`].
    pub fn decode_response(&self, payload: &[u8]) -> Result<ControlResponse, NetworkError> {
        bitcode::decode(payload).map_err(|e| NetworkError::SerializationError(format!("command response decode: {}", e)))
    }
}

/// Codec for commands using JSON for serialization.
#[derive(Default)]
pub struct ControlCodecJson;

impl ControlCodecJson {
    /// Encode a [`Command`] to JSON bytes.
    pub fn encode_command(&self, cmd: &ControlCommand) -> Vec<u8> {
        serde_json::to_vec(cmd).unwrap_or_default()
    }

    /// Decode JSON bytes into a [`Command`].
    pub fn decode_command(&self, payload: &[u8]) -> Result<ControlCommand, NetworkError> {
        serde_json::from_slice(payload).map_err(|e| NetworkError::SerializationError(format!("command decode: {}", e)))
    }

    /// Encode a [`CommandResponse`] to JSON bytes.
    pub fn encode_response(&self, resp: &ControlResponse) -> Vec<u8> {
        serde_json::to_vec(resp).unwrap_or_default()
    }

    /// Decode JSON bytes into a [`CommandResponse`].
    pub fn decode_response(&self, payload: &[u8]) -> Result<ControlResponse, NetworkError> {
        serde_json::from_slice(payload).map_err(|e| NetworkError::SerializationError(format!("command response decode: {}", e)))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_bitcode_command_a() {
        let codec = ControlCodecBitcode;
        let cmd = ControlCommand::CommandA {
            handle: 1,
            parameter: 1234,
        };
        let bytes = codec.encode_command(&cmd);
        let decoded = codec.decode_command(&bytes).unwrap();
        let ControlCommand::CommandA { handle, parameter } = decoded else {
            panic!("expected CommandA");
        };
        assert_eq!(handle, 1);
        assert_eq!(parameter, 1234);
    }

    #[test]
    fn test_roundtrip_json_command_a() {
        let codec = ControlCodecJson;
        let cmd = ControlCommand::CommandA {
            handle: 1,
            parameter: 1234,
        };
        let bytes = codec.encode_command(&cmd);
        let decoded = codec.decode_command(&bytes).unwrap();
        let ControlCommand::CommandA { handle, parameter } = decoded else {
            panic!("expected CommandA");
        };
        assert_eq!(handle, 1);
        assert_eq!(parameter, 1234);
    }

    #[test]
    fn test_roundtrip_bitcode_response() {
        let codec = ControlCodecBitcode;
        let resp = ControlResponse::CommandAResponse { handle: 1, result: 42 };
        let bytes = codec.encode_response(&resp);
        let decoded = codec.decode_response(&bytes).unwrap();
        let ControlResponse::CommandAResponse { handle, result } = decoded else {
            panic!("expected CommandAResponse");
        };
        assert_eq!(handle, 1);
        assert_eq!(result, 42);
    }

    #[test]
    fn test_roundtrip_json_response() {
        let codec = ControlCodecJson;
        let resp = ControlResponse::SendSdsResponse { handle: 2, success: true };
        let bytes = codec.encode_response(&resp);
        let decoded = codec.decode_response(&bytes).unwrap();
        let ControlResponse::SendSdsResponse { handle, success } = decoded else {
            panic!("expected SendSdsResponse");
        };
        assert_eq!(handle, 2);
        assert!(success);
    }

    #[test]
    fn test_roundtrip_central_sds_commands() {
        for command in [
            ControlCommand::DeliverSds {
                handle: 77,
                source_ssi: 4_010_001,
                dest_ssi: 4_010_002,
                dest_is_group: false,
                sds_type: 4,
                len_bits: 40,
                payload: vec![0x82, 0x04, 0x01, 0x01, b'A'],
            },
            ControlCommand::SendStatus {
                handle: 78,
                source_ssi: 4_010_001,
                dest_ssi: 4_010_002,
                pre_coded_status: 32_780,
            },
        ] {
            let json = ControlCodecJson;
            let decoded = json.decode_command(&json.encode_command(&command)).unwrap();
            assert_eq!(
                serde_json::to_value(decoded).unwrap(),
                serde_json::to_value(command.clone()).unwrap()
            );

            let bitcode = ControlCodecBitcode;
            let decoded = bitcode
                .decode_command(&bitcode.encode_command(&command))
                .unwrap();
            assert_eq!(
                serde_json::to_value(decoded).unwrap(),
                serde_json::to_value(command).unwrap()
            );
        }
    }

    #[test]
    fn test_roundtrip_central_sds_response() {
        let response = ControlResponse::SdsDeliveryResponse {
            handle: 77,
            success: true,
            message: "accepted by TBS".to_string(),
        };
        let codec = ControlCodecJson;
        let decoded = codec
            .decode_response(&codec.encode_response(&response))
            .unwrap();
        let ControlResponse::SdsDeliveryResponse {
            handle,
            success,
            message,
        } = decoded
        else {
            panic!("expected SdsDeliveryResponse");
        };
        assert_eq!(handle, 77);
        assert!(success);
        assert_eq!(message, "accepted by TBS");
    }

    #[test]
    fn test_decode_invalid_bytes() {
        let codec = ControlCodecBitcode;
        // Use truncated bytes that cannot form a valid Command
        assert!(codec.decode_command(&[]).is_err());
    }
}

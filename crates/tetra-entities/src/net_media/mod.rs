//! Shared TBS media-bridge protocol and bounded in-process channels.
//!
//! The protocol types are available to backend-only crates without the full RF
//! runtime.  The channel implementation is runtime-gated and connects UMAC to
//! the Control-Room/Node-Gateway worker without putting network I/O into the
//! TDMA router thread.

use bitcode::{Decode, Encode};
use serde::{Deserialize, Serialize};

/// Packed TETRA speech service 0 frame size: 274 payload bits rounded to bytes.
pub const TETRA_ACELP_FRAME_BYTES: usize = 35;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaCodec {
    /// TETRA encoded speech service 0, one 274-bit TCH/S frame packed into 35 bytes.
    TetraAcelp0,
}

#[derive(Debug, Clone, Encode, Decode, Serialize, Deserialize)]
pub struct MediaUplinkFrame {
    pub node_id: String,
    pub sequence: u64,
    pub timestamp: String,
    pub carrier_num: u16,
    /// Logical TBS timeslot (1..=7; TS5..TS7 map to secondary-carrier air TS2..TS4).
    pub logical_ts: u8,
    pub codec: MediaCodec,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, Encode, Decode, Serialize, Deserialize)]
pub struct MediaDownlinkFrame {
    /// Logical Media-Switch session/call identifier used for diagnostics and taps.
    pub session_id: String,
    pub source_node_id: String,
    pub sequence: u64,
    /// Logical destination timeslot on the target TBS.
    pub logical_ts: u8,
    pub codec: MediaCodec,
    pub payload: Vec<u8>,
}

#[cfg(feature = "runtime")]
mod channel {
    use crossbeam_channel::{Receiver, Sender, TryRecvError, TrySendError, bounded};

    use super::{MediaCodec, MediaDownlinkFrame, TETRA_ACELP_FRAME_BYTES};

    #[derive(Debug, Clone)]
    pub struct LocalMediaUplinkFrame {
        pub sequence: u64,
        pub carrier_num: u16,
        pub logical_ts: u8,
        pub codec: MediaCodec,
        pub payload: Vec<u8>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum MediaSendError {
        Full,
        Disconnected,
        InvalidFrame,
    }

    #[derive(Clone)]
    pub struct MediaUplinkSink {
        tx: Sender<LocalMediaUplinkFrame>,
    }

    pub struct MediaUplinkSource {
        rx: Receiver<LocalMediaUplinkFrame>,
    }

    #[derive(Clone)]
    pub struct MediaDownlinkSink {
        tx: Sender<MediaDownlinkFrame>,
    }

    pub struct MediaDownlinkSource {
        rx: Receiver<MediaDownlinkFrame>,
    }

    impl MediaUplinkSink {
        pub fn try_send(&self, frame: LocalMediaUplinkFrame) -> Result<(), MediaSendError> {
            if frame.payload.len() != TETRA_ACELP_FRAME_BYTES {
                return Err(MediaSendError::InvalidFrame);
            }
            self.tx.try_send(frame).map_err(|error| match error {
                TrySendError::Full(_) => MediaSendError::Full,
                TrySendError::Disconnected(_) => MediaSendError::Disconnected,
            })
        }
    }

    impl MediaUplinkSource {
        pub fn try_recv(&self) -> Result<LocalMediaUplinkFrame, TryRecvError> {
            self.rx.try_recv()
        }
    }

    impl MediaDownlinkSink {
        pub fn try_send(&self, frame: MediaDownlinkFrame) -> Result<(), MediaSendError> {
            if frame.payload.len() != TETRA_ACELP_FRAME_BYTES {
                return Err(MediaSendError::InvalidFrame);
            }
            self.tx.try_send(frame).map_err(|error| match error {
                TrySendError::Full(_) => MediaSendError::Full,
                TrySendError::Disconnected(_) => MediaSendError::Disconnected,
            })
        }
    }

    impl MediaDownlinkSource {
        pub fn try_recv(&self) -> Result<MediaDownlinkFrame, TryRecvError> {
            self.rx.try_recv()
        }
    }

    /// Create independent bounded queues for UL (UMAC -> network worker) and DL
    /// (network worker -> UMAC). Bounded queues make overload visible and stop a
    /// slow management network from consuming unbounded RF-process memory.
    pub fn media_bridge_channel(
        capacity: usize,
    ) -> (
        MediaUplinkSink,
        MediaUplinkSource,
        MediaDownlinkSink,
        MediaDownlinkSource,
    ) {
        let capacity = capacity.max(16);
        let (uplink_tx, uplink_rx) = bounded(capacity);
        let (downlink_tx, downlink_rx) = bounded(capacity);
        (
            MediaUplinkSink { tx: uplink_tx },
            MediaUplinkSource { rx: uplink_rx },
            MediaDownlinkSink { tx: downlink_tx },
            MediaDownlinkSource { rx: downlink_rx },
        )
    }

    pub use crossbeam_channel::TryRecvError as MediaTryRecvError;
}

#[cfg(feature = "runtime")]
pub use channel::{
    LocalMediaUplinkFrame, MediaDownlinkSink, MediaDownlinkSource, MediaSendError,
    MediaTryRecvError, MediaUplinkSink, MediaUplinkSource, media_bridge_channel,
};

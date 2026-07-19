//! Local TETRA speech recorder.
//!
//! The entity receives call/floor lifecycle metadata from CMCE and a passive copy of
//! valid uplink TMD speech blocks from UMAC. Recordings are stored as 8-kHz mono 16-bit
//! PCM WAV files with JSON metadata sidecars.

mod archive;
pub mod entity;
pub mod service;
pub mod types;
mod wav;

pub use entity::RecorderEntity;
pub use service::RecorderHandle;
pub use types::{RecorderStatus, RecordingMetadata, RecordingSegment};

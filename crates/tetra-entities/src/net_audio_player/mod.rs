//! Local WAV/MP3 dispatch into TETRA group and individual calls.
//!
//! Audio is fully decoded and encoded before CMCE resources are requested. The RF core
//! therefore never waits on disk I/O or ffmpeg while a traffic channel is active.

mod entity;
mod media;
mod service;
mod types;

pub use entity::AudioPlayerEntity;
pub use service::AudioPlayerHandle;
pub use types::{AudioPlayerState, AudioPlayerStatus, AudioSourceType, AudioTargetType, MediaEntry, MediaSourceInfo};

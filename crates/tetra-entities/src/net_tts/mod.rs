//! Local Piper HTTP text-to-speech generation for the existing TETRA audio player.
//!
//! The TTS provider generates a complete WAV file before the audio player requests
//! CMCE/RF resources. Provider latency or failure can therefore never starve an
//! active TETRA traffic channel.

mod service;
mod templates;
mod types;

pub use service::TtsHandle;
pub use templates::{TtsTemplate, TtsTemplateDraft};
pub use types::{TtsState, TtsStatus, TtsVoiceStatus};

//! Local Piper HTTP text-to-speech generation for the recording library.
//!
//! Piper always generates a complete canonical recording-format WAV first. The
//! finished file is imported into the local recorder with JSON metadata and can
//! only be transmitted later through the ordinary recording selection workflow.


mod service;
mod templates;
mod types;

pub use service::TtsHandle;
pub use templates::{TtsTemplate, TtsTemplateDraft};
pub use types::{TtsState, TtsStatus, TtsVoiceStatus};

use serde::Serialize;

use crate::net_audio_player::AudioTargetType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TtsState {
    Idle,
    Synthesizing,
    Ready,
    Dispatching,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize)]
pub struct TtsVoiceStatus {
    pub id: String,
    pub name: String,
    pub provider_voice: String,
    pub speaker_id: Option<u32>,
    pub available: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TtsStatus {
    pub available: bool,
    pub provider_available: bool,
    pub provider_endpoint: String,
    pub provider_error: Option<String>,
    pub cache_directory: String,
    pub startup_warning: Option<String>,
    pub state: TtsState,
    pub job_id: Option<String>,
    pub audio_player_job_id: Option<String>,
    pub voice_id: Option<String>,
    pub speed: Option<f32>,
    pub text_preview: Option<String>,
    pub file_name: Option<String>,
    pub generated_audio_available: bool,
    pub target_type: Option<AudioTargetType>,
    pub target_id: Option<u32>,
    pub priority: Option<u8>,
    pub max_text_characters: usize,
    pub default_voice: String,
    pub default_speed: f32,
    pub default_priority: u8,
    pub last_error: Option<String>,
}

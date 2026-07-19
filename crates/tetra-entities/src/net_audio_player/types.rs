use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioTargetType {
    Group,
    Individual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioSourceType {
    Media,
    Recording,
    Tts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioPlayerState {
    Idle,
    Preparing,
    Calling,
    WaitingForAnswer,
    Playing,
    Finishing,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
pub struct AudioPlayerStatus {
    pub available: bool,
    pub state: AudioPlayerState,
    pub directory: String,
    pub cache_directory: String,
    pub startup_warning: Option<String>,
    pub job_id: Option<String>,
    pub file_name: Option<String>,
    pub source_type: Option<AudioSourceType>,
    pub source_id: Option<String>,
    pub target_type: Option<AudioTargetType>,
    pub target_id: Option<u32>,
    pub priority: Option<u8>,
    pub duration_ms: u64,
    pub position_ms: u64,
    pub total_blocks: usize,
    pub sent_blocks: usize,
    pub call_id: Option<u16>,
    pub timeslot: Option<u8>,
    pub ffmpeg_available: bool,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MediaSourceInfo {
    pub id: String,
    pub name: String,
    pub path: String,
    pub source_type: String,
    pub available: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MediaEntry {
    pub name: String,
    pub path: String,
    pub entry_type: String,
    pub size_bytes: Option<u64>,
    pub extension: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedAudioSource {
    pub path: std::path::PathBuf,
    pub display_name: String,
    pub source_type: AudioSourceType,
    pub source_id: Option<String>,
    pub cache_before_decode: bool,
}

#[derive(Debug, Clone)]
pub(crate) enum AudioPlayerCommand {
    Play {
        job_id: String,
        source: ResolvedAudioSource,
        target_type: AudioTargetType,
        target_id: u32,
        priority: u8,
    },
    Stop,
}

#[derive(Debug)]
pub(crate) struct PreparedAudio {
    pub job_id: String,
    pub target_type: AudioTargetType,
    pub target_id: u32,
    pub priority: u8,
    pub duration_ms: u64,
    pub blocks: Vec<Vec<u8>>,
}

#[derive(Debug)]
pub(crate) enum PrepareEvent {
    Ready(PreparedAudio),
    Failed { job_id: String, error: String },
}

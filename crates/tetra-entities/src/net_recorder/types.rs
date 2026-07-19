use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingSegment {
    pub source_issi: u32,
    pub timeslot: u8,
    pub carrier_num: u16,
    pub start_ms: u64,
    pub end_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingMetadata {
    pub schema_version: u8,
    pub id: String,
    pub call_id: u16,
    pub source_issi: u32,
    pub destination_id: u32,
    pub destination_type: String,
    pub started_at: String,
    pub ended_at: String,
    pub duration_ms: u64,
    pub audio_bytes: u64,
    pub relative_audio_path: String,
    pub recovered_after_unclean_shutdown: bool,
    pub segments: Vec<RecordingSegment>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecorderStatus {
    pub available: bool,
    pub active: bool,
    pub directory: String,
    pub mode: String,
    pub selected_groups: Vec<u32>,
    pub minimum_free_space_mb: u64,
    pub free_space_bytes: Option<u64>,
    pub used_bytes: u64,
    pub recording_count: usize,
    pub active_sessions: usize,
    pub active_call_ids: Vec<u16>,
    pub last_recording_id: Option<String>,
    pub last_error: Option<String>,
    pub archive_enabled: bool,
    pub archive_directory: String,
    pub archive_available: bool,
    pub archive_active: bool,
    pub archive_pending: usize,
    pub archive_completed: usize,
    pub archive_last_success_at: Option<String>,
    pub archive_last_error: Option<String>,
}

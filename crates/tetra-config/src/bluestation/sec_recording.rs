use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use toml::Value;

/// Selects which locally-originated speech calls are written to disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordingMode {
    /// Record every call for which CMCE exposes a local speech floor.
    All,
    /// Record only group calls whose GSSI appears in `selected_groups`.
    SelectedGroups,
}

impl Default for RecordingMode {
    fn default() -> Self {
        Self::All
    }
}

/// Local TETRA speech recording configuration.
#[derive(Debug, Clone)]
pub struct CfgRecording {
    /// Instantiate the recorder entity and expose its dashboard API.
    pub enabled: bool,
    /// Initial runtime state after process start. The dashboard may toggle this live.
    pub active: bool,
    /// Root directory for WAV files and JSON sidecars.
    pub directory: String,
    /// Call selection policy.
    pub mode: RecordingMode,
    /// GSSI allow-list used when `mode = "selected_groups"`.
    pub selected_groups: Vec<u32>,
    /// Do not begin a new recording when free space falls below this threshold.
    pub minimum_free_space_mb: u64,
    /// Delete completed recordings older than this many days. Zero disables retention cleanup.
    pub retention_days: u32,
    /// Hard limit for one recording. Prevents an orphaned call from filling the disk.
    pub max_recording_minutes: u32,
    /// Finalize a call after this many seconds without an active floor or call-end event.
    pub idle_finalize_secs: u32,
    /// Maximum number of entries returned by the recordings API.
    pub max_list_entries: usize,
    /// Copy completed WAV/JSON pairs to the configured archive directory.
    pub archive_enabled: bool,
    /// Existing writable directory on an OS-mounted server share for normal call recordings.
    pub archive_directory: String,
    /// Copy imported TTS library WAV/JSON pairs to a separate server directory.
    pub tts_archive_enabled: bool,
    /// Existing writable directory on an OS-mounted server share for TTS library WAVs.
    pub tts_archive_directory: String,
    /// Retry interval for pending copies while either share is unavailable.
    pub archive_retry_seconds: u64,
}

impl Default for CfgRecording {
    fn default() -> Self {
        apply_recording_patch(CfgRecordingDto::default()).expect("default recording config must be valid")
    }
}

#[derive(Debug, Deserialize)]
pub struct CfgRecordingDto {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub active: bool,
    #[serde(default = "default_directory")]
    pub directory: String,
    #[serde(default)]
    pub mode: RecordingMode,
    #[serde(default)]
    pub selected_groups: Vec<u32>,
    #[serde(default = "default_minimum_free_space_mb")]
    pub minimum_free_space_mb: u64,
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
    #[serde(default = "default_max_recording_minutes")]
    pub max_recording_minutes: u32,
    #[serde(default = "default_idle_finalize_secs")]
    pub idle_finalize_secs: u32,
    #[serde(default = "default_max_list_entries")]
    pub max_list_entries: usize,
    #[serde(default)]
    pub archive_enabled: bool,
    #[serde(default = "default_archive_directory")]
    pub archive_directory: String,
    #[serde(default)]
    pub tts_archive_enabled: bool,
    #[serde(default = "default_tts_archive_directory")]
    pub tts_archive_directory: String,
    #[serde(default = "default_archive_retry_seconds")]
    pub archive_retry_seconds: u64,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl Default for CfgRecordingDto {
    fn default() -> Self {
        Self {
            enabled: false,
            active: false,
            directory: default_directory(),
            mode: RecordingMode::All,
            selected_groups: Vec::new(),
            minimum_free_space_mb: default_minimum_free_space_mb(),
            retention_days: default_retention_days(),
            max_recording_minutes: default_max_recording_minutes(),
            idle_finalize_secs: default_idle_finalize_secs(),
            max_list_entries: default_max_list_entries(),
            archive_enabled: false,
            archive_directory: default_archive_directory(),
            tts_archive_enabled: false,
            tts_archive_directory: default_tts_archive_directory(),
            archive_retry_seconds: default_archive_retry_seconds(),
            extra: HashMap::new(),
        }
    }
}

fn default_directory() -> String {
    "/var/lib/netcore/recordings".to_string()
}

fn default_minimum_free_space_mb() -> u64 {
    2_048
}

fn default_retention_days() -> u32 {
    30
}

fn default_max_recording_minutes() -> u32 {
    120
}

fn default_idle_finalize_secs() -> u32 {
    15
}

fn default_max_list_entries() -> usize {
    2_000
}

fn default_archive_directory() -> String {
    "/mnt/nfs-share/Recordings".to_string()
}

fn default_tts_archive_directory() -> String {
    "/mnt/nfs-share/TTS-Dateien".to_string()
}

fn default_archive_retry_seconds() -> u64 {
    60
}

pub fn apply_recording_patch(mut src: CfgRecordingDto) -> Result<CfgRecording, String> {
    src.directory = src.directory.trim().to_string();
    if src.directory.is_empty() {
        return Err("recording: directory cannot be empty".to_string());
    }
    if src.max_recording_minutes == 0 {
        return Err("recording: max_recording_minutes must be greater than zero".to_string());
    }
    if src.idle_finalize_secs == 0 {
        return Err("recording: idle_finalize_secs must be greater than zero".to_string());
    }
    if src.max_list_entries == 0 {
        return Err("recording: max_list_entries must be greater than zero".to_string());
    }
    src.archive_directory = src.archive_directory.trim().to_string();
    src.tts_archive_directory = src.tts_archive_directory.trim().to_string();
    validate_archive_directory(
        "archive_directory",
        src.archive_enabled,
        &src.archive_directory,
        &src.directory,
    )?;
    validate_archive_directory(
        "tts_archive_directory",
        src.tts_archive_enabled,
        &src.tts_archive_directory,
        &src.directory,
    )?;
    if src.archive_enabled
        && src.tts_archive_enabled
        && src.archive_directory == src.tts_archive_directory
    {
        return Err(
            "recording: archive_directory and tts_archive_directory must differ".to_string(),
        );
    }
    if (src.archive_enabled || src.tts_archive_enabled) && src.archive_retry_seconds == 0 {
        return Err("recording: archive_retry_seconds must be greater than zero".to_string());
    }
    if src.selected_groups.iter().any(|gssi| *gssi == 0 || *gssi > 0x00ff_ffff) {
        return Err("recording: selected_groups entries must be valid 24-bit GSSIs".to_string());
    }
    src.selected_groups.sort_unstable();
    src.selected_groups.dedup();

    Ok(CfgRecording {
        enabled: src.enabled,
        active: src.active,
        directory: src.directory,
        mode: src.mode,
        selected_groups: src.selected_groups,
        minimum_free_space_mb: src.minimum_free_space_mb,
        retention_days: src.retention_days,
        max_recording_minutes: src.max_recording_minutes,
        idle_finalize_secs: src.idle_finalize_secs,
        max_list_entries: src.max_list_entries,
        archive_enabled: src.archive_enabled,
        archive_directory: src.archive_directory,
        tts_archive_enabled: src.tts_archive_enabled,
        tts_archive_directory: src.tts_archive_directory,
        archive_retry_seconds: src.archive_retry_seconds,
    })
}

fn validate_archive_directory(
    field: &str,
    enabled: bool,
    value: &str,
    local_directory: &str,
) -> Result<(), String> {
    if !enabled {
        return Ok(());
    }
    if value.is_empty() {
        return Err(format!(
            "recording: {field} cannot be empty when its archive is enabled"
        ));
    }
    if !Path::new(value).is_absolute() {
        return Err(format!("recording: {field} must be an absolute path"));
    }
    if value == local_directory {
        return Err(format!(
            "recording: {field} must differ from directory"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_safe_and_disabled() {
        let cfg = CfgRecording::default();
        assert!(!cfg.enabled);
        assert!(!cfg.active);
        assert_eq!(cfg.mode, RecordingMode::All);
        assert!(cfg.minimum_free_space_mb > 0);
    }

    #[test]
    fn rejects_invalid_group_ids() {
        let dto = CfgRecordingDto {
            selected_groups: vec![0, 0x0100_0000],
            ..CfgRecordingDto::default()
        };
        assert!(apply_recording_patch(dto).is_err());
    }

    #[test]
    fn rejects_relative_archive_directory() {
        let dto = CfgRecordingDto {
            archive_enabled: true,
            archive_directory: "relative/archive".to_string(),
            ..CfgRecordingDto::default()
        };
        assert!(apply_recording_patch(dto).is_err());
    }

    #[test]
    fn rejects_relative_tts_archive_directory() {
        let dto = CfgRecordingDto {
            tts_archive_enabled: true,
            tts_archive_directory: "relative/tts".to_string(),
            ..CfgRecordingDto::default()
        };
        assert!(apply_recording_patch(dto).is_err());
    }

    #[test]
    fn keeps_recording_and_tts_archive_separate() {
        let dto = CfgRecordingDto {
            archive_enabled: true,
            tts_archive_enabled: true,
            archive_directory: "/mnt/nfs-share/Recordings".to_string(),
            tts_archive_directory: "/mnt/nfs-share/TTS-Dateien".to_string(),
            ..CfgRecordingDto::default()
        };
        let cfg = apply_recording_patch(dto).expect("split archive config should be valid");
        assert_eq!(cfg.archive_directory, "/mnt/nfs-share/Recordings");
        assert_eq!(cfg.tts_archive_directory, "/mnt/nfs-share/TTS-Dateien");
    }
}

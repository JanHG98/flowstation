use std::collections::HashMap;

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
    })
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
}

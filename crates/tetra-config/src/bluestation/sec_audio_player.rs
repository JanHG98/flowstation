use std::collections::HashMap;

use serde::Deserialize;
use toml::Value;

/// Local audio dispatch configuration.
#[derive(Debug, Clone)]
pub struct CfgAudioPlayer {
    /// Instantiate the audio-player entity and dashboard API.
    pub enabled: bool,
    /// Root directory for locally managed WAV/MP3 files.
    pub directory: String,
    /// TETRA identity displayed as the network-side source of generated calls.
    pub source_issi: u32,
    /// Default TETRA call priority used when the UI does not override it.
    pub default_priority: u8,
    /// Reject source files larger than this value.
    pub max_file_size_mb: u64,
    /// Reject or truncate decoded audio beyond this duration.
    pub max_duration_seconds: u32,
    /// Number of encoded silence blocks appended before call release.
    pub tail_silence_blocks: u8,
    /// Maximum time to wait for an individual subscriber to answer.
    pub individual_answer_timeout_seconds: u32,
    /// ffmpeg executable used for MP3 and non-native WAV conversion.
    pub ffmpeg_path: String,
}

impl Default for CfgAudioPlayer {
    fn default() -> Self {
        apply_audio_player_patch(CfgAudioPlayerDto::default()).expect("default audio-player config must be valid")
    }
}

#[derive(Debug, Deserialize)]
pub struct CfgAudioPlayerDto {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_directory")]
    pub directory: String,
    #[serde(default = "default_source_issi")]
    pub source_issi: u32,
    #[serde(default = "default_priority")]
    pub default_priority: u8,
    #[serde(default = "default_max_file_size_mb")]
    pub max_file_size_mb: u64,
    #[serde(default = "default_max_duration_seconds")]
    pub max_duration_seconds: u32,
    #[serde(default = "default_tail_silence_blocks")]
    pub tail_silence_blocks: u8,
    #[serde(default = "default_individual_answer_timeout_seconds")]
    pub individual_answer_timeout_seconds: u32,
    #[serde(default = "default_ffmpeg_path")]
    pub ffmpeg_path: String,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl Default for CfgAudioPlayerDto {
    fn default() -> Self {
        Self {
            enabled: false,
            directory: default_directory(),
            source_issi: default_source_issi(),
            default_priority: default_priority(),
            max_file_size_mb: default_max_file_size_mb(),
            max_duration_seconds: default_max_duration_seconds(),
            tail_silence_blocks: default_tail_silence_blocks(),
            individual_answer_timeout_seconds: default_individual_answer_timeout_seconds(),
            ffmpeg_path: default_ffmpeg_path(),
            extra: HashMap::new(),
        }
    }
}

fn default_directory() -> String {
    "/var/lib/netcore/audio".to_string()
}
fn default_source_issi() -> u32 {
    4_010_099
}
fn default_priority() -> u8 {
    5
}
fn default_max_file_size_mb() -> u64 {
    100
}
fn default_max_duration_seconds() -> u32 {
    1_800
}
fn default_tail_silence_blocks() -> u8 {
    3
}
fn default_individual_answer_timeout_seconds() -> u32 {
    30
}
fn default_ffmpeg_path() -> String {
    "ffmpeg".to_string()
}

pub fn apply_audio_player_patch(mut src: CfgAudioPlayerDto) -> Result<CfgAudioPlayer, String> {
    src.directory = src.directory.trim().to_string();
    src.ffmpeg_path = src.ffmpeg_path.trim().to_string();
    if src.directory.is_empty() {
        return Err("audio_player: directory cannot be empty".to_string());
    }
    if src.source_issi == 0 || src.source_issi > 0x00ff_ffff {
        return Err("audio_player: source_issi must be a valid 24-bit ISSI".to_string());
    }
    if src.default_priority > 15 {
        return Err("audio_player: default_priority must be 0-15".to_string());
    }
    if src.max_file_size_mb == 0 {
        return Err("audio_player: max_file_size_mb must be greater than zero".to_string());
    }
    if src.max_duration_seconds == 0 {
        return Err("audio_player: max_duration_seconds must be greater than zero".to_string());
    }
    if src.tail_silence_blocks > 20 {
        return Err("audio_player: tail_silence_blocks must be 0-20".to_string());
    }
    if src.individual_answer_timeout_seconds == 0 {
        return Err("audio_player: individual_answer_timeout_seconds must be greater than zero".to_string());
    }
    if src.ffmpeg_path.is_empty() {
        return Err("audio_player: ffmpeg_path cannot be empty".to_string());
    }

    Ok(CfgAudioPlayer {
        enabled: src.enabled,
        directory: src.directory,
        source_issi: src.source_issi,
        default_priority: src.default_priority,
        max_file_size_mb: src.max_file_size_mb,
        max_duration_seconds: src.max_duration_seconds,
        tail_silence_blocks: src.tail_silence_blocks,
        individual_answer_timeout_seconds: src.individual_answer_timeout_seconds,
        ffmpeg_path: src.ffmpeg_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_safe_and_disabled() {
        let cfg = CfgAudioPlayer::default();
        assert!(!cfg.enabled);
        assert_eq!(cfg.source_issi, 4_010_099);
        assert!(cfg.max_duration_seconds > 0);
    }

    #[test]
    fn rejects_invalid_identity_and_priority() {
        let dto = CfgAudioPlayerDto {
            source_issi: 0,
            default_priority: 16,
            ..CfgAudioPlayerDto::default()
        };
        assert!(apply_audio_player_patch(dto).is_err());
    }
}

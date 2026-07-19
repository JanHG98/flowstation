use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use tetra_config::bluestation::CfgAudioPlayer;
use uuid::Uuid;

use super::types::{
    AudioPlayerCommand, AudioPlayerState, AudioPlayerStatus, AudioSourceType, AudioTargetType, MediaEntry, ResolvedAudioSource,
};

#[derive(Debug)]
struct LiveStatus {
    state: AudioPlayerState,
    job_id: Option<String>,
    file_name: Option<String>,
    source_type: Option<AudioSourceType>,
    target_type: Option<AudioTargetType>,
    target_id: Option<u32>,
    priority: Option<u8>,
    duration_ms: u64,
    position_ms: u64,
    total_blocks: usize,
    sent_blocks: usize,
    call_id: Option<u16>,
    timeslot: Option<u8>,
    last_error: Option<String>,
}

impl Default for LiveStatus {
    fn default() -> Self {
        Self {
            state: AudioPlayerState::Idle,
            job_id: None,
            file_name: None,
            source_type: None,
            target_type: None,
            target_id: None,
            priority: None,
            duration_ms: 0,
            position_ms: 0,
            total_blocks: 0,
            sent_blocks: 0,
            call_id: None,
            timeslot: None,
            last_error: None,
        }
    }
}

struct AudioPlayerShared {
    config: CfgAudioPlayer,
    root: PathBuf,
    command_tx: crossbeam_channel::Sender<AudioPlayerCommand>,
    live: Mutex<LiveStatus>,
    ffmpeg_available: bool,
}

#[derive(Clone)]
pub struct AudioPlayerHandle {
    inner: Arc<AudioPlayerShared>,
}

impl AudioPlayerHandle {
    pub(crate) fn new(
        config: CfgAudioPlayer,
        command_tx: crossbeam_channel::Sender<AudioPlayerCommand>,
        ffmpeg_available: bool,
    ) -> Result<Self, String> {
        let root = PathBuf::from(&config.directory);
        fs::create_dir_all(&root).map_err(|e| format!("cannot create {}: {e}", root.display()))?;
        let root = root
            .canonicalize()
            .map_err(|e| format!("cannot canonicalize {}: {e}", root.display()))?;
        Ok(Self {
            inner: Arc::new(AudioPlayerShared {
                config,
                root,
                command_tx,
                live: Mutex::new(LiveStatus::default()),
                ffmpeg_available,
            }),
        })
    }

    pub fn config(&self) -> &CfgAudioPlayer {
        &self.inner.config
    }

    pub fn root(&self) -> &Path {
        &self.inner.root
    }

    pub fn status(&self) -> AudioPlayerStatus {
        let live = self.inner.live.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        AudioPlayerStatus {
            available: true,
            state: live.state,
            directory: self.inner.root.display().to_string(),
            job_id: live.job_id.clone(),
            file_name: live.file_name.clone(),
            source_type: live.source_type,
            target_type: live.target_type,
            target_id: live.target_id,
            priority: live.priority,
            duration_ms: live.duration_ms,
            position_ms: live.position_ms,
            total_blocks: live.total_blocks,
            sent_blocks: live.sent_blocks,
            call_id: live.call_id,
            timeslot: live.timeslot,
            ffmpeg_available: self.inner.ffmpeg_available,
            last_error: live.last_error.clone(),
        }
    }

    pub fn list_media(&self, relative: &str) -> Result<Vec<MediaEntry>, String> {
        let directory = self.resolve_directory(relative)?;
        let relative_base = directory.strip_prefix(&self.inner.root).map_err(|e| e.to_string())?;
        let mut entries = Vec::new();
        for entry in fs::read_dir(&directory).map_err(|e| format!("cannot read {}: {e}", directory.display()))? {
            let entry = entry.map_err(|e| e.to_string())?;
            let file_type = entry.file_type().map_err(|e| e.to_string())?;
            if file_type.is_symlink() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let child_relative = relative_base.join(&name).to_string_lossy().replace('\\', "/");
            if file_type.is_dir() {
                entries.push(MediaEntry {
                    name,
                    path: child_relative,
                    entry_type: "directory".to_string(),
                    size_bytes: None,
                    extension: None,
                });
            } else if file_type.is_file() {
                let extension = entry
                    .path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(str::to_ascii_lowercase);
                if !matches!(extension.as_deref(), Some("wav" | "mp3")) {
                    continue;
                }
                entries.push(MediaEntry {
                    name,
                    path: child_relative,
                    entry_type: "file".to_string(),
                    size_bytes: entry.metadata().ok().map(|metadata| metadata.len()),
                    extension,
                });
            }
        }
        entries.sort_by(|a, b| {
            let a_dir = a.entry_type == "directory";
            let b_dir = b.entry_type == "directory";
            b_dir.cmp(&a_dir).then_with(|| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()))
        });
        Ok(entries)
    }

    pub fn play_media(
        &self,
        relative_path: &str,
        target_type: AudioTargetType,
        target_id: u32,
        priority: Option<u8>,
    ) -> Result<String, String> {
        let path = self.resolve_media_file(relative_path)?;
        let display_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("audio")
            .to_string();
        self.play_resolved(
            ResolvedAudioSource {
                path,
                display_name,
                source_type: AudioSourceType::Media,
            },
            target_type,
            target_id,
            priority,
        )
    }

    pub fn play_recording(
        &self,
        path: PathBuf,
        display_name: String,
        target_type: AudioTargetType,
        target_id: u32,
        priority: Option<u8>,
    ) -> Result<String, String> {
        let canonical = path.canonicalize().map_err(|e| format!("cannot open recording: {e}"))?;
        if !canonical.is_file() {
            return Err("recording is not a regular file".to_string());
        }
        self.play_resolved(
            ResolvedAudioSource {
                path: canonical,
                display_name,
                source_type: AudioSourceType::Recording,
            },
            target_type,
            target_id,
            priority,
        )
    }

    fn play_resolved(
        &self,
        source: ResolvedAudioSource,
        target_type: AudioTargetType,
        target_id: u32,
        priority: Option<u8>,
    ) -> Result<String, String> {
        if target_id == 0 || target_id > 0x00ff_ffff {
            return Err("target must be a valid 24-bit ISSI/GSSI".to_string());
        }
        let priority = priority.unwrap_or(self.inner.config.default_priority);
        if priority > 15 {
            return Err("priority must be 0-15".to_string());
        }
        let job_id = Uuid::new_v4().to_string();
        // Reserve the player and enqueue the command while holding the same status lock.
        // This prevents a concurrent Stop request from overtaking Play between those two steps.
        let mut live = self.inner.live.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if !matches!(live.state, AudioPlayerState::Idle | AudioPlayerState::Failed) {
            return Err("an audio transmission is already active".to_string());
        }
        *live = LiveStatus {
            state: AudioPlayerState::Preparing,
            job_id: Some(job_id.clone()),
            file_name: Some(source.display_name.clone()),
            source_type: Some(source.source_type),
            target_type: Some(target_type),
            target_id: Some(target_id),
            priority: Some(priority),
            ..LiveStatus::default()
        };
        if self
            .inner
            .command_tx
            .send(AudioPlayerCommand::Play {
                job_id: job_id.clone(),
                source,
                target_type,
                target_id,
                priority,
            })
            .is_err()
        {
            live.state = AudioPlayerState::Failed;
            live.last_error = Some("audio-player entity is not running".to_string());
            return Err("audio-player entity is not running".to_string());
        }
        Ok(job_id)
    }

    pub fn stop(&self) -> Result<(), String> {
        self.inner
            .command_tx
            .send(AudioPlayerCommand::Stop)
            .map_err(|_| "audio-player entity is not running".to_string())
    }

    pub(crate) fn mark_preparing(
        &self,
        job_id: String,
        file_name: String,
        source_type: AudioSourceType,
        target_type: AudioTargetType,
        target_id: u32,
        priority: u8,
    ) {
        let mut live = self.inner.live.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        *live = LiveStatus {
            state: AudioPlayerState::Preparing,
            job_id: Some(job_id),
            file_name: Some(file_name),
            source_type: Some(source_type),
            target_type: Some(target_type),
            target_id: Some(target_id),
            priority: Some(priority),
            ..LiveStatus::default()
        };
    }

    pub(crate) fn set_state(&self, state: AudioPlayerState) {
        let mut live = self.inner.live.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        live.state = state;
    }

    pub(crate) fn mark_prepared(&self, duration_ms: u64, total_blocks: usize) {
        let mut live = self.inner.live.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        live.duration_ms = duration_ms;
        live.total_blocks = total_blocks;
        live.position_ms = 0;
        live.sent_blocks = 0;
    }

    pub(crate) fn mark_media_ready(&self, call_id: u16, ts: u8) {
        let mut live = self.inner.live.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        live.call_id = Some(call_id);
        live.timeslot = Some(ts);
        live.state = AudioPlayerState::Playing;
    }

    pub(crate) fn mark_progress(&self, sent_blocks: usize) {
        let mut live = self.inner.live.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        live.sent_blocks = sent_blocks;
        live.position_ms = (sent_blocks as u64 * 60).min(live.duration_ms);
    }

    pub(crate) fn mark_idle(&self) {
        let mut live = self.inner.live.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        *live = LiveStatus::default();
    }

    pub(crate) fn mark_failed(&self, error: impl Into<String>) {
        let error = error.into();
        tracing::error!("AudioPlayer: {}", error);
        let mut live = self.inner.live.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        live.state = AudioPlayerState::Failed;
        live.last_error = Some(error);
        live.call_id = None;
        live.timeslot = None;
    }

    fn resolve_directory(&self, relative: &str) -> Result<PathBuf, String> {
        let relative = safe_relative_path(relative)?;
        let path = self.inner.root.join(relative);
        let canonical = path.canonicalize().map_err(|e| format!("directory not found: {e}"))?;
        if !canonical.starts_with(&self.inner.root) || !canonical.is_dir() {
            return Err("directory escapes the configured media root".to_string());
        }
        Ok(canonical)
    }

    fn resolve_media_file(&self, relative: &str) -> Result<PathBuf, String> {
        let relative = safe_relative_path(relative)?;
        let path = self.inner.root.join(relative);
        let canonical = path.canonicalize().map_err(|e| format!("file not found: {e}"))?;
        if !canonical.starts_with(&self.inner.root) || !canonical.is_file() {
            return Err("file escapes the configured media root".to_string());
        }
        let extension = canonical
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !matches!(extension.as_str(), "wav" | "mp3") {
            return Err("only .wav and .mp3 files are supported".to_string());
        }
        Ok(canonical)
    }
}

fn safe_relative_path(path: &str) -> Result<PathBuf, String> {
    let path = Path::new(path.trim().trim_start_matches('/'));
    let mut clean = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            _ => return Err("invalid media path".to_string()),
        }
    }
    Ok(clean)
}

pub(crate) fn detect_ffmpeg(path: &str) -> bool {
    Command::new(path)
        .arg("-version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

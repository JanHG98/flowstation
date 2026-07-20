use std::ffi::CString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read};
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{SyncSender, sync_channel};
use std::sync::{Arc, Mutex};

use tetra_config::bluestation::{CfgRecording, RecordingMode};
use uuid::Uuid;

use super::archive::{recording_is_archived, spawn_archive_worker};
use super::types::{RecorderStatus, RecordingMetadata};
use super::wav::recover_part;

#[derive(Default)]
pub(super) struct LiveStatus {
    active_sessions: usize,
    active_call_ids: Vec<u16>,
    last_recording_id: Option<String>,
    last_error: Option<String>,
    pub(super) archive_available: bool,
    pub(super) archive_active: bool,
    pub(super) archive_pending: usize,
    pub(super) archive_completed: usize,
    pub(super) archive_last_success_at: Option<String>,
    pub(super) archive_last_error: Option<String>,
}

pub(super) struct RecorderShared {
    pub(super) config: CfgRecording,
    pub(super) root: PathBuf,
    active: AtomicBool,
    live: Mutex<LiveStatus>,
    archive_tx: Option<SyncSender<()>>,
}

#[derive(Clone)]
pub struct RecorderHandle {
    inner: Arc<RecorderShared>,
}

impl RecorderHandle {
    pub(crate) fn new(config: CfgRecording) -> io::Result<Self> {
        let root = PathBuf::from(&config.directory);
        fs::create_dir_all(&root)?;
        let (archive_tx, archive_rx) = if config.archive_enabled {
            let (tx, rx) = sync_channel(1);
            (Some(tx), Some(rx))
        } else {
            (None, None)
        };
        let handle = Self {
            inner: Arc::new(RecorderShared {
                active: AtomicBool::new(config.active),
                config,
                root,
                live: Mutex::new(LiveStatus::default()),
                archive_tx,
            }),
        };
        handle.recover_partials();
        handle.recover_metadata_partials();
        handle.cleanup_retention();
        if let Some(rx) = archive_rx {
            spawn_archive_worker(&handle.inner, rx);
        }
        Ok(handle)
    }

    pub fn is_active(&self) -> bool {
        self.inner.active.load(Ordering::Relaxed)
    }

    pub fn set_active(&self, active: bool) {
        self.inner.active.store(active, Ordering::Relaxed);
    }

    pub fn config(&self) -> &CfgRecording {
        &self.inner.config
    }

    pub fn root(&self) -> &Path {
        &self.inner.root
    }

    pub(crate) fn set_active_calls(&self, mut ids: Vec<u16>) {
        let active_sessions = ids.len();
        ids.sort_unstable();
        ids.dedup();
        if let Ok(mut live) = self.inner.live.lock() {
            live.active_sessions = active_sessions;
            live.active_call_ids = ids;
        }
    }

    pub(crate) fn note_completed(&self, id: String) {
        if let Ok(mut live) = self.inner.live.lock() {
            live.last_recording_id = Some(id);
            live.last_error = None;
        }
        if let Some(tx) = &self.inner.archive_tx {
            let _ = tx.try_send(());
        }
    }

    pub(crate) fn note_error(&self, error: impl Into<String>) {
        let error = error.into();
        tracing::error!("Recorder: {}", error);
        if let Ok(mut live) = self.inner.live.lock() {
            live.last_error = Some(error);
        }
    }

    pub fn should_record(&self, destination_id: u32, destination_is_group: bool) -> bool {
        if !self.is_active() {
            return false;
        }
        match self.inner.config.mode {
            RecordingMode::All => true,
            RecordingMode::SelectedGroups => destination_is_group && self.inner.config.selected_groups.binary_search(&destination_id).is_ok(),
        }
    }

    pub fn has_minimum_free_space(&self) -> bool {
        let required = self.inner.config.minimum_free_space_mb.saturating_mul(1024 * 1024);
        available_space(&self.inner.root).map(|free| free >= required).unwrap_or(false)
    }

    pub fn status(&self) -> RecorderStatus {
        let recordings = self.scan_recordings();
        let (
            active_sessions,
            active_call_ids,
            last_recording_id,
            last_error,
            archive_available,
            archive_active,
            archive_pending,
            archive_completed,
            archive_last_success_at,
            archive_last_error,
        ) = self
            .inner
            .live
            .lock()
            .map(|live| {
                (
                    live.active_sessions,
                    live.active_call_ids.clone(),
                    live.last_recording_id.clone(),
                    live.last_error.clone(),
                    live.archive_available,
                    live.archive_active,
                    live.archive_pending,
                    live.archive_completed,
                    live.archive_last_success_at.clone(),
                    live.archive_last_error.clone(),
                )
            })
            .unwrap_or_default();
        RecorderStatus {
            available: true,
            active: self.is_active(),
            directory: self.inner.root.display().to_string(),
            mode: match self.inner.config.mode {
                RecordingMode::All => "all",
                RecordingMode::SelectedGroups => "selected_groups",
            }
            .to_string(),
            selected_groups: self.inner.config.selected_groups.clone(),
            minimum_free_space_mb: self.inner.config.minimum_free_space_mb,
            free_space_bytes: available_space(&self.inner.root),
            used_bytes: directory_size(&self.inner.root),
            recording_count: recordings.len(),
            active_sessions,
            active_call_ids,
            last_recording_id,
            last_error,
            archive_enabled: self.inner.config.archive_enabled,
            archive_directory: self.inner.config.archive_directory.clone(),
            archive_available,
            archive_active,
            archive_pending,
            archive_completed,
            archive_last_success_at,
            archive_last_error,
        }
    }

    pub fn list_recordings(&self, limit: Option<usize>) -> Vec<RecordingMetadata> {
        let mut metadata = self.scan_recordings();
        metadata.truncate(limit.unwrap_or(self.inner.config.max_list_entries).min(self.inner.config.max_list_entries));
        metadata
    }

    /// Import a finished 8-kHz mono PCM WAV into the local recording library.
    /// The WAV and JSON sidecar are written exactly like normal call recordings,
    /// so playback, deletion, retention and NFS archiving use the same code path.
    pub fn import_named_wav(&self, source: &Path, title: &str, origin: &str) -> Result<RecordingMetadata, String> {
        if !self.has_minimum_free_space() {
            return Err(format!(
                "minimum free space threshold reached ({} MiB)",
                self.inner.config.minimum_free_space_mb
            ));
        }
        let title = normalize_library_title(title)?;
        let origin = normalize_library_origin(origin)?;
        let (duration_ms, audio_bytes) = inspect_recording_wav(source)?;
        let now = chrono::Local::now();
        let id = Uuid::new_v4().to_string();
        let day_dir = self
            .root()
            .join(now.format("%Y").to_string())
            .join(now.format("%m").to_string())
            .join(now.format("%d").to_string());
        fs::create_dir_all(&day_dir).map_err(|error| format!("cannot create {}: {error}", day_dir.display()))?;
        let safe_title = library_filename_component(&title);
        let stem = format!(
            "{}-{}_{}_{}",
            origin.to_uppercase(),
            safe_title,
            now.format("%Y-%m-%d_%H-%M-%S"),
            id
        );
        let final_audio_path = day_dir.join(format!("{stem}.wav"));
        let part_audio_path = day_dir.join(format!("{stem}.wav.part"));
        let final_metadata_path = day_dir.join(format!("{stem}.json"));

        let result = (|| -> Result<RecordingMetadata, String> {
            fs::copy(source, &part_audio_path).map_err(|error| {
                format!(
                    "cannot copy generated WAV {} -> {}: {error}",
                    source.display(),
                    part_audio_path.display()
                )
            })?;
            OpenOptions::new()
                .write(true)
                .open(&part_audio_path)
                .and_then(|file| file.sync_all())
                .map_err(|error| format!("cannot sync {}: {error}", part_audio_path.display()))?;
            fs::rename(&part_audio_path, &final_audio_path).map_err(|error| {
                format!(
                    "cannot publish generated WAV {} -> {}: {error}",
                    part_audio_path.display(),
                    final_audio_path.display()
                )
            })?;

            let relative_audio_path = final_audio_path
                .strip_prefix(self.root())
                .map_err(|error| error.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            let timestamp = now.to_rfc3339();
            let metadata = RecordingMetadata {
                schema_version: 1,
                id: id.clone(),
                title: Some(title.clone()),
                origin: Some(origin.clone()),
                call_id: 0,
                source_issi: 0,
                destination_id: 0,
                destination_type: "library".to_string(),
                started_at: timestamp.clone(),
                ended_at: timestamp,
                duration_ms,
                audio_bytes,
                relative_audio_path,
                recovered_after_unclean_shutdown: false,
                segments: Vec::new(),
            };
            write_recording_metadata_atomic(&final_metadata_path, &metadata)?;
            Ok(metadata)
        })();

        match result {
            Ok(metadata) => {
                self.note_completed(metadata.id.clone());
                tracing::info!(
                    "Recorder: imported {} WAV title={} id={} duration_ms={} path={}",
                    origin,
                    title,
                    metadata.id,
                    metadata.duration_ms,
                    final_audio_path.display()
                );
                Ok(metadata)
            }
            Err(error) => {
                let _ = fs::remove_file(&part_audio_path);
                let _ = fs::remove_file(&final_audio_path);
                let _ = fs::remove_file(&final_metadata_path);
                Err(error)
            }
        }
    }

    pub(super) fn scan_recordings(&self) -> Vec<RecordingMetadata> {
        let mut metadata = Vec::new();
        let mut files = Vec::new();
        collect_files_with_suffix(&self.inner.root, ".json", &mut files);
        for path in files {
            match fs::read_to_string(&path)
                .ok()
                .and_then(|body| serde_json::from_str::<RecordingMetadata>(&body).ok())
            {
                Some(item) => metadata.push(item),
                None => tracing::warn!("Recorder: ignoring invalid metadata {}", path.display()),
            }
        }
        metadata.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        metadata
    }

    pub fn find_recording(&self, id: &str) -> Option<RecordingMetadata> {
        if !valid_id(id) {
            return None;
        }
        self.scan_recordings().into_iter().find(|item| item.id == id)
    }

    pub fn audio_path(&self, id: &str) -> Result<PathBuf, String> {
        let metadata = self.find_recording(id).ok_or_else(|| "recording not found".to_string())?;
        let relative = safe_relative_path(&metadata.relative_audio_path)?;
        let path = self.inner.root.join(relative);
        let canonical_root = self.inner.root.canonicalize().map_err(|e| e.to_string())?;
        let canonical_path = path.canonicalize().map_err(|e| e.to_string())?;
        if !canonical_path.starts_with(&canonical_root) {
            return Err("recording path escapes configured root".to_string());
        }
        Ok(canonical_path)
    }

    pub fn delete_recording(&self, id: &str) -> Result<(), String> {
        if !valid_id(id) {
            return Err("invalid recording id".to_string());
        }
        let metadata = self.find_recording(id).ok_or_else(|| "recording not found".to_string())?;
        let audio = self.audio_path(id)?;
        let json = audio.with_extension("json");
        let archived = audio.with_extension("archived");
        if audio.exists() {
            fs::remove_file(&audio).map_err(|e| format!("failed to delete {}: {e}", audio.display()))?;
        }
        if json.exists() {
            fs::remove_file(&json).map_err(|e| format!("failed to delete {}: {e}", json.display()))?;
        } else {
            // Metadata may not share the WAV stem if manually imported; locate it by id.
            let mut files = Vec::new();
            collect_files_with_suffix(&self.inner.root, ".json", &mut files);
            for path in files {
                if fs::read_to_string(&path)
                    .ok()
                    .and_then(|body| serde_json::from_str::<RecordingMetadata>(&body).ok())
                    .is_some_and(|item| item.id == metadata.id)
                {
                    let _ = fs::remove_file(path);
                    break;
                }
            }
        }
        if archived.exists() {
            fs::remove_file(&archived).map_err(|e| format!("failed to delete {}: {e}", archived.display()))?;
        }
        Ok(())
    }

    fn recover_partials(&self) {
        let mut parts = Vec::new();
        collect_files_with_suffix(&self.inner.root, ".wav.part", &mut parts);
        for part in parts {
            let final_path = PathBuf::from(part.to_string_lossy().trim_end_matches(".part"));
            match recover_part(&part, &final_path) {
                Ok(data_bytes) => {
                    let json_part = PathBuf::from(format!("{}.json.part", final_path.with_extension("").display()));
                    let json_final = final_path.with_extension("json");
                    let metadata_recovered = if let Ok(body) = fs::read_to_string(&json_part)
                        && let Ok(mut metadata) = serde_json::from_str::<RecordingMetadata>(&body)
                    {
                        metadata.recovered_after_unclean_shutdown = true;
                        metadata.ended_at = chrono::Local::now().to_rfc3339();
                        metadata.audio_bytes = data_bytes;
                        metadata.duration_ms = (data_bytes / 2).saturating_mul(1000) / 8_000;
                        if let Some(last) = metadata.segments.last_mut() {
                            last.end_ms = metadata.duration_ms;
                        }
                        serde_json::to_vec_pretty(&metadata)
                            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
                            .and_then(|final_body| fs::write(&json_final, final_body))
                            .is_ok()
                    } else {
                        false
                    };
                    if metadata_recovered {
                        let _ = fs::remove_file(&json_part);
                    }
                    tracing::warn!("Recorder: recovered partial WAV {}", final_path.display());
                }
                Err(e) => self.note_error(format!("failed to recover {}: {e}", part.display())),
            }
        }
    }

    fn recover_metadata_partials(&self) {
        let mut parts = Vec::new();
        collect_files_with_suffix(&self.inner.root, ".json.part", &mut parts);
        for json_part in parts {
            let stem = json_part.to_string_lossy().trim_end_matches(".json.part").to_string();
            let wav_path = PathBuf::from(format!("{stem}.wav"));
            if !wav_path.is_file() {
                continue;
            }
            let data_bytes = wav_path.metadata().map(|m| m.len().saturating_sub(44)).unwrap_or(0);
            let Ok(body) = fs::read_to_string(&json_part) else { continue };
            let Ok(mut metadata) = serde_json::from_str::<RecordingMetadata>(&body) else { continue };
            metadata.recovered_after_unclean_shutdown = true;
            metadata.ended_at = chrono::Local::now().to_rfc3339();
            metadata.audio_bytes = data_bytes;
            metadata.duration_ms = (data_bytes / 2).saturating_mul(1000) / 8_000;
            if let Some(last) = metadata.segments.last_mut() {
                last.end_ms = metadata.duration_ms;
            }
            let json_final = PathBuf::from(format!("{stem}.json"));
            match serde_json::to_vec_pretty(&metadata)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
                .and_then(|body| fs::write(&json_final, body))
            {
                Ok(()) => {
                    let _ = fs::remove_file(&json_part);
                    tracing::warn!("Recorder: recovered metadata {}", json_final.display());
                }
                Err(e) => self.note_error(format!("failed to recover {}: {e}", json_part.display())),
            }
        }
    }

    fn cleanup_retention(&self) {
        let days = self.inner.config.retention_days;
        if days == 0 {
            return;
        }
        let cutoff = std::time::SystemTime::now()
            .checked_sub(std::time::Duration::from_secs(days as u64 * 86_400))
            .unwrap_or(std::time::UNIX_EPOCH);
        for item in self.scan_recordings() {
            let Ok(audio) = self.audio_path(&item.id) else { continue };
            let modified = audio.metadata().and_then(|m| m.modified()).unwrap_or(std::time::SystemTime::now());
            if modified < cutoff {
                if self.inner.config.archive_enabled && !recording_is_archived(&self.inner, &item) {
                    tracing::warn!(
                        "Recorder: retention kept unarchived recording id={} because archive copy is not confirmed",
                        item.id
                    );
                    continue;
                }
                if let Err(e) = self.delete_recording(&item.id) {
                    self.note_error(format!("retention cleanup failed for {}: {e}", item.id));
                }
            }
        }
    }
}


impl RecorderShared {
    pub(super) fn update_archive_status(&self, update: impl FnOnce(&mut LiveStatus)) {
        if let Ok(mut live) = self.live.lock() {
            update(&mut live);
        }
    }

    pub(super) fn scan_recordings(&self) -> Vec<RecordingMetadata> {
        let mut metadata = Vec::new();
        let mut files = Vec::new();
        collect_files_with_suffix(&self.root, ".json", &mut files);
        for path in files {
            match fs::read_to_string(&path)
                .ok()
                .and_then(|body| serde_json::from_str::<RecordingMetadata>(&body).ok())
            {
                Some(item) => metadata.push(item),
                None => tracing::warn!("Recorder: ignoring invalid metadata {}", path.display()),
            }
        }
        metadata.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        metadata
    }
}

fn normalize_library_title(title: &str) -> Result<String, String> {
    let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
    let count = title.chars().count();
    if count == 0 || count > 120 {
        return Err("recording name must contain 1-120 characters".to_string());
    }
    if title.chars().any(char::is_control) {
        return Err("recording name contains invalid control characters".to_string());
    }
    Ok(title)
}

fn normalize_library_origin(origin: &str) -> Result<String, String> {
    let origin = origin.trim().to_ascii_lowercase();
    if origin.is_empty()
        || origin.len() > 24
        || !origin.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Err("invalid recording origin".to_string());
    }
    Ok(origin)
}

fn library_filename_component(title: &str) -> String {
    let mut out = String::new();
    let mut separator = false;
    for ch in title.chars() {
        if ch.is_alphanumeric() || matches!(ch, '-' | '_') {
            out.push(ch);
            separator = false;
        } else if !separator {
            out.push('_');
            separator = true;
        }
        if out.chars().count() >= 80 {
            break;
        }
    }
    let out = out.trim_matches('_');
    if out.is_empty() {
        "Durchsage".to_string()
    } else {
        out.to_string()
    }
}

fn inspect_recording_wav(path: &Path) -> Result<(u64, u64), String> {
    let mut file = File::open(path).map_err(|error| format!("cannot open generated WAV {}: {error}", path.display()))?;
    let mut header = [0u8; 44];
    file.read_exact(&mut header)
        .map_err(|error| format!("cannot read generated WAV header {}: {error}", path.display()))?;
    let pcm_format = u16::from_le_bytes([header[20], header[21]]);
    let channels = u16::from_le_bytes([header[22], header[23]]);
    let sample_rate = u32::from_le_bytes([header[24], header[25], header[26], header[27]]);
    let bits_per_sample = u16::from_le_bytes([header[34], header[35]]);
    let data_bytes = u32::from_le_bytes([header[40], header[41], header[42], header[43]]) as u64;
    if &header[0..4] != b"RIFF"
        || &header[8..12] != b"WAVE"
        || &header[12..16] != b"fmt "
        || &header[36..40] != b"data"
        || pcm_format != 1
        || channels != 1
        || sample_rate != 8_000
        || bits_per_sample != 16
    {
        return Err("generated WAV is not canonical PCM s16le/mono/8000Hz".to_string());
    }
    let file_len = file.metadata().map_err(|error| error.to_string())?.len();
    if data_bytes == 0 || file_len < 44u64.saturating_add(data_bytes) || data_bytes % 2 != 0 {
        return Err("generated WAV has an invalid or empty data chunk".to_string());
    }
    let samples = data_bytes / 2;
    let duration_ms = samples.saturating_mul(1000) / 8_000;
    Ok((duration_ms, data_bytes))
}

fn write_recording_metadata_atomic(path: &Path, metadata: &RecordingMetadata) -> Result<(), String> {
    let body = serde_json::to_vec_pretty(metadata).map_err(|error| error.to_string())?;
    let tmp = PathBuf::from(format!("{}.tmp", path.display()));
    fs::write(&tmp, body).map_err(|error| format!("cannot write {}: {error}", tmp.display()))?;
    OpenOptions::new()
        .write(true)
        .open(&tmp)
        .and_then(|file| file.sync_all())
        .map_err(|error| format!("cannot sync {}: {error}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|error| format!("cannot rename {} -> {}: {error}", tmp.display(), path.display()))
}

fn valid_id(id: &str) -> bool {
    id.len() == 36 && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

pub(super) fn safe_relative_path(path: &str) -> Result<PathBuf, String> {
    let path = Path::new(path);
    if path.is_absolute() {
        return Err("absolute recording path rejected".to_string());
    }
    let mut clean = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => clean.push(part),
            _ => return Err("invalid recording path".to_string()),
        }
    }
    Ok(clean)
}

fn collect_files_with_suffix(root: &Path, suffix: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else { return };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else { continue };
        if file_type.is_symlink() {
            continue;
        }
        let path = entry.path();
        if file_type.is_dir() {
            collect_files_with_suffix(&path, suffix, out);
        } else if file_type.is_file() && path.to_string_lossy().ends_with(suffix) {
            out.push(path);
        }
    }
}

fn directory_size(root: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(root) else { return 0 };
    entries
        .flatten()
        .map(|entry| {
            let Ok(file_type) = entry.file_type() else { return 0 };
            if file_type.is_symlink() {
                return 0;
            }
            let path = entry.path();
            if file_type.is_dir() {
                directory_size(&path)
            } else if file_type.is_file() {
                entry.metadata().map(|m| m.len()).unwrap_or(0)
            } else {
                0
            }
        })
        .sum()
}

fn available_space(path: &Path) -> Option<u64> {
    let c_path = CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    let rc = unsafe { libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) };
    if rc != 0 {
        return None;
    }
    let stat = unsafe { stat.assume_init() };
    Some((stat.f_bavail as u64).saturating_mul(stat.f_frsize as u64))
}

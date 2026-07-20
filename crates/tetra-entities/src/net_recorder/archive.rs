use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Weak};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tetra_config::bluestation::CfgRecording;

use super::service::RecorderShared;
use super::types::RecordingMetadata;

pub(super) fn spawn_archive_worker(shared: &Arc<RecorderShared>, rx: Receiver<()>) {
    let weak = Arc::downgrade(shared);
    if let Err(error) = std::thread::Builder::new()
        .name("netcore-recording-archive".to_string())
        .spawn(move || archive_worker(weak, rx))
    {
        tracing::error!("Recorder archive: failed to start worker: {error}");
    }
}

fn archive_worker(shared: Weak<RecorderShared>, rx: Receiver<()>) {
    let mut run_immediately = true;
    loop {
        let Some(inner) = shared.upgrade() else {
            return;
        };
        let retry = Duration::from_secs(inner.config.archive_retry_seconds.max(1));
        drop(inner);

        if !run_immediately {
            match rx.recv_timeout(retry) {
                Ok(()) | Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
        run_immediately = false;

        let Some(inner) = shared.upgrade() else {
            return;
        };
        run_archive_cycle(&inner);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArchiveKind {
    Recording,
    Tts,
}

impl ArchiveKind {
    fn label(self) -> &'static str {
        match self {
            Self::Recording => "recording",
            Self::Tts => "TTS WAV",
        }
    }
}

#[derive(Debug, Clone)]
struct ArchiveTarget {
    kind: ArchiveKind,
    root: PathBuf,
}

fn archive_target(config: &CfgRecording, metadata: &RecordingMetadata) -> Option<ArchiveTarget> {
    if is_tts_recording(metadata) {
        config.tts_archive_enabled.then(|| ArchiveTarget {
            kind: ArchiveKind::Tts,
            root: PathBuf::from(&config.tts_archive_directory),
        })
    } else {
        config.archive_enabled.then(|| ArchiveTarget {
            kind: ArchiveKind::Recording,
            root: PathBuf::from(&config.archive_directory),
        })
    }
}

fn is_tts_recording(metadata: &RecordingMetadata) -> bool {
    metadata
        .origin
        .as_deref()
        .is_some_and(|origin| origin.eq_ignore_ascii_case("tts"))
}

pub(super) fn recording_requires_archive(inner: &RecorderShared, metadata: &RecordingMetadata) -> bool {
    archive_target(&inner.config, metadata).is_some()
}

fn run_archive_cycle(inner: &RecorderShared) {
    if !inner.config.archive_enabled && !inner.config.tts_archive_enabled {
        return;
    }

    inner.update_archive_status(|status| {
        status.archive_active = true;
    });

    let recordings = inner
        .scan_recordings()
        .into_iter()
        .filter_map(|metadata| archive_target(&inner.config, &metadata).map(|target| (metadata, target)))
        .collect::<Vec<_>>();

    let mut root_availability: HashMap<PathBuf, Result<(), String>> = HashMap::new();
    if inner.config.archive_enabled {
        let root = PathBuf::from(&inner.config.archive_directory);
        root_availability.insert(root.clone(), verify_archive_root(&root));
    }
    if inner.config.tts_archive_enabled {
        let root = PathBuf::from(&inner.config.tts_archive_directory);
        root_availability
            .entry(root.clone())
            .or_insert_with(|| verify_archive_root(&root));
    }

    for error in root_availability.values().filter_map(|result| result.as_ref().err()) {
        tracing::warn!("Recorder archive: {error}");
    }

    let mut completed = 0usize;
    let mut pending = 0usize;
    let mut all_roots_available = root_availability.values().all(Result::is_ok);
    let mut last_success = None;
    let mut last_error = root_availability.values().find_map(|result| result.as_ref().err().cloned());

    for (metadata, target) in recordings {
        let availability = root_availability
            .entry(target.root.clone())
            .or_insert_with(|| verify_archive_root(&target.root));
        if let Err(error) = availability {
            all_roots_available = false;
            pending += 1;
            let detail = format!("{} {}: {error}", target.kind.label(), metadata.id);
            last_error = Some(detail.clone());
            tracing::warn!("Recorder archive: {detail}");
            continue;
        }

        match archive_one(inner, &target, &metadata) {
            Ok(ArchiveOutcome::AlreadyPresent) => {
                completed += 1;
                if let Err(error) = write_archive_marker(inner, &metadata, &target.root) {
                    pending += 1;
                    completed = completed.saturating_sub(1);
                    last_error = Some(format!("{}: {error}", metadata.id));
                }
            }
            Ok(ArchiveOutcome::Copied) => {
                if let Err(error) = write_archive_marker(inner, &metadata, &target.root) {
                    pending += 1;
                    last_error = Some(format!("{}: {error}", metadata.id));
                    continue;
                }
                completed += 1;
                let now = chrono::Local::now().to_rfc3339();
                last_success = Some(now);
                tracing::info!(
                    "Recorder archive: copied {} id={} to {}",
                    target.kind.label(),
                    metadata.id,
                    target.root.display()
                );
            }
            Err(error) => {
                pending += 1;
                last_error = Some(format!("{}: {error}", metadata.id));
                tracing::warn!(
                    "Recorder archive: failed {} id={}: {error}",
                    target.kind.label(),
                    metadata.id
                );
            }
        }
    }

    inner.update_archive_status(|status| {
        status.archive_active = false;
        status.archive_available = all_roots_available;
        status.archive_pending = pending;
        status.archive_completed = completed;
        if let Some(value) = last_success {
            status.archive_last_success_at = Some(value);
        }
        status.archive_last_error = last_error;
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArchiveOutcome {
    AlreadyPresent,
    Copied,
}

#[derive(Debug, Serialize, Deserialize)]
struct ArchiveMarker {
    schema_version: u8,
    recording_id: String,
    archived_at: String,
    archive_directory: String,
    audio_bytes: u64,
    metadata_bytes: u64,
}

pub(super) fn recording_is_archived(inner: &RecorderShared, metadata: &RecordingMetadata) -> bool {
    let Some(target) = archive_target(&inner.config, metadata) else {
        return true;
    };
    let Ok(relative_audio) = super::service::safe_relative_path(&metadata.relative_audio_path) else {
        return false;
    };
    let source_audio = inner.root.join(&relative_audio);
    let source_json = source_audio.with_extension("json");
    let marker_path = source_audio.with_extension("archived");
    let Ok(body) = fs::read_to_string(marker_path) else {
        return false;
    };
    let Ok(marker) = serde_json::from_str::<ArchiveMarker>(&body) else {
        return false;
    };
    let audio_bytes = source_audio.metadata().map(|value| value.len()).ok();
    let metadata_bytes = source_json.metadata().map(|value| value.len()).ok();
    let target_directory = target.root.to_string_lossy();
    marker.schema_version == 1
        && !marker.archived_at.is_empty()
        && marker.recording_id == metadata.id
        && marker.archive_directory.as_str() == target_directory.as_ref()
        && Some(marker.audio_bytes) == audio_bytes
        && Some(marker.metadata_bytes) == metadata_bytes
}

fn write_archive_marker(
    inner: &RecorderShared,
    metadata: &RecordingMetadata,
    archive_root: &Path,
) -> Result<(), String> {
    let relative_audio = super::service::safe_relative_path(&metadata.relative_audio_path)?;
    let source_audio = inner.root.join(relative_audio);
    let source_json = source_audio.with_extension("json");
    let marker_path = source_audio.with_extension("archived");
    let marker = ArchiveMarker {
        schema_version: 1,
        recording_id: metadata.id.clone(),
        archived_at: chrono::Local::now().to_rfc3339(),
        archive_directory: archive_root.to_string_lossy().into_owned(),
        audio_bytes: source_audio
            .metadata()
            .map_err(|error| format!("cannot stat {}: {error}", source_audio.display()))?
            .len(),
        metadata_bytes: source_json
            .metadata()
            .map_err(|error| format!("cannot stat {}: {error}", source_json.display()))?
            .len(),
    };
    let body = serde_json::to_vec_pretty(&marker).map_err(|error| error.to_string())?;
    let tmp = PathBuf::from(format!("{}.tmp", marker_path.display()));
    fs::write(&tmp, body).map_err(|error| format!("cannot write {}: {error}", tmp.display()))?;
    fs::rename(&tmp, &marker_path)
        .map_err(|error| format!("cannot rename {} -> {}: {error}", tmp.display(), marker_path.display()))?;
    Ok(())
}

fn archive_one(
    inner: &RecorderShared,
    target: &ArchiveTarget,
    metadata: &RecordingMetadata,
) -> Result<ArchiveOutcome, String> {
    let relative_audio = super::service::safe_relative_path(&metadata.relative_audio_path)?;
    let source_audio = inner.root.join(&relative_audio);
    let source_json = source_audio.with_extension("json");
    if !source_audio.is_file() {
        return Err(format!("source WAV missing: {}", source_audio.display()));
    }
    if !source_json.is_file() {
        return Err(format!("source metadata missing: {}", source_json.display()));
    }
    let canonical_local_root = inner
        .root
        .canonicalize()
        .map_err(|error| format!("cannot resolve local recording root: {error}"))?;
    let canonical_source_audio = source_audio
        .canonicalize()
        .map_err(|error| format!("cannot resolve {}: {error}", source_audio.display()))?;
    let canonical_source_json = source_json
        .canonicalize()
        .map_err(|error| format!("cannot resolve {}: {error}", source_json.display()))?;
    if !canonical_source_audio.starts_with(&canonical_local_root)
        || !canonical_source_json.starts_with(&canonical_local_root)
    {
        return Err("source recording path escapes configured local root".to_string());
    }

    let destination_relative = archive_relative_audio(target.kind, &relative_audio)?;
    let destination_audio = target.root.join(destination_relative);
    let destination_json = destination_audio.with_extension("json");
    if files_match(&source_audio, &destination_audio) && files_match(&source_json, &destination_json) {
        return Ok(ArchiveOutcome::AlreadyPresent);
    }

    let parent = destination_audio
        .parent()
        .ok_or_else(|| "archive destination has no parent directory".to_string())?;
    fs::create_dir_all(parent).map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    let canonical_archive_root = target
        .root
        .canonicalize()
        .map_err(|error| format!("cannot resolve archive root: {error}"))?;
    let canonical_parent = parent
        .canonicalize()
        .map_err(|error| format!("cannot resolve {}: {error}", parent.display()))?;
    if !canonical_parent.starts_with(&canonical_archive_root) {
        return Err("archive destination escapes configured archive root".to_string());
    }

    copy_atomic(&canonical_source_audio, &destination_audio, &metadata.id)?;
    // JSON is copied last and therefore acts as the completion marker for one item.
    copy_atomic(&canonical_source_json, &destination_json, &metadata.id)?;

    if !files_match(&source_audio, &destination_audio) || !files_match(&source_json, &destination_json) {
        return Err("archive verification failed after copy".to_string());
    }
    Ok(ArchiveOutcome::Copied)
}

fn archive_relative_audio(kind: ArchiveKind, relative_audio: &Path) -> Result<PathBuf, String> {
    match kind {
        ArchiveKind::Recording => Ok(relative_audio.to_path_buf()),
        ArchiveKind::Tts => relative_audio
            .file_name()
            .map(PathBuf::from)
            .ok_or_else(|| "TTS archive source has no filename".to_string()),
    }
}

fn verify_archive_root(root: &Path) -> Result<(), String> {
    if !root.is_dir() {
        return Err(format!(
            "archive directory is unavailable or not a directory: {}",
            root.display()
        ));
    }
    let probe = root.join(format!(".netcore-write-test-{}", std::process::id()));
    let result = (|| -> io::Result<()> {
        let mut file = OpenOptions::new().write(true).create_new(true).open(&probe)?;
        file.write_all(b"netcore")?;
        file.sync_all()?;
        fs::remove_file(&probe)?;
        Ok(())
    })();
    if probe.exists() {
        let _ = fs::remove_file(&probe);
    }
    result.map_err(|error| format!("archive directory is not writable: {}: {error}", root.display()))
}

fn files_match(source: &Path, destination: &Path) -> bool {
    let Ok(source_meta) = source.metadata() else {
        return false;
    };
    let Ok(destination_meta) = destination.metadata() else {
        return false;
    };
    source_meta.is_file() && destination_meta.is_file() && source_meta.len() == destination_meta.len()
}

fn copy_atomic(source: &Path, destination: &Path, id: &str) -> Result<(), String> {
    let file_name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| format!("invalid archive filename: {}", destination.display()))?;
    let tmp = destination.with_file_name(format!(".{file_name}.{id}.part"));
    if tmp.exists() {
        fs::remove_file(&tmp).map_err(|error| format!("cannot remove stale {}: {error}", tmp.display()))?;
    }
    fs::copy(source, &tmp)
        .map_err(|error| format!("copy {} -> {} failed: {error}", source.display(), tmp.display()))?;
    let file = OpenOptions::new()
        .write(true)
        .open(&tmp)
        .map_err(|error| format!("cannot reopen {} for sync: {error}", tmp.display()))?;
    file.sync_all()
        .map_err(|error| format!("sync {} failed: {error}", tmp.display()))?;
    fs::rename(&tmp, destination)
        .map_err(|error| format!("rename {} -> {} failed: {error}", tmp.display(), destination.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata(origin: Option<&str>) -> RecordingMetadata {
        RecordingMetadata {
            schema_version: 1,
            id: "test-id".to_string(),
            title: Some("Test".to_string()),
            origin: origin.map(|value| value.to_string()),
            call_id: 0,
            source_issi: 0,
            destination_id: 0,
            destination_type: "library".to_string(),
            started_at: String::new(),
            ended_at: String::new(),
            duration_ms: 0,
            audio_bytes: 0,
            relative_audio_path: "2026/07/20/TTS-Test.wav".to_string(),
            recovered_after_unclean_shutdown: false,
            segments: Vec::new(),
        }
    }

    #[test]
    fn recognizes_tts_origin_case_insensitively() {
        assert!(is_tts_recording(&metadata(Some("TTS"))));
        assert!(!is_tts_recording(&metadata(Some("recording"))));
        assert!(!is_tts_recording(&metadata(None)));
    }

    #[test]
    fn routes_tts_and_recordings_to_different_roots() {
        let mut config = CfgRecording::default();
        config.archive_enabled = true;
        config.archive_directory = "/mnt/nfs-share/Recordings".to_string();
        config.tts_archive_enabled = true;
        config.tts_archive_directory = "/mnt/nfs-share/TTS-Dateien".to_string();

        let tts = archive_target(&config, &metadata(Some("tts"))).unwrap();
        let recording = archive_target(&config, &metadata(None)).unwrap();
        assert_eq!(tts.kind, ArchiveKind::Tts);
        assert_eq!(tts.root, PathBuf::from("/mnt/nfs-share/TTS-Dateien"));
        assert_eq!(recording.kind, ArchiveKind::Recording);
        assert_eq!(recording.root, PathBuf::from("/mnt/nfs-share/Recordings"));
    }

    #[test]
    fn tts_archive_is_flat_while_recordings_keep_date_tree() {
        let path = Path::new("2026/07/20/TTS-Test.wav");
        assert_eq!(
            archive_relative_audio(ArchiveKind::Tts, path).unwrap(),
            PathBuf::from("TTS-Test.wav")
        );
        assert_eq!(
            archive_relative_audio(ArchiveKind::Recording, path).unwrap(),
            PathBuf::from("2026/07/20/TTS-Test.wav")
        );
    }
}

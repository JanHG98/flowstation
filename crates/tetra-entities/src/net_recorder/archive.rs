use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Weak};
use std::time::Duration;

use super::service::RecorderShared;
use serde::{Deserialize, Serialize};

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

fn run_archive_cycle(inner: &RecorderShared) {
    if !inner.config.archive_enabled {
        return;
    }

    inner.update_archive_status(|status| {
        status.archive_active = true;
    });

    let recordings = inner.scan_recordings();
    let archive_root = PathBuf::from(&inner.config.archive_directory);
    let availability = verify_archive_root(&archive_root);
    if let Err(error) = availability {
        let completed = recordings.iter().filter(|metadata| recording_is_archived(inner, metadata)).count();
        inner.update_archive_status(|status| {
            status.archive_active = false;
            status.archive_available = false;
            status.archive_pending = recordings.len().saturating_sub(completed);
            status.archive_completed = completed;
            status.archive_last_error = Some(error.clone());
        });
        tracing::warn!("Recorder archive: {error}");
        return;
    }

    let mut completed = 0usize;
    let mut pending = 0usize;
    let mut last_success = None;
    let mut last_error = None;

    for metadata in recordings {
        match archive_one(inner, &archive_root, &metadata) {
            Ok(ArchiveOutcome::AlreadyPresent) => {
                completed += 1;
                if let Err(error) = write_archive_marker(inner, &metadata) {
                    pending += 1;
                    completed = completed.saturating_sub(1);
                    last_error = Some(format!("{}: {error}", metadata.id));
                }
            }
            Ok(ArchiveOutcome::Copied) => {
                if let Err(error) = write_archive_marker(inner, &metadata) {
                    pending += 1;
                    last_error = Some(format!("{}: {error}", metadata.id));
                    continue;
                }
                completed += 1;
                let now = chrono::Local::now().to_rfc3339();
                last_success = Some(now.clone());
                tracing::info!(
                    "Recorder archive: copied recording id={} to {}",
                    metadata.id,
                    archive_root.display()
                );
            }
            Err(error) => {
                pending += 1;
                last_error = Some(format!("{}: {error}", metadata.id));
                tracing::warn!("Recorder archive: failed recording id={}: {error}", metadata.id);
            }
        }
    }

    inner.update_archive_status(|status| {
        status.archive_active = false;
        status.archive_available = true;
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
    if !inner.config.archive_enabled {
        return true;
    }
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
    marker.schema_version == 1
        && !marker.archived_at.is_empty()
        && marker.recording_id == metadata.id
        && marker.archive_directory == inner.config.archive_directory
        && Some(marker.audio_bytes) == audio_bytes
        && Some(marker.metadata_bytes) == metadata_bytes
}

fn write_archive_marker(inner: &RecorderShared, metadata: &RecordingMetadata) -> Result<(), String> {
    let relative_audio = super::service::safe_relative_path(&metadata.relative_audio_path)?;
    let source_audio = inner.root.join(relative_audio);
    let source_json = source_audio.with_extension("json");
    let marker_path = source_audio.with_extension("archived");
    let marker = ArchiveMarker {
        schema_version: 1,
        recording_id: metadata.id.clone(),
        archived_at: chrono::Local::now().to_rfc3339(),
        archive_directory: inner.config.archive_directory.clone(),
        audio_bytes: source_audio.metadata().map_err(|error| format!("cannot stat {}: {error}", source_audio.display()))?.len(),
        metadata_bytes: source_json.metadata().map_err(|error| format!("cannot stat {}: {error}", source_json.display()))?.len(),
    };
    let body = serde_json::to_vec_pretty(&marker).map_err(|error| error.to_string())?;
    let tmp = PathBuf::from(format!("{}.tmp", marker_path.display()));
    fs::write(&tmp, body).map_err(|error| format!("cannot write {}: {error}", tmp.display()))?;
    fs::rename(&tmp, &marker_path).map_err(|error| format!("cannot rename {} -> {}: {error}", tmp.display(), marker_path.display()))?;
    Ok(())
}

fn archive_one(inner: &RecorderShared, archive_root: &Path, metadata: &RecordingMetadata) -> Result<ArchiveOutcome, String> {
    let relative_audio = super::service::safe_relative_path(&metadata.relative_audio_path)?;
    let source_audio = inner.root.join(&relative_audio);
    let source_json = source_audio.with_extension("json");
    if !source_audio.is_file() {
        return Err(format!("source WAV missing: {}", source_audio.display()));
    }
    if !source_json.is_file() {
        return Err(format!("source metadata missing: {}", source_json.display()));
    }
    let canonical_local_root = inner.root.canonicalize().map_err(|error| format!("cannot resolve local recording root: {error}"))?;
    let canonical_source_audio = source_audio.canonicalize().map_err(|error| format!("cannot resolve {}: {error}", source_audio.display()))?;
    let canonical_source_json = source_json.canonicalize().map_err(|error| format!("cannot resolve {}: {error}", source_json.display()))?;
    if !canonical_source_audio.starts_with(&canonical_local_root) || !canonical_source_json.starts_with(&canonical_local_root) {
        return Err("source recording path escapes configured local root".to_string());
    }

    let destination_audio = archive_root.join(&relative_audio);
    let destination_json = destination_audio.with_extension("json");
    if files_match(&source_audio, &destination_audio) && files_match(&source_json, &destination_json) {
        return Ok(ArchiveOutcome::AlreadyPresent);
    }

    let parent = destination_audio
        .parent()
        .ok_or_else(|| "archive destination has no parent directory".to_string())?;
    fs::create_dir_all(parent).map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    let canonical_archive_root = archive_root.canonicalize().map_err(|error| format!("cannot resolve archive root: {error}"))?;
    let canonical_parent = parent.canonicalize().map_err(|error| format!("cannot resolve {}: {error}", parent.display()))?;
    if !canonical_parent.starts_with(&canonical_archive_root) {
        return Err("archive destination escapes configured archive root".to_string());
    }

    copy_atomic(&canonical_source_audio, &destination_audio, &metadata.id)?;
    // JSON is copied last and therefore acts as the completion marker for one recording.
    copy_atomic(&canonical_source_json, &destination_json, &metadata.id)?;

    if !files_match(&source_audio, &destination_audio) || !files_match(&source_json, &destination_json) {
        return Err("archive verification failed after copy".to_string());
    }
    Ok(ArchiveOutcome::Copied)
}

fn verify_archive_root(root: &Path) -> Result<(), String> {
    if !root.is_dir() {
        return Err(format!("archive directory is unavailable or not a directory: {}", root.display()));
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
    fs::copy(source, &tmp).map_err(|error| format!("copy {} -> {} failed: {error}", source.display(), tmp.display()))?;
    let file = OpenOptions::new()
        .write(true)
        .open(&tmp)
        .map_err(|error| format!("cannot reopen {} for sync: {error}", tmp.display()))?;
    file.sync_all().map_err(|error| format!("sync {} failed: {error}", tmp.display()))?;
    fs::rename(&tmp, destination).map_err(|error| format!("rename {} -> {} failed: {error}", tmp.display(), destination.display()))?;
    Ok(())
}

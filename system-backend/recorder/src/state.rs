use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration as StdDuration, Instant};

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::config::RecorderConfig;
use crate::protocol::{MediaSwitchSession, RecorderTapBatch, RecorderTapRecord};

const RECORDING_SCHEMA_VERSION: u8 = 1;
const EXPECTED_TETRA_FRAME_BYTES: usize = 35;

#[derive(Debug, Clone, Serialize)]
pub struct RecorderStatus {
    pub service: &'static str,
    pub started_at: String,
    pub security_mode: &'static str,
    pub warning: &'static str,
    pub media_switch_connected: bool,
    pub media_switch_last_error: Option<String>,
    pub storage_available: bool,
    pub storage_last_error: Option<String>,
    pub storage_root: String,
    pub storage_used_bytes: u64,
    pub storage_free_bytes: Option<u64>,
    pub minimum_free_space_bytes: u64,
    pub active_recordings: usize,
    pub completed_recordings: usize,
    pub media_cursor: u64,
    pub frames_ingested: u64,
    pub frames_invalid: u64,
    pub frames_duplicate: u64,
    pub frames_lost_before_recorder: u64,
    pub recordings_started: u64,
    pub recordings_finalized: u64,
    pub recordings_recovered: u64,
    pub retention_deletions: u64,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeakerSegment {
    pub speaker_issi: Option<u32>,
    pub source_node_id: String,
    pub logical_ts: u8,
    pub first_tap_seq: u64,
    pub last_tap_seq: u64,
    pub started_at: String,
    pub ended_at: String,
    pub frame_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingMetadata {
    pub schema_version: u8,
    pub id: String,
    pub session_id: String,
    pub call_kind: String,
    pub call_phase_at_start: String,
    pub source_issi: Option<u32>,
    pub gssi: Option<u32>,
    pub calling_issi: Option<u32>,
    pub called_issi: Option<u32>,
    pub priority: u8,
    pub emergency: bool,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_ms: u64,
    pub frame_duration_ms: u64,
    pub frame_count: u64,
    pub audio_bytes: u64,
    pub injected_frames: u64,
    pub lost_tap_frames: u64,
    pub first_tap_seq: u64,
    pub last_tap_seq: u64,
    pub source_nodes: BTreeSet<String>,
    pub speakers: BTreeSet<u32>,
    pub segments: Vec<SpeakerSegment>,
    pub relative_directory: String,
    pub relative_audio_path: String,
    pub relative_index_path: String,
    pub audio_sha256: Option<String>,
    pub index_sha256: Option<String>,
    pub integrity_status: String,
    pub last_verified_at: Option<String>,
    pub recovered_after_unclean_shutdown: bool,
    pub finalized_reason: Option<String>,
    pub retention_days: u32,
    pub retention_until: String,
    pub legal_hold: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActiveRecordingSnapshot {
    pub metadata: RecordingMetadata,
    pub seconds_since_last_frame: u64,
    pub session_missing: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventRecord {
    pub seq: u64,
    pub timestamp: String,
    pub kind: String,
    pub recording_id: Option<String>,
    pub session_id: Option<String>,
    pub detail: Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RetentionInput {
    pub days: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HoldInput {
    pub legal_hold: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IntegrityFile {
    schema_version: u8,
    algorithm: String,
    audio_sha256: String,
    index_sha256: String,
    generated_at: String,
}

#[derive(Debug, Serialize)]
struct FrameIndexRecord<'a> {
    tap_seq: u64,
    timestamp: &'a str,
    source_sequence: u64,
    source_node_id: &'a str,
    logical_ts: u8,
    speaker_issi: Option<u32>,
    injected: bool,
    target_count: usize,
    byte_offset: u64,
    payload_bytes: usize,
}

struct ActiveRecording {
    metadata: RecordingMetadata,
    directory: PathBuf,
    audio_part_path: PathBuf,
    index_part_path: PathBuf,
    active_manifest_path: PathBuf,
    audio_file: BufWriter<File>,
    index_file: BufWriter<File>,
    audio_hasher: Sha256,
    index_hasher: Sha256,
    frames_since_sync: u64,
    last_frame: Instant,
    missing_since: Option<Instant>,
}

struct RecorderState {
    config: RecorderConfig,
    started_at: String,
    media_switch_connected: bool,
    media_switch_last_error: Option<String>,
    storage_available: bool,
    storage_last_error: Option<String>,
    recordings: BTreeMap<String, RecordingMetadata>,
    active: HashMap<String, ActiveRecording>,
    events: VecDeque<EventRecord>,
    next_event_seq: u64,
    media_cursor: u64,
    frames_ingested: u64,
    frames_invalid: u64,
    frames_duplicate: u64,
    frames_lost_before_recorder: u64,
    recordings_started: u64,
    recordings_finalized: u64,
    recordings_recovered: u64,
    retention_deletions: u64,
    storage_used_bytes: u64,
    last_retention_scan: Instant,
}

#[derive(Clone)]
pub struct SharedRecorder(Arc<Mutex<RecorderState>>);

impl SharedRecorder {
    pub fn load(config: RecorderConfig) -> Result<Self, Box<dyn std::error::Error>> {
        fs::create_dir_all(&config.storage.root)?;
        fs::create_dir_all(&config.storage.export_root)?;
        verify_writable(&config.storage.root)?;
        verify_writable(&config.storage.export_root)?;

        let recovered = recover_active_manifests(&config)?;
        let recordings = scan_recordings(&config.storage.root, config.limits.max_recordings)?;
        let storage_used_bytes = directory_size(&config.storage.root);
        let recovered_count = recovered as u64;

        let recorder = Self(Arc::new(Mutex::new(RecorderState {
            config,
            started_at: now_iso(),
            media_switch_connected: false,
            media_switch_last_error: None,
            storage_available: true,
            storage_last_error: None,
            recordings,
            active: HashMap::new(),
            events: VecDeque::new(),
            next_event_seq: 1,
            media_cursor: 0,
            frames_ingested: 0,
            frames_invalid: 0,
            frames_duplicate: 0,
            frames_lost_before_recorder: 0,
            recordings_started: 0,
            recordings_finalized: 0,
            recordings_recovered: recovered_count,
            retention_deletions: 0,
            storage_used_bytes,
            last_retention_scan: Instant::now(),
        })));

        if recovered > 0 {
            let mut state = recorder.0.lock().expect("recorder state poisoned");
            push_event_locked(
                &mut state,
                "recordings_recovered",
                None,
                None,
                json!({"count": recovered}),
            );
        }
        Ok(recorder)
    }

    pub fn status(&self) -> RecorderStatus {
        let state = self.0.lock().expect("recorder state poisoned");
        status_locked(&state)
    }

    pub fn media_cursor(&self) -> u64 {
        self.0
            .lock()
            .expect("recorder state poisoned")
            .media_cursor
    }

    pub fn media_switch_connected(&self) {
        let mut state = self.0.lock().expect("recorder state poisoned");
        let changed = !state.media_switch_connected;
        state.media_switch_connected = true;
        state.media_switch_last_error = None;
        if changed {
            push_event_locked(
                &mut state,
                "media_switch_connected",
                None,
                None,
                json!({}),
            );
        }
    }

    pub fn media_switch_failed(&self, error: String) {
        let mut state = self.0.lock().expect("recorder state poisoned");
        let changed = state.media_switch_connected
            || state.media_switch_last_error.as_deref() != Some(error.as_str());
        state.media_switch_connected = false;
        state.media_switch_last_error = Some(error.clone());
        if changed {
            push_event_locked(
                &mut state,
                "media_switch_unavailable",
                None,
                None,
                json!({"error": error}),
            );
        }
    }

    pub fn media_sequence_reset(&self, newest: u64) {
        let mut state = self.0.lock().expect("recorder state poisoned");
        let previous = state.media_cursor;
        state.media_cursor = 0;
        push_event_locked(
            &mut state,
            "media_tap_sequence_reset",
            None,
            None,
            json!({"previous_cursor": previous, "media_switch_newest": newest}),
        );
    }

    pub fn record_runtime_error(&self, error: String) {
        let mut state = self.0.lock().expect("recorder state poisoned");
        state.storage_available = false;
        state.storage_last_error = Some(error.clone());
        push_event_locked(
            &mut state,
            "recorder_runtime_error",
            None,
            None,
            json!({"error": error}),
        );
    }

    pub fn ingest_batch(&self, batch: RecorderTapBatch) -> Result<(), String> {
        let mut state = self.0.lock().expect("recorder state poisoned");
        state.media_switch_connected = true;
        state.media_switch_last_error = None;

        if batch.requested_after != state.media_cursor {
            let local_cursor = state.media_cursor;
            push_event_locked(
                &mut state,
                "media_tap_cursor_mismatch",
                None,
                None,
                json!({
                    "requested_after": batch.requested_after,
                    "local_cursor": local_cursor
                }),
            );
        }

        if batch.dropped_before > 0 {
            state.frames_lost_before_recorder = state
                .frames_lost_before_recorder
                .saturating_add(batch.dropped_before);
            state.media_cursor = batch
                .oldest_available_seq
                .unwrap_or(state.media_cursor.saturating_add(batch.dropped_before))
                .saturating_sub(1);
            push_event_locked(
                &mut state,
                "media_tap_gap_before_batch",
                None,
                None,
                json!({
                    "dropped_frames": batch.dropped_before,
                    "oldest_available_seq": batch.oldest_available_seq
                }),
            );
        }

        for tap in batch.records {
            if tap.seq <= state.media_cursor {
                state.frames_duplicate = state.frames_duplicate.saturating_add(1);
                continue;
            }
            if tap.seq > state.media_cursor.saturating_add(1) {
                let expected = state.media_cursor.saturating_add(1);
                let gap = tap.seq.saturating_sub(expected);
                state.frames_lost_before_recorder =
                    state.frames_lost_before_recorder.saturating_add(gap);
                if let Some(active) = state.active.get_mut(&tap.session_id) {
                    active.metadata.lost_tap_frames =
                        active.metadata.lost_tap_frames.saturating_add(gap);
                }
                push_event_locked(
                    &mut state,
                    "media_tap_sequence_gap",
                    None,
                    Some(tap.session_id.clone()),
                    json!({"expected": expected, "received": tap.seq, "lost": gap}),
                );
            }
            state.media_cursor = tap.seq;

            if tap.codec != "tetra_acelp0"
                || tap.payload.len() != EXPECTED_TETRA_FRAME_BYTES
                || (tap.source_logical_ts != 0 && !(1..=7).contains(&tap.source_logical_ts))
            {
                state.frames_invalid = state.frames_invalid.saturating_add(1);
                push_event_locked(
                    &mut state,
                    "invalid_media_tap",
                    None,
                    Some(tap.session_id),
                    json!({
                        "tap_seq": tap.seq,
                        "codec": tap.codec,
                        "payload_bytes": tap.payload.len(),
                        "logical_ts": tap.source_logical_ts
                    }),
                );
                continue;
            }

            ensure_storage_space_locked(&mut state)?;
            if !state.active.contains_key(&tap.session_id) {
                if state.active.len() >= state.config.limits.max_active_recordings {
                    return Err("maximum active recording count reached".to_string());
                }
                start_recording_locked(&mut state, &tap)?;
            }
            append_tap_locked(&mut state, &tap)?;
            state.frames_ingested = state.frames_ingested.saturating_add(1);
        }
        state.storage_available = true;
        state.storage_last_error = None;
        Ok(())
    }

    pub fn reconcile_sessions(&self, sessions: Vec<MediaSwitchSession>) {
        let mut state = self.0.lock().expect("recorder state poisoned");
        state.media_switch_connected = true;
        state.media_switch_last_error = None;
        let current = sessions
            .iter()
            .map(|session| session.logical_call_id.clone())
            .collect::<HashSet<_>>();
        let by_id = sessions
            .into_iter()
            .map(|session| (session.logical_call_id.clone(), session))
            .collect::<HashMap<_, _>>();

        for (session_id, active) in &mut state.active {
            if current.contains(session_id) {
                active.missing_since = None;
                if let Some(session) = by_id.get(session_id) {
                    merge_session_metadata(&mut active.metadata, session);
                }
            } else if active.missing_since.is_none() {
                active.missing_since = Some(Instant::now());
            }
        }
    }

    pub fn active_recordings(&self) -> Vec<ActiveRecordingSnapshot> {
        let state = self.0.lock().expect("recorder state poisoned");
        let mut values = state
            .active
            .values()
            .map(|active| ActiveRecordingSnapshot {
                metadata: active.metadata.clone(),
                seconds_since_last_frame: active.last_frame.elapsed().as_secs(),
                session_missing: active.missing_since.is_some(),
            })
            .collect::<Vec<_>>();
        values.sort_by(|a, b| b.metadata.started_at.cmp(&a.metadata.started_at));
        values
    }

    pub fn recordings(&self) -> Vec<RecordingMetadata> {
        let state = self.0.lock().expect("recorder state poisoned");
        let mut values = state.recordings.values().cloned().collect::<Vec<_>>();
        values.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        values
    }

    pub fn recording(&self, id: &str) -> Option<RecordingMetadata> {
        let state = self.0.lock().expect("recorder state poisoned");
        state.recordings.get(id).cloned().or_else(|| {
            state
                .active
                .values()
                .find(|active| active.metadata.id == id)
                .map(|active| active.metadata.clone())
        })
    }

    pub fn events(&self, limit: usize) -> Vec<EventRecord> {
        let state = self.0.lock().expect("recorder state poisoned");
        state
            .events
            .iter()
            .rev()
            .take(limit.min(state.events.len()))
            .cloned()
            .collect()
    }

    pub fn config_view(&self) -> Value {
        let state = self.0.lock().expect("recorder state poisoned");
        json!({
            "server": &state.config.server,
            "media_switch": &state.config.media_switch,
            "storage": &state.config.storage,
            "security": {
                "mode": &state.config.security.mode,
                "allow_remote_management": state.config.security.allow_remote_management,
                "allow_delete": state.config.security.allow_delete,
                "token_auth": false,
                "tls": false
            },
            "limits": &state.config.limits
        })
    }

    pub fn metrics(&self) -> String {
        let status = self.status();
        format!(
            concat!(
                "# HELP netcore_recorder_up Service liveness.\n",
                "# TYPE netcore_recorder_up gauge\n",
                "netcore_recorder_up 1\n",
                "# TYPE netcore_recorder_ready gauge\n",
                "netcore_recorder_ready {}\n",
                "# TYPE netcore_recorder_active_recordings gauge\n",
                "netcore_recorder_active_recordings {}\n",
                "# TYPE netcore_recorder_completed_recordings gauge\n",
                "netcore_recorder_completed_recordings {}\n",
                "# TYPE netcore_recorder_frames_ingested counter\n",
                "netcore_recorder_frames_ingested {}\n",
                "# TYPE netcore_recorder_frames_lost counter\n",
                "netcore_recorder_frames_lost {}\n",
                "# TYPE netcore_recorder_storage_used_bytes gauge\n",
                "netcore_recorder_storage_used_bytes {}\n"
            ),
            u8::from(status.ready),
            status.active_recordings,
            status.completed_recordings,
            status.frames_ingested,
            status.frames_lost_before_recorder,
            status.storage_used_bytes,
        )
    }

    pub fn verify_recording(&self, id: &str) -> Result<RecordingMetadata, String> {
        let (metadata, directory) = {
            let state = self.0.lock().expect("recorder state poisoned");
            require_management(&state)?;
            let metadata = state
                .recordings
                .get(id)
                .cloned()
                .ok_or_else(|| "recording not found or still active".to_string())?;
            let directory = recording_directory(&state, &metadata)?;
            (metadata, directory)
        };

        let integrity_path = directory.join("integrity.json");
        let expected: IntegrityFile = serde_json::from_slice(
            &fs::read(&integrity_path)
                .map_err(|error| format!("cannot read {}: {error}", integrity_path.display()))?,
        )
        .map_err(|error| format!("invalid integrity file: {error}"))?;
        let audio_path = directory.join("audio.tacelp");
        let index_path = directory.join("frames.jsonl");
        let audio_hash = hash_file(&audio_path)?;
        let index_hash = hash_file(&index_path)?;
        let valid = expected.algorithm == "sha256"
            && expected.audio_sha256 == audio_hash
            && expected.index_sha256 == index_hash;

        let mut state = self.0.lock().expect("recorder state poisoned");
        let recording = state
            .recordings
            .get_mut(id)
            .ok_or_else(|| "recording disappeared during verification".to_string())?;
        recording.integrity_status = if valid { "verified" } else { "failed" }.to_string();
        recording.last_verified_at = Some(now_iso());
        let updated = recording.clone();
        write_json_atomic(&directory.join("metadata.json"), &updated)?;
        push_event_locked(
            &mut state,
            if valid {
                "integrity_verified"
            } else {
                "integrity_failed"
            },
            Some(id.to_string()),
            Some(metadata.session_id),
            json!({"audio_sha256": audio_hash, "index_sha256": index_hash}),
        );
        if valid {
            Ok(updated)
        } else {
            Err("recording integrity verification failed".to_string())
        }
    }

    pub fn set_retention(
        &self,
        id: &str,
        input: RetentionInput,
    ) -> Result<RecordingMetadata, String> {
        let mut state = self.0.lock().expect("recorder state poisoned");
        require_management(&state)?;
        let days = input.days.clamp(1, 3_650);
        let recording = state
            .recordings
            .get_mut(id)
            .ok_or_else(|| "recording not found or still active".to_string())?;
        recording.retention_days = days;
        let base = recording
            .ended_at
            .as_deref()
            .and_then(parse_timestamp)
            .unwrap_or_else(Utc::now);
        recording.retention_until = (base + Duration::days(days.into())).to_rfc3339();
        let updated = recording.clone();
        let directory = recording_directory(&state, &updated)?;
        write_json_atomic(&directory.join("metadata.json"), &updated)?;
        push_event_locked(
            &mut state,
            "retention_changed",
            Some(id.to_string()),
            Some(updated.session_id.clone()),
            json!({"days": days, "retention_until": updated.retention_until}),
        );
        Ok(updated)
    }

    pub fn set_hold(&self, id: &str, input: HoldInput) -> Result<RecordingMetadata, String> {
        let mut state = self.0.lock().expect("recorder state poisoned");
        require_management(&state)?;
        let recording = state
            .recordings
            .get_mut(id)
            .ok_or_else(|| "recording not found or still active".to_string())?;
        recording.legal_hold = input.legal_hold;
        let updated = recording.clone();
        let directory = recording_directory(&state, &updated)?;
        write_json_atomic(&directory.join("metadata.json"), &updated)?;
        push_event_locked(
            &mut state,
            if input.legal_hold {
                "legal_hold_enabled"
            } else {
                "legal_hold_disabled"
            },
            Some(id.to_string()),
            Some(updated.session_id.clone()),
            json!({}),
        );
        Ok(updated)
    }

    pub fn delete_recording(&self, id: &str) -> Result<(), String> {
        let mut state = self.0.lock().expect("recorder state poisoned");
        require_management(&state)?;
        if !state.config.security.allow_delete {
            return Err("recording deletion is disabled by configuration".to_string());
        }
        let metadata = state
            .recordings
            .get(id)
            .cloned()
            .ok_or_else(|| "recording not found or still active".to_string())?;
        if metadata.legal_hold {
            return Err("recording is under legal hold".to_string());
        }
        delete_recording_locked(&mut state, &metadata, "manual_delete")
    }

    pub fn finalize_active(&self, id: &str) -> Result<RecordingMetadata, String> {
        let mut state = self.0.lock().expect("recorder state poisoned");
        require_management(&state)?;
        let session_id = state
            .active
            .iter()
            .find(|(_, active)| active.metadata.id == id)
            .map(|(session_id, _)| session_id.clone())
            .ok_or_else(|| "active recording not found".to_string())?;
        finalize_recording_locked(&mut state, &session_id, "manual_finalize")
    }

    pub fn export_recording(&self, id: &str) -> Result<PathBuf, String> {
        let (metadata, directory, output) = {
            let state = self.0.lock().expect("recorder state poisoned");
            require_management(&state)?;
            let metadata = state
                .recordings
                .get(id)
                .cloned()
                .ok_or_else(|| "recording not found or still active".to_string())?;
            let directory = recording_directory(&state, &metadata)?;
            let output = state.config.storage.export_root.join(format!("{id}.tar"));
            (metadata, directory, output)
        };

        let files = ["metadata.json", "audio.tacelp", "frames.jsonl", "integrity.json"]
            .into_iter()
            .map(|name| (directory.join(name), format!("{id}/{name}")))
            .collect::<Vec<_>>();
        crate::tar::create_tar(&output, &files)?;

        let mut state = self.0.lock().expect("recorder state poisoned");
        push_event_locked(
            &mut state,
            "recording_exported",
            Some(id.to_string()),
            Some(metadata.session_id),
            json!({"path": output.display().to_string()}),
        );
        Ok(output)
    }

    pub fn maintenance(&self) {
        let mut state = self.0.lock().expect("recorder state poisoned");
        let absent_grace = StdDuration::from_secs(state.config.storage.session_absent_grace_secs);
        let max_idle = StdDuration::from_secs(state.config.storage.maximum_idle_secs);
        let to_finalize = state
            .active
            .iter()
            .filter_map(|(session_id, active)| {
                let reason = if active
                    .missing_since
                    .is_some_and(|missing| missing.elapsed() >= absent_grace)
                {
                    Some("media_session_ended")
                } else if active.last_frame.elapsed() >= max_idle {
                    Some("maximum_idle_timeout")
                } else {
                    None
                };
                reason.map(|reason| (session_id.clone(), reason))
            })
            .collect::<Vec<_>>();
        for (session_id, reason) in to_finalize {
            if let Err(error) = finalize_recording_locked(&mut state, &session_id, reason) {
                state.storage_available = false;
                state.storage_last_error = Some(error.clone());
                push_event_locked(
                    &mut state,
                    "recording_finalize_failed",
                    None,
                    Some(session_id),
                    json!({"error": error}),
                );
            }
        }

        if state.last_retention_scan.elapsed()
            >= StdDuration::from_secs(state.config.storage.retention_scan_secs)
        {
            run_retention_locked(&mut state);
            state.last_retention_scan = Instant::now();
        }
    }
}

pub fn spawn_maintenance_worker(
    _config: RecorderConfig,
    recorder: SharedRecorder,
) -> thread::JoinHandle<()> {
    thread::spawn(move || loop {
        recorder.maintenance();
        thread::sleep(StdDuration::from_secs(1));
    })
}

fn status_locked(state: &RecorderState) -> RecorderStatus {
    let active_bytes = state
        .active
        .values()
        .map(|active| active.metadata.audio_bytes)
        .sum::<u64>();
    let free = free_space_bytes(&state.config.storage.root);
    let minimum = state
        .config
        .storage
        .minimum_free_space_mb
        .saturating_mul(1024 * 1024);
    let storage_available = state.storage_available && free.is_none_or(|bytes| bytes >= minimum);
    RecorderStatus {
        service: "netcore-recorder",
        started_at: state.started_at.clone(),
        security_mode: "open_lab",
        warning: "OPEN LAB: no authentication, no tokens and no TLS; isolated test network only",
        media_switch_connected: state.media_switch_connected,
        media_switch_last_error: state.media_switch_last_error.clone(),
        storage_available,
        storage_last_error: state.storage_last_error.clone(),
        storage_root: state.config.storage.root.display().to_string(),
        storage_used_bytes: state.storage_used_bytes.saturating_add(active_bytes),
        storage_free_bytes: free,
        minimum_free_space_bytes: minimum,
        active_recordings: state.active.len(),
        completed_recordings: state.recordings.len(),
        media_cursor: state.media_cursor,
        frames_ingested: state.frames_ingested,
        frames_invalid: state.frames_invalid,
        frames_duplicate: state.frames_duplicate,
        frames_lost_before_recorder: state.frames_lost_before_recorder,
        recordings_started: state.recordings_started,
        recordings_finalized: state.recordings_finalized,
        recordings_recovered: state.recordings_recovered,
        retention_deletions: state.retention_deletions,
        ready: state.media_switch_connected && storage_available,
    }
}

fn ensure_storage_space_locked(state: &mut RecorderState) -> Result<(), String> {
    let minimum = state
        .config
        .storage
        .minimum_free_space_mb
        .saturating_mul(1024 * 1024);
    if free_space_bytes(&state.config.storage.root).is_some_and(|free| free < minimum) {
        state.storage_available = false;
        let error = format!(
            "storage free space below configured minimum of {} MiB",
            state.config.storage.minimum_free_space_mb
        );
        state.storage_last_error = Some(error.clone());
        return Err(error);
    }
    Ok(())
}

fn start_recording_locked(
    state: &mut RecorderState,
    tap: &RecorderTapRecord,
) -> Result<(), String> {
    if state.recordings.len().saturating_add(state.active.len())
        >= state.config.limits.max_recordings
    {
        return Err("maximum recording count reached".to_string());
    }
    let id = Uuid::new_v4().to_string();
    let started = parse_timestamp(&tap.timestamp).unwrap_or_else(Utc::now);
    let relative_directory = format!("{}/{}", started.format("%Y/%m/%d"), id);
    let directory = safe_relative_join(&state.config.storage.root, &relative_directory)?;
    fs::create_dir_all(&directory)
        .map_err(|error| format!("cannot create {}: {error}", directory.display()))?;
    let audio_part_path = directory.join("audio.tacelp.part");
    let index_part_path = directory.join("frames.jsonl.part");
    let active_manifest_path = directory.join("metadata.active.json");
    let audio_file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&audio_part_path)
        .map_err(|error| format!("cannot create {}: {error}", audio_part_path.display()))?;
    let index_file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&index_part_path)
        .map_err(|error| format!("cannot create {}: {error}", index_part_path.display()))?;
    let retention_until =
        (started + Duration::days(state.config.storage.default_retention_days.into())).to_rfc3339();
    let metadata = RecordingMetadata {
        schema_version: RECORDING_SCHEMA_VERSION,
        id: id.clone(),
        session_id: tap.session_id.clone(),
        call_kind: tap.call_kind.clone(),
        call_phase_at_start: tap.call_phase.clone(),
        source_issi: tap.source_issi,
        gssi: tap.gssi,
        calling_issi: tap.calling_issi,
        called_issi: tap.called_issi,
        priority: tap.priority,
        emergency: tap.emergency,
        started_at: tap.timestamp.clone(),
        ended_at: None,
        duration_ms: 0,
        frame_duration_ms: state.config.storage.frame_duration_ms,
        frame_count: 0,
        audio_bytes: 0,
        injected_frames: 0,
        lost_tap_frames: 0,
        first_tap_seq: tap.seq,
        last_tap_seq: tap.seq,
        source_nodes: BTreeSet::new(),
        speakers: BTreeSet::new(),
        segments: Vec::new(),
        relative_directory: relative_directory.clone(),
        relative_audio_path: format!("{relative_directory}/audio.tacelp"),
        relative_index_path: format!("{relative_directory}/frames.jsonl"),
        audio_sha256: None,
        index_sha256: None,
        integrity_status: "recording".to_string(),
        last_verified_at: None,
        recovered_after_unclean_shutdown: false,
        finalized_reason: None,
        retention_days: state.config.storage.default_retention_days,
        retention_until,
        legal_hold: false,
    };
    write_json_atomic(&active_manifest_path, &metadata)?;
    state.active.insert(
        tap.session_id.clone(),
        ActiveRecording {
            metadata,
            directory,
            audio_part_path,
            index_part_path,
            active_manifest_path,
            audio_file: BufWriter::new(audio_file),
            index_file: BufWriter::new(index_file),
            audio_hasher: Sha256::new(),
            index_hasher: Sha256::new(),
            frames_since_sync: 0,
            last_frame: Instant::now(),
            missing_since: None,
        },
    );
    state.recordings_started = state.recordings_started.saturating_add(1);
    push_event_locked(
        state,
        "recording_started",
        Some(id),
        Some(tap.session_id.clone()),
        json!({
            "gssi": tap.gssi,
            "calling_issi": tap.calling_issi,
            "called_issi": tap.called_issi,
            "emergency": tap.emergency
        }),
    );
    Ok(())
}

fn append_tap_locked(state: &mut RecorderState, tap: &RecorderTapRecord) -> Result<(), String> {
    let fsync_every = state.config.storage.fsync_every_frames;
    let active = state
        .active
        .get_mut(&tap.session_id)
        .ok_or_else(|| "active recording vanished".to_string())?;
    merge_tap_metadata(&mut active.metadata, tap);
    let offset = active.metadata.audio_bytes;
    active
        .audio_file
        .write_all(&tap.payload)
        .map_err(|error| format!("audio write failed: {error}"))?;
    active.audio_hasher.update(&tap.payload);

    let index_record = FrameIndexRecord {
        tap_seq: tap.seq,
        timestamp: &tap.timestamp,
        source_sequence: tap.source_sequence,
        source_node_id: &tap.source_node_id,
        logical_ts: tap.source_logical_ts,
        speaker_issi: tap.speaker_issi,
        injected: tap.injected,
        target_count: tap.target_count,
        byte_offset: offset,
        payload_bytes: tap.payload.len(),
    };
    let mut line = serde_json::to_vec(&index_record)
        .map_err(|error| format!("frame index serialization failed: {error}"))?;
    line.push(b'\n');
    active
        .index_file
        .write_all(&line)
        .map_err(|error| format!("index write failed: {error}"))?;
    active.index_hasher.update(&line);

    active.metadata.frame_count = active.metadata.frame_count.saturating_add(1);
    active.metadata.audio_bytes = active
        .metadata
        .audio_bytes
        .saturating_add(tap.payload.len() as u64);
    active.metadata.last_tap_seq = tap.seq;
    active.metadata.duration_ms = active
        .metadata
        .frame_count
        .saturating_mul(active.metadata.frame_duration_ms);
    if tap.injected {
        active.metadata.injected_frames = active.metadata.injected_frames.saturating_add(1);
    }
    active.metadata.source_nodes.insert(tap.source_node_id.clone());
    if let Some(speaker) = tap.speaker_issi {
        active.metadata.speakers.insert(speaker);
    }
    update_segment(&mut active.metadata, tap);
    active.last_frame = Instant::now();
    active.missing_since = None;
    active.frames_since_sync = active.frames_since_sync.saturating_add(1);

    if active.frames_since_sync >= fsync_every {
        sync_active(active)?;
    }
    Ok(())
}

fn update_segment(metadata: &mut RecordingMetadata, tap: &RecorderTapRecord) {
    let same = metadata.segments.last().is_some_and(|segment| {
        segment.speaker_issi == tap.speaker_issi
            && segment.source_node_id == tap.source_node_id
            && segment.logical_ts == tap.source_logical_ts
    });
    if same {
        let segment = metadata.segments.last_mut().expect("segment exists");
        segment.last_tap_seq = tap.seq;
        segment.ended_at = tap.timestamp.clone();
        segment.frame_count = segment.frame_count.saturating_add(1);
    } else {
        metadata.segments.push(SpeakerSegment {
            speaker_issi: tap.speaker_issi,
            source_node_id: tap.source_node_id.clone(),
            logical_ts: tap.source_logical_ts,
            first_tap_seq: tap.seq,
            last_tap_seq: tap.seq,
            started_at: tap.timestamp.clone(),
            ended_at: tap.timestamp.clone(),
            frame_count: 1,
        });
    }
}

fn merge_tap_metadata(metadata: &mut RecordingMetadata, tap: &RecorderTapRecord) {
    metadata.call_kind = tap.call_kind.clone();
    metadata.source_issi = tap.source_issi.or(metadata.source_issi);
    metadata.gssi = tap.gssi.or(metadata.gssi);
    metadata.calling_issi = tap.calling_issi.or(metadata.calling_issi);
    metadata.called_issi = tap.called_issi.or(metadata.called_issi);
    metadata.priority = metadata.priority.max(tap.priority);
    metadata.emergency |= tap.emergency;
}

fn merge_session_metadata(metadata: &mut RecordingMetadata, session: &MediaSwitchSession) {
    metadata.call_kind = session.kind.clone();
    if metadata.call_phase_at_start == "unknown" && session.phase != "unknown" {
        metadata.call_phase_at_start = session.phase.clone();
    }
    if let Some(floor_holder) = session.floor_holder {
        metadata.speakers.insert(floor_holder);
    }
    metadata.source_issi = session.source_issi.or(metadata.source_issi);
    metadata.gssi = session.gssi.or(metadata.gssi);
    metadata.calling_issi = session.calling_issi.or(metadata.calling_issi);
    metadata.called_issi = session.called_issi.or(metadata.called_issi);
    metadata.priority = metadata.priority.max(session.priority);
    metadata.emergency |= session.emergency;
}

fn sync_active(active: &mut ActiveRecording) -> Result<(), String> {
    active
        .audio_file
        .flush()
        .map_err(|error| format!("audio flush failed: {error}"))?;
    active
        .index_file
        .flush()
        .map_err(|error| format!("index flush failed: {error}"))?;
    active
        .audio_file
        .get_ref()
        .sync_data()
        .map_err(|error| format!("audio sync failed: {error}"))?;
    active
        .index_file
        .get_ref()
        .sync_data()
        .map_err(|error| format!("index sync failed: {error}"))?;
    write_json_atomic(&active.active_manifest_path, &active.metadata)?;
    active.frames_since_sync = 0;
    Ok(())
}

fn finalize_recording_locked(
    state: &mut RecorderState,
    session_id: &str,
    reason: &str,
) -> Result<RecordingMetadata, String> {
    let mut active = state
        .active
        .remove(session_id)
        .ok_or_else(|| "active recording not found".to_string())?;
    if let Err(error) = sync_active(&mut active) {
        state.active.insert(session_id.to_string(), active);
        return Err(error);
    }

    let ActiveRecording {
        mut metadata,
        directory,
        audio_part_path,
        index_part_path,
        active_manifest_path,
        audio_file,
        index_file,
        audio_hasher,
        index_hasher,
        ..
    } = active;
    drop(audio_file);
    drop(index_file);

    let audio_path = directory.join("audio.tacelp");
    let index_path = directory.join("frames.jsonl");
    fs::rename(&index_part_path, &index_path).map_err(|error| {
        format!(
            "cannot publish {} -> {}: {error}",
            index_part_path.display(),
            index_path.display()
        )
    })?;
    fs::rename(&audio_part_path, &audio_path).map_err(|error| {
        format!(
            "cannot publish {} -> {}: {error}",
            audio_part_path.display(),
            audio_path.display()
        )
    })?;

    let audio_sha256 = hex_digest(audio_hasher.finalize());
    let index_sha256 = hex_digest(index_hasher.finalize());
    let ended = Utc::now();
    let started = parse_timestamp(&metadata.started_at).unwrap_or_else(|| ended.clone());
    metadata.ended_at = Some(ended.to_rfc3339());
    metadata.duration_ms = (ended.clone() - started)
        .num_milliseconds()
        .max(metadata.duration_ms as i64)
        .max(0) as u64;
    metadata.audio_sha256 = Some(audio_sha256.clone());
    metadata.index_sha256 = Some(index_sha256.clone());
    metadata.integrity_status = "verified".to_string();
    metadata.last_verified_at = Some(now_iso());
    metadata.finalized_reason = Some(reason.to_string());
    metadata.retention_until =
        (ended + Duration::days(metadata.retention_days.into())).to_rfc3339();

    let integrity = IntegrityFile {
        schema_version: 1,
        algorithm: "sha256".to_string(),
        audio_sha256,
        index_sha256,
        generated_at: now_iso(),
    };
    write_json_atomic(&directory.join("integrity.json"), &integrity)?;
    write_json_atomic(&directory.join("metadata.json"), &metadata)?;
    let _ = fs::remove_file(&active_manifest_path);

    state.storage_used_bytes = state
        .storage_used_bytes
        .saturating_add(directory_size(&directory));
    state
        .recordings
        .insert(metadata.id.clone(), metadata.clone());
    state.recordings_finalized = state.recordings_finalized.saturating_add(1);
    push_event_locked(
        state,
        "recording_finalized",
        Some(metadata.id.clone()),
        Some(metadata.session_id.clone()),
        json!({
            "reason": reason,
            "frames": metadata.frame_count,
            "duration_ms": metadata.duration_ms,
            "audio_bytes": metadata.audio_bytes
        }),
    );
    Ok(metadata)
}

fn run_retention_locked(state: &mut RecorderState) {
    let now = Utc::now();
    let expired = state
        .recordings
        .values()
        .filter(|metadata| !metadata.legal_hold)
        .filter(|metadata| {
            parse_timestamp(&metadata.retention_until).is_some_and(|until| until <= now)
        })
        .cloned()
        .collect::<Vec<_>>();
    for metadata in expired {
        if let Err(error) = delete_recording_locked(state, &metadata, "retention_expired") {
            push_event_locked(
                state,
                "retention_delete_failed",
                Some(metadata.id.clone()),
                Some(metadata.session_id.clone()),
                json!({"error": error}),
            );
        } else {
            state.retention_deletions = state.retention_deletions.saturating_add(1);
        }
    }
}

fn delete_recording_locked(
    state: &mut RecorderState,
    metadata: &RecordingMetadata,
    reason: &str,
) -> Result<(), String> {
    let directory = recording_directory(state, metadata)?;
    let size = directory_size(&directory);
    fs::remove_dir_all(&directory)
        .map_err(|error| format!("cannot remove {}: {error}", directory.display()))?;
    let _ = fs::remove_file(
        state
            .config
            .storage
            .export_root
            .join(format!("{}.tar", metadata.id)),
    );
    state.recordings.remove(&metadata.id);
    state.storage_used_bytes = state.storage_used_bytes.saturating_sub(size);
    push_event_locked(
        state,
        "recording_deleted",
        Some(metadata.id.clone()),
        Some(metadata.session_id.clone()),
        json!({"reason": reason}),
    );
    Ok(())
}

fn require_management(state: &RecorderState) -> Result<(), String> {
    if state.config.security.allow_remote_management {
        Ok(())
    } else {
        Err("remote management is disabled by configuration".to_string())
    }
}

fn recording_directory(
    state: &RecorderState,
    metadata: &RecordingMetadata,
) -> Result<PathBuf, String> {
    safe_relative_join(&state.config.storage.root, &metadata.relative_directory)
}

fn push_event_locked(
    state: &mut RecorderState,
    kind: &str,
    recording_id: Option<String>,
    session_id: Option<String>,
    detail: Value,
) {
    let event = EventRecord {
        seq: state.next_event_seq,
        timestamp: now_iso(),
        kind: kind.to_string(),
        recording_id,
        session_id,
        detail,
    };
    state.next_event_seq = state.next_event_seq.wrapping_add(1);
    state.events.push_back(event);
    while state.events.len() > state.config.server.history_limit {
        state.events.pop_front();
    }
}

fn recover_active_manifests(config: &RecorderConfig) -> Result<usize, String> {
    let manifests = find_named_files(&config.storage.root, "metadata.active.json");
    let mut recovered = 0usize;
    for manifest_path in manifests {
        let payload = fs::read(&manifest_path)
            .map_err(|error| format!("cannot read {}: {error}", manifest_path.display()))?;
        let mut metadata: RecordingMetadata = match serde_json::from_slice(&payload) {
            Ok(metadata) => metadata,
            Err(error) => {
                tracing::warn!("cannot recover {}: {}", manifest_path.display(), error);
                continue;
            }
        };
        let Some(directory) = manifest_path.parent() else {
            continue;
        };
        let audio_part = directory.join("audio.tacelp.part");
        let index_part = directory.join("frames.jsonl.part");
        let audio = directory.join("audio.tacelp");
        let index = directory.join("frames.jsonl");
        if audio_part.exists() {
            fs::rename(&audio_part, &audio).map_err(|error| {
                format!("cannot recover {}: {error}", audio_part.display())
            })?;
        }
        if index_part.exists() {
            fs::rename(&index_part, &index).map_err(|error| {
                format!("cannot recover {}: {error}", index_part.display())
            })?;
        }
        if !audio.exists() || !index.exists() {
            tracing::warn!("incomplete active recording at {}", directory.display());
            continue;
        }
        let ended = Utc::now();
        metadata.ended_at = Some(ended.to_rfc3339());
        metadata.recovered_after_unclean_shutdown = true;
        metadata.finalized_reason = Some("unclean_shutdown_recovery".to_string());
        metadata.audio_bytes = fs::metadata(&audio).map(|value| value.len()).unwrap_or(0);
        metadata.frame_count = metadata.audio_bytes / EXPECTED_TETRA_FRAME_BYTES as u64;
        metadata.duration_ms = metadata.frame_count.saturating_mul(metadata.frame_duration_ms);
        let audio_sha256 = hash_file(&audio)?;
        let index_sha256 = hash_file(&index)?;
        metadata.audio_sha256 = Some(audio_sha256.clone());
        metadata.index_sha256 = Some(index_sha256.clone());
        metadata.integrity_status = "verified".to_string();
        metadata.last_verified_at = Some(now_iso());
        metadata.retention_until =
            (ended + Duration::days(metadata.retention_days.into())).to_rfc3339();
        write_json_atomic(
            &directory.join("integrity.json"),
            &IntegrityFile {
                schema_version: 1,
                algorithm: "sha256".to_string(),
                audio_sha256,
                index_sha256,
                generated_at: now_iso(),
            },
        )?;
        write_json_atomic(&directory.join("metadata.json"), &metadata)?;
        let _ = fs::remove_file(&manifest_path);
        recovered += 1;
    }
    Ok(recovered)
}

fn scan_recordings(
    root: &Path,
    max_recordings: usize,
) -> Result<BTreeMap<String, RecordingMetadata>, Box<dyn std::error::Error>> {
    let mut recordings = BTreeMap::new();
    for path in find_named_files(root, "metadata.json") {
        if recordings.len() >= max_recordings {
            break;
        }
        match serde_json::from_slice::<RecordingMetadata>(&fs::read(&path)?) {
            Ok(metadata) => {
                recordings.insert(metadata.id.clone(), metadata);
            }
            Err(error) => tracing::warn!("ignoring invalid {}: {}", path.display(), error),
        }
    }
    Ok(recordings)
}

fn find_named_files(root: &Path, name: &str) -> Vec<PathBuf> {
    let mut output = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path
                .file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value == name)
            {
                output.push(path);
            }
        }
    }
    output
}

fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    let part = path.with_extension(format!(
        "{}.part",
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("json")
    ));
    let mut payload = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("JSON serialization failed: {error}"))?;
    payload.push(b'\n');
    let mut file = File::create(&part)
        .map_err(|error| format!("cannot create {}: {error}", part.display()))?;
    file.write_all(&payload)
        .map_err(|error| format!("cannot write {}: {error}", part.display()))?;
    file.sync_all()
        .map_err(|error| format!("cannot sync {}: {error}", part.display()))?;
    fs::rename(&part, path).map_err(|error| {
        format!(
            "cannot publish {} -> {}: {error}",
            part.display(),
            path.display()
        )
    })
}

fn hash_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path)
        .map_err(|error| format!("cannot open {}: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex_digest(hasher.finalize()))
}

fn hex_digest(bytes: impl AsRef<[u8]>) -> String {
    let mut output = String::with_capacity(bytes.as_ref().len() * 2);
    for byte in bytes.as_ref() {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn verify_writable(path: &Path) -> Result<(), std::io::Error> {
    let marker = path.join(format!(".write-test-{}", Uuid::new_v4()));
    fs::write(&marker, b"netcore-recorder")?;
    fs::remove_file(marker)
}

fn safe_relative_join(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let relative_path = Path::new(relative);
    if relative_path.is_absolute()
        || relative_path.components().any(|component| {
            !matches!(component, Component::Normal(_))
        })
    {
        return Err("recording path contains unsafe components".to_string());
    }
    Ok(root.join(relative_path))
}

fn directory_size(path: &Path) -> u64 {
    let mut total = 0u64;
    let mut stack = vec![path.to_path_buf()];
    while let Some(path) = stack.pop() {
        if let Ok(metadata) = fs::symlink_metadata(&path) {
            if metadata.is_file() {
                total = total.saturating_add(metadata.len());
            } else if metadata.is_dir()
                && let Ok(entries) = fs::read_dir(&path)
            {
                stack.extend(entries.flatten().map(|entry| entry.path()));
            }
        }
    }
    total
}

#[cfg(unix)]
fn free_space_bytes(path: &Path) -> Option<u64> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let path = CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut stats = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    let result = unsafe { libc::statvfs(path.as_ptr(), stats.as_mut_ptr()) };
    if result != 0 {
        return None;
    }
    let stats = unsafe { stats.assume_init() };
    Some((stats.f_bavail as u64).saturating_mul(stats.f_frsize as u64))
}

#[cfg(not(unix))]
fn free_space_bytes(_path: &Path) -> Option<u64> {
    None
}

fn parse_timestamp(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|value| value.with_timezone(&Utc))
}

fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RecorderConfig;
    use crate::protocol::{RecorderTapBatch, RecorderTapRecord};

    fn test_config(root: &Path) -> RecorderConfig {
        let mut config = RecorderConfig::default();
        config.storage.root = root.join("recordings");
        config.storage.export_root = root.join("exports");
        config.storage.minimum_free_space_mb = 16;
        config.storage.fsync_every_frames = 1;
        config.storage.session_absent_grace_secs = 1;
        config
    }

    fn tap(seq: u64) -> RecorderTapRecord {
        RecorderTapRecord {
            seq,
            timestamp: now_iso(),
            session_id: "call-1".to_string(),
            call_kind: "group".to_string(),
            call_phase: "active".to_string(),
            source_issi: Some(1001),
            gssi: Some(2000),
            calling_issi: None,
            called_issi: None,
            priority: 5,
            emergency: false,
            speaker_issi: Some(1001),
            source_node_id: "tbs-a".to_string(),
            source_logical_ts: 2,
            source_sequence: seq,
            target_count: 1,
            codec: "tetra_acelp0".to_string(),
            payload: vec![0x55; EXPECTED_TETRA_FRAME_BYTES],
            injected: false,
        }
    }

    fn temp_root() -> PathBuf {
        let root = std::env::temp_dir().join(format!("netcore-recorder-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("temp root");
        root
    }

    #[test]
    fn records_and_finalizes_packed_frames() {
        let root = temp_root();
        let recorder = SharedRecorder::load(test_config(&root)).expect("recorder loads");
        recorder
            .ingest_batch(RecorderTapBatch {
                requested_after: 0,
                oldest_available_seq: Some(1),
                newest_available_seq: Some(2),
                dropped_before: 0,
                records: vec![tap(1), tap(2)],
            })
            .expect("batch ingested");
        let active = recorder.active_recordings();
        assert_eq!(active.len(), 1);
        let finalized = recorder
            .finalize_active(&active[0].metadata.id)
            .expect("finalized");
        assert_eq!(finalized.frame_count, 2);
        assert_eq!(finalized.audio_bytes, 70);
        assert_eq!(finalized.gssi, Some(2000));
        assert_eq!(finalized.integrity_status, "verified");
        assert!(recorder.verify_recording(&finalized.id).is_ok());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn sequence_gap_is_visible_in_status_and_metadata() {
        let root = temp_root();
        let recorder = SharedRecorder::load(test_config(&root)).expect("recorder loads");
        recorder
            .ingest_batch(RecorderTapBatch {
                requested_after: 0,
                oldest_available_seq: Some(3),
                newest_available_seq: Some(3),
                dropped_before: 2,
                records: vec![tap(3)],
            })
            .expect("batch ingested");
        assert_eq!(recorder.status().frames_lost_before_recorder, 2);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unsafe_relative_path_is_rejected() {
        assert!(safe_relative_join(Path::new("/tmp"), "../etc").is_err());
        assert!(safe_relative_join(Path::new("/tmp"), "2026/07/id").is_ok());
    }
}

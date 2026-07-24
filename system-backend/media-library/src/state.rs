use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::config::{MediaLibraryConfig, OPEN_LAB_MODE, SHADOW_MODE};
use crate::media;
use crate::model::{
    ActionInput, ApprovalInput, AssetRecord, AssetUpdateInput, AuditRecord, BackupRecord,
    ConfigView, DispatchClaim, DispatchInput, DispatchJob, EventRecord, ImportClaim,
    ImportUrlInput, ProcessingClaim, ProcessResult, RecorderImportInput, StatusView, UploadInput,
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
struct PersistentState {
    assets: BTreeMap<String, AssetRecord>,
    jobs: Vec<DispatchJob>,
    events: VecDeque<EventRecord>,
    audit: VecDeque<AuditRecord>,
    backups: Vec<BackupRecord>,
    next_event_seq: u64,
    next_audit_seq: u64,
    dedupe_hits: u64,
    imports_completed: u64,
    processing_completed: u64,
    dispatch_frames_sent: u64,
}

struct LibraryInner {
    config: MediaLibraryConfig,
    state: PersistentState,
    started_at: DateTime<Utc>,
    storage_available: bool,
    storage_last_error: Option<String>,
    archive_available: bool,
    media_switch_connected: bool,
    recorder_connected: bool,
    application_gateway_connected: bool,
    dependency_errors: BTreeMap<String, String>,
    last_dependency_probe_at: Option<DateTime<Utc>>,
}

#[derive(Clone)]
pub struct SharedLibrary {
    inner: Arc<Mutex<LibraryInner>>,
}

impl SharedLibrary {
    pub fn load(config: MediaLibraryConfig) -> Result<Self, Box<dyn std::error::Error>> {
        for directory in [
            &config.storage.root,
            &config.storage.temp_root,
            &config.storage.backup_root,
        ] {
            fs::create_dir_all(directory)?;
        }
        if let Some(parent) = config.storage.state_file.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut state = if config.storage.state_file.is_file() {
            serde_json::from_slice::<PersistentState>(&fs::read(&config.storage.state_file)?)?
        } else {
            PersistentState::default()
        };
        let now = Utc::now();
        for asset in state.assets.values_mut() {
            if matches!(asset.state.as_str(), "importing_active" | "processing_active") {
                asset.state = if asset.original_path.is_some() {
                    "processing".to_string()
                } else {
                    "importing".to_string()
                };
                asset.updated_at = now;
                asset.last_error = Some("recovered after service restart".to_string());
            }
        }
        for job in &mut state.jobs {
            if job.state == "playing" {
                job.state = "failed".to_string();
                job.last_error = Some(
                    "playout interrupted by service restart; manual retry required to avoid duplicate audio"
                        .to_string(),
                );
                job.updated_at = now;
                job.completed_at = Some(now);
            }
        }
        let archive_available = config
            .storage
            .archive_root
            .as_ref()
            .is_some_and(|path| path.is_dir());
        let library = Self {
            inner: Arc::new(Mutex::new(LibraryInner {
                config,
                state,
                started_at: now,
                storage_available: true,
                storage_last_error: None,
                archive_available,
                media_switch_connected: false,
                recorder_connected: false,
                application_gateway_connected: false,
                dependency_errors: BTreeMap::new(),
                last_dependency_probe_at: None,
            })),
        };
        {
            let mut inner = lock(&library.inner);
            persist_locked(&mut inner)?;
        }
        Ok(library)
    }

    pub fn status(&self) -> StatusView {
        let inner = lock(&self.inner);
        let assets = inner.state.assets.values().collect::<Vec<_>>();
        let jobs = &inner.state.jobs;
        StatusView {
            service: "media-library".to_string(),
            security_mode: OPEN_LAB_MODE.to_string(),
            operating_mode: inner.config.runtime.operating_mode.clone(),
            ready: inner.storage_available,
            storage_available: inner.storage_available,
            archive_available: inner.archive_available,
            media_switch_connected: inner.media_switch_connected,
            recorder_connected: inner.recorder_connected,
            application_gateway_connected: inner.application_gateway_connected,
            assets_total: assets.len(),
            assets_importing: assets
                .iter()
                .filter(|asset| asset.state.starts_with("importing"))
                .count(),
            assets_processing: assets
                .iter()
                .filter(|asset| asset.state.starts_with("processing"))
                .count(),
            assets_ready: assets.iter().filter(|asset| asset.state == "ready").count(),
            assets_failed: assets.iter().filter(|asset| asset.state == "failed").count(),
            assets_approved: assets
                .iter()
                .filter(|asset| asset.approval == "approved")
                .count(),
            preview_ready: assets.iter().filter(|asset| asset.preview_ready).count(),
            broadcast_ready: assets.iter().filter(|asset| asset.broadcast_ready).count(),
            jobs_queued: jobs.iter().filter(|job| job.state == "queued").count(),
            jobs_playing: jobs.iter().filter(|job| job.state == "playing").count(),
            jobs_completed: jobs.iter().filter(|job| job.state == "completed").count(),
            jobs_failed: jobs.iter().filter(|job| job.state == "failed").count(),
            storage_used_bytes: media::total_directory_bytes(&inner.config.storage.root),
            started_at: inner.started_at,
            last_dependency_probe_at: inner.last_dependency_probe_at,
            last_error: inner.storage_last_error.clone(),
        }
    }

    pub fn config_view(&self) -> ConfigView {
        let inner = lock(&self.inner);
        ConfigView {
            server_bind: inner.config.server.bind.to_string(),
            public_base_url: inner.config.server.public_base_url.clone(),
            security_mode: inner.config.security.mode.clone(),
            token_auth: inner.config.security.token_auth,
            tls: inner.config.security.tls,
            allow_delete: inner.config.security.allow_delete,
            operating_mode: inner.config.runtime.operating_mode.clone(),
            storage_root: inner.config.storage.root.clone(),
            archive_root: inner.config.storage.archive_root.clone(),
            max_asset_bytes: inner.config.storage.max_asset_bytes,
            max_total_bytes: inner.config.storage.max_total_bytes,
            ffmpeg_available: inner
                .config
                .codec
                .ffmpeg_command
                .first()
                .is_some_and(|path| Path::new(path).is_file()),
            tetra_encoder_configured: !inner.config.codec.encoder_command.is_empty(),
            tetra_decoder_configured: !inner.config.codec.decoder_command.is_empty(),
            dependencies: BTreeMap::from([
                (
                    "media-switch".to_string(),
                    inner.config.dependencies.media_switch_base_url.clone(),
                ),
                (
                    "recorder".to_string(),
                    inner.config.dependencies.recorder_base_url.clone(),
                ),
                (
                    "application-gateway".to_string(),
                    inner.config.dependencies.application_gateway_base_url.clone(),
                ),
            ]),
        }
    }

    pub fn assets(
        &self,
        query: Option<&str>,
        kind: Option<&str>,
        state: Option<&str>,
        approval: Option<&str>,
        limit: usize,
    ) -> Vec<AssetRecord> {
        let inner = lock(&self.inner);
        let query = query.map(str::to_ascii_lowercase);
        let mut assets = inner
            .state
            .assets
            .values()
            .filter(|asset| kind.is_none_or(|value| asset.kind == value))
            .filter(|asset| state.is_none_or(|value| asset.state == value))
            .filter(|asset| approval.is_none_or(|value| asset.approval == value))
            .filter(|asset| {
                query.as_ref().is_none_or(|query| {
                    asset.title.to_ascii_lowercase().contains(query)
                        || asset.asset_id.to_ascii_lowercase().contains(query)
                        || asset.original_filename.to_ascii_lowercase().contains(query)
                        || asset.tags.iter().any(|tag| tag.to_ascii_lowercase().contains(query))
                        || asset
                            .text
                            .as_ref()
                            .is_some_and(|text| text.to_ascii_lowercase().contains(query))
                })
            })
            .cloned()
            .collect::<Vec<_>>();
        assets.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        assets.truncate(limit);
        assets
    }

    pub fn asset(&self, asset_id: &str) -> Option<AssetRecord> {
        lock(&self.inner).state.assets.get(asset_id).cloned()
    }

    pub fn create_upload(&self, input: UploadInput) -> Result<AssetRecord, String> {
        let bytes = media::decode_base64(&input.data_base64)?;
        let mut inner = lock(&self.inner);
        require_management(&inner)?;
        validate_new_asset_locked(&inner, bytes.len() as u64)?;
        let now = Utc::now();
        let asset_id = Uuid::new_v4().to_string();
        let filename = media::safe_filename(&input.filename, "upload.bin");
        let media_type = input
            .media_type
            .unwrap_or_else(|| media_type_from_filename(&filename));
        let extension = media::extension_for(&filename, &media_type, &bytes);
        let directory = inner.config.storage.root.join(&asset_id);
        let original_path = directory.join(format!("original.{extension}"));
        media::write_atomic(&original_path, &bytes, inner.config.storage.fsync_imports)?;
        let sha256 = media::sha256_bytes(&bytes);
        let duplicate_of = inner
            .state
            .assets
            .values()
            .find(|asset| asset.sha256.as_deref() == Some(&sha256))
            .map(|asset| asset.asset_id.clone());
        if duplicate_of.is_some() {
            inner.state.dedupe_hits = inner.state.dedupe_hits.saturating_add(1);
        }
        let approve = input.approve.unwrap_or(false);
        let asset = AssetRecord {
            asset_id: asset_id.clone(),
            title: nonempty(&input.name, "Untitled media")?,
            description: clean_optional(input.description),
            kind: normalize_kind(input.kind.as_deref().unwrap_or("announcement")),
            state: "processing".to_string(),
            approval: if approve { "approved" } else { "draft" }.to_string(),
            tags: normalize_tags(input.tags),
            source: "local_upload".to_string(),
            source_url: None,
            source_reference: None,
            original_filename: filename,
            media_type,
            original_path: Some(original_path),
            preview_path: None,
            tetra_path: None,
            archive_path: None,
            sha256: Some(sha256),
            preview_sha256: None,
            tetra_sha256: None,
            size_bytes: Some(bytes.len() as u64),
            metadata: Default::default(),
            preview_ready: false,
            broadcast_ready: false,
            archived: false,
            voice: None,
            text: None,
            broadcast_hint: input.broadcast,
            duplicate_of,
            processing_attempts: 0,
            last_error: None,
            created_at: now,
            updated_at: now,
            approved_at: approve.then_some(now),
            approved_by: approve.then(|| actor(input.actor.as_deref())),
        };
        inner.state.assets.insert(asset_id.clone(), asset.clone());
        push_event_locked(&mut inner, "asset_uploaded", Some(&asset_id), None, json!({"bytes": bytes.len()}));
        audit_locked(
            &mut inner,
            actor(input.actor.as_deref()),
            "upload",
            "asset",
            &asset_id,
            "ok",
            json!({"filename": asset.original_filename, "sha256": asset.sha256}),
        );
        persist_locked(&mut inner)?;
        Ok(asset)
    }

    pub fn create_import_url(&self, input: ImportUrlInput) -> Result<AssetRecord, String> {
        let mut inner = lock(&self.inner);
        require_management(&inner)?;
        if !inner.config.security.allow_url_import {
            return Err("URL import is disabled by configuration".to_string());
        }
        validate_import_url(&inner.config, &input.source_url)?;
        validate_new_asset_locked(&inner, input.size_bytes.unwrap_or(0))?;
        if input.schema.as_deref().is_some_and(|schema| schema != "netcore-media-import-v1") {
            return Err("unsupported import schema".to_string());
        }
        let now = Utc::now();
        let asset_id = Uuid::new_v4().to_string();
        let kind = normalize_kind(input.kind.as_deref().unwrap_or("other"));
        let approve = input.approve.unwrap_or_else(|| {
            kind == "tts" && inner.config.runtime.auto_approve_tts
        });
        let filename = input.filename.unwrap_or_else(|| {
            reqwest::Url::parse(&input.source_url)
                .ok()
                .and_then(|url| {
                    url.path_segments()
                        .and_then(Iterator::last)
                        .filter(|value| !value.is_empty())
                        .map(str::to_string)
                })
                .unwrap_or_else(|| format!("{asset_id}.bin"))
        });
        let asset = AssetRecord {
            asset_id: asset_id.clone(),
            title: nonempty(&input.name, "Imported media")?,
            description: None,
            kind,
            state: "importing".to_string(),
            approval: if approve { "approved" } else { "draft" }.to_string(),
            tags: normalize_tags(input.tags),
            source: input.source.unwrap_or_else(|| "url_import".to_string()),
            source_url: Some(input.source_url),
            source_reference: None,
            original_filename: media::safe_filename(&filename, "import.bin"),
            media_type: input.media_type.unwrap_or_else(|| media_type_from_filename(&filename)),
            original_path: None,
            preview_path: None,
            tetra_path: None,
            archive_path: None,
            sha256: input.sha256.map(|value| value.to_ascii_lowercase()),
            preview_sha256: None,
            tetra_sha256: None,
            size_bytes: input.size_bytes,
            metadata: Default::default(),
            preview_ready: false,
            broadcast_ready: false,
            archived: false,
            voice: clean_optional(input.voice),
            text: clean_optional(input.text),
            broadcast_hint: input.broadcast,
            duplicate_of: None,
            processing_attempts: 0,
            last_error: None,
            created_at: now,
            updated_at: now,
            approved_at: approve.then_some(now),
            approved_by: approve.then(|| actor(input.actor.as_deref())),
        };
        inner.state.assets.insert(asset_id.clone(), asset.clone());
        push_event_locked(&mut inner, "url_import_queued", Some(&asset_id), None, json!({"source": asset.source}));
        audit_locked(
            &mut inner,
            actor(input.actor.as_deref()),
            "import-url",
            "asset",
            &asset_id,
            "queued",
            json!({"host": asset.source_url.as_ref().and_then(|url| reqwest::Url::parse(url).ok()).and_then(|url| url.host_str().map(str::to_string))}),
        );
        persist_locked(&mut inner)?;
        Ok(asset)
    }

    pub fn create_recorder_import(&self, input: RecorderImportInput) -> Result<AssetRecord, String> {
        let recording_id = input.recording_id.trim();
        if recording_id.is_empty() || !recording_id.chars().all(|character| character.is_ascii_alphanumeric() || character == '-') {
            return Err("invalid recorder recording_id".to_string());
        }
        let recorder_url = {
            let inner = lock(&self.inner);
            inner.config.dependencies.recorder_base_url.clone()
        };
        let mut asset = self.create_import_url(ImportUrlInput {
            schema: Some("netcore-media-import-v1".to_string()),
            source: Some("recorder".to_string()),
            source_url: format!("{recorder_url}/api/v1/recordings/{recording_id}/audio.tacelp"),
            name: input.name.unwrap_or_else(|| format!("Recording {recording_id}")),
            filename: Some(format!("{recording_id}.tacelp")),
            sha256: None,
            size_bytes: None,
            media_type: Some("application/x-tetra-acelp".to_string()),
            kind: Some("recording".to_string()),
            voice: None,
            text: None,
            tags: vec!["recorder".to_string()],
            approve: input.approve,
            actor: input.actor,
            broadcast: None,
        })?;
        let mut inner = lock(&self.inner);
        if let Some(stored) = inner.state.assets.get_mut(&asset.asset_id) {
            stored.source_reference = Some(recording_id.to_string());
            asset = stored.clone();
            persist_locked(&mut inner)?;
        }
        Ok(asset)
    }

    pub fn update_asset(&self, asset_id: &str, input: AssetUpdateInput) -> Result<AssetRecord, String> {
        let mut inner = lock(&self.inner);
        require_management(&inner)?;
        let asset = inner
            .state
            .assets
            .get_mut(asset_id)
            .ok_or_else(|| "asset not found".to_string())?;
        if let Some(title) = input.title {
            asset.title = nonempty(&title, "asset title")?;
        }
        if input.description.is_some() {
            asset.description = clean_optional(input.description);
        }
        if let Some(kind) = input.kind {
            asset.kind = normalize_kind(&kind);
        }
        if let Some(tags) = input.tags {
            asset.tags = normalize_tags(tags);
        }
        if input.broadcast_hint.is_some() {
            asset.broadcast_hint = input.broadcast_hint;
        }
        asset.updated_at = Utc::now();
        let result = asset.clone();
        audit_locked(&mut inner, actor(input.actor.as_deref()), "update", "asset", asset_id, "ok", json!({}));
        persist_locked(&mut inner)?;
        Ok(result)
    }

    pub fn approve_asset(&self, asset_id: &str, input: ApprovalInput) -> Result<AssetRecord, String> {
        self.set_approval(asset_id, "approved", input)
    }

    pub fn reject_asset(&self, asset_id: &str, input: ApprovalInput) -> Result<AssetRecord, String> {
        self.set_approval(asset_id, "rejected", input)
    }

    fn set_approval(&self, asset_id: &str, approval: &str, input: ApprovalInput) -> Result<AssetRecord, String> {
        let mut inner = lock(&self.inner);
        require_management(&inner)?;
        let now = Utc::now();
        let asset = inner
            .state
            .assets
            .get_mut(asset_id)
            .ok_or_else(|| "asset not found".to_string())?;
        asset.approval = approval.to_string();
        asset.approved_at = (approval == "approved").then_some(now);
        asset.approved_by = (approval == "approved").then(|| actor(input.actor.as_deref()));
        asset.updated_at = now;
        let result = asset.clone();
        push_event_locked(&mut inner, &format!("asset_{approval}"), Some(asset_id), None, json!({"note":input.note}));
        audit_locked(&mut inner, actor(input.actor.as_deref()), approval, "asset", asset_id, "ok", json!({"note":input.note}));
        persist_locked(&mut inner)?;
        Ok(result)
    }

    pub fn reprocess_asset(&self, asset_id: &str, input: ActionInput) -> Result<AssetRecord, String> {
        let mut inner = lock(&self.inner);
        require_management(&inner)?;
        if inner
            .state
            .jobs
            .iter()
            .any(|job| job.asset_id == asset_id && matches!(job.state.as_str(), "queued" | "playing"))
        {
            return Err("asset has a queued or active playout job".to_string());
        }
        let asset = inner
            .state
            .assets
            .get_mut(asset_id)
            .ok_or_else(|| "asset not found".to_string())?;
        if asset.original_path.as_ref().is_none_or(|path| !path.is_file()) {
            return Err("asset has no local original file".to_string());
        }
        asset.state = "processing".to_string();
        asset.preview_path = None;
        asset.tetra_path = None;
        asset.preview_sha256 = None;
        asset.tetra_sha256 = None;
        asset.preview_ready = false;
        asset.broadcast_ready = false;
        asset.last_error = None;
        asset.updated_at = Utc::now();
        let result = asset.clone();
        push_event_locked(&mut inner, "asset_reprocess_queued", Some(asset_id), None, json!({}));
        audit_locked(&mut inner, actor(input.actor.as_deref()), "reprocess", "asset", asset_id, "queued", json!({}));
        persist_locked(&mut inner)?;
        Ok(result)
    }

    pub fn delete_asset(&self, asset_id: &str, input: ActionInput) -> Result<(), String> {
        let mut inner = lock(&self.inner);
        require_management(&inner)?;
        if !inner.config.security.allow_delete {
            return Err("asset deletion is disabled by configuration".to_string());
        }
        if inner
            .state
            .jobs
            .iter()
            .any(|job| job.asset_id == asset_id && matches!(job.state.as_str(), "queued" | "playing"))
        {
            return Err("asset has a queued or active playout job".to_string());
        }
        let current_state = inner
            .state
            .assets
            .get(asset_id)
            .map(|asset| asset.state.clone())
            .ok_or_else(|| "asset not found".to_string())?;
        if matches!(current_state.as_str(), "importing_active" | "processing_active") {
            return Err("asset is currently being imported or processed".to_string());
        }
        let asset = inner
            .state
            .assets
            .remove(asset_id)
            .expect("asset existence checked above");
        let directory = inner.config.storage.root.join(asset_id);
        if directory.exists() {
            fs::remove_dir_all(&directory)
                .map_err(|error| format!("cannot delete {}: {error}", directory.display()))?;
        }
        push_event_locked(&mut inner, "asset_deleted", Some(asset_id), None, json!({"title":asset.title}));
        audit_locked(&mut inner, actor(input.actor.as_deref()), "delete", "asset", asset_id, "ok", json!({"sha256":asset.sha256}));
        persist_locked(&mut inner)
    }

    pub fn archive_asset(&self, asset_id: &str, input: ActionInput) -> Result<AssetRecord, String> {
        let mut inner = lock(&self.inner);
        require_management(&inner)?;
        let archive_root = inner
            .config
            .storage
            .archive_root
            .clone()
            .ok_or_else(|| "archive_root is not configured".to_string())?;
        if !archive_root.is_dir() {
            inner.archive_available = false;
            return Err(format!("archive root {} is not mounted", archive_root.display()));
        }
        let asset = inner
            .state
            .assets
            .get(asset_id)
            .cloned()
            .ok_or_else(|| "asset not found".to_string())?;
        if asset.state != "ready" {
            return Err("only ready assets can be archived".to_string());
        }
        let archive_version = Utc::now().format("%Y%m%dT%H%M%S%.3fZ").to_string();
        let destination = archive_root.join(asset_id).join(archive_version);
        fs::create_dir_all(&destination)
            .map_err(|error| format!("cannot create archive directory: {error}"))?;
        let mut archived_files = Vec::new();
        for source in [asset.original_path.as_ref(), asset.preview_path.as_ref(), asset.tetra_path.as_ref()]
            .into_iter()
            .flatten()
        {
            if source.is_file() {
                let filename = source.file_name().ok_or_else(|| "asset path has no filename".to_string())?;
                let copied = destination.join(filename);
                let size_bytes = media::copy_atomic(
                    source,
                    &copied,
                    inner.config.storage.fsync_imports,
                )?;
                archived_files.push(json!({
                    "filename":filename.to_string_lossy(),
                    "size_bytes":size_bytes,
                    "sha256":media::sha256_file(&copied)?,
                }));
            }
        }
        let manifest = json!({
            "schema":"netcore-media-library-archive-v1",
            "archived_at":Utc::now(),
            "asset":&asset,
            "files":archived_files,
        });
        media::write_atomic(
            &destination.join("manifest.json"),
            &serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?,
            inner.config.storage.fsync_imports,
        )?;
        let stored = inner.state.assets.get_mut(asset_id).expect("asset still exists");
        stored.archived = true;
        stored.archive_path = Some(destination.clone());
        stored.updated_at = Utc::now();
        let result = stored.clone();
        push_event_locked(&mut inner, "asset_archived", Some(asset_id), None, json!({"path":destination}));
        audit_locked(&mut inner, actor(input.actor.as_deref()), "archive", "asset", asset_id, "ok", json!({"path":destination}));
        persist_locked(&mut inner)?;
        Ok(result)
    }

    pub fn create_dispatch(&self, input: DispatchInput) -> Result<DispatchJob, String> {
        let mut inner = lock(&self.inner);
        require_management(&inner)?;
        if input.target_logical_ts.is_some_and(|value| !(1..=7).contains(&value)) {
            return Err("target_logical_ts must be in 1..=7".to_string());
        }
        if inner.state.jobs.len() >= inner.config.runtime.max_jobs {
            return Err("dispatch job limit reached".to_string());
        }
        let asset = inner
            .state
            .assets
            .get(&input.asset_id)
            .cloned()
            .ok_or_else(|| "asset not found".to_string())?;
        if asset.state != "ready" {
            return Err("asset is not ready".to_string());
        }
        if asset.approval != "approved" {
            return Err("asset is not approved".to_string());
        }
        if !asset.broadcast_ready || asset.tetra_path.as_ref().is_none_or(|path| !path.is_file()) {
            return Err("asset has no validated packed TETRA playout cache".to_string());
        }
        let hint = asset.broadcast_hint.clone().unwrap_or_default();
        let session_id = input
            .session_id
            .or(hint.session_id)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "session_id is required; Media Library does not create CMCE calls".to_string())?;
        let now = Utc::now();
        let job = DispatchJob {
            job_id: Uuid::new_v4().to_string(),
            asset_id: asset.asset_id.clone(),
            session_id,
            target_node: input.target_node,
            target_logical_ts: input.target_logical_ts,
            destination_kind: input.destination_kind.or(hint.destination_kind),
            destination_id: input.destination_id.or(hint.destination_id),
            priority: input.priority.or(hint.priority).unwrap_or(3).min(15),
            state: "queued".to_string(),
            frame_index: 0,
            frame_count: asset.metadata.tetra_frame_count.unwrap_or_else(|| {
                asset
                    .tetra_path
                    .as_ref()
                    .and_then(|path| fs::metadata(path).ok())
                    .map(|metadata| metadata.len() / 35)
                    .unwrap_or(0)
            }),
            attempts: 0,
            max_attempts: inner.config.runtime.max_attempts,
            queued_targets: 0,
            cancel_requested: false,
            last_error: None,
            created_at: now,
            updated_at: now,
            started_at: None,
            completed_at: None,
        };
        inner.state.jobs.push(job.clone());
        push_event_locked(&mut inner, "dispatch_queued", Some(&job.asset_id), Some(&job.job_id), json!({"session_id":job.session_id}));
        audit_locked(&mut inner, actor(input.actor.as_deref()), "dispatch", "job", &job.job_id, "queued", json!({"asset_id":job.asset_id,"session_id":job.session_id}));
        persist_locked(&mut inner)?;
        Ok(job)
    }

    pub fn jobs(&self, state: Option<&str>, limit: usize) -> Vec<DispatchJob> {
        let inner = lock(&self.inner);
        let mut jobs = inner
            .state
            .jobs
            .iter()
            .filter(|job| state.is_none_or(|value| job.state == value))
            .cloned()
            .collect::<Vec<_>>();
        jobs.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        jobs.truncate(limit);
        jobs
    }

    pub fn job(&self, job_id: &str) -> Option<DispatchJob> {
        lock(&self.inner)
            .state
            .jobs
            .iter()
            .find(|job| job.job_id == job_id)
            .cloned()
    }

    pub fn cancel_job(&self, job_id: &str, input: ActionInput) -> Result<DispatchJob, String> {
        let mut inner = lock(&self.inner);
        require_management(&inner)?;
        let job = inner
            .state
            .jobs
            .iter_mut()
            .find(|job| job.job_id == job_id)
            .ok_or_else(|| "job not found".to_string())?;
        if !matches!(job.state.as_str(), "queued" | "playing") {
            return Err("only queued or playing jobs can be cancelled".to_string());
        }
        job.cancel_requested = true;
        if job.state == "queued" {
            job.state = "cancelled".to_string();
            job.completed_at = Some(Utc::now());
        }
        job.updated_at = Utc::now();
        let result = job.clone();
        audit_locked(&mut inner, actor(input.actor.as_deref()), "cancel", "job", job_id, "ok", json!({}));
        persist_locked(&mut inner)?;
        Ok(result)
    }

    pub fn retry_job(&self, job_id: &str, input: ActionInput) -> Result<DispatchJob, String> {
        let mut inner = lock(&self.inner);
        require_management(&inner)?;
        let job = inner
            .state
            .jobs
            .iter_mut()
            .find(|job| job.job_id == job_id)
            .ok_or_else(|| "job not found".to_string())?;
        if !matches!(job.state.as_str(), "failed" | "cancelled" | "shadowed") {
            return Err("only failed, cancelled or shadowed jobs can be retried".to_string());
        }
        if job.attempts >= job.max_attempts {
            return Err("job retry limit reached".to_string());
        }
        job.state = "queued".to_string();
        job.frame_index = 0;
        job.queued_targets = 0;
        job.cancel_requested = false;
        job.last_error = None;
        job.started_at = None;
        job.completed_at = None;
        job.updated_at = Utc::now();
        let result = job.clone();
        audit_locked(&mut inner, actor(input.actor.as_deref()), "retry", "job", job_id, "queued", json!({"warning":"playout starts from frame zero"}));
        persist_locked(&mut inner)?;
        Ok(result)
    }

    pub fn events(&self, limit: usize) -> Vec<EventRecord> {
        lock(&self.inner)
            .state
            .events
            .iter()
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn audit(&self, limit: usize) -> Vec<AuditRecord> {
        lock(&self.inner)
            .state
            .audit
            .iter()
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn backups(&self) -> Vec<BackupRecord> {
        lock(&self.inner).state.backups.clone()
    }

    pub fn backup(&self, input: ActionInput) -> Result<BackupRecord, String> {
        let mut inner = lock(&self.inner);
        require_management(&inner)?;
        let backup_id = format!("media-library-{}", Utc::now().format("%Y%m%dT%H%M%SZ"));
        let path = inner.config.storage.backup_root.join(format!("{backup_id}.json"));
        let bytes = serde_json::to_vec_pretty(&inner.state).map_err(|error| error.to_string())?;
        media::write_atomic(&path, &bytes, true)?;
        let record = BackupRecord {
            backup_id: backup_id.clone(),
            path: path.clone(),
            sha256: media::sha256_bytes(&bytes),
            size_bytes: bytes.len() as u64,
            created_at: Utc::now(),
        };
        inner.state.backups.push(record.clone());
        audit_locked(&mut inner, actor(input.actor.as_deref()), "backup", "state", &backup_id, "ok", json!({"path":path}));
        persist_locked(&mut inner)?;
        Ok(record)
    }

    pub fn export(&self) -> Value {
        let inner = lock(&self.inner);
        json!({
            "schema":"netcore-media-library-export-v1",
            "generated_at":Utc::now(),
            "status":self.status_locked(&inner),
            "assets":&inner.state.assets,
            "jobs":&inner.state.jobs,
            "events":&inner.state.events,
            "audit":&inner.state.audit,
            "backups":&inner.state.backups,
        })
    }

    fn status_locked(&self, inner: &LibraryInner) -> Value {
        json!({
            "service":"media-library",
            "operating_mode":&inner.config.runtime.operating_mode,
            "assets":inner.state.assets.len(),
            "jobs":inner.state.jobs.len(),
            "storage_available":inner.storage_available,
        })
    }

    pub fn metrics(&self) -> String {
        let inner = lock(&self.inner);
        let status = self.status_locked_view(&inner);
        format!(
            concat!(
                "# HELP netcore_media_library_up Media Library process state\n",
                "# TYPE netcore_media_library_up gauge\n",
                "netcore_media_library_up 1\n",
                "# TYPE netcore_media_library_ready gauge\n",
                "netcore_media_library_ready {}\n",
                "# TYPE netcore_media_library_assets gauge\n",
                "netcore_media_library_assets{{state=\"total\"}} {}\n",
                "netcore_media_library_assets{{state=\"ready\"}} {}\n",
                "netcore_media_library_assets{{state=\"failed\"}} {}\n",
                "netcore_media_library_assets{{capability=\"preview\"}} {}\n",
                "netcore_media_library_assets{{capability=\"broadcast\"}} {}\n",
                "# TYPE netcore_media_library_jobs gauge\n",
                "netcore_media_library_jobs{{state=\"queued\"}} {}\n",
                "netcore_media_library_jobs{{state=\"playing\"}} {}\n",
                "netcore_media_library_jobs{{state=\"failed\"}} {}\n",
                "# TYPE netcore_media_library_storage_bytes gauge\n",
                "netcore_media_library_storage_bytes {}\n",
                "# TYPE netcore_media_library_dedupe_hits_total counter\n",
                "netcore_media_library_dedupe_hits_total {}\n",
                "# TYPE netcore_media_library_dispatch_frames_total counter\n",
                "netcore_media_library_dispatch_frames_total {}\n"
            ),
            bool_metric(status.ready),
            status.assets_total,
            status.assets_ready,
            status.assets_failed,
            status.preview_ready,
            status.broadcast_ready,
            status.jobs_queued,
            status.jobs_playing,
            status.jobs_failed,
            status.storage_used_bytes,
            inner.state.dedupe_hits,
            inner.state.dispatch_frames_sent,
        )
    }

    fn status_locked_view(&self, inner: &LibraryInner) -> StatusView {
        let assets = inner.state.assets.values().collect::<Vec<_>>();
        StatusView {
            service: "media-library".to_string(),
            security_mode: OPEN_LAB_MODE.to_string(),
            operating_mode: inner.config.runtime.operating_mode.clone(),
            ready: inner.storage_available,
            storage_available: inner.storage_available,
            archive_available: inner.archive_available,
            media_switch_connected: inner.media_switch_connected,
            recorder_connected: inner.recorder_connected,
            application_gateway_connected: inner.application_gateway_connected,
            assets_total: assets.len(),
            assets_importing: assets.iter().filter(|asset| asset.state.starts_with("importing")).count(),
            assets_processing: assets.iter().filter(|asset| asset.state.starts_with("processing")).count(),
            assets_ready: assets.iter().filter(|asset| asset.state == "ready").count(),
            assets_failed: assets.iter().filter(|asset| asset.state == "failed").count(),
            assets_approved: assets.iter().filter(|asset| asset.approval == "approved").count(),
            preview_ready: assets.iter().filter(|asset| asset.preview_ready).count(),
            broadcast_ready: assets.iter().filter(|asset| asset.broadcast_ready).count(),
            jobs_queued: inner.state.jobs.iter().filter(|job| job.state == "queued").count(),
            jobs_playing: inner.state.jobs.iter().filter(|job| job.state == "playing").count(),
            jobs_completed: inner.state.jobs.iter().filter(|job| job.state == "completed").count(),
            jobs_failed: inner.state.jobs.iter().filter(|job| job.state == "failed").count(),
            storage_used_bytes: media::total_directory_bytes(&inner.config.storage.root),
            started_at: inner.started_at,
            last_dependency_probe_at: inner.last_dependency_probe_at,
            last_error: inner.storage_last_error.clone(),
        }
    }

    pub fn file_for(&self, asset_id: &str, kind: &str) -> Result<(PathBuf, &'static str, String), String> {
        let inner = lock(&self.inner);
        let asset = inner.state.assets.get(asset_id).ok_or_else(|| "asset not found".to_string())?;
        let (path, content_type, filename) = match kind {
            "original" => (
                asset.original_path.clone(),
                asset.media_type.as_str(),
                asset.original_filename.clone(),
            ),
            "preview" => (
                asset.preview_path.clone(),
                "audio/wav",
                format!("{}-preview.wav", asset.asset_id),
            ),
            "tacelp" => (
                asset.tetra_path.clone(),
                "application/x-tetra-acelp",
                format!("{}.tacelp", asset.asset_id),
            ),
            _ => return Err("unknown asset file kind".to_string()),
        };
        let path = path.ok_or_else(|| format!("asset has no {kind} file"))?;
        if !media::file_is_within(&path, &inner.config.storage.root) {
            return Err("asset file escaped storage root or is unavailable".to_string());
        }
        let content_type = match content_type {
            "audio/wav" => "audio/wav",
            "audio/mpeg" => "audio/mpeg",
            "application/x-tetra-acelp" => "application/x-tetra-acelp",
            _ => "application/octet-stream",
        };
        Ok((path, content_type, filename))
    }

    pub fn waveform(&self, asset_id: &str, points: usize) -> Result<Vec<f32>, String> {
        let path = {
            let inner = lock(&self.inner);
            inner
                .state
                .assets
                .get(asset_id)
                .and_then(|asset| asset.preview_path.clone())
                .ok_or_else(|| "asset has no preview".to_string())?
        };
        media::waveform(&path, points)
    }

    pub fn claim_import(&self) -> Option<ImportClaim> {
        let mut inner = lock(&self.inner);
        let asset_id = inner
            .state
            .assets
            .values()
            .find(|asset| asset.state == "importing")?
            .asset_id
            .clone();
        let asset = inner.state.assets.get_mut(&asset_id)?;
        let Some(source_url) = asset.source_url.clone() else {
            asset.state = "failed".to_string();
            asset.last_error = Some("importing asset has no source URL".to_string());
            asset.updated_at = Utc::now();
            let _ = persist_locked(&mut inner);
            return None;
        };
        asset.state = "importing_active".to_string();
        asset.updated_at = Utc::now();
        let claim = ImportClaim {
            asset_id: asset.asset_id.clone(),
            source_url,
            expected_sha256: asset.sha256.clone(),
            expected_size_bytes: asset.size_bytes,
            filename: asset.original_filename.clone(),
            media_type: asset.media_type.clone(),
        };
        let _ = persist_locked(&mut inner);
        Some(claim)
    }

    pub fn complete_import(&self, claim: &ImportClaim, bytes: &[u8]) -> Result<AssetRecord, String> {
        let mut inner = lock(&self.inner);
        if !inner.state.assets.contains_key(&claim.asset_id) {
            return Err("import asset was removed before download completed".to_string());
        }
        validate_incoming_bytes_locked(&inner, bytes.len() as u64)?;
        if let Some(expected) = claim.expected_size_bytes
            && expected != bytes.len() as u64
        {
            return Err(format!("import size mismatch: expected {expected}, received {}", bytes.len()));
        }
        let sha256 = media::sha256_bytes(bytes);
        if let Some(expected) = &claim.expected_sha256
            && !expected.eq_ignore_ascii_case(&sha256)
        {
            return Err(format!("import SHA-256 mismatch: expected {expected}, received {sha256}"));
        }
        let extension = media::extension_for(&claim.filename, &claim.media_type, bytes);
        let directory = inner.config.storage.root.join(&claim.asset_id);
        let original_path = directory.join(format!("original.{extension}"));
        media::write_atomic(&original_path, bytes, inner.config.storage.fsync_imports)?;
        let duplicate_of = inner
            .state
            .assets
            .values()
            .find(|asset| asset.asset_id != claim.asset_id && asset.sha256.as_deref() == Some(&sha256))
            .map(|asset| asset.asset_id.clone());
        if duplicate_of.is_some() {
            inner.state.dedupe_hits = inner.state.dedupe_hits.saturating_add(1);
        }
        let asset = inner
            .state
            .assets
            .get_mut(&claim.asset_id)
            .ok_or_else(|| "import asset disappeared".to_string())?;
        asset.original_path = Some(original_path);
        asset.sha256 = Some(sha256);
        asset.size_bytes = Some(bytes.len() as u64);
        asset.duplicate_of = duplicate_of;
        asset.state = "processing".to_string();
        asset.updated_at = Utc::now();
        asset.last_error = None;
        let result = asset.clone();
        inner.state.imports_completed = inner.state.imports_completed.saturating_add(1);
        push_event_locked(&mut inner, "url_import_completed", Some(&claim.asset_id), None, json!({"bytes":bytes.len()}));
        persist_locked(&mut inner)?;
        Ok(result)
    }

    pub fn fail_import(&self, asset_id: &str, error: String) {
        let mut inner = lock(&self.inner);
        if let Some(asset) = inner.state.assets.get_mut(asset_id) {
            asset.state = "failed".to_string();
            asset.last_error = Some(error.clone());
            asset.updated_at = Utc::now();
        }
        push_event_locked(&mut inner, "url_import_failed", Some(asset_id), None, json!({"error":error}));
        let _ = persist_locked(&mut inner);
    }

    pub fn claim_processing(&self) -> Option<ProcessingClaim> {
        let mut inner = lock(&self.inner);
        let asset = inner
            .state
            .assets
            .values_mut()
            .find(|asset| asset.state == "processing")?;
        asset.state = "processing_active".to_string();
        asset.processing_attempts = asset.processing_attempts.saturating_add(1);
        asset.updated_at = Utc::now();
        let claim = ProcessingClaim { asset: asset.clone() };
        let _ = persist_locked(&mut inner);
        Some(claim)
    }

    pub fn complete_processing(&self, asset_id: &str, result: ProcessResult) -> Result<AssetRecord, String> {
        let mut inner = lock(&self.inner);
        let asset = inner
            .state
            .assets
            .get_mut(asset_id)
            .ok_or_else(|| "processing asset disappeared".to_string())?;
        asset.metadata = result.metadata;
        asset.preview_path = result.preview_path;
        asset.tetra_path = result.tetra_path;
        asset.preview_sha256 = result.preview_sha256;
        asset.tetra_sha256 = result.tetra_sha256;
        asset.preview_ready = result.preview_ready;
        asset.broadcast_ready = result.broadcast_ready;
        asset.state = "ready".to_string();
        asset.last_error = None;
        asset.updated_at = Utc::now();
        let asset_result = asset.clone();
        inner.state.processing_completed = inner.state.processing_completed.saturating_add(1);
        push_event_locked(
            &mut inner,
            "asset_processing_completed",
            Some(asset_id),
            None,
            json!({"preview_ready":asset_result.preview_ready,"broadcast_ready":asset_result.broadcast_ready}),
        );
        persist_locked(&mut inner)?;
        Ok(asset_result)
    }

    pub fn fail_processing(&self, asset_id: &str, error: String) {
        let mut inner = lock(&self.inner);
        if let Some(asset) = inner.state.assets.get_mut(asset_id) {
            asset.state = "failed".to_string();
            asset.last_error = Some(error.clone());
            asset.updated_at = Utc::now();
        }
        push_event_locked(&mut inner, "asset_processing_failed", Some(asset_id), None, json!({"error":error}));
        let _ = persist_locked(&mut inner);
    }

    pub fn claim_dispatch(&self) -> Option<DispatchClaim> {
        let mut inner = lock(&self.inner);
        let index = inner.state.jobs.iter().position(|job| job.state == "queued")?;
        let shadow = inner.config.runtime.operating_mode == SHADOW_MODE;
        let asset_id = inner.state.jobs[index].asset_id.clone();
        let tetra_asset = (!shadow)
            .then(|| {
                inner.state.assets.get(&asset_id).and_then(|asset| {
                    asset
                        .tetra_path
                        .clone()
                        .map(|path| (path, asset.tetra_sha256.clone()))
                })
            })
            .flatten();

        let now = Utc::now();
        let job = &mut inner.state.jobs[index];
        job.updated_at = now;
        if shadow {
            job.state = "shadowed".to_string();
            job.completed_at = Some(now);
            let job_id = job.job_id.clone();
            push_event_locked(
                &mut inner,
                "dispatch_shadowed",
                Some(&asset_id),
                Some(&job_id),
                json!({}),
            );
            let _ = persist_locked(&mut inner);
            return None;
        }
        job.attempts = job.attempts.saturating_add(1);
        let Some((tetra_path, expected_tetra_sha256)) = tetra_asset else {
            job.state = "failed".to_string();
            job.last_error = Some("asset TETRA cache disappeared".to_string());
            job.completed_at = Some(now);
            let _ = persist_locked(&mut inner);
            return None;
        };
        job.state = "playing".to_string();
        job.started_at = Some(now);
        job.completed_at = None;
        let claim = DispatchClaim {
            job: job.clone(),
            tetra_path,
            expected_tetra_sha256,
        };
        let _ = persist_locked(&mut inner);
        Some(claim)
    }

    pub fn dispatch_cancel_requested(&self, job_id: &str) -> bool {
        lock(&self.inner)
            .state
            .jobs
            .iter()
            .find(|job| job.job_id == job_id)
            .is_none_or(|job| job.cancel_requested)
    }

    pub fn dispatch_progress(&self, job_id: &str, frame_index: u64, queued_targets: u64) {
        let mut inner = lock(&self.inner);
        let updated = if let Some(job) = inner.state.jobs.iter_mut().find(|job| job.job_id == job_id) {
            job.frame_index = frame_index;
            job.queued_targets = queued_targets;
            job.updated_at = Utc::now();
            true
        } else {
            false
        };
        if updated {
            inner.state.dispatch_frames_sent = inner.state.dispatch_frames_sent.saturating_add(1);
        }
        let _ = persist_locked(&mut inner);
    }

    pub fn complete_dispatch(&self, job_id: &str) {
        let mut inner = lock(&self.inner);
        let mut identifiers = None;
        if let Some(job) = inner.state.jobs.iter_mut().find(|job| job.job_id == job_id) {
            if job.cancel_requested {
                job.state = "cancelled".to_string();
            } else {
                job.state = "completed".to_string();
                job.frame_index = job.frame_count;
            }
            job.updated_at = Utc::now();
            job.completed_at = Some(Utc::now());
            identifiers = Some((job.asset_id.clone(), job.job_id.clone(), job.state.clone()));
        }
        if let Some((asset_id, job_id, state)) = identifiers {
            push_event_locked(&mut inner, &format!("dispatch_{state}"), Some(&asset_id), Some(&job_id), json!({}));
        }
        let _ = persist_locked(&mut inner);
    }

    pub fn fail_dispatch(&self, job_id: &str, error: String) {
        let mut inner = lock(&self.inner);
        let mut identifiers = None;
        if let Some(job) = inner.state.jobs.iter_mut().find(|job| job.job_id == job_id) {
            job.state = "failed".to_string();
            job.last_error = Some(error.clone());
            job.updated_at = Utc::now();
            job.completed_at = Some(Utc::now());
            identifiers = Some((job.asset_id.clone(), job.job_id.clone()));
        }
        if let Some((asset_id, job_id)) = identifiers {
            push_event_locked(&mut inner, "dispatch_failed", Some(&asset_id), Some(&job_id), json!({"error":error}));
        }
        let _ = persist_locked(&mut inner);
    }

    pub fn update_dependency_probe(&self, service: &str, connected: bool, error: Option<String>) {
        let mut inner = lock(&self.inner);
        match service {
            "media-switch" => inner.media_switch_connected = connected,
            "recorder" => inner.recorder_connected = connected,
            "application-gateway" => inner.application_gateway_connected = connected,
            _ => return,
        }
        if let Some(error) = error {
            inner.dependency_errors.insert(service.to_string(), error);
        } else {
            inner.dependency_errors.remove(service);
        }
        inner.last_dependency_probe_at = Some(Utc::now());
    }

    pub fn maintenance(&self, actor_name: Option<String>) -> Result<Value, String> {
        let mut inner = lock(&self.inner);
        require_management(&inner)?;
        let archive_available = inner
            .config
            .storage
            .archive_root
            .as_ref()
            .is_some_and(|path| path.is_dir());
        inner.archive_available = archive_available;
        trim_locked(&mut inner);
        audit_locked(
            &mut inner,
            actor(actor_name.as_deref()),
            "maintenance",
            "service",
            "media-library",
            "ok",
            json!({"archive_available":archive_available}),
        );
        persist_locked(&mut inner)?;
        Ok(json!({"archive_available":archive_available,"assets":inner.state.assets.len(),"jobs":inner.state.jobs.len()}))
    }
}

fn validate_new_asset_locked(inner: &LibraryInner, incoming_bytes: u64) -> Result<(), String> {
    if inner.state.assets.len() >= inner.config.runtime.max_assets {
        return Err("asset inventory limit reached".to_string());
    }
    validate_incoming_bytes_locked(inner, incoming_bytes)
}

fn validate_incoming_bytes_locked(inner: &LibraryInner, incoming_bytes: u64) -> Result<(), String> {
    if incoming_bytes > inner.config.storage.max_asset_bytes {
        return Err(format!(
            "asset exceeds configured {} byte limit",
            inner.config.storage.max_asset_bytes
        ));
    }
    let used = media::total_directory_bytes(&inner.config.storage.root);
    if used.saturating_add(incoming_bytes) > inner.config.storage.max_total_bytes {
        return Err("media library storage limit reached".to_string());
    }
    Ok(())
}

fn validate_import_url(config: &MediaLibraryConfig, value: &str) -> Result<(), String> {
    let url = reqwest::Url::parse(value).map_err(|error| format!("invalid source_url: {error}"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("source_url must use http:// or https://".to_string());
    }
    if url.username() != "" || url.password().is_some() {
        return Err("source_url must not contain inline credentials".to_string());
    }
    let host = url.host_str().ok_or_else(|| "source_url has no host".to_string())?;
    if !config.security.allow_private_import_urls && import_host_is_private(host) {
        return Err("private import URLs are disabled".to_string());
    }
    Ok(())
}


fn import_host_is_private(host: &str) -> bool {
    let host = host.trim_matches(['[', ']']).to_ascii_lowercase();
    if host == "localhost" || host.ends_with(".localhost") || host.ends_with(".local") {
        return true;
    }
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(address)) => {
            address.is_private()
                || address.is_loopback()
                || address.is_link_local()
                || address.is_unspecified()
                || address.is_multicast()
        }
        Ok(IpAddr::V6(address)) => {
            address.is_loopback()
                || address.is_unique_local()
                || address.is_unicast_link_local()
                || address.is_unspecified()
                || address.is_multicast()
        }
        Err(_) => false,
    }
}

fn normalize_kind(value: &str) -> String {
    let value = value.trim().to_ascii_lowercase().replace([' ', '_'], "-");
    match value.as_str() {
        "tts" | "recording" | "announcement" | "alarm" | "music" | "prompt" | "other" => value,
        _ => "other".to_string(),
    }
}

fn normalize_tags(values: Vec<String>) -> BTreeSet<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .take(64)
        .collect()
}

fn nonempty(value: &str, label: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        Err(format!("{label} must not be empty"))
    } else {
        Ok(value.chars().take(200).collect())
    }
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().chars().take(10_000).collect::<String>())
        .filter(|value| !value.is_empty())
}

fn media_type_from_filename(filename: &str) -> String {
    let lower = filename.to_ascii_lowercase();
    if lower.ends_with(".wav") {
        "audio/wav".to_string()
    } else if lower.ends_with(".mp3") {
        "audio/mpeg".to_string()
    } else if lower.ends_with(".tacelp") {
        "application/x-tetra-acelp".to_string()
    } else {
        "application/octet-stream".to_string()
    }
}

fn actor(value: Option<&str>) -> String {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("open-lab-operator")
        .chars()
        .take(120)
        .collect()
}

fn require_management(_inner: &LibraryInner) -> Result<(), String> {
    // Remote reachability is enforced by server.bind. A loopback-only bind must
    // still permit local management requests.
    Ok(())
}

fn push_event_locked(
    inner: &mut LibraryInner,
    kind: &str,
    asset_id: Option<&str>,
    job_id: Option<&str>,
    detail: Value,
) {
    inner.state.next_event_seq = inner.state.next_event_seq.saturating_add(1);
    inner.state.events.push_back(EventRecord {
        seq: inner.state.next_event_seq,
        timestamp: Utc::now(),
        kind: kind.to_string(),
        asset_id: asset_id.map(str::to_string),
        job_id: job_id.map(str::to_string),
        detail,
    });
    while inner.state.events.len() > inner.config.runtime.max_events {
        inner.state.events.pop_front();
    }
}

fn audit_locked(
    inner: &mut LibraryInner,
    actor: String,
    action: &str,
    object_type: &str,
    object_id: &str,
    result: &str,
    detail: Value,
) {
    inner.state.next_audit_seq = inner.state.next_audit_seq.saturating_add(1);
    inner.state.audit.push_back(AuditRecord {
        seq: inner.state.next_audit_seq,
        timestamp: Utc::now(),
        actor,
        action: action.to_string(),
        object_type: object_type.to_string(),
        object_id: object_id.to_string(),
        result: result.to_string(),
        detail,
    });
    while inner.state.audit.len() > inner.config.runtime.max_audit_records {
        inner.state.audit.pop_front();
    }
}

fn trim_locked(inner: &mut LibraryInner) {
    while inner.state.events.len() > inner.config.runtime.max_events {
        inner.state.events.pop_front();
    }
    while inner.state.audit.len() > inner.config.runtime.max_audit_records {
        inner.state.audit.pop_front();
    }
    if inner.state.jobs.len() > inner.config.runtime.max_jobs {
        inner.state.jobs.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        inner.state.jobs.truncate(inner.config.runtime.max_jobs);
    }
}

fn persist_locked(inner: &mut LibraryInner) -> Result<(), String> {
    trim_locked(inner);
    let bytes = serde_json::to_vec_pretty(&inner.state).map_err(|error| error.to_string())?;
    match media::write_atomic(&inner.config.storage.state_file, &bytes, true) {
        Ok(()) => {
            inner.storage_available = true;
            inner.storage_last_error = None;
            for asset in inner.state.assets.values() {
                if let Some(directory) = asset
                    .original_path
                    .as_ref()
                    .and_then(|path| path.parent())
                    .filter(|path| path.starts_with(&inner.config.storage.root))
                {
                    let _ = media::write_atomic(
                        &directory.join("metadata.json"),
                        &serde_json::to_vec_pretty(asset).unwrap_or_default(),
                        false,
                    );
                }
            }
            Ok(())
        }
        Err(error) => {
            inner.storage_available = false;
            inner.storage_last_error = Some(error.clone());
            Err(error)
        }
    }
}

fn bool_metric(value: bool) -> u8 {
    if value { 1 } else { 0 }
}

fn lock(inner: &Arc<Mutex<LibraryInner>>) -> MutexGuard<'_, LibraryInner> {
    inner.lock().expect("media library state poisoned")
}

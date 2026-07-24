use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::thread;
use std::time::{Duration, Instant};

use reqwest::blocking::{Client, Response};
use serde_json::{json, Value};

use crate::config::MediaLibraryConfig;
use crate::media;
use crate::model::{DispatchClaim, ImportClaim};
use crate::state::SharedLibrary;

pub fn spawn_worker(
    config: MediaLibraryConfig,
    library: SharedLibrary,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let client = match build_client(&config) {
            Ok(client) => client,
            Err(error) => {
                tracing::error!("Media Library cannot build HTTP client: {error}");
                return;
            }
        };
        let mut last_probe = Instant::now()
            .checked_sub(Duration::from_secs(config.runtime.probe_interval_secs))
            .unwrap_or_else(Instant::now);
        let mut last_maintenance = Instant::now();
        loop {
            run_cycle(&config, &library, &client, &mut last_probe, &mut last_maintenance);
            thread::sleep(Duration::from_millis(config.runtime.worker_interval_ms));
        }
    })
}

pub fn build_client(config: &MediaLibraryConfig) -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(config.runtime.import_timeout_secs))
        .user_agent(format!("netcore-media-library/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| error.to_string())
}

pub fn run_cycle(
    config: &MediaLibraryConfig,
    library: &SharedLibrary,
    client: &Client,
    last_probe: &mut Instant,
    last_maintenance: &mut Instant,
) {
    if last_probe.elapsed() >= Duration::from_secs(config.runtime.probe_interval_secs) {
        probe_dependencies(config, library, client);
        *last_probe = Instant::now();
    }

    if let Some(claim) = library.claim_import() {
        match download_import(config, client, &claim)
            .and_then(|bytes| library.complete_import(&claim, &bytes).map(|_| ()))
        {
            Ok(()) => tracing::info!(asset_id = %claim.asset_id, "Media import completed"),
            Err(error) => {
                tracing::warn!(asset_id = %claim.asset_id, "Media import failed: {error}");
                library.fail_import(&claim.asset_id, error);
            }
        }
    }

    if let Some(claim) = library.claim_processing() {
        let asset_id = claim.asset.asset_id.clone();
        let result = claim
            .asset
            .original_path
            .as_ref()
            .ok_or_else(|| "asset has no original path".to_string())
            .and_then(|original| {
                let directory = original
                    .parent()
                    .ok_or_else(|| "asset original path has no parent".to_string())?;
                media::process_asset(config, original, directory, &claim.asset.media_type)
            });
        match result.and_then(|result| library.complete_processing(&asset_id, result).map(|_| ())) {
            Ok(()) => tracing::info!(asset_id = %asset_id, "Media processing completed"),
            Err(error) => {
                tracing::warn!(asset_id = %asset_id, "Media processing failed: {error}");
                library.fail_processing(&asset_id, error);
            }
        }
    }

    if let Some(claim) = library.claim_dispatch() {
        if let Err(error) = play_dispatch(config, library, client, &claim) {
            tracing::warn!(job_id = %claim.job.job_id, "Media playout failed: {error}");
            library.fail_dispatch(&claim.job.job_id, error);
        }
    }

    if last_maintenance.elapsed() >= Duration::from_secs(60) {
        let _ = library.maintenance(Some("worker".to_string()));
        *last_maintenance = Instant::now();
    }
}

fn download_import(
    config: &MediaLibraryConfig,
    client: &Client,
    claim: &ImportClaim,
) -> Result<Vec<u8>, String> {
    let response = client
        .get(&claim.source_url)
        .send()
        .map_err(|error| format!("source request failed: {error}"))?;
    let response = require_success(response)?;
    if let Some(length) = response.content_length()
        && length > config.storage.max_asset_bytes
    {
        return Err(format!(
            "source Content-Length {length} exceeds {} byte limit",
            config.storage.max_asset_bytes
        ));
    }
    let mut limited = response.take(config.storage.max_asset_bytes.saturating_add(1));
    let mut bytes = Vec::new();
    limited
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read source response: {error}"))?;
    if bytes.len() as u64 > config.storage.max_asset_bytes {
        return Err("source exceeded configured asset size while downloading".to_string());
    }
    if bytes.is_empty() {
        return Err("source returned an empty body".to_string());
    }
    Ok(bytes)
}

fn require_success(response: Response) -> Result<Response, String> {
    if response.status().is_success() {
        Ok(response)
    } else {
        Err(format!("source returned HTTP {}", response.status()))
    }
}

fn play_dispatch(
    config: &MediaLibraryConfig,
    library: &SharedLibrary,
    client: &Client,
    claim: &DispatchClaim,
) -> Result<(), String> {
    let mut file = File::open(&claim.tetra_path)
        .map_err(|error| format!("cannot open TETRA cache {}: {error}", claim.tetra_path.display()))?;
    let size = file
        .metadata()
        .map_err(|error| format!("cannot stat TETRA cache: {error}"))?
        .len();
    if size == 0 || size % config.codec.frame_bytes as u64 != 0 {
        return Err("TETRA cache is empty or not aligned to 35-byte frames".to_string());
    }
    if let Some(expected) = &claim.expected_tetra_sha256 {
        let actual = media::sha256_file(&claim.tetra_path)?;
        if !expected.eq_ignore_ascii_case(&actual) {
            return Err(format!(
                "TETRA cache integrity mismatch: expected {expected}, received {actual}"
            ));
        }
    }
    let frame_count = size / config.codec.frame_bytes as u64;
    if claim.job.frame_count != 0 && claim.job.frame_count != frame_count {
        tracing::warn!(
            job_id = %claim.job.job_id,
            expected = claim.job.frame_count,
            actual = frame_count,
            "Dispatch frame count changed since queueing"
        );
    }
    file.seek(SeekFrom::Start(claim.job.frame_index.saturating_mul(config.codec.frame_bytes as u64)))
        .map_err(|error| format!("cannot seek TETRA cache: {error}"))?;
    let endpoint = format!(
        "{}/api/v1/sessions/{}/inject",
        config.dependencies.media_switch_base_url,
        url_component(&claim.job.session_id)
    );
    let mut frame_index = claim.job.frame_index;
    let mut queued_targets = claim.job.queued_targets;
    let mut frame = vec![0u8; config.codec.frame_bytes];
    let frame_interval = Duration::from_millis(config.runtime.frame_interval_ms);
    let mut next_deadline = Instant::now();

    while frame_index < frame_count {
        if library.dispatch_cancel_requested(&claim.job.job_id) {
            library.complete_dispatch(&claim.job.job_id);
            return Ok(());
        }
        file.read_exact(&mut frame)
            .map_err(|error| format!("cannot read TETRA frame {frame_index}: {error}"))?;
        let response = client
            .post(&endpoint)
            .timeout(Duration::from_secs(5))
            .json(&json!({
                "payload": frame,
                "target_node": claim.job.target_node.clone(),
                "target_logical_ts": claim.job.target_logical_ts,
            }))
            .send()
            .map_err(|error| format!("Media Switch injection failed at frame {frame_index}: {error}"))?;
        if !response.status().is_success() {
            let status = response.status();
            let excerpt = response.text().unwrap_or_default();
            return Err(format!(
                "Media Switch rejected frame {frame_index} with HTTP {status}: {}",
                excerpt.chars().take(500).collect::<String>()
            ));
        }
        let body = response.json::<Value>().unwrap_or(Value::Null);
        queued_targets = queued_targets.saturating_add(
            body.get("queued_targets")
                .and_then(Value::as_u64)
                .unwrap_or(0),
        );
        frame_index = frame_index.saturating_add(1);
        library.dispatch_progress(&claim.job.job_id, frame_index, queued_targets);
        next_deadline += frame_interval;
        if let Some(delay) = next_deadline.checked_duration_since(Instant::now()) {
            thread::sleep(delay);
        } else {
            next_deadline = Instant::now();
        }
    }
    library.complete_dispatch(&claim.job.job_id);
    Ok(())
}

fn probe_dependencies(config: &MediaLibraryConfig, library: &SharedLibrary, client: &Client) {
    for (service, base_url) in [
        ("media-switch", &config.dependencies.media_switch_base_url),
        ("recorder", &config.dependencies.recorder_base_url),
        (
            "application-gateway",
            &config.dependencies.application_gateway_base_url,
        ),
    ] {
        let outcome = client
            .get(format!("{base_url}/health/live"))
            .timeout(Duration::from_secs(3))
            .send();
        match outcome {
            Ok(response) if response.status().is_success() => {
                library.update_dependency_probe(service, true, None)
            }
            Ok(response) => library.update_dependency_probe(
                service,
                false,
                Some(format!("HTTP {}", response.status())),
            ),
            Err(error) => library.update_dependency_probe(service, false, Some(error.to_string())),
        }
    }
}

fn url_component(value: &str) -> String {
    let mut output = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            output.push(byte as char);
        } else {
            output.push_str(&format!("%{byte:02X}"));
        }
    }
    output
}

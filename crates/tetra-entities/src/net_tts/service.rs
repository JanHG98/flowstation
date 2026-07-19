use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use reqwest::blocking::Client;
use reqwest::redirect::Policy;
use tetra_config::bluestation::{CfgTts, CfgTtsVoice};
use uuid::Uuid;

use crate::net_audio_player::{AudioPlayerHandle, AudioPlayerState, AudioTargetType};

use super::types::{TtsState, TtsStatus, TtsVoiceStatus};

#[derive(Debug, Clone, Copy)]
enum TtsIntent {
    Preview,
    Dispatch {
        target_type: AudioTargetType,
        target_id: u32,
        priority: u8,
    },
}

#[derive(Debug)]
struct ProviderStatus {
    available: bool,
    error: Option<String>,
    last_probe: Option<Instant>,
}

impl Default for ProviderStatus {
    fn default() -> Self {
        Self {
            available: false,
            error: Some("Piper provider has not been checked yet".to_string()),
            last_probe: None,
        }
    }
}

#[derive(Debug)]
struct LiveStatus {
    state: TtsState,
    job_id: Option<String>,
    audio_player_job_id: Option<String>,
    voice_id: Option<String>,
    speed: Option<f32>,
    text_preview: Option<String>,
    file_name: Option<String>,
    generated_path: Option<PathBuf>,
    target_type: Option<AudioTargetType>,
    target_id: Option<u32>,
    priority: Option<u8>,
    dispatch_seen: bool,
    last_error: Option<String>,
}

impl Default for LiveStatus {
    fn default() -> Self {
        Self {
            state: TtsState::Idle,
            job_id: None,
            audio_player_job_id: None,
            voice_id: None,
            speed: None,
            text_preview: None,
            file_name: None,
            generated_path: None,
            target_type: None,
            target_id: None,
            priority: None,
            dispatch_seen: false,
            last_error: None,
        }
    }
}

struct TtsShared {
    config: CfgTts,
    cache_root: PathBuf,
    startup_warning: Option<String>,
    client: Client,
    audio_player: AudioPlayerHandle,
    provider: Mutex<ProviderStatus>,
    live: Mutex<LiveStatus>,
    cancel_requested: AtomicBool,
}

#[derive(Clone)]
pub struct TtsHandle {
    inner: Arc<TtsShared>,
}

impl TtsHandle {
    pub fn new(mut config: CfgTts, audio_player: AudioPlayerHandle) -> Result<Self, String> {
        let configured_cache = PathBuf::from(&config.cache_directory);
        let (cache_root, startup_warning) = match prepare_writable_cache(&configured_cache) {
            Ok(path) => (path, None),
            Err(primary_error) => {
                let candidates = [
                    std::env::temp_dir().join("netcore-tts"),
                    audio_player.root().join(".netcore-tts-cache"),
                ];
                let mut failures = Vec::new();
                let mut selected = None;
                for candidate in candidates {
                    match prepare_writable_cache(&candidate) {
                        Ok(path) => {
                            selected = Some(path);
                            break;
                        }
                        Err(error) => failures.push(format!("{}: {error}", candidate.display())),
                    }
                }
                let Some(path) = selected else {
                    return Err(format!(
                        "TTS cache unavailable at {} ({primary_error}); fallback attempts failed: {}",
                        configured_cache.display(),
                        failures.join("; ")
                    ));
                };
                let warning = format!(
                    "configured TTS cache {} is unavailable ({primary_error}); using fallback {}",
                    configured_cache.display(),
                    path.display()
                );
                tracing::warn!("TTS: {warning}");
                config.cache_directory = path.display().to_string();
                (path, Some(warning))
            }
        };
        cleanup_stale_cache(&cache_root, config.cache_retention_minutes);

        let client = Client::builder()
            .connect_timeout(Duration::from_secs(config.synthesis_timeout_seconds.min(10)))
            .timeout(Duration::from_secs(config.synthesis_timeout_seconds))
            .redirect(Policy::none())
            .user_agent(format!("NetCore-Tetra/{} TTS", tetra_core::STACK_VERSION))
            .build()
            .map_err(|error| format!("cannot create TTS HTTP client: {error}"))?;

        let handle = Self {
            inner: Arc::new(TtsShared {
                config,
                cache_root,
                startup_warning,
                client,
                audio_player,
                provider: Mutex::new(ProviderStatus::default()),
                live: Mutex::new(LiveStatus::default()),
                cancel_requested: AtomicBool::new(false),
            }),
        };

        handle.spawn_monitor();
        Ok(handle)
    }

    pub fn config(&self) -> &CfgTts {
        &self.inner.config
    }

    pub fn cache_root(&self) -> &Path {
        &self.inner.cache_root
    }

    pub fn startup_warning(&self) -> Option<&str> {
        self.inner.startup_warning.as_deref()
    }

    pub fn status(&self) -> TtsStatus {
        let provider = self.inner.provider.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let live = self.inner.live.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        TtsStatus {
            available: true,
            provider_available: provider.available,
            provider_endpoint: self.inner.config.endpoint.clone(),
            provider_error: provider.error.clone(),
            cache_directory: self.inner.cache_root.display().to_string(),
            startup_warning: self.inner.startup_warning.clone(),
            state: live.state,
            job_id: live.job_id.clone(),
            audio_player_job_id: live.audio_player_job_id.clone(),
            voice_id: live.voice_id.clone(),
            speed: live.speed,
            text_preview: live.text_preview.clone(),
            file_name: live.file_name.clone(),
            generated_audio_available: live.generated_path.as_ref().is_some_and(|path| path.is_file()),
            target_type: live.target_type,
            target_id: live.target_id,
            priority: live.priority,
            max_text_characters: self.inner.config.max_text_characters,
            default_voice: self.inner.config.default_voice.clone(),
            default_speed: self.inner.config.default_speed,
            default_priority: self.inner.config.default_priority,
            last_error: live.last_error.clone(),
        }
    }

    pub fn voices(&self) -> Vec<TtsVoiceStatus> {
        let provider = self.inner.provider.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        self.inner
            .config
            .voices
            .iter()
            .map(|voice| TtsVoiceStatus {
                id: voice.id.clone(),
                name: voice.name.clone(),
                provider_voice: voice.provider_voice.clone(),
                speaker_id: voice.speaker_id,
                available: provider.available,
                error: if provider.available { None } else { provider.error.clone() },
            })
            .collect()
    }

    pub fn generate_preview(&self, text: &str, voice_id: Option<&str>, speed: Option<f32>) -> Result<String, String> {
        self.start_job(text, voice_id, speed, TtsIntent::Preview)
    }

    pub fn generate_and_dispatch(
        &self,
        text: &str,
        voice_id: Option<&str>,
        speed: Option<f32>,
        target_type: AudioTargetType,
        target_id: u32,
        priority: Option<u8>,
    ) -> Result<String, String> {
        validate_target(target_id)?;
        let priority = priority.unwrap_or(self.inner.config.default_priority);
        validate_priority(priority)?;
        ensure_audio_player_available(&self.inner.audio_player)?;
        self.start_job(
            text,
            voice_id,
            speed,
            TtsIntent::Dispatch {
                target_type,
                target_id,
                priority,
            },
        )
    }

    pub fn dispatch_ready(
        &self,
        job_id: &str,
        target_type: AudioTargetType,
        target_id: u32,
        priority: Option<u8>,
    ) -> Result<String, String> {
        validate_target(target_id)?;
        let priority = priority.unwrap_or(self.inner.config.default_priority);
        validate_priority(priority)?;
        ensure_audio_player_available(&self.inner.audio_player)?;

        let (path, display_name) = {
            let live = self.inner.live.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            if live.state != TtsState::Ready || live.job_id.as_deref() != Some(job_id) {
                return Err("the requested TTS preview is no longer ready".to_string());
            }
            let path = live
                .generated_path
                .clone()
                .filter(|path| path.is_file())
                .ok_or_else(|| "generated TTS audio is missing".to_string())?;
            let display_name = live.file_name.clone().unwrap_or_else(|| "TTS-Durchsage".to_string());
            (path, display_name)
        };

        let audio_job_id = self
            .inner
            .audio_player
            .play_generated_audio(path, display_name, target_type, target_id, Some(priority))?;
        let mut live = self.inner.live.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if live.job_id.as_deref() != Some(job_id) {
            let _ = self.inner.audio_player.stop();
            return Err("TTS job changed while dispatch was being queued".to_string());
        }
        live.state = TtsState::Dispatching;
        live.audio_player_job_id = Some(audio_job_id.clone());
        live.target_type = Some(target_type);
        live.target_id = Some(target_id);
        live.priority = Some(priority);
        live.dispatch_seen = true;
        Ok(audio_job_id)
    }

    pub fn preview_path(&self, job_id: &str) -> Result<PathBuf, String> {
        let live = self.inner.live.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if live.job_id.as_deref() != Some(job_id) {
            return Err("TTS preview not found".to_string());
        }
        if !matches!(live.state, TtsState::Ready | TtsState::Dispatching | TtsState::Cancelled | TtsState::Failed) {
            return Err("TTS preview is not ready yet".to_string());
        }
        let path = live.generated_path.as_ref().ok_or_else(|| "TTS preview has no generated audio".to_string())?;
        if !path.is_file() || !path.starts_with(&self.inner.cache_root) {
            return Err("TTS preview file is unavailable".to_string());
        }
        Ok(path.clone())
    }

    pub fn stop(&self) -> Result<(), String> {
        self.inner.cancel_requested.store(true, Ordering::SeqCst);
        let (state, path) = {
            let mut live = self.inner.live.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            let state = live.state;
            let path = live.generated_path.clone();
            match state {
                TtsState::Idle => return Ok(()),
                TtsState::Synthesizing => {
                    live.state = TtsState::Cancelled;
                    live.last_error = Some("TTS generation cancelled by operator".to_string());
                }
                TtsState::Dispatching => {
                    live.state = TtsState::Cancelled;
                    live.last_error = Some("TTS dispatch cancelled by operator".to_string());
                }
                TtsState::Ready | TtsState::Failed | TtsState::Cancelled => {
                    *live = LiveStatus::default();
                }
            }
            (state, path)
        };
        if state == TtsState::Dispatching {
            self.inner.audio_player.stop()?;
        }
        if matches!(state, TtsState::Ready | TtsState::Dispatching | TtsState::Failed | TtsState::Cancelled)
            && !self.inner.config.keep_generated_audio
        {
            remove_generated_file(path.as_deref());
        }
        Ok(())
    }

    fn start_job(&self, text: &str, voice_id: Option<&str>, speed: Option<f32>, intent: TtsIntent) -> Result<String, String> {
        self.refresh_provider_if_stale();
        {
            let provider = self.inner.provider.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            if !provider.available {
                return Err(provider
                    .error
                    .clone()
                    .unwrap_or_else(|| "Piper TTS provider is unavailable".to_string()));
            }
        }

        let text = normalize_text(text, self.inner.config.max_text_characters)?;
        let voice = self.resolve_voice(voice_id)?.clone();
        let speed = speed.unwrap_or(self.inner.config.default_speed);
        validate_speed(speed)?;
        cleanup_stale_cache(&self.inner.cache_root, self.inner.config.cache_retention_minutes);

        let job_id = Uuid::new_v4().to_string();
        let display_name = tts_display_name(&text);
        let previous_path = {
            let mut live = self.inner.live.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            if matches!(live.state, TtsState::Synthesizing | TtsState::Dispatching) {
                return Err("a TTS job is already active".to_string());
            }
            let previous_path = live.generated_path.take();
            *live = LiveStatus {
                state: TtsState::Synthesizing,
                job_id: Some(job_id.clone()),
                voice_id: Some(voice.id.clone()),
                speed: Some(speed),
                text_preview: Some(short_text_preview(&text)),
                file_name: Some(display_name),
                target_type: match intent {
                    TtsIntent::Preview => None,
                    TtsIntent::Dispatch { target_type, .. } => Some(target_type),
                },
                target_id: match intent {
                    TtsIntent::Preview => None,
                    TtsIntent::Dispatch { target_id, .. } => Some(target_id),
                },
                priority: match intent {
                    TtsIntent::Preview => None,
                    TtsIntent::Dispatch { priority, .. } => Some(priority),
                },
                ..LiveStatus::default()
            };
            previous_path
        };
        if !self.inner.config.keep_generated_audio {
            remove_generated_file(previous_path.as_deref());
        }
        self.inner.cancel_requested.store(false, Ordering::SeqCst);

        let worker = self.clone();
        let worker_job_id = job_id.clone();
        thread::Builder::new()
            .name("tts-synthesis".into())
            .spawn(move || worker.run_job(worker_job_id, text, voice, speed, intent))
            .map_err(|error| {
                self.mark_failed(&job_id, format!("cannot spawn TTS worker: {error}"));
                format!("cannot spawn TTS worker: {error}")
            })?;
        Ok(job_id)
    }

    fn run_job(&self, job_id: String, text: String, voice: CfgTtsVoice, speed: f32, intent: TtsIntent) {
        let path = match self.synthesize(&job_id, &text, &voice, speed) {
            Ok(path) => path,
            Err(error) => {
                if self.inner.cancel_requested.load(Ordering::SeqCst) {
                    self.mark_cancelled(&job_id, error);
                } else {
                    self.mark_failed(&job_id, error);
                }
                return;
            }
        };

        if self.inner.cancel_requested.load(Ordering::SeqCst) || !self.current_job_matches(&job_id) {
            if !self.inner.config.keep_generated_audio {
                remove_generated_file(Some(&path));
            }
            self.mark_cancelled(&job_id, "TTS generation cancelled by operator".to_string());
            return;
        }

        match intent {
            TtsIntent::Preview => {
                let mut live = self.inner.live.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                if live.job_id.as_deref() == Some(job_id.as_str()) {
                    live.state = TtsState::Ready;
                    live.generated_path = Some(path);
                    live.last_error = None;
                }
            }
            TtsIntent::Dispatch {
                target_type,
                target_id,
                priority,
            } => {
                let display_name = {
                    let live = self.inner.live.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                    live.file_name.clone().unwrap_or_else(|| "TTS-Durchsage".to_string())
                };
                match self
                    .inner
                    .audio_player
                    .play_generated_audio(path.clone(), display_name, target_type, target_id, Some(priority))
                {
                    Ok(audio_job_id) => {
                        let accepted = {
                            let mut live = self.inner.live.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                            if live.job_id.as_deref() == Some(job_id.as_str())
                                && live.state == TtsState::Synthesizing
                                && !self.inner.cancel_requested.load(Ordering::SeqCst)
                            {
                                live.state = TtsState::Dispatching;
                                live.generated_path = Some(path.clone());
                                live.audio_player_job_id = Some(audio_job_id);
                                live.dispatch_seen = true;
                                live.last_error = None;
                                true
                            } else {
                                false
                            }
                        };
                        if !accepted {
                            let _ = self.inner.audio_player.stop();
                            if !self.inner.config.keep_generated_audio {
                                remove_generated_file(Some(&path));
                            }
                            self.mark_cancelled(&job_id, "TTS dispatch cancelled by operator".to_string());
                        }
                    }
                    Err(error) => {
                        if self.inner.cancel_requested.load(Ordering::SeqCst) {
                            if !self.inner.config.keep_generated_audio {
                                remove_generated_file(Some(&path));
                            }
                            self.mark_cancelled(&job_id, "TTS dispatch cancelled by operator".to_string());
                        } else {
                            {
                                let mut live = self.inner.live.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                                if live.job_id.as_deref() == Some(job_id.as_str()) {
                                    live.generated_path = Some(path);
                                }
                            }
                            self.mark_failed(&job_id, format!("TTS generated, but dispatch could not start: {error}"));
                        }
                    }
                }
            }
        }
    }

    fn synthesize(&self, job_id: &str, text: &str, voice: &CfgTtsVoice, speed: f32) -> Result<PathBuf, String> {
        let final_path = self.inner.cache_root.join(format!("{job_id}.wav"));
        let part_path = self.inner.cache_root.join(format!("{job_id}.part.wav"));
        let max_bytes = self.inner.config.max_output_file_mb.saturating_mul(1024 * 1024);
        let length_scale = 1.0_f32 / speed;
        let mut payload = serde_json::json!({
            "text": text,
            "voice": voice.provider_voice,
            "length_scale": length_scale,
        });
        if let Some(speaker_id) = voice.speaker_id {
            payload["speaker_id"] = serde_json::json!(speaker_id);
        }

        tracing::info!(
            "TTS: synthesizing job={} voice={} speed={:.2} chars={}",
            job_id,
            voice.id,
            speed,
            text.chars().count()
        );
        let mut response = self
            .inner
            .client
            .post(&self.inner.config.endpoint)
            .json(&payload)
            .send()
            .map_err(|error| format!("Piper HTTP request failed: {error}"))?;
        if !response.status().is_success() {
            let status = response.status();
            let mut body = String::new();
            let _ = response.by_ref().take(4096).read_to_string(&mut body);
            let body = body.trim();
            return Err(if body.is_empty() {
                format!("Piper returned HTTP {status}")
            } else {
                format!("Piper returned HTTP {status}: {body}")
            });
        }
        if let Some(length) = response.content_length()
            && length > max_bytes
        {
            return Err(format!(
                "Piper output is too large: {:.1} MiB (limit {} MiB)",
                length as f64 / 1_048_576.0,
                self.inner.config.max_output_file_mb
            ));
        }

        let mut output = File::create(&part_path).map_err(|error| format!("cannot create {}: {error}", part_path.display()))?;
        let copied = std::io::copy(&mut response.take(max_bytes.saturating_add(1)), &mut output)
            .map_err(|error| format!("cannot store Piper WAV: {error}"))?;
        output.sync_all().map_err(|error| format!("cannot sync Piper WAV: {error}"))?;
        if copied > max_bytes {
            let _ = fs::remove_file(&part_path);
            return Err(format!("Piper output exceeds {} MiB", self.inner.config.max_output_file_mb));
        }
        validate_wav(&part_path)?;
        fs::rename(&part_path, &final_path)
            .map_err(|error| format!("cannot finalize generated TTS WAV {}: {error}", final_path.display()))?;
        Ok(final_path)
    }

    fn resolve_voice(&self, voice_id: Option<&str>) -> Result<&CfgTtsVoice, String> {
        let id = voice_id
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .unwrap_or(&self.inner.config.default_voice);
        self.inner
            .config
            .voices
            .iter()
            .find(|voice| voice.id == id)
            .ok_or_else(|| format!("unknown TTS voice '{id}'"))
    }

    fn current_job_matches(&self, job_id: &str) -> bool {
        self.inner
            .live
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .job_id
            .as_deref()
            == Some(job_id)
    }

    fn mark_failed(&self, job_id: &str, error: String) {
        tracing::error!("TTS: job={} failed: {}", job_id, error);
        let mut live = self.inner.live.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if live.job_id.as_deref() == Some(job_id) {
            live.state = TtsState::Failed;
            live.last_error = Some(error);
            live.audio_player_job_id = None;
        }
    }

    fn mark_cancelled(&self, job_id: &str, reason: String) {
        let mut live = self.inner.live.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if live.job_id.as_deref() == Some(job_id) {
            live.state = TtsState::Cancelled;
            live.last_error = Some(reason);
            live.audio_player_job_id = None;
        }
    }

    fn spawn_monitor(&self) {
        let handle = self.clone();
        thread::Builder::new()
            .name("tts-monitor".into())
            .spawn(move || loop {
                thread::sleep(Duration::from_millis(500));
                handle.reconcile_dispatch();
                handle.refresh_provider_if_stale();
            })
            .expect("failed to spawn TTS monitor thread");
    }

    fn reconcile_dispatch(&self) {
        let audio = self.inner.audio_player.status();
        let mut completed_path = None;
        let mut live = self.inner.live.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if live.state != TtsState::Dispatching {
            return;
        }
        let Some(expected_job) = live.audio_player_job_id.as_deref() else {
            return;
        };
        if audio.job_id.as_deref() == Some(expected_job) {
            live.dispatch_seen = true;
            if audio.state == AudioPlayerState::Failed {
                live.state = TtsState::Failed;
                live.last_error = audio.last_error.or_else(|| Some("TTS radio dispatch failed".to_string()));
                live.audio_player_job_id = None;
            }
            return;
        }
        if live.dispatch_seen && matches!(audio.state, AudioPlayerState::Idle | AudioPlayerState::Failed) {
            if audio.state == AudioPlayerState::Failed {
                live.state = TtsState::Failed;
                live.last_error = audio.last_error.or_else(|| Some("TTS radio dispatch failed".to_string()));
                live.audio_player_job_id = None;
            } else if self.inner.config.keep_generated_audio {
                live.state = TtsState::Ready;
                live.last_error = None;
                live.audio_player_job_id = None;
                live.target_type = None;
                live.target_id = None;
                live.priority = None;
                live.dispatch_seen = false;
            } else {
                completed_path = live.generated_path.take();
                *live = LiveStatus::default();
            }
        }
        drop(live);
        remove_generated_file(completed_path.as_deref());
    }

    fn refresh_provider_if_stale(&self) {
        let stale = {
            let provider = self.inner.provider.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            provider
                .last_probe
                .map(|instant| instant.elapsed() >= Duration::from_secs(10))
                .unwrap_or(true)
        };
        if stale {
            self.probe_provider();
        }
    }

    fn probe_provider(&self) {
        let url = format!("{}/voices", self.inner.config.endpoint.trim_end_matches('/'));
        let result = self.inner.client.get(url).send().and_then(|response| response.error_for_status());
        let mut provider = self.inner.provider.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        provider.last_probe = Some(Instant::now());
        match result {
            Ok(_) => {
                provider.available = true;
                provider.error = None;
            }
            Err(error) => {
                provider.available = false;
                provider.error = Some(format!("Piper provider unavailable: {error}"));
            }
        }
    }
}

fn normalize_text(text: &str, max_characters: usize) -> Result<String, String> {
    let normalized = text
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .chars()
        .filter(|ch| !ch.is_control() || matches!(ch, '\n' | '\t'))
        .collect::<String>();
    let normalized = normalized.trim().to_string();
    if normalized.is_empty() {
        return Err("TTS text cannot be empty".to_string());
    }
    let count = normalized.chars().count();
    if count > max_characters {
        return Err(format!("TTS text has {count} characters; limit is {max_characters}"));
    }
    Ok(normalized)
}

fn validate_speed(speed: f32) -> Result<(), String> {
    if !speed.is_finite() || !(0.50..=1.50).contains(&speed) {
        return Err("TTS speed must be between 0.50 and 1.50".to_string());
    }
    Ok(())
}

fn validate_target(target_id: u32) -> Result<(), String> {
    if target_id == 0 || target_id > 0x00ff_ffff {
        return Err("target must be a valid 24-bit ISSI/GSSI".to_string());
    }
    Ok(())
}

fn validate_priority(priority: u8) -> Result<(), String> {
    if priority > 15 {
        return Err("priority must be 0-15".to_string());
    }
    Ok(())
}

fn ensure_audio_player_available(player: &AudioPlayerHandle) -> Result<(), String> {
    let state = player.status().state;
    if !matches!(state, AudioPlayerState::Idle | AudioPlayerState::Failed) {
        return Err("an audio transmission is already active".to_string());
    }
    Ok(())
}

fn short_text_preview(text: &str) -> String {
    const LIMIT: usize = 180;
    let mut preview = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if preview.chars().count() > LIMIT {
        preview = preview.chars().take(LIMIT).collect::<String>();
        preview.push('…');
    }
    preview
}

fn tts_display_name(text: &str) -> String {
    let mut short = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if short.chars().count() > 56 {
        short = short.chars().take(56).collect::<String>();
        short.push('…');
    }
    format!("TTS · {short}")
}

fn prepare_writable_cache(path: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(path).map_err(|error| format!("cannot create {}: {error}", path.display()))?;
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("cannot canonicalize {}: {error}", path.display()))?;
    if !canonical.is_dir() {
        return Err(format!("{} is not a directory", canonical.display()));
    }
    let probe = canonical.join(format!(".write-probe-{}", Uuid::new_v4()));
    fs::write(&probe, b"netcore-tts-cache-probe")
        .map_err(|error| format!("{} is not writable: {error}", canonical.display()))?;
    fs::remove_file(&probe).map_err(|error| format!("cannot remove TTS cache probe {}: {error}", probe.display()))?;
    Ok(canonical)
}

fn cleanup_stale_cache(root: &Path, retention_minutes: u64) {
    if retention_minutes == 0 {
        return;
    }
    let cutoff = SystemTime::now()
        .checked_sub(Duration::from_secs(retention_minutes.saturating_mul(60)))
        .unwrap_or(UNIX_EPOCH);
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_file() || !is_tts_cache_name(&entry.file_name().to_string_lossy()) {
            continue;
        }
        let modified = entry.metadata().ok().and_then(|metadata| metadata.modified().ok());
        if modified.is_some_and(|modified| modified <= cutoff)
            && let Err(error) = fs::remove_file(entry.path())
        {
            tracing::warn!("TTS: cannot remove stale cache entry {}: {}", entry.path().display(), error);
        }
    }
}

fn is_tts_cache_name(name: &str) -> bool {
    let Some((uuid, suffix)) = name.split_once('.') else {
        return false;
    };
    Uuid::parse_str(uuid).is_ok() && matches!(suffix, "wav" | "part.wav")
}

fn validate_wav(path: &Path) -> Result<(), String> {
    let metadata = path.metadata().map_err(|error| format!("cannot inspect Piper output: {error}"))?;
    if !metadata.is_file() || metadata.len() < 44 {
        return Err("Piper returned an empty or invalid WAV file".to_string());
    }
    let mut file = File::open(path).map_err(|error| format!("cannot open Piper WAV: {error}"))?;
    let mut header = [0u8; 12];
    file.read_exact(&mut header)
        .map_err(|error| format!("cannot read Piper WAV header: {error}"))?;
    if &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" {
        return Err("Piper response is not a RIFF/WAVE file".to_string());
    }
    Ok(())
}

fn remove_generated_file(path: Option<&Path>) {
    if let Some(path) = path
        && let Err(error) = fs::remove_file(path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!("TTS: cannot remove generated file {}: {}", path.display(), error);
    }
}

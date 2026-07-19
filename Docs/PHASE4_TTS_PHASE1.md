# NetCore TTS Phase 1 — local Piper text announcements

## Scope

Phase 1 adds free-text speech synthesis to **Integrationen → Audio-Zentrale**:

- free-text editor with character limit
- configured voice selection
- speech-speed control
- group or individual target selection
- preview generation and browser playback
- direct “generate and send” workflow
- dispatch of a previously generated preview
- cancellation and live status
- local cache with controlled fallback paths
- complete WAV generation before any TETRA traffic resource is requested

Template storage/editor functionality is deliberately reserved for TTS Phase 2.

## Architecture

```text
Dashboard text
  -> authenticated TTS API
  -> local Piper HTTP provider
  -> complete WAV in /var/cache/netcore/tts
  -> existing AudioPlayer preparation (8 kHz mono + TETRA ACELP)
  -> existing group/individual call setup
  -> TDMA-paced radio playout
```

Provider latency never occurs while a TETRA traffic channel is held.

## Configuration

```toml
[tts]
enabled = true
endpoint = "http://127.0.0.1:5000"
cache_directory = "/var/cache/netcore/tts"
default_voice = "de-thorsten"
default_speed = 0.95
default_priority = 5
max_text_characters = 2000
synthesis_timeout_seconds = 90
max_output_file_mb = 25
cache_retention_minutes = 1440
keep_generated_audio = false

[[tts.voices]]
id = "de-thorsten"
name = "Deutsch – Thorsten"
provider_voice = "de_DE-thorsten-medium"
```

`speed` is operator-facing: `1.0` is normal, `0.95` is five percent slower. Piper receives the
corresponding inverse `length_scale`.

## API

```text
GET       /api/audio/tts/status
GET       /api/audio/tts/voices
GET|HEAD  /api/audio/tts/preview?job_id=<uuid>
POST      /api/audio/tts/generate
POST      /api/audio/tts/dispatch
POST      /api/audio/tts/send
POST      /api/audio/tts/stop
```

Generate preview:

```json
{"text":"Achtung. Dies ist eine Testdurchsage.","voice_id":"de-thorsten","speed":0.95}
```

Generate and dispatch:

```json
{
  "text":"Achtung. Dies ist eine Testdurchsage.",
  "voice_id":"de-thorsten",
  "speed":0.95,
  "target_type":"group",
  "target_id":1001,
  "priority":5
}
```

## Cache behaviour

The configured cache is write-tested at startup. Fallback order:

1. configured `cache_directory`
2. `/tmp/netcore-tts`
3. `<audio_player.directory>/.netcore-tts-cache`

Only UUID-named `.wav` and `.part.wav` files created by this service are cleaned automatically.
Generated audio is removed after a completed radio dispatch unless `keep_generated_audio = true`.

## Safety/validation

- text is normalized and control characters are removed
- Unicode character count is limited
- voice IDs are an explicit allowlist
- speed range is `0.50..=1.50`
- priority range is `0..=15`
- target is a valid non-zero 24-bit ISSI/GSSI
- HTTP redirects are disabled
- provider request has connect and total timeouts
- Content-Length and streamed output are size-limited
- output must contain valid RIFF/WAVE headers
- a complete file is atomically finalized before it becomes previewable or dispatchable
- only one TTS synthesis/dispatch job is active at a time
- existing AudioPlayer locking still guarantees only one radio audio transmission at a time

## Build

```bash
rm -rf target
cargo clean
cargo build --release \
  -p bluestation-bs \
  -p netcore-control-room \
  -p netcore-control-room-operator \
  --features "bluestation-bs/asterisk,bluestation-bs/recording,bluestation-bs/audio-player"
```

No additional Rust feature is required: TTS is compiled with `audio-player` because it depends on
that dispatcher.

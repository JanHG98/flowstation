# NetCore TTS Phase 1.1 — RF reliability hardening

## Reason for the fix

A field log showed two important behaviours:

1. CMCE returned `NetworkCallReady` only a few milliseconds after the initial D-SETUP was queued.
   Audio playback therefore started almost immediately. Very short prompts could finish before a
   subscriber had switched from the control channel to the assigned traffic channel.
2. AudioPlayer returned to `idle` after three seconds, while CMCE group hangtime lasts five seconds.
   A subsequent audio job could reuse the still-open group call with a new Brew UUID. The log showed
   `CMCE FSM: network call start changed brew_uuid`.

The same log did **not** show a subscriber being rejected or deregistered. ISSI 5102 sent normal
`RoamingLocationUpdating` requests; the network accepted them and restored the stored group
affiliations.

## Changes

### 720 ms TCH/S lead-in

`lead_in_silence_blocks = 12` prepends twelve valid ACELP silence blocks before every WAV/MP3/TTS
source. This keeps the traffic channel alive while radios process D-SETUP and tune to TCH/S.

### Full group-release guard

`group_release_guard_seconds = 6` keeps the AudioPlayer in `finishing` until the five-second CMCE
hangtime has expired. A new dispatch therefore starts as a clean new call rather than replacing the
UUID of a call in `NoActiveSpeaker`.

### Dashboard interlock

The TTS send buttons now observe the common AudioPlayer state. During preparation, calling,
playback, or finishing, sending is disabled and the UI explains why. Preview generation remains
available. A local request lock also prevents accidental double submissions before the next status
poll arrives.

### Better diagnostics

The service now logs:

```text
TTS: preview ready ...
TTS: direct dispatch queued ...
TTS: dispatch queued ...
TTS: dispatch completed ...
TTS: dispatch failed ...
```

## Configuration

```toml
[audio_player]
lead_in_silence_blocks = 12
tail_silence_blocks = 3
group_release_guard_seconds = 6
```

Allowed ranges:

- `lead_in_silence_blocks`: 0–40
- `tail_silence_blocks`: 0–20
- `group_release_guard_seconds`: 5–30

One audio block equals 60 ms.

## Piper endpoint

The bundled service and examples use:

```toml
[tts]
endpoint = "http://127.0.0.1:5005"
```

FlowStation probes `/voices` and sends synthesis requests to `/synthesize`.

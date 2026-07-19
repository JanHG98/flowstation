# Phase 3.1 – Audio-player cache fallback fix

## Symptom

The Audio Centre shows `audio-player service unavailable` for status, local media and the NFS browser while recording remains available.

## Cause

Phase 3 creates and canonicalizes the configured cache directory during `AudioPlayerEntity` startup. If the systemd service user cannot create or write `/var/cache/netcore/audio`, entity construction fails before the dashboard receives an `AudioPlayerHandle`. Consequently all audio-player API routes return HTTP 503, including local media routes that do not themselves need the NFS cache.

## Fix

The configured cache is now tested with a real write probe. If it is unavailable, FlowStation tries these writable fallbacks:

1. the configured `audio_player.cache_directory`;
2. `${TMPDIR}/netcore-audio` (normally `/tmp/netcore-audio`);
3. `<audio_player.directory>/.netcore-audio-cache`.

The selected path is written into the runtime audio-player configuration, so NFS preparation uses the same verified cache. A startup warning is exposed through `/api/audio/status` and shown in the Audio Centre. The dashboard service remains available.

The preferred permanent configuration remains a dedicated cache owned by the service user:

```bash
sudo install -d -m 0750 -o bluestation -g bluestation /var/cache/netcore/audio
```

Replace `bluestation:bluestation` with the values shown by:

```bash
systemctl show bluestation-bs -p User -p Group
```

## Expected startup log

Normal configured cache:

```text
WAV/MP3 audio dispatch enabled (local: /var/lib/netcore/audio, cache: /var/cache/netcore/audio, shares: 1)
```

Fallback operation:

```text
AudioPlayer: configured audio cache /var/cache/netcore/audio is unavailable (...); using fallback /tmp/netcore-audio
WAV/MP3 audio dispatch enabled (local: /var/lib/netcore/audio, cache: /tmp/netcore-audio, shares: 1)
```

## Changed files

```text
bins/bluestation-bs/src/main.rs
crates/tetra-entities/src/net_audio_player/service.rs
crates/tetra-entities/src/net_audio_player/types.rs
crates/tetra-entities/src/net_dashboard/html.rs
Docs/PHASE3_AUDIO_PLAYER_CACHE_FALLBACK_FIX.md
```

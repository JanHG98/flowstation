# Phase 4 – Common Audio Launch Gate

## Problem

TTS materialization and saved recordings already use the same `AudioPlayerHandle::play_recording()` path. The remaining timing difference was that a generated TTS job could finish preparation and start a network group call immediately after CPU- and filesystem-heavy synthesis/conversion work. A saved recording usually reached the call-control path from a quieter scheduler state.

## Fix

All group audio, regardless of source, now enters one common launch gate after complete ACELP preparation:

1. The complete WAV is decoded and all ACELP blocks are prepared.
2. The prepared job waits for a 1000 ms settle guard.
3. The call starts only at a non-special TDMA frame on timeslot 4, providing a clean lead into the following MCCH opportunity.
4. TTS, recordings and other group audio use exactly the same gate and call-start code.

Frames 1, 17 and 18 are skipped for launch because they may carry special broadcast/common-SCCH activity. Individual audio calls retain their existing immediate launch behavior.

## New log lines

- `AudioPlayer: prepared media queued for common launch gate ...`
- `AudioPlayer: launching prepared media through common recording/TTS gate ...`

The existing `AudioPlayer: prepared ...` and `CMCE: starting NEW network call ...` messages follow only after the common gate opens.

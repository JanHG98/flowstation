#!/usr/bin/env python3
from __future__ import annotations

import base64
import hashlib
import struct

FRAME_BYTES = 35


def canonical_wav(samples: list[int], rate: int = 8000) -> bytes:
    data = b"".join(struct.pack("<h", sample) for sample in samples)
    return (
        b"RIFF" + struct.pack("<I", 36 + len(data)) + b"WAVE"
        + b"fmt " + struct.pack("<IHHIIHH", 16, 1, 1, rate, rate * 2, 2, 16)
        + b"data" + struct.pack("<I", len(data)) + data
    )


def inspect_wav(raw: bytes) -> tuple[int, int, int, int]:
    assert raw[:4] == b"RIFF" and raw[8:12] == b"WAVE"
    offset = 12
    channels = rate = bits = data_len = None
    while offset + 8 <= len(raw):
        chunk = raw[offset:offset + 4]
        length = struct.unpack_from("<I", raw, offset + 4)[0]
        start = offset + 8
        if chunk == b"fmt ":
            _, channels, rate, _, _, bits = struct.unpack_from("<HHIIHH", raw, start)
        elif chunk == b"data":
            data_len = length
        offset = start + length + (length & 1)
    assert None not in (channels, rate, bits, data_len)
    return channels, rate, bits, data_len


def waveform(raw: bytes, points: int) -> list[float]:
    data_start = raw.index(b"data") + 8
    samples = [x[0] for x in struct.iter_unpack("<h", raw[data_start:])]
    chunk = max(1, (len(samples) + points - 1) // points)
    return [max(abs(v) for v in samples[i:i + chunk]) / 32767 for i in range(0, len(samples), chunk)]


def main() -> None:
    wav = canonical_wav([0, 1000, -2000, 32767, -32768] * 100)
    assert inspect_wav(wav)[:3] == (1, 8000, 16)
    encoded = base64.b64encode(wav).decode()
    assert base64.b64decode(encoded) == wav
    assert len(hashlib.sha256(wav).hexdigest()) == 64
    peaks = waveform(wav, 64)
    assert peaks and max(peaks) > 0.99

    tacelp = bytes(range(FRAME_BYTES)) * 12
    assert len(tacelp) % FRAME_BYTES == 0
    assert len(tacelp) // FRAME_BYTES == 12
    assert 12 * 60 == 720
    assert (tacelp + b"x") and len(tacelp + b"x") % FRAME_BYTES != 0

    job = {"frame_index": 0, "frame_count": 12, "state": "queued"}
    job["state"] = "playing"
    for _ in range(job["frame_count"]):
        job["frame_index"] += 1
    job["state"] = "completed"
    assert job == {"frame_index": 12, "frame_count": 12, "state": "completed"}

    print("Media Library reference model: OK")


if __name__ == "__main__":
    main()

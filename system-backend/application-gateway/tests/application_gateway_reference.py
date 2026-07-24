#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import tempfile
from pathlib import Path


def render(body: str, event: dict, json_mode: bool = False) -> str:
    def value(raw: str) -> str:
        if not json_mode:
            return raw
        return json.dumps(raw, ensure_ascii=False)[1:-1]

    result = body
    for key in ("source", "event_type", "destination", "text"):
        result = result.replace("{{" + key + "}}", value(str(event.get(key) or "")))
    for key, raw in event.get("payload", {}).items():
        if raw is None:
            raw = ""
        elif not isinstance(raw, str):
            raw = json.dumps(raw, separators=(",", ":"))
        result = result.replace("{{payload." + key + "}}", value(raw))
    return result


def rule_matches(rule: dict, event: dict) -> bool:
    source = rule["source_connector"] in ("*", event["source_connector"])
    kind = rule["event_type"] in ("*", event["event_type"])
    needle = rule.get("text_contains")
    text = needle is None or needle.lower() in (event.get("text") or "").lower()
    return source and kind and text


def backoff(base: int, maximum: int, attempts: int) -> int:
    return min(base * (2 ** max(0, attempts - 1)), maximum)


def wav_bytes() -> bytes:
    # Minimal PCM RIFF/WAVE with one silent 16-bit mono sample.
    data = b"\x00\x00"
    header = (
        b"RIFF" + (36 + len(data)).to_bytes(4, "little") + b"WAVE"
        + b"fmt " + (16).to_bytes(4, "little") + (1).to_bytes(2, "little")
        + (1).to_bytes(2, "little") + (8000).to_bytes(4, "little")
        + (16000).to_bytes(4, "little") + (2).to_bytes(2, "little")
        + (16).to_bytes(2, "little") + b"data" + len(data).to_bytes(4, "little")
    )
    return header + data


def main() -> None:
    event = {
        "source": "geoalarm",
        "source_connector": "geoalarm",
        "event_type": "alarm.created",
        "destination": "2000",
        "text": 'Tür "Nord" offen',
        "payload": {"zone": 4},
    }
    template = '{"source":"{{source}}","text":"{{text}}","zone":"{{payload.zone}}"}'
    rendered = json.loads(render(template, event, json_mode=True))
    assert rendered == {"source": "geoalarm", "text": 'Tür "Nord" offen', "zone": "4"}

    assert rule_matches(
        {"source_connector": "geoalarm", "event_type": "alarm.created", "text_contains": "nord"},
        event,
    )
    assert not rule_matches(
        {"source_connector": "telegram", "event_type": "alarm.created", "text_contains": None},
        event,
    )
    assert [backoff(2, 120, n) for n in range(1, 9)] == [2, 4, 8, 16, 32, 64, 120, 120]

    with tempfile.TemporaryDirectory() as directory:
        path = Path(directory) / "tts.wav"
        path.write_bytes(wav_bytes())
        raw = path.read_bytes()
        assert len(raw) >= 44 and raw[:4] == b"RIFF" and raw[8:12] == b"WAVE"
        assert len(hashlib.sha256(raw).hexdigest()) == 64

    print("Application Gateway reference model: OK")


if __name__ == "__main__":
    main()

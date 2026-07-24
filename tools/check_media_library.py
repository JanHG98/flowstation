#!/usr/bin/env python3
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]
REQUIRED = [
    "system-backend/media-library/Cargo.toml",
    "system-backend/media-library/README.md",
    "system-backend/media-library/src/main.rs",
    "system-backend/media-library/src/config.rs",
    "system-backend/media-library/src/model.rs",
    "system-backend/media-library/src/media.rs",
    "system-backend/media-library/src/state.rs",
    "system-backend/media-library/src/worker.rs",
    "system-backend/media-library/src/http.rs",
    "system-backend/media-library/config/media-library.example.toml",
    "system-backend/media-library/systemd/netcore-media-library.service",
    "system-backend/media-library/install/install.sh",
    "system-backend/media-library/install/update.sh",
    "system-backend/media-library/install/uninstall.sh",
    "system-backend/media-library/tests/media_library_reference.py",
    "Docs/SWMI_CORE_1_PACKAGE_O_MEDIA_LIBRARY.md",
    ".github/workflows/swmi-core-media-library.yml",
]
MARKERS = {
    "Cargo.toml": "system-backend/media-library",
    "Cargo.lock": 'name = "netcore-media-library"',
    "system-backend/services.toml": "management_port = 8230",
    "system-backend/media-library/src/config.rs": "0.0.0.0:8230",
    "system-backend/media-library/src/config.rs#2": "token_auth: false",
    "system-backend/media-library/src/media.rs": "TETRA_FRAME_BYTES: usize = 35",
    "system-backend/media-library/src/media.rs#2": "inspect_wav",
    "system-backend/media-library/src/media.rs#3": "command_partial_path",
    "system-backend/media-library/src/model.rs": "Deserialize, Default)]\npub struct ApprovalInput",
    "system-backend/media-library/src/state.rs": 'approval != "approved"',
    "system-backend/media-library/src/state.rs#2": "claim_dispatch",
    "system-backend/media-library/src/worker.rs": "/api/v1/sessions/{}/inject",
    "system-backend/media-library/src/http.rs": "/api/v1/assets/import-url",
    "system-backend/media-library/src/http.rs#2": "OPEN LAB",
    "system-backend/recorder/src/http.rs": "audio.tacelp",
    "system-backend/application-gateway/config/application-gateway.example.toml": 'endpoint = "http://127.0.0.1:8230/api/v1/assets/import-url"',
    "system-backend/observability/config/observability.example.toml": 'target_id = "media-library"',
    "system-backend/control-room/config/control-room.example.toml": 'name = "media-library"',
    "Docs/BACKEND_WEBUI_SERVICE_MATRIX.md": "kontrollierte Einspeisung in bestehende Media-Switch-Sessions",
}


def rust_balanced(path: Path) -> str | None:
    text = path.read_text(errors="replace")
    stack: list[tuple[str, int]] = []
    pairs = {")": "(", "]": "[", "}": "{"}
    i = 0
    line = 1
    while i < len(text):
        if text[i] == "\n":
            line += 1; i += 1; continue
        if text.startswith("//", i):
            end = text.find("\n", i); i = len(text) if end < 0 else end; continue
        if text.startswith("/*", i):
            depth = 1; i += 2
            while i < len(text) and depth:
                if text.startswith("/*", i): depth += 1; i += 2
                elif text.startswith("*/", i): depth -= 1; i += 2
                else:
                    if text[i] == "\n": line += 1
                    i += 1
            if depth: return f"unterminated block comment at line {line}"
            continue
        if text[i] == "r":
            match = re.match(r'r(#{0,255})"', text[i:])
            if match:
                hashes = match.group(1); end_marker = '"' + hashes
                start = i + 2 + len(hashes); end = text.find(end_marker, start)
                if end < 0: return f"unterminated raw string at line {line}"
                line += text[i:end + len(end_marker)].count("\n")
                i = end + len(end_marker); continue
        if text[i] == '"':
            i += 1
            while i < len(text):
                if text[i] == "\\": i += 2; continue
                if text[i] == '"': i += 1; break
                if text[i] == "\n": line += 1
                i += 1
            continue
        if text[i] == "'":
            end = i + 1
            while end < min(len(text), i + 16):
                if text[end] == "'" and text[end - 1] != "\\": i = end + 1; break
                end += 1
            else: i += 1
            continue
        char = text[i]
        if char in "([{" : stack.append((char, line))
        elif char in ")]}" :
            if not stack or stack[-1][0] != pairs[char]: return f"delimiter mismatch {char} at line {line}"
            stack.pop()
        i += 1
    if stack: return f"unclosed delimiter {stack[-1][0]} from line {stack[-1][1]}"
    return None


def main() -> int:
    errors: list[str] = []
    for relative in REQUIRED:
        if not (ROOT / relative).is_file(): errors.append(f"missing {relative}")
    for synthetic, marker in MARKERS.items():
        relative = synthetic.split("#", 1)[0]
        path = ROOT / relative
        if not path.is_file() or marker not in path.read_text(errors="replace"):
            errors.append(f"missing marker {marker!r} in {relative}")

    for relative in [
        "Cargo.toml", "system-backend/services.toml",
        "system-backend/media-library/Cargo.toml",
        "system-backend/media-library/config/media-library.example.toml",
        "system-backend/application-gateway/config/application-gateway.example.toml",
        "system-backend/observability/config/observability.example.toml",
        "system-backend/control-room/config/control-room.example.toml",
    ]:
        try: tomllib.loads((ROOT / relative).read_text())
        except Exception as error: errors.append(f"invalid TOML {relative}: {error}")

    try:
        config = tomllib.loads((ROOT / "system-backend/media-library/config/media-library.example.toml").read_text())
        if config["security"]["mode"] != "open_lab" or config["security"]["token_auth"] or config["security"]["tls"]:
            errors.append("Media Library example must remain open_lab without token/TLS")
        if config["server"]["bind"].split(":")[-1] != "8230": errors.append("Media Library port must be 8230")
        if config["runtime"]["operating_mode"] != "shadow": errors.append("Example must default to shadow")
        if config["codec"]["frame_bytes"] != 35: errors.append("TETRA frame size must be 35")
        if config["runtime"]["frame_interval_ms"] != 60: errors.append("TETRA playout interval must be 60 ms")
    except Exception:
        pass

    for relative in [
        "system-backend/media-library/src/main.rs",
        "system-backend/media-library/src/config.rs",
        "system-backend/media-library/src/model.rs",
        "system-backend/media-library/src/media.rs",
        "system-backend/media-library/src/state.rs",
        "system-backend/media-library/src/worker.rs",
        "system-backend/media-library/src/http.rs",
        "system-backend/recorder/src/state.rs",
        "system-backend/recorder/src/http.rs",
    ]:
        error = rust_balanced(ROOT / relative)
        if error: errors.append(f"{relative}: {error}")

    webui = (ROOT / "system-backend/media-library/src/http.rs").read_text()
    match = re.search(r"<script>(.*)</script>", webui, flags=re.S)
    if not match:
        errors.append("Media Library WebUI JavaScript not found")
    else:
        with tempfile.NamedTemporaryFile("w", suffix=".js", delete=False) as handle:
            handle.write(match.group(1)); js_path = Path(handle.name)
        try:
            result = subprocess.run(["node", "--check", str(js_path)], capture_output=True, text=True)
            if result.returncode: errors.append(f"WebUI JavaScript invalid: {result.stderr.strip()}")
        except FileNotFoundError:
            pass
        finally:
            js_path.unlink(missing_ok=True)

    for script in sorted((ROOT / "system-backend/media-library/install").glob("*.sh")):
        result = subprocess.run(["bash", "-n", str(script)], capture_output=True, text=True)
        if result.returncode: errors.append(f"shell syntax {script.relative_to(ROOT)}: {result.stderr.strip()}")

    result = subprocess.run([sys.executable, str(ROOT / "system-backend/media-library/tests/media_library_reference.py")], capture_output=True, text=True)
    if result.returncode: errors.append(f"reference test failed: {result.stderr.strip() or result.stdout.strip()}")

    if errors:
        print("\n".join(errors), file=sys.stderr); return 1
    print("Media Library static package check: OK"); return 0


if __name__ == "__main__":
    raise SystemExit(main())

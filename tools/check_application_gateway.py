#!/usr/bin/env python3
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]
REQUIRED = [
    "system-backend/application-gateway/Cargo.toml",
    "system-backend/application-gateway/README.md",
    "system-backend/application-gateway/src/main.rs",
    "system-backend/application-gateway/src/config.rs",
    "system-backend/application-gateway/src/model.rs",
    "system-backend/application-gateway/src/state.rs",
    "system-backend/application-gateway/src/worker.rs",
    "system-backend/application-gateway/src/http.rs",
    "system-backend/application-gateway/config/application-gateway.example.toml",
    "system-backend/application-gateway/systemd/netcore-application-gateway.service",
    "system-backend/application-gateway/install/install.sh",
    "system-backend/application-gateway/install/update.sh",
    "system-backend/application-gateway/install/uninstall.sh",
    "system-backend/application-gateway/tests/application_gateway_reference.py",
    "Docs/SWMI_CORE_1_PACKAGE_N_APPLICATION_GATEWAY.md",
    ".github/workflows/swmi-core-application-gateway.yml",
]
MARKERS = {
    "system-backend/application-gateway/src/main.rs": "worker::spawn_worker",
    "system-backend/application-gateway/src/config.rs": "0.0.0.0:8220",
    "system-backend/application-gateway/src/config.rs#2": "management_token_auth: false",
    "system-backend/application-gateway/src/config.rs#3": "default_connectors",
    "system-backend/application-gateway/src/state.rs": "dead_letter",
    "system-backend/application-gateway/src/state.rs#2": "secret_statuses_locked",
    "system-backend/application-gateway/src/worker.rs": "send_piper",
    "system-backend/application-gateway/src/worker.rs#2": "validate_wav",
    "system-backend/application-gateway/src/http.rs": "/api/v1/webhooks/",
    "system-backend/application-gateway/src/http.rs#2": "OPEN LAB",
    "system-backend/services.toml": "management_port = 8220",
    "Cargo.toml": "system-backend/application-gateway",
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
        if char in "([{": stack.append((char, line))
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

    tomls = [
        "Cargo.toml",
        "system-backend/services.toml",
        "system-backend/application-gateway/Cargo.toml",
        "system-backend/application-gateway/config/application-gateway.example.toml",
    ]
    for relative in tomls:
        try: tomllib.loads((ROOT / relative).read_text())
        except Exception as error: errors.append(f"invalid TOML {relative}: {error}")

    try:
        config = tomllib.loads((ROOT / "system-backend/application-gateway/config/application-gateway.example.toml").read_text())
        if config["security"]["mode"] != "open_lab" or config["security"]["management_token_auth"] or config["security"]["management_tls"]:
            errors.append("Application Gateway example must remain open_lab without management token/TLS")
        if config["server"]["bind"].split(":")[-1] != "8220": errors.append("Application Gateway port must be 8220")
        if config["runtime"]["operating_mode"] != "shadow": errors.append("Example must default to shadow mode")
        if len(config.get("connectors", [])) < 12: errors.append("Example must contain the planned connector inventory")
        if len(config.get("rules", [])) < 2: errors.append("Example must contain baseline routing rules")
        if len(config.get("templates", [])) < 3: errors.append("Example must contain text, JSON and TTS templates")
        ids = {row["connector_id"] for row in config.get("connectors", [])}
        for required in {"sds-router", "piper-tts", "media-library", "telegram", "dapnet", "meshcom", "snom", "geoalarm", "weather", "tpg2200", "directory", "generic-webhook"}:
            if required not in ids: errors.append(f"missing connector {required}")
    except Exception:
        pass

    for relative in [
        "system-backend/application-gateway/src/main.rs",
        "system-backend/application-gateway/src/config.rs",
        "system-backend/application-gateway/src/model.rs",
        "system-backend/application-gateway/src/state.rs",
        "system-backend/application-gateway/src/worker.rs",
        "system-backend/application-gateway/src/http.rs",
    ]:
        error = rust_balanced(ROOT / relative)
        if error: errors.append(f"{relative}: {error}")

    webui = (ROOT / "system-backend/application-gateway/src/http.rs").read_text()
    match = re.search(r"<script>(.*)</script>", webui, flags=re.S)
    if not match:
        errors.append("Application Gateway WebUI JavaScript not found")
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

    for script in sorted((ROOT / "system-backend/application-gateway/install").glob("*.sh")):
        result = subprocess.run(["bash", "-n", str(script)], capture_output=True, text=True)
        if result.returncode: errors.append(f"shell syntax {script.relative_to(ROOT)}: {result.stderr.strip()}")

    result = subprocess.run([sys.executable, str(ROOT / "system-backend/application-gateway/tests/application_gateway_reference.py")], capture_output=True, text=True)
    if result.returncode: errors.append(f"reference test failed: {result.stderr.strip() or result.stdout.strip()}")

    if errors:
        print("\n".join(errors), file=sys.stderr); return 1
    print("Application Gateway static package check: OK"); return 0


if __name__ == "__main__":
    raise SystemExit(main())

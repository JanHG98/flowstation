#!/usr/bin/env python3
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]
REQUIRED = [
    "system-backend/kmf/Cargo.toml",
    "system-backend/kmf/src/main.rs",
    "system-backend/kmf/src/config.rs",
    "system-backend/kmf/src/crypto.rs",
    "system-backend/kmf/src/protocol.rs",
    "system-backend/kmf/src/state.rs",
    "system-backend/kmf/src/http.rs",
    "system-backend/kmf/config/kmf.example.toml",
    "system-backend/kmf/systemd/netcore-kmf.service",
    "system-backend/kmf/install/install.sh",
    "system-backend/kmf/install/update.sh",
    "system-backend/kmf/install/uninstall.sh",
    "system-backend/kmf/tests/lab_edge_unwrap.py",
    "Docs/SWMI_CORE_1_PACKAGE_J_KMF.md",
]
MARKERS = {
    "Cargo.toml": '"system-backend/kmf"',
    "Cargo.lock": 'name = "netcore-kmf"',
    "system-backend/services.toml": "management_port = 8190",
    "system-backend/kmf/src/state.rs": "raw_keys_exposed_by_management_api: false",
    "system-backend/kmf/src/http.rs": "/api/v1/edge/actions/claim",
    "system-backend/kmf/src/crypto.rs": "lab_sha256_stream_mac_v1",
    "system-backend/kmf/systemd/netcore-kmf.service": "UMask=0077",
    "system-backend/kmf/README.md": "CCK/GCK/SCK",
}
FORBIDDEN_MANAGEMENT_EXPOSURE = [
    '"raw_key"',
    '"key_hex"',
    '"master_key_hex"',
    '"transport_secret_hex": node_secret',
]


def rust_balanced(path: Path) -> str | None:
    text = path.read_text(errors="replace")
    stack: list[tuple[str, int]] = []
    pairs = {')': '(', ']': '[', '}': '{'}
    i = 0
    line = 1
    while i < len(text):
        if text[i] == "\n":
            line += 1
            i += 1
            continue
        if text.startswith("//", i):
            end = text.find("\n", i)
            i = len(text) if end < 0 else end
            continue
        if text.startswith("/*", i):
            depth = 1
            i += 2
            while i < len(text) and depth:
                if text.startswith("/*", i):
                    depth += 1
                    i += 2
                elif text.startswith("*/", i):
                    depth -= 1
                    i += 2
                else:
                    if text[i] == "\n":
                        line += 1
                    i += 1
            if depth:
                return f"unterminated block comment at line {line}"
            continue
        if text[i] == "r":
            match = re.match(r'r(#{0,255})"', text[i:])
            if match:
                hashes = match.group(1)
                start_len = 2 + len(hashes)
                end_marker = '"' + hashes
                end = text.find(end_marker, i + start_len)
                if end < 0:
                    return f"unterminated raw string at line {line}"
                line += text[i:end + len(end_marker)].count("\n")
                i = end + len(end_marker)
                continue
        if text[i] == '"':
            i += 1
            while i < len(text):
                if text[i] == "\\":
                    i += 2
                    continue
                if text[i] == '"':
                    i += 1
                    break
                if text[i] == "\n":
                    line += 1
                i += 1
            continue
        if text[i] == "'":
            end = i + 1
            while end < min(len(text), i + 12):
                if text[end] == "'" and text[end - 1] != "\\":
                    i = end + 1
                    break
                end += 1
            else:
                i += 1
            continue
        char = text[i]
        if char in "([{":
            stack.append((char, line))
        elif char in ")]}":
            if not stack or stack[-1][0] != pairs[char]:
                return f"delimiter mismatch {char} at line {line}"
            stack.pop()
        i += 1
    if stack:
        return f"unclosed delimiter {stack[-1][0]} from line {stack[-1][1]}"
    return None


def main() -> int:
    errors: list[str] = []
    for relative in REQUIRED:
        if not (ROOT / relative).is_file():
            errors.append(f"missing {relative}")
    for relative, marker in MARKERS.items():
        path = ROOT / relative
        if not path.is_file() or marker not in path.read_text(errors="replace"):
            errors.append(f"missing marker {marker!r} in {relative}")

    for relative in [
        "Cargo.toml",
        "system-backend/services.toml",
        "system-backend/kmf/Cargo.toml",
        "system-backend/kmf/config/kmf.example.toml",
    ]:
        try:
            tomllib.loads((ROOT / relative).read_text())
        except Exception as error:
            errors.append(f"invalid TOML {relative}: {error}")

    for path in sorted((ROOT / "system-backend/kmf/src").glob("*.rs")):
        error = rust_balanced(path)
        if error:
            errors.append(f"{path.relative_to(ROOT)}: {error}")

    http = (ROOT / "system-backend/kmf/src/http.rs").read_text()
    match = re.search(r"<script>(.*)</script>", http, flags=re.S)
    if not match:
        errors.append("WebUI JavaScript not found")
    else:
        with tempfile.NamedTemporaryFile("w", suffix=".js", delete=False) as handle:
            handle.write(match.group(1))
            js_path = Path(handle.name)
        try:
            result = subprocess.run(["node", "--check", str(js_path)], capture_output=True, text=True)
            if result.returncode:
                errors.append(f"WebUI JavaScript invalid: {result.stderr.strip()}")
        except FileNotFoundError:
            pass
        finally:
            js_path.unlink(missing_ok=True)

    for script in sorted((ROOT / "system-backend/kmf/install").glob("*.sh")):
        result = subprocess.run(["bash", "-n", str(script)], capture_output=True, text=True)
        if result.returncode:
            errors.append(f"shell syntax {script.relative_to(ROOT)}: {result.stderr.strip()}")

    result = subprocess.run(
        [sys.executable, "-m", "py_compile", str(ROOT / "system-backend/kmf/tests/lab_edge_unwrap.py")],
        capture_output=True,
        text=True,
    )
    if result.returncode:
        errors.append(f"lab edge helper invalid: {result.stderr.strip()}")

    config = tomllib.loads((ROOT / "system-backend/kmf/config/kmf.example.toml").read_text())
    if config["security"]["mode"] != "open_lab":
        errors.append("KMF example config must remain open_lab for this phase")
    if config["server"]["bind"].split(":")[-1] != "8190":
        errors.append("KMF management port must be 8190")
    if config["security"]["expose_raw_keys"]:
        errors.append("raw key exposure must remain disabled")
    if config["vault"]["provider"] != "lab_file_vault":
        errors.append("unexpected KMF vault provider")

    state_text = (ROOT / "system-backend/kmf/src/state.rs").read_text()
    http_text = (ROOT / "system-backend/kmf/src/http.rs").read_text()
    management_text = state_text[state_text.index("pub fn status"):state_text.index("pub fn metrics")]
    for marker in FORBIDDEN_MANAGEMENT_EXPOSURE:
        if marker in management_text or marker in http_text:
            errors.append(f"management plane contains forbidden secret marker {marker!r}")
    if "transport_secret_hex" not in state_text:
        errors.append("node bootstrap file generation is missing")
    if "write_private_file(&bootstrap_path" not in state_text:
        errors.append("bootstrap must be written as a private server-side file")
    if "SealedBlob" not in state_text or "envelope_context" not in state_text:
        errors.append("node-bound OTAR envelope is missing")
    if "previous_hash" not in state_text or "record_hash" not in state_text:
        errors.append("tamper-evident audit hash chain is missing")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("KMF static package check: OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

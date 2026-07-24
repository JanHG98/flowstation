#!/usr/bin/env python3
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]
REQUIRED = [
    "system-backend/transit/Cargo.toml",
    "system-backend/transit/src/main.rs",
    "system-backend/transit/src/config.rs",
    "system-backend/transit/src/protocol.rs",
    "system-backend/transit/src/state.rs",
    "system-backend/transit/src/transport.rs",
    "system-backend/transit/src/http.rs",
    "system-backend/transit/config/transit.example.toml",
    "system-backend/transit/systemd/netcore-transit.service",
    "system-backend/transit/install/install.sh",
    "system-backend/transit/install/update.sh",
    "system-backend/transit/install/uninstall.sh",
    "system-backend/transit/tests/transit_reference.py",
    "Docs/SWMI_CORE_1_PACKAGE_K_TRANSIT.md",
]
MARKERS = {
    "Cargo.toml": '"system-backend/transit"',
    "Cargo.lock": 'name = "netcore-transit"',
    "system-backend/services.toml": "management_port = 8200",
    "system-backend/transit/src/config.rs": 'pub const TRANSIT_PROTOCOL_VERSION: &str = "netcore-transit-v1"',
    "system-backend/transit/src/state.rs": '"loop_rejected"',
    "system-backend/transit/src/state.rs#2": '"automatic_failover"',
    "system-backend/transit/src/http.rs": "/api/v1/peer/envelopes",
    "system-backend/transit/src/transport.rs": "spawn_transport_worker",
    "system-backend/transit/README.md": "noch kein ETSI ISI",
}


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
    for synthetic, marker in MARKERS.items():
        relative = synthetic.split("#", 1)[0]
        path = ROOT / relative
        if not path.is_file() or marker not in path.read_text(errors="replace"):
            errors.append(f"missing marker {marker!r} in {relative}")

    for relative in [
        "Cargo.toml",
        "system-backend/services.toml",
        "system-backend/transit/Cargo.toml",
        "system-backend/transit/config/transit.example.toml",
    ]:
        try:
            tomllib.loads((ROOT / relative).read_text())
        except Exception as error:
            errors.append(f"invalid TOML {relative}: {error}")

    for path in sorted((ROOT / "system-backend/transit/src").glob("*.rs")):
        error = rust_balanced(path)
        if error:
            errors.append(f"{path.relative_to(ROOT)}: {error}")

    http = (ROOT / "system-backend/transit/src/http.rs").read_text()
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

    for script in sorted((ROOT / "system-backend/transit/install").glob("*.sh")):
        result = subprocess.run(["bash", "-n", str(script)], capture_output=True, text=True)
        if result.returncode:
            errors.append(f"shell syntax {script.relative_to(ROOT)}: {result.stderr.strip()}")

    reference = ROOT / "system-backend/transit/tests/transit_reference.py"
    result = subprocess.run([sys.executable, str(reference)], capture_output=True, text=True)
    if result.returncode:
        errors.append(f"reference test failed: {result.stderr.strip() or result.stdout.strip()}")

    config = tomllib.loads((ROOT / "system-backend/transit/config/transit.example.toml").read_text())
    if config["security"]["mode"] != "open_lab":
        errors.append("Transit example config must remain open_lab for this phase")
    if config["server"]["bind"].split(":")[-1] != "8200":
        errors.append("Transit management port must be 8200")
    if config["security"]["token_auth"] or config["security"]["tls"]:
        errors.append("Token auth and TLS must remain disabled in the open-lab package")
    if config["region"]["protocol_version"] != "netcore-transit-v1":
        errors.append("Unexpected Transit protocol version")
    if config["region"]["operating_mode"] != "shadow":
        errors.append("Example config must default to shadow")

    state = (ROOT / "system-backend/transit/src/state.rs").read_text()
    for marker in ["Path Vector", "max_hops", "dedupe_key", "backup_peers"]:
        if marker not in state and marker not in (ROOT / "system-backend/transit/README.md").read_text():
            errors.append(f"missing transit safety concept {marker}")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Transit static package check: OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

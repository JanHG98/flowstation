#!/usr/bin/env python3
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]
REQUIRED = [
    "system-backend/ip-gateway/Cargo.toml",
    "system-backend/ip-gateway/src/main.rs",
    "system-backend/ip-gateway/src/config.rs",
    "system-backend/ip-gateway/src/state.rs",
    "system-backend/ip-gateway/src/tun.rs",
    "system-backend/ip-gateway/src/kernel.rs",
    "system-backend/ip-gateway/src/packet_core.rs",
    "system-backend/ip-gateway/src/runtime.rs",
    "system-backend/ip-gateway/src/dns.rs",
    "system-backend/ip-gateway/src/dataplane.rs",
    "system-backend/ip-gateway/src/http.rs",
    "system-backend/ip-gateway/config/ip-gateway.example.toml",
    "system-backend/ip-gateway/systemd/netcore-ip-gateway.service",
    "system-backend/ip-gateway/install/install.sh",
    "Docs/SWMI_CORE_1_PACKAGE_H_IP_GATEWAY.md",
    "Docs/SWMI_CORE_1_PACKAGE_H_APPLY.md",
]
MARKERS = {
    "Cargo.toml": '"system-backend/ip-gateway"',
    "system-backend/services.toml": "management_port = 8170",
    "system-backend/ip-gateway/src/tun.rs": "TUNSETIFF",
    "system-backend/ip-gateway/src/kernel.rs": "netcore_ip_gateway_nat",
    "system-backend/ip-gateway/src/packet_core.rs": "/api/v1/npdu-outbox",
    "system-backend/ip-gateway/src/dns.rs": "build_a_response",
    "system-backend/ip-gateway/src/dataplane.rs": "text/vnd.wap.wml",
    "system-backend/ip-gateway/src/state.rs": "101u32.to_le_bytes",
    "system-backend/ip-gateway/src/http.rs": "/api/v1/kernel/reconcile",
    "system-backend/packet-core/src/state.rs": ".database.npdu_outbox.iter().take(limit)",
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
            # Rust lifetime, or character literal. A valid char literal closes quickly.
            end = i + 1
            escaped = False
            while end < min(len(text), i + 12):
                if not escaped and text[end] == "'":
                    i = end + 1
                    break
                escaped = not escaped and text[end] == "\\"
                if text[end] != "\\":
                    escaped = False
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
        "system-backend/ip-gateway/config/ip-gateway.example.toml",
        "system-backend/services.toml",
        "Cargo.toml",
    ]:
        try:
            tomllib.loads((ROOT / relative).read_text())
        except Exception as error:
            errors.append(f"invalid TOML {relative}: {error}")
    for path in sorted((ROOT / "system-backend/ip-gateway/src").glob("*.rs")):
        error = rust_balanced(path)
        if error:
            errors.append(f"{path.relative_to(ROOT)}: {error}")
    http = (ROOT / "system-backend/ip-gateway/src/http.rs").read_text()
    match = re.search(r'<script>(.*)</script>', http, flags=re.S)
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
    for script in sorted((ROOT / "system-backend/ip-gateway/install").glob("*.sh")):
        result = subprocess.run(["bash", "-n", str(script)], capture_output=True, text=True)
        if result.returncode:
            errors.append(f"shell syntax {script.relative_to(ROOT)}: {result.stderr.strip()}")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("IP Gateway static package check: OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
from pathlib import Path
import json
import re
import subprocess
import sys
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]
REQUIRED = [
    "system-backend/security-core/Cargo.toml",
    "system-backend/security-core/src/main.rs",
    "system-backend/security-core/src/config.rs",
    "system-backend/security-core/src/crypto.rs",
    "system-backend/security-core/src/protocol.rs",
    "system-backend/security-core/src/gateway.rs",
    "system-backend/security-core/src/state.rs",
    "system-backend/security-core/src/http.rs",
    "system-backend/security-core/config/security-core.example.toml",
    "system-backend/security-core/systemd/netcore-security-core.service",
    "system-backend/security-core/install/install.sh",
    "system-backend/security-core/install/update.sh",
    "system-backend/security-core/install/uninstall.sh",
    "system-backend/security-core/tests/lab_response.py",
    "Docs/SWMI_CORE_1_PACKAGE_I_SECURITY_CORE.md",
]
MARKERS = {
    "Cargo.toml": '"system-backend/security-core"',
    "Cargo.lock": 'name = "netcore-security-core"',
    "system-backend/services.toml": "management_port = 8180",
    "system-backend/security-core/src/state.rs": "raw_secrets_exposed_by_management_api: false",
    "system-backend/security-core/src/http.rs": "/api/v1/edge/actions/claim",
    "system-backend/security-core/src/crypto.rs": "netcore-security-core/lab-dck/v1",
    "system-backend/security-core/systemd/netcore-security-core.service": "UMask=0077",
    "system-backend/security-core/README.md": "lab_hmac_sha256",
}
FORBIDDEN_NORMAL_API_MARKERS = [
    '"raw_seed": state.',
    '"dck_hex"',
    '"challenge_hex"',
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
            # Lifetimes and character literals are irrelevant for delimiter balance.
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
        elif char in ")]} ".replace(" ", ""):
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
        "system-backend/security-core/Cargo.toml",
        "system-backend/security-core/config/security-core.example.toml",
    ]:
        try:
            tomllib.loads((ROOT / relative).read_text())
        except Exception as error:
            errors.append(f"invalid TOML {relative}: {error}")

    for path in sorted((ROOT / "system-backend/security-core/src").glob("*.rs")):
        error = rust_balanced(path)
        if error:
            errors.append(f"{path.relative_to(ROOT)}: {error}")

    http = (ROOT / "system-backend/security-core/src/http.rs").read_text()
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

    for script in sorted((ROOT / "system-backend/security-core/install").glob("*.sh")):
        result = subprocess.run(["bash", "-n", str(script)], capture_output=True, text=True)
        if result.returncode:
            errors.append(f"shell syntax {script.relative_to(ROOT)}: {result.stderr.strip()}")

    result = subprocess.run(
        [sys.executable, "-m", "py_compile", str(ROOT / "system-backend/security-core/tests/lab_response.py")],
        capture_output=True,
        text=True,
    )
    if result.returncode:
        errors.append(f"lab helper invalid: {result.stderr.strip()}")

    config = tomllib.loads((ROOT / "system-backend/security-core/config/security-core.example.toml").read_text())
    if config["security"]["mode"] != "open_lab":
        errors.append("security core example config must remain open_lab for this phase")
    if config["server"]["bind"].split(":")[-1] != "8180":
        errors.append("security core management port must be 8180")

    # The management model may contain secret field names only inside the dedicated edge action payload.
    state_text = (ROOT / "system-backend/security-core/src/state.rs").read_text()
    redacted_block = state_text[state_text.index("pub fn redacted_config"):state_text.index("pub fn upsert_profile")]
    if "challenge_hex" in redacted_block or "dck_hex" in redacted_block:
        errors.append("redacted management config exposes secret-bearing fields")
    if '"raw_seed": "never exposed"' not in redacted_block:
        errors.append("redacted management config must explicitly suppress the raw seed")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Security Core static package check: OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

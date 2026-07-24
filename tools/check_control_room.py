#!/usr/bin/env python3
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]
REQUIRED = [
    "bins/netcore-control-room/src/operations.rs",
    "bins/netcore-control-room/src/webui.rs",
    "system-backend/control-room/Readme.md",
    "system-backend/control-room/config/control-room.example.toml",
    "system-backend/control-room/systemd/netcore-control-room.service",
    "system-backend/control-room/install/install.sh",
    "system-backend/control-room/install/update.sh",
    "system-backend/control-room/install/uninstall.sh",
    "system-backend/control-room/tests/control_room_reference.py",
    "Docs/SWMI_CORE_1_PACKAGE_L_CONTROL_ROOM.md",
]
MARKERS = {
    "bins/netcore-control-room/src/main.rs": "operations.start_poller()",
    "bins/netcore-control-room/src/operations.rs": "auto_service_incidents",
    "bins/netcore-control-room/src/operations.rs#2": "authoritative_data_owner",
    "bins/netcore-control-room/src/operations.rs#3": "federated_domain_overview",
    "bins/netcore-control-room/src/http.rs": "/api/v1/control-room/overview",
    "bins/netcore-control-room/src/http.rs#2": "/api/v1/incidents",
    "bins/netcore-control-room/src/webui.rs": "OPEN LAB",
    "system-backend/services.toml": "management_port = 9010",
    "system-backend/control-room/Readme.md": "nicht** Eigentümer",
}


def rust_balanced(path: Path) -> str | None:
    text = path.read_text(errors="replace")
    stack: list[tuple[str, int]] = []
    pairs = {')': '(', ']': '[', '}': '{'}
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
                hashes = match.group(1); start_len = 2 + len(hashes); end_marker = '"' + hashes
                end = text.find(end_marker, i + start_len)
                if end < 0: return f"unterminated raw string at line {line}"
                line += text[i:end + len(end_marker)].count("\n"); i = end + len(end_marker); continue
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
            while end < min(len(text), i + 12):
                if text[end] == "'" and text[end - 1] != "\\": i = end + 1; break
                end += 1
            else: i += 1
            continue
        char = text[i]
        if char in "([{": stack.append((char, line))
        elif char in ")]}":
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

    for relative in ["Cargo.toml", "system-backend/services.toml", "system-backend/control-room/config/control-room.example.toml"]:
        try: tomllib.loads((ROOT / relative).read_text())
        except Exception as error: errors.append(f"invalid TOML {relative}: {error}")

    config = tomllib.loads((ROOT / "system-backend/control-room/config/control-room.example.toml").read_text())
    if config["auth"]["enabled"]:
        errors.append("Control Room example must remain no-auth in this phase")
    if config["server"]["bind"].split(":")[-1] != "9010":
        errors.append("Control Room port must be 9010")
    if len(config.get("services", [])) < 14:
        errors.append("Control Room example must list all implemented backend services")

    for relative in [
        "bins/netcore-control-room/src/main.rs",
        "bins/netcore-control-room/src/config.rs",
        "bins/netcore-control-room/src/operations.rs",
        "bins/netcore-control-room/src/http.rs",
        "bins/netcore-control-room/src/server.rs",
        "bins/netcore-control-room/src/webui.rs",
    ]:
        error = rust_balanced(ROOT / relative)
        if error: errors.append(f"{relative}: {error}")

    webui = (ROOT / "bins/netcore-control-room/src/webui.rs").read_text()
    match = re.search(r"<script>(.*)</script>", webui, flags=re.S)
    if not match:
        errors.append("Control Room WebUI JavaScript not found")
    else:
        rendered = match.group(1).replace("{{", "{").replace("}}", "}")
        with tempfile.NamedTemporaryFile("w", suffix=".js", delete=False) as handle:
            handle.write(rendered); js_path = Path(handle.name)
        try:
            result = subprocess.run(["node", "--check", str(js_path)], capture_output=True, text=True)
            if result.returncode: errors.append(f"WebUI JavaScript invalid: {result.stderr.strip()}")
        except FileNotFoundError: pass
        finally: js_path.unlink(missing_ok=True)

    for script in sorted((ROOT / "system-backend/control-room/install").glob("*.sh")):
        result = subprocess.run(["bash", "-n", str(script)], capture_output=True, text=True)
        if result.returncode: errors.append(f"shell syntax {script.relative_to(ROOT)}: {result.stderr.strip()}")

    reference = subprocess.run([sys.executable, str(ROOT / "system-backend/control-room/tests/control_room_reference.py")], capture_output=True, text=True)
    if reference.returncode: errors.append(f"reference test failed: {reference.stderr.strip() or reference.stdout.strip()}")

    if errors:
        print("\n".join(errors), file=sys.stderr); return 1
    print("Control Room static package check: OK"); return 0


if __name__ == "__main__":
    raise SystemExit(main())

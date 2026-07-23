#!/usr/bin/env python3
from pathlib import Path
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]

required = {
    "workspace member": (ROOT / "Cargo.toml", '"system-backend/node-gateway"'),
    "gateway package": (ROOT / "system-backend/node-gateway/Cargo.toml", 'name = "netcore-node-gateway"'),
    "open lab constant": (ROOT / "system-backend/node-gateway/src/config.rs", 'OPEN_LAB_MODE: &str = "open_lab"'),
    "no fake secure mode": (ROOT / "system-backend/node-gateway/src/config.rs", "intentionally implements only open_lab"),
    "node websocket": (ROOT / "system-backend/node-gateway/src/ws.rs", "handle_node_websocket"),
    "backend websocket": (ROOT / "system-backend/node-gateway/src/ws.rs", "handle_backend_websocket"),
    "compatibility marker": (ROOT / "system-backend/node-gateway/src/ws.rs", '"x-netcore-control-room"'),
    "gateway marker": (ROOT / "system-backend/node-gateway/src/ws.rs", '"x-netcore-node-gateway"'),
    "webui warning": (ROOT / "system-backend/node-gateway/src/http.rs", "OFFENER TESTMODUS"),
    "health live": (ROOT / "system-backend/node-gateway/src/http.rs", '"/health/live"'),
    "metrics": (ROOT / "system-backend/node-gateway/src/http.rs", '"/metrics"'),
    "systemd": (ROOT / "system-backend/node-gateway/systemd/netcore-node-gateway.service", "OPEN LAB MODE"),
    "install script": (ROOT / "system-backend/node-gateway/install/install.sh", "cargo build --release -p netcore-node-gateway"),
    "package docs": (ROOT / "Docs/SWMI_MOBILITY_1_PACKAGE_D_NODE_GATEWAY.md", "ohne Tokens"),
}

errors = []
for label, (path, marker) in required.items():
    if not path.is_file():
        errors.append(f"{label}: missing {path.relative_to(ROOT)}")
        continue
    if marker not in path.read_text(encoding="utf-8"):
        errors.append(f"{label}: marker not found: {marker!r}")

services_path = ROOT / "system-backend/services.toml"
with services_path.open("rb") as handle:
    manifest = tomllib.load(handle)
node = next((service for service in manifest.get("services", []) if service.get("name") == "node-gateway"), None)
if not node:
    errors.append("services.toml: node-gateway missing")
else:
    expected = {
        "webui": True,
        "scheme": "http",
        "management_port": 8080,
        "security_mode": "open_lab",
        "token_auth": False,
        "tls": False,
    }
    for key, value in expected.items():
        if node.get(key) != value:
            errors.append(f"services.toml: node-gateway {key}={node.get(key)!r}, expected {value!r}")

for forbidden in ("node_token", "api_token", "bearer_token", "bootstrap_password"):
    for path in (ROOT / "system-backend/node-gateway").rglob("*"):
        if path.is_file() and path.suffix in {".rs", ".toml"}:
            if forbidden in path.read_text(encoding="utf-8").lower():
                errors.append(f"forbidden token/password field {forbidden!r} in {path.relative_to(ROOT)}")

if errors:
    print("Node Gateway checks failed:")
    for error in errors:
        print(f"  - {error}")
    sys.exit(1)

print("SWMI Mobility 1 Package D Node Gateway checks passed.")
print("  deployable LXC service: present")
print("  integrated WebUI and REST API: present")
print("  TBS and backend WebSockets: present")
print("  explicit open_lab mode: present")
print("  token/password fields: absent")

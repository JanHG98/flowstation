#!/usr/bin/env python3
from pathlib import Path
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]

required = {
    "workspace member": (ROOT / "Cargo.toml", '"system-backend/mobility-core"'),
    "package": (ROOT / "system-backend/mobility-core/Cargo.toml", 'name = "netcore-mobility-core"'),
    "open lab constant": (ROOT / "system-backend/mobility-core/src/config.rs", 'OPEN_LAB_MODE: &str = "open_lab"'),
    "gateway client": (ROOT / "system-backend/mobility-core/src/gateway.rs", "BACKEND_PROTOCOL_VERSION"),
    "central subscriber state": (ROOT / "system-backend/mobility-core/src/state.rs", "SubscriberRecord"),
    "three phase transfer": (ROOT / "system-backend/mobility-core/src/state.rs", "MobilityRemoveContext"),
    "context export command": (ROOT / "crates/tetra-entities/src/net_control/commands.rs", "MobilityExportContext"),
    "context import command": (ROOT / "crates/tetra-entities/src/net_control/commands.rs", "MobilityImportContext"),
    "context cleanup command": (ROOT / "crates/tetra-entities/src/net_control/commands.rs", "MobilityRemoveContext"),
    "MM command handling": (ROOT / "crates/tetra-entities/src/mm/mm_bs.rs", "MobilityContextExported"),
    "structured gateway action": (ROOT / "system-backend/node-gateway/src/state.rs", "command_id: Option<String>"),
    "webui warning": (ROOT / "system-backend/mobility-core/src/http.rs", "OFFENER TESTMODUS"),
    "systemd": (ROOT / "system-backend/mobility-core/systemd/netcore-mobility-core.service", "OPEN LAB MODE"),
    "install script": (ROOT / "system-backend/mobility-core/install/install.sh", "cargo build --release -p netcore-mobility-core"),
    "package docs": (ROOT / "Docs/SWMI_MOBILITY_1_PACKAGE_E_MOBILITY_CORE.md", "keine Tokens"),
}

errors = []
for label, (path, marker) in required.items():
    if not path.is_file():
        errors.append(f"{label}: missing {path.relative_to(ROOT)}")
        continue
    if marker not in path.read_text(encoding="utf-8"):
        errors.append(f"{label}: marker not found: {marker!r}")

with (ROOT / "system-backend/services.toml").open("rb") as handle:
    manifest = tomllib.load(handle)
service = next(
    (entry for entry in manifest.get("services", []) if entry.get("name") == "mobility-core"),
    None,
)
expected = {
    "webui": True,
    "scheme": "http",
    "management_port": 8090,
    "security_mode": "open_lab",
    "token_auth": False,
    "tls": False,
}
if service is None:
    errors.append("services.toml: mobility-core missing")
else:
    for key, value in expected.items():
        if service.get(key) != value:
            errors.append(
                f"services.toml: mobility-core {key}={service.get(key)!r}, expected {value!r}"
            )

for forbidden in ("node_token", "api_token", "bearer_token", "bootstrap_password"):
    for path in (ROOT / "system-backend/mobility-core").rglob("*"):
        if path.is_file() and path.suffix in {".rs", ".toml"}:
            if forbidden in path.read_text(encoding="utf-8").lower():
                errors.append(
                    f"forbidden token/password field {forbidden!r} in {path.relative_to(ROOT)}"
                )

if errors:
    print("Mobility Core checks failed:")
    for error in errors:
        print(f"  - {error}")
    sys.exit(1)

print("SWMI Mobility 1 Package E Mobility Core checks passed.")
print("  deployable LXC service: present")
print("  open-lab WebUI and REST API: present")
print("  central subscriber state: present")
print("  three-step MM context transfer: present")
print("  token/password fields: absent")

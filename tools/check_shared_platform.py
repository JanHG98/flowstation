#!/usr/bin/env python3
from __future__ import annotations

import json
import subprocess
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REQUIRED = [
    "system-backend/shared/contracts/Cargo.toml",
    "system-backend/shared/contracts/src/lib.rs",
    "system-backend/shared/contracts/src/address.rs",
    "system-backend/shared/contracts/src/envelope.rs",
    "system-backend/shared/service-common/Cargo.toml",
    "system-backend/shared/database-common/Cargo.toml",
    "system-backend/shared/telemetry-common/Cargo.toml",
    "system-backend/shared/web-ui/assets/netcore.css",
    "system-backend/shared/web-ui/assets/netcore.js",
    "deploy/open-lab/inventory.example.toml",
    "deploy/open-lab/netcore-deploy.py",
    "deploy/open-lab/generated/service-catalog.json",
    "tests/integration/open_lab_contract_test.py",
    "Docs/SWMI_CORE_1_PACKAGE_P_SHARED_PLATFORM.md",
    "Docs/OPEN_LAB_LXC_DEPLOYMENT.md",
    ".github/workflows/swmi-core-shared-platform.yml",
]


def main() -> int:
    errors: list[str] = []
    for relative in REQUIRED:
        if not (ROOT / relative).is_file():
            errors.append(f"missing {relative}")

    cargo = (ROOT / "Cargo.toml").read_text()
    lock = (ROOT / "Cargo.lock").read_text()
    for crate in ["netcore-contracts", "netcore-service-common", "netcore-database-common", "netcore-telemetry-common"]:
        if crate not in cargo:
            errors.append(f"workspace missing {crate}")
        if f'name = "{crate}"' not in lock:
            errors.append(f"Cargo.lock missing {crate}")

    with (ROOT / "system-backend/services.toml").open("rb") as handle:
        registry = tomllib.load(handle)
    services = [item for item in registry["services"] if item["name"] != "shared"]
    for service in services:
        if service.get("security_mode") != "open_lab" or service.get("token_auth") or service.get("tls"):
            errors.append(f"{service['name']}: current package must remain explicit open_lab without token/TLS")

    for schema in (ROOT / "system-backend/shared/contracts/schemas").glob("*.json"):
        try:
            json.loads(schema.read_text())
        except Exception as error:
            errors.append(f"invalid schema {schema.name}: {error}")

    commands = [
        [sys.executable, "deploy/open-lab/netcore-deploy.py", "validate"],
        [sys.executable, "deploy/open-lab/netcore-deploy.py", "render"],
        [sys.executable, "tests/integration/open_lab_contract_test.py"],
        ["node", "--check", "system-backend/shared/web-ui/assets/netcore.js"],
    ]
    for command in commands:
        try:
            result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
        except FileNotFoundError:
            if command[0] == "node":
                continue
            raise
        if result.returncode:
            errors.append(f"command failed {' '.join(command)}: {result.stderr.strip() or result.stdout.strip()}")

    for script in [ROOT / "deploy/open-lab/netcore-deploy.py"]:
        if not script.stat().st_mode & 0o111:
            errors.append(f"not executable: {script.relative_to(ROOT)}")

    pdfs = list(ROOT.rglob("*.pdf"))
    if pdfs:
        errors.append(f"repository package contains PDF files: {len(pdfs)}")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print(f"Shared platform static package check: OK ({len(services)} deployable services)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

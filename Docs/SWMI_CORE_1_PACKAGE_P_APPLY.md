# Package P anwenden

Das Paket ist bereits vollständig in den Repository-Stand integriert.

## Prüfen

```bash
python3 tools/check_shared_platform.py
python3 deploy/open-lab/netcore-deploy.py validate
python3 deploy/open-lab/netcore-deploy.py render
python3 tests/integration/open_lab_contract_test.py
```

Mit installiertem Rust-Toolchain zusätzlich:

```bash
cargo test --locked --package netcore-contracts \
  --package netcore-service-common \
  --package netcore-database-common \
  --package netcore-telemetry-common
cargo fmt --all --check
cargo clippy --locked --package netcore-contracts \
  --package netcore-service-common \
  --package netcore-database-common \
  --package netcore-telemetry-common --all-targets -- -D warnings
```

## LXC-Konfiguration rendern

```bash
cp deploy/open-lab/inventory.example.toml deploy/open-lab/inventory.toml
$EDITOR deploy/open-lab/inventory.toml
python3 deploy/open-lab/netcore-deploy.py --inventory deploy/open-lab/inventory.toml plan
python3 deploy/open-lab/netcore-deploy.py --inventory deploy/open-lab/inventory.toml render
```

Vor `apply` müssen die gerenderten Dateien unter `deploy/open-lab/generated/configs/` geprüft werden.

#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../../.."
sudo systemctl stop netcore-group-core.service 2>/dev/null || true
sudo rm -f /usr/local/bin/netcore-group-core
rm -rf target/release/netcore-group-core target/release/deps/netcore_group_core-* target/release/.fingerprint/netcore-group-core-*
cargo clean -p netcore-group-core 2>/dev/null || true
cargo build --release -p netcore-group-core
sudo install -o root -g root -m 0755 target/release/netcore-group-core /usr/local/bin/netcore-group-core
sudo install -d -o root -g root -m 0755 /etc/netcore
if [[ ! -f /etc/netcore/group-core.toml ]]; then sudo install -o root -g root -m 0644 system-backend/group-core/config/group-core.example.toml /etc/netcore/group-core.toml; fi
if ! id netcore >/dev/null 2>&1; then sudo useradd --system --home /var/lib/netcore-group-core --shell /usr/sbin/nologin netcore; fi
sudo install -d -o netcore -g netcore -m 0750 /var/lib/netcore-group-core
sudo install -o root -g root -m 0644 system-backend/group-core/systemd/netcore-group-core.service /etc/systemd/system/netcore-group-core.service
sudo systemctl daemon-reload
sudo systemctl enable --now netcore-group-core.service

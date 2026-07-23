#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../../.."
sudo systemctl stop netcore-call-control.service 2>/dev/null || true
sudo rm -f /usr/local/bin/netcore-call-control
rm -rf target/release/netcore-call-control target/release/deps/netcore_call_control-* target/release/.fingerprint/netcore-call-control-*
cargo clean -p netcore-call-control 2>/dev/null || true
cargo build --release -p netcore-call-control
sudo install -o root -g root -m 0755 target/release/netcore-call-control /usr/local/bin/netcore-call-control
sudo install -d -o root -g root -m 0755 /etc/netcore
if [[ ! -f /etc/netcore/call-control.toml ]]; then
  sudo install -o root -g root -m 0644 system-backend/call-control/config/call-control.example.toml /etc/netcore/call-control.toml
fi
if ! id netcore >/dev/null 2>&1; then
  sudo useradd --system --home /var/lib/netcore-call-control --shell /usr/sbin/nologin netcore
fi
sudo install -d -o netcore -g netcore -m 0750 /var/lib/netcore-call-control
sudo install -o root -g root -m 0644 system-backend/call-control/systemd/netcore-call-control.service /etc/systemd/system/netcore-call-control.service
sudo systemctl daemon-reload
sudo systemctl enable --now netcore-call-control.service

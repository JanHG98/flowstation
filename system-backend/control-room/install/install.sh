#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
if ! id -u netcore >/dev/null 2>&1; then
  useradd --system --home /var/lib/netcore-control-room --shell /usr/sbin/nologin netcore
fi
install -d -m 0755 /opt/netcore/control-room /etc/netcore-control-room
install -d -o netcore -g netcore -m 0750 /var/lib/netcore-control-room
cargo build --locked --release --package netcore-control-room --manifest-path "$ROOT/Cargo.toml"
install -m 0755 "$ROOT/target/release/netcore-control-room" /usr/local/bin/netcore-control-room
install -m 0644 "$ROOT/system-backend/control-room/systemd/netcore-control-room.service" /etc/systemd/system/netcore-control-room.service
if [[ ! -f /etc/netcore-control-room/control-room.toml ]]; then
  install -m 0640 "$ROOT/system-backend/control-room/config/control-room.example.toml" /etc/netcore-control-room/control-room.toml
fi
systemctl daemon-reload
systemctl enable --now netcore-control-room.service

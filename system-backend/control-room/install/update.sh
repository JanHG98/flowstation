#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cargo build --locked --release --package netcore-control-room --manifest-path "$ROOT/Cargo.toml"
systemctl stop netcore-control-room.service
install -m 0755 "$ROOT/target/release/netcore-control-room" /usr/local/bin/netcore-control-room
install -m 0644 "$ROOT/system-backend/control-room/systemd/netcore-control-room.service" /etc/systemd/system/netcore-control-room.service
systemctl daemon-reload
systemctl start netcore-control-room.service

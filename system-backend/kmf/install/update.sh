#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
if [[ ${EUID:-$(id -u)} -ne 0 ]]; then
  echo "Bitte als root ausführen." >&2
  exit 1
fi
cargo build --release --package netcore-kmf
systemctl stop netcore-kmf.service
install -m 0755 "$ROOT/target/release/netcore-kmf" /usr/local/bin/netcore-kmf
install -m 0644 "$ROOT/system-backend/kmf/systemd/netcore-kmf.service" /etc/systemd/system/netcore-kmf.service
systemctl daemon-reload
systemctl start netcore-kmf.service
systemctl --no-pager --full status netcore-kmf.service

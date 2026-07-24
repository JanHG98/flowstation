#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
PREFIX="${PREFIX:-/opt/netcore-transit}"

if [[ ${EUID} -ne 0 ]]; then
  echo "update.sh must run as root" >&2
  exit 1
fi

cargo build --locked --release --package netcore-transit --manifest-path "${ROOT}/Cargo.toml"
install -o root -g root -m 0755 "${ROOT}/target/release/netcore-transit" "${PREFIX}/bin/netcore-transit"
install -o root -g root -m 0644 "${ROOT}/system-backend/transit/README.md" "${PREFIX}/README.md"
install -o root -g root -m 0644 "${ROOT}/system-backend/transit/systemd/netcore-transit.service" /etc/systemd/system/netcore-transit.service
systemctl daemon-reload
systemctl restart netcore-transit.service

#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
PREFIX="${PREFIX:-/opt/netcore-transit}"
CONFIG="${CONFIG:-/etc/netcore/transit.toml}"
SERVICE="${SERVICE:-/etc/systemd/system/netcore-transit.service}"

if [[ ${EUID} -ne 0 ]]; then
  echo "install.sh must run as root" >&2
  exit 1
fi

id -u netcore-transit >/dev/null 2>&1 || useradd --system --home /var/lib/netcore-transit --shell /usr/sbin/nologin netcore-transit
install -d -o netcore-transit -g netcore-transit -m 0750 /var/lib/netcore-transit
install -d -o root -g root -m 0755 "${PREFIX}/bin"

cargo build --locked --release --package netcore-transit --manifest-path "${ROOT}/Cargo.toml"
install -o root -g root -m 0755 "${ROOT}/target/release/netcore-transit" "${PREFIX}/bin/netcore-transit"
install -o root -g root -m 0644 "${ROOT}/system-backend/transit/README.md" "${PREFIX}/README.md"
install -d -o root -g netcore-transit -m 0750 "$(dirname "${CONFIG}")"
if [[ ! -e "${CONFIG}" ]]; then
  install -o root -g netcore-transit -m 0640 "${ROOT}/system-backend/transit/config/transit.example.toml" "${CONFIG}"
fi
install -o root -g root -m 0644 "${ROOT}/system-backend/transit/systemd/netcore-transit.service" "${SERVICE}"
systemctl daemon-reload
systemctl enable --now netcore-transit.service

echo "NetCore Transit installed. WebUI: http://<lxc>:8200"
echo "OPEN LAB: place this LXC only on an isolated test-management network."

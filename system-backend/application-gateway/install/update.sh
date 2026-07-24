#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
PREFIX="${PREFIX:-/opt/netcore-application-gateway}"
[[ ${EUID} -eq 0 ]] || { echo "update.sh must run as root" >&2; exit 1; }
cargo build --locked --release --package netcore-application-gateway --manifest-path "${ROOT}/Cargo.toml"
install -d -o root -g root -m 0755 "${PREFIX}/bin"
install -o root -g root -m 0755 "${ROOT}/target/release/netcore-application-gateway" "${PREFIX}/bin/netcore-application-gateway"
install -o root -g root -m 0644 "${ROOT}/system-backend/application-gateway/README.md" "${PREFIX}/README.md"
install -o root -g root -m 0644 "${ROOT}/system-backend/application-gateway/systemd/netcore-application-gateway.service" /etc/systemd/system/netcore-application-gateway.service
systemctl daemon-reload
systemctl restart netcore-application-gateway.service
systemctl --no-pager --full status netcore-application-gateway.service

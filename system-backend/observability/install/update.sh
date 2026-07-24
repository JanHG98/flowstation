#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
PREFIX="${PREFIX:-/opt/netcore-observability}"
[[ ${EUID} -eq 0 ]] || { echo "update.sh must run as root" >&2; exit 1; }
cargo build --locked --release --package netcore-observability --manifest-path "${ROOT}/Cargo.toml"
install -o root -g root -m 0755 "${ROOT}/target/release/netcore-observability" "${PREFIX}/bin/netcore-observability"
cp -a "${ROOT}/system-backend/observability/stack/." "${PREFIX}/stack/"
systemctl restart netcore-observability.service

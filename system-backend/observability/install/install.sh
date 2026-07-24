#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
PREFIX="${PREFIX:-/opt/netcore-observability}"
CONFIG="${CONFIG:-/etc/netcore/observability.toml}"
SERVICE="${SERVICE:-/etc/systemd/system/netcore-observability.service}"
[[ ${EUID} -eq 0 ]] || { echo "install.sh must run as root" >&2; exit 1; }
id -u netcore-observability >/dev/null 2>&1 || useradd --system --home /var/lib/netcore-observability --shell /usr/sbin/nologin netcore-observability
install -d -o netcore-observability -g netcore-observability -m 0750 /var/lib/netcore-observability /var/lib/netcore-observability/diagnostics
install -d -o root -g root -m 0755 "${PREFIX}/bin" "${PREFIX}/stack" "$(dirname "${CONFIG}")"
cargo build --locked --release --package netcore-observability --manifest-path "${ROOT}/Cargo.toml"
install -o root -g root -m 0755 "${ROOT}/target/release/netcore-observability" "${PREFIX}/bin/netcore-observability"
install -o root -g root -m 0644 "${ROOT}/system-backend/observability/README.md" "${PREFIX}/README.md"
cp -a "${ROOT}/system-backend/observability/stack/." "${PREFIX}/stack/"
if [[ ! -e "${CONFIG}" ]]; then install -o root -g netcore-observability -m 0640 "${ROOT}/system-backend/observability/config/observability.example.toml" "${CONFIG}"; fi
install -o root -g root -m 0644 "${ROOT}/system-backend/observability/systemd/netcore-observability.service" "${SERVICE}"
systemctl daemon-reload
systemctl enable --now netcore-observability.service
echo "NetCore Observability installed. WebUI: http://<lxc>:8210"
echo "OPEN LAB: no login, no tokens and no TLS. Isolated management network only."

#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
PREFIX="${PREFIX:-/opt/netcore-application-gateway}"
CONFIG="${CONFIG:-/etc/netcore/application-gateway.toml}"
SERVICE="${SERVICE:-/etc/systemd/system/netcore-application-gateway.service}"
[[ ${EUID} -eq 0 ]] || { echo "install.sh must run as root" >&2; exit 1; }
id -u netcore-app-gateway >/dev/null 2>&1 || useradd --system --home /var/lib/netcore-application-gateway --shell /usr/sbin/nologin netcore-app-gateway
install -d -o netcore-app-gateway -g netcore-app-gateway -m 0750 /var/lib/netcore-application-gateway /var/lib/netcore-application-gateway/spool /var/lib/netcore-application-gateway/backups
install -d -o root -g root -m 0755 "${PREFIX}/bin" "$(dirname "${CONFIG}")"
cargo build --locked --release --package netcore-application-gateway --manifest-path "${ROOT}/Cargo.toml"
install -o root -g root -m 0755 "${ROOT}/target/release/netcore-application-gateway" "${PREFIX}/bin/netcore-application-gateway"
install -o root -g root -m 0644 "${ROOT}/system-backend/application-gateway/README.md" "${PREFIX}/README.md"
if [[ ! -e "${CONFIG}" ]]; then
  install -o root -g netcore-app-gateway -m 0640 "${ROOT}/system-backend/application-gateway/config/application-gateway.example.toml" "${CONFIG}"
fi
install -o root -g root -m 0644 "${ROOT}/system-backend/application-gateway/systemd/netcore-application-gateway.service" "${SERVICE}"
systemctl daemon-reload
systemctl enable --now netcore-application-gateway.service
echo "NetCore Application Gateway installed. WebUI: http://<lxc>:8220"
echo "OPEN LAB: no login, no management tokens and no TLS. Isolated management network only."

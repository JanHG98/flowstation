#!/usr/bin/env bash
set -euo pipefail

if [[ ${EUID} -ne 0 ]]; then
  echo "Bitte als root ausführen." >&2
  exit 1
fi

REPO_ROOT=${REPO_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)}
cd "$REPO_ROOT"

systemctl stop netcore-node-gateway.service 2>/dev/null || true
rm -f /usr/local/bin/netcore-node-gateway
rm -rf target/release/netcore-node-gateway target/release/deps/netcore_node_gateway-*
cargo build --release -p netcore-node-gateway

getent group netcore >/dev/null || groupadd --system netcore
id netcore >/dev/null 2>&1 || useradd --system --gid netcore --home-dir /var/lib/netcore-node-gateway --shell /usr/sbin/nologin netcore
install -d -o netcore -g netcore /var/lib/netcore-node-gateway
install -d -m 0755 /etc/netcore
install -m 0755 target/release/netcore-node-gateway /usr/local/bin/netcore-node-gateway
if [[ ! -f /etc/netcore/node-gateway.toml ]]; then
  install -m 0644 system-backend/node-gateway/config/node-gateway.example.toml /etc/netcore/node-gateway.toml
fi
install -m 0644 system-backend/node-gateway/systemd/netcore-node-gateway.service /etc/systemd/system/netcore-node-gateway.service
systemctl daemon-reload
systemctl enable --now netcore-node-gateway.service
systemctl --no-pager --full status netcore-node-gateway.service

echo "WebUI: http://<LXC-IP>:8080/"
echo "WARNUNG: offener Testmodus ohne Authentifizierung oder Tokens."

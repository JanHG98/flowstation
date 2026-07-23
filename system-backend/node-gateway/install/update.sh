#!/usr/bin/env bash
set -euo pipefail

if [[ ${EUID} -ne 0 ]]; then
  echo "Bitte als root ausführen." >&2
  exit 1
fi

REPO_ROOT=${REPO_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)}
cd "$REPO_ROOT"

systemctl stop netcore-node-gateway.service
rm -f /usr/local/bin/netcore-node-gateway
rm -rf target/release/netcore-node-gateway target/release/deps/netcore_node_gateway-*
cargo clean -p netcore-node-gateway
cargo build --release -p netcore-node-gateway
install -m 0755 target/release/netcore-node-gateway /usr/local/bin/netcore-node-gateway
install -m 0644 system-backend/node-gateway/systemd/netcore-node-gateway.service /etc/systemd/system/netcore-node-gateway.service
systemctl daemon-reload
systemctl start netcore-node-gateway.service
systemctl --no-pager --full status netcore-node-gateway.service

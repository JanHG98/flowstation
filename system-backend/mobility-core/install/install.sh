#!/usr/bin/env bash
set -euo pipefail

if [[ ${EUID} -ne 0 ]]; then
  echo "Bitte als root ausführen." >&2
  exit 1
fi

REPO_ROOT=${REPO_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)}
cd "$REPO_ROOT"

systemctl stop netcore-mobility-core.service 2>/dev/null || true
rm -f /usr/local/bin/netcore-mobility-core
rm -rf target/release/netcore-mobility-core target/release/deps/netcore_mobility_core-*
cargo build --release -p netcore-mobility-core

getent group netcore >/dev/null || groupadd --system netcore
id netcore >/dev/null 2>&1 || useradd --system --gid netcore --home-dir /var/lib/netcore-mobility-core --shell /usr/sbin/nologin netcore
install -d -o netcore -g netcore /var/lib/netcore-mobility-core
install -d -m 0755 /etc/netcore
install -m 0755 target/release/netcore-mobility-core /usr/local/bin/netcore-mobility-core
if [[ ! -f /etc/netcore/mobility-core.toml ]]; then
  install -m 0644 system-backend/mobility-core/config/mobility-core.example.toml /etc/netcore/mobility-core.toml
fi
install -m 0644 system-backend/mobility-core/systemd/netcore-mobility-core.service /etc/systemd/system/netcore-mobility-core.service
systemctl daemon-reload
systemctl enable --now netcore-mobility-core.service
systemctl --no-pager --full status netcore-mobility-core.service

echo "WebUI: http://<LXC-IP>:8090/"
echo "WARNUNG: offener Testmodus ohne Authentifizierung, Tokens oder TLS."

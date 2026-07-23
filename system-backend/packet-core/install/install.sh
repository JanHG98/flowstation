#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
CONFIG_SRC="$REPO_ROOT/system-backend/packet-core/config/packet-core.example.toml"
UNIT_SRC="$REPO_ROOT/system-backend/packet-core/systemd/netcore-packet-core.service"

if [[ $EUID -ne 0 ]]; then
  echo "Bitte als root/sudo ausführen." >&2
  exit 1
fi

systemctl stop netcore-packet-core.service 2>/dev/null || true
getent group netcore >/dev/null || groupadd --system netcore
id -u netcore >/dev/null 2>&1 || useradd --system --gid netcore --home-dir /var/lib/netcore-packet-core --shell /usr/sbin/nologin netcore
install -d -o netcore -g netcore /var/lib/netcore-packet-core
install -d /etc/netcore
if [[ ! -f /etc/netcore/packet-core.toml ]]; then
  install -m 0640 -o root -g netcore "$CONFIG_SRC" /etc/netcore/packet-core.toml
fi

cd "$REPO_ROOT"
cargo build --release -p netcore-packet-core
install -m 0755 target/release/netcore-packet-core /usr/local/bin/netcore-packet-core
install -m 0644 "$UNIT_SRC" /etc/systemd/system/netcore-packet-core.service
systemctl daemon-reload
systemctl enable --now netcore-packet-core.service
systemctl --no-pager --full status netcore-packet-core.service

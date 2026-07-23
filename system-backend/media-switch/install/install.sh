#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
CONFIG_SRC="$REPO_ROOT/system-backend/media-switch/config/media-switch.example.toml"
UNIT_SRC="$REPO_ROOT/system-backend/media-switch/systemd/netcore-media-switch.service"

if [[ $EUID -ne 0 ]]; then
  echo "Bitte als root/sudo ausführen." >&2
  exit 1
fi

systemctl stop netcore-media-switch.service 2>/dev/null || true
rm -f /usr/local/bin/netcore-media-switch
rm -rf "$REPO_ROOT/target"

getent group netcore >/dev/null || groupadd --system netcore
id -u netcore >/dev/null 2>&1 || useradd --system --gid netcore --home-dir /var/lib/netcore-media-switch --shell /usr/sbin/nologin netcore
install -d -o netcore -g netcore /var/lib/netcore-media-switch
install -d /etc/netcore
if [[ ! -f /etc/netcore/media-switch.toml ]]; then
  install -m 0640 -o root -g netcore "$CONFIG_SRC" /etc/netcore/media-switch.toml
fi

cd "$REPO_ROOT"
cargo clean
cargo build --release -p netcore-media-switch
install -m 0755 target/release/netcore-media-switch /usr/local/bin/netcore-media-switch
install -m 0644 "$UNIT_SRC" /etc/systemd/system/netcore-media-switch.service
systemctl daemon-reload
systemctl enable --now netcore-media-switch.service
systemctl --no-pager --full status netcore-media-switch.service

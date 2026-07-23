#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
CONFIG_SRC="$REPO_ROOT/system-backend/recorder/config/recorder.example.toml"
UNIT_SRC="$REPO_ROOT/system-backend/recorder/systemd/netcore-recorder.service"

if [[ $EUID -ne 0 ]]; then
  echo "Bitte als root/sudo ausführen." >&2
  exit 1
fi

systemctl stop netcore-recorder.service 2>/dev/null || true
rm -f /usr/local/bin/netcore-recorder
rm -rf "$REPO_ROOT/target"

getent group netcore >/dev/null || groupadd --system netcore
id -u netcore >/dev/null 2>&1 || useradd --system --gid netcore --home-dir /var/lib/netcore-recorder --shell /usr/sbin/nologin netcore
install -d -o netcore -g netcore /var/lib/netcore-recorder
install -d -o netcore -g netcore /var/lib/netcore-recorder/recordings
install -d -o netcore -g netcore /var/lib/netcore-recorder/exports
install -d /etc/netcore
if [[ ! -f /etc/netcore/recorder.toml ]]; then
  install -m 0640 -o root -g netcore "$CONFIG_SRC" /etc/netcore/recorder.toml
fi

cd "$REPO_ROOT"
cargo clean
cargo build --release -p netcore-recorder
install -m 0755 target/release/netcore-recorder /usr/local/bin/netcore-recorder
install -m 0644 "$UNIT_SRC" /etc/systemd/system/netcore-recorder.service
systemctl daemon-reload
systemctl enable --now netcore-recorder.service
systemctl --no-pager --full status netcore-recorder.service

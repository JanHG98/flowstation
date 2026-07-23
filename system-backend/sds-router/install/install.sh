#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
CONFIG_SRC="$REPO_ROOT/system-backend/sds-router/config/sds-router.example.toml"
UNIT_SRC="$REPO_ROOT/system-backend/sds-router/systemd/netcore-sds-router.service"

if [[ $EUID -ne 0 ]]; then
  echo "Bitte als root/sudo ausführen." >&2
  exit 1
fi

systemctl stop netcore-sds-router.service 2>/dev/null || true
getent group netcore >/dev/null || groupadd --system netcore
id -u netcore >/dev/null 2>&1 || useradd --system --gid netcore --home-dir /var/lib/netcore-sds-router --shell /usr/sbin/nologin netcore
install -d -o netcore -g netcore /var/lib/netcore-sds-router
install -d /etc/netcore
if [[ ! -f /etc/netcore/sds-router.toml ]]; then
  install -m 0640 -o root -g netcore "$CONFIG_SRC" /etc/netcore/sds-router.toml
fi

cd "$REPO_ROOT"
cargo build --release -p netcore-sds-router
install -m 0755 target/release/netcore-sds-router /usr/local/bin/netcore-sds-router
install -m 0644 "$UNIT_SRC" /etc/systemd/system/netcore-sds-router.service
systemctl daemon-reload
systemctl enable --now netcore-sds-router.service
systemctl --no-pager --full status netcore-sds-router.service

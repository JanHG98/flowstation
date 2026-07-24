#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
CONFIG_SRC="$REPO_ROOT/system-backend/security-core/config/security-core.example.toml"
UNIT_SRC="$REPO_ROOT/system-backend/security-core/systemd/netcore-security-core.service"

if [[ $EUID -ne 0 ]]; then
  echo "Bitte als root/sudo ausführen." >&2
  exit 1
fi

systemctl stop netcore-security-core.service 2>/dev/null || true
getent group netcore-security >/dev/null || groupadd --system netcore-security
id -u netcore-security >/dev/null 2>&1 || useradd --system --gid netcore-security \
  --home-dir /var/lib/netcore-security-core --shell /usr/sbin/nologin netcore-security
install -d -m 0700 -o netcore-security -g netcore-security /var/lib/netcore-security-core
install -d /etc/netcore
if [[ ! -f /etc/netcore/security-core.toml ]]; then
  install -m 0640 -o root -g netcore-security "$CONFIG_SRC" /etc/netcore/security-core.toml
fi

cd "$REPO_ROOT"
cargo build --release -p netcore-security-core
install -m 0755 target/release/netcore-security-core /usr/local/bin/netcore-security-core
install -m 0644 "$UNIT_SRC" /etc/systemd/system/netcore-security-core.service
systemctl daemon-reload
systemctl enable --now netcore-security-core.service
systemctl --no-pager --full status netcore-security-core.service

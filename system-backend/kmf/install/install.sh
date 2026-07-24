#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
PACKAGE="$ROOT/system-backend/kmf"

if [[ ${EUID:-$(id -u)} -ne 0 ]]; then
  echo "Bitte als root ausführen." >&2
  exit 1
fi

getent group netcore-kmf >/dev/null || groupadd --system netcore-kmf
id -u netcore-kmf >/dev/null 2>&1 || useradd --system --gid netcore-kmf --home /var/lib/netcore-kmf --shell /usr/sbin/nologin netcore-kmf
install -d -m 0700 -o netcore-kmf -g netcore-kmf /var/lib/netcore-kmf /var/lib/netcore-kmf/backups /var/lib/netcore-kmf/bootstrap
install -d -m 0750 /etc/netcore

cargo build --release --package netcore-kmf
install -m 0755 "$ROOT/target/release/netcore-kmf" /usr/local/bin/netcore-kmf
if [[ ! -e /etc/netcore/kmf.toml ]]; then
  install -m 0640 -o root -g netcore-kmf "$PACKAGE/config/kmf.example.toml" /etc/netcore/kmf.toml
fi
install -m 0644 "$PACKAGE/systemd/netcore-kmf.service" /etc/systemd/system/netcore-kmf.service
systemctl daemon-reload
systemctl enable --now netcore-kmf.service

echo "KMF läuft im OPEN-LAB-Modus auf Port 8190. Managementnetz strikt isolieren."

#!/usr/bin/env bash
set -euo pipefail

if [[ ${EUID:-$(id -u)} -ne 0 ]]; then
  echo "Bitte als root ausführen." >&2
  exit 1
fi
systemctl disable --now netcore-kmf.service 2>/dev/null || true
rm -f /etc/systemd/system/netcore-kmf.service /usr/local/bin/netcore-kmf
systemctl daemon-reload
cat <<'MSG'
Binary und Service wurden entfernt.
Aus Sicherheitsgründen bleiben /etc/netcore/kmf.toml und /var/lib/netcore-kmf erhalten.
Master-Key, Vault, Bootstrap-Dateien und Backups nur nach gesonderter Prüfung manuell löschen.
MSG

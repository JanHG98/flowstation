#!/usr/bin/env bash
set -euo pipefail
if [[ ${EUID} -ne 0 ]]; then
  echo "Bitte als root ausführen." >&2
  exit 1
fi
systemctl disable --now netcore-mobility-core.service 2>/dev/null || true
rm -f /etc/systemd/system/netcore-mobility-core.service
rm -f /usr/local/bin/netcore-mobility-core
systemctl daemon-reload
echo "Konfiguration unter /etc/netcore/mobility-core.toml und Daten unter /var/lib/netcore-mobility-core wurden nicht gelöscht."

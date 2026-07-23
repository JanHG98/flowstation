#!/usr/bin/env bash
set -euo pipefail
if [[ $EUID -ne 0 ]]; then echo "Bitte als root/sudo ausführen." >&2; exit 1; fi
systemctl disable --now netcore-recorder.service 2>/dev/null || true
rm -f /etc/systemd/system/netcore-recorder.service
rm -f /usr/local/bin/netcore-recorder
systemctl daemon-reload
printf '%s\n' "Konfiguration, Aufnahmen und Exporte bleiben unter /etc/netcore und /var/lib/netcore-recorder erhalten."

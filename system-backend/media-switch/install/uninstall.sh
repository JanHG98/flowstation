#!/usr/bin/env bash
set -euo pipefail
if [[ $EUID -ne 0 ]]; then echo "Bitte als root/sudo ausführen." >&2; exit 1; fi
systemctl disable --now netcore-media-switch.service 2>/dev/null || true
rm -f /etc/systemd/system/netcore-media-switch.service
rm -f /usr/local/bin/netcore-media-switch
systemctl daemon-reload
printf '%s\n' "Konfiguration und Betriebsdaten bleiben unter /etc/netcore und /var/lib/netcore-media-switch erhalten."

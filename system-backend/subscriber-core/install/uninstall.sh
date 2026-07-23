#!/usr/bin/env bash
set -euo pipefail
if [[ ${EUID} -ne 0 ]]; then echo "Bitte als root ausführen." >&2; exit 1; fi
systemctl disable --now netcore-subscriber-core.service 2>/dev/null || true
rm -f /etc/systemd/system/netcore-subscriber-core.service /usr/local/bin/netcore-subscriber-core
systemctl daemon-reload
echo "Konfiguration und Daten unter /etc/netcore und /var/lib/netcore-subscriber-core wurden absichtlich behalten."

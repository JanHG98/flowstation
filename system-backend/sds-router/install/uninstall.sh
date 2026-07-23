#!/usr/bin/env bash
set -euo pipefail
if [[ $EUID -ne 0 ]]; then echo "Bitte als root/sudo ausführen." >&2; exit 1; fi
systemctl disable --now netcore-sds-router.service 2>/dev/null || true
rm -f /etc/systemd/system/netcore-sds-router.service
rm -f /usr/local/bin/netcore-sds-router
systemctl daemon-reload
printf '%s\n' "Konfiguration und Queue bleiben unter /etc/netcore und /var/lib/netcore-sds-router erhalten."

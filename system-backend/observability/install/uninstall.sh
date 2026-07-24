#!/usr/bin/env bash
set -euo pipefail
[[ ${EUID} -eq 0 ]] || { echo "uninstall.sh must run as root" >&2; exit 1; }
systemctl disable --now netcore-observability.service 2>/dev/null || true
rm -f /etc/systemd/system/netcore-observability.service
systemctl daemon-reload
rm -rf /opt/netcore-observability
echo "State and configuration intentionally retained in /var/lib/netcore-observability and /etc/netcore."

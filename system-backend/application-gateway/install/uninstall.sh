#!/usr/bin/env bash
set -euo pipefail
[[ ${EUID} -eq 0 ]] || { echo "uninstall.sh must run as root" >&2; exit 1; }
systemctl disable --now netcore-application-gateway.service 2>/dev/null || true
rm -f /etc/systemd/system/netcore-application-gateway.service
rm -rf /opt/netcore-application-gateway
systemctl daemon-reload
echo "Application Gateway binaries removed. Configuration and /var/lib/netcore-application-gateway were preserved."

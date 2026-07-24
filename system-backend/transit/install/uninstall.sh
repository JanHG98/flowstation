#!/usr/bin/env bash
set -euo pipefail

if [[ ${EUID} -ne 0 ]]; then
  echo "uninstall.sh must run as root" >&2
  exit 1
fi

systemctl disable --now netcore-transit.service 2>/dev/null || true
rm -f /etc/systemd/system/netcore-transit.service
systemctl daemon-reload
rm -rf /opt/netcore-transit

echo "Binary and service removed. Configuration and /var/lib/netcore-transit were kept deliberately."

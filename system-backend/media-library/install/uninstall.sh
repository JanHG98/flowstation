#!/usr/bin/env bash
set -euo pipefail
[[ ${EUID} -eq 0 ]] || { echo "uninstall.sh must run as root" >&2; exit 1; }
systemctl disable --now netcore-media-library.service 2>/dev/null || true
rm -f /etc/systemd/system/netcore-media-library.service
rm -rf /opt/netcore-media-library
systemctl daemon-reload
echo "Media Library binaries removed. Configuration and /var/lib/netcore-media-library were preserved."

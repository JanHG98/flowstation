#!/usr/bin/env bash
set -euo pipefail
systemctl disable --now netcore-control-room.service 2>/dev/null || true
rm -f /etc/systemd/system/netcore-control-room.service /usr/local/bin/netcore-control-room
systemctl daemon-reload
printf 'Configuration and state were kept in /etc/netcore-control-room and /var/lib/netcore-control-room\n'

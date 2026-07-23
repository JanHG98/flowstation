#!/usr/bin/env bash
set -euo pipefail
sudo systemctl disable --now netcore-call-control.service 2>/dev/null || true
sudo rm -f /etc/systemd/system/netcore-call-control.service /usr/local/bin/netcore-call-control
sudo systemctl daemon-reload
printf '%s\n' 'Konfiguration und Daten unter /etc/netcore und /var/lib/netcore-call-control wurden absichtlich nicht gelöscht.'

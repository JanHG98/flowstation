#!/usr/bin/env bash
set -euo pipefail
sudo systemctl disable --now netcore-group-core.service 2>/dev/null || true
sudo rm -f /etc/systemd/system/netcore-group-core.service /usr/local/bin/netcore-group-core
sudo systemctl daemon-reload
printf 'Konfiguration und Daten unter /etc/netcore sowie /var/lib/netcore-group-core wurden absichtlich nicht gelöscht.\n'

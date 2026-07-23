#!/usr/bin/env bash
set -euo pipefail

if [[ ${EUID} -ne 0 ]]; then
  echo "Bitte als root ausführen." >&2
  exit 1
fi

systemctl disable --now netcore-node-gateway.service 2>/dev/null || true
rm -f /etc/systemd/system/netcore-node-gateway.service
rm -f /usr/local/bin/netcore-node-gateway
systemctl daemon-reload

echo "Konfiguration unter /etc/netcore/node-gateway.toml und Datenordner wurden bewusst nicht gelöscht."

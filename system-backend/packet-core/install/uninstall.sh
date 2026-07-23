#!/usr/bin/env bash
set -euo pipefail
if [[ $EUID -ne 0 ]]; then echo "Bitte als root/sudo ausführen." >&2; exit 1; fi
systemctl disable --now netcore-packet-core.service 2>/dev/null || true
rm -f /etc/systemd/system/netcore-packet-core.service /usr/local/bin/netcore-packet-core
systemctl daemon-reload
cat <<'MSG'
Binary und Unit wurden entfernt.
Konfiguration und Zustandsdaten bleiben absichtlich erhalten:
  /etc/netcore/packet-core.toml
  /var/lib/netcore-packet-core/
MSG

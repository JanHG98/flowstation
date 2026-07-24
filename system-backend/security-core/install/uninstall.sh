#!/usr/bin/env bash
set -euo pipefail
if [[ $EUID -ne 0 ]]; then echo "Bitte als root/sudo ausführen." >&2; exit 1; fi
systemctl disable --now netcore-security-core.service 2>/dev/null || true
rm -f /etc/systemd/system/netcore-security-core.service /usr/local/bin/netcore-security-core
systemctl daemon-reload
cat <<'MSG'
Binary und Unit wurden entfernt.
Konfiguration und Zustandsdaten bleiben absichtlich erhalten:
  /etc/netcore/security-core.toml
  /var/lib/netcore-security-core
MSG

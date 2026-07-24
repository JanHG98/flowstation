#!/usr/bin/env bash
set -euo pipefail
[[ $EUID -eq 0 ]] || { echo "Bitte als root/sudo ausführen." >&2; exit 1; }
systemctl disable --now netcore-ip-gateway.service 2>/dev/null || true
rm -f /etc/systemd/system/netcore-ip-gateway.service /usr/local/bin/netcore-ip-gateway
systemctl daemon-reload
cat <<'NOTE'
Binärdatei und systemd-Unit wurden entfernt.
Bewusst erhalten:
  /etc/netcore/ip-gateway.toml
  /var/lib/netcore-ip-gateway/
Die nftables-Tabellen netcore_ip_gateway und netcore_ip_gateway_nat können bei Bedarf manuell entfernt werden.
NOTE

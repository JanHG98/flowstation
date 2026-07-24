#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
CONFIG_SRC="$REPO_ROOT/system-backend/ip-gateway/config/ip-gateway.example.toml"
UNIT_SRC="$REPO_ROOT/system-backend/ip-gateway/systemd/netcore-ip-gateway.service"

if [[ $EUID -ne 0 ]]; then
  echo "Bitte als root/sudo ausführen." >&2
  exit 1
fi

for command in ip nft; do
  command -v "$command" >/dev/null || {
    echo "Fehlendes Laufzeitwerkzeug: $command (Pakete iproute2 und nftables installieren)." >&2
    exit 1
  }
done
[[ -c /dev/net/tun ]] || {
  echo "/dev/net/tun fehlt. Im LXC muss das TUN-Device durchgereicht werden." >&2
  exit 1
}

systemctl stop netcore-ip-gateway.service 2>/dev/null || true
getent group netcore >/dev/null || groupadd --system netcore
id -u netcore >/dev/null 2>&1 || useradd --system --gid netcore --home-dir /var/lib/netcore-ip-gateway --shell /usr/sbin/nologin netcore
install -d -o netcore -g netcore /var/lib/netcore-ip-gateway /var/lib/netcore-ip-gateway/captures
install -d /etc/netcore
if [[ ! -f /etc/netcore/ip-gateway.toml ]]; then
  install -m 0640 -o root -g netcore "$CONFIG_SRC" /etc/netcore/ip-gateway.toml
fi

cd "$REPO_ROOT"
cargo build --release -p netcore-ip-gateway
install -m 0755 target/release/netcore-ip-gateway /usr/local/bin/netcore-ip-gateway
install -m 0644 "$UNIT_SRC" /etc/systemd/system/netcore-ip-gateway.service
systemctl daemon-reload
systemctl enable --now netcore-ip-gateway.service
systemctl --no-pager --full status netcore-ip-gateway.service

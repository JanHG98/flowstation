#!/usr/bin/env bash
set -euo pipefail
if [[ ${EUID} -ne 0 ]]; then echo "Bitte als root ausführen." >&2; exit 1; fi
REPO_ROOT=${REPO_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)}
cd "$REPO_ROOT"
systemctl stop netcore-subscriber-core.service 2>/dev/null || true
rm -f /usr/local/bin/netcore-subscriber-core
rm -rf target/release/netcore-subscriber-core target/release/deps/netcore_subscriber_core-*
cargo build --release -p netcore-subscriber-core
getent group netcore >/dev/null || groupadd --system netcore
id netcore >/dev/null 2>&1 || useradd --system --gid netcore --home-dir /var/lib/netcore-subscriber-core --shell /usr/sbin/nologin netcore
install -d -o netcore -g netcore /var/lib/netcore-subscriber-core
install -d -m 0755 /etc/netcore
install -m 0755 target/release/netcore-subscriber-core /usr/local/bin/netcore-subscriber-core
if [[ ! -f /etc/netcore/subscriber-core.toml ]]; then install -m 0644 system-backend/subscriber-core/config/subscriber-core.example.toml /etc/netcore/subscriber-core.toml; fi
install -m 0644 system-backend/subscriber-core/systemd/netcore-subscriber-core.service /etc/systemd/system/netcore-subscriber-core.service
systemctl daemon-reload
systemctl enable --now netcore-subscriber-core.service
systemctl --no-pager --full status netcore-subscriber-core.service
echo "WebUI: http://<LXC-IP>:8100/"
echo "WARNUNG: offener Testmodus ohne Authentifizierung, Tokens oder TLS."

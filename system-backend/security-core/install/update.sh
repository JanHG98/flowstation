#!/usr/bin/env bash
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
if [[ $EUID -ne 0 ]]; then echo "Bitte als root/sudo ausführen." >&2; exit 1; fi
cd "$REPO_ROOT"
cargo build --release -p netcore-security-core
systemctl stop netcore-security-core.service
install -m 0755 target/release/netcore-security-core /usr/local/bin/netcore-security-core
install -m 0644 system-backend/security-core/systemd/netcore-security-core.service /etc/systemd/system/netcore-security-core.service
systemctl daemon-reload
systemctl start netcore-security-core.service
systemctl --no-pager --full status netcore-security-core.service

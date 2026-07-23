#!/usr/bin/env bash
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
if [[ $EUID -ne 0 ]]; then echo "Bitte als root/sudo ausführen." >&2; exit 1; fi
systemctl stop netcore-sds-router.service 2>/dev/null || true
cd "$REPO_ROOT"
cargo build --release -p netcore-sds-router
install -m 0755 target/release/netcore-sds-router /usr/local/bin/netcore-sds-router
install -m 0644 system-backend/sds-router/systemd/netcore-sds-router.service /etc/systemd/system/netcore-sds-router.service
systemctl daemon-reload
systemctl restart netcore-sds-router.service
systemctl --no-pager --full status netcore-sds-router.service

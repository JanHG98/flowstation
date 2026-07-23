#!/usr/bin/env bash
set -euo pipefail
REPO_ROOT=${REPO_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)}
exec env REPO_ROOT="$REPO_ROOT" "$REPO_ROOT/system-backend/mobility-core/install/install.sh"

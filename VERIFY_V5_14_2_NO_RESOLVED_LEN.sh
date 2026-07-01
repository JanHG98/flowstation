#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
echo "Checking for stale resolved.len()..."
if grep -R "resolved\.len()" -n bins/netcore-control-room system-backend/control-room 2>/dev/null; then
  echo "ERROR: stale resolved.len() is still present"
  exit 1
fi
echo "OK: no stale resolved.len() found"
grep -R "V5_14_2_NO_RESOLVED_LEN_MARKER" -n bins/netcore-control-room/src/http.rs

#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../../.."
sudo systemctl stop netcore-call-control.service 2>/dev/null || true
sudo rm -f /usr/local/bin/netcore-call-control
rm -rf target/release/netcore-call-control target/release/deps/netcore_call_control-* target/release/.fingerprint/netcore-call-control-*
cargo clean -p netcore-call-control 2>/dev/null || true
cargo build --release -p netcore-call-control
sudo install -o root -g root -m 0755 target/release/netcore-call-control /usr/local/bin/netcore-call-control
sudo systemctl restart netcore-call-control.service

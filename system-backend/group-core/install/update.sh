#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../../.."
sudo systemctl stop netcore-group-core.service 2>/dev/null || true
sudo rm -f /usr/local/bin/netcore-group-core
rm -rf target/release/netcore-group-core target/release/deps/netcore_group_core-* target/release/.fingerprint/netcore-group-core-*
cargo clean -p netcore-group-core 2>/dev/null || true
cargo build --release -p netcore-group-core
sudo install -o root -g root -m 0755 target/release/netcore-group-core /usr/local/bin/netcore-group-core
sudo systemctl restart netcore-group-core.service

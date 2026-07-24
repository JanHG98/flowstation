#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
PREFIX="${PREFIX:-/opt/netcore-media-library}"
CONFIG="${CONFIG:-/etc/netcore/media-library.toml}"
SERVICE="${SERVICE:-/etc/systemd/system/netcore-media-library.service}"
[[ ${EUID} -eq 0 ]] || { echo "install.sh must run as root" >&2; exit 1; }
id -u netcore-media-library >/dev/null 2>&1 || useradd --system --home /var/lib/netcore-media-library --shell /usr/sbin/nologin netcore-media-library
install -d -o netcore-media-library -g netcore-media-library -m 0750 /var/lib/netcore-media-library /var/lib/netcore-media-library/assets /var/lib/netcore-media-library/tmp /var/lib/netcore-media-library/backups
install -d -o root -g root -m 0755 "${PREFIX}/bin" "$(dirname "${CONFIG}")"
cargo build --locked --release --package netcore-media-library --manifest-path "${ROOT}/Cargo.toml"
install -o root -g root -m 0755 "${ROOT}/target/release/netcore-media-library" "${PREFIX}/bin/netcore-media-library"
install -o root -g root -m 0644 "${ROOT}/system-backend/media-library/README.md" "${PREFIX}/README.md"
if [[ ! -e "${CONFIG}" ]]; then
  install -o root -g netcore-media-library -m 0640 "${ROOT}/system-backend/media-library/config/media-library.example.toml" "${CONFIG}"
fi
install -o root -g root -m 0644 "${ROOT}/system-backend/media-library/systemd/netcore-media-library.service" "${SERVICE}"
systemctl daemon-reload
systemctl enable --now netcore-media-library.service
command -v ffmpeg >/dev/null || echo "WARNING: ffmpeg is missing; non-canonical WAV and MP3 preview processing will fail." >&2
echo "NetCore Media Library installed. WebUI: http://<lxc>:8230"
echo "OPEN LAB: no login, no tokens and no TLS. Isolated management network only."

#!/usr/bin/env bash
set -euo pipefail

SERVICE_USER="${SERVICE_USER:-bluestation}"
VENV="${VENV:-/opt/netcore-piper}"
VOICE_DIR="${VOICE_DIR:-/var/lib/netcore/piper}"
PIPER_PORT="${PIPER_PORT:-5005}"
VOICE_LIST="${VOICE_LIST:-de_DE-thorsten-high de_DE-karlsson-low de_DE-pavoque-low de_DE-thorsten_emotional-medium}"

if [[ ${EUID} -ne 0 ]]; then
  echo "Run this helper as root (sudo)." >&2
  exit 1
fi
if ! id "$SERVICE_USER" >/dev/null 2>&1; then
  echo "Service user '$SERVICE_USER' does not exist." >&2
  exit 1
fi
if [[ ! -x "$VENV/bin/python" ]]; then
  echo "Piper virtual environment not found at $VENV." >&2
  exit 1
fi

install -d -o "$SERVICE_USER" -g "$(id -gn "$SERVICE_USER")" -m 0750 "$VOICE_DIR"
read -r -a voices <<< "$VOICE_LIST"
for voice in "${voices[@]}"; do
  echo "Downloading/checking Piper voice: $voice"
  runuser -u "$SERVICE_USER" -- \
    "$VENV/bin/python" -m piper.download_voices --data-dir "$VOICE_DIR" "$voice"
done

systemctl restart netcore-piper.service
sleep 2

echo
echo "Available Piper voices:"
curl --fail --silent --show-error "http://127.0.0.1:$PIPER_PORT/voices" \
  | "$VENV/bin/python" -c 'import json,sys; print("\n".join(sorted(json.load(sys.stdin).keys())))'

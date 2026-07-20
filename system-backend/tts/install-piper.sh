#!/usr/bin/env bash
set -euo pipefail

SERVICE_USER="${SERVICE_USER:-bluestation}"
SERVICE_GROUP="${SERVICE_GROUP:-$SERVICE_USER}"
DEFAULT_VOICE="${DEFAULT_VOICE:-${VOICE:-de_DE-thorsten-medium}}"
VOICE_LIST="${VOICE_LIST:-de_DE-thorsten-medium de_DE-thorsten-high de_DE-karlsson-low de_DE-pavoque-low de_DE-thorsten_emotional-medium}"
VENV="${VENV:-/opt/netcore-piper}"
VOICE_DIR="${VOICE_DIR:-/var/lib/netcore/piper}"
TTS_CACHE="${TTS_CACHE:-/var/cache/netcore/tts}"
TTS_TEMPLATES="${TTS_TEMPLATES:-/var/lib/netcore/tts/templates}"
PIPER_PORT="${PIPER_PORT:-5005}"
UNIT_PATH="/etc/systemd/system/netcore-piper.service"

if [[ ${EUID} -ne 0 ]]; then
  echo "Run this installer as root (sudo)." >&2
  exit 1
fi
if ! id "$SERVICE_USER" >/dev/null 2>&1; then
  echo "Service user '$SERVICE_USER' does not exist. Set SERVICE_USER and SERVICE_GROUP." >&2
  exit 1
fi

apt-get update
apt-get install -y python3 python3-venv curl
python3 -m venv "$VENV"
"$VENV/bin/python" -m pip install --upgrade pip
"$VENV/bin/python" -m pip install --upgrade 'piper-tts[http]'

install -d -o "$SERVICE_USER" -g "$SERVICE_GROUP" -m 0750 \
  "$VOICE_DIR" "$TTS_CACHE" "$TTS_TEMPLATES"

read -r -a voices <<< "$VOICE_LIST"
if [[ ! " ${voices[*]} " =~ " ${DEFAULT_VOICE} " ]]; then
  voices+=("$DEFAULT_VOICE")
fi
for voice in "${voices[@]}"; do
  echo "Downloading/checking Piper voice: $voice"
  runuser -u "$SERVICE_USER" -- \
    "$VENV/bin/python" -m piper.download_voices --data-dir "$VOICE_DIR" "$voice"
done

sed \
  -e "s|^User=.*|User=$SERVICE_USER|" \
  -e "s|^Group=.*|Group=$SERVICE_GROUP|" \
  -e "s|^WorkingDirectory=.*|WorkingDirectory=$VOICE_DIR|" \
  -e "s|^Environment=HOME=.*|Environment=HOME=$VOICE_DIR|" \
  -e "s|^ExecStart=.*|ExecStart=$VENV/bin/python -m piper.http_server -m $DEFAULT_VOICE --data-dir $VOICE_DIR --host 127.0.0.1 --port $PIPER_PORT|" \
  -e "s|^ReadWritePaths=.*|ReadWritePaths=$VOICE_DIR $TTS_CACHE|" \
  "$(dirname "$0")/netcore-piper.service" > "$UNIT_PATH"
chmod 0644 "$UNIT_PATH"

systemctl daemon-reload
systemctl enable --now netcore-piper.service
systemctl restart netcore-piper.service
systemctl --no-pager --full status netcore-piper.service || true

echo
echo "Piper voices available on port $PIPER_PORT:"
curl --fail --silent --show-error "http://127.0.0.1:$PIPER_PORT/voices" \
  | "$VENV/bin/python" -c 'import json,sys; print("\n".join(sorted(json.load(sys.stdin).keys())))'
echo

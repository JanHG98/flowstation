#!/usr/bin/env bash
set -euo pipefail

SERVICE_USER="${SERVICE_USER:-bluestation}"
SERVICE_GROUP="${SERVICE_GROUP:-$SERVICE_USER}"
VOICE="${VOICE:-de_DE-thorsten-medium}"
VENV="${VENV:-/opt/netcore-piper}"
VOICE_DIR="${VOICE_DIR:-/var/lib/netcore/piper}"
TTS_CACHE="${TTS_CACHE:-/var/cache/netcore/tts}"
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

install -d -o "$SERVICE_USER" -g "$SERVICE_GROUP" -m 0750 "$VOICE_DIR" "$TTS_CACHE"
runuser -u "$SERVICE_USER" -- "$VENV/bin/python" -m piper.download_voices --data-dir "$VOICE_DIR" "$VOICE"

sed \
  -e "s|^User=.*|User=$SERVICE_USER|" \
  -e "s|^Group=.*|Group=$SERVICE_GROUP|" \
  -e "s|^WorkingDirectory=.*|WorkingDirectory=$VOICE_DIR|" \
  -e "s|^Environment=HOME=.*|Environment=HOME=$VOICE_DIR|" \
  -e "s|^ExecStart=.*|ExecStart=$VENV/bin/python -m piper.http_server -m $VOICE --data-dir $VOICE_DIR --host 127.0.0.1 --port $PIPER_PORT|" \
  -e "s|^ReadWritePaths=.*|ReadWritePaths=$VOICE_DIR $TTS_CACHE|" \
  "$(dirname "$0")/netcore-piper.service" > "$UNIT_PATH"
chmod 0644 "$UNIT_PATH"

systemctl daemon-reload
systemctl enable --now netcore-piper.service
systemctl --no-pager --full status netcore-piper.service || true

echo
echo "Piper health check:"
curl --fail --silent --show-error http://127.0.0.1:$PIPER_PORT/voices | head -c 500 || true
echo

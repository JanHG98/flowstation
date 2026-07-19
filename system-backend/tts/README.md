# NetCore local Piper TTS provider

FlowStation TTS Phase 1 expects a Piper HTTP server on `127.0.0.1:5000`.
The provider is intentionally separate from the RF process: it creates a complete WAV first;
FlowStation then passes that file into the existing audio-player/ACELP path.

## Quick installation

Adjust the service account if `bluestation-bs` runs under another user:

```bash
cd system-backend/tts
sudo SERVICE_USER=bluestation SERVICE_GROUP=bluestation ./install-piper.sh
```

Optional voice override:

```bash
sudo VOICE=de_DE-thorsten-medium SERVICE_USER=bluestation ./install-piper.sh
```

## Manual health checks

```bash
systemctl status netcore-piper --no-pager
curl -fsS http://127.0.0.1:5000/voices
```

Synthesis test:

```bash
curl -fsS \
  -H 'Content-Type: application/json' \
  -d '{"text":"Achtung. Dies ist eine Testdurchsage.","voice":"de_DE-thorsten-medium","length_scale":1.0526}' \
  http://127.0.0.1:5000/synthesize \
  -o /tmp/netcore-tts-test.wav
file /tmp/netcore-tts-test.wav
```

The service template binds only to localhost. Keep it that way unless a separate reverse proxy,
authentication policy and firewall are deliberately configured.

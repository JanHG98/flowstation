# NetCore local Piper TTS provider

FlowStation expects a Piper HTTP server on `127.0.0.1:5005`.
The provider runs separately from the RF process: Piper first creates a complete WAV file;
FlowStation then feeds the validated file into the existing audio-player/ACELP path.

## Installed German voices

The installer downloads these models by default:

- `de_DE-thorsten-medium`
- `de_DE-thorsten-high`
- `de_DE-karlsson-low`
- `de_DE-pavoque-low`
- `de_DE-thorsten_emotional-medium`

Piper lists every `.onnx` model in its data directory through `/voices` and loads a selected
model on demand. FlowStation marks configured voices that are not actually present as
`NICHT INSTALLIERT` instead of silently letting Piper fall back to another voice.

## Complete installation or update

Adjust the service account when `tetra.service` runs under another user:

```bash
cd system-backend/tts
sudo SERVICE_USER=bluestation SERVICE_GROUP=bluestation ./install-piper.sh
```

For an existing Piper installation, only add the extra voices:

```bash
cd system-backend/tts
sudo SERVICE_USER=bluestation ./install-extra-voices.sh
```

Custom model set:

```bash
sudo \
  SERVICE_USER=bluestation \
  DEFAULT_VOICE=de_DE-thorsten-medium \
  VOICE_LIST="de_DE-thorsten-medium de_DE-karlsson-low de_DE-pavoque-low" \
  ./install-piper.sh
```

## Local template directory

The installer also creates:

```text
/var/lib/netcore/tts/templates
```

It is owned by the FlowStation service account and contains human-readable files named:

```text
<template-id>.tts.toml
```

Generated texts are automatically saved there when
`auto_save_generated_templates = true` is configured.

## Health checks

```bash
systemctl status netcore-piper --no-pager
curl -fsS http://127.0.0.1:5005/voices
```

Show only installed model names:

```bash
curl -fsS http://127.0.0.1:5005/voices \
  | /opt/netcore-piper/bin/python -c \
    'import json,sys; print("\n".join(sorted(json.load(sys.stdin).keys())))'
```

Synthesis test:

```bash
curl -fsS \
  -H 'Content-Type: application/json' \
  -d '{"text":"Achtung. Dies ist eine Testdurchsage.","voice":"de_DE-karlsson-low","length_scale":1.0526}' \
  http://127.0.0.1:5005/synthesize \
  -o /tmp/netcore-tts-test.wav
file /tmp/netcore-tts-test.wav
```

The service binds only to localhost. Keep it that way unless a reverse proxy,
authentication policy and firewall are deliberately configured.

# LXC-Deployment

## Empfohlene Basis

- Debian 12/13 LXC
- 2 vCPU
- 2 GiB RAM
- eigenes Storage-Mount entsprechend der gewünschten Aufbewahrung
- Rust-Toolchain nur für den Build nötig

## Netzwerk

Der Recorder benötigt:

- ausgehend HTTP zum Media Switch Port 8130
- eingehend HTTP Port 8140 für WebUI/API

Es gibt in dieser Phase kein TLS und keine Anmeldung. Port 8140 gehört deshalb ausschließlich in das isolierte Managementnetz.

## Installation

```bash
sudo apt update
sudo apt install -y build-essential curl pkg-config
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
cd /opt/netcore-tetra
sudo system-backend/recorder/install/install.sh
```

Danach `/etc/netcore/recorder.toml` prüfen. Besonders `tap_url`, `sessions_url`, Storage-Pfade und freien Mindestplatz anpassen.

## Mountpoint

Für großen externen Storage kann `/var/lib/netcore-recorder` direkt als LXC-Mountpoint eingebunden werden. Der Pfad muss für Benutzer und Gruppe `netcore` schreibbar sein.

## Reihenfolge

```text
Node Gateway → Call Control → Media Switch → Recorder → TBS
```

Der Recorder verbindet sich automatisch erneut. Er darf daher auch später starten; der Replay-Ring bestimmt nur, wie lange Frames während seiner Abwesenheit nachgeholt werden können.

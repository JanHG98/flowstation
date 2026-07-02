# NetCore / FlowStation Build- und Startbefehle

Stand: Control-Room-Zweig mit Basisstation, Control-Room-Core und nativer Operator-Konsole.

## 1. Basisstation / TBS

Dieser Befehl ist für die Maschine gedacht, auf der die eigentliche Basisstation läuft.  
Hier darf `bluestation-bs` mit Asterisk-Feature gebaut werden, weil dort die Funk-/Codec-/SDR-Abhängigkeiten hingehören.

```bash
git fetch && \
git checkout control-room && \
git pull --ff-only && \
cargo build --release \
  -p bluestation-bs \
  -p netcore-control-room \
  -p netcore-control-room-operator \
  --features bluestation-bs/asterisk && \
./target/release/netcore-control-room-operator \
  --api http://10.0.1.25:9010 \
  dashboard
```

Hinweis:  
Der Operator verbindet sich hier gegen den Control-Room-Core unter:

```text
http://10.0.1.25:9010
```

Wenn der Control-Room-Core im LXC läuft, muss die TBS-Config ebenfalls auf diese IP zeigen:

```toml
[control_room]
enabled = true
host = "10.0.1.25"
port = 9010
use_tls = false
endpoint_path = "/node"
```

---

## 2. Control-Room-LXC / Container

Dieser Befehl ist für den LXC gedacht.  
Hier wird bewusst **nicht** `bluestation-bs` gebaut, damit im Container keine unnötigen SDR-/Codec-Abhängigkeiten wie SoapySDR, GSM oder TETRA-Codec mitgezogen werden.

```bash
git fetch && \
git checkout control-room && \
git pull --ff-only && \
cargo build --release \
  -p netcore-control-room \
  -p netcore-control-room-operator && \
./target/release/netcore-control-room \
  --bind 0.0.0.0:9010
```

Damit lauscht der Control-Room-Core im Container auf allen Interfaces auf Port `9010`.

Test im Container:

```bash
curl http://127.0.0.1:9010/health | jq
```

Test von der Basisstation oder einem anderen Rechner:

```bash
curl http://10.0.1.25:9010/health | jq
```

---

## 3. Operator-Konsole separat starten

Wenn der Control-Room-Core bereits als systemd-Service läuft, kann die Operator-Konsole separat gestartet werden:

```bash
./target/release/netcore-control-room-operator \
  --api http://10.0.1.25:9010 \
  dashboard
```

Alternativ mit Umgebungsvariable:

```bash
export NETCORE_CONTROL_ROOM_API=http://10.0.1.25:9010

./target/release/netcore-control-room-operator dashboard
```

---

## 4. Empfohlene Rollenverteilung

```text
Basisstation / TBS:
- bluestation-bs
- SDR / Funk / Asterisk / Codec-Kram
- verbindet sich als Node zum Control-Room-Core

Control-Room-LXC:
- netcore-control-room
- State / API / Commands / Telemetry
- keine SDR-Hardware nötig

Operator-Rechner:
- netcore-control-room-operator
- native Leitstellen-Konsole
- spricht per API mit dem Control-Room-Core
```

Kurz gesagt:

```text
TBS funkt.
LXC führt.
Operator bedient.
```

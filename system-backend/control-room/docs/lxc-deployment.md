# NetCore Control Room im LXC betreiben

Diese Anleitung trennt die Rollen sauber:

```text
TBS funkt.
LXC führt den Control-Room-Core.
Operator bedient mit der nativen Operator-Konsole.
```

## Pakete im LXC

Der Control Room benötigt keine SDR-/Codec-Libraries. Für den Build reichen die normalen Rust-/Build-Werkzeuge:

```bash
apt update
apt install -y git curl build-essential pkg-config ca-certificates jq sqlite3
```

Rust installieren, falls noch nicht vorhanden:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

## Repo bauen

```bash
cd /opt/netcore/flowstation
git fetch && \
git checkout control-room && \
git pull --ff-only && \
cargo build --release \
  -p netcore-control-room \
  -p netcore-control-room-operator
```

Wichtig: Im LXC **nicht** `bluestation-bs` bauen, sonst werden unnötige Funk-/Codec-Abhängigkeiten gezogen.

## Config installieren

```bash
install -d -o root -g root /etc/netcore-control-room
install -m 0644 system-backend/control-room/config/control-room.example.toml /etc/netcore-control-room/control-room.toml
```

Bei Bedarf die Bind-Adresse oder den Datenbankpfad in `/etc/netcore-control-room/control-room.toml` anpassen.

## Service-User und Datenverzeichnis

```bash
useradd --system --home /var/lib/netcore-control-room --shell /usr/sbin/nologin netcore || true
install -d -o netcore -g netcore /var/lib/netcore-control-room
```

## systemd-Service installieren

```bash
install -m 0644 system-backend/control-room/systemd/netcore-control-room.service /etc/systemd/system/netcore-control-room.service
systemctl daemon-reload
systemctl enable --now netcore-control-room
```

Logs:

```bash
journalctl -u netcore-control-room -f
```

## Tests

Im LXC:

```bash
curl http://127.0.0.1:9010/health | jq
curl http://127.0.0.1:9010/api/overview | jq
```

Von der TBS oder vom Operator-Rechner:

```bash
curl http://10.0.1.25:9010/health | jq
```

## Persistenz prüfen

```bash
sqlite3 /var/lib/netcore-control-room/control-room.sqlite3 '.tables'
sqlite3 /var/lib/netcore-control-room/control-room.sqlite3 'select command_id,status,updated_at from commands order by updated_at desc limit 10;'
sqlite3 /var/lib/netcore-control-room/control-room.sqlite3 'select node_id,issi,latitude,longitude,updated_at from locations;'
```

Die Datenbank speichert aktuell:

- Node-Sessions
- Eventlog, ohne RF-/Health-Dauerfeuer bei `persist_noisy_events = false`
- Command-Audit inklusive Responses
- SDS-Log
- letzte bekannte Locations
- Emergencies

## TBS-Config

Auf der TBS muss der Control Room auf den LXC zeigen:

```toml
[control_room]
enabled = true
host = "10.0.1.25"
port = 9010
use_tls = false
endpoint_path = "/node"
```

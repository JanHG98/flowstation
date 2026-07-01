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
apt install -y git curl build-essential pkg-config ca-certificates jq sqlite3 openssl
```

Rust installieren, falls noch nicht vorhanden:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

## Repo bauen

```bash
cd /opt/netcore/flowstation
git fetch && git checkout control-room && git pull --ff-only && cargo build --release   -p netcore-control-room   -p netcore-control-room-operator
```

Wichtig: Im LXC **nicht** `bluestation-bs` bauen, sonst werden unnötige Funk-/Codec-Abhängigkeiten gezogen.

## Config installieren

```bash
install -d -o root -g root /etc/netcore-control-room
install -m 0644 system-backend/control-room/config/control-room.example.toml /etc/netcore-control-room/control-room.toml
install -m 0600 system-backend/control-room/systemd/netcore-control-room.env.example /etc/netcore-control-room/control-room.env
```

Tokens erzeugen und eintragen:

```bash
NODE_TOKEN=$(openssl rand -hex 32)
OPERATOR_TOKEN=$(openssl rand -hex 32)

sed -i "s/^NETCORE_CONTROL_ROOM_NODE_TOKEN=.*/NETCORE_CONTROL_ROOM_NODE_TOKEN=$NODE_TOKEN/" /etc/netcore-control-room/control-room.env
sed -i "s/^NETCORE_CONTROL_ROOM_OPERATOR_TOKEN=.*/NETCORE_CONTROL_ROOM_OPERATOR_TOKEN=$OPERATOR_TOKEN/" /etc/netcore-control-room/control-room.env

cat /etc/netcore-control-room/control-room.env
```

Auth einschalten, sobald die TBS den Node-Token kennt:

```bash
sed -i 's/^enabled = false/enabled = true/' /etc/netcore-control-room/control-room.toml
```

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

## Tests ohne Auth

```bash
curl http://127.0.0.1:9010/health | jq
curl http://127.0.0.1:9010/api/overview | jq
```

## Tests mit Auth

```bash
source /etc/netcore-control-room/control-room.env

curl http://127.0.0.1:9010/health | jq
curl -H "Authorization: Bearer $NETCORE_CONTROL_ROOM_OPERATOR_TOKEN"   http://127.0.0.1:9010/api/overview | jq

./target/release/netcore-control-room-operator   --api http://10.0.1.25:9010   --token "$NETCORE_CONTROL_ROOM_OPERATOR_TOKEN"   overview
```

Oder dauerhaft für den Operator:

```bash
export NETCORE_CONTROL_ROOM_OPERATOR_TOKEN="..."
./target/release/netcore-control-room-operator --api http://10.0.1.25:9010 dashboard
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

## TBS-Config mit Auth

Auf der TBS muss der Control Room auf den LXC zeigen und den Node-Token mitsenden.

```toml
[control_room]
enabled = true
host = "10.0.1.25"
port = 9010
use_tls = false
endpoint_path = "/node"

node_id = "tbs-04010001"
station_name = "NetCore TBS 04010001"
site = "Main"

# Gleicher Wert wie NETCORE_CONTROL_ROOM_NODE_TOKEN im LXC.
token = "<node-token>"
```

Danach TBS neu starten.

## RBAC-Token verwalten

Der Bootstrap-Operator-Token aus `/etc/netcore-control-room/control-room.env` hat standardmäßig Admin-Rechte. Damit erzeugst du benannte Tokens:

```bash
source /etc/netcore-control-room/control-room.env

./target/release/netcore-control-room-operator \
  --api http://10.0.1.25:9010 \
  --token "$NETCORE_CONTROL_ROOM_OPERATOR_TOKEN" \
  tokens create --label "Jan Admin" --role admin --created-by jan
```

Die Ausgabe enthält den Klartext-Token genau einmal. Danach liegt nur noch der Hash in SQLite.

Weitere Beispiele:

```bash
./target/release/netcore-control-room-operator --api http://10.0.1.25:9010 --token "$NETCORE_CONTROL_ROOM_OPERATOR_TOKEN" tokens create --label "ELW Display" --role viewer --created-by jan
./target/release/netcore-control-room-operator --api http://10.0.1.25:9010 --token "$NETCORE_CONTROL_ROOM_OPERATOR_TOKEN" tokens create --label "Operator Jan" --role operator --created-by jan
./target/release/netcore-control-room-operator --api http://10.0.1.25:9010 --token "$NETCORE_CONTROL_ROOM_OPERATOR_TOKEN" tokens list
```

Deaktivieren/löschen:

```bash
./target/release/netcore-control-room-operator --api http://10.0.1.25:9010 --token "$NETCORE_CONTROL_ROOM_OPERATOR_TOKEN" tokens disable --id tok_...
./target/release/netcore-control-room-operator --api http://10.0.1.25:9010 --token "$NETCORE_CONTROL_ROOM_OPERATOR_TOKEN" tokens delete --id tok_...
```

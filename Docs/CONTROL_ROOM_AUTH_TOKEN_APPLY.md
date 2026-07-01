# NetCore Control Room Auth/API-Token Patch

Dieser Patch ergänzt Token-Auth für:

- Basisstation / TBS am WebSocket `/node`
- Operator/API-Clients über HTTP API
- optionale `/health`-Freigabe für Monitoring

## Einbau per ZIP

Im Repo-Root:

```bash
cd /opt/netcore/flowstation
unzip -o /pfad/zu/netcore-control-room-auth-token-files.zip
```

Oder per Patch:

```bash
cd /opt/netcore/flowstation
git apply /pfad/zu/netcore-control-room-auth-token.git.patch
```

## Build im Control-Room-LXC

```bash
cargo build --release   -p netcore-control-room   -p netcore-control-room-operator
```

## Secret-Datei installieren

```bash
install -d -o root -g root /etc/netcore-control-room
install -m 0644 system-backend/control-room/config/control-room.example.toml /etc/netcore-control-room/control-room.toml
install -m 0600 system-backend/control-room/systemd/netcore-control-room.env.example /etc/netcore-control-room/control-room.env
```

Token erzeugen:

```bash
NODE_TOKEN=$(openssl rand -hex 32)
OPERATOR_TOKEN=$(openssl rand -hex 32)

sed -i "s/^NETCORE_CONTROL_ROOM_NODE_TOKEN=.*/NETCORE_CONTROL_ROOM_NODE_TOKEN=$NODE_TOKEN/" /etc/netcore-control-room/control-room.env
sed -i "s/^NETCORE_CONTROL_ROOM_OPERATOR_TOKEN=.*/NETCORE_CONTROL_ROOM_OPERATOR_TOKEN=$OPERATOR_TOKEN/" /etc/netcore-control-room/control-room.env
```

Auth in `/etc/netcore-control-room/control-room.toml` aktivieren:

```toml
[auth]
enabled = true
allow_health_unauthenticated = true
node_token_env = "NETCORE_CONTROL_ROOM_NODE_TOKEN"
operator_token_env = "NETCORE_CONTROL_ROOM_OPERATOR_TOKEN"
```

systemd-Service aktualisieren und neustarten:

```bash
install -m 0644 system-backend/control-room/systemd/netcore-control-room.service /etc/systemd/system/netcore-control-room.service
systemctl daemon-reload
systemctl restart netcore-control-room
journalctl -u netcore-control-room -f
```

## TBS-Config

Auf der TBS in `[control_room]` denselben Node-Token setzen:

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

token = "<NETCORE_CONTROL_ROOM_NODE_TOKEN>"
```

TBS danach neu starten.

## Operator

```bash
source /etc/netcore-control-room/control-room.env

./target/release/netcore-control-room-operator   --api http://10.0.1.25:9010   --token "$NETCORE_CONTROL_ROOM_OPERATOR_TOKEN"   dashboard
```

Oder dauerhaft:

```bash
export NETCORE_CONTROL_ROOM_OPERATOR_TOKEN=<operator-token>
./target/release/netcore-control-room-operator --api http://10.0.1.25:9010 dashboard
```

## Tests

```bash
source /etc/netcore-control-room/control-room.env

curl http://127.0.0.1:9010/health | jq

curl -H "Authorization: Bearer $NETCORE_CONTROL_ROOM_OPERATOR_TOKEN"   http://127.0.0.1:9010/api/overview | jq

curl http://127.0.0.1:9010/api/overview
# Erwartung bei auth.enabled=true: unauthorized
```

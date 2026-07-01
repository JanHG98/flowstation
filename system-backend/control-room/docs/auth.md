# Control Room Auth / API-Token

Der Control Room unterstützt einfache Token-Authentifizierung für zwei Rollen:

```text
Node      = Basisstation / TBS am WebSocket /node
Operator  = native Leitstellen-Konsole und HTTP API
```

## LXC

Token erzeugen:

```bash
openssl rand -hex 32
openssl rand -hex 32
```

In `/etc/netcore-control-room/control-room.env` eintragen:

```bash
NETCORE_CONTROL_ROOM_NODE_TOKEN=<node-token>
NETCORE_CONTROL_ROOM_OPERATOR_TOKEN=<operator-token>
```

In `/etc/netcore-control-room/control-room.toml` aktivieren:

```toml
[auth]
enabled = true
allow_health_unauthenticated = true
node_token_env = "NETCORE_CONTROL_ROOM_NODE_TOKEN"
operator_token_env = "NETCORE_CONTROL_ROOM_OPERATOR_TOKEN"
```

Service neu starten:

```bash
systemctl restart netcore-control-room
journalctl -u netcore-control-room -f
```

## TBS

In der TBS-Config:

```toml
[control_room]
enabled = true
host = "10.0.1.25"
port = 9010
endpoint_path = "/node"
token = "<node-token>"
```

Die TBS sendet den Token als Basic-Auth-Passwort mit Username `node`. Der Server prüft den Token.

## Operator

Einmalig:

```bash
./target/release/netcore-control-room-operator   --api http://10.0.1.25:9010   --token <operator-token>   overview
```

Dauerhaft:

```bash
export NETCORE_CONTROL_ROOM_OPERATOR_TOKEN=<operator-token>
./target/release/netcore-control-room-operator --api http://10.0.1.25:9010 dashboard
```

## Curl

```bash
curl -H "Authorization: Bearer <operator-token>" http://10.0.1.25:9010/api/overview | jq
```

`/health` kann bewusst offen bleiben, wenn `allow_health_unauthenticated = true` gesetzt ist.

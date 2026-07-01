# Control Room Auth, RBAC und Token-Verwaltung

Der Control Room unterstützt Token-Authentifizierung mit Rollenmodell.

## Rollen

```text
node      Basisstation/TBS am WebSocket /node
viewer    nur lesende API/Operator-Dashboard
operator  viewer + Funkbedienung: Kick, DGNA, Emergency Clear, SDS/Commands
admin     operator + gefährliche Commands + Tokenverwaltung
```

Rollen sind hierarchisch, außer `node`:

```text
admin > operator > viewer
node ist nur für Basisstationen
```

## Geschützte Bereiche

```text
WS /node                                      node
WS /ui                                        viewer/operator/admin
GET /api/*                                    viewer
POST /api/nodes/{node}/commands/kick          operator
POST /api/nodes/{node}/commands/dgna          operator
POST /api/nodes/{node}/commands/clear-emergency operator
POST /api/nodes/{node}/commands/restart-service admin
POST /api/nodes/{node}/commands/shutdown-service admin
/api/admin/tokens/*                           admin
```

`/health` kann über `allow_health_unauthenticated = true` offen bleiben.

## Bootstrap-Tokens

Für den Start gibt es weiterhin zwei Bootstrap-Tokens in:

```text
/etc/netcore-control-room/control-room.env
```

```bash
NETCORE_CONTROL_ROOM_NODE_TOKEN=<node-token>
NETCORE_CONTROL_ROOM_OPERATOR_TOKEN=<operator-token>
```

Der Operator-Token hat per Default Admin-Rechte:

```toml
[auth]
operator_token_role = "admin"
```

Damit kannst du später echte, benannte Tokens erzeugen.

## Token-Registry

Registry-Tokens werden in SQLite gespeichert:

```text
/var/lib/netcore-control-room/control-room.sqlite3
auth_tokens
```

Der Klartext-Token wird **nur beim Erstellen einmal angezeigt**. In SQLite liegt nur ein SHA-256-Hash.

## Operator CLI

Tokenliste anzeigen:

```bash
./target/release/netcore-control-room-operator \
  --api http://10.0.1.25:9010 \
  --token "$NETCORE_CONTROL_ROOM_OPERATOR_TOKEN" \
  tokens list
```

Viewer-Token erstellen:

```bash
./target/release/netcore-control-room-operator \
  --api http://10.0.1.25:9010 \
  --token "$NETCORE_CONTROL_ROOM_OPERATOR_TOKEN" \
  tokens create --label "ELW Display" --role viewer --created-by jan
```

Operator-Token erstellen:

```bash
./target/release/netcore-control-room-operator \
  --api http://10.0.1.25:9010 \
  --token "$NETCORE_CONTROL_ROOM_OPERATOR_TOKEN" \
  tokens create --label "Jan Operator" --role operator --created-by jan
```

Admin-Token erstellen:

```bash
./target/release/netcore-control-room-operator \
  --api http://10.0.1.25:9010 \
  --token "$NETCORE_CONTROL_ROOM_OPERATOR_TOKEN" \
  tokens create --label "Jan Admin" --role admin --created-by jan
```

Node-Token für weitere TBS erstellen:

```bash
./target/release/netcore-control-room-operator \
  --api http://10.0.1.25:9010 \
  --token "$NETCORE_CONTROL_ROOM_OPERATOR_TOKEN" \
  tokens create --label "TBS Event" --role node --created-by jan
```

Token deaktivieren:

```bash
./target/release/netcore-control-room-operator \
  --api http://10.0.1.25:9010 \
  --token "$NETCORE_CONTROL_ROOM_OPERATOR_TOKEN" \
  tokens disable --id tok_...
```

Token wieder aktivieren:

```bash
./target/release/netcore-control-room-operator \
  --api http://10.0.1.25:9010 \
  --token "$NETCORE_CONTROL_ROOM_OPERATOR_TOKEN" \
  tokens enable --id tok_...
```

Token löschen:

```bash
./target/release/netcore-control-room-operator \
  --api http://10.0.1.25:9010 \
  --token "$NETCORE_CONTROL_ROOM_OPERATOR_TOKEN" \
  tokens delete --id tok_...
```

## HTTP/curl

```bash
curl -H "Authorization: Bearer <token>" http://10.0.1.25:9010/api/overview | jq
```

Token erstellen:

```bash
curl -X POST http://10.0.1.25:9010/api/admin/tokens \
  -H "Authorization: Bearer $NETCORE_CONTROL_ROOM_OPERATOR_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"label":"ELW Display","role":"viewer","created_by":"jan"}' | jq
```

## Sicherheitshinweis

Die Bootstrap-Tokens sind praktisch für den Start. Für den Betrieb ist sauberer:

1. Bootstrap-Admin nutzen.
2. Benannte Registry-Tokens erstellen.
3. Clients auf Registry-Tokens umstellen.
4. Bootstrap-Operator-Token lang und geheim halten oder später aus der Config entfernen.

# NetCore Control Room RBAC / Tokenverwaltung einbauen

Dieses ZIP enthält vollständige Dateien zum Überschreiben. Es enthält bewusst keine Patch-Dateien.

## 0. Voraussetzung

Aktueller Stand:

- Control Room Core läuft im LXC
- SQLite-Persistenz ist aktiv
- TBS meldet sich bereits mit Node-Token an
- Operator-Token funktioniert als Bootstrap-Admin

## 1. ZIP entpacken

Im Repo-Root:

```bash
cd /opt/netcore/flowstation
unzip -o /pfad/zu/netcore-control-room-rbac-token-registry-files.zip
```

## 2. Bauen

Im LXC nur Control Room und Operator bauen:

```bash
cargo build --release \
  -p netcore-control-room \
  -p netcore-control-room-operator
```

Hinweis: Durch `sha2` kann Cargo beim ersten Build neue Crates laden und `Cargo.lock` aktualisieren.

## 3. Config aktualisieren

Wenn du deine bestehende Config behalten willst, reicht normalerweise diese Ergänzung in `/etc/netcore-control-room/control-room.toml`:

```toml
[auth]
enabled = true
allow_health_unauthenticated = true
node_token_env = "NETCORE_CONTROL_ROOM_NODE_TOKEN"
operator_token_env = "NETCORE_CONTROL_ROOM_OPERATOR_TOKEN"
operator_token_role = "admin"
```

Oder Beispiel-Config neu kopieren:

```bash
install -m 0644 system-backend/control-room/config/control-room.example.toml /etc/netcore-control-room/control-room.toml
```

Dann `auth.enabled = true` setzen, sobald TBS und Operator-Token passen.

## 4. Service neu starten

```bash
systemctl daemon-reload
systemctl restart netcore-control-room
journalctl -u netcore-control-room -f
```

Du solltest weiterhin sehen:

```text
SQLite persistence enabled
NetCore Control Room listening bind=0.0.0.0:9010
websocket connected ... path=/node
```

## 5. DB-Migration prüfen

```bash
sqlite3 /var/lib/netcore-control-room/control-room.sqlite3 '.tables'
```

Neu muss dabei sein:

```text
auth_tokens
```

Optional:

```bash
sqlite3 /var/lib/netcore-control-room/control-room.sqlite3 \
  'select version,applied_at from schema_migrations order by version;'
```

## 6. Bootstrap-Admin testen

```bash
source /etc/netcore-control-room/control-room.env

./target/release/netcore-control-room-operator \
  --api http://10.0.1.25:9010 \
  --token "$NETCORE_CONTROL_ROOM_OPERATOR_TOKEN" \
  tokens list
```

Erwartung am Anfang:

```json
{
  "count": 0,
  "tokens": []
}
```

## 7. Ersten Admin-Registry-Token erstellen

```bash
./target/release/netcore-control-room-operator \
  --api http://10.0.1.25:9010 \
  --token "$NETCORE_CONTROL_ROOM_OPERATOR_TOKEN" \
  tokens create --label "Jan Admin" --role admin --created-by jan
```

Die Ausgabe enthält `token`. Diesen Token sofort kopieren und sicher ablegen. Er wird später nie wieder im Klartext angezeigt.

Danach testen:

```bash
export NETCORE_CONTROL_ROOM_ADMIN_TOKEN='<der neue token>'

./target/release/netcore-control-room-operator \
  --api http://10.0.1.25:9010 \
  --token "$NETCORE_CONTROL_ROOM_ADMIN_TOKEN" \
  tokens list
```

## 8. Weitere Rollen-Tokens erstellen

Viewer, z. B. für Display/Monitoring:

```bash
./target/release/netcore-control-room-operator \
  --api http://10.0.1.25:9010 \
  --token "$NETCORE_CONTROL_ROOM_ADMIN_TOKEN" \
  tokens create --label "ELW Display" --role viewer --created-by jan
```

Operator:

```bash
./target/release/netcore-control-room-operator \
  --api http://10.0.1.25:9010 \
  --token "$NETCORE_CONTROL_ROOM_ADMIN_TOKEN" \
  tokens create --label "Jan Operator" --role operator --created-by jan
```

Weitere TBS/Node:

```bash
./target/release/netcore-control-room-operator \
  --api http://10.0.1.25:9010 \
  --token "$NETCORE_CONTROL_ROOM_ADMIN_TOKEN" \
  tokens create --label "TBS Event" --role node --created-by jan
```

## 9. Token deaktivieren / aktivieren / löschen

```bash
./target/release/netcore-control-room-operator \
  --api http://10.0.1.25:9010 \
  --token "$NETCORE_CONTROL_ROOM_ADMIN_TOKEN" \
  tokens disable --id tok_...

./target/release/netcore-control-room-operator \
  --api http://10.0.1.25:9010 \
  --token "$NETCORE_CONTROL_ROOM_ADMIN_TOKEN" \
  tokens enable --id tok_...

./target/release/netcore-control-room-operator \
  --api http://10.0.1.25:9010 \
  --token "$NETCORE_CONTROL_ROOM_ADMIN_TOKEN" \
  tokens delete --id tok_...
```

## 10. RBAC-Schnelltest

Viewer darf lesen:

```bash
./target/release/netcore-control-room-operator \
  --api http://10.0.1.25:9010 \
  --token '<viewer-token>' \
  overview
```

Viewer darf keine Commands:

```bash
./target/release/netcore-control-room-operator \
  --api http://10.0.1.25:9010 \
  --token '<viewer-token>' \
  kick --node SRV-M_TBS-01 --issi 2010002 --operator jan
```

Erwartung: `403 Forbidden`.

Operator darf normale Commands, aber keine Tokenverwaltung.

Admin darf alles.

## Rollen

```text
node      nur /node WebSocket für TBS
viewer    Lesen/Dashboard
operator  Lesen + normale Funkbefehle
admin     Alles + Tokenverwaltung + Service-Restart/Shutdown
```


## Buildfix-Hinweis

Dieser ZIP-Stand entfernt Debug-Derives an AuthState/PersistenceHandle, damit der Build nicht versucht, rusqlite-Interna als Debug auszugeben. Bitte diesen ZIP-Stand komplett entpacken und den vorherigen RBAC-ZIP ignorieren.

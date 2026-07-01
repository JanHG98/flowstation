# NetCore Control Room

Dieser Bereich enthält die Control-Room-/Leitstellen-Bausteine.

## Komponenten

```text
bins/netcore-control-room                 Control-Room-Core als Dienst
system-backend/control-room/operator      native Operator-Konsole
system-backend/control-room/config        Beispiel-Config
system-backend/control-room/systemd       systemd-Unit/env-Beispiele
system-backend/control-room/schema        SQLite-Schema-Doku
system-backend/control-room/docs          Deployment/Auth/RBAC-Doku
```

## Architektur

```text
TBS / FlowStation  →  /node WebSocket  →  Control-Room-Core im LXC
Operator-Konsole   →  HTTP/API         →  Control-Room-Core im LXC
```

Die Operator-Konsole ist bewusst **keine Web-App**, sondern ein eigenständig lauffähiges Programm.

## RBAC

Rollen:

```text
node      Basisstation/TBS
viewer    Lesen/Dashboard
operator  Funkbedienung
admin     Administration/Tokenverwaltung
```

Details: `docs/auth.md`

## Build im LXC

```bash
git fetch && \
git checkout control-room && \
git pull --ff-only && \
cargo build --release \
  -p netcore-control-room \
  -p netcore-control-room-operator
```

## Start Core

```bash
./target/release/netcore-control-room \
  --config /etc/netcore-control-room/control-room.toml
```

## Start Operator

```bash
./target/release/netcore-control-room-operator \
  --api http://10.0.1.25:9010 \
  --token "$NETCORE_CONTROL_ROOM_OPERATOR_TOKEN" \
  dashboard
```

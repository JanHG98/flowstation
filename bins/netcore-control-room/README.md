# NetCore Control Room Core

Minimaler Rust-Core für die spätere NetCore-Tetra Leitstelle.

Der Server nimmt Base-Station-Nodes über WebSocket an, baut daraus einen In-Memory-State und stellt diesen per HTTP-JSON sowie UI-WebSocket bereit.

## Start

```bash
cargo run --release -p netcore-control-room -- --bind 127.0.0.1:9010
```

Für einen lokalen Test reicht `127.0.0.1:9010`. Wenn die Basisstation auf einem anderen Host läuft, kannst du z. B. binden auf:

```bash
cargo run --release -p netcore-control-room -- --bind 0.0.0.0:9010
```

Dann aber bitte nicht offen ins Internet hängen. Aktuell ist das ein Core-MVP ohne Authentifizierung.

## Basisstation-Config

```toml
[control_room]
enabled = true
host = "127.0.0.1"
port = 9010
use_tls = false
endpoint_path = "/node"

node_id = "tbs-04010001"
station_name = "NetCore TBS 04010001"
site = "Lab / Rack"
```

## Endpunkte

| Endpoint | Zweck |
|---|---|
| `WS /node` | Base-Station-Verbindung |
| `WS /ui` | Live-Feed für spätere Leitstellen-UI |
| `GET /health` | einfacher Healthcheck |
| `GET /api/overview` | schlanker Leitstellenstatus für UI/Dispo |
| `GET /api/state` | kompletter Debug-State, bewusst groß |
| `GET /api/rf` | RF-/SDR-Snapshot separat |
| `GET /api/health/full` | technische Health-Daten separat |
| `GET /api/nodes` | nur Nodes mit vollem Node-State |
| `GET /api/events?limit=50&quiet=true` | letzte Events, optional ohne RF-/Health-Rauschen |
| `GET /api/events?type=sds_log&limit=20` | Events nach Typ filtern |
| `GET /api/commands?limit=50` | Command-Audit |
| `POST /api/nodes/{node_id}/commands` | generisches Command an Node senden |
| `POST /api/nodes/{node_id}/commands/kick` | Shortcut für `KickMs` |
| `POST /api/nodes/{node_id}/commands/dgna` | Shortcut für DGNA Attach/Detach |
| `POST /api/nodes/{node_id}/commands/clear-emergency` | Shortcut für Emergency Clear |
| `POST /api/commands` | vollständigen `ControlCommandEnvelope` senden |

## Command-Beispiele

### Kick MS

```bash
curl -X POST http://127.0.0.1:9010/api/nodes/tbs-04010001/commands/kick \
  -H 'Content-Type: application/json' \
  -d '{"operator_id":"jan","issi":2010001}'
```

### Clear Emergency

```bash
curl -X POST http://127.0.0.1:9010/api/nodes/tbs-04010001/commands/clear-emergency \
  -H 'Content-Type: application/json' \
  -d '{"operator_id":"jan","issi":0}'
```

### SDS senden

Die SDS-Payload ist aktuell bewusst roh, passend zum vorhandenen `ControlCommand::SendSds`:

```bash
curl -X POST http://127.0.0.1:9010/api/nodes/tbs-04010001/commands \
  -H 'Content-Type: application/json' \
  -d '{
    "operator_id":"jan",
    "command":{
      "SendSds":{
        "handle":1001,
        "source_ssi":9999,
        "dest_ssi":2010001,
        "dest_is_group":false,
        "len_bits":16,
        "payload":[1,65]
      }
    }
  }'
```

## Was dieser MVP schon macht

- Node-Hello annehmen und bestätigen
- Node-Heartbeat verarbeiten
- Telemetry-Envelopes in State überführen
- Teilnehmer, Gruppen, Calls, SDS, Notrufe, Health grob modellieren
- Commands per HTTP entgegennehmen und an die BS weiterleiten
- Shortcut-Routen für Kick, DGNA und Emergency-Clear bereitstellen
- Acks/Responses auditieren
- schlanken `/api/overview` für die spätere Leitstellenoberfläche bereitstellen
- RF/Health/Debug-State getrennt ausgeben, damit die UI nicht mit Telemetry-Lawinen zugemüllt wird
- UI-WebSocket für spätere Leitstellenoberfläche bereitstellen

## Bewusste Grenzen dieses ersten Core-MVP

- keine Authentifizierung
- keine Persistenz
- noch keine echte UI
- noch kein Multi-Operator-Rechtemodell
- noch kein BREW-Routing zwischen mehreren Sites

Das ist Absicht: erst den stabilen Kern bauen, dann UI/Auth/Persistenz sauber daraufsetzen.

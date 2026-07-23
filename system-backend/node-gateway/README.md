# Node Gateway

## Status

Erster tatsächlich deploybarer NetCore-System-Backend-Dienst. Der Dienst ist für einen eigenen Proxmox-LXC vorbereitet und enthält eine integrierte Verwaltungs-WebUI.

## Zweck

Der Node Gateway ist der zentrale Einstiegspunkt für NetCore-TBS-Instanzen. Er nimmt die bestehenden TBS-WebSocket-Verbindungen an, verwaltet Sessions und stellt einen normalisierten Transport für spätere Backend-Dienste bereit.

## Umgesetzt

- kompatibler TBS-WebSocket unter `/ws/node`
- Aushandlung von `netcore-control-room-node-v1`
- Hello-, Heartbeat-, Telemetrie-, ACK-, Response- und Error-Verarbeitung
- Duplicate-Node-Erkennung mit kontrolliertem Austausch der alten Session
- Hello-Timeout und Nachrichtengrößenlimits
- In-Memory-Node- und Ereigniszustand
- Kommandotransport vom API-/Backend-Pfad zur TBS
- Backend-WebSocket unter `/ws/backend`
- REST-API unter `/api/v1`
- Prometheus-Metriken
- OpenAPI-Beschreibung
- eigene integrierte WebUI
- systemd-Unit und Installationsskripte

## Offener Testmodus

Diese Version verwendet ausdrücklich **keine Tokens**. Ebenso gibt es noch keine Benutzer, Passwörter, Zertifikate oder TLS-Verschlüsselung.

```toml
[security]
mode = "open_lab"
allow_remote_management = true
```

Andere Security-Modi werden abgewiesen, statt nicht vorhandene Sicherheit vorzutäuschen. Der LXC darf daher ausschließlich in einem isolierten Testnetz betrieben werden.

## Endpunkte

| Endpunkt | Funktion |
|---|---|
| `GET /` | Verwaltungs-WebUI |
| `WS /ws/node` | TBS-Verbindungen |
| `WS /ws/backend` | späterer Mobility-/Call-/SDS-Backend-Transport |
| `GET /api/v1/status` | Gateway-Übersicht |
| `GET /api/v1/nodes` | Nodes |
| `GET /api/v1/nodes/{id}` | Node-Details |
| `POST /api/v1/nodes/{id}/ping` | Application Ping |
| `POST /api/v1/nodes/{id}/disconnect` | Node trennen |
| `POST /api/v1/nodes/{id}/commands` | typisiertes TBS-Kommando |
| `GET /api/v1/events` | Ereignis-History |
| `GET /metrics` | Prometheus |
| `GET /openapi.json` | API-Beschreibung |
| `GET /health/live` | Liveness |
| `GET /health/ready` | Readiness |

## WebUI

Die WebUI zeigt:

- verbundene, getrennte und stale Nodes
- Stations-, Zell- und Carrierdaten
- Stackversion und Capabilities
- Heartbeat-, Telemetrie- und Response-Zähler
- letzte Gateway-Ereignisse
- Ping- und Disconnect-Aktionen
- gut sichtbare Warnung zum offenen Testmodus

## Build

```bash
cargo build --release -p netcore-node-gateway
```

## Start im Repo

```bash
target/release/netcore-node-gateway \
  --config system-backend/node-gateway/config/node-gateway.example.toml
```

## Grenzen dieses Pakets

- keine persistente Datenbank
- keine fachliche Teilnehmer-, Gruppen-, Mobility- oder Ruflogik
- kein Media-Transport
- noch keine abgesicherte Produktivbetriebsart
- der bestehende TBS-Node-Protocol-Datentyp wird zunächst wiederverwendet; die spätere Versionierung erfolgt unter `system-backend/shared/edge-protocol`

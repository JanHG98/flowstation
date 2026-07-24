# NetCore Control Room

## Zweck

Der Control Room ist die zentrale Leitstellen-, Bedien- und Lageebene für eine NetCore-Tetra-Region. Er führt die Zustände der Fachsysteme zusammen, bleibt aber ausdrücklich **nicht** Eigentümer von Teilnehmer-, Gruppen-, Mobility-, Call-, SDS-, Packet- oder Schlüsselzuständen.

## Umsetzung dieser Phase

- Browser-WebUI auf Port `9010`
- bestehende TBS- und Operator-WebSockets `/node` und `/ui`
- Lageübersicht für TBS, Teilnehmer, Gruppen, aktive Rufe und Notfälle
- Health-/Readiness-Polling aller Core-, Edge-, Media-, Data-, Security-, Transit-, Observability- und Application-Dienste
- Abruf der jeweiligen `/api/v1/status`-Zusammenfassung
- kuratiertes federiertes Kernlagebild mit bevorzugten Kennzahlen aus den autoritativen Fachkernen
- direkte Links zu den eigenständigen Service-WebUIs
- Browser-Schnellaktionen für Kick, Clear Emergency und DGNA über die bestehenden typisierten TBS-Kommandos
- automatische Störungsfälle nach konfigurierbarer Fehlerfolge
- manuelles Incident-Journal mit Ack, Lösung und Notizen
- persistentes Schichtbuch
- Prometheus-Metriken, OpenAPI, Export, Health und Readiness
- SQLite-Persistenz für bestehendes Event-/Command-Audit
- JSON-Persistenz mit Backup für Service-Lage, Incidents und Schichtbuch
- systemd- und LXC-Installationsskripte

## Architekturgrenze

Der Control Room ist eine **Presentation und Operator Plane**. Er spiegelt Zustände und stellt Bedienwege bereit, erzeugt aber keine parallele Wahrheit neben den Fachkernen. Ein generischer Schreib-Proxy zu beliebigen Backend-Endpunkten ist bewusst nicht enthalten. Für Fachverwaltung bleibt jede Dienst-WebUI unabhängig erreichbar.

Der bisherige direkte TBS-WebSocket bleibt als Kompatibilitäts- und Übergangspfad erhalten. Mit zunehmender Core-Autorität sollen Lagebilder bevorzugt aus den Fachdiensten kommen.

## Open-Lab-Modus

Aktuell absichtlich:

- keine Benutzeranmeldung,
- keine Tokens,
- kein Node-Token,
- kein TLS,
- kein mTLS zwischen den LXCs.

Damit besitzt jeder Client im erreichbaren Netz Operatorrechte. Nur im isolierten Labor- und Managementnetz betreiben.

## Start

```bash
cargo build --locked --release --package netcore-control-room
./target/release/netcore-control-room \
  --config system-backend/control-room/config/control-room.example.toml \
  --no-auth
```

WebUI:

```text
http://<control-room-lxc>:9010/
```

## Wichtige API-Endpunkte

```text
GET  /health/live
GET  /health/ready
GET  /metrics
GET  /api/v1/control-room/overview   # inkl. federated.domains und preferred_counts
GET  /api/v1/services
POST /api/v1/services/poll
GET  /api/v1/incidents
POST /api/v1/incidents
POST /api/v1/incidents/{id}/ack
POST /api/v1/incidents/{id}/resolve
POST /api/v1/incidents/{id}/notes
GET  /api/v1/shift-log
POST /api/v1/shift-log
GET  /api/v1/dependencies
GET  /api/v1/config
GET  /api/v1/export
GET  /api/v1/openapi.json
```

Die bisherigen `/api/*`- und WebSocket-Endpunkte bleiben kompatibel.

## Bewusste Grenzen

- keine produktive Authentisierung oder RBAC-Aktivierung in dieser Phase,
- kein beliebiger HTTP-Schreibproxy zu Fachdiensten,
- keine Audio-Konsole im Browser; der bestehende native Operator-Client bleibt erhalten,
- keine lokale Prometheus-/Grafana-/Loki-Installation; diese bleibt im separaten Observability-LXC,
- kein HA-Cluster oder gemeinsamer Operator-Session-State,
- kein zertifiziertes Leitstellenprodukt.

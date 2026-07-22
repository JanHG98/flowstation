# Transit

## Zweck

Transit ist die DXTT-ähnliche Vermittlung zwischen mehreren NetCore-Core-Regionen.

## Kernaufgaben

- DXT-zu-DXT-Routing
- Gruppenruf-, Einzelruf-, SDS- und Media-Transit
- Routingtabellen, Loop Prevention und Regional Failover
- Spätere ETSI-ISI-Anbindung

## Aktivierung

Produktiv erforderlich, sobald mindestens zwei eigenständige Core-Regionen existieren.

## WebUI zur Verwaltung

Transit erhält eine eigene Verwaltungsoberfläche für Regionen, Peers und überregionale Routen.

### Geplante Ansichten

- verbundene Core-Regionen und ISI-Peers
- Routingtabellen und Pfadpräferenzen
- transitive Calls, SDS und Media Sessions
- Redundanz, Failover und Loop Detection
- Linkqualität, Latenz und Protokollversionen
- Wartungs- und Störungszustände

### Kritische Aktionen

- Peer aktivieren, sperren oder in Wartung setzen
- Routingpräferenz ändern
- kontrollierten Failover auslösen
- fehlerhafte Transit-Session bereinigen

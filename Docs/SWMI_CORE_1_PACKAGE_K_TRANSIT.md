# SWMI Core 1 – Package K: Transit

## Ziel

Dieses Paket ergänzt nach Security Core und KMF die regionale Vermittlung zwischen eigenständigen NetCore-Tetra-Core-Regionen.

## Implementierte Runtime

```text
system-backend/transit/
```

Der Dienst läuft standardmäßig auf Port `8200` und liefert WebUI, REST-API, Liveness, Readiness, Metrics, OpenAPI, systemd-Unit und Installationsskripte.

## Regionale Funktionen

- Peer- und Regionenverwaltung,
- Teilnehmerregion für ISSI,
- Gruppenreichweite für GSSI,
- Routen nach Service, Region, ISSI, GSSI, Präfix und Default,
- Individual-/Gruppenruf-, SDS-, Media-, Mobility- und Supplementary-Service-Transit,
- Sessions mit Legs pro Zielregion,
- persistente Outbound- und Local-Delivery-Queues.

## Stabilität und Loop Prevention

- Path Vector mit regionalen Hops,
- konfigurierbares Hop Limit,
- Deduplizierung über `dedupe_key`,
- TTL und Ablauf,
- Peer-Heartbeat und Timeout,
- Retry mit Backoff,
- mehrere Kandidaten und Backup-Peers,
- automatischer und kontrollierter Failover.

## Betriebsmodi

`shadow` berechnet und dokumentiert Pfade, sendet aber keine Peer-Nachrichten. `authoritative` aktiviert HTTP-Heartbeat, Envelope-Transport, Retry und Failover.

## Kein ETSI-Etikettenschwindel

Das Protokoll `netcore-transit-v1` ist eine interne semantische Region-zu-Region-Schnittstelle. Es implementiert noch keine ETSI-ISI-Stage-3-PDUs, keine Fremd-SwMI-Interoperabilität und keine ISI-Sicherheitsprofile. Der spätere ISI-Adapter wird standardisierte Verfahren auf dieses interne Modell abbilden.

## Nicht als Produktion ausgeben

- keine Benutzeranmeldung, Tokens oder TLS,
- keine mTLS-Peeridentität,
- keine signierten Route Advertisements,
- kein RBAC,
- keine standardisierte ISI-Codierung,
- keine WAN-QoS-/Bandbreitenreservierung.

Diese Grenzen sind in WebUI, Logs, README und Konfiguration sichtbar markiert.

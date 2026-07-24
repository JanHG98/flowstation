# NetCore Transit

## Zweck

Transit ist die DXTT-ähnliche Vermittlung zwischen eigenständigen NetCore-Tetra-Core-Regionen. Der Dienst bestimmt Teilnehmer- und Gruppenregionen, wählt redundante Pfade und transportiert Mobility-, Einzelruf-, Gruppenruf-, SDS-, Media- und Supplementary-Service-Ereignisse zwischen Regionen.

> Diese Phase implementiert das NetCore-native Protokoll `netcore-transit-v1`. Es ist **noch kein ETSI ISI** und wird nicht als solches ausgegeben.

## Enthalten

- Regionen und Peer-Links mit Heartbeat, Latenz, Admin- und Betriebszustand
- statische Routen nach Dienst, Region, ISSI, GSSI, Präfix oder Default
- direkte und transitive Regionalpfade
- Teilnehmerregion über ISSI sowie Gruppenreichweite über GSSI
- Path Vector, Hop Limit und regionale Loop Prevention
- Deduplizierung über `dedupe_key`
- Sessions und regionale Legs für Calls, SDS und Media
- redundante Pfade, automatischer und kontrollierter Failover
- persistente Outbound- und Local-Delivery-Queues
- Retry, Backoff, TTL, Peer-Timeout und Recovery nach Neustart
- WebUI, REST-API, OpenAPI, Metrics, Health, Export und Backup
- systemd- und LXC-Installationsdateien

## WebUI

Standardport: `8200`

```text
http://<transit-lxc>:8200/
```

Die WebUI besitzt Ansichten für Übersicht, Regionen/Peers, Routing, Teilnehmer-/Gruppenregionen, Sessions, Traffic/Queues, Ereignisse, Wartung und API.

## Shadow und Authoritative

```toml
[region]
operating_mode = "shadow"
```

`shadow` berechnet Pfade, erzeugt Sessions und zeigt den vorgesehenen Transit, sendet jedoch keine Heartbeats oder Envelopes an andere Regionen.

```toml
[region]
operating_mode = "authoritative"
```

`authoritative` aktiviert den HTTP-Peer-Transport, Heartbeats, Retry, Deduplizierung und Failover.

## Peer-Protokoll

Peer-Ingress:

```text
POST /api/v1/peer/heartbeat
POST /api/v1/peer/envelopes
```

Lokale Core-Dienste:

```text
POST /api/v1/transit/submit
GET  /api/v1/local-deliveries
POST /api/v1/local-deliveries/{id}/ack
```

Ein Envelope trägt Ursprung, unmittelbaren vorherigen Hop, Zielregion, Service/Operation, Adressen, Session-/Korrelations-ID, Priorität, TTL, Path Vector und Payload.

## Loop Prevention

Ein Envelope wird verworfen, wenn:

- die lokale Region bereits im Path Vector vorkommt,
- `max_hops` erreicht ist,
- derselbe `dedupe_key` innerhalb des Dedupe-Fensters erneut eintrifft,
- kein gesunder Peer ohne Rückweg in den bisherigen Pfad existiert.

## Offene Testumgebung

Aktuell absichtlich:

- keine Anmeldung,
- keine Tokens,
- kein TLS,
- keine mTLS-Peeridentität,
- keine signierten Route Advertisements.

Der Dienst darf daher nur in einem isolierten Labor- und Managementnetz betrieben werden.

## Bewusste Grenzen

Noch nicht enthalten:

- ETSI-ISI-Protokollstacks und ANF-ISI-PDUs,
- Interoperabilität mit fremden SwMIs,
- produktive mTLS-/PKI-Peeridentität,
- RBAC und Freigabeworkflows,
- standardisierte ISI-Media- und Supplementary-Service-Profile,
- Bandbreitenreservierung und QoS-Signalisierung auf WAN-Ebene.

Diese Punkte folgen auf der NetCore-Transit-Basis als eigener ISI-/Interworking-Ausbau.

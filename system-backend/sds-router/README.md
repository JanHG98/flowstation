# NetCore SDS Router

Der SDS Router ist der zentrale SwMI-Dienst für **SDS und pre-coded Status**. Er nimmt verlustfrei normalisierte SDS-Edge-Ereignisse der TBS entgegen, entscheidet über Individual-, Gruppen- und Anwendungsziele und beauftragt die zuständigen TBS über den Node Gateway mit der Air-Interface-Zustellung.

## Aktueller Umfang

- Individual- und Gruppen-SDS, Typ 1 bis 4
- pre-coded Status
- exakte Erhaltung von SDS-Typ, Bitlänge und Payload
- Teilnehmer- und Gruppenpräsenz aus TBS-Telemetrie
- Store-and-forward bei nicht erreichbaren Teilnehmern oder TBS
- TTL, Priorität, Retry mit Backoff und maximale Versuchszahl
- Offline-, In-flight-, Partial-, Failed- und Dead-Letter-Zustände
- Duplikaterkennung mit konfigurierbarem Zeitfenster
- Protokoll-ID-Routen an externe Anwendungen
- optionale feste Individual-/Gruppenrouten zu einer TBS
- Application-Outbox mit explizitem ACK/NACK
- persistente JSON-Datenbank mit Backup
- REST-API, OpenAPI, Prometheus-Metriken, Health und eigene WebUI
- systemd-Unit sowie Install-, Update- und Uninstall-Skripte

## Bewusste Edge/Core-Grenze

Die TBS behält weiterhin:

- U-/D-SDS-PDU-Encoding und -Decoding,
- MCCH/FACCH-Ressourcenwahl,
- Energy-Economy- und Wake-up-Fenster,
- lokale sicherheitskritische Notrufbehandlung,
- lokale WX-/Command-ISSI-Sonderfunktionen,
- eine kurze Air-Interface-nahe Zustellqueue.

Der zentrale Router arbeitet oberhalb dieser Grenze mit einem verlustfreien SDS-Datensatz. Die TBS-Konfiguration aktiviert die Übergabe mit:

```toml
[control_room]
central_sds_routing = true
```

Solange der Wert `false` bleibt, arbeitet die bestehende lokale SDS-Logik weiter. Dadurch ist das Update rückwärtskompatibel und die Zentralisierung kann TBS für TBS eingeschaltet werden.

## Open-Lab-Betrieb

Der aktuelle Dienst läuft absichtlich ohne Benutzerkonten, Token und TLS. Jeder Client mit Netzzugriff kann Nachrichten und Nutzdaten lesen, Nachrichten senden, Zustellungen wiederholen und Routen verändern. Nur in einem isolierten Testnetz einsetzen.

Standardzugriff:

```text
http://<LXC-IP>:8150/
```

## Schnellstart im Repo

```bash
cargo run -p netcore-sds-router -- --no-config --bind 0.0.0.0:8150
```

Mit Konfiguration:

```bash
cp system-backend/sds-router/config/sds-router.example.toml /etc/netcore/sds-router.toml
cargo run -p netcore-sds-router -- --config /etc/netcore/sds-router.toml
```

## API-Auswahl

```text
GET    /api/v1/status
GET    /api/v1/messages
POST   /api/v1/messages
GET    /api/v1/messages/{id}
POST   /api/v1/messages/{id}/retry
POST   /api/v1/messages/{id}/requeue
POST   /api/v1/messages/{id}/cancel
DELETE /api/v1/messages/{id}
GET    /api/v1/routes
POST   /api/v1/routes
PUT    /api/v1/routes/{id}
DELETE /api/v1/routes/{id}
GET    /api/v1/application-outbox
POST   /api/v1/application-outbox/{application}/{id}/ack
GET    /api/v1/nodes
GET    /api/v1/subscribers
GET    /api/v1/groups
GET    /api/v1/events
GET    /metrics
GET    /health/live
GET    /health/ready
GET    /openapi.json
```

Weitere Details stehen unter `docs/`.

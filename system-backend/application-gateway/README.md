# NetCore-Tetra Application Gateway

## Zweck

Der Application Gateway kapselt externe Anwendungen, Webhooks und Automatisierungen von den fachlichen SwMI-Diensten ab. Er nimmt Ereignisse entgegen, rendert Vorlagen, wendet Routingregeln an und stellt Zustellungen mit Retry, TTL, Deduplizierung, Rate Limit und Circuit Breaker zu.

Der Dienst ist **kein** SDS Router, kein Media Switch und kein Air-Interface-Protokollstack. Er übersetzt ausschließlich zwischen stabilen NetCore-APIs und externen Anwendungen.

## Enthalten

- Connector Registry für interne und externe Adapter
- bidirektionale Webhook-Ingress-Endpunkte
- priorisierte Routingregeln nach Quelle, Ereignistyp und Textinhalt
- Text-, JSON- und TTS-Vorlagen
- persistente Event-, Delivery- und Dead-Letter-Queues
- Idempotency-/Dedupe-Fenster
- Retry mit exponentiellem Backoff und TTL
- Rate Limit und Circuit Breaker pro Connector
- Health-Probes und Connector-Zustände
- getrennte Secret-Datei mit redaktierten Management-Antworten
- TTS-Orchestrierung über Piper mit validiertem WAV-Spool
- Übergabevertrag zur späteren Media Library
- WebUI, REST-API, OpenAPI, Prometheus-Metriken, Audit, Backup und Export
- systemd- und LXC-Installationsskripte

## Connectoren

Die Standardkonfiguration enthält Adapter beziehungsweise Verträge für:

| Connector | Richtung | Status in diesem Paket |
|---|---|---|
| SDS Router | outbound | nativer NetCore-Aufruf |
| Piper TTS | outbound | nativer Synthese-Workflow |
| Media Library | outbound | Import-URL-Vertrag, bis LXC 15 standardmäßig deaktiviert |
| Telegram Bot | bidirectional | Bot-API outbound, Webhook-Ingress über Gateway |
| DAPNET | bidirectional | gekapselter HTTP-Relay-Vertrag |
| MeshCom | bidirectional | gekapselter HTTP-Relay-Vertrag |
| Snom Notify | outbound | XML-Notify |
| GeoAlarm | bidirectional | gekapselter HTTP-Vertrag |
| WX/METAR | bidirectional | HTTP-Abfrage plus routbarer Ingress |
| TPG2200 | outbound | SDS-Bridge über definierte Protocol-ID |
| Directory | bidirectional | generischer Directory-/Status-Vertrag |
| Generic Webhook | bidirectional | NetCore Event Envelope v1 |

Externe Protokolle werden bewusst hinter Adapterverträgen gehalten. Ein DAPNET- oder MeshCom-spezifischer Relay kann ausgetauscht werden, ohne SDS Router oder TBS zu verändern.

## WebUI

Standardport: `8220`

```text
http://<application-gateway-lxc>:8220/
```

Die WebUI zeigt Übersicht, Connectoren, Routing, Vorlagen, manuelle Dispatches, TTS-Jobs, Queues, Dead Letters, Audit und API-Links.

## Betriebsmodi

```toml
[runtime]
operating_mode = "shadow"
```

`shadow` nimmt Ereignisse an, routet sie und markiert fällige Zustellungen als unterdrückt. Es werden keine externen HTTP-Nebenwirkungen ausgelöst.

```toml
[runtime]
operating_mode = "authoritative"
```

`authoritative` aktiviert Connector-Aufrufe, Health-Probes, Retry, Circuit Breaker und Piper-Synthese.

## Open Lab

Die Management-Ebene läuft in der aktuellen Projektphase absichtlich ohne Anmeldung, Management-Token und TLS. Das bedeutet **nicht**, dass externe Connector-Credentials ungeschützt in der WebUI erscheinen dürfen:

- Secret-Werte liegen getrennt in `secrets.json` mit Dateimodus `0600`.
- Management-Antworten zeigen nur Vorhandensein, Zeitstempel und Fingerprint.
- Exporte und normale Backups enthalten keine Secret-Werte.
- Der LXC gehört in ein isoliertes Managementnetz.

## TTS-Workflow

```text
WebUI/API
   → TTS Job
   → Piper HTTP
   → validiertes RIFF/WAVE im Spool
   → optionaler Import-URL-Auftrag an Media Library
   → spätere Aussendung aus dem Media-/Recording-Workflow
```

Der Gateway sendet ein erzeugtes WAV nicht direkt in den Media Switch. Damit bleibt die bereits bewährte Trennung erhalten: Synthese erzeugt eine Datei, die eigentliche Aussendung erfolgt aus dem Media-Library-/Recording-Pfad.

## Wichtige Endpunkte

```text
GET  /health/live
GET  /health/ready
GET  /metrics
GET  /openapi.json
GET  /api/v1/status
GET  /api/v1/connectors
POST /api/v1/events
POST /api/v1/dispatch
POST /api/v1/webhooks/{connector_id}
GET  /api/v1/deliveries
POST /api/v1/deliveries/{id}/retry
POST /api/v1/tts/jobs
POST /api/v1/tts/jobs/{id}/publish
GET  /api/v1/tts/jobs/{id}/artifact
```

## Bewusste Grenzen

Noch nicht enthalten sind:

- produktives RBAC, TLS oder mTLS
- signierte Webhooks und Replay-Schutz mit Fremdsystem-PKI
- native Herstellerprotokolle ohne vorgeschalteten Adapter
- ein vollständiger Workflow-Designer
- direkte Audioaussendung oder Media-Transcoding
- produktives Secret Backend wie Vault/HSM
- garantierte genau-einmal-Zustellung über Fremdsystemgrenzen

Details stehen unter `docs/`.

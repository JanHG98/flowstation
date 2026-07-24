# SWMI Core 1 – Package N: Application Gateway

## Ergebnis

Dieses Paket implementiert den eigenständigen Application-Gateway-LXC als Integrationsgrenze zwischen NetCore-Tetra und externen Anwendungen.

## Enthalten

- Rust-Dienst `netcore-application-gateway` auf Port 8220,
- Connector Registry für SDS Router, Piper, Media Library, Telegram, DAPNET, MeshCom, Snom, GeoAlarm, WX/METAR, TPG2200, Directory und generische Webhooks,
- bidirektionaler Webhook-Ingress,
- Event-Normalisierung, Routingregeln und Text-/JSON-/TTS-Vorlagen,
- persistente Outbox mit Retry, TTL, Deduplizierung und Dead-Letter-Zustand,
- Rate Limit, Health-Probes und Circuit Breaker pro Connector,
- getrennte Secret-Ablage mit Redaction und Fingerprints,
- Piper-TTS-Orchestrierung mit RIFF/WAVE-Prüfung und begrenztem Spool,
- Import-URL-Vertrag zur nachfolgenden Media Library,
- WebUI, REST-API, OpenAPI, Metrics, Audit, Backup und Export,
- systemd- und LXC-Installationsskripte,
- statischer Paketchecker und CI.

## Betriebsregel

Der Application Gateway besitzt nur Integrations-, Zustellungs- und Connector-Zustände. Teilnehmer, Gruppen, Rufe, Mobility, SDS, Medien und Schlüssel bleiben Eigentum der jeweiligen Core-Dienste.

## Sicherheitsstatus

Die Management-Ebene bleibt entsprechend der aktuellen Projektbedingung `open_lab`: keine Anmeldung, keine Management-Tokens und kein TLS. Connector-Credentials werden trotzdem getrennt gespeichert und niemals in Management-GETs oder normalen Exporten ausgegeben.

## TTS-Grenze

Piper erzeugt ein validiertes WAV-Artefakt. Die Aussendung erfolgt nicht direkt aus dem TTS-Request, sondern später über die Media Library beziehungsweise den stabilen Recording-/Playback-Workflow.

## Bewusste Grenze

Fremdprotokolle wie DAPNET oder MeshCom werden in diesem Paket über explizite HTTP-Adapterverträge angebunden. Es wird keine native Protokollkonformität behauptet, solange der jeweilige Adapter nicht implementiert und getestet ist.

## Nächster Baustein

`media-library`: zentrale Verwaltung von Recordings, TTS-Dateien, Metadaten, Vorschau, Audio-Decoding und kontrollierter Aussendung.

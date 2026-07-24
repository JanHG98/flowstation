# SWMI Core 1 – Package M: Observability / NMS

## Ergebnis

Dieses Paket implementiert den eigenständigen Observability-LXC als Betriebsplattform getrennt vom Control Room.

## Enthalten

- Rust-Managementdienst `netcore-observability` auf Port 8210,
- eigener Prometheus-Textparser und bounded In-Memory-/JSON-Zeitreihenspeicher,
- periodische Liveness-, Readiness- und Metrics-Scrapes,
- synthetische Target-Metriken,
- Alarmregel-State-Machine mit Pending, Firing und Resolved,
- Acknowledge, manuelle Resolve-Aktion und zeitbegrenzte Silences,
- strukturierter JSON-Log-Ingest und Suche,
- NetCore-JSON-Trace-Span-Ingest und Suche,
- Audit, Retention, Backup und SHA-256-verifizierte Diagnosepakete,
- Stack-Health für Prometheus, Grafana, Loki und Alertmanager,
- native Konfigurationen für Prometheus, Grafana, Loki, Promtail und Alertmanager,
- optionale Journald-Forwarder-Agentdateien,
- WebUI, REST-API, OpenAPI, `/metrics`, Liveness und Readiness,
- systemd- und LXC-Installationsskripte,
- statischer Paketchecker und CI.

## Betriebsregel

Observability bleibt aus allen fachlichen Datenpfaden heraus. Ein Ausfall des NMS darf TBS, Call Control, Mobility, Media, SDS oder Packet Data niemals blockieren.

## Sicherheitsstatus

Das Paket bleibt entsprechend der aktuellen Projektbedingung `open_lab`: keine Anmeldung, keine Tokens und kein TLS. Dies wird in Konfiguration, Logs, WebUI und Dokumentation angezeigt.

## Grenze

Prometheus/Loki sind für Langzeitdaten vorgesehen. Der interne Rust-Speicher ist bewusst bounded und dient Betriebsübersicht, Alarmzustand und Diagnose. Vollständiges OTLP/Tempo, HA-Storage, produktive Eskalationsreceiver und RBAC folgen später.

## Nächster Baustein

`application-gateway`: gekapselte Connectoren zu Telegram, DAPNET, MeshCom, Snom, Geoalarm, WX/METAR, TPG2200, Directory und Fremd-APIs.

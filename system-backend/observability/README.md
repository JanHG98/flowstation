# NetCore Observability / NMS

## Zweck

`netcore-observability` ist die zentrale Betriebs- und Überwachungsebene für die NetCore-Tetra-SwMI. Der Dienst sammelt Prometheus-Metriken, nimmt strukturierte Logs und Trace-Spans entgegen, bewertet Alarmregeln, verwaltet Stummschaltungen und erstellt Diagnosepakete. Die eigene WebUI bleibt unabhängig vom Control Room erreichbar.

Das Paket enthält zusätzlich eine vorkonfigurierte klassische NMS-Landschaft aus:

- Prometheus für langfristiges Scraping und Rule Evaluation,
- Grafana für Dashboards,
- Loki und Promtail für zentrale Logs,
- Alertmanager für Alarmrouting und Wartungsfenster.

Der Rust-Dienst ist dabei **nicht** nur ein Linkmenü: Er besitzt einen bounded internen Collector und Alarmpfad, damit Zielzustände, Regeln, Audit und Diagnose auch dann sichtbar bleiben, wenn einzelne Stack-Komponenten ausfallen.

## WebUI

Standardport: `8210`

```text
http://<observability-lxc>:8210/
```

Ansichten:

- Übersicht und Gesamtzustand,
- Scrape Targets und Abhängigkeiten,
- Metrikkatalog und bounded Zeitreihen,
- Alarmregeln, Alarmzustände, Quittierungen und Silences,
- strukturierte Logs,
- Trace-Spans,
- Audit,
- Retention, Backup und Diagnosepakete,
- Konfiguration, API und Links zu Grafana/Prometheus/Loki/Alertmanager.

## Open-Lab-Bedingung

Aktuell ausdrücklich:

- keine Benutzerkonten,
- keine Tokens,
- kein TLS,
- keine mTLS-Serviceidentität,
- jede erreichbare Gegenstelle kann Logs/Spans einspeisen und Regeln, Targets oder Silences ändern.

Der LXC darf daher ausschließlich in einem isolierten Test- und Managementnetz betrieben werden. Produktiv folgen zentrale Anmeldung, RBAC, TLS/mTLS, signierte Ingest-Pfade und getrennte Rollen für Operator und Auditor.

## Datenpfade

```text
Services /metrics ───────┐
Services /health/* ──────┼─> interner Collector / NMS-WebUI
JSON Logs ───────────────┤
JSON Trace-Spans ─────────┘

Services /metrics ─────────> Prometheus ──> Grafana
journald ──> Promtail ──────> Loki ───────> Grafana
Prometheus Rules ───────────> Alertmanager
```

Der interne Collector akzeptiert das Prometheus-Textformat. Log- und Trace-Ingest verwenden zunächst bewusst das NetCore-JSON-v1-Schema; ein vollwertiger OTLP/Tempo-Pfad ist noch nicht als implementiert markiert.

## Relevante API-Endpunkte

```text
GET  /api/v1/status
GET  /api/v1/targets
POST /api/v1/targets
POST /api/v1/targets/{id}/test
GET  /api/v1/metrics/catalog
GET  /api/v1/metrics/series
POST /api/v1/logs/ingest
POST /api/v1/traces/ingest
GET  /api/v1/rules
POST /api/v1/rules
GET  /api/v1/alerts
POST /api/v1/alerts/{id}/acknowledge
GET  /api/v1/silences
POST /api/v1/silences
POST /api/v1/diagnostics
POST /api/v1/maintenance/scrape-now
POST /api/v1/maintenance/tick
POST /api/v1/maintenance/backup
GET  /metrics
```

## Retention und Schutzgrenzen

Alle internen Speicher sind hart begrenzt:

- maximale Zahl an Serien,
- maximale Samples je Serie,
- Alters- und Mengenlimits für Logs, Spans, Alerts und Audit,
- maximale HTTP-Body- und Scrape-Antwortgröße,
- Diagnosepakete mit SHA-256-Manifest.

Monitoring darf den Produktivbetrieb nicht blockieren. Ein nicht erreichbares NMS oder ein voller interner Puffer führt deshalb weder zu Backpressure auf Call Control noch zu einem Ausfall der TBS.

## Klassischer Stack

Die Dateien unter `stack/` sind für native Prozesse im Observability-LXC vorbereitet. `install/install-stack.sh` installiert ausschließlich Konfiguration und systemd-Units und aktiviert nur Komponenten, deren Binaries bereits vorhanden sind. Es lädt bewusst keine fremden Binaries aus dem Internet herunter.

Standardports:

| Komponente | Port |
| --- | ---: |
| Observability WebUI/API | 8210 |
| Grafana | 3000 |
| Prometheus | 9090 |
| Alertmanager | 9093 |
| Loki | 3100 |

## Bewusste Grenzen dieser Phase

Noch nicht enthalten:

- produktive Authentisierung/RBAC,
- TLS/mTLS,
- Remote Write und hochverfügbare Langzeit-Metrikablage,
- echtes OTLP/Tempo-Backend,
- automatische Pager-/Mail-/SMS-Eskalation mit produktiven Secrets,
- manipulationssichere externe Audit-Ablage,
- Clusterbetrieb von Prometheus/Loki/Alertmanager.

Diese Grenzen sind in WebUI, Konfiguration und Dokumentation sichtbar. Das Paket behauptet nicht, ein HA-SIEM oder zertifiziertes SOC zu sein.

# Observability

## Zweck

Observability stellt zentrale Metriken, Logs, Traces und Alarmierungen bereit.

## Geplante Komponenten

- Prometheus und Grafana
- Loki oder vergleichbare zentrale Logs
- Alertmanager und Health Checks
- RF-, Call-, Mobility- und Packet-Data-Metriken

## Architekturregel

Monitoring darf den Produktivbetrieb niemals blockieren.

## WebUI zur Verwaltung

Observability erhält eine eigene Einstiegs- und Verwaltungsoberfläche. Sie bündelt die eingebetteten Oberflächen von Grafana, Logsystem und Alerting.

### Geplante Ansichten

- Gesamtzustand aller NetCore-Dienste
- Metriken, Logs und Traces
- aktive Alarme und Eskalationen
- RF-, Call-, Mobility-, SDS- und Packet-Data-Dashboards
- Datenquellen, Retention und Speicherzustand
- Links zu den Verwaltungsoberflächen der betroffenen Dienste

### Kritische Aktionen

- Alarm quittieren oder stummschalten
- Diagnosepaket erzeugen
- Datenquelle testen
- Retention und Alarmregeln verwalten

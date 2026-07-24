# SWMI Core 1 – Package L: Control Room

## Ergebnis

Der bestehende Control-Room-Core wird als eigenständiger LXC-Dienst mit Browser-WebUI auf Port `9010` weitergeführt. Er aggregiert die Lage der autoritativen Core-Dienste, ohne deren Teilnehmer-, Mobility-, Gruppen-, Call-, SDS-, Packet- oder Security-Zustand zu duplizieren.

## Funktionen

- Service-Registry für alle bisher umgesetzten LXCs
- zyklische Live-/Ready-Prüfung und Status-Snapshots
- optionale `/api/v1/status`-Zusammenfassungen
- kuratierte Domain-Kennzahlen und bevorzugte Gesamt-KPIs aus den autoritativen Diensten
- kritische/degradierte/offline Service-Lage
- automatische Service-Incidents nach Fehlerfolge
- manuelle Incidents, Acknowledge, Resolve und Notizen
- persistentes Schichtbuch
- bestehende TBS-Lage, aktive Rufe, Notfälle und Operator-Kommandos
- direkte Verlinkung zu jeder eigenständigen Dienst-WebUI
- versionierte API, OpenAPI, Export und Prometheus-Metriken
- systemd-/LXC-Deployment

## Architekturgrenze

Der Control Room ist Presentation und Operator Plane. Die jeweiligen Fachkerne bleiben autoritative Datenhalter. Es gibt absichtlich keinen generischen Schreibproxy, der beliebige Managementaufrufe zu anderen Diensten weiterreicht.

## Sicherheitsstatus

Diese Phase bleibt `open_lab`: keine Anmeldung, keine Tokens, kein Node-Token und kein TLS. Die bereits vorhandene RBAC-Basis bleibt im Code für den späteren gesicherten Betrieb, ist im Beispiel und im systemd-Service aber ausdrücklich deaktiviert.

## Nächster Baustein

`observability` / NMS mit Prometheus, Grafana, Loki und Alertmanager.

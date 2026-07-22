# Node Gateway

## Zweck

Der Node Gateway ist der zentrale Einstiegspunkt für alle NetCore-TBS-Instanzen in das System-Backend.

## Kernaufgaben

- Verbindungen zu Basisstationen aufbauen und verwalten
- Nodes authentisieren und Protokollversionen prüfen
- Heartbeats, Reconnects, Steuerbefehle und Telemetrie vermitteln
- TBS von den internen Backend-Diensten entkoppeln

## Nicht zuständig

- Keine fachliche Teilnehmer-, Gruppen- oder Ruflogik
- Kein Sprachtransport und keine dauerhafte Fachdatenhaltung

## WebUI zur Verwaltung

Der Node Gateway erhält eine eigene, direkt im Dienst ausgelieferte Verwaltungsoberfläche.

### Geplante Ansichten

- verbundene und getrennte TBS-Nodes
- Session-, Heartbeat- und Reconnect-Status
- Zertifikate, Node-Identitäten und Protokollversionen
- Capabilities und Kompatibilitätswarnungen
- Command- und Telemetriefluss
- Abhängigkeiten, Logs, Konfiguration und Wartungsmodus

### Kritische Aktionen

- Node trennen oder sperren
- Zertifikat widerrufen beziehungsweise erneuern
- Wartungsmodus setzen
- kontrollierten Reconnect anfordern

Alle schreibenden Aktionen werden rollenbasiert geschützt und auditiert.

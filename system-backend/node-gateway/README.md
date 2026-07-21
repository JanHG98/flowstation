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

# Call Control

## Zweck

Call Control ist der zentrale, eigenständig deploybare Dienst für netzweite logische Gruppen- und Individualrufe. Die TBS behalten weiterhin die zeitkritischen CMCE-, Funkkanal- und Floor-Prozeduren; Call Control koordiniert die lokalen Call Legs über mehrere Zellen hinweg.

## Kernaufgaben

- logische Gruppen- und Individualrufe verwalten
- passende TBS anhand von Affiliationen und Registrierungen auswählen
- lokale Call Legs starten und beenden
- Priorität, Notrufstatus und Floor-Zustand zusammenführen
- Floor-Anforderungen, Queueing und Operator-Handover koordinieren
- laufende TBS-Rufe aus Telemetrie erkennen
- Restore Context zwischen Quell- und Ziel-TBS übertragen
- Zustände, Fehler, Timeouts und Teilstarts persistent dokumentieren

## WebUI

Die eigene WebUI läuft standardmäßig unter `http://<LXC-IP>:8120/` und bleibt unabhängig vom Control Room erreichbar.

Sie zeigt logische Calls, TBS-Legs, Floor Holder, Queue, Teilnehmerlage, Restore-Vorgänge, Basisstationen und Ereignisse. Gruppen- und Individualrufe sowie Floor- und Restore-Aktionen können direkt ausgeführt werden.

## Open-Lab-Modus

Diese Ausbaustufe besitzt absichtlich keine Tokens, Passwörter, Benutzeranmeldung, TLS oder RBAC. Sie darf nur in einem isolierten Testnetz betrieben werden.

## Datenhaltung

- `/var/lib/netcore-call-control/calls.json`
- `/var/lib/netcore-call-control/calls.json.bak`

## Abhängigkeiten

- Node Gateway auf `/ws/backend`
- kompatible TBS mit `call_control` und für Restore zusätzlich `call_restore_context`
- Teilnehmer- und Gruppenlage aus TBS-Telemetrie
- später Media Switch für den eigentlichen netzweiten Sprachtransport

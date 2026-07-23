# Queues, Wiederholungen und Reports

## Zustände

- `received`: angenommen, noch nicht geplant
- `queued`: mindestens ein Zustellweg wartet
- `offline`: aktuell keine zuständige oder erreichbare TBS
- `in_flight`: Kommando an eine TBS übergeben
- `delivered`: alle vorgesehenen Legs angenommen beziehungsweise bestätigt
- `partial`: nur ein Teil der Legs erfolgreich
- `failed`: Zustellung fehlgeschlagen
- `expired`: TTL abgelaufen
- `cancelled`: operatorseitig gestoppt
- `dead_letter`: endgültig nicht zustellbar

Jede TBS und jede Anwendung bildet ein eigenes Delivery Leg. Gruppen-SDS können deshalb teilweise erfolgreich sein, ohne dass bereits alle Standorte erreicht wurden.

## Retry

Der Backoff startet mit `initial_retry_secs` und wächst exponentiell bis `max_retry_secs`. Nach `max_attempts` wird ein TBS-Leg endgültig fehlgeschlagen. Die Bedienoberfläche kann Nachrichten manuell erneut einreihen.

## Zustellberichte

Die Antwort `SdsDeliveryResponse` bestätigt zunächst nur, dass die lokale TBS den Auftrag für die Air-Interface-Zustellung angenommen hat. Terminalberichte, etwa SDS-TL Delivery Reports, werden als eigene Meldung verarbeitet und im Nachrichtenobjekt gespeichert.

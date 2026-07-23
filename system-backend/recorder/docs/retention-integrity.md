# Retention, Legal Hold und Integrität

## Retention

Beim Finalisieren wird `retention_until` aus Rufende plus `retention_days` berechnet. Der Wartungsworker prüft abgelaufene Aufnahmen regelmäßig und entfernt sie nur, wenn kein Legal Hold aktiv ist.

Eine Änderung über die API setzt die Frist erneut ausgehend vom Rufende. Der erlaubte Bereich liegt bei 1 bis 3650 Tagen.

## Legal Hold

`legal_hold = true` blockiert sowohl die automatische Retention-Löschung als auch manuelle Löschung. Das Lösen des Holds löscht die Aufnahme nicht sofort; sie wird beim nächsten Retention-Lauf entfernt, wenn die Frist bereits abgelaufen ist.

## Löschung

Löschung ist endgültig und entfernt:

- das komplette Aufnahmeverzeichnis
- einen eventuell erzeugten TAR-Export
- den Eintrag im Recorder-Zustand

Sie kann global mit `security.allow_delete = false` deaktiviert werden.

## Integritätsprüfung

Die API berechnet SHA-256 über Audio und Index neu und vergleicht beide Werte mit `integrity.json`. Ein Fehler setzt den Status auf `failed` und erzeugt ein Ereignis. Die API liefert dann absichtlich einen Fehlerstatus zurück.

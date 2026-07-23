# Gruppenrichtlinie

Die Richtlinie wird versioniert über den Node Gateway an jede kompatible TBS übertragen.

Sie enthält:

- GSSI und Aktivierungszustand
- Freigaben für Affiliation, DGNA, Gruppenruf, Gruppen-SDS und Notruf
- Mindestpriorität für Gruppenrufe
- Class of Usage für zentral ausgelöste DGNA-Anhänge
- Teilnehmermitgliedschaften
- Auto-Attach-Markierungen
- optionale TBS-Bereiche pro Gruppe

## Lokale TBS-Durchsetzung

Nach erfolgreicher Synchronisation prüft die TBS neue Gruppenaffiliationen und DGNA lokal. Bei aktivierter Mitgliedschaftsdurchsetzung darf ein Teilnehmer einen Gruppenruf nur starten, wenn er lokal zu dieser Gruppe affiliiert ist. Gruppenruf- und Notruffreigabe sowie die konfigurierte Mindestpriorität werden in CMCE geprüft.

Bei einer neuen Policy können bestehende Affiliationen bereinigt und Auto-Attach-Mitgliedschaften per DGNA gesetzt werden. Ohne zentrale Policy bleibt das bisherige lokale Gruppenverhalten bestehen.

## Aktuelle Grenze

`sds_allowed` wird bereits als Teil der Richtlinie verteilt. Die verbindliche netzweite Auswertung für Gruppen-SDS folgt mit dem späteren `sds-router`; bis dahin ändert dieses Feld den bestehenden lokalen SDS-Pfad noch nicht.

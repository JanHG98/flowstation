# Group Core

## Zweck

Der Group Core ist der zentrale, eigenständig deploybare Dienst für GSSI-Stammdaten, Teilnehmermitgliedschaften, aktuelle Affiliationen und DGNA.

## Kernaufgaben

- Gruppenprofile und Dienstfreigaben verwalten
- feste und dynamische Mitgliedschaften verwalten
- automatische Gruppenanbindung definieren
- Gruppenrichtlinien versioniert an alle TBS verteilen
- aktuelle Affiliationen aus TBS-Telemetrie darstellen
- DGNA-Operationen auslösen und bis zur TBS-Antwort verfolgen
- Gruppenruf-, Mitgliedschafts- und Notrufzulassung lokal auf der TBS anwenden
- Mindestpriorität und Class of Usage zentral vorgeben

## WebUI

Die eigene WebUI läuft standardmäßig unter `http://<LXC-IP>:8110/` und bleibt unabhängig vom Control Room erreichbar.

Sie enthält Gruppen-, Mitgliedschafts-, Affiliation-, DGNA-, TBS-Sync- und Ereignisansichten.

## Open-Lab-Modus

Diese Ausbaustufe besitzt absichtlich keine Tokens, Passwörter, Benutzeranmeldung oder TLS. Sie darf nur in einem isolierten Testnetz betrieben werden.

## Datenhaltung

- `/var/lib/netcore-group-core/groups.json`
- `/var/lib/netcore-group-core/groups.json.bak`

## Abhängigkeiten

- Node Gateway auf `/ws/backend`
- kompatible TBS mit `group_policy`- und `dgna`-Capability
- später Subscriber Core, Call Control und SDS Router

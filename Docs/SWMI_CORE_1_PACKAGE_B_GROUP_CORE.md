# SWMI Core 1 – Paket B: Group Core

## Ziel

Dieses Paket ergänzt den eigenständig deploybaren `netcore-group-core` als vierten LXC-Dienst.

## Enthalten

- persistente GSSI-Stammdaten
- Teilnehmermitgliedschaften
- Auto-Attach und Locked-Markierung
- Bereichsfilter pro TBS
- versionierte TBS-Gruppenrichtlinie
- lokale Affiliation-Prüfung in MM
- Gruppenruf-Freigabe, Mitgliedschaftsprüfung, Notruffreigabe und zentrale Mindestpriorität in CMCE
- zentrale Class of Usage bei DGNA-Anhängen
- DGNA mit expliziter TBS-Antwort
- Live-Affiliationen aus Telemetrie
- eigene WebUI, REST-API, Metriken und OpenAPI
- systemd-Unit sowie Installations-, Update- und Uninstall-Skripte

## Open Lab

Der Dienst läuft ausschließlich mit `security.mode = "open_lab"`. Token-, Passwort- und TLS-Felder existieren in dieser Phase nicht.

## TBS-Verhalten

Die TBS speichert die letzte zentrale Richtlinie lokal im Runtime-State. Ohne zentrale Richtlinie bleibt das historische offene Gruppenverhalten bestehen. Mit Richtlinie werden Gruppenattach, DGNA, die Affiliation des rufenden Teilnehmers, Gruppenruf- und Notrufzulassung lokal geprüft. Die konfigurierte Mindestpriorität und Class of Usage werden auf der TBS angewendet. Bei einer Synchronisation können bestehende Affiliationen bereinigt und Auto-Attach-Mitgliedschaften per DGNA gesetzt werden.

## Bewusste Grenzen

- keine Benutzeranmeldung oder RBAC
- keine PostgreSQL-Datenbank; JSON ist für die Testphase ausreichend
- `locked` wird im zentralen Datenmodell transportiert, aber eine spätere Operator-/Subscriber-Rollenlogik folgt mit Security und Control Room
- `sds_allowed` wird bereits verteilt und gespeichert; die verbindliche netzweite SDS-Durchsetzung folgt mit dem zentralen SDS Router
- netzweite Call-Legs und Media-Verteilung folgen in `call-control` und `media-switch`

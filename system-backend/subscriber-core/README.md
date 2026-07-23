# Subscriber Core

## Zweck

Der Subscriber Core ist die zentrale Teilnehmerdatenbank und Zugangsrichtlinie des NetCore-TETRA-Testnetzes.

## Aktueller Funktionsumfang

- persistente Teilnehmerprofile als atomar geschriebene JSON-Datenbank
- ISSI, Home-MCC/MNC, Name, Organisation und Gerätezuordnung
- Freigabe, Sperre, Rufpriorität und Dienstberechtigungen
- Standardgruppen als Vorbereitung für den kommenden Group Core
- Live-Sicht auf registrierte Funkgeräte aus der TBS-Telemetrie
- automatische Verteilung der Zulassungsrichtlinie an alle verbundenen TBS
- expliziter Closed-Empty-Modus: keine Profile bedeutet **deny all**, nicht versehentlich offenes Netz
- JSON-Import/Export und CSV-Export
- eigene WebUI, REST-API, Metriken und OpenAPI

## WebUI

```text
http://<LXC-IP>:8100/
```

Die WebUI bietet Teilnehmer-CRUD, Import/Export, Live-Registrierungen, TBS-Synchronisationsstatus und Ereignisprotokoll.

## Offener Testmodus

Diese Stufe arbeitet absichtlich ohne Tokens, Login, Passwörter, TLS oder Client-Zertifikate. Jeder erreichbare Client kann Teilnehmer und Zugangsregeln ändern. Nur in einem isolierten Testnetz betreiben.

## Zugangsmodi

- `allow_list`: nur Profile mit `enabled = true` und `registration_allowed = true`
- `open_network`: alle ISSIs dürfen registrieren; Profile dienen nur als Stammdaten

Änderungen werden bei `auto_sync = true` sofort an jede verbundene und kompatible TBS verteilt. Entfernte oder gesperrte Teilnehmer können zur sofortigen Re-Registrierung gezwungen werden.

## Architekturgrenze

Aktueller Aufenthaltsort und Context Transfer verbleiben im Mobility Core. Gruppenautorität folgt im Group Core. Kryptografische Schlüssel gehören niemals in diesen Dienst.

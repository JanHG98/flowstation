# Control Room

## Zweck

Der Control Room ist die zentrale Bedien-, Leitstellen- und Übersichtsebene.

## Kernaufgaben

- TBS, Teilnehmer, Gruppen, Calls, SDS und Notrufe anzeigen
- Operator-Kommandos, DGNA und Rufauslösung bereitstellen
- Rollen, Rechte und Audit verwalten
- Leitstellen-Audio integrieren

## Architekturregel

Der Control Room ist keine autoritative Teilnehmer-, Mobility- oder Call-Datenbank.

## WebUI zur Verwaltung

Der Control Room besitzt bereits seine zentrale Bedienoberfläche und wird als eigenständige Service-WebUI weitergeführt.

### Geplante Verwaltungsbereiche

- Operatoren, Rollen und Sitzungen
- Leitstellenprofile und Arbeitsplätze
- Backend-Abhängigkeiten und deren Erreichbarkeit
- API-Tokens, Node-Zugänge und Integrationen
- Audit, Systemstatus und Wartungsfunktionen
- Verlinkung zu den eigenständigen WebUIs aller Backend-Dienste

Der Control Room ersetzt die Service-WebUIs nicht. Jeder Container bleibt auch bei ausgefallenem Control Room separat administrierbar.

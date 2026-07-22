# SDS Router

## Zweck

Der SDS Router übernimmt die netzweite Zustellung und Speicherung von SDS- und Statusnachrichten.

## Kernaufgaben

- Individual- und Gruppen-SDS routen
- Store-and-forward, TTL und Prioritäten verwalten
- Zustellberichte, Wiederholungen und Offline-Queues behandeln
- Protokoll-ID-basiert an Anwendungen weiterleiten

## Abgrenzung

MCCH/FACCH-Zustellung und Air-Interface-PDUs bleiben auf der TBS.

## WebUI zur Verwaltung

Der SDS Router erhält eine eigene Verwaltungsoberfläche für Nachrichten, Queues und Zustellpfade.

### Geplante Ansichten

- eingehende und ausgehende SDS
- Zustellstatus, Wiederholungen und TTL
- Online-, Offline- und Dead-Letter-Queues
- Individual-, Gruppen- und Protokoll-ID-Routen
- zuständige TBS und Anwendungsgateways
- Nachrichtentrace und Audit

### Kritische Aktionen

- SDS manuell senden
- Zustellung wiederholen oder abbrechen
- Dead-Letter-Nachricht erneut einreihen
- Route oder Protokollzuordnung ändern

Nutzdaten werden abhängig von Rolle und Datenschutzrichtlinie maskiert.

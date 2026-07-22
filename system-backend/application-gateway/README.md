# Application Gateway

## Zweck

Der Application Gateway bündelt externe Anwendungen und nicht zeitkritische Integrationen.

## Geplante Integrationen

- Telegram, DAPNET und MeshCom
- Snom, Geoalarm und TPG2200
- WX/METAR, Status Directory und Webhooks

## Architekturregel

Ausfälle externer Dienste dürfen weder TBS noch kritische Core-Funktionen beeinträchtigen.

## WebUI zur Verwaltung

Der Application Gateway erhält eine eigene Verwaltungsoberfläche für alle externen Connectoren.

### Geplante Ansichten

- Telegram, DAPNET, MeshCom, Snom, Geoalarm und Wetterdienste
- Connector-Status, Rate Limits und letzte Fehler
- Ein- und Ausgangsqueues
- Routing- und Transformationsregeln
- Testaktionen und Webhook-Historie
- maskierte Zugangsdaten und Audit

### Kritische Aktionen

- Connector aktivieren oder deaktivieren
- Testnachricht senden
- Queue wiederholen oder leeren
- Konfiguration neu laden

Geheimnisse werden nur gesetzt oder ersetzt, niemals im Klartext angezeigt.

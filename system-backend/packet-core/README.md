# Packet Core

## Zweck

Der Packet Core verwaltet TETRA-SNDCP-Kontexte und paketorientierte Datendienste.

## Kernaufgaben

- PDP Contexts und NSAPI verwalten
- READY/STANDBY, Data Transmit und Reconnect steuern
- Adresszuordnung, Fragmentierung und Reassembly übernehmen
- Mobility Anchoring und Paketprioritäten verwalten

## Abgrenzung

TUN/TAP, NAT und Firewalling gehören zum IP Gateway; lokale PDCH-Zuteilung bleibt auf der TBS.

## WebUI zur Verwaltung

Der Packet Core erhält eine eigene Verwaltungsoberfläche für SNDCP- und PDP-Kontexte.

### Geplante Ansichten

- aktive PDP Contexts und NSAPI
- READY-, STANDBY- und Reconnect-Zustände
- IP-Zuordnungen und Teilnehmerberechtigungen
- Fragmentierung, Reassembly und Flow Control
- PDCH-Anforderungen und Durchsatz
- Fehler, Timeouts und Context-Historie

### Kritische Aktionen

- Context kontrolliert trennen
- Reconnect beziehungsweise Reinitialisierung auslösen
- Adresszuordnung erneuern
- Paketdatenzugriff sperren oder freigeben

# IP Gateway

## Zweck

Der IP Gateway verbindet den TETRA-Packet-Core mit IP-Netzen und Testdiensten.

## Kernaufgaben

- TUN/TAP, Routing und IP-Adressräume bereitstellen
- NAT, Firewalling und DNS übernehmen
- WAP- und Testendpunkte anbieten
- Paketmitschnitt und Fehleranalyse ermöglichen

## Sicherheitsgrundsatz

Zugriffe aus dem TETRA-Paketdatennetz werden standardmäßig restriktiv gefiltert.

## WebUI zur Verwaltung

Der IP Gateway erhält eine eigene Verwaltungsoberfläche für IP-Anbindung, Routing und Sicherheit.

### Geplante Ansichten

- TUN/TAP-Interfaces und Adresspools
- Routingtabellen, NAT und Firewallregeln
- DNS- und Gatewayzustand
- aktive Teilnehmerflüsse und Datenmengen
- WAP- und Testendpunkte
- Packet-Capture-Aufträge und Diagnose

### Kritische Aktionen

- Route oder Firewallregel ändern
- Teilnehmerfluss sperren
- Capture starten oder beenden
- Gateway beziehungsweise Interface neu laden

Standardmäßig ist die Oberfläche nur über das Managementnetz erreichbar.

# Open-Lab-Modus

Port **8180** besitzt derzeit absichtlich keine Anmeldung, Tokens oder TLS. Jeder Client im erreichbaren Netz kann Policies ändern, Teilnehmer sperren, Kontexte widerrufen und Edge-Aktionen abrufen.

Daher gilt zwingend:

- eigener isolierter Test-VLAN/LXC-Bridge
- keine Portweiterleitung ins Internet
- kein produktiver Schlüsselbestand
- keine produktiven ISSI-/KMF-Daten
- `shadow` als Startmodus

Der Security Core sichert in dieser Phase die Funkteilnehmerlogik, **nicht** seine eigene Managementoberfläche. Das klingt zunächst paradox, ist aber für die aktuelle offene Testumgebung ausdrücklich so gewollt.

# Open-Lab-Modus

Dieser Zwischenstand implementiert absichtlich keine Konten, Token, mTLS oder TLS. Das betrifft nicht nur lesende Diagnosezugriffe: Ein erreichbarer Client kann Routen, NAT, Firewall, DNS, Blocklisten und Captures verändern sowie einen Kernel-Reconcile auslösen.

Daher gelten bis zum Security-Ausbau:

- eigener isolierter Management-VLAN,
- keine Portweiterleitung aus fremden Netzen,
- WebUI nicht direkt aus dem TETRA-Datennetz freigeben,
- Konfigurationsdatei und State-Verzeichnis nur für root/netcore,
- `shadow` als sicherer Startmodus.

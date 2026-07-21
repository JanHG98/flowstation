# Repository-Vergleich – NetCore-Tetra, Nexus-BS und FlowStation-Forks

Stand: 21.07.2026

## Ergebnis in einem Satz

NetCore-Tetra ist gegenüber den drei FlowStation-/BlueStation-Vergleichsständen bereits deutlich weiter ausgebaut. Der klar relevante Nexus-Vorsprung war der terminalseitige SNDCP/WAP-IP-Pfad; dieser ist in diesem Paket als Clean-room-Implementierung ergänzt. Weitere Nexus-Funktionen wurden absichtlich nicht ungefragt übernommen.

## Vergleich

| Bereich | NetCore-Tetra vor diesem Paket | Nexus-BS | MidnightBlue tetra-bluestation | misadeks / ea5gvk FlowStation | Ergebnis |
|---|---|---|---|---|---|
| Registrierung / Gruppen | vorhanden und stark erweitert | vorhanden | grundlegender Alpha-Stand | vorhanden | kein Port nötig |
| Gruppen- und Einzelruf | vorhanden, inklusive Duplex und Dual Carrier | vorhanden | teilweise | vorhanden | NetCore mindestens gleichauf |
| Emergency / Pre-emption | vorhanden und getestet | vorhanden | begrenzt | im FlowStation-Funktionsumfang | kein Port nötig |
| SDS / U-STATUS / HMD | umfangreich vorhanden | vorhanden | grundlegend | vorhanden | kein Port nötig |
| DGNA | vorhanden, API und Tests | vorhanden | nicht Schwerpunkt | vorhanden | kein Port nötig |
| Dashboard / OTA / Profile | umfangreich vorhanden | eigenes Web-/Betriebskonzept | gering | vorhanden | NetCore weiter ausgebaut |
| Control Room | eigenständiger NetCore-Control-Room-Stack | nicht derselbe Schwerpunkt | nicht vorhanden | nicht in diesem Umfang | NetCore-spezifischer Vorsprung |
| Dual Carrier | vorhanden | kein für diesen Port relevanter Vorsprung | nicht vorhanden | nicht im verglichenen Basisumfang | NetCore-spezifischer Vorsprung |
| WAP über SNDCP/IP | nur PDP-/CHAP-Vorarbeit | vollständiger Terminalpfad | nicht vorhanden | nicht vollständig vorhanden | **jetzt eingebaut** |
| WAP über SDS Type 4 | nicht vorhanden | Legacy-/Diagnosepfad vorhanden | nicht vorhanden | nicht gefunden | Kandidat, nur nach Freigabe |
| Parrot-/Echo-Einzelruf | nicht als eigener Funkdienst vorhanden | eigener Parrot-Dienst | nicht vorhanden | Echo-Funktion im FlowStation-Umfeld | Kandidat, nur nach Freigabe |
| Vollständige SNDCP-Suite | bisher Stub; jetzt WAP-MVP | deutlich breitere Primitive/Sessionlogik | nicht vorhanden | nicht vollständig | Kandidat, nur nach Freigabe |

## Bereits in NetCore vorhanden – daher nicht erneut eingebaut

- DGNA einschließlich Zuweisung, Entfernung, API-Pfad und Tests
- Emergency-Priorisierung und Pre-emption bei voller Zelle
- OTA-Updatepfad
- Control-Room-Operatorprofile
- zahlreiche Integrationen wie Asterisk, DAPNET, EchoLink, MeshCom, GeoAlarm, Snom und TPG2200
- Live-Dashboard, Teilnehmerverwaltung, SDS und U-STATUS
- Dual-Carrier-Timeslotverwaltung

## Eingebaut: WAP/SNDCP-MVP

Enthalten sind:

- dynamische/statische PDP-Kontexte
- Motorola-CHAP-Success
- IPv4/UDP
- WTP/WSP Connect, Resume und GET
- XHTML/WML-Statusseite
- PDCH-Zuweisung auf TS2
- gemeinsamer Timeslot-Schutz gegen Sprachrufkollisionen
- Konfigurations- und Protokolltests

Nicht enthalten sind allgemeines IP-Routing, TCP, NAT oder ein Internetzugang.

## Noch offene Funktionskandidaten – nicht eingebaut

### A. Parrot-/Echo-Einzelruf

Ein dedizierter TETRA-Einzelrufdienst, der Sprache aufzeichnet und zurückspielt. Das ist für Funk-/Audio-/Latenztests sehr praktisch und unabhängig von EchoLink oder Asterisk.

### B. Legacy-WAP über SDS Type 4

Ein separater, einfacher Diagnose-/Informationspfad über SDS statt Paketdaten. Das ist kein Ersatz für den jetzt eingebauten Terminalbrowser, kann aber bei alten Endgeräten oder als Fallback nützlich sein.

### C. Vollständiger SNDCP-Ausbau

Zusätzliche SN-PDUs und Zustandslogik, beispielsweise RECONNECT, PAGE, MODIFY und DATA PRIORITY, detailliertere Reject-Ursachen, Retransmission-/Session-Caches sowie optional ein TCP/HTTP-Debugpfad.

## Lizenzhinweis

Nexus-spezifische Teile stehen nicht einfach unter derselben permissiven Lizenz wie der historische BlueStation-Unterbau. Deshalb wurde der WAP-Code nicht kopiert, sondern anhand des vorhandenen Clean-room-Vertrags, der Protokollfelder und der eingefrorenen Bytevektoren neu implementiert.

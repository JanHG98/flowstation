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

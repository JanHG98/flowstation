# SWMI Core 1 – Paket G: Packet Core

## Ergebnis

Paket G ergänzt den eigenständigen LXC-Dienst `system-backend/packet-core/` mit WebUI auf Port 8160. Der Dienst zentralisiert die langlebige SNDCP-Netzsicht, ohne PHY, MAC, LLC oder lokale TDMA-Entscheidungen aus der TBS herauszureißen.

## Funktionen

- PDP-Kontexte und NSAPI 1..14
- READY/STANDBY/RESPONSE-WAITING/SUSPENDED
- Activate, Data Transmit, Reconnect, Modify, End of Data und Deactivate
- IPv4-Adresspool und Mobility Anchor
- PDCH-/Bearer-Sicht aus TBS-Telemetrie
- Priorität und Flow Control
- Fragmentierung, Reassembly und N-PDU-Outbox
- Shadow- und Authoritative-Modus
- versioniertes Edge-Protokoll `netcore-packet-edge-v1`
- Node-Gateway-Kommandos zur lokalen SNDCP-Entity
- persistente Datenbank, API, OpenAPI, Metrics und WebUI

## Edge-Integration

Die TBS-Control-Plane besitzt vier neue Kommandofamilien:

- `PacketDataContextDeactivate`
- `PacketDataContextModify`
- `PacketDataWake`
- `PacketDataEndOfData`

Sie werden direkt an `TetraEntity::Sndcp` geroutet. Die SNDCP-Entity antwortet mit `PacketDataActionResult`, sodass der Packet Core Aktionen eindeutig korrelieren kann.

## Bewusste Grenze

TUN/TAP, NAT, Firewall, DNS, Routing, WAP-Testserver und Packet Capture sind nicht Teil dieses Pakets. Sie folgen als LXC 09 `ip-gateway`. Dadurch bleibt der Packet Core ein sauberer SNDCP-/Kontextdienst und bekommt keine halbfertige Netzwerkkarte angeklebt.

## Sicherheitsstatus

Der Dienst läuft in der aktuellen Teststufe als `open_lab`: keine Anmeldung, keine Token und kein TLS. Jeder erreichbare Client kann sensible Packet-Data-Zustände lesen und verändern.

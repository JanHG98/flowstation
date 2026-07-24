# NetCore IP Gateway

Der IP Gateway ist der **Layer-3-Übergang zwischen dem zentralen Packet Core und normalen IPv4-Netzen**. Er übernimmt vollständige IP-N-PDUs aus der Packet-Core-Outbox, speist sie über ein Linux-TUN-Interface in den Kernel ein und liefert zum TETRA-Adresspool geroutete Pakete wieder an den passenden PDP-Kontext zurück.

## Aktueller Umfang

- Linux-TUN-Interface für rohe IPv4-N-PDUs
- Kopplung an den Packet Core über dessen offene HTTP-API
- IP-Lease-/PDP-Kontextspiegelung anhand IPv4 → ISSI/NSAPI
- bidirektionaler Pakettransport mit Downlink-Retry-Queue
- IPv4-Routing und Forwarding
- nftables-Firewall mit Default-Policy, benutzerdefinierten Regeln und Flow-Block
- Masquerading, SNAT und DNAT
- integrierter DNS-Forwarder mit statischen A-Records
- WAP/WML-Testseite und allgemeiner HTTP-Testserver
- UDP-Echo-Testdienst
- Flow-Tabelle mit 5-Tupel, Teilnehmerbezug und Zählern
- integrierte PCAP-Erzeugung für rohe IPv4-Pakete (`DLT_RAW`)
- persistente Regeln, DNS-Einträge, Blocklisten, Capture-Metadaten und Flow-Historie
- WebUI, REST-API, OpenAPI, Health und Prometheus-Metriken
- systemd-Unit mit den benötigten Linux-Capabilities

## Warum TUN und nicht künstlich TAP?

SNDCP transportiert IP-N-PDUs und keine Ethernet-Frames. Das korrekte Kernel-Gegenstück ist deshalb ein **TUN-Interface**. Ein TAP-Interface würde zusätzlich Ethernet, ARP, MAC-Adressverwaltung und Proxying erfinden, obwohl diese Schicht auf der TETRA-Seite gar nicht existiert. Der Dienst bildet daher bewusst Layer 3 sauber ab, statt einen scheinbaren Layer-2-Support vorzutäuschen.

## Shadow und Authoritative

Standard ist:

```toml
[interface]
mode = "shadow"
```

Im Shadow-Modus werden Packet Core, PDP-Kontexte, Regeln und der vollständige Kernel-Plan angezeigt. Der Dienst öffnet aber kein TUN-Interface, löscht keine N-PDUs und verändert weder Routing noch nftables.

Für echten Pakettransport:

```toml
[interface]
mode = "authoritative"
```

Dann benötigt der LXC `/dev/net/tun`, `CAP_NET_ADMIN`, `CAP_NET_RAW` und für DNS-Port 53 `CAP_NET_BIND_SERVICE`.

## Datenweg

```text
MS / SNDCP
   ↓ Uplink-Fragmente
TBS Edge
   ↓ vollständige N-PDU
Packet Core Outbox
   ↓ HTTP Pull + ACK/Delete
IP Gateway
   ↓ write(ntc-tun0)
Linux Routing / nftables / NAT / lokale Dienste / externe Netze
   ↓ Paket mit Zieladresse aus dem TETRA-IP-Pool
read(ntc-tun0)
   ↓ IPv4 → PDP-Kontext → ISSI/NSAPI
Packet Core Downlink Queue
   ↓ Fragmentierung / Flow Control
TBS Edge → MS
```

## Ports

| Port | Funktion |
|---:|---|
| 8170/tcp | WebUI und Management-API |
| 53/udp | integrierter DNS-Forwarder |
| 8088/tcp | HTTP-, WAP- und Diagnosetestserver |
| 7007/udp | UDP-Echo |

## Open-Lab-Betrieb

Dieser Ausbau läuft absichtlich ohne Benutzerkonten, Token und TLS. Jeder Client mit Managementzugriff kann Routing, NAT, Firewall, Blocklisten und Packet Captures verändern. Nur in einem isolierten Testnetz betreiben.

## Schnellstart

Shadow-Modus ohne Konfigurationsdatei:

```bash
cargo run -p netcore-ip-gateway -- --no-config --bind 0.0.0.0:8170
```

Installation im LXC:

```bash
sudo system-backend/ip-gateway/install/install.sh
sudo editor /etc/netcore/ip-gateway.toml
sudo systemctl restart netcore-ip-gateway
```

WebUI:

```text
http://<LXC-IP>:8170/
```

## API-Auswahl

```text
GET    /api/v1/status
GET    /api/v1/contexts
GET    /api/v1/flows
GET    /api/v1/routes
POST   /api/v1/routes
PUT    /api/v1/routes/{id}
DELETE /api/v1/routes/{id}
GET    /api/v1/nat
POST   /api/v1/nat
GET    /api/v1/firewall
POST   /api/v1/firewall
GET    /api/v1/dns
POST   /api/v1/dns
GET    /api/v1/blocked
POST   /api/v1/blocked
GET    /api/v1/captures
POST   /api/v1/captures
POST   /api/v1/captures/{id}/stop
GET    /api/v1/captures/{id}/download
GET    /api/v1/kernel/plan
POST   /api/v1/kernel/reconcile
GET    /metrics
GET    /health/live
GET    /health/ready
GET    /openapi.json
```

Weitere Details stehen unter `docs/`.

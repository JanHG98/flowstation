# NetCore-Tetra – vollständige allgemeine IPv4-Paketdatenplattform

Stand: 21.07.2026

## Ergebnis

Diese Ausbaustufe setzt auf dem zuvor gelieferten vollständigen SNDCP-v1-Stand auf
und ergänzt den kompletten praktisch nutzbaren IPv4-Datenpfad oberhalb des
Funkprotokolls:

- Linux-TUN-Interface `ntetra0` für rohe IPv4-N-PDUs;
- lokale TCP-, UDP- und ICMP-Dienste des Basisstationshosts;
- IPv4-Forwarding und mobile-zu-mobile Weiterleitung über den Linux-Kernel;
- wahlweise geroutetes Teilnehmernetz oder NAT/NAPT-Masquerading;
- nftables- und iptables-Backend mit eigenen NetCore-Objekten;
- standardmäßig nur ESTABLISHED/RELATED in Richtung Teilnehmernetz;
- strikte IPv4-Prüfung, Fragmentierung und begrenzte Reassembly;
- dynamische/statische PDP-Adressen und IPCP-DNS-Aushandlung;
- Downlink-Zuordnung nach IPv4-Ziel, QoS-Filtern und erlernten Flows;
- Paging und begrenztes Downlink-Puffern für Teilnehmer in STANDBY;
- Crash-sicheres Aufräumen von Firewall und geänderten Sysctls;
- Installations-, Status-, Cleanup- und Deinstallationshelfer.

TUN ist hier bewusst richtig: SNDCP transportiert IP-N-PDUs und keine
Ethernet-Frames. TAP, ARP und eine künstliche Layer-2-Broadcastdomäne werden daher
nicht eingeführt.

## Patch-Basis

Der Patch

`netcore-tetra-packet-data-complete-2026-07-21.patch`

setzt exakt auf folgendem zuvor gelieferten Stand auf:

`netcore-tetra-sndcp-complete-2026-07-21.zip`

Für einen anderen Git-Stand ist das vollständige neue ZIP die sichere Variante.

## Aktive Standardkonfiguration

```toml
[cell_info]
sndcp_service = true
advanced_link = true

[cell_info.wap_ip]
enabled = true
address = "10.0.0.1"
dynamic_pool_prefix = "10.0.0"
dynamic_pool_first_host = 2
dynamic_pool_last_host = 254
mtu_code = 2
strict_source_address = true

[cell_info.packet_data_gateway]
enabled = true
interface_name = "ntetra0"
prefix_len = 24
auto_configure = true
enable_ipv4_forwarding = true
managed_forwarding = true
allow_unsolicited_inbound = false
nat_mode = "masquerade"
firewall_backend = "auto"
dns_servers = ["1.1.1.1", "9.9.9.9"]
channel_capacity = 256
downlink_queue_packets_per_context = 64
downlink_queue_bytes_per_context = 262144
downlink_queue_ttl_secs = 30
page_retry_secs = 5
fragment_reassembly_timeout_secs = 30
fragment_reassembly_max_datagrams = 128
fragment_reassembly_max_bytes = 4194304
automatic_filter_ttl_secs = 300
automatic_filter_max_bindings = 4096
```

Damit ist ausgehender Internetzugang per NAT aktiviert. Neue, von außen initiierte
Verbindungen zum Teilnehmernetz bleiben blockiert.

## Installation des Host-Netzteils

Benötigt werden Linux, `/dev/net/tun`, `iproute2` und entweder nftables oder
iptables.

```bash
sudo apt update
sudo apt install -y iproute2 nftables iptables tcpdump

cd ~/netcore-tetra
sudo contrib/packet-data/netcore-tetra-packet-gateway-install tetra.service
sudo systemctl cat tetra.service
```

Der Installer legt einen systemd-Drop-in und folgenden Crash-Cleanup ab:

```text
/usr/local/libexec/netcore-tetra-packet-gateway-cleanup
```

Der Drop-in ergänzt `CAP_NET_ADMIN`, erlaubt den Zugriff auf `/dev/net/tun` und
Kernel-Tunables und ruft den Cleanup auch nach einem Crash auf. Er überschreibt
absichtlich kein bereits vorhandenes `CapabilityBoundingSet`. Wenn die
Basis-Unit dort eine restriktive Liste nutzt, muss `CAP_NET_ADMIN` in diese
vorhandene Liste aufgenommen werden.

## Sauberer Build

```bash
cd ~/netcore-tetra
sudo systemctl stop tetra.service

cp -a config.toml "config.toml.pre-packet-data-$(date +%F-%H%M%S)"
cp -a config.toml.fallback "config.toml.fallback.pre-packet-data-$(date +%F-%H%M%S)"

rm -rf target
cargo clean

cargo test -p tetra-core timeslot_alloc
cargo test -p tetra-config
cargo test -p tetra-entities sndcp --features runtime

cargo build --release \
  -p bluestation-bs \
  -p netcore-control-room \
  -p netcore-control-room-operator \
  --features bluestation-bs/asterisk

sudo systemctl daemon-reload
sudo systemctl restart tetra.service
```

## Status und Diagnose

```bash
sudo contrib/packet-data/netcore-tetra-packet-gateway-status ntetra0
ip -details address show ntetra0
ip -4 route show dev ntetra0
sudo tcpdump -ni ntetra0

sudo journalctl -u tetra.service -n 500 --no-pager \
  | grep -iE 'SNDCP|PDP|PDCH|packet gateway|TUN|PAGE|IPv4|error|panic'
```

Nach erfolgreicher PDP-Aktivierung kann der Host den Teilnehmer testen:

```bash
ping -I ntetra0 <zugewiesene-Teilnehmer-IP>
```

## Betriebsarten

### NAT/NAPT – aktive Voreinstellung

```toml
managed_forwarding = true
allow_unsolicited_inbound = false
nat_mode = "masquerade"
```

### Geroutetes Netz ohne NAT

```toml
managed_forwarding = true
allow_unsolicited_inbound = false
nat_mode = "disabled"
```

Der Upstream-Router benötigt dann eine Rückroute für `10.0.0.0/24` über die
LAN-Adresse der Basisstation.

### Nur lokale Dienste auf der Basisstation

```toml
enable_ipv4_forwarding = false
managed_forwarding = false
nat_mode = "disabled"
firewall_backend = "none"
```

## Rückbau

```bash
sudo systemctl stop tetra.service
sudo contrib/packet-data/netcore-tetra-packet-gateway-uninstall tetra.service
```

Der Rückbau entfernt ausschließlich die NetCore-eigenen Firewall-Objekte und
stellt zuvor gespeicherte Sysctl-Werte wieder her. Das allgemeine Host-Firewallset
wird nicht geleert.

## Ehrliche technische Grenzen

Die allgemeine IPv4-Plattform ist vollständig für das aktuell beworbene
Single-Slot-IPv4-Profil. Nicht vorgetäuscht werden weiterhin eigenständige,
optionale TETRA-Projekte:

- IPv6-PDP-Kontexte;
- Mobile IPv4;
- RFC-1144/VJ-, RFC-2507- oder Payload-Kompression;
- Paketdaten-AIE/TEA;
- Enhanced-Multislot-PDCH und Scheduled Access;
- Multicast-/Broadcast-Fan-out an mehrere Funkteilnehmer.

Der Host-Netzstack ist allgemein nutzbar; die Funkkapazität bleibt vorerst ein
PDCH auf Hauptcarrier TS2 und ist damit der praktische Durchsatz-Flaschenhals.

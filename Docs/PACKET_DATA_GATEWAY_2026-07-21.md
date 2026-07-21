# NetCore-Tetra general IPv4 packet-data gateway

Date: 2026-07-21

## Purpose

This extension turns the completed SNDCP IPv4 profile into a practical Linux IP
network. TETRA terminals receive an IPv4 address during PDP-context activation.
Raw IPv4 N-PDUs then cross a Linux TUN interface named `ntetra0` by default.
The Linux kernel supplies ordinary local delivery, routing, TCP, UDP, ICMP,
conntrack and optional IPv4 source NAT.

TUN is intentional. SNDCP transports network-layer packets, not Ethernet
frames, so TAP, ARP and an Ethernet broadcast domain are not inserted between
the terminal and the IP stack.

## End-to-end data path

### Uplink

1. The terminal activates a primary or secondary PDP context.
2. NetCore assigns or validates its IPv4 address and negotiates the SNDCP MTU.
3. SN-DATA or SN-UNITDATA delivers an IPv4 N-PDU.
4. NetCore validates SNDCP compression flags, NSAPI state, negotiated MTU,
   IPv4 checksum, fragment structure and source address.
5. IPv4 fragments are reassembled under bounded memory and timeout limits.
6. Local WAP traffic is served directly when enabled.
7. All other accepted IPv4 packets are written to `ntetra0` and enter the Linux
   IP receive path.

### Downlink

1. The Linux kernel routes a packet to the subscriber subnet through `ntetra0`.
2. The TUN worker returns the raw IPv4 packet to the SNDCP entity.
3. NetCore selects the PDP context by destination address and, when configured,
   static or automatically learned QoS flow filters.
4. A READY context receives the packet immediately.
5. A STANDBY context is paged with SN-PAGE and receives a bounded queued packet
   set after bearer activation.
6. Packets larger than the negotiated SNDCP MTU are fragmented as IPv4 and sent
   as SN-UNITDATA. The Linux TUN MTU normally prevents oversized DF packets from
   reaching this point and lets the kernel perform normal PMTU handling.

## Address assignment and DNS

There is no DHCP exchange on the radio bearer. PDP activation is the address
assignment procedure. The existing `[cell_info.wap_ip]` SNDCP profile contains
the gateway address, dynamic pool and SNDCP MTU even when the WAP page itself is
disabled.

DNS addresses are negotiated in the Protocol Configuration Options using PPP
IPCP primary and secondary DNS options. Up to two IPv4 DNS servers can be
configured.

## Configuration

```toml
[cell_info]
sndcp_service = true
advanced_link = true

[cell_info.wap_ip]
enabled = true
address = "10.0.0.1"
port = 9200
response_ttl = 32
dynamic_pool_prefix = "10.0.0"
dynamic_pool_first_host = 2
dynamic_pool_last_host = 254
allow_static_ipv4 = true
mtu_code = 2
strict_source_address = true

[cell_info.packet_data_gateway]
enabled = true
interface_name = "ntetra0"
prefix_len = 24
# mtu = 576                 # omitted: use negotiated SNDCP MTU
auto_configure = true
enable_ipv4_forwarding = true
managed_forwarding = true
allow_unsolicited_inbound = false
nat_mode = "masquerade"     # disabled | masquerade
firewall_backend = "auto"   # auto | nftables | iptables | none
# external_interface = "eth0" # omitted: default-route interface
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

### NAT mode

`nat_mode = "masquerade"` provides normal outbound internet access through the
host's external interface. Return traffic is admitted through conntrack as
ESTABLISHED or RELATED. New unsolicited external connections are blocked unless
`allow_unsolicited_inbound = true` is set deliberately.

### Routed mode without NAT

Use:

```toml
nat_mode = "disabled"
managed_forwarding = true
```

The upstream router then needs a return route for the subscriber subnet, for
example `10.0.0.0/24 via <base-station-LAN-address>`. Keep
`allow_unsolicited_inbound = false` for stateful outbound-only routing, or set it
to true when the subscriber addresses must be directly reachable from the
external routed network.

### Local-only mode

For terminal access only to services on the base-station host:

```toml
enable_ipv4_forwarding = false
managed_forwarding = false
nat_mode = "disabled"
firewall_backend = "none"
```

The host retains `10.0.0.1/24` on `ntetra0`, but packets are not forwarded to
another interface.

## Host requirements

Required:

- Linux with TUN support and `/dev/net/tun`
- `iproute2`
- `nftables` or `iptables` when managed forwarding or NAT is enabled
- `CAP_NET_ADMIN` for the base-station service

Install the supplied systemd integration:

```bash
sudo contrib/packet-data/netcore-tetra-packet-gateway-install tetra.service
sudo systemctl cat tetra.service
sudo systemctl restart tetra.service
```

The drop-in grants ambient `CAP_NET_ADMIN`, exposes `/dev/net/tun`, permits
kernel tunables and installs an `ExecStopPost` cleanup path. It deliberately
does not replace an existing `CapabilityBoundingSet`. When the base unit already
uses a restrictive bounding set, add `CAP_NET_ADMIN` to that existing set.

Some hardened service units may additionally require `/dev/net/tun` in
`DeviceAllow=` and `AF_NETLINK` in `RestrictAddressFamilies=`.

## Operations

Status:

```bash
sudo contrib/packet-data/netcore-tetra-packet-gateway-status ntetra0
ip -details addr show ntetra0
ip route show 10.0.0.0/24
sudo nft list table ip netcore_tetra
```

The status helper falls back to the dedicated iptables chains when nftables is
not in use.

Uninstall host integration and restore managed network state:

```bash
sudo contrib/packet-data/netcore-tetra-packet-gateway-uninstall tetra.service
```

## Crash recovery

Before changing a kernel sysctl, NetCore persists the previous value under
`/run/netcore-tetra/packet-gateway-sysctls`. A normal shutdown restores values
in process. The systemd `ExecStopPost` helper also runs after crashes and removes
NetCore's nftables table or iptables chains before restoring the saved sysctls.
A subsequent start detects and restores a stale sysctl file before applying new
settings.

The TUN device is non-persistent and disappears automatically when its file
descriptor closes, including after process termination.

## Diagnostic sequence

```bash
sudo journalctl -u tetra.service -n 500 --no-pager \
  | grep -iE 'SNDCP|PDP|PDCH|packet gateway|TUN|PAGE|IPv4|error|panic'

sudo contrib/packet-data/netcore-tetra-packet-gateway-status ntetra0

# From the base-station host, after a terminal has activated PDP:
ping -I ntetra0 <terminal-address>

# Packet capture:
sudo tcpdump -ni ntetra0
```

## Practical throughput boundary

The IP stack is general-purpose, but the current radio bearer remains one PDCH
on main-carrier TS2. TCP, UDP and ICMP work as IP protocols; their throughput,
latency and application suitability remain constrained by the TETRA air
interface, signalling overhead, RF quality, retransmissions and terminal
firmware.

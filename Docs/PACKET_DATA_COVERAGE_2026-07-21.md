# Packet-data implementation coverage

Date: 2026-07-21

## Implemented end to end

| Area | Coverage |
|---|---|
| PDP activation | Dynamic/static IPv4, primary and secondary NSAPIs, SNEI, PCO |
| DNS configuration | PPP IPCP primary/secondary DNS ACK/NAK |
| SNDCP user plane | SN-DATA and SN-UNITDATA, no compression profile |
| SNDCP control plane | Types 0-13 and implemented subtypes from the complete SNDCP stage |
| Bearer lifecycle | READY, CONTEXT_READY, STANDBY, paging, reconnect, timer cleanup |
| Kernel integration | Linux TUN raw IPv4 interface |
| IPv4 validation | Version, IHL, total length, checksum, flags and fragment bounds |
| Fragment reassembly | Out-of-order, duplicate handling, overlap rejection, timeout and memory bounds |
| Fragment generation | Per-context MTU, DF refusal, copied IPv4 options |
| Local host access | TCP, UDP and ICMP through the Linux IP stack |
| Forwarding | TUN to external, mobile-to-mobile through the kernel |
| NAT/NAPT | nftables or iptables masquerade with conntrack return path |
| Routed subnet | NAT-disabled route mode with upstream return route |
| Inbound policy | Stateful by default; optional deliberate unsolicited inbound routing |
| QoS context selection | Static port/range/DiffServ filters and learned automatic flow bindings |
| Downlink while idle | Bounded queue plus SN-PAGE and flush after bearer activation |
| Crash cleanup | Dedicated firewall objects and persisted sysctl restoration |
| Operations | Install, uninstall, cleanup and status helpers |

## Delegated to the Linux kernel

Once packets cross `ntetra0`, Linux provides:

- IPv4 routing and local delivery;
- TCP state machines and retransmission;
- UDP;
- ICMP;
- PMTU behaviour based on the TUN MTU;
- conntrack;
- source NAT/port translation;
- application sockets and ordinary packet capture/filtering.

## Intentionally not emulated

### DHCP

PDP activation already assigns the terminal address and carries protocol
configuration. The raw-IP bearer has no Ethernet/ARP broadcast segment on which
normal DHCP would operate.

### Ethernet/TAP/ARP

SNDCP carries network-layer N-PDUs. Ethernet emulation would add a synthetic
layer that terminals do not send over this bearer.

## Optional TETRA profiles still not advertised

These are separate radio/SNDCP capability projects rather than missing pieces of
the IPv4 router:

- IPv6 PDP contexts;
- Mobile IPv4 foreign-agent or co-located care-of operation;
- RFC 1144/VJ and RFC 2507 header compression;
- SNDCP payload compression profiles;
- packet-data AIE/TEA;
- enhanced multi-slot PDCH and scheduled-access MAC service.

The codec recognises or negotiates several of these requests and rejects them
with the appropriate cause instead of advertising false support.

## Current radio bearer constraint

The runtime still reserves one packet-data bearer on main-carrier TS2. Multiple
NSAPIs of one ISSI may share it, but another ISSI waits until that bearer is
released. Therefore the host networking stack is general-purpose while radio
capacity remains deliberately conservative and compatible with the existing MAC
scheduler.

## Traffic classes not distributed as subscriber fan-out

The router currently maps unicast destination addresses to PDP contexts.
IPv4 multicast and subnet-directed broadcast are not replicated into separate
per-subscriber radio transmissions. They may reach local host services but are
not a group packet-data bearer.

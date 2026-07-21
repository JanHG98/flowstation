# Packet-data security and failure containment

Date: 2026-07-21

## Default security posture

The shipped configuration is outbound-oriented:

- subscriber source IPv4 addresses must match the address assigned to the PDP
  context;
- only active and available NSAPIs may inject traffic;
- malformed IPv4, bad checksums, invalid fragment layouts and overlapping
  fragments are discarded;
- external-to-subscriber forwarding permits only conntrack
  ESTABLISHED/RELATED traffic by default;
- unmatched forwarding into or out of the TUN interface is explicitly dropped,
  independent of the host's global FORWARD policy;
- NAT is restricted to the configured subscriber CIDR and selected external
  interface;
- firewall objects use the dedicated nftables table `ip netcore_tetra` or the
  dedicated iptables chains `NETCORE_TETRA_FWD` and `NETCORE_TETRA_NAT`.

`allow_unsolicited_inbound = true` deliberately changes the inbound routing
policy. It should be used only when the subscriber network is routed and direct
reachability is required.

## Resource limits

The implementation bounds:

- TUN ingress and egress channel capacity;
- queued downlink packets and bytes per PDP context;
- queued packet lifetime;
- paging retry frequency;
- incomplete fragment datagram count;
- total fragment reassembly memory;
- fragment reassembly lifetime;
- automatically learned QoS flow bindings;
- SNDCP response replay cache.

These limits prevent a weak or malfunctioning radio link from turning packet
reassembly, paging or downlink buffering into unbounded host memory growth.

## Fragmentation rules

IPv4 fragment validation rejects:

- the reserved IPv4 flag;
- non-final fragment payload lengths not divisible by eight;
- fragment ranges beyond the IPv4 datagram limit;
- overlaps and conflicting duplicate fragments;
- inconsistent final lengths;
- malformed IPv4 options.

Reassembly uses the first fragment's complete header. Outbound non-initial
fragments carry only IPv4 options with the copied bit set.

## Host-state containment

Managed network configuration records the original sysctl values before writes.
Normal shutdown and systemd crash cleanup restore them. Firewall cleanup removes
only NetCore-owned tables/chains and does not flush the host's general firewall.

The cleanup state reader accepts only `/proc/sys/net/*` paths. This prevents a
modified runtime file from being used as an arbitrary privileged write target.

## Encryption and confidentiality

General packet data does not make the TETRA air interface encrypted. Packet-data
AIE/TEA is not implemented by this gateway. Sensitive applications must use an
end-to-end protected protocol that the terminal supports, such as TLS or an
application-level cryptographic protocol. NAT is address translation, not
confidentiality.

## Recommended deployment controls

- keep `strict_source_address = true`;
- keep `allow_unsolicited_inbound = false` unless direct inbound routing is a
  documented requirement;
- use a dedicated external VLAN or router policy for the subscriber subnet;
- restrict DNS and reachable destinations upstream when internet-wide access is
  unnecessary;
- monitor `ntetra0` and SNDCP logs for unexpected flows;
- retain conservative queue and reassembly limits;
- do not expose management services on `10.0.0.1` unless intended for terminals;
- apply ordinary Linux patching and firewall governance to the base station.

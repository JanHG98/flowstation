# NetCore-Tetra SNDCP v1 implementation

Status: 2026-07-21

## Scope

This implementation provides a complete SwMI-side SNDCP v1 protocol core for the
capability profile advertised by NetCore-Tetra:

- IPv4 PDP contexts;
- no protocol-header or data compression;
- one single-slot phase-modulation PDCH on the main carrier, logical TS2;
- primary and secondary PDP contexts with NSAPI 1..14;
- multiple simultaneous NSAPIs for one subscriber;
- local IPv4/UDP WAP gateway using WTP/WSP;
- strict source-address validation by default.

All standard top-level SN-PDU types 0..13 and their standard subtypes are decoded
and encoded. Optional capabilities that are not available end-to-end in the
current NetCore radio/MAC/IP stack are parsed and rejected with the corresponding
SNDCP cause instead of being advertised as working.

## Implemented protocol families

- SN-ACTIVATE PDP CONTEXT DEMAND / ACCEPT / REJECT
- SN-DEACTIVATE PDP CONTEXT DEMAND / ACCEPT
- SN-UNITDATA and SN-DATA
- SN-DATA TRANSMIT REQUEST / RESPONSE
- SN-END OF DATA
- SN-RECONNECT, including multiple ordered NSAPIs
- SN-PAGE REQUEST / RESPONSE wire codec
- SN-NOT SUPPORTED
- SN-DATA PRIORITY ACKNOWLEDGEMENT / INFORMATION / REQUEST
- SN-MODIFY REQUEST / RESPONSE / AVAILABILITY / USAGE
- Type-2 and Type-3/4 optional information-element chains
- QoS IE including symmetric/asymmetric sets, filters, scheduled access,
  CONTEXT_READY and additional parameter blocks
- phase-modulation resource request IE

## Runtime behaviour

- Stable SNEI allocation per active ISSI.
- Dynamic IPv4 address pool and optional static IPv4 contexts.
- Secondary contexts share the primary context address.
- Configurable total and per-subscriber context limits.
- Subscriber-global READY timer and per-context CONTEXT_READY timer.
- STANDBY expiry removes every context belonging to that subscriber.
- READY expiry actively sends SN-END OF DATA and releases the PDCH.
- MM deregistration/T351 cleanup immediately releases contexts, SNEI, routes,
  retransmission cache and radio resources.
- Retransmitted requests receive an idempotent cached response for 30 seconds.
- TS2 ownership is coordinated through the shared NetCore timeslot allocator.
- Compressed N-PDUs, unsupported IP families and oversized N-PDUs are rejected.

## Deliberately not advertised

The wire codec can retain or parse the relevant negotiation data, but NetCore does
not advertise the following until the required lower or upper layers exist:

- IPv6 packet-data routing;
- Mobile IPv4 foreign-agent or co-located care-of-address service;
- RFC 1144/VJ, RFC 2507 or data-compression profiles;
- enhanced multi-slot PDCH allocation;
- scheduled-access MAC service;
- generic Internet routing, NAT or TUN/TAP forwarding;
- packet-data AIE.

This is therefore a complete SNDCP v1 implementation for the advertised
NetCore-Tetra profile, not a false claim that every optional external protocol
profile defined around SNDCP is available.

## Configuration

The active profile lives under `[cell_info.wap_ip]`:

```toml
pdu_priority_max = 4
ready_timer_code = 8
standby_timer_code = 4
response_wait_timer_code = 7
mtu_code = 2
network_default_data_priority = 4
max_contexts_per_issi = 4
max_total_contexts = 64
strict_source_address = true
```

## Validation commands

```bash
cargo test -p tetra-core timeslot_alloc
cargo test -p tetra-config
cargo test -p tetra-entities sndcp --features runtime
cargo build --release \
  -p bluestation-bs \
  -p netcore-control-room \
  -p netcore-control-room-operator \
  --features bluestation-bs/asterisk
```

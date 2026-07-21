# Paketdaten-Gateway Compile-Fix R1

Behobene Compilerfehler:

1. `IPV4_UDP_HEADER_BYTES` wird nun zentral in `sndcp/ip.rs` definiert.
2. `snei_optional_section()` erzeugt das optionale SNEI-Type-2-IE für SN-PAGE.
3. `IpError::UnsupportedProtocol(u8)` bildet nicht unterstützte IPv4-Protokolle ab.
4. Die nftables-Setup-Closure besitzt einen expliziten `Result<(), GatewayError>`-Typ.
5. WAP-Rohantworten berechnen ihr 548-Byte-Budget aus `576 - IPV4_UDP_HEADER_BYTES`.

Der Patch setzt auf `netcore-tetra-packet-data-complete-2026-07-21.zip` auf.

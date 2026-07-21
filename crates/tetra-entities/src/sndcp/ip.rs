//! Minimal IPv4/UDP primitives for the local WAP-over-SNDCP endpoint.
//! Fresh implementation from RFC 768/791/1071 and `Docs/wap-port-spec.md`.

pub const IPV4_PROTOCOL_UDP: u8 = 17;
pub const IPV4_UDP_HEADER_BYTES: usize = 28;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IpError {
    TooShort,
    NotIpv4,
    InvalidHeaderLength,
    InvalidTotalLength,
    Fragmented,
    UnsupportedProtocol(u8),
    InvalidUdpLength,
    PacketTooLarge,
}

#[derive(Debug, Clone, Copy)]
pub struct Ipv4Packet<'a> {
    pub identification: u16,
    pub ttl: u8,
    pub protocol: u8,
    pub source: [u8; 4],
    pub destination: [u8; 4],
    pub payload: &'a [u8],
}

#[derive(Debug, Clone, Copy)]
pub struct UdpDatagram<'a> {
    pub source_port: u16,
    pub destination_port: u16,
    pub payload: &'a [u8],
}

pub fn internet_checksum(bytes: &[u8]) -> u16 {
    let mut sum = 0u32;
    for chunk in bytes.chunks(2) {
        let word = if chunk.len() == 2 {
            u16::from_be_bytes([chunk[0], chunk[1]])
        } else {
            u16::from_be_bytes([chunk[0], 0])
        };
        sum = sum.wrapping_add(u32::from(word));
        while sum > 0xffff {
            sum = (sum & 0xffff) + (sum >> 16);
        }
    }
    !(sum as u16)
}

pub fn parse_ipv4_packet(packet: &[u8]) -> Result<Ipv4Packet<'_>, IpError> {
    if packet.len() < 20 {
        return Err(IpError::TooShort);
    }
    if packet[0] >> 4 != 4 {
        return Err(IpError::NotIpv4);
    }
    let header_len = usize::from(packet[0] & 0x0f) * 4;
    if header_len < 20 || header_len > packet.len() {
        return Err(IpError::InvalidHeaderLength);
    }
    let total_len = usize::from(u16::from_be_bytes([packet[2], packet[3]]));
    if total_len < header_len || total_len > packet.len() {
        return Err(IpError::InvalidTotalLength);
    }
    let fragment = u16::from_be_bytes([packet[6], packet[7]]);
    if fragment & 0x3fff != 0 {
        return Err(IpError::Fragmented);
    }
    Ok(Ipv4Packet {
        identification: u16::from_be_bytes([packet[4], packet[5]]),
        ttl: packet[8],
        protocol: packet[9],
        source: packet[12..16].try_into().expect("fixed IPv4 source"),
        destination: packet[16..20].try_into().expect("fixed IPv4 destination"),
        payload: &packet[header_len..total_len],
    })
}

pub fn parse_udp_datagram(segment: &[u8]) -> Result<UdpDatagram<'_>, IpError> {
    if segment.len() < 8 {
        return Err(IpError::TooShort);
    }
    let len = usize::from(u16::from_be_bytes([segment[4], segment[5]]));
    if len < 8 || len > segment.len() {
        return Err(IpError::InvalidUdpLength);
    }
    Ok(UdpDatagram {
        source_port: u16::from_be_bytes([segment[0], segment[1]]),
        destination_port: u16::from_be_bytes([segment[2], segment[3]]),
        payload: &segment[8..len],
    })
}

pub fn build_ipv4_udp_npdu(
    source: [u8; 4],
    destination: [u8; 4],
    source_port: u16,
    destination_port: u16,
    identification: u16,
    ttl: u8,
    payload: &[u8],
) -> Result<Vec<u8>, IpError> {
    let udp_len = 8usize.checked_add(payload.len()).ok_or(IpError::PacketTooLarge)?;
    let total_len = 20usize.checked_add(udp_len).ok_or(IpError::PacketTooLarge)?;
    if total_len > usize::from(u16::MAX) {
        return Err(IpError::PacketTooLarge);
    }

    let mut packet = vec![0u8; total_len];
    packet[0] = 0x45;
    packet[1] = 0;
    packet[2..4].copy_from_slice(&(total_len as u16).to_be_bytes());
    packet[4..6].copy_from_slice(&identification.to_be_bytes());
    packet[6..8].copy_from_slice(&0u16.to_be_bytes());
    packet[8] = ttl;
    packet[9] = IPV4_PROTOCOL_UDP;
    packet[12..16].copy_from_slice(&source);
    packet[16..20].copy_from_slice(&destination);
    let checksum = internet_checksum(&packet[..20]);
    packet[10..12].copy_from_slice(&checksum.to_be_bytes());

    packet[20..22].copy_from_slice(&source_port.to_be_bytes());
    packet[22..24].copy_from_slice(&destination_port.to_be_bytes());
    packet[24..26].copy_from_slice(&(udp_len as u16).to_be_bytes());
    packet[26..28].copy_from_slice(&0u16.to_be_bytes()); // RFC 768: zero means omitted for IPv4.
    packet[28..].copy_from_slice(payload);
    Ok(packet)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn builds_reference_ipv4_udp_packet() {
        let p = build_ipv4_udp_npdu(
            [192, 0, 2, 1],
            [192, 0, 2, 2],
            49152,
            9200,
            7,
            32,
            b"wap",
        )
        .unwrap();
        assert_eq!(hex(&p), "4500001f00070000201116c4c0000201c0000202c00023f0000b0000776170");
        let ip = parse_ipv4_packet(&p).unwrap();
        let udp = parse_udp_datagram(ip.payload).unwrap();
        assert_eq!(udp.payload, b"wap");
        assert_eq!(&p[26..28], &[0, 0]);
    }

    #[test]
    fn checksum_of_header_with_checksum_is_zero() {
        let p = build_ipv4_udp_npdu([10, 0, 0, 1], [10, 0, 0, 2], 9200, 49152, 0x2223, 32, b"x").unwrap();
        assert_eq!(internet_checksum(&p[..20]), 0);
    }
}

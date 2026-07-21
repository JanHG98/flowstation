//! Minimal IPv4/UDP primitives used by the TETRA SNDCP WAP endpoint.
//!
//! The implementation is intentionally dependency-free and keeps the wire format explicit:
//! IPv4 without options, UDP checksum disabled (legal for IPv4), network byte order throughout.

pub const IPV4_PROTOCOL_UDP: u8 = 17;
const IPV4_HEADER_LEN: usize = 20;
const UDP_HEADER_LEN: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IpError {
    Ipv4TooShort,
    UnsupportedVersion(u8),
    InvalidHeaderLength(usize),
    InvalidTotalLength { total: usize, available: usize },
    Fragmented,
    UdpTooShort,
    InvalidUdpLength { total: usize, available: usize },
    PacketTooLarge(usize),
}

#[derive(Debug, Clone, Copy)]
pub struct Ipv4Packet<'a> {
    pub source: [u8; 4],
    pub destination: [u8; 4],
    pub protocol: u8,
    pub identification: u16,
    pub ttl: u8,
    pub fragmented: bool,
    pub payload: &'a [u8],
}

#[derive(Debug, Clone, Copy)]
pub struct UdpDatagram<'a> {
    pub source_port: u16,
    pub destination_port: u16,
    pub payload: &'a [u8],
}

pub fn parse_ipv4_packet(packet: &[u8]) -> Result<Ipv4Packet<'_>, IpError> {
    if packet.len() < IPV4_HEADER_LEN {
        return Err(IpError::Ipv4TooShort);
    }
    let version = packet[0] >> 4;
    if version != 4 {
        return Err(IpError::UnsupportedVersion(version));
    }
    let header_len = ((packet[0] & 0x0f) as usize) * 4;
    if header_len < IPV4_HEADER_LEN || header_len > packet.len() {
        return Err(IpError::InvalidHeaderLength(header_len));
    }
    let total_len = u16::from_be_bytes([packet[2], packet[3]]) as usize;
    if total_len < header_len || total_len > packet.len() {
        return Err(IpError::InvalidTotalLength {
            total: total_len,
            available: packet.len(),
        });
    }
    let flags_fragment = u16::from_be_bytes([packet[6], packet[7]]);
    let fragmented = (flags_fragment & 0x3fff) != 0;
    Ok(Ipv4Packet {
        source: [packet[12], packet[13], packet[14], packet[15]],
        destination: [packet[16], packet[17], packet[18], packet[19]],
        protocol: packet[9],
        identification: u16::from_be_bytes([packet[4], packet[5]]),
        ttl: packet[8],
        fragmented,
        payload: &packet[header_len..total_len],
    })
}

pub fn parse_udp_datagram(segment: &[u8]) -> Result<UdpDatagram<'_>, IpError> {
    if segment.len() < UDP_HEADER_LEN {
        return Err(IpError::UdpTooShort);
    }
    let total_len = u16::from_be_bytes([segment[4], segment[5]]) as usize;
    if total_len < UDP_HEADER_LEN || total_len > segment.len() {
        return Err(IpError::InvalidUdpLength {
            total: total_len,
            available: segment.len(),
        });
    }
    Ok(UdpDatagram {
        source_port: u16::from_be_bytes([segment[0], segment[1]]),
        destination_port: u16::from_be_bytes([segment[2], segment[3]]),
        payload: &segment[UDP_HEADER_LEN..total_len],
    })
}

pub fn build_ipv4_udp_npdu(
    source: [u8; 4],
    destination: [u8; 4],
    source_port: u16,
    destination_port: u16,
    payload: &[u8],
    identification: u16,
    ttl: u8,
) -> Result<Vec<u8>, IpError> {
    let udp_len = UDP_HEADER_LEN + payload.len();
    let total_len = IPV4_HEADER_LEN + udp_len;
    if total_len > u16::MAX as usize {
        return Err(IpError::PacketTooLarge(total_len));
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
    let checksum = internet_checksum(&packet[..IPV4_HEADER_LEN]);
    packet[10..12].copy_from_slice(&checksum.to_be_bytes());

    let udp = &mut packet[IPV4_HEADER_LEN..];
    udp[0..2].copy_from_slice(&source_port.to_be_bytes());
    udp[2..4].copy_from_slice(&destination_port.to_be_bytes());
    udp[4..6].copy_from_slice(&(udp_len as u16).to_be_bytes());
    udp[6..8].copy_from_slice(&0u16.to_be_bytes()); // IPv4 permits an omitted UDP checksum.
    udp[8..].copy_from_slice(payload);
    Ok(packet)
}

pub fn internet_checksum(data: &[u8]) -> u16 {
    let mut sum = 0u32;
    let mut chunks = data.chunks_exact(2);
    for chunk in &mut chunks {
        sum += u16::from_be_bytes([chunk[0], chunk[1]]) as u32;
        sum = (sum & 0xffff) + (sum >> 16);
    }
    if let Some(&last) = chunks.remainder().first() {
        sum += (last as u32) << 8;
        sum = (sum & 0xffff) + (sum >> 16);
    }
    while sum >> 16 != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_reference_udp_vector() {
        let packet = build_ipv4_udp_npdu([192, 0, 2, 1], [192, 0, 2, 2], 49_152, 9_200, b"wap", 7, 32).unwrap();
        assert_eq!(packet, hex("4500001f00070000201116c4c0000201c0000202c00023f0000b0000776170"));
        assert_eq!(internet_checksum(&packet[..20]), 0);
    }

    #[test]
    fn parser_honours_ipv4_and_udp_lengths() {
        let mut packet = build_ipv4_udp_npdu([10, 0, 0, 2], [10, 0, 0, 1], 49_152, 9_200, b"GET /", 9, 64).unwrap();
        packet.extend_from_slice(&[0xaa, 0xbb, 0xcc]);

        let ip = parse_ipv4_packet(&packet).unwrap();
        assert_eq!(ip.source, [10, 0, 0, 2]);
        assert_eq!(ip.destination, [10, 0, 0, 1]);
        assert_eq!(ip.identification, 9);
        let udp = parse_udp_datagram(ip.payload).unwrap();
        assert_eq!(udp.source_port, 49_152);
        assert_eq!(udp.destination_port, 9_200);
        assert_eq!(udp.payload, b"GET /");
    }

    fn hex(value: &str) -> Vec<u8> {
        value
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }
}

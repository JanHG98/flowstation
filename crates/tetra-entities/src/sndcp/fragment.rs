//! Bounded IPv4 fragmentation/reassembly at the SNDCP edge.

use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

use super::ip::{internet_checksum, parse_ipv4_packet_any, IpError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct FragmentKey {
    source: [u8; 4],
    destination: [u8; 4],
    identification: u16,
    protocol: u8,
}

#[derive(Debug)]
struct FragmentSet {
    created_at: Instant,
    updated_at: Instant,
    header: Option<Vec<u8>>,
    fragments: BTreeMap<usize, Vec<u8>>,
    final_payload_len: Option<usize>,
    stored_bytes: usize,
}

#[derive(Debug)]
pub struct Ipv4Reassembler {
    timeout: Duration,
    max_datagrams: usize,
    max_bytes: usize,
    stored_bytes: usize,
    sets: HashMap<FragmentKey, FragmentSet>,
}

impl Ipv4Reassembler {
    pub fn new(timeout: Duration, max_datagrams: usize, max_bytes: usize) -> Self {
        Self {
            timeout,
            max_datagrams: max_datagrams.max(1),
            max_bytes: max_bytes.max(65_535),
            stored_bytes: 0,
            sets: HashMap::new(),
        }
    }

    pub fn push(&mut self, packet: &[u8], now: Instant) -> Result<Option<Vec<u8>>, IpError> {
        self.sweep(now);
        let parsed = parse_ipv4_packet_any(packet)?;
        if !parsed.is_fragmented() {
            return Ok(Some(packet[..parsed.total_len].to_vec()));
        }
        let key = FragmentKey {
            source: parsed.source,
            destination: parsed.destination,
            identification: parsed.identification,
            protocol: parsed.protocol,
        };
        if !self.sets.contains_key(&key) && self.sets.len() >= self.max_datagrams {
            self.evict_oldest();
        }
        let payload = parsed.payload.to_vec();
        if payload.len() > self.max_bytes {
            return Err(IpError::PacketTooLarge);
        }
        let offset = parsed.fragment_offset_bytes;
        let end = offset.checked_add(payload.len()).ok_or(IpError::PacketTooLarge)?;

        let mut duplicate = false;
        let mut invalid = false;
        if let Some(set) = self.sets.get(&key) {
            invalid |= !parsed.more_fragments && set.final_payload_len.is_some_and(|known| known != end);
            invalid |= set.final_payload_len.is_some_and(|final_len| end > final_len);
            invalid |= !parsed.more_fragments
                && set.fragments.iter().any(|(existing_offset, existing)| {
                    existing_offset.saturating_add(existing.len()) > end
                });
            if let Some(existing) = set.fragments.get(&offset) {
                duplicate = existing == &payload;
                invalid |= !duplicate;
            } else {
                invalid |= set.fragments.iter().any(|(existing_offset, existing)| {
                    let existing_end = existing_offset.saturating_add(existing.len());
                    offset < existing_end && *existing_offset < end
                });
            }
        }
        if invalid {
            self.remove(key);
            return Err(IpError::OverlappingFragments);
        }
        if duplicate {
            let complete = self.try_complete(key)?;
            if complete.is_some() {
                self.remove(key);
            }
            return Ok(complete);
        }

        {
            let set = self.sets.entry(key).or_insert_with(|| FragmentSet {
                created_at: now,
                updated_at: now,
                header: None,
                fragments: BTreeMap::new(),
                final_payload_len: None,
                stored_bytes: 0,
            });
            set.updated_at = now;
            if offset == 0 {
                set.header = Some(packet[..parsed.header_len].to_vec());
            }
            if !parsed.more_fragments {
                set.final_payload_len = Some(end);
            }
            set.fragments.insert(offset, payload.clone());
            set.stored_bytes = set.stored_bytes.saturating_add(payload.len());
        }
        self.stored_bytes = self.stored_bytes.saturating_add(payload.len());
        while self.stored_bytes > self.max_bytes {
            self.evict_oldest();
            if !self.sets.contains_key(&key) {
                return Ok(None);
            }
        }
        let complete = self.try_complete(key)?;
        if complete.is_some() {
            self.remove(key);
        }
        Ok(complete)
    }

    pub fn sweep(&mut self, now: Instant) {
        let expired = self.sets.iter().filter_map(|(key, set)| {
            (now.duration_since(set.updated_at) >= self.timeout).then_some(*key)
        }).collect::<Vec<_>>();
        for key in expired {
            self.remove(key);
        }
    }

    fn try_complete(&self, key: FragmentKey) -> Result<Option<Vec<u8>>, IpError> {
        let Some(set) = self.sets.get(&key) else { return Ok(None); };
        let (Some(header), Some(final_len)) = (set.header.as_ref(), set.final_payload_len) else { return Ok(None); };
        let mut cursor = 0usize;
        for (offset, payload) in &set.fragments {
            if *offset > cursor {
                return Ok(None);
            }
            cursor = cursor.max(offset.saturating_add(payload.len()));
        }
        if cursor < final_len {
            return Ok(None);
        }
        let total_len = header.len().checked_add(final_len).ok_or(IpError::PacketTooLarge)?;
        if total_len > u16::MAX as usize {
            return Err(IpError::PacketTooLarge);
        }
        let mut packet = vec![0u8; total_len];
        packet[..header.len()].copy_from_slice(header);
        for (offset, payload) in &set.fragments {
            let start = header.len() + *offset;
            let end = (start + payload.len()).min(packet.len());
            if start < end {
                packet[start..end].copy_from_slice(&payload[..end - start]);
            }
        }
        packet[2..4].copy_from_slice(&(total_len as u16).to_be_bytes());
        packet[6..8].copy_from_slice(&0u16.to_be_bytes());
        packet[10..12].copy_from_slice(&0u16.to_be_bytes());
        let checksum = internet_checksum(&packet[..header.len()]);
        packet[10..12].copy_from_slice(&checksum.to_be_bytes());
        Ok(Some(packet))
    }

    fn remove(&mut self, key: FragmentKey) {
        if let Some(set) = self.sets.remove(&key) {
            self.stored_bytes = self.stored_bytes.saturating_sub(set.stored_bytes);
        }
    }

    fn evict_oldest(&mut self) {
        let oldest = self.sets.iter().min_by_key(|(_, set)| set.created_at).map(|(key, _)| *key);
        if let Some(key) = oldest {
            self.remove(key);
        }
    }
}

pub fn fragment_ipv4_packet(packet: &[u8], mtu: usize) -> Result<Vec<Vec<u8>>, IpError> {
    let parsed = parse_ipv4_packet_any(packet)?;
    if parsed.total_len <= mtu {
        return Ok(vec![packet[..parsed.total_len].to_vec()]);
    }
    if parsed.dont_fragment {
        return Err(IpError::DontFragment);
    }
    if parsed.is_fragmented() {
        return Err(IpError::Fragmented);
    }

    let first_header = packet[..parsed.header_len].to_vec();
    let copied_options = copied_ipv4_options(&first_header[20..])?;
    let mut later_header = first_header[..20].to_vec();
    later_header.extend_from_slice(&copied_options);
    later_header[0] = (4 << 4) | u8::try_from(later_header.len() / 4).map_err(|_| IpError::InvalidOptions)?;

    let mut fragments = Vec::new();
    let mut offset = 0usize;
    while offset < parsed.payload.len() {
        let header = if offset == 0 { &first_header } else { &later_header };
        if mtu <= header.len() {
            return Err(IpError::MtuTooSmall);
        }
        let max_payload = mtu - header.len();
        let remaining = parsed.payload.len() - offset;
        let payload_len = if remaining <= max_payload {
            remaining
        } else {
            (max_payload / 8) * 8
        };
        if payload_len == 0 {
            return Err(IpError::MtuTooSmall);
        }
        let more = offset + payload_len < parsed.payload.len();
        let total_len = header.len() + payload_len;
        let mut fragment = vec![0u8; total_len];
        fragment[..header.len()].copy_from_slice(header);
        fragment[header.len()..].copy_from_slice(&parsed.payload[offset..offset + payload_len]);
        fragment[2..4].copy_from_slice(&(total_len as u16).to_be_bytes());
        let mut flags_offset = (offset / 8) as u16;
        if more {
            flags_offset |= 0x2000;
        }
        fragment[6..8].copy_from_slice(&flags_offset.to_be_bytes());
        fragment[10..12].copy_from_slice(&0u16.to_be_bytes());
        let checksum = internet_checksum(&fragment[..header.len()]);
        fragment[10..12].copy_from_slice(&checksum.to_be_bytes());
        fragments.push(fragment);
        offset += payload_len;
    }
    Ok(fragments)
}

/// Return only IPv4 options whose copied bit is set, padded to a 32-bit boundary.
/// EOL and NOP are parsed defensively but are not copied to non-initial fragments.
fn copied_ipv4_options(options: &[u8]) -> Result<Vec<u8>, IpError> {
    let mut copied = Vec::new();
    let mut cursor = 0usize;
    while cursor < options.len() {
        let option_type = options[cursor];
        match option_type {
            0 => break,
            1 => cursor += 1,
            _ => {
                let Some(length) = options.get(cursor + 1).copied().map(usize::from) else {
                    return Err(IpError::InvalidOptions);
                };
                if length < 2 || cursor.checked_add(length).is_none_or(|end| end > options.len()) {
                    return Err(IpError::InvalidOptions);
                }
                if option_type & 0x80 != 0 {
                    copied.extend_from_slice(&options[cursor..cursor + length]);
                }
                cursor += length;
            }
        }
    }
    while copied.len() % 4 != 0 {
        copied.push(0);
    }
    if copied.len() > 40 {
        return Err(IpError::InvalidOptions);
    }
    Ok(copied)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sndcp::ip::build_ipv4_packet;

    #[test]
    fn fragments_and_reassembles_out_of_order() {
        let payload = (0..1200).map(|value| (value & 0xff) as u8).collect::<Vec<_>>();
        let packet = build_ipv4_packet([10, 0, 0, 2], [198, 51, 100, 7], 17, 0x1234, 32, false, &payload).unwrap();
        let mut fragments = fragment_ipv4_packet(&packet, 300).unwrap();
        fragments.reverse();
        let mut reassembler = Ipv4Reassembler::new(Duration::from_secs(30), 8, 65_535);
        let now = Instant::now();
        let mut result = None;
        for fragment in fragments { result = reassembler.push(&fragment, now).unwrap().or(result); }
        assert_eq!(result.unwrap(), packet);
    }

    #[test]
    fn refuses_df_packet_that_needs_fragmentation() {
        let packet = build_ipv4_packet([10, 0, 0, 2], [198, 51, 100, 7], 17, 1, 32, true, &vec![0; 1000]).unwrap();
        assert_eq!(fragment_ipv4_packet(&packet, 300), Err(IpError::DontFragment));
    }

    #[test]
    fn duplicate_final_fragment_completes_once_and_releases_state() {
        let payload = vec![0x5a; 900];
        let packet = build_ipv4_packet([10, 0, 0, 2], [198, 51, 100, 7], 17, 0x2222, 32, false, &payload).unwrap();
        let fragments = fragment_ipv4_packet(&packet, 300).unwrap();
        let mut reassembler = Ipv4Reassembler::new(Duration::from_secs(30), 8, 65_535);
        let now = Instant::now();
        assert!(reassembler.push(fragments.last().unwrap(), now).unwrap().is_none());
        assert!(reassembler.push(fragments.last().unwrap(), now).unwrap().is_none());
        let mut completed = None;
        for fragment in &fragments[..fragments.len() - 1] {
            completed = reassembler.push(fragment, now).unwrap().or(completed);
        }
        assert_eq!(completed.unwrap(), packet);
        assert_eq!(reassembler.stored_bytes, 0);
        assert!(reassembler.sets.is_empty());
    }

    #[test]
    fn only_copied_ipv4_options_are_present_after_first_fragment() {
        let payload = vec![0x44; 700];
        let mut packet = build_ipv4_packet([10, 0, 0, 2], [198, 51, 100, 7], 17, 0x3333, 32, false, &payload).unwrap();
        // Copied option 0x82/len4, followed by non-copied option 0x02/len4.
        packet.splice(20..20, [0x82, 4, 0xaa, 0xbb, 0x02, 4, 0xcc, 0xdd]);
        packet[0] = 0x47;
        packet[2..4].copy_from_slice(&(packet.len() as u16).to_be_bytes());
        packet[10..12].copy_from_slice(&0u16.to_be_bytes());
        let checksum = internet_checksum(&packet[..28]);
        packet[10..12].copy_from_slice(&checksum.to_be_bytes());

        let fragments = fragment_ipv4_packet(&packet, 220).unwrap();
        assert!(fragments.len() > 1);
        assert_eq!(fragments[0][0] & 0x0f, 7);
        assert_eq!(fragments[1][0] & 0x0f, 6);
        assert_eq!(&fragments[1][20..24], &[0x82, 4, 0xaa, 0xbb]);

        let mut reassembler = Ipv4Reassembler::new(Duration::from_secs(30), 8, 65_535);
        let now = Instant::now();
        let mut result = None;
        for fragment in fragments {
            result = reassembler.push(&fragment, now).unwrap().or(result);
        }
        assert_eq!(result.unwrap(), packet);
    }
}

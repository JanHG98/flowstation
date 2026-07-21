//! WAP 1.x/2.0 WTP/WSP adapter over IPv4/UDP.

use super::ip::{IPV4_PROTOCOL_UDP, IPV4_UDP_HEADER_BYTES, IpError, build_ipv4_udp_npdu, parse_ipv4_packet, parse_udp_datagram};
use super::wap_status::{
    WapStatusSnapshot, render_browser_wml, render_browser_wml_sector, render_browser_xhtml,
    render_browser_xhtml_sector, render_raw_xhtml,
};

const WTP_PDU_INVOKE: u8 = 1;
const WTP_PDU_ACK: u8 = 3;
const WTP_PDU_ABORT: u8 = 4;
const WTP_TID_RESPONSE_FLAG: u16 = 0x8000;
const WTP_TID_VALUE_MASK: u16 = 0x7fff;
const WSP_CONNECT: u8 = 0x01;
const WSP_REPLY: u8 = 0x04;
const WSP_RESUME: u8 = 0x09;
const WSP_GET: u8 = 0x40;
const WSP_CONTENT_WML: u8 = 0x88;
const WSP_CONTENT_XHTML: u8 = 0xc5;
const WSP_SDU_CAP: usize = 545;

#[derive(Debug, Clone, Copy)]
pub struct WapEndpoint {
    pub address: [u8; 4],
    pub port: u16,
    pub ttl: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct WapPolicy {
    pub accept_empty_probe: bool,
    pub accept_root_path: bool,
    pub accept_status_path: bool,
    pub accept_status_wml_path: bool,
    pub max_request_payload_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WapError {
    Ip(IpError),
    WrongDestination,
    WrongPort,
    PayloadTooLarge,
    UnsupportedPayload,
    UnsupportedPath,
    NoResponseRequired,
}

impl From<IpError> for WapError {
    fn from(value: IpError) -> Self {
        Self::Ip(value)
    }
}

fn read_uintvar(bytes: &[u8], offset: &mut usize) -> Option<usize> {
    let mut value = 0usize;
    for _ in 0..5 {
        let b = *bytes.get(*offset)?;
        *offset += 1;
        value = value.checked_shl(7)?.checked_add(usize::from(b & 0x7f))?;
        if b & 0x80 == 0 {
            return Some(value);
        }
    }
    None
}

fn write_uintvar(mut value: usize, out: &mut Vec<u8>) {
    let mut tmp = [0u8; 5];
    let mut i = tmp.len();
    loop {
        i -= 1;
        tmp[i] = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            break;
        }
    }
    for pos in i..tmp.len() {
        let mut b = tmp[pos];
        if pos + 1 != tmp.len() {
            b |= 0x80;
        }
        out.push(b);
    }
}

fn skip_tpis(payload: &[u8], mut offset: usize, con: bool) -> Option<usize> {
    if !con {
        return Some(offset);
    }
    loop {
        let h = *payload.get(offset)?;
        let cont = h & 0x80 != 0;
        if h & 0x04 != 0 {
            let len = usize::from(*payload.get(offset + 1)?);
            offset = offset.checked_add(2 + len)?;
        } else {
            offset = offset.checked_add(1 + usize::from(h & 0x03))?;
        }
        if !cont {
            return Some(offset);
        }
    }
}

fn parse_connect_caps(wsp: &[u8]) -> Option<Vec<(u8, usize)>> {
    if wsp.first().copied()? != WSP_CONNECT {
        return None;
    }
    let mut off = 2; // type + version
    let caps_len = read_uintvar(wsp, &mut off)?;
    let headers_len = read_uintvar(wsp, &mut off)?;
    if off.checked_add(caps_len + headers_len)? > wsp.len() {
        return None;
    }
    let end = off + caps_len;
    let mut caps = Vec::new();
    while off < end {
        let len = read_uintvar(wsp, &mut off)?;
        if len == 0 || off + len > end {
            return None;
        }
        let id = wsp[off];
        let params = &wsp[off + 1..off + len];
        let mut p = 0;
        let value = if params.is_empty() {
            0
        } else {
            let value = read_uintvar(params, &mut p).unwrap_or(0);
            if p != params.len() {
                off += len;
                continue;
            }
            value
        };
        caps.push((id, value));
        off += len;
    }
    Some(caps)
}

fn connect_reply(caps: &[(u8, usize)]) -> Vec<u8> {
    let mut encoded = Vec::new();
    for (id, requested) in caps {
        if !matches!(*id, 0x80 | 0x81) {
            continue;
        }
        let mut params = Vec::new();
        write_uintvar((*requested).min(WSP_SDU_CAP), &mut params);
        write_uintvar(1 + params.len(), &mut encoded);
        encoded.push(*id);
        encoded.extend_from_slice(&params);
    }
    let mut out = vec![0x02, 0x01];
    write_uintvar(encoded.len(), &mut out);
    out.push(0x00);
    out.extend_from_slice(&encoded);
    out
}

fn parse_path_from_wsp_get(wsp: &[u8]) -> Option<String> {
    let first = *wsp.first()?;
    if first != WSP_GET && !(0x50..=0x5f).contains(&first) {
        return None;
    }
    // WSP GET encodes the URI as: method octet, uintvar URI length, raw URI bytes.
    let mut off = 1;
    let uri_len = read_uintvar(wsp, &mut off)?;
    let end = off.checked_add(uri_len)?;
    let uri = std::str::from_utf8(wsp.get(off..end)?).ok()?;
    Some(uri.trim().to_string())
}

fn uri_path(uri: &str) -> &str {
    let uri = uri.trim();
    if let Some(rest) = uri.strip_prefix("http://").or_else(|| uri.strip_prefix("https://")) {
        return rest.find('/').map(|idx| &rest[idx..]).unwrap_or("/");
    }
    uri
}

fn normalize_path(uri: &str) -> String {
    let path = uri_path(uri).trim();
    let path = path.split(['?', '#']).next().unwrap_or(path).trim();
    let path = path.strip_prefix("./").unwrap_or(path);
    if path.is_empty() {
        "/".to_string()
    } else if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    }
}

fn path_allowed(uri: &str, policy: WapPolicy) -> Option<bool> {
    let base = normalize_path(uri);
    if base == "/" && policy.accept_root_path {
        Some(false)
    } else if matches!(base.as_str(), "/status" | "/status.xhtml") && policy.accept_status_path {
        Some(false)
    } else if base == "/status.wml" && policy.accept_status_wml_path {
        Some(true)
    } else {
        None
    }
}

fn wsp_status_reply(path: &str, policy: WapPolicy, snapshot: &WapStatusSnapshot) -> Result<Vec<u8>, WapError> {
    let wml = path_allowed(path, policy).ok_or(WapError::UnsupportedPath)?;
    let sector = path.split('#').next().unwrap_or(path).contains("?s=");
    let (ct, body) = if wml {
        let body = if sector { render_browser_wml_sector(snapshot, 144) } else { render_browser_wml(snapshot, 144) };
        (WSP_CONTENT_WML, body)
    } else if sector {
        (WSP_CONTENT_XHTML, render_browser_xhtml_sector(snapshot, 144))
    } else {
        (WSP_CONTENT_XHTML, render_browser_xhtml(snapshot, 104))
    };
    let mut out = vec![WSP_REPLY, 0x20, 0x01, ct];
    out.extend_from_slice(body.as_bytes());
    Ok(out)
}

fn wtp_result(tid: u16, wsp: &[u8]) -> Vec<u8> {
    let tid = (tid & WTP_TID_VALUE_MASK) | WTP_TID_RESPONSE_FLAG;
    let mut out = vec![0x12];
    out.extend_from_slice(&tid.to_be_bytes());
    out.extend_from_slice(wsp);
    out
}

fn handle_wtp(payload: &[u8], policy: WapPolicy, snapshot: &WapStatusSnapshot) -> Result<Option<Vec<u8>>, WapError> {
    if payload.len() < 3 {
        return Err(WapError::UnsupportedPayload);
    }
    let pdu_type = (payload[0] >> 3) & 0x0f;
    let tid = u16::from_be_bytes([payload[1], payload[2]]) & WTP_TID_VALUE_MASK;
    match pdu_type {
        WTP_PDU_ACK | WTP_PDU_ABORT => return Ok(None),
        WTP_PDU_INVOKE if payload.len() >= 4 => {}
        WTP_PDU_INVOKE => return Err(WapError::UnsupportedPayload),
        _ => return Err(WapError::UnsupportedPayload),
    }
    let invoke = payload[3];
    if invoke & 0xc0 != 0 || invoke & 0x0c != 0 || invoke & 0x03 != 2 {
        return Err(WapError::UnsupportedPayload);
    }
    let con = payload[0] & 0x80 != 0;
    let wsp_start = skip_tpis(payload, 4, con).ok_or(WapError::UnsupportedPayload)?;
    let wsp = payload.get(wsp_start..).ok_or(WapError::UnsupportedPayload)?;
    let response = match wsp.first().copied() {
        Some(WSP_CONNECT) => connect_reply(&parse_connect_caps(wsp).ok_or(WapError::UnsupportedPayload)?),
        Some(WSP_RESUME) => vec![WSP_REPLY, 0x20, 0x00],
        Some(method) if method == WSP_GET || (0x50..=0x5f).contains(&method) => {
            let path = parse_path_from_wsp_get(wsp).ok_or(WapError::UnsupportedPayload)?;
            wsp_status_reply(&path, policy, snapshot)?
        }
        _ => return Err(WapError::UnsupportedPayload),
    };
    Ok(Some(wtp_result(tid, &response)))
}

fn plain_get_path(payload: &[u8]) -> Option<&str> {
    let text = std::str::from_utf8(payload).ok()?;
    let mut parts = text.split_whitespace();
    if parts.next()? != "GET" {
        return None;
    }
    parts.next()
}

pub fn build_response_npdu(
    request_npdu: &[u8],
    endpoint: WapEndpoint,
    policy: WapPolicy,
    snapshot: &WapStatusSnapshot,
) -> Result<Option<Vec<u8>>, WapError> {
    let ip = parse_ipv4_packet(request_npdu)?;
    if ip.protocol != IPV4_PROTOCOL_UDP {
        return Err(WapError::Ip(IpError::UnsupportedProtocol(ip.protocol)));
    }
    if ip.destination != endpoint.address {
        return Err(WapError::WrongDestination);
    }
    let udp = parse_udp_datagram(ip.payload)?;
    if udp.destination_port != endpoint.port {
        return Err(WapError::WrongPort);
    }
    if udp.payload.len() > policy.max_request_payload_bytes {
        return Err(WapError::PayloadTooLarge);
    }

    let response_payload = if udp.payload.is_empty() {
        if !policy.accept_empty_probe {
            return Err(WapError::UnsupportedPayload);
        }
        Some(render_raw_xhtml(snapshot, 548).into_bytes())
    } else {
        // Binary WTP first. Byte 0 encodes a known WTP type in bits 6..3.
        let wtp_type = (udp.payload[0] >> 3) & 0x0f;
        if matches!(wtp_type, WTP_PDU_INVOKE | WTP_PDU_ACK | WTP_PDU_ABORT) {
            handle_wtp(udp.payload, policy, snapshot)?
        } else if let Some(path) = plain_get_path(udp.payload) {
            let _ = path_allowed(path, policy).ok_or(WapError::UnsupportedPath)?;
            Some(render_raw_xhtml(snapshot, 548).into_bytes())
        } else {
            return Err(WapError::UnsupportedPayload);
        }
    };

    let Some(response_payload) = response_payload else {
        return Ok(None);
    };
    let npdu = build_ipv4_udp_npdu(
        endpoint.address,
        ip.source,
        endpoint.port,
        udp.source_port,
        ip.identification.wrapping_add(1),
        endpoint.ttl,
        &response_payload,
    )?;
    if npdu.len() > 576 {
        return Err(WapError::PayloadTooLarge);
    }
    Ok(Some(npdu))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> WapStatusSnapshot {
        WapStatusSnapshot {
            title: "NetCore-Tetra".into(),
            state: "ONLINE".into(),
            version: "v1.3.0".into(),
            registered_ms: 2,
            attached_groups: 1,
            active_calls: 0,
            queued_sds: 0,
            uptime_secs: 93784,
            last_activity: "WAP 4010001".into(),
            health: "OK".into(),
        }
    }

    #[test]
    fn connect_reply_matches_reference_vector() {
        let caps = vec![(0x80, 327_680), (0x81, 327_680), (0x82, 0xf0)];
        assert_eq!(
            wtp_result(0x13cc, &connect_reply(&caps)),
            vec![0x12, 0x93, 0xcc, 0x02, 0x01, 0x08, 0x00, 0x03, 0x80, 0x84, 0x21, 0x03, 0x81, 0x84, 0x21]
        );
    }



    #[test]
    fn full_connect_invoke_produces_reference_reply() {
        // Invoke, TID 0x13cc, class 2, WSP Connect v1.0. Only the two SDU
        // capabilities are echoed; both are clamped from 327680 to 545.
        let request = [
            0x0b, 0x13, 0xcc, 0x12, 0x01, 0x10, 0x0a, 0x00,
            0x04, 0x80, 0x94, 0x80, 0x00,
            0x04, 0x81, 0x94, 0x80, 0x00,
        ];
        let policy = WapPolicy {
            accept_empty_probe: true,
            accept_root_path: true,
            accept_status_path: true,
            accept_status_wml_path: true,
            max_request_payload_bytes: 1024,
        };
        assert_eq!(
            handle_wtp(&request, policy, &snapshot()).unwrap().unwrap(),
            vec![0x12, 0x93, 0xcc, 0x02, 0x01, 0x08, 0x00, 0x03, 0x80, 0x84, 0x21, 0x03, 0x81, 0x84, 0x21]
        );
    }

    #[test]
    fn wsp_get_uses_uintvar_uri_and_returns_xhtml() {
        let uri = b"http://10.0.0.1:9200/status.xhtml";
        let mut request = vec![0x0b, 0x12, 0x34, 0x12, WSP_GET, uri.len() as u8];
        request.extend_from_slice(uri);
        let policy = WapPolicy {
            accept_empty_probe: true,
            accept_root_path: true,
            accept_status_path: true,
            accept_status_wml_path: true,
            max_request_payload_bytes: 1024,
        };
        let response = handle_wtp(&request, policy, &snapshot()).unwrap().unwrap();
        assert_eq!(&response[..7], &[0x12, 0x92, 0x34, 0x04, 0x20, 0x01, WSP_CONTENT_XHTML]);
        assert!(response.len() <= 3 + 4 + 104);
    }

    #[test]
    fn endpoint_response_swaps_addresses_ports_and_increments_id() {
        let endpoint = WapEndpoint { address: [10, 0, 0, 1], port: 9200, ttl: 32 };
        let policy = WapPolicy {
            accept_empty_probe: true,
            accept_root_path: true,
            accept_status_path: true,
            accept_status_wml_path: true,
            max_request_payload_bytes: 1024,
        };
        let request = build_ipv4_udp_npdu(
            [10, 0, 0, 226],
            endpoint.address,
            49152,
            endpoint.port,
            0x2222,
            64,
            b"GET /status.xhtml HTTP/1.0\r\n\r\n",
        )
        .unwrap();
        let response = build_response_npdu(&request, endpoint, policy, &snapshot()).unwrap().unwrap();
        let ip = parse_ipv4_packet(&response).unwrap();
        let udp = parse_udp_datagram(ip.payload).unwrap();
        assert_eq!(ip.source, [10, 0, 0, 1]);
        assert_eq!(ip.destination, [10, 0, 0, 226]);
        assert_eq!(ip.identification, 0x2223);
        assert_eq!(ip.ttl, 32);
        assert_eq!(udp.source_port, 9200);
        assert_eq!(udp.destination_port, 49152);
    }

    #[test]
    fn three_octet_invoke_is_rejected_without_panicking() {
        let policy = WapPolicy {
            accept_empty_probe: true,
            accept_root_path: true,
            accept_status_path: true,
            accept_status_wml_path: true,
            max_request_payload_bytes: 1024,
        };
        assert_eq!(handle_wtp(&[0x08, 0x00, 0x01], policy, &snapshot()), Err(WapError::UnsupportedPayload));
    }

    #[test]
    fn ack_needs_no_response() {
        let policy = WapPolicy { accept_empty_probe: true, accept_root_path: true, accept_status_path: true, accept_status_wml_path: true, max_request_payload_bytes: 1024 };
        assert_eq!(handle_wtp(&[0x18, 0x13, 0xcc], policy, &snapshot()).unwrap(), None);
    }
}

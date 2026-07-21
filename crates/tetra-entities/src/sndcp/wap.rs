//! WAP 1.x/2.0-over-UDP endpoint for TETRA SNDCP.
//!
//! This is a clean-room implementation of the public WTP/WSP wire formats. It supports the
//! transaction-class-2 flow used by Motorola/Openwave browsers: Connect, Resume, GET and the
//! corresponding WTP Result PDUs. ACK and ABORT intentionally generate no response.

use super::ip::{IPV4_PROTOCOL_UDP, IpError, build_ipv4_udp_npdu, parse_ipv4_packet, parse_udp_datagram};

const WTP_CON_FLAG: u8 = 0x80;
const WTP_INVOKE: u8 = 1;
const WTP_RESULT_LAST: u8 = 0x12;
const WTP_ACK: u8 = 3;
const WTP_ABORT: u8 = 4;
const WTP_RESPONSE_TID: u16 = 0x8000;
const WTP_TID_MASK: u16 = 0x7fff;

const WSP_CONNECT: u8 = 0x01;
const WSP_CONNECT_REPLY: u8 = 0x02;
const WSP_REPLY: u8 = 0x04;
const WSP_RESUME: u8 = 0x09;
const WSP_GET: u8 = 0x40;
const WSP_OK: u8 = 0x20;
const WSP_CT_WML: u8 = 0x88;
const WSP_CT_XHTML: u8 = 0xc5;
const WSP_CLIENT_SDU: u8 = 0x80;
const WSP_SERVER_SDU: u8 = 0x81;
const WSP_SDU_MAX: usize = 545;

#[derive(Debug, Clone, Copy)]
pub struct WapEndpoint {
    pub address: [u8; 4],
    pub port: u16,
    pub ttl: u8,
    pub max_request_bytes: usize,
}

#[derive(Debug, Clone)]
pub struct WapStatusSnapshot {
    pub title: String,
    pub service_state: String,
    pub registered_ms: usize,
    pub active_calls: usize,
    pub queued_sds: usize,
    pub uptime_secs: u64,
    pub carrier: u16,
    pub mcc: u16,
    pub mnc: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WapError {
    Ip(IpError),
    WrongDestination,
    WrongPort,
    Fragmented,
    UnsupportedProtocol(u8),
    RequestTooLarge { len: usize, max: usize },
    MalformedWtpWsp,
    UnsupportedPath(String),
}

impl From<IpError> for WapError {
    fn from(value: IpError) -> Self {
        Self::Ip(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PageFormat {
    Xhtml,
    Wml,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RequestKind {
    RawStatus { path: String },
    Connect { tid: u16, capabilities: Vec<Capability> },
    Resume { tid: u16 },
    Status { tid: u16, path: String },
    NoResponse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Capability {
    id: u8,
    value: Vec<u8>,
}

/// Build a complete IPv4/UDP response. `Ok(None)` means the inbound WTP PDU was an ACK/ABORT.
pub fn build_response(request_npdu: &[u8], endpoint: WapEndpoint, snapshot: &WapStatusSnapshot) -> Result<Option<Vec<u8>>, WapError> {
    let ip = parse_ipv4_packet(request_npdu)?;
    if ip.fragmented {
        return Err(WapError::Fragmented);
    }
    if ip.destination != endpoint.address {
        return Err(WapError::WrongDestination);
    }
    if ip.protocol != IPV4_PROTOCOL_UDP {
        return Err(WapError::UnsupportedProtocol(ip.protocol));
    }
    let udp = parse_udp_datagram(ip.payload)?;
    if udp.destination_port != endpoint.port {
        return Err(WapError::WrongPort);
    }
    if udp.payload.len() > endpoint.max_request_bytes {
        return Err(WapError::RequestTooLarge {
            len: udp.payload.len(),
            max: endpoint.max_request_bytes,
        });
    }

    let response_payload = match classify_request(udp.payload)? {
        RequestKind::NoResponse => return Ok(None),
        RequestKind::RawStatus { path } => render_page(snapshot, &path, 548),
        RequestKind::Connect { tid, capabilities } => {
            let wsp = build_connect_reply(&capabilities);
            build_wtp_result(tid, &wsp)
        }
        RequestKind::Resume { tid } => build_wtp_result(tid, &[WSP_REPLY, WSP_OK, 0]),
        RequestKind::Status { tid, path } => {
            let format = format_for_path(&path);
            let sector = sector_from_path(&path);
            let page_budget = match (format, sector) {
                (PageFormat::Xhtml, None) => 104,
                _ => 144,
            };
            let page = render_page(snapshot, &path, page_budget);
            let content_type = match format {
                PageFormat::Xhtml => WSP_CT_XHTML,
                PageFormat::Wml => WSP_CT_WML,
            };
            let mut wsp = vec![WSP_REPLY, WSP_OK, 1, content_type];
            wsp.extend_from_slice(&page);
            build_wtp_result(tid, &wsp)
        }
    };

    Ok(Some(build_ipv4_udp_npdu(
        endpoint.address,
        ip.source,
        endpoint.port,
        udp.source_port,
        &response_payload,
        ip.identification.wrapping_add(1),
        endpoint.ttl,
    )?))
}

fn classify_request(payload: &[u8]) -> Result<RequestKind, WapError> {
    if payload.is_empty() {
        return Ok(RequestKind::RawStatus { path: "/".into() });
    }

    if payload.len() >= 3 {
        let pdu_type = (payload[0] >> 3) & 0x0f;
        let tid = u16::from_be_bytes([payload[1], payload[2]]) & WTP_TID_MASK;
        if matches!(pdu_type, WTP_ACK | WTP_ABORT) {
            return Ok(RequestKind::NoResponse);
        }
        if pdu_type == WTP_INVOKE {
            let wsp = parse_wtp_invoke(payload)?;
            match wsp.first().copied() {
                Some(WSP_CONNECT) => {
                    return Ok(RequestKind::Connect {
                        tid,
                        capabilities: parse_connect(wsp)?,
                    });
                }
                Some(WSP_RESUME) => return Ok(RequestKind::Resume { tid }),
                Some(kind) if kind == WSP_GET || (0x50..=0x5f).contains(&kind) => {
                    let (uri_len, len_len) = read_uintvar(&wsp[1..]).ok_or(WapError::MalformedWtpWsp)?;
                    let start = 1 + len_len;
                    let end = start.checked_add(uri_len).ok_or(WapError::MalformedWtpWsp)?;
                    let uri = std::str::from_utf8(wsp.get(start..end).ok_or(WapError::MalformedWtpWsp)?)
                        .map_err(|_| WapError::MalformedWtpWsp)?;
                    let path = normalize_uri(uri);
                    ensure_status_path(&path)?;
                    return Ok(RequestKind::Status { tid, path });
                }
                _ => return Err(WapError::MalformedWtpWsp),
            }
        }
    }

    let text = std::str::from_utf8(payload).map_err(|_| WapError::MalformedWtpWsp)?;
    if let Some(rest) = text.trim_start().strip_prefix("GET ") {
        let path = normalize_uri(rest.split_whitespace().next().unwrap_or("/"));
        ensure_status_path(&path)?;
        return Ok(RequestKind::RawStatus { path });
    }
    Err(WapError::MalformedWtpWsp)
}

fn parse_wtp_invoke(payload: &[u8]) -> Result<&[u8], WapError> {
    if payload.len() < 5 {
        return Err(WapError::MalformedWtpWsp);
    }
    let invoke = payload[3];
    if ((invoke >> 6) & 0x03) != 0 || (invoke & 0x0c) != 0 || (invoke & 0x03) != 2 {
        return Err(WapError::MalformedWtpWsp);
    }
    let mut offset = 4;
    if payload[0] & WTP_CON_FLAG != 0 {
        loop {
            let h = *payload.get(offset).ok_or(WapError::MalformedWtpWsp)?;
            let continues = h & WTP_CON_FLAG != 0;
            if h & 0x04 != 0 {
                let len = *payload.get(offset + 1).ok_or(WapError::MalformedWtpWsp)? as usize;
                offset = offset.checked_add(2 + len).ok_or(WapError::MalformedWtpWsp)?;
            } else {
                offset = offset.checked_add(1 + (h & 0x03) as usize).ok_or(WapError::MalformedWtpWsp)?;
            }
            if offset > payload.len() {
                return Err(WapError::MalformedWtpWsp);
            }
            if !continues {
                break;
            }
        }
    }
    payload.get(offset..).filter(|v| !v.is_empty()).ok_or(WapError::MalformedWtpWsp)
}

fn parse_connect(wsp: &[u8]) -> Result<Vec<Capability>, WapError> {
    if wsp.len() < 4 || wsp[0] != WSP_CONNECT {
        return Err(WapError::MalformedWtpWsp);
    }
    let (caps_len, caps_len_len) = read_uintvar(&wsp[2..]).ok_or(WapError::MalformedWtpWsp)?;
    let headers_len_at = 2 + caps_len_len;
    let (headers_len, headers_len_len) = read_uintvar(&wsp[headers_len_at..]).ok_or(WapError::MalformedWtpWsp)?;
    let caps_start = headers_len_at + headers_len_len;
    let caps_end = caps_start.checked_add(caps_len).ok_or(WapError::MalformedWtpWsp)?;
    let headers_end = caps_end.checked_add(headers_len).ok_or(WapError::MalformedWtpWsp)?;
    if headers_end > wsp.len() {
        return Err(WapError::MalformedWtpWsp);
    }
    let mut input = &wsp[caps_start..caps_end];
    let mut caps = Vec::new();
    while !input.is_empty() {
        let (len, len_len) = read_uintvar(input).ok_or(WapError::MalformedWtpWsp)?;
        if len == 0 || len_len + len > input.len() {
            return Err(WapError::MalformedWtpWsp);
        }
        let body = &input[len_len..len_len + len];
        caps.push(Capability {
            id: body[0],
            value: body[1..].to_vec(),
        });
        input = &input[len_len + len..];
    }
    Ok(caps)
}

fn build_connect_reply(capabilities: &[Capability]) -> Vec<u8> {
    let mut caps_out = Vec::new();
    for id in [WSP_CLIENT_SDU, WSP_SERVER_SDU] {
        let Some(cap) = capabilities.iter().find(|c| c.id == id) else {
            continue;
        };
        let Some((requested, used)) = read_uintvar(&cap.value) else {
            continue;
        };
        if used != cap.value.len() {
            continue;
        }
        let mut value = Vec::new();
        write_uintvar(requested.min(WSP_SDU_MAX), &mut value);
        write_uintvar(1 + value.len(), &mut caps_out);
        caps_out.push(id);
        caps_out.extend_from_slice(&value);
    }
    let mut out = vec![WSP_CONNECT_REPLY, 0x01];
    write_uintvar(caps_out.len(), &mut out);
    write_uintvar(0, &mut out);
    out.extend_from_slice(&caps_out);
    out
}

fn build_wtp_result(tid: u16, wsp: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(3 + wsp.len());
    out.push(WTP_RESULT_LAST);
    out.extend_from_slice(&((tid & WTP_TID_MASK) | WTP_RESPONSE_TID).to_be_bytes());
    out.extend_from_slice(wsp);
    out
}

fn ensure_status_path(path: &str) -> Result<(), WapError> {
    let base = path.split(['?', '#']).next().unwrap_or(path);
    if matches!(base, "/" | "/status" | "/status.xhtml" | "/status.wml") {
        Ok(())
    } else {
        Err(WapError::UnsupportedPath(path.to_string()))
    }
}

fn normalize_uri(uri: &str) -> String {
    let uri = uri.trim();
    let path = if let Some(rest) = uri.strip_prefix("http://").or_else(|| uri.strip_prefix("https://")) {
        rest.find('/').map(|idx| &rest[idx..]).unwrap_or("/")
    } else {
        uri
    };
    if path.is_empty() {
        "/".into()
    } else if path.starts_with('/') {
        path.into()
    } else {
        format!("/{path}")
    }
}

fn format_for_path(path: &str) -> PageFormat {
    if path.split(['?', '#']).next().unwrap_or(path).ends_with(".wml") {
        PageFormat::Wml
    } else {
        PageFormat::Xhtml
    }
}

fn sector_from_path(path: &str) -> Option<usize> {
    let query = path.split_once('?')?.1.split('#').next().unwrap_or("");
    query.split('&').find_map(|part| {
        let (key, value) = part.split_once('=')?;
        (key == "s").then(|| value.parse::<usize>().ok()).flatten()
    })
}

fn render_page(snapshot: &WapStatusSnapshot, path: &str, budget: usize) -> Vec<u8> {
    let format = format_for_path(path);
    let sector = sector_from_path(path).unwrap_or(0).min(2);
    let title = escape(&snapshot.title, 18);
    let state = escape(&snapshot.service_state, 12);
    let uptime = compact_uptime(snapshot.uptime_secs);
    let body = match sector {
        0 => format!(
            "{title}<br/>{state} MS:{} C:{}<br/>Up:{uptime}<br/><a href=\"?s=1\">N</a>",
            snapshot.registered_ms, snapshot.active_calls
        ),
        1 => format!(
            "Net {}/{}<br/>Carrier:{}<br/>SDS:{}<br/><a href=\"?s=2\">N</a> <a href=\"?s=0\">H</a>",
            snapshot.mcc, snapshot.mnc, snapshot.carrier, snapshot.queued_sds
        ),
        _ => format!("Packet data OK<br/>WTP/WSP active<br/>UDP 9200<br/><a href=\"?s=0\">H</a>"),
    };
    let candidates = match format {
        PageFormat::Xhtml => vec![
            format!("<html><body>{body}</body></html>"),
            format!(
                "<html><body>{title}<br/>{state} MS:{}<br/><a href=\"?s=1\">N</a></body></html>",
                snapshot.registered_ms
            ),
            format!("<html><body>{title}<br/>{state}</body></html>"),
        ],
        PageFormat::Wml => vec![
            format!("<wml><card><p>{body}</p></card></wml>"),
            format!(
                "<wml><card><p>{title}<br/>{state} MS:{}<br/><a href=\"?s=1\">N</a></p></card></wml>",
                snapshot.registered_ms
            ),
            format!("<wml><card><p>{title}<br/>{state}</p></card></wml>"),
        ],
    };
    candidates
        .into_iter()
        .find(|candidate| candidate.len() <= budget)
        .unwrap_or_else(|| match format {
            PageFormat::Xhtml => "<html><body>NetCore OK</body></html>".into(),
            PageFormat::Wml => "<wml><card><p>NetCore OK</p></card></wml>".into(),
        })
        .into_bytes()
}

fn compact_uptime(seconds: u64) -> String {
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let minutes = (seconds % 3_600) / 60;
    if days > 0 {
        format!("{days}d{hours}h")
    } else if hours > 0 {
        format!("{hours}h{minutes}m")
    } else {
        format!("{minutes}m")
    }
}

fn escape(value: &str, max_bytes: usize) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        let fragment = match ch {
            '&' => "&amp;".to_string(),
            '<' => "&lt;".to_string(),
            '>' => "&gt;".to_string(),
            '"' => "&quot;".to_string(),
            '\'' => "&apos;".to_string(),
            '\n' | '\r' | '\t' => " ".to_string(),
            c if c.is_control() => "?".to_string(),
            c => c.to_string(),
        };
        if out.len() + fragment.len() > max_bytes {
            if out.len() < max_bytes {
                out.push('~');
            }
            break;
        }
        out.push_str(&fragment);
    }
    out
}

fn read_uintvar(buf: &[u8]) -> Option<(usize, usize)> {
    let mut value = 0usize;
    for (idx, octet) in buf.iter().copied().enumerate().take(5) {
        value = value.checked_shl(7)?.checked_add((octet & 0x7f) as usize)?;
        if octet & 0x80 == 0 {
            return Some((value, idx + 1));
        }
    }
    None
}

fn write_uintvar(mut value: usize, out: &mut Vec<u8>) {
    let mut stack = [0u8; 5];
    let mut idx = stack.len();
    loop {
        idx -= 1;
        stack[idx] = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            break;
        }
    }
    let continuation_end = stack.len() - 1;
    for byte in &mut stack[idx..continuation_end] {
        *byte |= 0x80;
    }
    out.extend_from_slice(&stack[idx..]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_reply_matches_openwave_shape() {
        let caps = vec![
            Capability {
                id: 0x80,
                value: vec![0x94, 0x80, 0x00],
            },
            Capability {
                id: 0x81,
                value: vec![0x94, 0x80, 0x00],
            },
        ];
        assert_eq!(
            build_wtp_result(0x13cc, &build_connect_reply(&caps)),
            vec![
                0x12, 0x93, 0xcc, 0x02, 0x01, 0x08, 0x00, 0x03, 0x80, 0x84, 0x21, 0x03, 0x81, 0x84, 0x21
            ]
        );
    }

    #[test]
    fn ack_requires_no_response() {
        assert_eq!(classify_request(&[0x18, 0x13, 0xcc]).unwrap(), RequestKind::NoResponse);
    }

    #[test]
    fn complete_ipv4_udp_connect_roundtrip_is_byte_exact() {
        let request_payload = vec![
            0x08, 0x13, 0xcc, 0x12, 0x01, 0x10, 0x08, 0x00, 0x03, 0x80, 0x84, 0x21, 0x03, 0x81, 0x84, 0x21,
        ];
        let request = build_ipv4_udp_npdu([10, 0, 0, 2], [10, 0, 0, 1], 49_152, 9_200, &request_payload, 0x2222, 64).unwrap();
        let snapshot = WapStatusSnapshot {
            title: "NetCore-TETRA".into(),
            service_state: "ON AIR".into(),
            registered_ms: 2,
            active_calls: 1,
            queued_sds: 0,
            uptime_secs: 125,
            carrier: 720,
            mcc: 262,
            mnc: 42,
        };
        let response = build_response(
            &request,
            WapEndpoint {
                address: [10, 0, 0, 1],
                port: 9_200,
                ttl: 32,
                max_request_bytes: 1_024,
            },
            &snapshot,
        )
        .unwrap()
        .expect("CONNECT requires a response");

        let ip = parse_ipv4_packet(&response).unwrap();
        assert_eq!(ip.source, [10, 0, 0, 1]);
        assert_eq!(ip.destination, [10, 0, 0, 2]);
        assert_eq!(ip.identification, 0x2223);
        assert_eq!(ip.ttl, 32);
        let udp = parse_udp_datagram(ip.payload).unwrap();
        assert_eq!(udp.source_port, 9_200);
        assert_eq!(udp.destination_port, 49_152);
        assert_eq!(
            udp.payload,
            &[
                0x12, 0x93, 0xcc, 0x02, 0x01, 0x08, 0x00, 0x03, 0x80, 0x84, 0x21, 0x03, 0x81, 0x84, 0x21
            ]
        );
    }

    #[test]
    fn status_page_stays_inside_openwave_index_budget() {
        let snapshot = WapStatusSnapshot {
            title: "NetCore-TETRA & Test".into(),
            service_state: "ON AIR".into(),
            registered_ms: 12,
            active_calls: 3,
            queued_sds: 4,
            uptime_secs: 86_500,
            carrier: 720,
            mcc: 262,
            mnc: 42,
        };
        let page = render_page(&snapshot, "/status.xhtml", 104);
        assert!(page.len() <= 104);
        assert!(std::str::from_utf8(&page).unwrap().contains("NetCore-TETRA"));
    }
}

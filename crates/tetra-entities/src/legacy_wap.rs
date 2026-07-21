//! Legacy WAP delivery through SDS Type 4.
//!
//! EN 300 392-2 assigns protocol identifier 0x04 to WAP/WDP carried directly
//! in SDS Type 4 and 0x84 to WAP carried through the SDS-TL transfer service.
//! The latter therefore includes an SDS-TL transfer header after the PID.

use bitcode::{Decode, Encode};
use serde::{Deserialize, Serialize};

pub const WAP_WDP_PROTOCOL_ID: u8 = 0x04;
pub const WAP_SDS_TL_PROTOCOL_ID: u8 = 0x84;
pub const SDS_TYPE4_MAX_BYTES: usize = 255;
pub const SDS_TL_NO_REPORT_FLAGS: u8 = 0x00;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegacyWapTransport {
    /// PID 0x04 followed by the application WAP/WML payload.
    Wdp,
    /// PID 0x84 followed by a minimal SDS-TL TRANSFER header and payload.
    SdsTl,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegacyWapError {
    EmptyPayload,
    PayloadTooLarge { len: usize, max: usize },
}

pub fn build_type4_payload(
    payload: &[u8],
    transport: LegacyWapTransport,
    message_reference: u8,
) -> Result<Vec<u8>, LegacyWapError> {
    if payload.is_empty() {
        return Err(LegacyWapError::EmptyPayload);
    }
    let header_len = match transport {
        LegacyWapTransport::Wdp => 1,
        LegacyWapTransport::SdsTl => 3,
    };
    let total = header_len + payload.len();
    if total > SDS_TYPE4_MAX_BYTES {
        return Err(LegacyWapError::PayloadTooLarge {
            len: total,
            max: SDS_TYPE4_MAX_BYTES,
        });
    }
    let mut out = Vec::with_capacity(total);
    match transport {
        LegacyWapTransport::Wdp => out.push(WAP_WDP_PROTOCOL_ID),
        LegacyWapTransport::SdsTl => {
            out.push(WAP_SDS_TL_PROTOCOL_ID);
            out.push(SDS_TL_NO_REPORT_FLAGS);
            out.push(message_reference);
        }
    }
    out.extend_from_slice(payload);
    Ok(out)
}

pub fn render_compact_wml(title: &str, message: &str, target_url: Option<&str>) -> String {
    let title = escape_xml(title.trim());
    let message = escape_xml(message.trim());
    let link = target_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("<br/><a href=\"{}\">Oeffnen</a>", escape_xml(value)))
        .unwrap_or_default();
    format!(
        "<?xml version=\"1.0\"?><!DOCTYPE wml PUBLIC \"-//WAPFORUM//DTD WML 1.1//EN\" \"http://www.wapforum.org/DTD/wml_1.1.xml\"><wml><card title=\"{title}\"><p><b>{title}</b><br/>{message}{link}</p></card></wml>"
    )
}

/// Render a WML card and shrink the human text until the complete SDS Type 4
/// payload fits. XML framing and the optional URL are never cut mid-token.
pub fn build_compact_wml_type4(
    title: &str,
    message: &str,
    target_url: Option<&str>,
    transport: LegacyWapTransport,
    message_reference: u8,
) -> Result<Vec<u8>, LegacyWapError> {
    let chars = message.chars().collect::<Vec<_>>();
    // Validate the fixed XML/title/URL framing before searching the largest
    // message prefix. This also rejects a URL/title combination that can never
    // fit, instead of spinning through the entire message.
    let empty_wml = render_compact_wml(title, "", target_url);
    build_type4_payload(empty_wml.as_bytes(), transport, message_reference)?;

    let mut low = 0usize;
    let mut high = chars.len();
    while low < high {
        let mid = low + (high - low + 1) / 2;
        let current = chars[..mid].iter().collect::<String>();
        let wml = render_compact_wml(title, &current, target_url);
        if build_type4_payload(wml.as_bytes(), transport, message_reference).is_ok() {
            low = mid;
        } else {
            high = mid - 1;
        }
    }
    let current = chars[..low].iter().collect::<String>();
    let wml = render_compact_wml(title, &current, target_url);
    build_type4_payload(wml.as_bytes(), transport, message_reference)
}

fn escape_xml(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_wdp_starts_with_pid_04() {
        let payload = build_type4_payload(b"<wml/>", LegacyWapTransport::Wdp, 7).unwrap();
        assert_eq!(payload[0], WAP_WDP_PROTOCOL_ID);
        assert_eq!(&payload[1..], b"<wml/>");
    }

    #[test]
    fn sds_tl_has_transfer_header() {
        let payload = build_type4_payload(b"<wml/>", LegacyWapTransport::SdsTl, 0x42).unwrap();
        assert_eq!(&payload[..3], &[WAP_SDS_TL_PROTOCOL_ID, SDS_TL_NO_REPORT_FLAGS, 0x42]);
    }

    #[test]
    fn compact_renderer_never_exceeds_type4_limit() {
        let message = "A".repeat(1000);
        let payload = build_compact_wml_type4(
            "NetCore",
            &message,
            Some("http://10.0.0.1:9200/"),
            LegacyWapTransport::Wdp,
            1,
        )
        .unwrap();
        assert!(payload.len() <= SDS_TYPE4_MAX_BYTES);
    }
}

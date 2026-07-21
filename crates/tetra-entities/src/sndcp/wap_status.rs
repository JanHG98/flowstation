//! Small terminal-friendly status page renderer.

#[derive(Debug, Clone)]
pub struct WapStatusSnapshot {
    pub title: String,
    pub state: String,
    pub version: String,
    pub registered_ms: usize,
    pub attached_groups: usize,
    pub active_calls: usize,
    pub queued_sds: usize,
    pub uptime_secs: u64,
    pub last_activity: String,
    pub health: String,
}

pub fn escape_xhtml_text_limited(input: &str, max: usize) -> String {
    let mut out = String::new();
    let mut truncated = false;
    for ch in input.chars() {
        let frag = match ch {
            '&' => "&amp;".to_string(),
            '<' => "&lt;".to_string(),
            '>' => "&gt;".to_string(),
            '"' => "&quot;".to_string(),
            '\'' => "&apos;".to_string(),
            '\n' | '\r' | '\t' => " ".to_string(),
            c if c.is_control() => "?".to_string(),
            c => c.to_string(),
        };
        if out.len() + frag.len() > max {
            truncated = true;
            break;
        }
        out.push_str(&frag);
    }
    if truncated && out.len() < max {
        out.push('~');
    }
    out
}

fn uptime(secs: u64) -> String {
    let d = (secs / 86_400).min(99);
    let h = (secs % 86_400) / 3_600;
    let m = (secs % 3_600) / 60;
    let s = secs % 60;
    if d > 0 {
        format!("{d}d{h}h{m}m{s}s")
    } else if h > 0 {
        format!("{h}h{m}m{s}s")
    } else {
        format!("{m}m{s}s")
    }
}

fn compact_body(snapshot: &WapStatusSnapshot, br: &str) -> String {
    let title = escape_xhtml_text_limited(&snapshot.title, 24);
    let state = escape_xhtml_text_limited(&snapshot.state, 20);
    let version = escape_xhtml_text_limited(snapshot.version.trim_start_matches('v'), 32);
    let last = escape_xhtml_text_limited(&snapshot.last_activity, 32);
    let health = escape_xhtml_text_limited(&snapshot.health, 32);
    format!(
        "{title}: {state}{br}MS:{} G:{} P:{} SDS:{}{br}Version: {version}{br}Uptime {}{br}Last: {last}{br}Health:{health}",
        snapshot.registered_ms,
        snapshot.attached_groups,
        snapshot.active_calls,
        snapshot.queued_sds,
        uptime(snapshot.uptime_secs)
    )
}

fn fit_document(prefix: &str, body: String, suffix: &str, max: usize) -> String {
    if prefix.len() + body.len() + suffix.len() <= max {
        return format!("{prefix}{body}{suffix}");
    }
    let budget = max.saturating_sub(prefix.len() + suffix.len());
    let tiny = escape_xhtml_text_limited(&body, budget);
    format!("{prefix}{tiny}{suffix}")
}

pub fn render_raw_xhtml(snapshot: &WapStatusSnapshot, max: usize) -> String {
    const PREFIX: &str = "<!DOCTYPE html PUBLIC \"-//WAPFORUM//DTD XHTML Mobile 1.0//EN\" \"http://www.wapforum.org/DTD/xhtml-mobile10.dtd\"><html xmlns=\"http://www.w3.org/1999/xhtml\"><body>";
    fit_document(PREFIX, compact_body(snapshot, "<br />"), "</body></html>", max)
}

fn first_fitting(candidates: impl IntoIterator<Item = String>, max: usize) -> String {
    let mut shortest = String::new();
    for candidate in candidates {
        if shortest.is_empty() || candidate.len() < shortest.len() {
            shortest = candidate.clone();
        }
        if candidate.len() <= max {
            return candidate;
        }
    }
    shortest
}

pub fn render_browser_xhtml(snapshot: &WapStatusSnapshot, max: usize) -> String {
    let title = escape_xhtml_text_limited(&snapshot.title, 12);
    let state = escape_xhtml_text_limited(&snapshot.state, 12);
    first_fitting(
        [
            format!(
                "<html><body>{title}<br/>{state}<br/>MS:{} G:{} P:{}<br/><a href=\"/status.wml?s=1\">N</a></body></html>",
                snapshot.registered_ms, snapshot.attached_groups, snapshot.active_calls
            ),
            format!(
                "<html><body>{title}:{state}<br/>MS:{} G:{}<br/><a href=\"/status.wml?s=1\">N</a></body></html>",
                snapshot.registered_ms, snapshot.attached_groups
            ),
            format!("<html><body>{title}:{state}</body></html>"),
        ],
        max,
    )
}

pub fn render_browser_xhtml_sector(snapshot: &WapStatusSnapshot, max: usize) -> String {
    let health = escape_xhtml_text_limited(&snapshot.health, 18);
    let last = escape_xhtml_text_limited(&snapshot.last_activity, 18);
    first_fitting(
        [
            format!(
                "<html><body>Health {health}<br/>Up {}<br/>Last {last}<br/><a href=\"/\">H</a></body></html>",
                uptime(snapshot.uptime_secs)
            ),
            format!("<html><body>Health {health}<br/><a href=\"/\">H</a></body></html>"),
        ],
        max,
    )
}

pub fn render_browser_wml(snapshot: &WapStatusSnapshot, max: usize) -> String {
    let title = escape_xhtml_text_limited(&snapshot.title, 12);
    let state = escape_xhtml_text_limited(&snapshot.state, 12);
    first_fitting(
        [
            format!(
                "<wml><card><p>{title}<br/>{state}<br/>MS:{} G:{} P:{}<br/><a href=\"/status.wml?s=1\">N</a></p></card></wml>",
                snapshot.registered_ms, snapshot.attached_groups, snapshot.active_calls
            ),
            format!("<wml><card><p>{title}:{state}<br/><a href=\"/\">H</a></p></card></wml>"),
        ],
        max,
    )
}

pub fn render_browser_wml_sector(snapshot: &WapStatusSnapshot, max: usize) -> String {
    let health = escape_xhtml_text_limited(&snapshot.health, 18);
    let last = escape_xhtml_text_limited(&snapshot.last_activity, 18);
    first_fitting(
        [
            format!(
                "<wml><card><p>Health {health}<br/>Up {}<br/>Last {last}<br/><a href=\"/\">H</a></p></card></wml>",
                uptime(snapshot.uptime_secs)
            ),
            format!("<wml><card><p>Health {health}<br/><a href=\"/\">H</a></p></card></wml>"),
        ],
        max,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_never_splits_entity() {
        assert_eq!(escape_xhtml_text_limited("&&&&", 10), "&amp;&amp;");
        assert_eq!(escape_xhtml_text_limited("&&&&", 11), "&amp;&amp;~");
        assert_eq!(escape_xhtml_text_limited("<tag>", 8), "&lt;tag~");
    }

    #[test]
    fn openwave_pages_stay_inside_their_hard_caps() {
        let snapshot = WapStatusSnapshot {
            title: "NetCore-Tetra".into(),
            state: "ONLINE".into(),
            version: "v1.3.0".into(),
            registered_ms: 12,
            attached_groups: 4,
            active_calls: 2,
            queued_sds: 1,
            uptime_secs: 93_784,
            last_activity: "SDS 4010001>4010002".into(),
            health: "OK".into(),
        };
        let index = render_browser_xhtml(&snapshot, 104);
        let sector = render_browser_wml_sector(&snapshot, 144);
        assert!(index.len() <= 104);
        assert!(sector.len() <= 144);
        assert!(index.contains("<html><body>"));
        assert!(sector.contains("<wml><card><p>"));
    }
}

//! Dashboard multi-carrier ON/OFF support.
//!
//! This module grew out of the v1.3.0 Dual-Carrier toggle. It intentionally keeps the
//! public route/module names (`dualcarrier`, `dual_carrier`) so older dashboard code and
//! bookmarks keep working, but the TOML handling now understands an optional third carrier:
//!
//! ```toml
//! secondary_carrier = 721
//! dual_carrier_enabled = true
//! third_carrier = 719
//! third_carrier_enabled = true
//! ```
//!
//! Additional carriers are fixed at startup (PHY/SDR tuning, UMAC schedulers and the
//! timeslot allocator read them once at construction), so toggling writes config.toml and
//! schedules a controlled restart.

/// Current multi-carrier configuration as read straight from the TOML file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DualCarrierState {
    /// The `dual_carrier_enabled` switch (absent = true for backward compatibility).
    pub enabled: bool,
    /// The configured `secondary_carrier` number, if any (preserved even while disabled).
    pub secondary_carrier: Option<u16>,
    /// The `third_carrier_enabled` switch (absent = true for backward compatibility).
    pub third_enabled: bool,
    /// The configured `third_carrier` number, if any (preserved even while disabled).
    pub third_carrier: Option<u16>,
}

impl DualCarrierState {
    /// Secondary carrier is operationally active only when switched on AND configured.
    pub fn secondary_active(&self) -> bool {
        self.enabled && self.secondary_carrier.is_some()
    }

    /// Third carrier is operationally active only when switched on AND configured.
    pub fn third_active(&self) -> bool {
        self.third_enabled && self.third_carrier.is_some()
    }

    /// Any additional carrier active.
    pub fn active(&self) -> bool {
        self.secondary_active() || self.third_active()
    }
}

/// Read the multi-carrier switches + configured carrier numbers from the TOML file.
/// Tolerant of a missing/garbled file: defaults to enabled=true, no additional carriers.
pub fn read_dual_carrier(config_path: &str) -> DualCarrierState {
    let txt = std::fs::read_to_string(config_path).unwrap_or_default();
    let mut in_cell = false;
    let mut enabled = true;
    let mut secondary_carrier = None;
    let mut third_enabled = true;
    let mut third_carrier = None;

    for line in txt.lines() {
        let trimmed = line.trim_start();
        if let Some(name) = table_name(trimmed) {
            in_cell = is_cell_info_table(name);
            continue;
        }
        if !in_cell || trimmed.starts_with('#') {
            continue;
        }
        if let Some(v) = active_value(trimmed, "secondary_carrier") {
            secondary_carrier = value_token(v).parse::<u16>().ok();
        } else if let Some(v) = active_value(trimmed, "dual_carrier_enabled") {
            enabled = value_token(v) == "true";
        } else if let Some(v) = active_value(trimmed, "third_carrier") {
            third_carrier = value_token(v).parse::<u16>().ok();
        } else if let Some(v) = active_value(trimmed, "third_carrier_enabled") {
            third_enabled = value_token(v) == "true";
        }
    }
    DualCarrierState { enabled, secondary_carrier, third_enabled, third_carrier }
}

/// For an active (uncommented) `key = <value>` line, return the trimmed value part; else None.
fn active_value<'a>(trimmed: &'a str, key: &str) -> Option<&'a str> {
    if !trimmed.starts_with(key) {
        return None;
    }
    trimmed[key.len()..].trim_start().strip_prefix('=').map(str::trim)
}

/// Strip a trailing `# inline comment` and surrounding whitespace from a TOML scalar value.
fn value_token(v: &str) -> &str {
    v.split('#').next().unwrap_or(v).trim()
}

/// Return the TOML table name for real table headers (`[cell_info]`,
/// `[cell_info.sds_command_control]`, `[[cell_info.neighbor_cells_ca]]`).
/// Ordinary array values such as `local_ssi_ranges = [` are intentionally ignored.
fn table_name(trimmed: &str) -> Option<&str> {
    let head = value_token(trimmed);
    let inner = if let Some(rest) = head.strip_prefix("[[") {
        rest.strip_suffix("]]" )?
    } else if let Some(rest) = head.strip_prefix('[') {
        rest.strip_suffix(']')?
    } else {
        return None;
    };

    let name = inner.trim();
    if name.is_empty() {
        return None;
    }

    if name
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
    {
        Some(name)
    } else {
        None
    }
}

fn is_cell_info_table(name: &str) -> bool {
    name == "cell_info"
}

/// Produce a new TOML body with the secondary/third-carrier switches and configured
/// carrier numbers set inside `[cell_info]`, preserving everything else including comments.
///
/// Passing `None` for a carrier value leaves any existing active carrier-number line untouched,
/// so OFF remembers what ON should use next time.
pub fn compute_toml(
    original: &str,
    enabled: bool,
    secondary_carrier: Option<u16>,
    third_enabled: bool,
    third_carrier: Option<u16>,
) -> String {
    let enabled_line = format!("dual_carrier_enabled = {enabled}");
    let secondary_line = secondary_carrier.map(|c| format!("secondary_carrier = {c}"));
    let third_enabled_line = format!("third_carrier_enabled = {third_enabled}");
    let third_line = third_carrier.map(|c| format!("third_carrier = {c}"));

    let mut out: Vec<String> = Vec::new();
    let mut in_cell = false;
    let mut cell_seen = false;
    let mut wrote_enabled = false;
    let mut wrote_secondary = secondary_line.is_none();
    let mut wrote_third_enabled = false;
    let mut wrote_third = third_line.is_none();

    let is_active_key = |trimmed: &str, key: &str| {
        !trimmed.starts_with('#')
            && trimmed.starts_with(key)
            && trimmed[key.len()..].trim_start().starts_with('=')
    };

    let flush_missing = |out: &mut Vec<String>,
                         wrote_enabled: &mut bool,
                         wrote_secondary: &mut bool,
                         wrote_third_enabled: &mut bool,
                         wrote_third: &mut bool| {
        if !*wrote_enabled {
            out.push(enabled_line.clone());
            *wrote_enabled = true;
        }
        if !*wrote_secondary {
            if let Some(ref s) = secondary_line {
                out.push(s.clone());
            }
            *wrote_secondary = true;
        }
        if !*wrote_third_enabled {
            out.push(third_enabled_line.clone());
            *wrote_third_enabled = true;
        }
        if !*wrote_third {
            if let Some(ref s) = third_line {
                out.push(s.clone());
            }
            *wrote_third = true;
        }
    };

    for line in original.lines() {
        let trimmed = line.trim_start();

        if let Some(name) = table_name(trimmed) {
            if in_cell {
                flush_missing(
                    &mut out,
                    &mut wrote_enabled,
                    &mut wrote_secondary,
                    &mut wrote_third_enabled,
                    &mut wrote_third,
                );
            }
            in_cell = is_cell_info_table(name);
            if in_cell {
                cell_seen = true;
            }
            out.push(line.to_string());
            continue;
        }

        if in_cell {
            if !wrote_enabled && is_active_key(trimmed, "dual_carrier_enabled") {
                out.push(enabled_line.clone());
                wrote_enabled = true;
                continue;
            }
            if !wrote_secondary && is_active_key(trimmed, "secondary_carrier") {
                if let Some(ref s) = secondary_line {
                    out.push(s.clone());
                }
                wrote_secondary = true;
                continue;
            }
            if !wrote_third_enabled && is_active_key(trimmed, "third_carrier_enabled") {
                out.push(third_enabled_line.clone());
                wrote_third_enabled = true;
                continue;
            }
            if !wrote_third && is_active_key(trimmed, "third_carrier") {
                if let Some(ref s) = third_line {
                    out.push(s.clone());
                }
                wrote_third = true;
                continue;
            }
        }

        out.push(line.to_string());
    }

    if in_cell {
        flush_missing(
            &mut out,
            &mut wrote_enabled,
            &mut wrote_secondary,
            &mut wrote_third_enabled,
            &mut wrote_third,
        );
    }

    if !cell_seen {
        if !out.is_empty() && !out.last().map(|l| l.is_empty()).unwrap_or(true) {
            out.push(String::new());
        }
        out.push("[cell_info]".to_string());
        out.push(enabled_line.clone());
        if let Some(ref s) = secondary_line {
            out.push(s.clone());
        }
        out.push(third_enabled_line.clone());
        if let Some(ref s) = third_line {
            out.push(s.clone());
        }
    }

    let mut new_content = out.join("\n");
    if original.ends_with('\n') {
        new_content.push('\n');
    }
    new_content
}

/// Apply the carrier toggle to the config file (backup, then write).
pub fn write_dual_carrier(
    config_path: &str,
    enabled: bool,
    secondary_carrier: Option<u16>,
    third_enabled: bool,
    third_carrier: Option<u16>,
) -> std::io::Result<()> {
    let original = std::fs::read_to_string(config_path)?;
    let new_content = compute_toml(&original, enabled, secondary_carrier, third_enabled, third_carrier);
    let backup = format!("{config_path}.multicarrier.bak");
    let _ = std::fs::copy(config_path, &backup);
    std::fs::write(config_path, new_content)
}

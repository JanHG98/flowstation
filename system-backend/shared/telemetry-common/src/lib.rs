//! Minimal Prometheus text exposition helpers used by NetCore service metrics endpoints.

use std::collections::BTreeMap;

pub fn escape_label(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\n', "\\n").replace('"', "\\\"")
}

pub fn metric_line(name: &str, labels: &BTreeMap<&str, String>, value: impl std::fmt::Display) -> String {
    let mut line = String::new();
    line.push_str(name);
    if !labels.is_empty() {
        line.push('{');
        for (index, (key, label_value)) in labels.iter().enumerate() {
            if index > 0 {
                line.push(',');
            }
            line.push_str(key);
            line.push_str("=\"");
            line.push_str(&escape_label(label_value));
            line.push('"');
        }
        line.push('}');
    }
    line.push(' ');
    line.push_str(&value.to_string());
    line.push('\n');
    line
}

pub fn help_and_type(name: &str, help: &str, metric_type: &str) -> String {
    format!("# HELP {name} {}\n# TYPE {name} {metric_type}\n", help.replace('\n', " "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_prometheus_labels() {
        assert_eq!(escape_label("a\"b\\c\nd"), "a\\\"b\\\\c\\nd");
    }

    #[test]
    fn renders_labels_in_stable_order() {
        let labels = BTreeMap::from([("service", "group-core".to_owned()), ("state", "up".to_owned())]);
        assert_eq!(metric_line("netcore_up", &labels, 1), "netcore_up{service=\"group-core\",state=\"up\"} 1\n");
    }
}

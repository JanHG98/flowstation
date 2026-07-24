use std::io::Write;
use std::process::{Command, Stdio};

use serde::Serialize;

use crate::config::{IpGatewayConfig, MODE_AUTHORITATIVE};
use crate::state::{FirewallRule, KernelStateSnapshot, NatRule, RouteRule};

const FILTER_TABLE: &str = "netcore_ip_gateway";
const NAT_TABLE: &str = "netcore_ip_gateway_nat";

#[derive(Debug, Clone, Serialize)]
pub struct KernelPlan {
    pub mode: String,
    pub authoritative: bool,
    pub revision: u64,
    pub commands: Vec<String>,
    pub nft_ruleset: String,
    pub warnings: Vec<String>,
}

pub fn build_plan(config: &IpGatewayConfig, snapshot: &KernelStateSnapshot) -> KernelPlan {
    let mut commands = vec![
        format!(
            "ip link set dev {} mtu {} up",
            config.interface.name, config.interface.mtu
        ),
        format!(
            "ip address replace {} dev {}{}",
            config.interface.address,
            config.interface.name,
            if config.routing.install_connected_route {
                ""
            } else {
                " noprefixroute"
            }
        ),
    ];
    if config.routing.enable_ipv4_forwarding {
        commands.push("sysctl -w net.ipv4.ip_forward=1".to_string());
    }
    for route in snapshot.routes.iter().filter(|route| route.enabled) {
        commands.push(format_route_command("replace", route, &config.interface.name));
    }
    commands.push(format!("nft delete table inet {FILTER_TABLE}  # ignore if absent"));
    commands.push(format!("nft delete table ip {NAT_TABLE}  # ignore if absent"));
    if config.firewall.enabled || config.nat.enabled {
        commands.push("nft -f - <<'NFT'\n<rendered ruleset>\nNFT".to_string());
    }
    let mut warnings = Vec::new();
    if config.interface.mode != MODE_AUTHORITATIVE {
        warnings.push("shadow mode: plan is rendered but no kernel state is changed".to_string());
    }
    if config.firewall.allow_general_internet {
        warnings.push(
            "general outbound IPv4 from the TETRA packet-data network is enabled".to_string(),
        );
    }
    KernelPlan {
        mode: config.interface.mode.clone(),
        authoritative: config.interface.mode == MODE_AUTHORITATIVE,
        revision: snapshot.revision,
        commands,
        nft_ruleset: render_nft(config, snapshot),
        warnings,
    }
}

pub fn reconcile(
    config: &IpGatewayConfig,
    snapshot: &KernelStateSnapshot,
    previous: Option<&KernelStateSnapshot>,
) -> Result<KernelPlan, String> {
    let plan = build_plan(config, snapshot);
    if config.interface.mode != MODE_AUTHORITATIVE {
        return Ok(plan);
    }

    let mtu = config.interface.mtu.to_string();
    run(
        "ip",
        &[
            "link",
            "set",
            "dev",
            &config.interface.name,
            "mtu",
            mtu.as_str(),
            "up",
        ],
    )?;
    let mut address_arguments = vec![
        "address",
        "replace",
        config.interface.address.as_str(),
        "dev",
        config.interface.name.as_str(),
    ];
    if !config.routing.install_connected_route {
        address_arguments.push("noprefixroute");
    }
    run("ip", &address_arguments)?;
    if config.routing.enable_ipv4_forwarding {
        std::fs::write("/proc/sys/net/ipv4/ip_forward", b"1\n")
            .map_err(|error| format!("enable IPv4 forwarding: {error}"))?;
    }

    if let Some(previous) = previous {
        for old_route in previous.routes.iter().filter(|route| route.enabled) {
            let current = snapshot
                .routes
                .iter()
                .find(|route| route.id == old_route.id && route.enabled);
            let changed = current.map_or(true, |route| {
                route.destination != old_route.destination
                    || route.gateway != old_route.gateway
                    || route.interface != old_route.interface
                    || route.metric != old_route.metric
            });
            if changed {
                let _ = run_route("del", old_route, &config.interface.name);
            }
        }
    }
    for route in snapshot.routes.iter().filter(|route| route.enabled) {
        run_route("replace", route, &config.interface.name)?;
    }

    let _ = run("nft", &["delete", "table", "inet", FILTER_TABLE]);
    let _ = run("nft", &["delete", "table", "ip", NAT_TABLE]);
    if config.firewall.enabled || config.nat.enabled {
        apply_nft(&plan.nft_ruleset)?;
    }
    Ok(plan)
}

fn run_route(action: &str, route: &RouteRule, default_interface: &str) -> Result<(), String> {
    let mut arguments = vec!["route".to_string(), action.to_string(), route.destination.clone()];
    if let Some(gateway) = &route.gateway {
        arguments.push("via".to_string());
        arguments.push(gateway.clone());
    }
    arguments.push("dev".to_string());
    arguments.push(
        route
            .interface
            .clone()
            .unwrap_or_else(|| default_interface.to_string()),
    );
    if let Some(metric) = route.metric {
        arguments.push("metric".to_string());
        arguments.push(metric.to_string());
    }
    let refs: Vec<_> = arguments.iter().map(String::as_str).collect();
    run("ip", &refs)
}

fn format_route_command(action: &str, route: &RouteRule, default_interface: &str) -> String {
    let mut command = format!("ip route {action} {}", route.destination);
    if let Some(gateway) = &route.gateway {
        command.push_str(&format!(" via {gateway}"));
    }
    command.push_str(&format!(
        " dev {}",
        route.interface.as_deref().unwrap_or(default_interface)
    ));
    if let Some(metric) = route.metric {
        command.push_str(&format!(" metric {metric}"));
    }
    command
}

fn render_nft(config: &IpGatewayConfig, snapshot: &KernelStateSnapshot) -> String {
    let mut output = String::new();
    if config.firewall.enabled {
        output.push_str(&format!("table inet {FILTER_TABLE} {{\n"));
        output.push_str("  chain input {\n    type filter hook input priority 0; policy accept;\n");
        for blocked in &snapshot.blocked_addresses {
            output.push_str(&format!(
                "    iifname \"{}\" ip saddr {} drop comment \"operator block\"\n",
                nft_escape(&config.interface.name),
                blocked.address
            ));
        }
        output.push_str("    ct state established,related accept\n");
        if config.dns.enabled {
            output.push_str(&format!(
                "    iifname \"{}\" udp dport {} accept comment \"NetCore DNS\"\n",
                nft_escape(&config.interface.name),
                config.dns.bind.port()
            ));
        }
        if config.test_server.enabled {
            output.push_str(&format!(
                "    iifname \"{}\" tcp dport {} accept comment \"NetCore WAP/test server\"\n",
                nft_escape(&config.interface.name),
                config.test_server.bind.port()
            ));
            output.push_str(&format!(
                "    iifname \"{}\" udp dport {} accept comment \"NetCore UDP echo\"\n",
                nft_escape(&config.interface.name),
                config.test_server.udp_echo_bind.port()
            ));
        }
        if config.firewall.allow_icmp {
            output.push_str(&format!(
                "    iifname \"{}\" ip protocol icmp accept\n",
                nft_escape(&config.interface.name)
            ));
        }
        render_firewall_rules(&mut output, "input", snapshot);
        output.push_str(&format!(
            "    iifname \"{}\" drop comment \"Block unlisted access from TETRA packet data\"\n",
            nft_escape(&config.interface.name)
        ));
        output.push_str("  }\n");

        output.push_str(&format!(
            "  chain forward {{\n    type filter hook forward priority 0; policy {};\n",
            if config.firewall.default_forward_policy == "accept" {
                "accept"
            } else {
                "drop"
            }
        ));
        for blocked in &snapshot.blocked_addresses {
            output.push_str(&format!(
                "    ip saddr {} drop comment \"operator block\"\n    ip daddr {} drop comment \"operator block\"\n",
                blocked.address, blocked.address
            ));
        }
        if config.firewall.allow_established {
            output.push_str("    ct state established,related accept\n");
        }
        render_firewall_rules(&mut output, "forward", snapshot);
        if config.firewall.allow_icmp {
            output.push_str(&format!(
                "    iifname \"{}\" ip protocol icmp accept\n",
                nft_escape(&config.interface.name)
            ));
        }
        if config.firewall.allow_general_internet {
            output.push_str(&format!(
                "    iifname \"{}\" oifname \"{}\" ip saddr {} accept comment \"TETRA packet data outbound\"\n",
                nft_escape(&config.interface.name),
                nft_escape(&config.nat.egress_interface),
                config.interface.network
            ));
        }
        if config.firewall.log_drops {
            output.push_str("    limit rate 10/second log prefix \"netcore-ip-gateway drop \"\n");
        }
        output.push_str("  }\n");

        output.push_str("  chain output {\n    type filter hook output priority 0; policy accept;\n");
        render_firewall_rules(&mut output, "output", snapshot);
        output.push_str("  }\n}\n");
    }

    if config.nat.enabled {
        output.push_str(&format!("table ip {NAT_TABLE} {{\n"));
        output.push_str("  chain prerouting {\n    type nat hook prerouting priority dstnat; policy accept;\n");
        for rule in snapshot
            .nat_rules
            .iter()
            .filter(|rule| rule.enabled && rule.kind == "dnat")
        {
            output.push_str("    ");
            output.push_str(&render_nat_rule(rule, config));
            output.push('\n');
        }
        output.push_str("  }\n");
        output.push_str("  chain postrouting {\n    type nat hook postrouting priority srcnat; policy accept;\n");
        if config.nat.masquerade {
            output.push_str(&format!(
                "    oifname \"{}\" ip saddr {} masquerade comment \"NetCore default NAT\"\n",
                nft_escape(&config.nat.egress_interface),
                config.interface.network
            ));
        }
        for rule in snapshot
            .nat_rules
            .iter()
            .filter(|rule| rule.enabled && rule.kind != "dnat")
        {
            output.push_str("    ");
            output.push_str(&render_nat_rule(rule, config));
            output.push('\n');
        }
        output.push_str("  }\n}\n");
    }
    output
}

fn render_firewall_rules(output: &mut String, chain: &str, snapshot: &KernelStateSnapshot) {
    let mut rules: Vec<&FirewallRule> = snapshot
        .firewall_rules
        .iter()
        .filter(|rule| rule.enabled && rule.chain == chain)
        .collect();
    rules.sort_by_key(|rule| rule.priority);
    for rule in rules {
        output.push_str("    ");
        if let Some(interface) = &rule.in_interface {
            output.push_str(&format!("iifname \"{}\" ", nft_escape(interface)));
        }
        if let Some(interface) = &rule.out_interface {
            output.push_str(&format!("oifname \"{}\" ", nft_escape(interface)));
        }
        if let Some(source) = &rule.source_cidr {
            output.push_str(&format!("ip saddr {source} "));
        }
        if let Some(destination) = &rule.destination_cidr {
            output.push_str(&format!("ip daddr {destination} "));
        }
        match rule.protocol.as_str() {
            "tcp" | "udp" => {
                output.push_str(&format!("{} ", rule.protocol));
                if let Some(port) = rule.source_port {
                    output.push_str(&format!("sport {port} "));
                }
                if let Some(port) = rule.destination_port {
                    output.push_str(&format!("dport {port} "));
                }
            }
            "icmp" => output.push_str("ip protocol icmp "),
            _ => {}
        }
        if rule.log {
            output.push_str(&format!(
                "limit rate 10/second log prefix \"netcore {} \" ",
                nft_escape(&rule.name)
            ));
        }
        output.push_str(&rule.action);
        output.push_str(&format!(" comment \"{}\"\n", nft_escape(&rule.name)));
    }
}

fn render_nat_rule(rule: &NatRule, config: &IpGatewayConfig) -> String {
    let mut output = String::new();
    if let Some(interface) = &rule.out_interface {
        output.push_str(&format!("oifname \"{}\" ", nft_escape(interface)));
    } else if rule.kind != "dnat" {
        output.push_str(&format!(
            "oifname \"{}\" ",
            nft_escape(&config.nat.egress_interface)
        ));
    }
    if let Some(source) = &rule.source_cidr {
        output.push_str(&format!("ip saddr {source} "));
    }
    if let Some(destination) = &rule.destination_cidr {
        output.push_str(&format!("ip daddr {destination} "));
    }
    if let Some(protocol) = &rule.protocol {
        output.push_str(&format!("{protocol} "));
        if let Some(port) = rule.destination_port {
            output.push_str(&format!("dport {port} "));
        }
    }
    match rule.kind.as_str() {
        "masquerade" => output.push_str("masquerade"),
        "snat" => {
            output.push_str(&format!("snat to {}", rule.to_address.as_deref().unwrap_or("0.0.0.0")));
            if let Some(port) = rule.to_port {
                output.push_str(&format!(":{port}"));
            }
        }
        "dnat" => {
            output.push_str(&format!("dnat to {}", rule.to_address.as_deref().unwrap_or("0.0.0.0")));
            if let Some(port) = rule.to_port {
                output.push_str(&format!(":{port}"));
            }
        }
        _ => output.push_str("counter"),
    }
    output.push_str(&format!(" comment \"{}\"", nft_escape(&rule.name)));
    output
}

fn apply_nft(ruleset: &str) -> Result<(), String> {
    let mut child = Command::new("nft")
        .args(["-f", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("start nft: {error}"))?;
    child
        .stdin
        .as_mut()
        .ok_or_else(|| "nft stdin unavailable".to_string())?
        .write_all(ruleset.as_bytes())
        .map_err(|error| format!("write nft ruleset: {error}"))?;
    let output = child
        .wait_with_output()
        .map_err(|error| format!("wait for nft: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "nft failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn run(program: &str, arguments: &[&str]) -> Result<(), String> {
    let output = Command::new(program)
        .args(arguments)
        .output()
        .map_err(|error| format!("start {program}: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{} {} failed: {}",
            program,
            arguments.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn nft_escape(value: &str) -> String {
    value.replace('\\', "_").replace('"', "_")
}

//! Linux TUN gateway for general IPv4 packet data over TETRA SNDCP.
//!
//! SNDCP transports network-layer N-PDUs, therefore TUN (raw IP) is the
//! correct kernel integration point. TAP/Ethernet framing is intentionally not
//! used. The Linux kernel supplies routing, local sockets, TCP/UDP/ICMP,
//! conntrack and optional source NAT.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::net::Ipv4Addr;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crossbeam_channel::{bounded, Receiver, Sender, TryRecvError, TrySendError};
use tetra_config::bluestation::{CfgPacketDataGateway, PacketGatewayFirewallBackend, PacketGatewayNatMode};

const MAX_IPV4_PACKET: usize = 65_535;
const WORKER_IDLE: Duration = Duration::from_millis(2);
const NFT_TABLE: &str = "netcore_tetra";
const IPT_FILTER_CHAIN: &str = "NETCORE_TETRA_FWD";
const IPT_NAT_CHAIN: &str = "NETCORE_TETRA_NAT";
const SYSCTL_STATE_PATH: &str = "/run/netcore-tetra/packet-gateway-sysctls";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacketGatewayConfig {
    pub interface_name: String,
    pub gateway_address: Ipv4Addr,
    pub prefix_len: u8,
    pub mtu: u16,
    pub auto_configure: bool,
    pub enable_ipv4_forwarding: bool,
    pub managed_forwarding: bool,
    pub allow_unsolicited_inbound: bool,
    pub nat_mode: PacketGatewayNatMode,
    pub firewall_backend: PacketGatewayFirewallBackend,
    pub external_interface: Option<String>,
    pub channel_capacity: usize,
}

impl PacketGatewayConfig {
    pub fn from_cell(cfg: &CfgPacketDataGateway, gateway_address: Ipv4Addr, negotiated_mtu: usize) -> Self {
        let negotiated = negotiated_mtu.min(u16::MAX as usize) as u16;
        let mtu = cfg.mtu.unwrap_or(negotiated).min(negotiated).max(68);
        Self {
            interface_name: cfg.interface_name.clone(),
            gateway_address,
            prefix_len: cfg.prefix_len,
            mtu,
            auto_configure: cfg.auto_configure,
            enable_ipv4_forwarding: cfg.enable_ipv4_forwarding,
            managed_forwarding: cfg.managed_forwarding,
            allow_unsolicited_inbound: cfg.allow_unsolicited_inbound,
            nat_mode: cfg.nat_mode,
            firewall_backend: cfg.firewall_backend,
            external_interface: cfg.external_interface.clone(),
            channel_capacity: cfg.channel_capacity.max(16),
        }
    }

    pub fn network_cidr(&self) -> String {
        let address = u32::from(self.gateway_address);
        let mask = if self.prefix_len == 0 { 0 } else { u32::MAX << (32 - self.prefix_len) };
        format!("{}/{}", Ipv4Addr::from(address & mask), self.prefix_len)
    }

    fn validate(&self) -> Result<(), GatewayError> {
        validate_interface_name(&self.interface_name)?;
        if !(1..=30).contains(&self.prefix_len) {
            return Err(GatewayError::InvalidConfig("prefix_len must be 1..=30".into()));
        }
        if self.mtu < 68 {
            return Err(GatewayError::InvalidConfig("MTU must be at least 68 octets".into()));
        }
        if let Some(interface) = &self.external_interface {
            validate_interface_name(interface)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum GatewayError {
    UnsupportedPlatform,
    InvalidConfig(String),
    Io(io::Error),
    Command { program: String, args: Vec<String>, status: Option<i32>, stderr: String },
    WorkerStartup(String),
    ChannelClosed,
    ChannelFull,
}

impl From<io::Error> for GatewayError {
    fn from(value: io::Error) -> Self { Self::Io(value) }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct GatewayStatsSnapshot {
    pub packets_from_mobile: u64,
    pub bytes_from_mobile: u64,
    pub packets_to_mobile: u64,
    pub bytes_to_mobile: u64,
    pub dropped_from_mobile: u64,
    pub dropped_to_mobile: u64,
    pub io_errors: u64,
    pub running: bool,
}

#[derive(Default)]
struct GatewayStats {
    packets_from_mobile: AtomicU64,
    bytes_from_mobile: AtomicU64,
    packets_to_mobile: AtomicU64,
    bytes_to_mobile: AtomicU64,
    dropped_from_mobile: AtomicU64,
    dropped_to_mobile: AtomicU64,
    io_errors: AtomicU64,
    running: AtomicBool,
}

struct RunningGuard(Arc<GatewayStats>);

impl Drop for RunningGuard {
    fn drop(&mut self) {
        self.0.running.store(false, Ordering::Release);
    }
}

impl GatewayStats {
    fn snapshot(&self) -> GatewayStatsSnapshot {
        GatewayStatsSnapshot {
            packets_from_mobile: self.packets_from_mobile.load(Ordering::Relaxed),
            bytes_from_mobile: self.bytes_from_mobile.load(Ordering::Relaxed),
            packets_to_mobile: self.packets_to_mobile.load(Ordering::Relaxed),
            bytes_to_mobile: self.bytes_to_mobile.load(Ordering::Relaxed),
            dropped_from_mobile: self.dropped_from_mobile.load(Ordering::Relaxed),
            dropped_to_mobile: self.dropped_to_mobile.load(Ordering::Relaxed),
            io_errors: self.io_errors.load(Ordering::Relaxed),
            running: self.running.load(Ordering::Acquire),
        }
    }
}

pub struct PacketGateway {
    inject_tx: Sender<Vec<u8>>,
    egress_rx: Receiver<Vec<u8>>,
    stop_tx: Sender<()>,
    stats: Arc<GatewayStats>,
    join: Option<JoinHandle<()>>,
}

impl PacketGateway {
    pub fn spawn(config: PacketGatewayConfig) -> Result<Self, GatewayError> {
        config.validate()?;
        if !cfg!(target_os = "linux") {
            return Err(GatewayError::UnsupportedPlatform);
        }
        let (inject_tx, inject_rx) = bounded(config.channel_capacity);
        let (egress_tx, egress_rx) = bounded(config.channel_capacity);
        let (stop_tx, stop_rx) = bounded(1);
        let (startup_tx, startup_rx) = bounded(1);
        let stats = Arc::new(GatewayStats::default());
        let worker_stats = Arc::clone(&stats);
        let thread_name = format!("sndcp-{}", config.interface_name);
        let join = thread::Builder::new().name(thread_name).spawn(move || {
            let result = worker_main(config, inject_rx, egress_tx, stop_rx, worker_stats, startup_tx.clone());
            if let Err(error) = result {
                let _ = startup_tx.try_send(Err(format_gateway_error(&error)));
                tracing::error!("SNDCP packet gateway stopped: {:?}", error);
            }
        })?;
        match startup_rx.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(())) => Ok(Self { inject_tx, egress_rx, stop_tx, stats, join: Some(join) }),
            Ok(Err(error)) => {
                let _ = stop_tx.try_send(());
                let _ = join.join();
                Err(GatewayError::WorkerStartup(error))
            }
            Err(error) => {
                let _ = stop_tx.try_send(());
                let _ = join.join();
                Err(GatewayError::WorkerStartup(format!("startup timeout: {error}")))
            }
        }
    }

    pub fn inject_from_mobile(&self, packet: Vec<u8>) -> Result<(), GatewayError> {
        match self.inject_tx.try_send(packet) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => {
                self.stats.dropped_from_mobile.fetch_add(1, Ordering::Relaxed);
                Err(GatewayError::ChannelFull)
            }
            Err(TrySendError::Disconnected(_)) => Err(GatewayError::ChannelClosed),
        }
    }

    pub fn try_packet_to_mobile(&self) -> Result<Option<Vec<u8>>, GatewayError> {
        match self.egress_rx.try_recv() {
            Ok(packet) => Ok(Some(packet)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(GatewayError::ChannelClosed),
        }
    }

    pub fn stats(&self) -> GatewayStatsSnapshot { self.stats.snapshot() }
}

impl Drop for PacketGateway {
    fn drop(&mut self) {
        let _ = self.stop_tx.try_send(());
        if let Some(join) = self.join.take() { let _ = join.join(); }
    }
}

fn format_gateway_error(error: &GatewayError) -> String {
    match error {
        GatewayError::UnsupportedPlatform => "unsupported platform".to_string(),
        GatewayError::InvalidConfig(value) => format!("invalid configuration: {value}"),
        GatewayError::Io(value) => format!("I/O: {value}"),
        GatewayError::Command { program, args, status, stderr } => {
            format!("command {program} {} failed status={status:?}: {stderr}", args.join(" "))
        }
        GatewayError::WorkerStartup(value) => value.clone(),
        GatewayError::ChannelClosed => "channel closed".to_string(),
        GatewayError::ChannelFull => "channel full".to_string(),
    }
}

fn worker_main(
    config: PacketGatewayConfig,
    inject_rx: Receiver<Vec<u8>>,
    egress_tx: Sender<Vec<u8>>,
    stop_rx: Receiver<()>,
    stats: Arc<GatewayStats>,
    startup_tx: Sender<Result<(), String>>,
) -> Result<(), GatewayError> {
    let mut tun = open_tun(&config.interface_name)?;
    set_nonblocking(&tun)?;
    let mut network = NetworkConfiguration::new(config.clone());
    if config.auto_configure { network.apply()?; }
    stats.running.store(true, Ordering::Release);
    let _running_guard = RunningGuard(Arc::clone(&stats));
    let _ = startup_tx.send(Ok(()));
    let mut pending_inject: Option<Vec<u8>> = None;
    let mut read_buffer = vec![0u8; MAX_IPV4_PACKET];

    'worker: loop {
        if stop_rx.try_recv().is_ok() { break; }
        loop {
            if pending_inject.is_none() {
                pending_inject = match inject_rx.try_recv() {
                    Ok(packet) => Some(packet),
                    Err(TryRecvError::Empty) => None,
                    Err(TryRecvError::Disconnected) => break 'worker,
                };
            }
            let Some(packet) = pending_inject.as_ref() else { break; };
            match tun.write(packet) {
                Ok(written) if written == packet.len() => {
                    let packet = pending_inject.take().expect("pending packet existed");
                    stats.packets_from_mobile.fetch_add(1, Ordering::Relaxed);
                    stats.bytes_from_mobile.fetch_add(packet.len() as u64, Ordering::Relaxed);
                }
                Ok(_) => {
                    pending_inject = None;
                    stats.dropped_from_mobile.fetch_add(1, Ordering::Relaxed);
                    stats.io_errors.fetch_add(1, Ordering::Relaxed);
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => {
                    pending_inject = None;
                    stats.dropped_from_mobile.fetch_add(1, Ordering::Relaxed);
                    stats.io_errors.fetch_add(1, Ordering::Relaxed);
                    tracing::warn!("SNDCP TUN write failed: {}", error);
                }
            }
        }
        loop {
            match tun.read(&mut read_buffer) {
                Ok(0) => break,
                Ok(length) => {
                    let packet = read_buffer[..length].to_vec();
                    match egress_tx.try_send(packet) {
                        Ok(()) => {
                            stats.packets_to_mobile.fetch_add(1, Ordering::Relaxed);
                            stats.bytes_to_mobile.fetch_add(length as u64, Ordering::Relaxed);
                        }
                        Err(TrySendError::Full(_)) => { stats.dropped_to_mobile.fetch_add(1, Ordering::Relaxed); }
                        Err(TrySendError::Disconnected(_)) => break 'worker,
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => {
                    stats.io_errors.fetch_add(1, Ordering::Relaxed);
                    tracing::warn!("SNDCP TUN read failed: {}", error);
                    break;
                }
            }
        }
        thread::sleep(WORKER_IDLE);
    }
    network.cleanup();
    Ok(())
}

#[cfg(target_os = "linux")]
fn open_tun(name: &str) -> Result<File, GatewayError> {
    use std::os::fd::AsRawFd;
    const TUNSETIFF: libc::c_ulong = 0x4004_54ca;
    const IFF_TUN: libc::c_short = 0x0001;
    const IFF_NO_PI: libc::c_short = 0x1000;
    const IFNAMSIZ: usize = 16;

    let file = OpenOptions::new().read(true).write(true).open("/dev/net/tun")?;
    // Linux struct ifreq is 40 bytes on the supported x86_64/aarch64 targets.
    // The interface-name and flags fields occupy the leading bytes used here.
    let mut ifreq = [0u8; 40];
    let name_bytes = name.as_bytes();
    if name_bytes.is_empty() || name_bytes.len() >= IFNAMSIZ {
        return Err(GatewayError::InvalidConfig("TUN interface name must be 1-15 bytes".into()));
    }
    ifreq[..name_bytes.len()].copy_from_slice(name_bytes);
    ifreq[IFNAMSIZ..IFNAMSIZ + 2].copy_from_slice(&(IFF_TUN | IFF_NO_PI).to_ne_bytes());
    let result = unsafe { libc::ioctl(file.as_raw_fd(), TUNSETIFF, ifreq.as_mut_ptr()) };
    if result < 0 { return Err(GatewayError::Io(io::Error::last_os_error())); }
    Ok(file)
}

#[cfg(not(target_os = "linux"))]
fn open_tun(_name: &str) -> Result<File, GatewayError> { Err(GatewayError::UnsupportedPlatform) }

#[cfg(target_os = "linux")]
fn set_nonblocking(file: &File) -> Result<(), GatewayError> {
    use std::os::fd::AsRawFd;
    let flags = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFL) };
    if flags < 0 { return Err(GatewayError::Io(io::Error::last_os_error())); }
    if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(GatewayError::Io(io::Error::last_os_error()));
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn set_nonblocking(_file: &File) -> Result<(), GatewayError> { Err(GatewayError::UnsupportedPlatform) }

fn validate_interface_name(value: &str) -> Result<(), GatewayError> {
    if value.is_empty() || value.len() > 15 || !value.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.')) {
        return Err(GatewayError::InvalidConfig(format!("unsafe interface name {value:?}")));
    }
    Ok(())
}

#[derive(Debug, Clone)]
enum AppliedFirewall {
    None,
    Nftables,
    Iptables,
}

#[derive(Debug, Clone)]
struct IptRule {
    table: Option<&'static str>,
    chain: &'static str,
    spec: Vec<String>,
}

struct NetworkConfiguration {
    config: PacketGatewayConfig,
    applied_firewall: AppliedFirewall,
    sysctls: Vec<(String, String)>,
}

impl NetworkConfiguration {
    fn new(config: PacketGatewayConfig) -> Self {
        Self { config, applied_firewall: AppliedFirewall::None, sysctls: Vec::new() }
    }

    fn apply(&mut self) -> Result<(), GatewayError> {
        restore_persisted_sysctls()?;
        run("ip", &["link", "set", "dev", &self.config.interface_name, "mtu", &self.config.mtu.to_string(), "up"])?;
        let address = format!("{}/{}", self.config.gateway_address, self.config.prefix_len);
        run("ip", &["addr", "replace", &address, "dev", &self.config.interface_name])?;
        let network = self.config.network_cidr();
        run("ip", &["route", "replace", &network, "dev", &self.config.interface_name, "src", &self.config.gateway_address.to_string()])?;
        self.set_sysctl(&format!("/proc/sys/net/ipv4/conf/{}/rp_filter", self.config.interface_name), "0")?;
        self.set_sysctl(&format!("/proc/sys/net/ipv4/conf/{}/send_redirects", self.config.interface_name), "0")?;
        if self.config.enable_ipv4_forwarding { self.set_sysctl("/proc/sys/net/ipv4/ip_forward", "1")?; }
        if self.config.managed_forwarding || self.config.nat_mode == PacketGatewayNatMode::Masquerade {
            self.apply_firewall()?;
        }
        Ok(())
    }

    fn set_sysctl(&mut self, path: &str, value: &str) -> Result<(), GatewayError> {
        let previous = fs::read_to_string(path)?.trim().to_string();
        if previous == value {
            return Ok(());
        }
        self.sysctls.push((path.to_string(), previous));
        if let Err(error) = self.persist_sysctl_state() {
            self.sysctls.pop();
            return Err(error);
        }
        if let Err(error) = fs::write(path, format!("{value}\n")) {
            self.sysctls.pop();
            let _ = self.persist_sysctl_state();
            return Err(GatewayError::Io(error));
        }
        Ok(())
    }

    fn persist_sysctl_state(&self) -> Result<(), GatewayError> {
        let path = std::path::Path::new(SYSCTL_STATE_PATH);
        let Some(parent) = path.parent() else {
            return Err(GatewayError::InvalidConfig("invalid sysctl state path".into()));
        };
        fs::create_dir_all(parent)?;
        if self.sysctls.is_empty() {
            match fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(GatewayError::Io(error)),
            }
            return Ok(());
        }
        let temporary = path.with_extension("tmp");
        let mut contents = String::new();
        for (sysctl, previous) in &self.sysctls {
            contents.push_str(sysctl);
            contents.push('\t');
            contents.push_str(previous);
            contents.push('\n');
        }
        fs::write(&temporary, contents)?;
        fs::rename(temporary, path)?;
        Ok(())
    }

    fn apply_firewall(&mut self) -> Result<(), GatewayError> {
        let external = match self.config.external_interface.clone() {
            Some(value) => value,
            None => detect_default_interface()?,
        };
        validate_interface_name(&external)?;
        let backend = match self.config.firewall_backend {
            PacketGatewayFirewallBackend::Auto => {
                if command_exists("nft") { PacketGatewayFirewallBackend::Nftables }
                else if command_exists("iptables") { PacketGatewayFirewallBackend::Iptables }
                else { return Err(GatewayError::InvalidConfig("neither nft nor iptables is available".into())); }
            }
            other => other,
        };
        match backend {
            PacketGatewayFirewallBackend::Nftables => self.apply_nftables(&external),
            PacketGatewayFirewallBackend::Iptables => self.apply_iptables(&external),
            PacketGatewayFirewallBackend::None => Ok(()),
            PacketGatewayFirewallBackend::Auto => unreachable!(),
        }
    }

    fn apply_nftables(&mut self, external: &str) -> Result<(), GatewayError> {
        let _ = run_allow_failure("nft", &["delete", "table", "ip", NFT_TABLE]);
        run("nft", &["add", "table", "ip", NFT_TABLE])?;
        let result = (|| {
            run("nft", &["add", "chain", "ip", NFT_TABLE, "forward", "{", "type", "filter", "hook", "forward", "priority", "filter", ";", "policy", "accept", ";", "}"])?;
            run("nft", &["add", "rule", "ip", NFT_TABLE, "forward", "iifname", &self.config.interface_name, "oifname", &self.config.interface_name, "accept"])?;
            run("nft", &["add", "rule", "ip", NFT_TABLE, "forward", "iifname", &self.config.interface_name, "oifname", external, "accept"])?;
            if self.config.allow_unsolicited_inbound {
                run("nft", &["add", "rule", "ip", NFT_TABLE, "forward", "iifname", external, "oifname", &self.config.interface_name, "accept"])?;
            } else {
                run("nft", &["add", "rule", "ip", NFT_TABLE, "forward", "iifname", external, "oifname", &self.config.interface_name, "ct", "state", "established,related", "accept"])?;
            }
            // Do not depend on the host's global FORWARD policy for subscriber isolation.
            run("nft", &["add", "rule", "ip", NFT_TABLE, "forward", "iifname", &self.config.interface_name, "drop"])?;
            run("nft", &["add", "rule", "ip", NFT_TABLE, "forward", "oifname", &self.config.interface_name, "drop"])?;
            if self.config.nat_mode == PacketGatewayNatMode::Masquerade {
                run("nft", &["add", "chain", "ip", NFT_TABLE, "postrouting", "{", "type", "nat", "hook", "postrouting", "priority", "srcnat", ";", "policy", "accept", ";", "}"])?;
                run("nft", &["add", "rule", "ip", NFT_TABLE, "postrouting", "ip", "saddr", &self.config.network_cidr(), "oifname", external, "masquerade"])?;
            }
            Ok(())
        })();
        if result.is_err() { let _ = run_allow_failure("nft", &["delete", "table", "ip", NFT_TABLE]); }
        result?;
        self.applied_firewall = AppliedFirewall::Nftables;
        Ok(())
    }

    fn apply_iptables(&mut self, external: &str) -> Result<(), GatewayError> {
        let nat_enabled = self.config.nat_mode == PacketGatewayNatMode::Masquerade;
        cleanup_iptables_chains(true);
        let result = (|| -> Result<(), GatewayError> {
            run("iptables", &["-N", IPT_FILTER_CHAIN])?;
            run("iptables", &["-A", IPT_FILTER_CHAIN, "-i", &self.config.interface_name, "-o", &self.config.interface_name, "-j", "ACCEPT"])?;
            run("iptables", &["-A", IPT_FILTER_CHAIN, "-i", &self.config.interface_name, "-o", external, "-j", "ACCEPT"])?;
            if self.config.allow_unsolicited_inbound {
                run("iptables", &["-A", IPT_FILTER_CHAIN, "-i", external, "-o", &self.config.interface_name, "-j", "ACCEPT"])?;
            } else {
                run("iptables", &["-A", IPT_FILTER_CHAIN, "-i", external, "-o", &self.config.interface_name, "-m", "conntrack", "--ctstate", "ESTABLISHED,RELATED", "-j", "ACCEPT"])?;
            }
            run("iptables", &["-A", IPT_FILTER_CHAIN, "-i", &self.config.interface_name, "-j", "DROP"])?;
            run("iptables", &["-A", IPT_FILTER_CHAIN, "-o", &self.config.interface_name, "-j", "DROP"])?;
            run("iptables", &["-A", IPT_FILTER_CHAIN, "-j", "RETURN"])?;
            let forward_jump = IptRule { table: None, chain: "FORWARD", spec: vec!["-j".into(), IPT_FILTER_CHAIN.into()] };
            if !iptables_rule_exists(&forward_jump)? {
                iptables_mutate("-I", &forward_jump)?;
            }
            if nat_enabled {
                run("iptables", &["-t", "nat", "-N", IPT_NAT_CHAIN])?;
                run("iptables", &["-t", "nat", "-A", IPT_NAT_CHAIN, "-s", &self.config.network_cidr(), "-o", external, "-j", "MASQUERADE"])?;
                run("iptables", &["-t", "nat", "-A", IPT_NAT_CHAIN, "-j", "RETURN"])?;
                let nat_jump = IptRule { table: Some("nat"), chain: "POSTROUTING", spec: vec!["-j".into(), IPT_NAT_CHAIN.into()] };
                if !iptables_rule_exists(&nat_jump)? {
                    iptables_mutate("-I", &nat_jump)?;
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            cleanup_iptables_chains(true);
            return Err(error);
        }
        self.applied_firewall = AppliedFirewall::Iptables;
        Ok(())
    }

    fn cleanup(&mut self) {
        match std::mem::replace(&mut self.applied_firewall, AppliedFirewall::None) {
            AppliedFirewall::None => {}
            AppliedFirewall::Nftables => { let _ = run_allow_failure("nft", &["delete", "table", "ip", NFT_TABLE]); }
            AppliedFirewall::Iptables => {
                cleanup_iptables_chains(true);
            }
        }
        for (path, previous) in self.sysctls.drain(..).rev() {
            let _ = fs::write(path, format!("{previous}\n"));
        }
        let _ = fs::remove_file(SYSCTL_STATE_PATH);
    }
}

impl Drop for NetworkConfiguration {
    fn drop(&mut self) { self.cleanup(); }
}

fn restore_persisted_sysctls() -> Result<(), GatewayError> {
    let path = std::path::Path::new(SYSCTL_STATE_PATH);
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(GatewayError::Io(error)),
    };
    for line in contents.lines() {
        let Some((sysctl, previous)) = line.split_once('\t') else { continue; };
        if sysctl.starts_with("/proc/sys/net/") {
            fs::write(sysctl, format!("{previous}\n"))?;
        }
    }
    fs::remove_file(path)?;
    Ok(())
}

fn cleanup_iptables_chains(include_nat: bool) {
    let forward_jump = IptRule { table: None, chain: "FORWARD", spec: vec!["-j".into(), IPT_FILTER_CHAIN.into()] };
    for _ in 0..16 {
        match iptables_rule_exists(&forward_jump) {
            Ok(true) => { let _ = iptables_mutate("-D", &forward_jump); }
            _ => break,
        }
    }
    let _ = run_allow_failure("iptables", &["-F", IPT_FILTER_CHAIN]);
    let _ = run_allow_failure("iptables", &["-X", IPT_FILTER_CHAIN]);

    if include_nat {
        let nat_jump = IptRule { table: Some("nat"), chain: "POSTROUTING", spec: vec!["-j".into(), IPT_NAT_CHAIN.into()] };
        for _ in 0..16 {
            match iptables_rule_exists(&nat_jump) {
                Ok(true) => { let _ = iptables_mutate("-D", &nat_jump); }
                _ => break,
            }
        }
        let _ = run_allow_failure("iptables", &["-t", "nat", "-F", IPT_NAT_CHAIN]);
        let _ = run_allow_failure("iptables", &["-t", "nat", "-X", IPT_NAT_CHAIN]);
    }
}

fn command_exists(program: &str) -> bool {
    Command::new(program).arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok()
}

fn detect_default_interface() -> Result<String, GatewayError> {
    let routes = fs::read_to_string("/proc/net/route")?;
    for line in routes.lines().skip(1) {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.len() >= 4 && fields[1] == "00000000" {
            let flags = u16::from_str_radix(fields[3], 16).unwrap_or(0);
            if flags & 0x0001 != 0 {
                validate_interface_name(fields[0])?;
                return Ok(fields[0].to_string());
            }
        }
    }
    Err(GatewayError::InvalidConfig("could not detect default-route interface".into()))
}

fn run(program: &str, args: &[&str]) -> Result<(), GatewayError> {
    let output = Command::new(program).args(args).output().map_err(GatewayError::Io)?;
    if output.status.success() { return Ok(()); }
    Err(GatewayError::Command {
        program: program.to_string(),
        args: args.iter().map(|value| (*value).to_string()).collect(),
        status: output.status.code(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
    })
}

fn run_allow_failure(program: &str, args: &[&str]) -> bool {
    matches!(
        Command::new(program).args(args).stdout(Stdio::null()).stderr(Stdio::null()).status(),
        Ok(status) if status.success()
    )
}

fn iptables_args(action: &str, rule: &IptRule) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(table) = rule.table { args.extend(["-t".to_string(), table.to_string()]); }
    args.push(action.to_string());
    args.push(rule.chain.to_string());
    args.extend(rule.spec.clone());
    args
}

fn iptables_rule_exists(rule: &IptRule) -> Result<bool, GatewayError> {
    let args = iptables_args("-C", rule);
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    let status = Command::new("iptables").args(&refs).stdout(Stdio::null()).stderr(Stdio::null()).status()?;
    Ok(status.success())
}

fn iptables_mutate(action: &str, rule: &IptRule) -> Result<(), GatewayError> {
    let args = iptables_args(action, rule);
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    run("iptables", &refs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_cidr_uses_gateway_prefix() {
        let cfg = PacketGatewayConfig {
            interface_name: "ntetra0".into(), gateway_address: Ipv4Addr::new(10, 23, 4, 1), prefix_len: 24,
            mtu: 576, auto_configure: false, enable_ipv4_forwarding: false, managed_forwarding: false,
            allow_unsolicited_inbound: false, nat_mode: PacketGatewayNatMode::Disabled, firewall_backend: PacketGatewayFirewallBackend::None,
            external_interface: None, channel_capacity: 32,
        };
        assert_eq!(cfg.network_cidr(), "10.23.4.0/24");
    }

    #[test]
    fn iptables_nat_table_precedes_action() {
        let rule = IptRule { table: Some("nat"), chain: "POSTROUTING", spec: vec!["-j".into(), IPT_NAT_CHAIN.into()] };
        assert_eq!(iptables_args("-I", &rule)[..4].join(" "), "-t nat -I POSTROUTING");
    }
}

use serde::Deserialize;
use std::collections::HashMap;

use tetra_core::ranges::{SortedDisjointSsiRanges, SsiRange};
use toml::Value;

/// Text coding scheme for Home Mode Display SDS payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum HomeModeDisplaySdsTextCodingScheme {
    /// ISO-8859-1 8-bit Latin alphabet
    LATIN,
    /// UCS-2 / UTF-16BE for Unicode text
    UTF16,
}

/// Compiled Home Mode Display configuration (present when `[cell_info.home_mode_display]` exists).
#[derive(Debug, Clone)]
pub struct CfgHomeModeDisplay {
    /// ISSI used as the source address of the broadcast SDS.
    pub source_issi: u32,
    /// Broadcast interval in TDMA multiframes (1 multiframe = 18 frames = 72 timeslots).
    pub interval_multiframes: u32,
    /// SDS Type4 protocol identifier byte. Default: 220 (0xDC).
    pub protocol_id: u8,
    /// Text coding scheme prepended to user data. LATIN = ISO-8859-1, UTF16 = UCS-2/UTF-16BE.
    pub text_coding_scheme: HomeModeDisplaySdsTextCodingScheme,
    /// Text to broadcast (UTF-8 source, encoded per text_coding_scheme on TX).
    pub text: String,
}

/// Serde DTO for `[cell_info.home_mode_display]` config block.
#[derive(Default, Deserialize)]
pub struct HomeModeDisplayDto {
    pub source_issi: Option<u32>,
    #[serde(alias = "interval_frames")]
    pub interval_multiframes: Option<u32>,
    pub protocol_id: Option<u8>,
    pub text_coding_scheme: Option<HomeModeDisplaySdsTextCodingScheme>,
    pub text: Option<String>,
}


/// Compiled configuration for the opt-in WAP-over-SNDCP IPv4 service.
#[derive(Debug, Clone)]
pub struct CfgWapIp {
    pub enabled: bool,
    pub address: std::net::Ipv4Addr,
    pub port: u16,
    pub response_ttl: u8,
    /// Three-octet prefix written as `a.b.c`.
    pub dynamic_pool_prefix: String,
    pub dynamic_pool_first_host: u8,
    pub dynamic_pool_last_host: u8,
    pub allow_static_ipv4: bool,
    pub accept_empty_probe: bool,
    pub accept_root_path: bool,
    pub accept_status_path: bool,
    pub accept_status_wml_path: bool,
    pub max_request_payload_bytes: usize,
    pub assume_pdch_ready_after_data_transmit: bool,
    /// SNDCP timer/priority profile advertised in ACTIVATE ACCEPT.
    pub pdu_priority_max: u8,
    pub ready_timer_code: u8,
    pub standby_timer_code: u8,
    pub response_wait_timer_code: u8,
    pub mtu_code: u8,
    pub network_default_data_priority: u8,
    /// Resource bounds for all primary and secondary PDP contexts.
    pub max_contexts_per_issi: usize,
    pub max_total_contexts: usize,
    /// Reject source-address spoofing inside the local packet-data profile.
    pub strict_source_address: bool,
}

impl Default for CfgWapIp {
    fn default() -> Self {
        Self {
            enabled: false,
            address: std::net::Ipv4Addr::new(10, 0, 0, 1),
            port: 9200,
            response_ttl: 32,
            dynamic_pool_prefix: "10.0.0".to_string(),
            dynamic_pool_first_host: 2,
            dynamic_pool_last_host: 254,
            allow_static_ipv4: true,
            accept_empty_probe: true,
            accept_root_path: true,
            accept_status_path: true,
            accept_status_wml_path: true,
            max_request_payload_bytes: 1024,
            assume_pdch_ready_after_data_transmit: false,
            pdu_priority_max: 4,
            ready_timer_code: 8,
            standby_timer_code: 4,
            response_wait_timer_code: 7,
            mtu_code: 2,
            network_default_data_priority: 4,
            max_contexts_per_issi: 4,
            max_total_contexts: 64,
            strict_source_address: true,
        }
    }
}

impl CfgWapIp {
    /// Parse the configured three-octet pool prefix.
    pub fn pool_prefix_octets(&self) -> Option<[u8; 3]> {
        let parts: Vec<_> = self.dynamic_pool_prefix.split('.').collect();
        if parts.len() != 3 {
            return None;
        }
        Some([parts[0].parse().ok()?, parts[1].parse().ok()?, parts[2].parse().ok()?])
    }
}

/// Serde DTO for `[cell_info.wap_ip]`. Unknown keys are rejected so a typo cannot
/// leave packet data advertised while the gateway silently uses a default.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WapIpDto {
    pub enabled: Option<bool>,
    pub address: Option<std::net::Ipv4Addr>,
    pub port: Option<u16>,
    pub response_ttl: Option<u8>,
    pub dynamic_pool_prefix: Option<String>,
    pub dynamic_pool_first_host: Option<u8>,
    pub dynamic_pool_last_host: Option<u8>,
    pub allow_static_ipv4: Option<bool>,
    pub accept_empty_probe: Option<bool>,
    pub accept_root_path: Option<bool>,
    pub accept_status_path: Option<bool>,
    pub accept_status_wml_path: Option<bool>,
    pub max_request_payload_bytes: Option<usize>,
    pub assume_pdch_ready_after_data_transmit: Option<bool>,
    pub pdu_priority_max: Option<u8>,
    pub ready_timer_code: Option<u8>,
    pub standby_timer_code: Option<u8>,
    pub response_wait_timer_code: Option<u8>,
    pub mtu_code: Option<u8>,
    pub network_default_data_priority: Option<u8>,
    pub max_contexts_per_issi: Option<usize>,
    pub max_total_contexts: Option<usize>,
    pub strict_source_address: Option<bool>,
}


/// Host firewall backend used for the managed packet-data forwarding rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PacketGatewayFirewallBackend {
    Auto,
    Nftables,
    Iptables,
    None,
}

/// IPv4 source-NAT mode for traffic leaving the TETRA subscriber subnet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PacketGatewayNatMode {
    Disabled,
    Masquerade,
}

/// Linux TUN/IP gateway above SNDCP. The radio bearer carries raw IPv4 N-PDUs;
/// the kernel provides normal routing, TCP/UDP/ICMP, conntrack and optional NAT.
#[derive(Debug, Clone)]
pub struct CfgPacketDataGateway {
    pub enabled: bool,
    pub interface_name: String,
    pub prefix_len: u8,
    pub mtu: Option<u16>,
    pub auto_configure: bool,
    pub enable_ipv4_forwarding: bool,
    pub managed_forwarding: bool,
    /// Permit new connections routed from the external interface into the subscriber subnet.
    /// False keeps inbound forwarding stateful (ESTABLISHED/RELATED only).
    pub allow_unsolicited_inbound: bool,
    pub nat_mode: PacketGatewayNatMode,
    pub firewall_backend: PacketGatewayFirewallBackend,
    pub external_interface: Option<String>,
    /// RFC 1877 IPCP DNS addresses returned in PDP activation PCO. At most two.
    pub dns_servers: Vec<std::net::Ipv4Addr>,
    pub channel_capacity: usize,
    /// Maximum simultaneously active one-slot PDCH bearers. Zero means all
    /// traffic slots available for the configured carrier count.
    pub max_pdch_bearers: usize,
    /// Keep this many traffic slots outside the packet-data pool so voice and
    /// emergency call setup retain deterministic headroom. Zero disables the guard.
    pub reserved_voice_slots: usize,
    /// Prefer Carrier 2 for new packet-data bearers, preserving main-carrier
    /// traffic slots for latency-sensitive voice whenever possible.
    pub prefer_secondary_carrier: bool,
    pub downlink_queue_packets_per_context: usize,
    pub downlink_queue_bytes_per_context: usize,
    pub downlink_queue_ttl_secs: u64,
    pub page_retry_secs: u64,
    pub fragment_reassembly_timeout_secs: u64,
    pub fragment_reassembly_max_datagrams: usize,
    pub fragment_reassembly_max_bytes: usize,
    pub automatic_filter_ttl_secs: u64,
    pub automatic_filter_max_bindings: usize,
}

impl Default for CfgPacketDataGateway {
    fn default() -> Self {
        Self {
            enabled: false,
            interface_name: "ntetra0".to_string(),
            prefix_len: 24,
            mtu: None,
            auto_configure: true,
            enable_ipv4_forwarding: false,
            managed_forwarding: false,
            allow_unsolicited_inbound: false,
            nat_mode: PacketGatewayNatMode::Disabled,
            firewall_backend: PacketGatewayFirewallBackend::Auto,
            external_interface: None,
            dns_servers: Vec::new(),
            channel_capacity: 256,
            max_pdch_bearers: 0,
            reserved_voice_slots: 1,
            prefer_secondary_carrier: true,
            downlink_queue_packets_per_context: 64,
            downlink_queue_bytes_per_context: 262_144,
            downlink_queue_ttl_secs: 30,
            page_retry_secs: 5,
            fragment_reassembly_timeout_secs: 30,
            fragment_reassembly_max_datagrams: 128,
            fragment_reassembly_max_bytes: 4_194_304,
            automatic_filter_ttl_secs: 300,
            automatic_filter_max_bindings: 4096,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PacketDataGatewayDto {
    pub enabled: Option<bool>,
    pub interface_name: Option<String>,
    pub prefix_len: Option<u8>,
    pub mtu: Option<u16>,
    pub auto_configure: Option<bool>,
    pub enable_ipv4_forwarding: Option<bool>,
    pub managed_forwarding: Option<bool>,
    pub allow_unsolicited_inbound: Option<bool>,
    pub nat_mode: Option<PacketGatewayNatMode>,
    pub firewall_backend: Option<PacketGatewayFirewallBackend>,
    pub external_interface: Option<String>,
    pub dns_servers: Option<Vec<std::net::Ipv4Addr>>,
    pub channel_capacity: Option<usize>,
    pub max_pdch_bearers: Option<usize>,
    pub reserved_voice_slots: Option<usize>,
    pub prefer_secondary_carrier: Option<bool>,
    pub downlink_queue_packets_per_context: Option<usize>,
    pub downlink_queue_bytes_per_context: Option<usize>,
    pub downlink_queue_ttl_secs: Option<u64>,
    pub page_retry_secs: Option<u64>,
    pub fragment_reassembly_timeout_secs: Option<u64>,
    pub fragment_reassembly_max_datagrams: Option<usize>,
    pub fragment_reassembly_max_bytes: Option<usize>,
    pub automatic_filter_ttl_secs: Option<u64>,
    pub automatic_filter_max_bindings: Option<usize>,
}

/// Service details for a neighbor cell — mirrors BsServiceDetails but for config parsing.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CfgBsServiceDetails {
    #[serde(default)]
    pub registration: bool,
    #[serde(default)]
    pub deregistration: bool,
    #[serde(default)]
    pub priority_cell: bool,
    #[serde(default)]
    pub no_minimum_mode: bool,
    #[serde(default)]
    pub migration: bool,
    #[serde(default)]
    pub system_wide_services: bool,
    #[serde(default)]
    pub voice_service: bool,
    #[serde(default)]
    pub circuit_mode_data_service: bool,
    #[serde(default)]
    pub sndcp_service: bool,
    #[serde(default)]
    pub aie_service: bool,
    #[serde(default)]
    pub advanced_link: bool,
}

/// Configuration for a single CA neighbor cell, included in D-NWRK-BROADCAST.
/// Per ETSI EN 300 392-2 clause 18.5.17 / Table 18.64.
#[derive(Debug, Clone, Deserialize)]
pub struct CfgNeighborCellCa {
    /// 5 bits — cell identifier within the CA cluster (0-31)
    pub cell_identifier_ca: u8,
    /// 2 bits — cell reselection types supported (0-3)
    pub cell_reselection_types_supported: u8,
    /// 1 bit — true if this neighbor is time-synchronized with us
    pub neighbor_cell_synchronized: bool,
    /// 2 bits — current load indicator (0=low, 3=high)
    pub cell_load_ca: u8,
    /// 12 bits — main carrier number of the neighbor cell (0-4095)
    pub main_carrier_number: u16,

    /// Optional: carrier number extension (10 bits, 0-1023)
    pub main_carrier_number_extension: Option<u16>,
    /// Optional: MCC of the neighbor (10 bits, 0-1023)
    pub mcc: Option<u16>,
    /// Optional: MNC of the neighbor (14 bits, 0-16383)
    pub mnc: Option<u16>,
    /// Optional: location area of the neighbor (14 bits, 0-16383)
    pub location_area: Option<u16>,
    /// Optional: max MS TX power allowed in neighbor cell (3 bits, 0-7)
    pub maximum_ms_transmit_power: Option<u8>,
    /// Optional: minimum RX level for access (4 bits, 0-15)
    pub minimum_rx_access_level: Option<u8>,
    /// Optional: subscriber class mask (16 bits)
    pub subscriber_class: Option<u16>,
    /// Optional: BS service details for the neighbor cell
    pub bs_service_details: Option<CfgBsServiceDetails>,
    /// Optional: timeshare/security parameters (5 bits, 0-31)
    pub timeshare_cell_information_or_security_parameters: Option<u8>,
    /// Optional: TDMA frame offset relative to this cell (6 bits, 0-63)
    pub tdma_frame_offset: Option<u8>,
}

#[derive(Debug, Clone)]
pub struct CfgCellInfo {
    // 2 bits, from 18.4.2.1 D-MLE-SYNC
    pub neighbor_cell_broadcast: u8,
    // 2 bits, from 18.4.2.1 D-MLE-SYNC
    pub late_entry_supported: bool,

    /// 12 bits, from MAC SYSINFO
    pub main_carrier: u16,
    /// Optional secondary carrier for dual-carrier BS operation. `None` means single-carrier.
    pub secondary_carrier: Option<u16>,
    /// 4 bits, from MAC SYSINFO
    pub freq_band: u8,
    /// Offset in Hz from 25kHz aligned carrier. Options: 0, 6250, -6250, 12500 Hz
    /// Represented as 0-3 in SYSINFO
    pub freq_offset_hz: i16,
    /// Index in duplex setting table. Sent in SYSINFO. Maps to a specific duplex spacing in Hz.
    /// Custom spacing can be provided optionally by setting
    pub duplex_spacing_id: u8,
    /// Custom duplex spacing in Hz, for users that use a modified, non-standard duplex spacing table.
    pub custom_duplex_spacing: Option<u32>,
    /// 1 bits, from MAC SYSINFO
    pub reverse_operation: bool,

    // 14 bits, from 18.4.2.2 D-MLE-SYSINFO
    pub location_area: u16,
    // 16 bits, from 18.4.2.2 D-MLE-SYSINFO
    pub subscriber_class: u16,

    // 1-bit service flags
    pub registration: bool,
    pub deregistration: bool,
    pub priority_cell: bool,
    pub no_minimum_mode: bool,
    pub migration: bool,
    pub system_wide_services: bool,
    pub voice_service: bool,
    pub circuit_mode_data_service: bool,
    pub sndcp_service: bool,
    pub aie_service: bool,
    pub advanced_link: bool,

    /// Opt-in terminal-browser WAP/IP endpoint carried over SNDCP packet data.
    pub wap_ip: CfgWapIp,
    /// Optional general IPv4 TUN/router/NAT gateway above SNDCP.
    pub packet_data_gateway: CfgPacketDataGateway,

    // From SYNC
    pub system_code: u8,
    pub colour_code: u8,
    pub sharing_mode: u8,
    pub ts_reserved_frames: u8,
    pub u_plane_dtx: bool,
    pub frame_18_ext: bool,

    pub ms_txpwr_max_cell: u8,

    pub local_ssi_ranges: SortedDisjointSsiRanges,

    /// IANA timezone name (e.g. "Europe/Amsterdam"). When set, enables D-NWRK-BROADCAST
    /// time broadcasting so MSs can synchronize their clocks.
    pub timezone: Option<String>,

    /// Periodic automatic broadcast of Home Mode Display SDS (PID 220).
    /// Enabled when `Some`, i.e. `[cell_info.home_mode_display]` exists in config.
    /// Broadcasts the configured text to all MSs once per interval as a D-SDS-DATA to GSSI 0xFFFFFF.
    pub home_mode_display: Option<CfgHomeModeDisplay>,

    /// Optional supplemental periodic SDS broadcast with a custom PID.
    /// Useful for sending status messages (e.g. PID 130) alongside PID 220.
    /// Configured via `[cell_info.sds_broadcast]`. Uses the same structure as home_mode_display.
    pub sds_broadcast: Option<CfgHomeModeDisplay>,

    /// Neighbor cells to include in D-NWRK-BROADCAST for cell reselection.
    /// Up to 7 entries. MSs use this list to find alternative cells when signal degrades.
    pub neighbor_cells_ca: Vec<CfgNeighborCellCa>,

    /// Group call hangtime in seconds: how long an idle group call circuit stays open
    /// after the last speaker releases the floor, before the call is torn down.
    /// During hangtime, any MS can retake the floor without a new D-SETUP/D-CONNECT cycle.
    /// Default: 5 seconds. Range: 0–300.
    pub hangtime_secs: u32,

    /// Maximum active call duration in seconds (ETSI T310 equivalent, EN 300 392-2 §14.9.1).
    /// After this time the BS sends D-RELEASE regardless of call activity.
    /// Shorter values free up timeslots faster when MS leaves coverage without disconnecting.
    /// Default: 120 seconds (2 minutes). Range: 30–300.
    pub call_timeout_secs: u32,

    /// UL inactivity timeout in seconds: if no voice frames are received from the transmitting
    /// MS for this duration, the BS forces TX-CEASED and enters hangtime.
    /// Must be above T.213 (1s) to tolerate DTX and brief RF fading.
    /// Default: 3 seconds. Range: 1–30.
    pub ul_inactivity_secs: u32,

    /// Periodic registration interval in seconds (ETSI T351 equivalent).
    /// MS must re-register within this interval or be deregistered by the BS.
    /// 0 = disabled — MS registrations never expire.
    /// Default: 3600 (1 hour). Valid range when non-zero: 60–86400.
    pub periodic_registration_secs: u32,

    /// Remote control via SDS U-STATUS to ISSI 9999. None = disabled.
    pub sds_command_control: Option<CfgSdsCommandControl>,

    /// When true, a same-speaker floor retake during group-call hangtime tears the call down
    /// (D-RELEASE) instead of reusing the hanging circuit, so the next PTT runs a full
    /// U-SETUP/D-CONNECT/D-SETUP cycle. Workaround for legacy Motorola radios (MR5/MR19 era)
    /// that ACK a fast-retake floor grant but never key up the TCH/S, producing a "silent over"
    /// the rest of the group hears as dead air. Default: false (modern radios reuse the circuit
    /// fine and benefit from the lower retake latency). Opt in only for fleets with legacy sets.
    pub release_group_on_same_speaker_retake: bool,
}

impl CfgCellInfo {
    /// One predicate controls both capability advertisement and runtime acceptance.
    pub fn wap_ip_sndcp_profile_enabled(&self) -> bool {
        self.sndcp_service && (self.wap_ip.enabled || self.packet_data_gateway.enabled)
    }
}

#[derive(Default, Deserialize)]
pub struct CellInfoDto {
    pub main_carrier: u16,
    pub secondary_carrier: Option<u16>,
    /// Operational dual-carrier switch used by the dashboard. When false, the configured
    /// `secondary_carrier` value stays in the TOML but is not used by the running stack.
    /// Absent = true for backward compatibility.
    pub dual_carrier_enabled: Option<bool>,
    pub freq_band: u8,
    pub freq_offset: i16,
    pub duplex_spacing: u8,
    pub reverse_operation: bool,
    pub custom_duplex_spacing: Option<u32>,

    pub location_area: u16,

    pub neighbor_cell_broadcast: Option<u8>,
    pub late_entry_supported: Option<bool>,
    pub subscriber_class: Option<u16>,
    pub registration: Option<bool>,
    pub deregistration: Option<bool>,
    pub priority_cell: Option<bool>,
    pub no_minimum_mode: Option<bool>,
    pub migration: Option<bool>,
    pub system_wide_services: Option<bool>,
    pub voice_service: Option<bool>,
    pub circuit_mode_data_service: Option<bool>,
    pub sndcp_service: Option<bool>,
    pub aie_service: Option<bool>,
    pub advanced_link: Option<bool>,
    pub wap_ip: Option<WapIpDto>,
    pub packet_data_gateway: Option<PacketDataGatewayDto>,

    pub system_code: Option<u8>,
    pub colour_code: Option<u8>,
    pub sharing_mode: Option<u8>,
    pub ts_reserved_frames: Option<u8>,
    pub u_plane_dtx: Option<bool>,
    pub frame_18_ext: Option<bool>,

    pub ms_txpwr_max_cell: Option<u8>,

    pub local_ssi_ranges: Option<Vec<(u32, u32)>>,

    pub timezone: Option<String>,

    /// Home Mode Display periodic SDS broadcast. Enabled by presence of this sub-section.
    pub home_mode_display: Option<HomeModeDisplayDto>,

    /// Supplemental SDS broadcast with custom PID. Enabled by presence of this sub-section.
    pub sds_broadcast: Option<HomeModeDisplayDto>,

    /// Neighbor cells for D-NWRK-BROADCAST. Up to 7 entries.
    /// Parsed separately in parsing.rs from toml::Value to avoid serde flatten conflict.
    #[serde(skip)]
    pub neighbor_cells_ca: Vec<CfgNeighborCellCa>,

    /// Group call hangtime in seconds. Default: 5.
    pub hangtime_secs: Option<u32>,

    /// Active call timeout (T310) in seconds. Default: 120.
    pub call_timeout_secs: Option<u32>,

    /// UL inactivity timeout in seconds. Default: 3.
    pub ul_inactivity_secs: Option<u32>,

    /// Periodic registration interval in seconds. 0 = disabled. Default: 3600.
    pub periodic_registration_secs: Option<u32>,

    /// Remote control via SDS U-STATUS. Optional section.
    pub sds_command_control: Option<SdsCommandControlDto>,

    /// Tear down a group call on a same-speaker hangtime retake (legacy-Motorola silent-over
    /// workaround). Default: false.
    pub release_group_on_same_speaker_retake: Option<bool>,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

pub fn cell_dto_to_cfg(ci: CellInfoDto) -> CfgCellInfo {
    CfgCellInfo {
        main_carrier: ci.main_carrier,
        secondary_carrier: if ci.dual_carrier_enabled.unwrap_or(true) {
            ci.secondary_carrier
        } else {
            None
        },
        freq_band: ci.freq_band,
        freq_offset_hz: ci.freq_offset,
        duplex_spacing_id: ci.duplex_spacing,
        reverse_operation: ci.reverse_operation,
        custom_duplex_spacing: ci.custom_duplex_spacing,
        location_area: ci.location_area,
        neighbor_cell_broadcast: ci.neighbor_cell_broadcast.unwrap_or(0),
        late_entry_supported: ci.late_entry_supported.unwrap_or(false),
        subscriber_class: ci.subscriber_class.unwrap_or(65535), // All subscriber classes allowed
        registration: ci.registration.unwrap_or(true),
        deregistration: ci.deregistration.unwrap_or(true),
        priority_cell: ci.priority_cell.unwrap_or(false),
        no_minimum_mode: ci.no_minimum_mode.unwrap_or(false),
        migration: ci.migration.unwrap_or(false),
        system_wide_services: ci.system_wide_services.unwrap_or(false),
        voice_service: ci.voice_service.unwrap_or(true),
        circuit_mode_data_service: ci.circuit_mode_data_service.unwrap_or(false),
        sndcp_service: ci.sndcp_service.unwrap_or(false),
        aie_service: ci.aie_service.unwrap_or(false),
        advanced_link: ci.advanced_link.unwrap_or(false),
        wap_ip: {
            let dto = ci.wap_ip.unwrap_or_default();
            let defaults = CfgWapIp::default();
            CfgWapIp {
                enabled: dto.enabled.unwrap_or(defaults.enabled),
                address: dto.address.unwrap_or(defaults.address),
                port: dto.port.unwrap_or(defaults.port),
                response_ttl: dto.response_ttl.unwrap_or(defaults.response_ttl),
                dynamic_pool_prefix: dto.dynamic_pool_prefix.unwrap_or(defaults.dynamic_pool_prefix),
                dynamic_pool_first_host: dto.dynamic_pool_first_host.unwrap_or(defaults.dynamic_pool_first_host),
                dynamic_pool_last_host: dto.dynamic_pool_last_host.unwrap_or(defaults.dynamic_pool_last_host),
                allow_static_ipv4: dto.allow_static_ipv4.unwrap_or(defaults.allow_static_ipv4),
                accept_empty_probe: dto.accept_empty_probe.unwrap_or(defaults.accept_empty_probe),
                accept_root_path: dto.accept_root_path.unwrap_or(defaults.accept_root_path),
                accept_status_path: dto.accept_status_path.unwrap_or(defaults.accept_status_path),
                accept_status_wml_path: dto.accept_status_wml_path.unwrap_or(defaults.accept_status_wml_path),
                max_request_payload_bytes: dto.max_request_payload_bytes.unwrap_or(defaults.max_request_payload_bytes),
                assume_pdch_ready_after_data_transmit: dto
                    .assume_pdch_ready_after_data_transmit
                    .unwrap_or(defaults.assume_pdch_ready_after_data_transmit),
                pdu_priority_max: dto.pdu_priority_max.unwrap_or(defaults.pdu_priority_max),
                ready_timer_code: dto.ready_timer_code.unwrap_or(defaults.ready_timer_code),
                standby_timer_code: dto.standby_timer_code.unwrap_or(defaults.standby_timer_code),
                response_wait_timer_code: dto.response_wait_timer_code.unwrap_or(defaults.response_wait_timer_code),
                mtu_code: dto.mtu_code.unwrap_or(defaults.mtu_code),
                network_default_data_priority: dto
                    .network_default_data_priority
                    .unwrap_or(defaults.network_default_data_priority),
                max_contexts_per_issi: dto.max_contexts_per_issi.unwrap_or(defaults.max_contexts_per_issi),
                max_total_contexts: dto.max_total_contexts.unwrap_or(defaults.max_total_contexts),
                strict_source_address: dto.strict_source_address.unwrap_or(defaults.strict_source_address),
            }
        },
        packet_data_gateway: {
            let dto = ci.packet_data_gateway.unwrap_or_default();
            let defaults = CfgPacketDataGateway::default();
            CfgPacketDataGateway {
                enabled: dto.enabled.unwrap_or(defaults.enabled),
                interface_name: dto.interface_name.unwrap_or(defaults.interface_name),
                prefix_len: dto.prefix_len.unwrap_or(defaults.prefix_len),
                mtu: dto.mtu.or(defaults.mtu),
                auto_configure: dto.auto_configure.unwrap_or(defaults.auto_configure),
                enable_ipv4_forwarding: dto.enable_ipv4_forwarding.unwrap_or(defaults.enable_ipv4_forwarding),
                managed_forwarding: dto.managed_forwarding.unwrap_or(defaults.managed_forwarding),
                allow_unsolicited_inbound: dto
                    .allow_unsolicited_inbound
                    .unwrap_or(defaults.allow_unsolicited_inbound),
                nat_mode: dto.nat_mode.unwrap_or(defaults.nat_mode),
                firewall_backend: dto.firewall_backend.unwrap_or(defaults.firewall_backend),
                external_interface: dto.external_interface.or(defaults.external_interface),
                dns_servers: dto.dns_servers.unwrap_or(defaults.dns_servers),
                channel_capacity: dto.channel_capacity.unwrap_or(defaults.channel_capacity),
                max_pdch_bearers: dto.max_pdch_bearers.unwrap_or(defaults.max_pdch_bearers),
                reserved_voice_slots: dto.reserved_voice_slots.unwrap_or(defaults.reserved_voice_slots),
                prefer_secondary_carrier: dto
                    .prefer_secondary_carrier
                    .unwrap_or(defaults.prefer_secondary_carrier),
                downlink_queue_packets_per_context: dto
                    .downlink_queue_packets_per_context
                    .unwrap_or(defaults.downlink_queue_packets_per_context),
                downlink_queue_bytes_per_context: dto
                    .downlink_queue_bytes_per_context
                    .unwrap_or(defaults.downlink_queue_bytes_per_context),
                downlink_queue_ttl_secs: dto.downlink_queue_ttl_secs.unwrap_or(defaults.downlink_queue_ttl_secs),
                page_retry_secs: dto.page_retry_secs.unwrap_or(defaults.page_retry_secs),
                fragment_reassembly_timeout_secs: dto
                    .fragment_reassembly_timeout_secs
                    .unwrap_or(defaults.fragment_reassembly_timeout_secs),
                fragment_reassembly_max_datagrams: dto
                    .fragment_reassembly_max_datagrams
                    .unwrap_or(defaults.fragment_reassembly_max_datagrams),
                fragment_reassembly_max_bytes: dto
                    .fragment_reassembly_max_bytes
                    .unwrap_or(defaults.fragment_reassembly_max_bytes),
                automatic_filter_ttl_secs: dto
                    .automatic_filter_ttl_secs
                    .unwrap_or(defaults.automatic_filter_ttl_secs),
                automatic_filter_max_bindings: dto
                    .automatic_filter_max_bindings
                    .unwrap_or(defaults.automatic_filter_max_bindings),
            }
        },
        system_code: ci.system_code.unwrap_or(3), // 3 = ETSI EN 300 392-2 V3.1.1
        colour_code: ci.colour_code.unwrap_or(0),
        sharing_mode: ci.sharing_mode.unwrap_or(0),
        ts_reserved_frames: ci.ts_reserved_frames.unwrap_or(0),
        u_plane_dtx: ci.u_plane_dtx.unwrap_or(false),
        frame_18_ext: ci.frame_18_ext.unwrap_or(false),
        ms_txpwr_max_cell: ci.ms_txpwr_max_cell.unwrap_or(4), // 30 dBm (1W), Table 18.57
        local_ssi_ranges: ci
            .local_ssi_ranges
            .map(SortedDisjointSsiRanges::from_vec_tuple)
            .unwrap_or(default_tetrapack_local_ranges()),
        timezone: ci.timezone,
        home_mode_display: ci.home_mode_display.map(|h| CfgHomeModeDisplay {
            source_issi: h.source_issi.unwrap_or(0),
            interval_multiframes: h.interval_multiframes.unwrap_or(96),
            protocol_id: h.protocol_id.unwrap_or(220),
            text_coding_scheme: h.text_coding_scheme.unwrap_or(HomeModeDisplaySdsTextCodingScheme::LATIN),
            text: h.text.unwrap_or_default(),
        }),
        sds_broadcast: ci.sds_broadcast.map(|h| CfgHomeModeDisplay {
            source_issi: h.source_issi.unwrap_or(0),
            interval_multiframes: h.interval_multiframes.unwrap_or(96),
            protocol_id: h.protocol_id.unwrap_or(220),
            text_coding_scheme: h.text_coding_scheme.unwrap_or(HomeModeDisplaySdsTextCodingScheme::LATIN),
            text: h.text.unwrap_or_default(),
        }),
        neighbor_cells_ca: ci.neighbor_cells_ca,
        hangtime_secs: ci.hangtime_secs.unwrap_or(5).clamp(0, 300),
        call_timeout_secs: {
            let v = ci.call_timeout_secs.unwrap_or(120);
            if v == 0 { 0 } else { v.clamp(30, 86400) }
        },
        ul_inactivity_secs: ci.ul_inactivity_secs.unwrap_or(3).clamp(1, 30),
        periodic_registration_secs: {
            let v = ci.periodic_registration_secs.unwrap_or(3600);
            if v == 0 { 0 } else { v.clamp(60, 86400) }
        },
        sds_command_control: ci.sds_command_control.map(|dto| CfgSdsCommandControl {
            authorized_issis: dto.authorized_issis,
            commands: dto
                .commands
                .into_iter()
                .map(|e| CfgSdsCommandEntry {
                    status_code: e.status_code,
                    action: e.action,
                })
                .collect(),
        }),
        release_group_on_same_speaker_retake: ci.release_group_on_same_speaker_retake.unwrap_or(false),
    }
}

/// Default local SSI ranges are defined as 0-90 (inclusive), which fits the TetraPack configuration.
/// This helps prevent excessive flows of unroutable traffic to TetraPack, and can be overridden
/// by users if needed.
fn default_tetrapack_local_ranges() -> SortedDisjointSsiRanges {
    SortedDisjointSsiRanges::from_vec_ssirange(vec![SsiRange::new(0, 90)])
}

// ── SDS command control ────────────────────────────────────────────────────

/// A single SDS status code → action mapping for remote control via U-STATUS.
#[derive(Debug, Clone)]
pub struct CfgSdsCommandEntry {
    /// Pre-coded status value that triggers this action.
    pub status_code: u16,
    /// Action to execute: "restart", "shutdown", or "kick_all".
    pub action: String,
}

/// Remote control via SDS U-STATUS PDUs sent to ISSI 9999.
/// Only ISSIs listed in `authorized_issis` can trigger commands.
#[derive(Debug, Clone)]
pub struct CfgSdsCommandControl {
    pub authorized_issis: Vec<u32>,
    pub commands: Vec<CfgSdsCommandEntry>,
}

#[derive(Default, Deserialize)]
pub struct SdsCommandEntryDto {
    pub status_code: u16,
    pub action: String,
}

#[derive(Default, Deserialize)]
pub struct SdsCommandControlDto {
    #[serde(default)]
    pub authorized_issis: Vec<u32>,
    #[serde(default)]
    pub commands: Vec<SdsCommandEntryDto>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

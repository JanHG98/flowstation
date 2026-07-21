use serde::Deserialize;
use std::sync::{Arc, RwLock};
use tetra_core::freqs::FreqInfo;

use crate::bluestation::{
    CfgAsterisk, CfgCellInfo, CfgControl, CfgControlRoom, CfgDapnet, CfgEcholink, CfgEmergency, CfgGeoalarm, CfgHealth, CfgMeshcom, CfgNetInfo, CfgPhyIo,
    CfgAudioPlayer, CfgRecording, CfgRecovery, CfgTts, CfgSecurity, CfgSnomNotify, CfgTpg2200Action, CfgWxService, PhyBackend, StackState,
};

use super::sec_brew::CfgBrew;
use super::sec_dashboard::CfgDashboard;
use super::sec_telegram::CfgTelegram;
use super::sec_telemetry::CfgTelemetry;

/// Wrapper for a string that should be treated as a secret. Display and Debug will redact the actual value,
/// to prevent accidental logging of secrets.
#[derive(Clone)]
pub struct SecretField {
    pub val: String,
}

impl From<String> for SecretField {
    fn from(val: String) -> Self {
        Self { val }
    }
}

impl From<SecretField> for String {
    fn from(secret: SecretField) -> Self {
        secret.val
    }
}

impl AsRef<str> for SecretField {
    fn as_ref(&self) -> &str {
        &self.val
    }
}

impl std::fmt::Display for SecretField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "********")
    }
}

impl std::fmt::Debug for SecretField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecretField").field("val", &"********").finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum StackMode {
    Bs,
    Ms,
    Mon,
}

#[derive(Debug, Clone)]
pub struct StackConfig {
    pub stack_mode: StackMode,
    pub debug_log: Option<String>,

    /// Optional explicit systemd service unit name (e.g. "tetra", "tetra-flowstation",
    /// "bluestation"). Used by SDS command control (restart/shutdown) and dashboard
    /// OTA update. When unset, FlowStation auto-detects the unit from /proc/self/cgroup,
    /// then falls back to "tetra". Override via env var FLOWSTATION_SERVICE_UNIT also works.
    pub service_name: Option<String>,

    pub phy_io: CfgPhyIo,
    pub net: CfgNetInfo,
    pub cell: CfgCellInfo,

    /// Brew protocol (TetraPack/BrandMeister) configuration
    pub brew: Option<CfgBrew>,

    /// Optional secondary Brew protocol bridge.
    pub brew2: Option<CfgBrew>,

    /// Asterisk SIP/RTP bridge configuration.
    pub asterisk: CfgAsterisk,

    /// DAPNET inbound-message forwarding configuration.
    pub dapnet: CfgDapnet,

    /// EchoLink bridge configuration.
    pub echolink: CfgEcholink,

    /// MeshCom external UDP bridge configuration.
    pub meshcom: CfgMeshcom,

    /// Geo-fence alarm configuration for TETRA/MeshCom positions.
    pub geoalarm: CfgGeoalarm,

    /// Token-protected ActionURL trigger for Motorola TPG2200 Call-Out.
    pub tpg2200_action: CfgTpg2200Action,

    /// Snom XML minibrowser notifications via Asterisk AMI PJSIPNotify.
    pub snom_notify: CfgSnomNotify,

    /// Dashboard HTTP server configuration (None = disabled)
    pub dashboard: Option<CfgDashboard>,

    /// Local TETRA speech recording configuration.
    pub recording: CfgRecording,

    /// Local WAV/MP3 TETRA audio dispatch configuration.
    pub audio_player: CfgAudioPlayer,

    /// Local Piper HTTP text-to-speech configuration.
    pub tts: CfgTts,

    /// Telemetry endpoint configuration
    pub telemetry: Option<CfgTelemetry>,

    /// Control endpoint configuration
    pub control: Option<CfgControl>,

    /// NetCore Control-Room endpoint configuration
    pub control_room: Option<CfgControlRoom>,

    /// Access control / security configuration
    pub security: CfgSecurity,

    /// Built-in WX/METAR SDS service configuration
    pub wx_service: CfgWxService,

    /// Restart-recovery configuration (proactive cold-start re-registration). Default disabled.
    pub recovery: CfgRecovery,

    /// Telegram alerts configuration (None = no `[telegram_alerts]` section in the config file).
    pub telegram: Option<CfgTelegram>,

    /// Lite stack-health monitor configuration. Always present (defaults: monitor ON, watchdog
    /// restart OFF).
    pub health: CfgHealth,

    /// Emergency-state handling. Always present (defaults: LOCAL-only — no Brew forward,
    /// telegram_alert ON, clear_timeout_secs 30). See [`CfgEmergency`].
    pub emergency: CfgEmergency,
}

impl StackConfig {
    /// Return BS phase-modulated carrier numbers and their DL/UL frequencies.
    pub fn bs_phase_mod_carriers(&self) -> Result<Vec<(u16, u32, u32)>, String> {
        let mut carriers = Vec::with_capacity(if self.cell.secondary_carrier.is_some() { 2 } else { 1 });
        for carrier in [Some(self.cell.main_carrier), self.cell.secondary_carrier].into_iter().flatten() {
            let freq_info = FreqInfo::from_components(
                self.cell.freq_band,
                carrier,
                self.cell.freq_offset_hz,
                self.cell.reverse_operation,
                self.cell.duplex_spacing_id,
                self.cell.custom_duplex_spacing,
            )?;
            let (dl_freq, ul_freq) = freq_info.get_freqs();
            carriers.push((carrier, dl_freq, ul_freq));
        }
        Ok(carriers)
    }

    fn frequencies_fit_center(center_hz: f64, sample_rate_hz: f64, freqs_hz: &[u32]) -> bool {
        let half_bw = sample_rate_hz / 2.0;
        freqs_hz.iter().all(|freq| ((*freq as f64) - center_hz).abs() <= half_bw)
    }

    /// Validate that all required configuration fields are properly set.
    pub fn validate(&self) -> Result<(), &str> {
        // Check input device settings
        match self.phy_io.backend {
            PhyBackend::SoapySdr => {
                if self.phy_io.soapysdr.is_none() {
                    return Err("soapysdr configuration must be provided for Soapysdr backend");
                };
            }
            PhyBackend::None => {} // For testing
            PhyBackend::Undefined => {
                return Err("phy_io backend must be defined");
            }
        };

        if let Some(secondary_carrier) = self.cell.secondary_carrier
            && secondary_carrier == self.cell.main_carrier
        {
            return Err("cell.secondary_carrier must differ from cell.main_carrier");
        }

        // Sanity check on main carrier property fields in SYSINFO
        if self.phy_io.backend == PhyBackend::SoapySdr {
            let soapy_cfg = self
                .phy_io
                .soapysdr
                .as_ref()
                .expect("SoapySdr config must be set for SoapySdr PhyIo");

            let carriers = self.bs_phase_mod_carriers().map_err(|_| "Invalid cell info frequency settings")?;
            let (main_dl, main_ul) = carriers
                .iter()
                .find(|(carrier_num, _, _)| *carrier_num == self.cell.main_carrier)
                .map(|(_, dl, ul)| (*dl, *ul))
                .ok_or("main carrier missing from computed carrier list")?;

            println!("    Derived BS carriers: {:?}\n", carriers);

            if soapy_cfg.dl_freq as u32 != main_dl {
                return Err("PhyIo DlFrequency does not match computed FreqInfo");
            };
            if soapy_cfg.ul_freq as u32 != main_ul {
                return Err("PhyIo UlFrequency does not match computed FreqInfo");
            };

            if carriers.len() > 1 {
                let Some(sample_rate_hz) = soapy_cfg.fs else {
                    return Err(
                        "dual carrier requires phy_io.soapysdr.sample_rate to be set so the secondary carrier can be proven to fit the SDR passband",
                    );
                };
                let dl_freqs: Vec<u32> = carriers.iter().map(|(_, dl, _)| *dl).collect();
                let ul_freqs: Vec<u32> = carriers.iter().map(|(_, _, ul)| *ul).collect();

                let tx_center_hz = soapy_cfg.tx_center_freq.unwrap_or(soapy_cfg.dl_freq);
                let rx_center_hz = soapy_cfg.rx_center_freq.unwrap_or(soapy_cfg.ul_freq);

                if !Self::frequencies_fit_center(tx_center_hz, sample_rate_hz, &dl_freqs) {
                    return Err("configured TX center/sample-rate do not cover all BS downlink carriers");
                }
                if !Self::frequencies_fit_center(rx_center_hz, sample_rate_hz, &ul_freqs) {
                    return Err("configured RX center/sample-rate do not cover all BS uplink carriers");
                }
            };
        }

        if self.cell.ms_txpwr_max_cell > 7 {
            return Err("ms_txpwr_max_cell must be 0-7 (3 bits)");
        }

        // SNDCP can expose the local WAP endpoint, the general Linux IP gateway,
        // or both. Capability advertisement and runtime enablement must agree.
        let packet_profile_enabled = self.cell.wap_ip.enabled || self.cell.packet_data_gateway.enabled;
        if self.cell.sndcp_service && !packet_profile_enabled {
            return Err("cell.sndcp_service=true requires WAP/IP or packet_data_gateway");
        }
        if packet_profile_enabled && !self.cell.sndcp_service {
            return Err("packet-data services require cell.sndcp_service=true");
        }
        if packet_profile_enabled && !self.cell.advanced_link {
            return Err("packet-data services require cell.advanced_link=true");
        }
        if self.cell.wap_ip.enabled && self.cell.wap_ip.port == 0 {
            return Err("cell_info.wap_ip.port must be non-zero");
        }
        if self.cell.wap_ip.enabled && self.cell.wap_ip.response_ttl == 0 {
            return Err("cell_info.wap_ip.response_ttl must be non-zero");
        }
        if self.cell.wap_ip.max_request_payload_bytes == 0 || self.cell.wap_ip.max_request_payload_bytes > 1024 {
            return Err("cell_info.wap_ip.max_request_payload_bytes must be 1-1024");
        }
        if self.cell.wap_ip.pdu_priority_max > 7 {
            return Err("cell_info.wap_ip.pdu_priority_max must be 0-7");
        }
        if !(1..=14).contains(&self.cell.wap_ip.ready_timer_code) {
            return Err("cell_info.wap_ip.ready_timer_code must be 1-14");
        }
        if !(1..=15).contains(&self.cell.wap_ip.standby_timer_code) {
            return Err("cell_info.wap_ip.standby_timer_code must be 1-15");
        }
        if self.cell.wap_ip.response_wait_timer_code > 14 {
            return Err("cell_info.wap_ip.response_wait_timer_code must be 0-14");
        }
        if !(1..=5).contains(&self.cell.wap_ip.mtu_code) {
            return Err("cell_info.wap_ip.mtu_code must be 1-5");
        }
        if self.cell.wap_ip.network_default_data_priority > 7 {
            return Err("cell_info.wap_ip.network_default_data_priority must be 0-7");
        }
        if self.cell.wap_ip.max_contexts_per_issi == 0 || self.cell.wap_ip.max_contexts_per_issi > 14 {
            return Err("cell_info.wap_ip.max_contexts_per_issi must be 1-14");
        }
        if self.cell.wap_ip.max_total_contexts == 0
            || self.cell.wap_ip.max_total_contexts < self.cell.wap_ip.max_contexts_per_issi
        {
            return Err("cell_info.wap_ip.max_total_contexts must be non-zero and >= max_contexts_per_issi");
        }
        if self.cell.wap_ip.dynamic_pool_first_host == 0
            || self.cell.wap_ip.dynamic_pool_last_host == 255
            || self.cell.wap_ip.dynamic_pool_first_host > self.cell.wap_ip.dynamic_pool_last_host
        {
            return Err("cell_info.wap_ip dynamic pool hosts must be ordered inside 1-254");
        }
        let Some(pool_prefix) = self.cell.wap_ip.pool_prefix_octets() else {
            return Err("cell_info.wap_ip.dynamic_pool_prefix must contain exactly three IPv4 octets");
        };
        let endpoint = self.cell.wap_ip.address.octets();
        if endpoint[..3] == pool_prefix[..]
            && (self.cell.wap_ip.dynamic_pool_first_host..=self.cell.wap_ip.dynamic_pool_last_host).contains(&endpoint[3])
        {
            return Err("cell_info.wap_ip.address must not be inside the dynamic client pool");
        }

        let gateway = &self.cell.packet_data_gateway;
        let valid_interface_name = |value: &str| {
            !value.is_empty()
                && value.len() <= 15
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
        };
        if !valid_interface_name(&gateway.interface_name) {
            return Err("cell_info.packet_data_gateway.interface_name must be 1-15 safe interface characters");
        }
        if let Some(external) = &gateway.external_interface {
            if !valid_interface_name(external) {
                return Err("cell_info.packet_data_gateway.external_interface is invalid");
            }
            if external == &gateway.interface_name {
                return Err("packet-data TUN and external interfaces must differ");
            }
        }
        if !(1..=30).contains(&gateway.prefix_len) {
            return Err("cell_info.packet_data_gateway.prefix_len must be 1-30");
        }
        if gateway.mtu.is_some_and(|mtu| mtu < 68) {
            return Err("cell_info.packet_data_gateway.mtu must be at least 68");
        }
        if gateway.channel_capacity < 16 || gateway.channel_capacity > 65_536 {
            return Err("cell_info.packet_data_gateway.channel_capacity must be 16-65536");
        }
        if gateway.max_pdch_bearers > 6 {
            return Err("cell_info.packet_data_gateway.max_pdch_bearers must be 0-6");
        }
        if gateway.reserved_voice_slots > 5 {
            return Err("cell_info.packet_data_gateway.reserved_voice_slots must be 0-5");
        }
        let traffic_slots = if self.cell.secondary_carrier.is_some() { 6 } else { 3 };
        if gateway.enabled && gateway.reserved_voice_slots >= traffic_slots {
            return Err("cell_info.packet_data_gateway.reserved_voice_slots must leave at least one PDCH slot");
        }
        if gateway.downlink_queue_packets_per_context == 0
            || gateway.downlink_queue_bytes_per_context < 576
            || gateway.downlink_queue_ttl_secs == 0
            || gateway.page_retry_secs == 0
        {
            return Err("packet-data downlink queue and paging limits must be non-zero");
        }
        if gateway.fragment_reassembly_timeout_secs == 0
            || gateway.fragment_reassembly_max_datagrams == 0
            || gateway.fragment_reassembly_max_bytes < 65_535
        {
            return Err("packet-data fragment reassembly limits are invalid");
        }
        if gateway.automatic_filter_ttl_secs == 0 || gateway.automatic_filter_max_bindings == 0 {
            return Err("packet-data automatic filter limits must be non-zero");
        }
        if gateway.dns_servers.len() > 2 {
            return Err("cell_info.packet_data_gateway.dns_servers supports at most two IPv4 addresses");
        }
        for address in &gateway.dns_servers {
            if address.is_unspecified() || address.is_multicast() || *address == std::net::Ipv4Addr::BROADCAST {
                return Err("packet-data DNS addresses must be unicast IPv4 addresses");
            }
        }
        if (gateway.managed_forwarding
            || gateway.nat_mode == crate::bluestation::PacketGatewayNatMode::Masquerade)
            && !gateway.enable_ipv4_forwarding
        {
            return Err("managed forwarding/NAT requires enable_ipv4_forwarding=true");
        }
        if gateway.managed_forwarding
            && gateway.firewall_backend == crate::bluestation::PacketGatewayFirewallBackend::None
        {
            return Err("managed_forwarding=true requires nftables, iptables or auto firewall backend");
        }
        if gateway.allow_unsolicited_inbound && !gateway.managed_forwarding {
            return Err("allow_unsolicited_inbound=true requires managed_forwarding=true");
        }
        if gateway.nat_mode == crate::bluestation::PacketGatewayNatMode::Masquerade
            && gateway.firewall_backend == crate::bluestation::PacketGatewayFirewallBackend::None
        {
            return Err("NAT masquerade requires nftables, iptables or auto firewall backend");
        }
        if !gateway.auto_configure
            && (gateway.managed_forwarding
                || gateway.nat_mode == crate::bluestation::PacketGatewayNatMode::Masquerade)
        {
            return Err("managed forwarding/NAT requires packet_data_gateway.auto_configure=true");
        }
        if gateway.enabled {
            let gateway_u32 = u32::from(self.cell.wap_ip.address);
            let mask = u32::MAX << (32 - gateway.prefix_len);
            let network = gateway_u32 & mask;
            let broadcast = network | !mask;
            if gateway_u32 == network || gateway_u32 == broadcast {
                return Err("WAP/gateway address must not be the packet-data subnet network or broadcast address");
            }
            let first = u32::from(std::net::Ipv4Addr::new(
                pool_prefix[0], pool_prefix[1], pool_prefix[2], self.cell.wap_ip.dynamic_pool_first_host,
            ));
            let last = u32::from(std::net::Ipv4Addr::new(
                pool_prefix[0], pool_prefix[1], pool_prefix[2], self.cell.wap_ip.dynamic_pool_last_host,
            ));
            if first & mask != network || last & mask != network || first == network || last == broadcast {
                return Err("dynamic PDP address pool must fit completely inside packet-data subnet");
            }
        }

        // Validate timezone if configured
        if let Some(ref tz) = self.cell.timezone
            && tz.parse::<chrono_tz::Tz>().is_err()
        {
            return Err("Invalid IANA timezone name in cell.timezone");
        }

        // Validate neighbor cells
        if self.cell.neighbor_cells_ca.len() > 7 {
            return Err("cell.neighbor_cells_ca supports at most 7 entries");
        }

        // Check for duplicate cell_identifier_ca and main_carrier_number
        {
            let mut seen_ids = std::collections::HashSet::new();
            let mut seen_carriers = std::collections::HashSet::new();
            for cell in &self.cell.neighbor_cells_ca {
                if !seen_ids.insert(cell.cell_identifier_ca) {
                    return Err("cell.neighbor_cells_ca: duplicate cell_identifier_ca — each neighbour must have a unique identifier");
                }
                if !seen_carriers.insert(cell.main_carrier_number) {
                    return Err("cell.neighbor_cells_ca: duplicate main_carrier_number — each neighbour must be on a different carrier");
                }
            }
        }

        for cell in &self.cell.neighbor_cells_ca {
            if cell.bs_service_details.as_ref().is_some_and(|details| details.sndcp_service) {
                return Err("cell.neighbor_cells_ca: neighbor SNDCP advertisement is unsupported by the local WAP profile");
            }
            if cell.cell_identifier_ca > 0x1F {
                return Err("cell.neighbor_cells_ca: cell_identifier_ca must be 0-31");
            }
            if cell.cell_reselection_types_supported > 0x3 {
                return Err("cell.neighbor_cells_ca: cell_reselection_types_supported must be 0-3");
            }
            if cell.cell_load_ca > 0x3 {
                return Err("cell.neighbor_cells_ca: cell_load_ca must be 0-3");
            }
            if cell.main_carrier_number > 0xFFF {
                return Err("cell.neighbor_cells_ca: main_carrier_number must be 0-4095");
            }
            if let Some(v) = cell.main_carrier_number_extension
                && v > 0x3FF
            {
                return Err("cell.neighbor_cells_ca: main_carrier_number_extension must be 0-1023");
            }
            if let Some(v) = cell.mcc
                && v > 0x3FF
            {
                return Err("cell.neighbor_cells_ca: mcc must be 0-1023");
            }
            if let Some(v) = cell.mnc
                && v > 0x3FFF
            {
                return Err("cell.neighbor_cells_ca: mnc must be 0-16383");
            }
            if let Some(v) = cell.location_area
                && v > 0x3FFF
            {
                return Err("cell.neighbor_cells_ca: location_area must be 0-16383");
            }
            if let Some(v) = cell.maximum_ms_transmit_power
                && v > 0x7
            {
                return Err("cell.neighbor_cells_ca: maximum_ms_transmit_power must be 0-7");
            }
            if let Some(v) = cell.minimum_rx_access_level
                && v > 0xF
            {
                return Err("cell.neighbor_cells_ca: minimum_rx_access_level must be 0-15");
            }
            if let Some(v) = cell.timeshare_cell_information_or_security_parameters
                && v > 0x1F
            {
                return Err("cell.neighbor_cells_ca: timeshare_cell_information_or_security_parameters must be 0-31");
            }
            if let Some(v) = cell.tdma_frame_offset
                && v > 0x3F
            {
                return Err("cell.neighbor_cells_ca: tdma_frame_offset must be 0-63");
            }
        }

        // Restart recovery: an explicit allowlist must not exceed the cache cap (the numeric
        // ranges are already clamped in apply_recovery_patch). Only meaningful when enabled.
        if self.recovery.enabled
            && !self.recovery.issi_allowlist.is_empty()
            && self.recovery.issi_allowlist.len() as u32 > self.recovery.max_cached_issis
        {
            return Err("recovery.issi_allowlist has more entries than recovery.max_cached_issis");
        }

        if self.brew.is_some() && self.brew2.is_some() {
            let brew = self.brew.as_ref().expect("checked");
            let brew2 = self.brew2.as_ref().expect("checked");
            if !brew.has_local_issi_allowlist() || !brew2.has_local_issi_allowlist() {
                return Err("brew and brew2 require non-empty local_issi_allowlist when both are configured");
            }

            let Some(brew_allowlist) = brew.effective_local_issi_allowlist() else {
                return Err("brew local_issi_allowlist is required when brew2 is configured");
            };
            let Some(brew2_allowlist) = brew2.effective_local_issi_allowlist() else {
                return Err("brew2 local_issi_allowlist is required when brew is configured");
            };
            if brew_allowlist.is_empty() || brew2_allowlist.is_empty() {
                return Err("brew and brew2 effective local_issi_allowlist must not be empty");
            }
            let brew_set: std::collections::HashSet<u32> = brew_allowlist.into_iter().collect();
            if brew2_allowlist.into_iter().any(|issi| brew_set.contains(&issi)) {
                return Err("brew and brew2 local_issi_allowlist must not overlap");
            }
        }

        Ok(())
    }
}

/// Global shared configuration: immutable config + mutable state.
#[derive(Clone)]
pub struct SharedConfig {
    /// Read-only configuration (immutable after construction).
    cfg: Arc<StackConfig>,
    /// Mutable state guarded with RwLock (write by the stack, read by others).
    state: Arc<RwLock<StackState>>,
}

impl SharedConfig {
    pub fn from_parts(cfg: StackConfig, state: Option<StackState>) -> Self {
        // Check config for validity before returning the SharedConfig object
        match cfg.validate() {
            Ok(_) => {}
            Err(e) => panic!("Invalid stack configuration: {}", e),
        }

        Self {
            cfg: Arc::new(cfg),
            state: Arc::new(RwLock::new(state.unwrap_or_default())),
        }
    }

    /// Access immutable config.
    pub fn config(&self) -> Arc<StackConfig> {
        Arc::clone(&self.cfg)
    }

    /// Read guard for mutable state.
    pub fn state_read(&self) -> std::sync::RwLockReadGuard<'_, StackState> {
        self.state.read().expect("StackState RwLock blocked")
    }

    /// Write guard for mutable state.
    pub fn state_write(&self) -> std::sync::RwLockWriteGuard<'_, StackState> {
        self.state.write().expect("StackState RwLock blocked")
    }

    /// Effective WX/METAR service settings: the dashboard runtime override if present,
    /// otherwise the config file values. Returns an owned CfgWxService so callers don't
    /// hold the state lock.
    pub fn effective_wx_service(&self) -> crate::bluestation::CfgWxService {
        let base = self.cfg.wx_service.clone();
        if let Some(o) = self.state_read().wx_override.as_ref() {
            crate::bluestation::CfgWxService {
                enabled: o.enabled,
                service_issi: o.service_issi,
                periodic_enabled: o.periodic_enabled,
                periodic_issi: o.periodic_issi,
                periodic_is_group: o.periodic_is_group,
                periodic_icao: o.periodic_icao.clone(),
                periodic_interval_secs: o.periodic_interval_secs,
            }
        } else {
            base
        }
    }

    /// Effective Telegram alerts settings: the dashboard runtime override if present, otherwise
    /// the config file values (or defaults when there is no `[telegram_alerts]` section). Returns
    /// an owned [`CfgTelegram`] so callers don't hold the state lock. The alerter and the
    /// dashboard both read through this so a live edit applies without a restart.
    pub fn effective_telegram(&self) -> crate::bluestation::CfgTelegram {
        let base = self.cfg.telegram.clone().unwrap_or_default();
        if let Some(o) = self.state_read().telegram_override.as_ref() {
            crate::bluestation::CfgTelegram {
                enabled: o.enabled,
                bot_token: crate::bluestation::SecretField::from(o.bot_token.clone()),
                chat_ids: o.chat_ids.clone(),
                alert_connect: o.alert_connect,
                alert_disconnect: o.alert_disconnect,
                alert_t351: o.alert_t351,
                alert_lip: o.alert_lip,
                alert_backhaul: o.alert_backhaul,
                alert_critical_logs: o.alert_critical_logs,
                // Health alerts aren't part of the dashboard live-edit override yet — take the
                // base config value so the field is always populated.
                alert_health: base.alert_health,
                alert_brew_register: o.alert_brew_register,
                brew_register_prefix: o.brew_register_prefix.clone(),
                brew_register_issi_whitelist: o.brew_register_issi_whitelist.clone(),
                brew_register_issi_blacklist: o.brew_register_issi_blacklist.clone(),
            }
        } else {
            base
        }
    }

    /// Effective DAPNET settings: the dashboard runtime override if present, otherwise the config
    /// file values. Returns an owned [`CfgDapnet`] so callers don't hold the state lock.
    pub fn effective_dapnet(&self) -> crate::bluestation::CfgDapnet {
        let base = self.cfg.dapnet.clone();
        if let Some(o) = self.state_read().dapnet_override.as_ref() {
            crate::bluestation::CfgDapnet {
                enabled: o.enabled,
                api_url: o.api_url.clone(),
                username: o.username.clone(),
                password: crate::bluestation::SecretField::from(o.password.clone()),
                poll_interval_secs: o.poll_interval_secs.max(1),
                forward_sds: o.forward_sds,
                forward_callout: o.forward_callout,
                forward_telegram: o.forward_telegram,
                sds_source_issi: o.sds_source_issi,
                sds_dest_issi: o.sds_dest_issi,
                sds_dest_is_group: o.sds_dest_is_group,
                ric_issi_routes: o.ric_issi_routes.clone(),
                ric_gssi_routes: o.ric_gssi_routes.clone(),
                sds_allowed_rics: o.sds_allowed_rics.clone(),
                callout_allowed_rics: o.callout_allowed_rics.clone(),
                telegram_allowed_rics: o.telegram_allowed_rics.clone(),
                callout_source_issi: o.callout_source_issi,
                callout_dest_issi: o.callout_dest_issi,
                callout_tpg_ric: o.callout_tpg_ric,
                callout_incident_base: o.callout_incident_base.min(255),
                callout_priority: o.callout_priority.min(15),
                callout_issi_priorities: o.callout_issi_priorities.clone(),
                callout_tpg_ric_priorities: o.callout_tpg_ric_priorities.clone(),
                callout_text_prefix: o.callout_text_prefix.clone(),
                telegram_prefix: o.telegram_prefix.clone(),
                rwth_core_enabled: o.rwth_core_enabled,
                rwth_core_host: o.rwth_core_host.clone(),
                rwth_core_port: o.rwth_core_port,
                rwth_core_device: o.rwth_core_device.clone(),
                rwth_core_version: o.rwth_core_version.clone(),
                rwth_core_callsign: o.rwth_core_callsign.clone(),
                rwth_core_authkey: crate::bluestation::SecretField::from(o.rwth_core_authkey.clone()),
                rwth_messages_limit: o.rwth_messages_limit.max(1),
            }
        } else {
            base
        }
    }

    /// Effective EchoLink settings: the dashboard runtime override if present, otherwise the
    /// config file values. Returns an owned [`CfgEcholink`] so callers don't hold the state lock.
    pub fn effective_echolink(&self) -> crate::bluestation::CfgEcholink {
        let base = self.cfg.echolink.clone();
        if let Some(o) = self.state_read().echolink_override.as_ref() {
            crate::bluestation::CfgEcholink {
                enabled: o.enabled,
                callsign: o.callsign.clone(),
                password: crate::bluestation::SecretField::from(o.password.clone()),
                location: o.location.clone(),
                status_text: o.status_text.clone(),
                directory_servers: o.directory_servers.clone(),
                directory_port: o.directory_port,
                bind_addr: o.bind_addr.clone(),
                audio_port: o.audio_port,
                control_port: o.control_port,
                inbound_enabled: o.inbound_enabled,
                outbound_enabled: o.outbound_enabled,
                outbound_prefix: o.outbound_prefix.clone(),
                strip_outbound_prefix: o.strip_outbound_prefix,
                service_numbers: o.service_numbers.clone(),
                default_tetra_source_issi: o.default_tetra_source_issi,
                default_tetra_dest_issi: o.default_tetra_dest_issi,
                default_tetra_dest_is_group: o.default_tetra_dest_is_group,
                routes: o.routes.clone(),
                allowed_callsigns: o.allowed_callsigns.clone(),
                allowed_node_ids: o.allowed_node_ids.clone(),
                auto_connect: o.auto_connect.clone(),
                reconnect_interval_secs: o.reconnect_interval_secs.max(1),
                max_session_secs: o.max_session_secs.max(1),
            }
        } else {
            base
        }
    }

    /// Effective MeshCom settings: the dashboard runtime override if present, otherwise the
    /// config file values. Returns an owned [`CfgMeshcom`] so callers don't hold the state lock.
    pub fn effective_meshcom(&self) -> crate::bluestation::CfgMeshcom {
        let base = self.cfg.meshcom.clone();
        if let Some(o) = self.state_read().meshcom_override.as_ref() {
            crate::bluestation::CfgMeshcom {
                enabled: o.enabled,
                bind_addr: o.bind_addr.clone(),
                bind_port: if o.bind_port == 0 { base.bind_port } else { o.bind_port },
                tx_host: o.tx_host.clone(),
                tx_port: if o.tx_port == 0 { base.tx_port } else { o.tx_port },
                allow_broadcast: o.allow_broadcast,
                max_messages: o.max_messages.clamp(10, 10_000),
                max_nodes: o.max_nodes.clamp(10, 65_535),
                forward_sds: o.forward_sds,
                forward_sip: o.forward_sip,
                forward_telegram: o.forward_telegram,
                sds_source_issi: o.sds_source_issi.max(1),
                sds_dest_issi: o.sds_dest_issi,
                sds_dest_is_group: o.sds_dest_is_group,
                sds_allowed_sources: o.sds_allowed_sources.clone(),
                sip_title_prefix: o.sip_title_prefix.clone(),
                sip_allowed_sources: o.sip_allowed_sources.clone(),
                telegram_prefix: o.telegram_prefix.clone(),
                telegram_allowed_sources: o.telegram_allowed_sources.clone(),
            }
        } else {
            base
        }
    }

    /// Effective GeoAlarm settings: the dashboard runtime override if present, otherwise the
    /// config file values. Returns an owned [`CfgGeoalarm`] so callers don't hold the state lock.
    pub fn effective_geoalarm(&self) -> crate::bluestation::CfgGeoalarm {
        let base = self.cfg.geoalarm.clone();
        if let Some(o) = self.state_read().geoalarm_override.as_ref() {
            crate::bluestation::CfgGeoalarm {
                enabled: o.enabled,
                flowstation_lat: o.flowstation_lat,
                flowstation_lon: o.flowstation_lon,
                radius_m: if o.radius_m.is_finite() && o.radius_m > 0.0 {
                    o.radius_m
                } else {
                    base.radius_m
                },
                cooldown_secs: o.cooldown_secs.clamp(1, 86_400),
                trigger_tetra: o.trigger_tetra,
                trigger_meshcom: o.trigger_meshcom,
                forward_tpg2200: o.forward_tpg2200,
                forward_sds: o.forward_sds,
                forward_sip: o.forward_sip,
                forward_telegram: o.forward_telegram,
                tetra_issi_whitelist: o.tetra_issi_whitelist.clone(),
                tetra_issi_blacklist: o.tetra_issi_blacklist.clone(),
                meshcom_source_whitelist: o.meshcom_source_whitelist.clone(),
                meshcom_source_blacklist: o.meshcom_source_blacklist.clone(),
                telegram_tetra_issi_whitelist: o.telegram_tetra_issi_whitelist.clone(),
                telegram_tetra_issi_blacklist: o.telegram_tetra_issi_blacklist.clone(),
                telegram_meshcom_source_whitelist: o.telegram_meshcom_source_whitelist.clone(),
                telegram_meshcom_source_blacklist: o.telegram_meshcom_source_blacklist.clone(),
                sds_source_issi: o.sds_source_issi.max(1),
                sds_dest_issi: o.sds_dest_issi,
                sds_dest_is_group: o.sds_dest_is_group,
                tpg2200_source_issi: o.tpg2200_source_issi.max(1),
                tpg2200_dest_issi: o.tpg2200_dest_issi,
                tpg2200_ric: o.tpg2200_ric,
                tpg2200_incident_base: o.tpg2200_incident_base.min(255),
                tpg2200_priority: o.tpg2200_priority.min(15),
                tpg2200_issi_priorities: o.tpg2200_issi_priorities.clone(),
                tpg2200_ric_priorities: o.tpg2200_ric_priorities.clone(),
                tpg2200_text_prefix: o.tpg2200_text_prefix.clone(),
                tpg2200_max_text_chars: o.tpg2200_max_text_chars.clamp(8, 160),
                sip_title_prefix: o.sip_title_prefix.clone(),
                telegram_prefix: o.telegram_prefix.clone(),
            }
        } else {
            base
        }
    }

    /// Effective Snom XML NOTIFY settings: the dashboard runtime override if present, otherwise
    /// the config file values. Returns an owned [`CfgSnomNotify`] so callers don't hold the
    /// state lock while sending AMI requests.
    pub fn effective_snom_notify(&self) -> crate::bluestation::CfgSnomNotify {
        let base = self.cfg.snom_notify.clone();
        if let Some(o) = self.state_read().snom_notify_override.as_ref() {
            crate::bluestation::CfgSnomNotify {
                enabled: o.enabled,
                ami_host: o.ami_host.clone(),
                ami_port: o.ami_port,
                ami_username: o.ami_username.clone(),
                ami_password: crate::bluestation::SecretField::from(o.ami_password.clone()),
                endpoints: o.endpoints.clone(),
                notify_sds: o.notify_sds,
                notify_dapnet: o.notify_dapnet,
                notify_telegram: o.notify_telegram,
                sds_directions: o.sds_directions.clone(),
                dapnet_allowed_rics: o.dapnet_allowed_rics.clone(),
                sds_allowed_issis: o.sds_allowed_issis.clone(),
                title_prefix: o.title_prefix.clone(),
                notify_event: o.notify_event.clone(),
                content_type: o.content_type.clone(),
                subscription_state: o.subscription_state.clone(),
                max_text_chars: o.max_text_chars.clamp(40, 2000),
                connect_timeout_secs: o.connect_timeout_secs.clamp(1, 30),
            }
        } else {
            base
        }
    }
}

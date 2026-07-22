//! Shared, strongly typed service-access-point values used by TLMC and LTPD.
//!
//! The types in this module model local primitives from ETSI EN 300 392-2.
//! They are deliberately independent from any future network transport.  The
//! TLMC and LTPD SAPs remain in-process boundaries inside the TBS.

use core::fmt;
use std::time::Duration;

use tetra_core::{BitBuffer, EndpointId, LinkId, SsiType, TetraAddress};

/// Local identifier that correlates a request with a later report or cancel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct RequestHandle(pub u32);

/// Local identifier for a MAC channel-change decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ChannelChangeHandle(pub u32);

/// Decision returned to MAC for a channel allocation that requested a response.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChannelChangeDecision {
    Accept,
    Reject,
    #[default]
    Ignore,
}

/// Current availability of a lower-layer resource identified by an endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LowerLayerResourceAvailability {
    Available,
    Unavailable,
}

/// Reason carried by an MLE-CONFIGURE indication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LowerLayerResourceReason {
    ReceptionStopped,
    TransmissionStopped,
    UsageMarkerMismatch,
    LossOfRadioResources,
    RecoveryOfRadioResources,
    Other(u8),
}

/// Type of TETRA cell used by local mobility management.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CellType {
    #[default]
    ConventionalAccess,
    DirectAccess,
}

/// Service level currently offered by a cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CellServiceLevel {
    NoService,
    GracefulServiceDegradation,
    #[default]
    NormalService,
}

/// Stable identity and radio reference for a cell candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellIdentity {
    pub mcc: u16,
    pub mnc: u16,
    pub location_area: Option<u16>,
    pub colour_code: Option<u8>,
    pub main_carrier: u16,
    pub cell_type: CellType,
}

/// Unit used by a local measurement value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeasurementUnit {
    Db,
    Dbm,
    Raw,
}

/// Measurement with an explicit unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeasurementValue {
    pub value: i16,
    pub unit: MeasurementUnit,
}

impl MeasurementValue {
    pub const fn db(value: i16) -> Self {
        Self {
            value,
            unit: MeasurementUnit::Db,
        }
    }

    pub const fn dbm(value: i16) -> Self {
        Self {
            value,
            unit: MeasurementUnit::Dbm,
        }
    }

    pub const fn raw(value: i16) -> Self {
        Self {
            value,
            unit: MeasurementUnit::Raw,
        }
    }
}

/// Consolidated local measurement result used by mobility selection logic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasurementReport {
    pub endpoint_id: Option<EndpointId>,
    pub channel_number: Option<RfChannelNumber>,
    pub path_loss_c1: Option<MeasurementValue>,
    pub path_loss_c2: Option<MeasurementValue>,
    pub path_loss_c3: Option<MeasurementValue>,
    pub path_loss_c4: Option<MeasurementValue>,
    pub path_loss_c5: Option<MeasurementValue>,
    pub quality: Option<QualityIndication>,
}

/// Candidate returned by monitoring or scanning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellCandidate {
    pub identity: Option<CellIdentity>,
    pub channel_number: RfChannelNumber,
    pub service_level: CellServiceLevel,
    pub measurements: MeasurementReport,
}

/// Correlation identifier for a local scan operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ScanRequestId(pub u32);

/// Reason why MLE asks MAC to select a channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionCause {
    InitialCellSelection,
    AnnouncedReselectionType1,
    AnnouncedReselectionType2,
    AnnouncedReselectionType3,
    UnannouncedReselection,
    UndeclaredReselection,
    BaseStationControlledChannelChange,
    CallRestoration,
    Other(u8),
}

/// Outcome returned by TL-SELECT or related local selection procedures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionResult {
    Success,
    RandomAccessFailure,
    ReconnectionFailure,
    Reject,
    Other(u8),
}

/// Instruction to release a locally configured circuit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CallReleaseInstruction {
    #[default]
    Keep,
    Release,
}

/// Whether the U-plane is active for a circuit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UPlaneSwitch {
    #[default]
    Off,
    On,
}

/// Current transmit grant associated with an operating-mode instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TxGrantState {
    #[default]
    NotGranted,
    Granted,
}

/// Directionality of a circuit-mode resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DuplexMode {
    #[default]
    Simplex,
    Duplex,
}

/// Circuit type selected for the local U-plane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitMode {
    Speech,
    UnprotectedData72,
    LowProtectionData48,
    HighProtectionData24,
    Other(u8),
}

/// Complete local operating-mode instruction passed down to MAC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperatingMode {
    pub u_plane: UPlaneSwitch,
    pub tx_grant: TxGrantState,
    pub duplex: DuplexMode,
    pub circuit_mode: CircuitMode,
    pub interleaving_depth: Option<u8>,
    pub end_to_end_encrypted: bool,
    pub user_device: Option<u8>,
    pub endpoint_id: EndpointId,
}

/// Packet-data priority, including the ETSI "undefined" value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DataPriority {
    Priority0,
    Priority1,
    Priority2,
    Priority3,
    Priority4,
    Priority5,
    Priority6,
    Priority7,
    #[default]
    Undefined,
}

impl DataPriority {
    pub fn from_raw(value: u8) -> Option<Self> {
        Some(match value {
            0 => Self::Priority0,
            1 => Self::Priority1,
            2 => Self::Priority2,
            3 => Self::Priority3,
            4 => Self::Priority4,
            5 => Self::Priority5,
            6 => Self::Priority6,
            7 => Self::Priority7,
            _ => return None,
        })
    }

    pub const fn as_raw(self) -> Option<u8> {
        match self {
            Self::Priority0 => Some(0),
            Self::Priority1 => Some(1),
            Self::Priority2 => Some(2),
            Self::Priority3 => Some(3),
            Self::Priority4 => Some(4),
            Self::Priority5 => Some(5),
            Self::Priority6 => Some(6),
            Self::Priority7 => Some(7),
            Self::Undefined => None,
        }
    }
}

/// Local PDU priority (ETSI range 0 to 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PduPriority(u8);

impl PduPriority {
    pub fn new(value: u8) -> Option<Self> {
        (value <= 7).then_some(Self(value))
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

impl Default for PduPriority {
    fn default() -> Self {
        Self(0)
    }
}

/// SNDCP NSAPI (ETSI range 1 to 14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Nsapi(u8);

impl Nsapi {
    pub fn new(value: u8) -> Option<Self> {
        (1..=14).contains(&value).then_some(Self(value))
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

/// Data-priority random-access delay factor (ETSI range 0 to 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataPriorityRandomAccessDelayFactor(u8);

impl DataPriorityRandomAccessDelayFactor {
    pub fn new(value: u8) -> Option<Self> {
        (value <= 7).then_some(Self(value))
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

/// Data class visible to LLC/MAC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DataClass {
    Background,
    Telemetry,
    RealTime,
    #[default]
    NonClassified,
    Other(u8),
}

/// Data-category reliability level used by lower-layer adaptation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataCategory {
    pub class: DataClass,
    pub reliability_level: u8,
}

/// Original or extended advanced-link format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AdvancedLinkFormat {
    #[default]
    Original,
    Extended,
}

/// Throughput information used during advanced-link QoS negotiation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ThroughputInformation {
    pub bits_per_second: Option<u32>,
    pub timeslots: Option<u8>,
}

/// Layer-2 QoS negotiated for an advanced link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layer2Qos {
    pub throughput: ThroughputInformation,
    pub link_format: AdvancedLinkFormat,
    pub acknowledged_window_size: u8,
    pub max_tl_sdu_retransmissions: u8,
    pub max_segment_retransmissions: u8,
}

impl Layer2Qos {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !(1..=15).contains(&self.acknowledged_window_size) {
            return Err("acknowledged window size must be in 1..=15");
        }
        if self.max_tl_sdu_retransmissions > 7 {
            return Err("TL-SDU retransmissions must be in 0..=7");
        }
        if self.max_segment_retransmissions > 15 {
            return Err("segment retransmissions must be in 0..=15");
        }
        Ok(())
    }
}

impl Default for Layer2Qos {
    fn default() -> Self {
        Self {
            throughput: ThroughputInformation::default(),
            link_format: AdvancedLinkFormat::Original,
            acknowledged_window_size: 1,
            max_tl_sdu_retransmissions: 0,
            max_segment_retransmissions: 0,
        }
    }
}

/// Amount of data currently available for an advanced-link reservation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReservationInfo {
    pub octets_available: u32,
}

/// Result of an MLE-UNITDATA transfer request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferResult {
    SuccessMoreDataBuffered,
    SuccessBufferEmpty,
    DelayedByGracefulDegradation,
    FailedRemovedFromBuffer,
    RejectedByEmergencyCall,
    Other(u8),
}

/// Result reported during advanced-link setup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupReport {
    Success,
    ServiceChange,
    ParametersAcceptable,
    ParametersNotAcceptable,
    Other(u8),
}

/// SNDCP service state visible to MLE and lower layers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SndcpStatus {
    #[default]
    Idle,
    Standby,
    Ready,
}

/// Sleep permission passed from a layer-3 user to MLE.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SleepMode {
    StayAlive,
    #[default]
    SleepPermitted,
}

/// Channel-advice request flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChannelAdvice {
    #[default]
    NotRequested,
    Requested,
}

/// Stealing urgency for a signalling SDU.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StealingPermission {
    StealImmediately,
    StealWithinT214,
    StealWhenConvenient,
    #[default]
    NotRequired,
}

/// Scheduled-data classification for a TL-SDU.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScheduledDataStatus {
    #[default]
    NotScheduled,
    InitialScheduledData,
    ScheduledData,
}

/// Repetition request for an SNDCP schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduleRepetitionInformation {
    pub nsapi: Nsapi,
    pub start: bool,
    pub repetition_period_slots: u16,
}

impl ScheduleRepetitionInformation {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !(4..=706).contains(&self.repetition_period_slots) {
            return Err("schedule repetition period must be in 4..=706 slots");
        }
        Ok(())
    }
}

/// Periodic reporting policy configured for the MS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PeriodicReportingTimer {
    #[default]
    Disabled,
    Interval(Duration),
    UseSwmiRequested,
}

/// Local indication whether any layer-3 entity or advanced link is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MleActivityIndicator {
    #[default]
    Inactive,
    Active,
}

/// LLC timers measured in downlink signalling frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LlcTimerStatus {
    pub t251_running: bool,
    pub t252_running: bool,
    pub t261_running: bool,
    pub t263_running: bool,
    pub t265_running: bool,
}

/// Opaque link-performance score derived by LLC from acknowledgements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LinkPerformanceInformation {
    pub score: i16,
}

/// Control for graceful-service-degradation operation and repetitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GracefulServiceDegradationControl {
    pub active: bool,
    pub repetition_count: u8,
    pub repetition_interval: Duration,
}

/// Energy-economy group 0 to 7.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnergyEconomyGroup(u8);

impl EnergyEconomyGroup {
    pub fn new(value: u8) -> Option<Self> {
        (value <= 7).then_some(Self(value))
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

/// Absolute startpoint for an energy-economy or dual-watch cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnergyEconomyStartpoint {
    pub frame: u8,
    pub multiframe: u8,
}

impl EnergyEconomyStartpoint {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !(1..=18).contains(&self.frame) {
            return Err("frame must be in 1..=18");
        }
        if !(1..=60).contains(&self.multiframe) {
            return Err("multiframe must be in 1..=60");
        }
        Ok(())
    }
}

/// Timeslot to monitor in frame 18 while minimum mode is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame18Distribution {
    pub timeslot: u8,
}

/// SCCH selection information supplied by higher layers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScchInformation {
    pub configuration: u8,
}

/// Threshold set used for monitoring and cell selection.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ThresholdValues {
    pub cell_relinquishable: Option<MeasurementValue>,
    pub cell_improvable: Option<MeasurementValue>,
    pub cell_usable: Option<MeasurementValue>,
    pub channel_relinquishable: Option<MeasurementValue>,
    pub channel_improvable: Option<MeasurementValue>,
    pub channel_usable: Option<MeasurementValue>,
}

/// RF carrier number used by TLMC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct RfChannelNumber(pub u16);

/// Local channel-class reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ChannelClassLabel(pub u16);

/// Broad modulation family relevant to local scanning/monitoring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModulationMode {
    PhaseModulation,
    Qam,
    Other(u8),
}

/// Supported RF-channel bandwidth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelBandwidth {
    Khz25,
    Khz50,
    Khz100,
    Khz150,
    OtherKhz(u16),
}

/// Relation of a monitored channel to its cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelRole {
    ServingMainCarrier,
    NeighbourMainCarrier,
    IrregularCarrier,
    Unknown,
}

/// Conforming/channel-topology information passed with TL-SELECT.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelTopology {
    Conforming,
    NonConformingConcentric,
    Sectored,
    SuperSectored,
    Eccentric,
    Unknown,
}

/// Characteristics of an RF channel used for scan, monitor or select.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RfChannelCharacteristics {
    pub modulation: ModulationMode,
    pub bandwidth: ChannelBandwidth,
    pub max_ms_tx_power_dbm: Option<i16>,
    pub min_rx_access_level_dbm: Option<i16>,
    pub discontinuous: Option<bool>,
    pub role: ChannelRole,
    pub topology: ChannelTopology,
}

/// Information supplied with a selected or indicated RF channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelInformation {
    pub modulation: ModulationMode,
    pub bandwidth: ChannelBandwidth,
    pub topology: ChannelTopology,
}

/// Characteristics used to assess one channel class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelClassCharacteristics {
    pub modulation: ModulationMode,
    pub max_ms_tx_power_dbm: i16,
    pub min_rx_access_level_dbm: i16,
    pub bs_power_relative_to_main_db: i16,
}

/// Request to assess one channel class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelClassAssessmentRequest {
    pub label: ChannelClassLabel,
    pub characteristics: ChannelClassCharacteristics,
}

/// Measured/assessed path loss for one channel class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelClassMeasurement {
    pub label: ChannelClassLabel,
    pub path_loss: MeasurementValue,
}

/// Local reception-quality indication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QualityIndication {
    pub raw: i16,
}

/// Method MAC shall use while scanning a carrier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanningMeasurementMethod {
    Interrupting,
    NonInterrupting,
    Other(u8),
}

/// General lower-layer report values relevant to TLMC and LTPD.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer2Report {
    AbortedNotCompletelySent,
    AbortedSentAtLeastOnce,
    ChannelReplacementAdvisable,
    ChannelReplacementBeneficial,
    Close,
    CommonChannelDeallocated,
    CurrentChannelAcceptable,
    DisconnectionFailure,
    DownlinkFailure,
    FailedTransfer,
    FirstCompleteTransmission,
    IncomingDisconnection,
    Layer2TransmissionContinuing,
    LocalDisconnection,
    MaximumPathDelayExceeded,
    MaximumPathDelayAlmostExceeded,
    NetworkBroadcastNotReceived,
    NetworkBroadcastReceived,
    RandomAccessFailure,
    Reject,
    Reset,
    ScheduleTimingPrompt,
    ServiceChange,
    ServiceDefinition,
    ServiceNotSupported,
    ServiceTemporarilyUnavailable,
    SetupFailure,
    Success,
    UsageMarkerMismatch,
    UplinkFailure,
    Other(u16),
}

/// Result of an advanced-link reconnection attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconnectionResult {
    Success,
    Reject,
    Other(u8),
}

/// Current cell permission conveyed to SNDCP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermittedCellInformation {
    Permitted,
    NotPermitted,
}

/// Address classification supplied with MLE-RECEIVE/UNITDATA indication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceivedAddressType {
    IndividualAllocated,
    IndividualUnexchanged,
    Group,
    Other,
}

impl ReceivedAddressType {
    pub fn from_tetra_address(address: TetraAddress) -> Self {
        match address.ssi_type {
            SsiType::Issi | SsiType::Ssi => Self::IndividualAllocated,
            SsiType::Ussi => Self::IndividualUnexchanged,
            SsiType::Gssi => Self::Group,
            _ => Self::Other,
        }
    }
}

/// Services that remain available while a terminal is temporarily disabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PermittedTemporaryServices {
    pub ambience_listening: bool,
    pub lip: bool,
}

/// Snapshot of broadcast information relevant to SNDCP.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MleBroadcastParameters {
    pub mcc: Option<u16>,
    pub mnc: Option<u16>,
    pub location_area: Option<u16>,
    pub colour_code: Option<u8>,
    pub main_carrier: Option<u16>,
    pub packet_data_supported: Option<bool>,
    pub data_priority_supported: Option<bool>,
}

/// Explicit MLE cell-selection lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum MleCellState {
    #[default]
    Null,
    Serving(CellIdentity),
    Scanning,
    CandidateSelected(CellCandidate),
    Preparing(CellCandidate),
    WaitingForNewCell(CellCandidate),
    ChangingChannel(CellCandidate),
    Restoring(CellCandidate),
    Resuming(CellIdentity),
    Failed,
}

/// Explicit lifecycle of one channel-change request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelChangeState {
    Requested {
        handle: ChannelChangeHandle,
        candidate: CellCandidate,
    },
    AwaitingDecision {
        handle: ChannelChangeHandle,
        candidate: CellCandidate,
    },
    Accepted {
        handle: ChannelChangeHandle,
        candidate: CellCandidate,
    },
    Rejected {
        handle: ChannelChangeHandle,
        candidate: CellCandidate,
    },
    Committed {
        handle: ChannelChangeHandle,
        cell: CellIdentity,
    },
    Failed {
        handle: ChannelChangeHandle,
        result: SelectionResult,
    },
}

/// Local scan state used before Package C adds runtime transitions.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TlmcScanState {
    #[default]
    Idle,
    Requested {
        request_id: ScanRequestId,
        channel: RfChannelNumber,
    },
    Running {
        request_id: ScanRequestId,
        channel: RfChannelNumber,
    },
    Completed {
        request_id: ScanRequestId,
        candidate: CellCandidate,
    },
    Failed {
        request_id: ScanRequestId,
        report: Layer2Report,
    },
}

/// Local selection state used before Package C adds runtime transitions.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TlmcSelectionState {
    #[default]
    Idle,
    Requested {
        candidate: CellCandidate,
        cause: SelectionCause,
    },
    AwaitingResponse {
        candidate: CellCandidate,
    },
    Completed {
        cell: CellIdentity,
    },
    Failed {
        result: SelectionResult,
    },
}

/// Link lifecycle visible at the LTPD-SAP.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LtpdLinkState {
    #[default]
    Null,
    Open,
    Connecting,
    Connected,
    Busy,
    Broken,
    Reconnecting,
    Releasing,
    Closed,
    Disabled,
}

/// Stable key for one SNDCP/LTPD context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LtpdContextKey {
    pub subscriber_ssi: u32,
    pub nsapi: Nsapi,
    pub endpoint_id: EndpointId,
    pub link_id: LinkId,
}

/// Context passed between mobility and call-control during call restoration.
#[derive(Debug, Clone)]
pub struct RestoreContext {
    pub subscriber: TetraAddress,
    pub old_endpoint_id: EndpointId,
    pub old_link_id: LinkId,
    pub target_cell: Option<CellIdentity>,
    pub call_identifier: Option<u16>,
    pub cmce_restore_payload: Option<BitBuffer>,
}

impl fmt::Display for LtpdContextKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ssi={} nsapi={} endpoint={} link={}",
            self.subscriber_ssi,
            self.nsapi.get(),
            self.endpoint_id,
            self.link_id
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constrained_values_reject_out_of_range_inputs() {
        assert!(PduPriority::new(7).is_some());
        assert!(PduPriority::new(8).is_none());
        assert!(Nsapi::new(1).is_some());
        assert!(Nsapi::new(14).is_some());
        assert!(Nsapi::new(0).is_none());
        assert!(Nsapi::new(15).is_none());
        assert!(DataPriorityRandomAccessDelayFactor::new(7).is_some());
        assert!(DataPriorityRandomAccessDelayFactor::new(8).is_none());
    }

    #[test]
    fn layer2_qos_validation_matches_etsi_ranges() {
        let valid = Layer2Qos::default();
        assert!(valid.validate().is_ok());

        let invalid = Layer2Qos {
            acknowledged_window_size: 16,
            ..valid
        };
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn energy_economy_startpoint_has_explicit_ranges() {
        let valid = EnergyEconomyStartpoint {
            frame: 18,
            multiframe: 60,
        };
        assert!(valid.validate().is_ok());

        let invalid = EnergyEconomyStartpoint {
            frame: 19,
            multiframe: 60,
        };
        assert!(invalid.validate().is_err());
    }
}

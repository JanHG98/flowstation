//! TLC/TMC service primitives used between MLE, LLC and MAC.
//!
//! ETSI EN 300 392-2 defines the TLC-SAP and TMC-SAP as local management
//! interfaces.  In NetCore-Tetra they are represented by one merged `TlmcSap`.

use tetra_core::EndpointId;

use crate::common::{
    CallReleaseInstruction, ChannelChangeDecision, ChannelChangeHandle, ChannelClassAssessmentRequest,
    ChannelClassMeasurement, ChannelInformation, DataClass, DataPriority, DataPriorityRandomAccessDelayFactor,
    EnergyEconomyGroup, EnergyEconomyStartpoint, Frame18Distribution, GracefulServiceDegradationControl,
    Layer2Report, LinkPerformanceInformation, LlcTimerStatus, LowerLayerResourceAvailability, LowerLayerResourceReason, MeasurementReport,
    MeasurementValue, MleActivityIndicator, OperatingMode, PeriodicReportingTimer, QualityIndication,
    RequestHandle, RfChannelCharacteristics, RfChannelNumber, ScanRequestId, ScanningMeasurementMethod,
    ScheduleRepetitionInformation, ScchInformation, SelectionCause, SelectionResult, ThresholdValues,
};

/// Information identifying the currently valid network address scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TlmcValidAddress {
    pub mcc: u16,
    pub mnc: u16,
}

/// Result of path-loss assessment for one or more channel classes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlmcAssessmentInd {
    pub assessments: Vec<ChannelClassMeasurement>,
}

/// Start assessment for a list of channel classes on the serving cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlmcAssessmentListReq {
    pub classes: Vec<ChannelClassAssessmentRequest>,
}

/// Request reading SYSINFO-DA on a DA-neighbour main carrier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlmcCellReadReq {
    pub request_id: ScanRequestId,
    pub channel_number: RfChannelNumber,
    pub characteristics: Option<RfChannelCharacteristics>,
}

/// Compatibility alias for the earlier incorrectly named placeholder.
#[deprecated(note = "use TlmcCellReadReq; ETSI defines a request/confirm pair")]
pub type TlmcCellReadInd = TlmcCellReadReq;

/// Completion of a SYSINFO-DA cell-read request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlmcCellReadConf {
    pub request_id: ScanRequestId,
    pub channel_number: RfChannelNumber,
    pub report: Layer2Report,
}

/// TMC-CONFIGURE indication for loss or recovery of a local MAC resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlmcConfigureInd {
    pub endpoint_id: EndpointId,
    pub lower_layer_resource_availability: LowerLayerResourceAvailability,
    pub reason: LowerLayerResourceReason,
}

/// TL/TMC-CONFIGURE request.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TlmcConfigureReq {
    pub threshold_values: Option<ThresholdValues>,
    pub distribution_on_18th_frame: Option<Frame18Distribution>,
    pub scch_information: Option<ScchInformation>,
    pub energy_economy_group: Option<EnergyEconomyGroup>,
    pub energy_economy_startpoint: Option<EnergyEconomyStartpoint>,
    pub dual_watch_energy_economy_group: Option<EnergyEconomyGroup>,
    pub dual_watch_startpoint: Option<EnergyEconomyStartpoint>,
    pub mle_activity_indicator: Option<MleActivityIndicator>,
    pub channel_change_accepted: Option<ChannelChangeDecision>,
    pub channel_change_handle: Option<ChannelChangeHandle>,
    pub operating_mode: Option<OperatingMode>,
    pub call_release: Option<CallReleaseInstruction>,
    pub valid_addresses: Option<TlmcValidAddress>,
    pub ms_default_data_priority: Option<DataPriority>,
    pub layer_2_data_priority_lifetime: Option<std::time::Duration>,
    pub layer_2_data_priority_signalling_delay: Option<std::time::Duration>,
    pub data_priority_random_access_delay_factor: Option<DataPriorityRandomAccessDelayFactor>,
    pub schedule_repetition_information: Option<ScheduleRepetitionInformation>,
    pub data_class_activity_information: Option<DataClass>,
    pub endpoint_id: Option<EndpointId>,
    pub periodic_reporting_timer: Option<PeriodicReportingTimer>,
    pub graceful_service_degradation_mode_control: Option<GracefulServiceDegradationControl>,
    pub llc_timer_status: Option<LlcTimerStatus>,
    pub link_performance_information: Option<LinkPerformanceInformation>,
}

/// TL-CONFIGURE confirmation. Only values reflected by lower layers are carried.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TlmcConfigureConf {
    pub threshold_values: Option<ThresholdValues>,
    pub distribution_on_18th_frame: Option<Frame18Distribution>,
    pub scch_information: Option<ScchInformation>,
    pub energy_economy_group: Option<EnergyEconomyGroup>,
    pub energy_economy_startpoint: Option<EnergyEconomyStartpoint>,
    pub dual_watch_energy_economy_group: Option<EnergyEconomyGroup>,
    pub dual_watch_startpoint: Option<EnergyEconomyStartpoint>,
    pub operating_mode: Option<OperatingMode>,
    pub call_release: Option<CallReleaseInstruction>,
    pub valid_addresses: Option<TlmcValidAddress>,
    pub ms_default_data_priority: Option<DataPriority>,
    pub layer_2_data_priority_lifetime: Option<std::time::Duration>,
    pub layer_2_data_priority_signalling_delay: Option<std::time::Duration>,
    pub data_priority_random_access_delay_factor: Option<DataPriorityRandomAccessDelayFactor>,
    pub schedule_repetition_information: Option<ScheduleRepetitionInformation>,
    pub data_class_activity_information: Option<DataClass>,
    pub endpoint_id: Option<EndpointId>,
}

/// Quality of the current serving-channel link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlmcMeasurementInd {
    pub measurement: MeasurementReport,
}

/// Result of monitoring one RF channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlmcMonitorInd {
    pub channel_number: RfChannelNumber,
    pub path_loss_c2: MeasurementValue,
    pub quality: Option<QualityIndication>,
    pub channel_classes: Vec<ChannelClassMeasurement>,
}

/// One channel and optional channel classes included in a monitor list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlmcMonitorChannel {
    pub channel_number: RfChannelNumber,
    pub characteristics: RfChannelCharacteristics,
    pub channel_classes: Vec<ChannelClassAssessmentRequest>,
}

/// Start monitoring one or more RF channels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlmcMonitorListReq {
    pub channels: Vec<TlmcMonitorChannel>,
}

/// Local status/progress report generated by LLC or MAC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlmcReportInd {
    pub request_handle: Option<RequestHandle>,
    pub report: Layer2Report,
    pub endpoint_id: Option<EndpointId>,
    pub nsapi: Option<crate::common::Nsapi>,
}

/// Start scanning a neighbour-cell main carrier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlmcScanReq {
    pub request_id: ScanRequestId,
    pub channel_number: RfChannelNumber,
    pub measurement_method: ScanningMeasurementMethod,
    pub characteristics: Option<RfChannelCharacteristics>,
    pub threshold_level: Option<MeasurementValue>,
    pub channel_classes: Vec<ChannelClassAssessmentRequest>,
}

/// Completion of an explicit scan request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlmcScanConf {
    pub request_id: ScanRequestId,
    pub channel_number: RfChannelNumber,
    pub measurement_method: ScanningMeasurementMethod,
    pub threshold_level: MeasurementValue,
    pub report: Layer2Report,
    pub channel_classes: Vec<ChannelClassMeasurement>,
}

/// Updated scan measurement reported after monitoring/scanning has completed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlmcScanReportInd {
    pub request_id: Option<ScanRequestId>,
    pub channel_number: RfChannelNumber,
    pub path_loss_c1: MeasurementValue,
    pub report: Option<Layer2Report>,
    pub channel_classes: Vec<ChannelClassMeasurement>,
}

/// Request MAC to tune to an RF channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlmcSelectReq {
    pub channel_number: RfChannelNumber,
    pub channel_information: Option<ChannelInformation>,
    pub threshold_level: Option<MeasurementValue>,
    pub main_carrier_number: Option<RfChannelNumber>,
    pub main_carrier_information: Option<ChannelInformation>,
    pub cause: SelectionCause,
}

/// BS-controlled channel change reported by MAC to MLE.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlmcSelectInd {
    pub channel_number: RfChannelNumber,
    pub channel_information: ChannelInformation,
    pub threshold_level: Option<MeasurementValue>,
    pub report: Option<Layer2Report>,
    pub channel_change_handle: Option<ChannelChangeHandle>,
}

/// MLE response to a cell-change indication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlmcSelectResp {
    pub channel_number: RfChannelNumber,
    pub threshold_level: Option<MeasurementValue>,
    pub main_carrier_number: Option<RfChannelNumber>,
    pub report: Option<Layer2Report>,
    pub channel_change_handle: Option<ChannelChangeHandle>,
    pub decision: ChannelChangeDecision,
}

/// Completion of a requested channel selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlmcSelectConf {
    pub channel_number: RfChannelNumber,
    pub threshold_level: Option<MeasurementValue>,
    pub main_carrier_number: Option<RfChannelNumber>,
    pub report: Option<Layer2Report>,
    pub result: SelectionResult,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::{ChannelBandwidth, ChannelRole, ChannelTopology, ModulationMode};

    fn characteristics() -> RfChannelCharacteristics {
        RfChannelCharacteristics {
            modulation: ModulationMode::PhaseModulation,
            bandwidth: ChannelBandwidth::Khz25,
            max_ms_tx_power_dbm: None,
            min_rx_access_level_dbm: None,
            discontinuous: None,
            role: ChannelRole::NeighbourMainCarrier,
            topology: ChannelTopology::Conforming,
        }
    }

    #[test]
    fn scan_request_keeps_explicit_correlation_and_units() {
        let request = TlmcScanReq {
            request_id: ScanRequestId(42),
            channel_number: RfChannelNumber(720),
            measurement_method: ScanningMeasurementMethod::NonInterrupting,
            characteristics: Some(characteristics()),
            threshold_level: Some(MeasurementValue::db(-12)),
            channel_classes: Vec::new(),
        };

        assert_eq!(request.request_id, ScanRequestId(42));
        assert_eq!(request.channel_number, RfChannelNumber(720));
        assert_eq!(request.threshold_level, Some(MeasurementValue::db(-12)));
    }

    #[test]
    fn configure_default_does_not_invent_normative_values() {
        let configure = TlmcConfigureReq::default();
        assert!(configure.valid_addresses.is_none());
        assert!(configure.endpoint_id.is_none());
        assert!(configure.channel_change_accepted.is_none());
    }
}

//! Runtime state machine for the merged TLC/TMC service access point.
//!
//! TLMC is a local management interface between MLE and layer 2.  It does not
//! carry over the air and it is not part of the future TBS/backend protocol.
//! The runtime deliberately separates request/state handling from the actual RF
//! adapter: UMAC owns this state machine, while LMAC/PHY observations complete
//! scan, monitor, cell-read and selection operations.

use std::collections::HashMap;
use std::fmt;

use tetra_core::EndpointId;
use tetra_saps::common::{
    CellCandidate, CellIdentity, CellServiceLevel, ChannelChangeDecision, ChannelChangeHandle,
    ChannelClassAssessmentRequest, ChannelClassLabel, ChannelClassMeasurement, Layer2Report,
    LowerLayerResourceAvailability, LowerLayerResourceReason, MeasurementReport, MeasurementValue,
    QualityIndication, RfChannelNumber, ScanRequestId, SelectionResult, TlmcScanState,
    TlmcSelectionState,
};
use tetra_saps::tlmc::{
    TlmcAssessmentInd, TlmcAssessmentListReq, TlmcCellReadConf, TlmcCellReadReq,
    TlmcConfigureConf, TlmcConfigureInd, TlmcConfigureReq, TlmcMeasurementInd,
    TlmcMonitorChannel, TlmcMonitorInd, TlmcMonitorListReq, TlmcScanConf, TlmcScanReportInd,
    TlmcScanReq, TlmcSelectConf, TlmcSelectInd, TlmcSelectReq, TlmcSelectResp,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TlmcRuntimeError {
    InvalidConfiguration(&'static str),
    OperationBusy(&'static str),
    UnknownRequest(&'static str),
    RequestMismatch(&'static str),
    ChannelNotMonitored(RfChannelNumber),
    ChannelClassNotRequested(ChannelClassLabel),
    NoPendingSelection,
}

impl fmt::Display for TlmcRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration(reason) => write!(f, "invalid TLMC configuration: {reason}"),
            Self::OperationBusy(operation) => write!(f, "TLMC operation already active: {operation}"),
            Self::UnknownRequest(operation) => write!(f, "unknown TLMC request: {operation}"),
            Self::RequestMismatch(operation) => write!(f, "TLMC request correlation mismatch: {operation}"),
            Self::ChannelNotMonitored(channel) => write!(f, "channel {} is not monitored", channel.0),
            Self::ChannelClassNotRequested(label) => write!(f, "channel class {} was not requested", label.0),
            Self::NoPendingSelection => write!(f, "no TLMC selection is waiting for a response"),
        }
    }
}

impl std::error::Error for TlmcRuntimeError {}

/// Read-only operational view for diagnostics and the future TBS WebUI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlmcRuntimeSnapshot {
    pub scan_state: TlmcScanState,
    pub selection_state: TlmcSelectionState,
    pub configured_endpoint: Option<EndpointId>,
    pub monitored_channels: Vec<RfChannelNumber>,
    pub assessed_channel_classes: Vec<ChannelClassLabel>,
    pub pending_cell_read: Option<(ScanRequestId, RfChannelNumber)>,
    pub known_resource_count: usize,
    pub unavailable_resource_count: usize,
    pub last_measurement: Option<MeasurementReport>,
}

#[derive(Debug, Clone, Default)]
pub struct TlmcRuntime {
    configuration: TlmcConfigureReq,
    endpoint_resources: HashMap<EndpointId, LowerLayerResourceAvailability>,
    monitored_channels: HashMap<RfChannelNumber, TlmcMonitorChannel>,
    assessment_classes: HashMap<ChannelClassLabel, ChannelClassAssessmentRequest>,
    pending_scan: Option<TlmcScanReq>,
    pending_cell_read: Option<TlmcCellReadReq>,
    pending_select: Option<TlmcSelectReq>,
    pending_select_indication: Option<TlmcSelectInd>,
    scan_state: TlmcScanState,
    selection_state: TlmcSelectionState,
    current_cell: Option<CellIdentity>,
    last_measurement: Option<MeasurementReport>,
    last_monitor: HashMap<RfChannelNumber, TlmcMonitorInd>,
}

impl TlmcRuntime {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn snapshot(&self) -> TlmcRuntimeSnapshot {
        let mut monitored_channels: Vec<_> = self.monitored_channels.keys().copied().collect();
        monitored_channels.sort_by_key(|channel| channel.0);

        let mut assessed_channel_classes: Vec<_> = self.assessment_classes.keys().copied().collect();
        assessed_channel_classes.sort_by_key(|label| label.0);

        TlmcRuntimeSnapshot {
            scan_state: self.scan_state.clone(),
            selection_state: self.selection_state.clone(),
            configured_endpoint: self.configuration.endpoint_id,
            monitored_channels,
            assessed_channel_classes,
            pending_cell_read: self
                .pending_cell_read
                .as_ref()
                .map(|request| (request.request_id, request.channel_number)),
            known_resource_count: self.endpoint_resources.len(),
            unavailable_resource_count: self
                .endpoint_resources
                .values()
                .filter(|availability| **availability == LowerLayerResourceAvailability::Unavailable)
                .count(),
            last_measurement: self.last_measurement.clone(),
        }
    }

    pub fn configuration(&self) -> &TlmcConfigureReq {
        &self.configuration
    }

    pub fn current_cell(&self) -> Option<&CellIdentity> {
        self.current_cell.as_ref()
    }

    pub fn scan_state(&self) -> &TlmcScanState {
        &self.scan_state
    }

    pub fn selection_state(&self) -> &TlmcSelectionState {
        &self.selection_state
    }

    pub fn pending_scan(&self) -> Option<&TlmcScanReq> {
        self.pending_scan.as_ref()
    }

    pub fn pending_cell_read(&self) -> Option<&TlmcCellReadReq> {
        self.pending_cell_read.as_ref()
    }

    pub fn pending_select(&self) -> Option<&TlmcSelectReq> {
        self.pending_select.as_ref()
    }

    pub fn apply_configure(&mut self, request: TlmcConfigureReq) -> Result<TlmcConfigureConf, TlmcRuntimeError> {
        Self::validate_configure(&request)?;
        Self::merge_configure(&mut self.configuration, request);
        Ok(Self::configure_confirmation(&self.configuration))
    }

    fn validate_configure(request: &TlmcConfigureReq) -> Result<(), TlmcRuntimeError> {
        if let Some(distribution) = request.distribution_on_18th_frame {
            if !(1..=4).contains(&distribution.timeslot) {
                return Err(TlmcRuntimeError::InvalidConfiguration(
                    "frame-18 monitoring timeslot must be in 1..=4",
                ));
            }
        }
        if let Some(startpoint) = request.energy_economy_startpoint {
            startpoint
                .validate()
                .map_err(TlmcRuntimeError::InvalidConfiguration)?;
        }
        if let Some(startpoint) = request.dual_watch_startpoint {
            startpoint
                .validate()
                .map_err(TlmcRuntimeError::InvalidConfiguration)?;
        }
        if let Some(schedule) = request.schedule_repetition_information {
            schedule
                .validate()
                .map_err(TlmcRuntimeError::InvalidConfiguration)?;
        }
        Ok(())
    }

    fn merge_configure(current: &mut TlmcConfigureReq, update: TlmcConfigureReq) {
        macro_rules! merge_option {
            ($field:ident) => {
                if update.$field.is_some() {
                    current.$field = update.$field;
                }
            };
        }

        merge_option!(threshold_values);
        merge_option!(distribution_on_18th_frame);
        merge_option!(scch_information);
        merge_option!(energy_economy_group);
        merge_option!(energy_economy_startpoint);
        merge_option!(dual_watch_energy_economy_group);
        merge_option!(dual_watch_startpoint);
        merge_option!(mle_activity_indicator);
        merge_option!(channel_change_accepted);
        merge_option!(channel_change_handle);
        merge_option!(operating_mode);
        merge_option!(call_release);
        merge_option!(valid_addresses);
        merge_option!(ms_default_data_priority);
        merge_option!(layer_2_data_priority_lifetime);
        merge_option!(layer_2_data_priority_signalling_delay);
        merge_option!(data_priority_random_access_delay_factor);
        merge_option!(schedule_repetition_information);
        merge_option!(data_class_activity_information);
        merge_option!(endpoint_id);
        merge_option!(periodic_reporting_timer);
        merge_option!(graceful_service_degradation_mode_control);
        merge_option!(llc_timer_status);
        merge_option!(link_performance_information);
    }

    fn configure_confirmation(configuration: &TlmcConfigureReq) -> TlmcConfigureConf {
        TlmcConfigureConf {
            threshold_values: configuration.threshold_values.clone(),
            distribution_on_18th_frame: configuration.distribution_on_18th_frame,
            scch_information: configuration.scch_information,
            energy_economy_group: configuration.energy_economy_group,
            energy_economy_startpoint: configuration.energy_economy_startpoint,
            dual_watch_energy_economy_group: configuration.dual_watch_energy_economy_group,
            dual_watch_startpoint: configuration.dual_watch_startpoint,
            operating_mode: configuration.operating_mode.clone(),
            call_release: configuration.call_release,
            valid_addresses: configuration.valid_addresses,
            ms_default_data_priority: configuration.ms_default_data_priority,
            layer_2_data_priority_lifetime: configuration.layer_2_data_priority_lifetime,
            layer_2_data_priority_signalling_delay: configuration.layer_2_data_priority_signalling_delay,
            data_priority_random_access_delay_factor: configuration.data_priority_random_access_delay_factor,
            schedule_repetition_information: configuration.schedule_repetition_information,
            data_class_activity_information: configuration.data_class_activity_information,
            endpoint_id: configuration.endpoint_id,
        }
    }

    /// Record an edge-triggered lower-layer resource transition.
    pub fn resource_transition(
        &mut self,
        endpoint_id: EndpointId,
        availability: LowerLayerResourceAvailability,
        reason: LowerLayerResourceReason,
    ) -> Option<TlmcConfigureInd> {
        if self.endpoint_resources.get(&endpoint_id).copied() == Some(availability) {
            return None;
        }
        self.endpoint_resources.insert(endpoint_id, availability);
        Some(TlmcConfigureInd {
            endpoint_id,
            lower_layer_resource_availability: availability,
            reason,
        })
    }

    pub fn record_measurement(&mut self, measurement: MeasurementReport) -> TlmcMeasurementInd {
        self.last_measurement = Some(measurement.clone());
        TlmcMeasurementInd { measurement }
    }

    pub fn set_monitor_list(&mut self, request: TlmcMonitorListReq) {
        self.monitored_channels.clear();
        for channel in request.channels {
            self.monitored_channels.insert(channel.channel_number, channel);
        }
    }

    pub fn is_monitored(&self, channel_number: RfChannelNumber) -> bool {
        self.monitored_channels.contains_key(&channel_number)
    }

    pub fn record_monitor(
        &mut self,
        channel_number: RfChannelNumber,
        path_loss_c2: MeasurementValue,
        quality: Option<QualityIndication>,
        channel_classes: Vec<ChannelClassMeasurement>,
    ) -> Result<TlmcMonitorInd, TlmcRuntimeError> {
        if !self.is_monitored(channel_number) {
            return Err(TlmcRuntimeError::ChannelNotMonitored(channel_number));
        }
        let indication = TlmcMonitorInd {
            channel_number,
            path_loss_c2,
            quality,
            channel_classes,
        };
        self.last_monitor.insert(channel_number, indication.clone());
        Ok(indication)
    }

    pub fn set_assessment_list(&mut self, request: TlmcAssessmentListReq) {
        self.assessment_classes.clear();
        for class in request.classes {
            self.assessment_classes.insert(class.label, class);
        }
    }

    pub fn record_assessment(
        &self,
        assessments: Vec<ChannelClassMeasurement>,
    ) -> Result<TlmcAssessmentInd, TlmcRuntimeError> {
        for assessment in &assessments {
            if !self.assessment_classes.contains_key(&assessment.label) {
                return Err(TlmcRuntimeError::ChannelClassNotRequested(assessment.label));
            }
        }
        Ok(TlmcAssessmentInd { assessments })
    }

    pub fn begin_scan(&mut self, request: TlmcScanReq) -> Result<(), TlmcRuntimeError> {
        if self.pending_scan.is_some() {
            return Err(TlmcRuntimeError::OperationBusy("scan"));
        }
        self.scan_state = TlmcScanState::Requested {
            request_id: request.request_id,
            channel: request.channel_number,
        };
        self.pending_scan = Some(request.clone());
        self.scan_state = TlmcScanState::Running {
            request_id: request.request_id,
            channel: request.channel_number,
        };
        Ok(())
    }

    pub fn complete_scan(
        &mut self,
        request_id: ScanRequestId,
        channel_number: RfChannelNumber,
        threshold_level: MeasurementValue,
        report: Layer2Report,
        channel_classes: Vec<ChannelClassMeasurement>,
        identity: Option<CellIdentity>,
        service_level: CellServiceLevel,
    ) -> Result<TlmcScanConf, TlmcRuntimeError> {
        let request = self
            .pending_scan
            .take()
            .ok_or(TlmcRuntimeError::UnknownRequest("scan"))?;
        if request.request_id != request_id || request.channel_number != channel_number {
            self.pending_scan = Some(request);
            return Err(TlmcRuntimeError::RequestMismatch("scan"));
        }

        if report == Layer2Report::Success {
            let candidate = CellCandidate {
                identity,
                channel_number,
                service_level,
                measurements: MeasurementReport {
                    endpoint_id: self.configuration.endpoint_id,
                    channel_number: Some(channel_number),
                    path_loss_c1: Some(threshold_level),
                    path_loss_c2: None,
                    path_loss_c3: None,
                    path_loss_c4: None,
                    path_loss_c5: None,
                    quality: None,
                },
            };
            self.scan_state = TlmcScanState::Completed {
                request_id,
                candidate,
            };
        } else {
            self.scan_state = TlmcScanState::Failed { request_id, report };
        }

        Ok(TlmcScanConf {
            request_id,
            channel_number,
            measurement_method: request.measurement_method,
            threshold_level,
            report,
            channel_classes,
        })
    }

    pub fn record_scan_report(
        &mut self,
        request_id: Option<ScanRequestId>,
        channel_number: RfChannelNumber,
        path_loss_c1: MeasurementValue,
        report: Option<Layer2Report>,
        channel_classes: Vec<ChannelClassMeasurement>,
    ) -> TlmcScanReportInd {
        TlmcScanReportInd {
            request_id,
            channel_number,
            path_loss_c1,
            report,
            channel_classes,
        }
    }

    pub fn begin_cell_read(&mut self, request: TlmcCellReadReq) -> Result<(), TlmcRuntimeError> {
        if self.pending_cell_read.is_some() {
            return Err(TlmcRuntimeError::OperationBusy("cell read"));
        }
        self.pending_cell_read = Some(request);
        Ok(())
    }

    pub fn complete_cell_read(
        &mut self,
        request_id: ScanRequestId,
        channel_number: RfChannelNumber,
        report: Layer2Report,
    ) -> Result<TlmcCellReadConf, TlmcRuntimeError> {
        let request = self
            .pending_cell_read
            .take()
            .ok_or(TlmcRuntimeError::UnknownRequest("cell read"))?;
        if request.request_id != request_id || request.channel_number != channel_number {
            self.pending_cell_read = Some(request);
            return Err(TlmcRuntimeError::RequestMismatch("cell read"));
        }
        Ok(TlmcCellReadConf {
            request_id,
            channel_number,
            report,
        })
    }

    pub fn begin_select(&mut self, request: TlmcSelectReq) -> Result<(), TlmcRuntimeError> {
        if self.pending_select.is_some() || self.pending_select_indication.is_some() {
            return Err(TlmcRuntimeError::OperationBusy("selection"));
        }
        let candidate = Self::candidate_from_select_request(&request);
        self.selection_state = TlmcSelectionState::Requested {
            candidate: candidate.clone(),
            cause: request.cause,
        };
        self.pending_select = Some(request);
        self.selection_state = TlmcSelectionState::AwaitingResponse { candidate };
        Ok(())
    }

    pub fn complete_select(
        &mut self,
        channel_number: RfChannelNumber,
        threshold_level: Option<MeasurementValue>,
        main_carrier_number: Option<RfChannelNumber>,
        report: Option<Layer2Report>,
        result: SelectionResult,
        identity: Option<CellIdentity>,
    ) -> Result<TlmcSelectConf, TlmcRuntimeError> {
        let request = self
            .pending_select
            .take()
            .ok_or(TlmcRuntimeError::UnknownRequest("selection"))?;
        if request.channel_number != channel_number {
            self.pending_select = Some(request);
            return Err(TlmcRuntimeError::RequestMismatch("selection"));
        }

        let effective_result = if result == SelectionResult::Success {
            let selected_cell = identity.or_else(|| {
                self.configuration.valid_addresses.map(|address| CellIdentity {
                    mcc: address.mcc,
                    mnc: address.mnc,
                    location_area: None,
                    colour_code: None,
                    main_carrier: main_carrier_number.unwrap_or(channel_number).0,
                    cell_type: Default::default(),
                })
            });
            if let Some(cell) = selected_cell {
                self.current_cell = Some(cell.clone());
                self.selection_state = TlmcSelectionState::Completed { cell };
                SelectionResult::Success
            } else {
                let failure = SelectionResult::Other(0);
                self.selection_state = TlmcSelectionState::Failed { result: failure };
                failure
            }
        } else {
            self.selection_state = TlmcSelectionState::Failed { result };
            result
        };

        Ok(TlmcSelectConf {
            channel_number,
            threshold_level,
            main_carrier_number,
            report,
            result: effective_result,
        })
    }

    pub fn receive_select_indication(&mut self, indication: TlmcSelectInd) -> Result<(), TlmcRuntimeError> {
        if self.pending_select.is_some() || self.pending_select_indication.is_some() {
            return Err(TlmcRuntimeError::OperationBusy("selection indication"));
        }
        let candidate = CellCandidate {
            identity: None,
            channel_number: indication.channel_number,
            service_level: CellServiceLevel::NormalService,
            measurements: MeasurementReport {
                endpoint_id: self.configuration.endpoint_id,
                channel_number: Some(indication.channel_number),
                path_loss_c1: indication.threshold_level,
                path_loss_c2: None,
                path_loss_c3: None,
                path_loss_c4: None,
                path_loss_c5: None,
                quality: None,
            },
        };
        self.selection_state = TlmcSelectionState::AwaitingResponse { candidate };
        self.pending_select_indication = Some(indication);
        Ok(())
    }

    pub fn respond_select(&mut self, response: TlmcSelectResp) -> Result<(), TlmcRuntimeError> {
        let indication = self
            .pending_select_indication
            .take()
            .ok_or(TlmcRuntimeError::NoPendingSelection)?;
        if indication.channel_number != response.channel_number
            || indication.channel_change_handle != response.channel_change_handle
        {
            self.pending_select_indication = Some(indication);
            return Err(TlmcRuntimeError::RequestMismatch("selection response"));
        }

        let candidate = CellCandidate {
            identity: None,
            channel_number: response.channel_number,
            service_level: CellServiceLevel::NormalService,
            measurements: MeasurementReport {
                endpoint_id: self.configuration.endpoint_id,
                channel_number: Some(response.channel_number),
                path_loss_c1: response.threshold_level,
                path_loss_c2: None,
                path_loss_c3: None,
                path_loss_c4: None,
                path_loss_c5: None,
                quality: None,
            },
        };
        self.selection_state = match response.decision {
            ChannelChangeDecision::Accept => TlmcSelectionState::AwaitingResponse { candidate },
            ChannelChangeDecision::Reject | ChannelChangeDecision::Ignore => TlmcSelectionState::Failed {
                result: SelectionResult::Reject,
            },
        };
        Ok(())
    }

    pub fn channel_change_handle(&self) -> Option<ChannelChangeHandle> {
        self.pending_select_indication
            .as_ref()
            .and_then(|indication| indication.channel_change_handle)
    }

    fn candidate_from_select_request(request: &TlmcSelectReq) -> CellCandidate {
        CellCandidate {
            identity: None,
            channel_number: request.channel_number,
            service_level: CellServiceLevel::NormalService,
            measurements: MeasurementReport {
                endpoint_id: None,
                channel_number: Some(request.channel_number),
                path_loss_c1: request.threshold_level,
                path_loss_c2: None,
                path_loss_c3: None,
                path_loss_c4: None,
                path_loss_c5: None,
                quality: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tetra_saps::common::{
        ChannelBandwidth, ChannelInformation, ChannelRole, ChannelTopology, Frame18Distribution,
        ModulationMode, ScanningMeasurementMethod, SelectionCause,
    };

    fn channel_info() -> ChannelInformation {
        ChannelInformation {
            modulation: ModulationMode::PhaseModulation,
            bandwidth: ChannelBandwidth::Khz25,
            topology: ChannelTopology::Conforming,
        }
    }

    #[test]
    fn configure_merges_partial_updates_and_reports_resource_edges() {
        let mut runtime = TlmcRuntime::new();
        let confirmation = runtime
            .apply_configure(TlmcConfigureReq {
                endpoint_id: Some(7),
                distribution_on_18th_frame: Some(Frame18Distribution { timeslot: 2 }),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(confirmation.endpoint_id, Some(7));

        assert!(runtime
            .resource_transition(
                7,
                LowerLayerResourceAvailability::Unavailable,
                LowerLayerResourceReason::LossOfRadioResources,
            )
            .is_some());
        assert!(runtime
            .resource_transition(
                7,
                LowerLayerResourceAvailability::Unavailable,
                LowerLayerResourceReason::LossOfRadioResources,
            )
            .is_none());
    }

    #[test]
    fn scan_and_selection_have_correlated_lifecycles() {
        let mut runtime = TlmcRuntime::new();
        runtime
            .begin_scan(TlmcScanReq {
                request_id: ScanRequestId(9),
                channel_number: RfChannelNumber(720),
                measurement_method: ScanningMeasurementMethod::NonInterrupting,
                characteristics: None,
                threshold_level: None,
                channel_classes: Vec::new(),
            })
            .unwrap();
        let scan = runtime
            .complete_scan(
                ScanRequestId(9),
                RfChannelNumber(720),
                MeasurementValue::dbm(-91),
                Layer2Report::Success,
                Vec::new(),
                None,
                CellServiceLevel::NormalService,
            )
            .unwrap();
        assert_eq!(scan.report, Layer2Report::Success);

        runtime
            .begin_select(TlmcSelectReq {
                channel_number: RfChannelNumber(720),
                channel_information: Some(channel_info()),
                threshold_level: Some(MeasurementValue::dbm(-91)),
                main_carrier_number: Some(RfChannelNumber(720)),
                main_carrier_information: Some(channel_info()),
                cause: SelectionCause::InitialCellSelection,
            })
            .unwrap();
        let selected = runtime
            .complete_select(
                RfChannelNumber(720),
                Some(MeasurementValue::dbm(-91)),
                Some(RfChannelNumber(720)),
                Some(Layer2Report::Success),
                SelectionResult::Success,
                Some(CellIdentity {
                    mcc: 262,
                    mnc: 1,
                    location_area: Some(1),
                    colour_code: Some(1),
                    main_carrier: 720,
                    cell_type: Default::default(),
                }),
            )
            .unwrap();
        assert_eq!(selected.result, SelectionResult::Success);
        assert!(runtime.current_cell().is_some());
    }

    #[test]
    fn monitor_requires_an_explicit_monitor_list() {
        let mut runtime = TlmcRuntime::new();
        assert!(matches!(
            runtime.record_monitor(
                RfChannelNumber(720),
                MeasurementValue::db(-5),
                None,
                Vec::new(),
            ),
            Err(TlmcRuntimeError::ChannelNotMonitored(_))
        ));

        runtime.set_monitor_list(TlmcMonitorListReq {
            channels: vec![TlmcMonitorChannel {
                channel_number: RfChannelNumber(720),
                characteristics: tetra_saps::common::RfChannelCharacteristics {
                    modulation: ModulationMode::PhaseModulation,
                    bandwidth: ChannelBandwidth::Khz25,
                    max_ms_tx_power_dbm: None,
                    min_rx_access_level_dbm: None,
                    discontinuous: None,
                    role: ChannelRole::NeighbourMainCarrier,
                    topology: ChannelTopology::Conforming,
                },
                channel_classes: Vec::new(),
            }],
        });
        assert!(runtime
            .record_monitor(
                RfChannelNumber(720),
                MeasurementValue::db(-5),
                Some(QualityIndication { raw: 12 }),
                Vec::new(),
            )
            .is_ok());
    }
}

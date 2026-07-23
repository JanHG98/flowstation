use tetra_core::{TdmaTime, tetra_entities::TetraEntity};
use tetra_entities::net_control::{
    ControlCommand, ControlResponse, ManagedCallKind, ManagedCallRestoreContextPayload,
};

#[test]
fn group_leg_command_roundtrips_through_json() {
    let command = ControlCommand::CallControlGroupStart {
        handle: 41,
        operation_id: "2de8f5f2-61bc-4563-97b1-0f913e0d18ef".to_string(),
        source_issi: 9999,
        gssi: 15502,
        priority: 7,
    };
    let encoded = serde_json::to_vec(&command).expect("serialize managed group call");
    let decoded: ControlCommand =
        serde_json::from_slice(&encoded).expect("deserialize managed group call");
    match decoded {
        ControlCommand::CallControlGroupStart {
            handle,
            source_issi,
            gssi,
            priority,
            ..
        } => {
            assert_eq!(handle, 41);
            assert_eq!(source_issi, 9999);
            assert_eq!(gssi, 15502);
            assert_eq!(priority, 7);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn restore_context_roundtrips_without_losing_floor_or_network_origin() {
    let context = ManagedCallRestoreContextPayload::Group {
        call_id: 101,
        dest_gssi: 15502,
        source_issi: 1234,
        floor_holder: Some(1234),
        priority: 8,
        call_timeout: 4,
        created_at: TdmaTime::default(),
        tx_active: true,
        communication_type: 0,
        circuit_mode_type: 0,
        speech_service: Some(0),
        etee_encrypted: false,
        origin_local_caller: None,
        network_entity: Some(TetraEntity::AudioPlayer),
        network_uuid: Some("2de8f5f2-61bc-4563-97b1-0f913e0d18ef".to_string()),
    };
    let response = ControlResponse::CallControlRestoreContextExported {
        handle: 42,
        call_id: 101,
        found: true,
        context: Some(context),
        message: "ok".to_string(),
    };
    let encoded = serde_json::to_vec(&response).expect("serialize restore response");
    let decoded: ControlResponse =
        serde_json::from_slice(&encoded).expect("deserialize restore response");
    match decoded {
        ControlResponse::CallControlRestoreContextExported {
            context:
                Some(ManagedCallRestoreContextPayload::Group {
                    floor_holder,
                    network_entity,
                    tx_active,
                    ..
                }),
            ..
        } => {
            assert_eq!(floor_holder, Some(1234));
            assert_eq!(network_entity, Some(TetraEntity::AudioPlayer));
            assert!(tx_active);
        }
        other => panic!("unexpected response: {other:?}"),
    }
}

#[test]
fn leg_started_response_carries_local_resource_identity() {
    let response = ControlResponse::CallControlLegStarted {
        handle: 43,
        operation_id: "2de8f5f2-61bc-4563-97b1-0f913e0d18ef".to_string(),
        kind: ManagedCallKind::Individual,
        success: true,
        call_id: Some(7),
        timeslot: Some(3),
        usage: Some(11),
        floor_holder: None,
        message: "active".to_string(),
    };
    let value = serde_json::to_value(response).expect("serialize response");
    assert_eq!(value["CallControlLegStarted"]["call_id"], 7);
    assert_eq!(value["CallControlLegStarted"]["timeslot"], 3);
}

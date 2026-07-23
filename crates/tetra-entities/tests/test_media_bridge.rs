use tetra_entities::net_media::{
    LocalMediaUplinkFrame, MediaCodec, MediaDownlinkFrame, MediaSendError,
    TETRA_ACELP_FRAME_BYTES, media_bridge_channel,
};

#[test]
fn bounded_media_bridge_moves_packed_frames_in_both_directions() {
    let (uplink_sink, uplink_source, downlink_sink, downlink_source) =
        media_bridge_channel(32);

    uplink_sink
        .try_send(LocalMediaUplinkFrame {
            sequence: 7,
            carrier_num: 720,
            logical_ts: 2,
            codec: MediaCodec::TetraAcelp0,
            payload: vec![0x55; TETRA_ACELP_FRAME_BYTES],
        })
        .expect("valid uplink frame accepted");
    let uplink = uplink_source.try_recv().expect("uplink frame delivered");
    assert_eq!(uplink.sequence, 7);
    assert_eq!(uplink.payload.len(), TETRA_ACELP_FRAME_BYTES);

    downlink_sink
        .try_send(MediaDownlinkFrame {
            session_id: "call-1".to_string(),
            source_node_id: "tbs-a".to_string(),
            sequence: 8,
            logical_ts: 3,
            codec: MediaCodec::TetraAcelp0,
            payload: vec![0xaa; TETRA_ACELP_FRAME_BYTES],
        })
        .expect("valid downlink frame accepted");
    let downlink = downlink_source
        .try_recv()
        .expect("downlink frame delivered");
    assert_eq!(downlink.logical_ts, 3);
    assert_eq!(downlink.payload.len(), TETRA_ACELP_FRAME_BYTES);
}

#[test]
fn media_bridge_rejects_wrong_frame_size_before_rf_runtime() {
    let (uplink_sink, _uplink_source, downlink_sink, _downlink_source) =
        media_bridge_channel(16);

    let uplink_error = uplink_sink
        .try_send(LocalMediaUplinkFrame {
            sequence: 1,
            carrier_num: 720,
            logical_ts: 2,
            codec: MediaCodec::TetraAcelp0,
            payload: vec![0; TETRA_ACELP_FRAME_BYTES - 1],
        })
        .expect_err("short uplink frame rejected");
    assert_eq!(uplink_error, MediaSendError::InvalidFrame);

    let downlink_error = downlink_sink
        .try_send(MediaDownlinkFrame {
            session_id: "call-1".to_string(),
            source_node_id: "injector".to_string(),
            sequence: 1,
            logical_ts: 2,
            codec: MediaCodec::TetraAcelp0,
            payload: vec![0; TETRA_ACELP_FRAME_BYTES + 1],
        })
        .expect_err("long downlink frame rejected");
    assert_eq!(downlink_error, MediaSendError::InvalidFrame);
}

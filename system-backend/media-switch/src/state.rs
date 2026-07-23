use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tetra_entities::net_media::{
    MediaCodec, MediaDownlinkFrame, MediaUplinkFrame, TETRA_ACELP_FRAME_BYTES,
};
use tetra_entities::net_control_room::NodeToControlRoomMessage;

use crate::config::MediaSwitchConfig;
use crate::protocol::{
    BackendEvent, BackendRequest, CallControlCall, GatewaySnapshot,
    media_frame_from_node_message,
};

#[derive(Debug, Clone, Serialize)]
pub struct MediaSwitchStatus {
    pub service: &'static str,
    pub started_at: String,
    pub security_mode: &'static str,
    pub warning: &'static str,
    pub node_gateway_connected: bool,
    pub node_gateway_last_error: Option<String>,
    pub call_control_connected: bool,
    pub call_control_last_error: Option<String>,
    pub nodes_connected: usize,
    pub nodes_media_capable: usize,
    pub sessions_total: usize,
    pub sessions_active: usize,
    pub streams_active: usize,
    pub pending_frames: usize,
    pub frames_received: u64,
    pub frames_routed: u64,
    pub frames_sent: u64,
    pub frames_injected: u64,
    pub frames_dropped: u64,
    pub duplicate_frames: u64,
    pub unknown_stream_frames: u64,
    pub muted_frames: u64,
    pub buffer_overflows: u64,
    pub send_failures: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeRecord {
    pub node_id: String,
    pub station_name: String,
    pub gateway_session_id: String,
    pub site: Option<String>,
    pub connected: bool,
    pub stale: bool,
    pub last_seen: String,
    pub media_bridge: bool,
    pub media_frame_count: u64,
    pub mcc: u16,
    pub mnc: u16,
    pub location_area: u16,
    pub colour_code: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaLeg {
    pub node_id: String,
    pub local_call_id: Option<u16>,
    pub phase: String,
    pub logical_ts: u8,
    pub carrier_num: Option<u16>,
    pub floor_holder: Option<u32>,
    pub restored: bool,
    pub muted: bool,
    pub rx_frames: u64,
    pub tx_frames: u64,
    pub dropped_frames: u64,
    pub last_sequence: Option<u64>,
    pub last_frame_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaSession {
    pub logical_call_id: String,
    pub kind: String,
    pub phase: String,
    pub priority: u8,
    pub emergency: bool,
    pub floor_holder: Option<u32>,
    pub legs: BTreeMap<String, MediaLeg>,
    pub created_at: String,
    pub updated_at: String,
    pub frames_received: u64,
    pub frames_routed: u64,
    pub frames_dropped: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct StreamSnapshot {
    pub session_id: String,
    pub stream_id: String,
    pub node_id: String,
    pub local_call_id: Option<u16>,
    pub logical_ts: u8,
    pub carrier_num: Option<u16>,
    pub phase: String,
    pub muted: bool,
    pub floor_holder: Option<u32>,
    pub rx_frames: u64,
    pub tx_frames: u64,
    pub dropped_frames: u64,
    pub last_sequence: Option<u64>,
    pub last_frame_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BufferSnapshot {
    pub session_id: String,
    pub target_node_id: String,
    pub target_logical_ts: u8,
    pub queued_frames: usize,
    pub oldest_due_in_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TapRecord {
    pub seq: u64,
    pub timestamp: String,
    pub session_id: String,
    pub source_node_id: String,
    pub source_logical_ts: u8,
    pub source_sequence: u64,
    pub target_count: usize,
    pub payload_bytes: usize,
    pub injected: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventRecord {
    pub seq: u64,
    pub timestamp: String,
    pub kind: String,
    pub session_id: Option<String>,
    pub node_id: Option<String>,
    pub detail: Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MuteInput {
    pub node_id: String,
    pub logical_ts: u8,
    pub muted: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InjectionInput {
    pub payload: Vec<u8>,
    pub target_node: Option<String>,
    pub target_logical_ts: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct StreamKey {
    session_id: String,
    target_node_id: String,
    target_logical_ts: u8,
}

#[derive(Debug, Clone)]
struct BufferedFrame {
    due_at: Instant,
    frame: MediaDownlinkFrame,
}

struct MediaState {
    config: MediaSwitchConfig,
    started_at: String,
    gateway_connected: bool,
    gateway_last_error: Option<String>,
    call_control_connected: bool,
    call_control_last_error: Option<String>,
    nodes: BTreeMap<String, NodeRecord>,
    sessions: BTreeMap<String, MediaSession>,
    route_index: HashMap<(String, u8), String>,
    buffers: HashMap<StreamKey, VecDeque<BufferedFrame>>,
    pending_frames: usize,
    taps: VecDeque<TapRecord>,
    events: VecDeque<EventRecord>,
    next_event_seq: u64,
    next_tap_seq: u64,
    injection_sequence: u64,
    frames_received: u64,
    frames_routed: u64,
    frames_sent: u64,
    frames_injected: u64,
    frames_dropped: u64,
    duplicate_frames: u64,
    unknown_stream_frames: u64,
    muted_frames: u64,
    buffer_overflows: u64,
    send_failures: u64,
}

#[derive(Clone)]
pub struct SharedMedia(Arc<Mutex<MediaState>>);

impl SharedMedia {
    pub fn new(config: MediaSwitchConfig) -> Self {
        Self(Arc::new(Mutex::new(MediaState {
            config,
            started_at: now_iso(),
            gateway_connected: false,
            gateway_last_error: None,
            call_control_connected: false,
            call_control_last_error: None,
            nodes: BTreeMap::new(),
            sessions: BTreeMap::new(),
            route_index: HashMap::new(),
            buffers: HashMap::new(),
            pending_frames: 0,
            taps: VecDeque::new(),
            events: VecDeque::new(),
            next_event_seq: 1,
            next_tap_seq: 1,
            injection_sequence: 0,
            frames_received: 0,
            frames_routed: 0,
            frames_sent: 0,
            frames_injected: 0,
            frames_dropped: 0,
            duplicate_frames: 0,
            unknown_stream_frames: 0,
            muted_frames: 0,
            buffer_overflows: 0,
            send_failures: 0,
        })))
    }

    pub fn status(&self) -> MediaSwitchStatus {
        let state = self.0.lock().expect("media state poisoned");
        status_locked(&state)
    }

    pub fn nodes(&self) -> Vec<NodeRecord> {
        let state = self.0.lock().expect("media state poisoned");
        state.nodes.values().cloned().collect()
    }

    pub fn sessions(&self) -> Vec<MediaSession> {
        let state = self.0.lock().expect("media state poisoned");
        state.sessions.values().cloned().collect()
    }

    pub fn session(&self, session_id: &str) -> Option<MediaSession> {
        let state = self.0.lock().expect("media state poisoned");
        state.sessions.get(session_id).cloned()
    }

    pub fn streams(&self) -> Vec<StreamSnapshot> {
        let state = self.0.lock().expect("media state poisoned");
        streams_locked(&state)
    }

    pub fn buffers(&self) -> Vec<BufferSnapshot> {
        let state = self.0.lock().expect("media state poisoned");
        let now = Instant::now();
        let mut buffers = state
            .buffers
            .iter()
            .filter_map(|(key, queue)| {
                queue.front().map(|frame| BufferSnapshot {
                    session_id: key.session_id.clone(),
                    target_node_id: key.target_node_id.clone(),
                    target_logical_ts: key.target_logical_ts,
                    queued_frames: queue.len(),
                    oldest_due_in_ms: frame
                        .due_at
                        .checked_duration_since(now)
                        .map(|duration| duration.as_millis() as i64)
                        .unwrap_or(0),
                })
            })
            .collect::<Vec<_>>();
        buffers.sort_by(|a, b| {
            (&a.session_id, &a.target_node_id, a.target_logical_ts).cmp(&(
                &b.session_id,
                &b.target_node_id,
                b.target_logical_ts,
            ))
        });
        buffers
    }

    pub fn taps(&self, limit: usize) -> Vec<TapRecord> {
        let state = self.0.lock().expect("media state poisoned");
        state
            .taps
            .iter()
            .rev()
            .take(limit.min(state.taps.len()))
            .cloned()
            .collect()
    }

    pub fn events(&self, limit: usize) -> Vec<EventRecord> {
        let state = self.0.lock().expect("media state poisoned");
        state
            .events
            .iter()
            .rev()
            .take(limit.min(state.events.len()))
            .cloned()
            .collect()
    }

    pub fn config_view(&self) -> Value {
        let state = self.0.lock().expect("media state poisoned");
        json!({
            "server": &state.config.server,
            "node_gateway": &state.config.node_gateway,
            "call_control": &state.config.call_control,
            "media": &state.config.media,
            "security": {
                "mode": &state.config.security.mode,
                "allow_remote_management": state.config.security.allow_remote_management,
                "token_auth": false,
                "tls": false
            },
            "limits": &state.config.limits
        })
    }

    pub fn gateway_connected(&self) {
        let mut state = self.0.lock().expect("media state poisoned");
        state.gateway_connected = true;
        state.gateway_last_error = None;
        push_event_locked(
            &mut state,
            "node_gateway_connected",
            None,
            None,
            json!({}),
        );
    }

    pub fn gateway_disconnected(&self, error: String) {
        let mut state = self.0.lock().expect("media state poisoned");
        let changed = state.gateway_connected
            || state.gateway_last_error.as_deref() != Some(error.as_str());
        state.gateway_connected = false;
        state.gateway_last_error = Some(error.clone());
        if changed {
            push_event_locked(
                &mut state,
                "node_gateway_disconnected",
                None,
                None,
                json!({"error": error}),
            );
        }
    }

    pub fn call_control_failed(&self, error: String) {
        let mut state = self.0.lock().expect("media state poisoned");
        let changed = state.call_control_connected
            || state.call_control_last_error.as_deref() != Some(error.as_str());
        state.call_control_connected = false;
        state.call_control_last_error = Some(error.clone());
        if changed {
            push_event_locked(
                &mut state,
                "call_control_unavailable",
                None,
                None,
                json!({"error": error}),
            );
        }
    }

    pub fn handle_backend_event(&self, event: BackendEvent) {
        match event {
            BackendEvent::Snapshot { snapshot } => self.apply_gateway_snapshot(snapshot),
            BackendEvent::NodeMessage { node_id, message } => {
                if let Some(frame) = media_frame_from_node_message(&node_id, &message) {
                    self.route_uplink(frame);
                } else if matches!(message, NodeToControlRoomMessage::MediaFrame { .. }) {
                    let mut state = self.0.lock().expect("media state poisoned");
                    state.frames_dropped = state.frames_dropped.wrapping_add(1);
                    push_event_locked(
                        &mut state,
                        "node_id_mismatch",
                        None,
                        Some(node_id),
                        json!({"message":"media frame node_id does not match gateway envelope"}),
                    );
                }
            }
            BackendEvent::ActionResult {
                ok,
                message,
                request_id,
                command_id,
            } => {
                if !ok {
                    let mut state = self.0.lock().expect("media state poisoned");
                    state.send_failures = state.send_failures.wrapping_add(1);
                    state.frames_dropped = state.frames_dropped.wrapping_add(1);
                    push_event_locked(
                        &mut state,
                        "gateway_media_send_failed",
                        None,
                        None,
                        json!({
                            "message": message,
                            "request_id": request_id,
                            "command_id": command_id
                        }),
                    );
                }
            }
            BackendEvent::Event { .. } => {}
        }
    }

    pub fn reconcile_calls(&self, calls: Vec<CallControlCall>) {
        let mut state = self.0.lock().expect("media state poisoned");
        state.call_control_connected = true;
        state.call_control_last_error = None;

        let old_sessions = std::mem::take(&mut state.sessions);
        let mut sessions = BTreeMap::new();
        let max_sessions = state.config.limits.max_sessions;
        let max_streams = state.config.limits.max_streams;
        let mut stream_count = 0usize;

        for call in calls.into_iter().filter(is_routable_call).take(max_sessions) {
            let previous = old_sessions.get(&call.logical_call_id);
            let created_at = previous
                .map(|session| session.created_at.clone())
                .unwrap_or_else(now_iso);
            let mut legs = BTreeMap::new();

            for leg in call.legs.values().filter(|leg| is_routable_leg(&leg.phase)) {
                let Some(logical_ts) = leg.timeslot.filter(|ts| (1..=7).contains(ts)) else {
                    continue;
                };
                if stream_count >= max_streams {
                    break;
                }
                let stream_id = stream_id(&leg.node_id, logical_ts);
                let old_leg = previous.and_then(|session| session.legs.get(&stream_id));
                legs.insert(
                    stream_id,
                    MediaLeg {
                        node_id: leg.node_id.clone(),
                        local_call_id: leg.local_call_id,
                        phase: leg.phase.clone(),
                        logical_ts,
                        carrier_num: leg.carrier_num,
                        floor_holder: leg.floor_holder,
                        restored: leg.restored,
                        muted: old_leg.is_some_and(|old| old.muted),
                        rx_frames: old_leg.map_or(0, |old| old.rx_frames),
                        tx_frames: old_leg.map_or(0, |old| old.tx_frames),
                        dropped_frames: old_leg.map_or(0, |old| old.dropped_frames),
                        last_sequence: old_leg.and_then(|old| old.last_sequence),
                        last_frame_at: old_leg.and_then(|old| old.last_frame_at.clone()),
                    },
                );
                stream_count += 1;
            }

            if legs.is_empty() {
                continue;
            }

            sessions.insert(
                call.logical_call_id.clone(),
                MediaSession {
                    logical_call_id: call.logical_call_id,
                    kind: call.kind,
                    phase: call.phase,
                    priority: call.priority,
                    emergency: call.emergency,
                    floor_holder: call.floor_holder,
                    legs,
                    created_at,
                    updated_at: now_iso(),
                    frames_received: previous.map_or(0, |session| session.frames_received),
                    frames_routed: previous.map_or(0, |session| session.frames_routed),
                    frames_dropped: previous.map_or(0, |session| session.frames_dropped),
                },
            );
        }

        let removed = old_sessions
            .keys()
            .filter(|session_id| !sessions.contains_key(*session_id))
            .cloned()
            .collect::<Vec<_>>();
        state.sessions = sessions;
        rebuild_route_index_locked(&mut state);
        prune_buffers_locked(&mut state);
        for session_id in &removed {
            flush_session_locked(&mut state, session_id);
        }
        if !removed.is_empty() {
            push_event_locked(
                &mut state,
                "media_sessions_removed",
                None,
                None,
                json!({"sessions": removed}),
            );
        }
    }

    pub fn drain_due_frames(&self) -> Vec<BackendRequest> {
        let mut state = self.0.lock().expect("media state poisoned");
        let now = Instant::now();
        let max_frames = state.config.media.max_frames_per_tick;
        let keys = state.buffers.keys().cloned().collect::<Vec<_>>();
        let mut output = Vec::new();
        let mut empty_keys = Vec::new();

        for key in keys {
            while output.len() < max_frames {
                let frame = state
                    .buffers
                    .get_mut(&key)
                    .and_then(|queue| {
                        if queue.front().is_some_and(|front| front.due_at <= now) {
                            queue.pop_front()
                        } else {
                            None
                        }
                    });
                let Some(buffered) = frame else {
                    break;
                };
                state.pending_frames = state.pending_frames.saturating_sub(1);
                state.frames_sent = state.frames_sent.wrapping_add(1);
                if let Some(session) = state.sessions.get_mut(&key.session_id) {
                    if let Some(leg) = session
                        .legs
                        .get_mut(&stream_id(&key.target_node_id, key.target_logical_ts))
                    {
                        leg.tx_frames = leg.tx_frames.wrapping_add(1);
                        leg.last_frame_at = Some(now_iso());
                    }
                }
                output.push(BackendRequest::MediaFrame {
                    node_id: key.target_node_id.clone(),
                    frame: buffered.frame,
                });
            }
            if state.buffers.get(&key).is_some_and(VecDeque::is_empty) {
                empty_keys.push(key);
            }
            if output.len() >= max_frames {
                break;
            }
        }

        for key in empty_keys {
            state.buffers.remove(&key);
        }
        output
    }

    pub fn mute_stream(&self, session_id: &str, input: MuteInput) -> Result<(), String> {
        let mut state = self.0.lock().expect("media state poisoned");
        if !state.config.security.allow_remote_management {
            return Err("remote management is disabled by configuration".to_string());
        }
        if !(1..=7).contains(&input.logical_ts) {
            return Err("logical_ts must be in 1..=7".to_string());
        }
        let key = stream_id(&input.node_id, input.logical_ts);
        let session = state
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| "media session not found".to_string())?;
        let leg = session
            .legs
            .get_mut(&key)
            .ok_or_else(|| "media stream not found".to_string())?;
        leg.muted = input.muted;
        let muted = leg.muted;
        push_event_locked(
            &mut state,
            if muted { "stream_muted" } else { "stream_unmuted" },
            Some(session_id.to_string()),
            Some(input.node_id),
            json!({"logical_ts": input.logical_ts}),
        );
        Ok(())
    }

    pub fn flush_session(&self, session_id: &str) -> Result<usize, String> {
        let mut state = self.0.lock().expect("media state poisoned");
        if !state.config.security.allow_remote_management {
            return Err("remote management is disabled by configuration".to_string());
        }
        if !state.sessions.contains_key(session_id) {
            return Err("media session not found".to_string());
        }
        let removed = flush_session_locked(&mut state, session_id);
        push_event_locked(
            &mut state,
            "session_buffer_flushed",
            Some(session_id.to_string()),
            None,
            json!({"frames": removed}),
        );
        Ok(removed)
    }

    pub fn inject(&self, session_id: &str, input: InjectionInput) -> Result<usize, String> {
        let mut state = self.0.lock().expect("media state poisoned");
        if !state.config.security.allow_remote_management {
            return Err("remote management is disabled by configuration".to_string());
        }
        if input.payload.len() != TETRA_ACELP_FRAME_BYTES {
            return Err(format!(
                "payload must contain exactly {TETRA_ACELP_FRAME_BYTES} packed bytes"
            ));
        }
        if input
            .target_logical_ts
            .is_some_and(|logical_ts| !(1..=7).contains(&logical_ts))
        {
            return Err("target_logical_ts must be in 1..=7".to_string());
        }

        let targets = state
            .sessions
            .get(session_id)
            .ok_or_else(|| "media session not found".to_string())?
            .legs
            .values()
            .filter(|leg| !leg.muted)
            .filter(|leg| node_can_receive(&state, &leg.node_id))
            .filter(|leg| {
                input
                    .target_node
                    .as_ref()
                    .is_none_or(|node| node == &leg.node_id)
            })
            .filter(|leg| {
                input
                    .target_logical_ts
                    .is_none_or(|logical_ts| logical_ts == leg.logical_ts)
            })
            .map(|leg| (leg.node_id.clone(), leg.logical_ts))
            .collect::<Vec<_>>();

        if targets.is_empty() {
            return Err("no matching active destination stream".to_string());
        }

        state.injection_sequence = state.injection_sequence.wrapping_add(1);
        let sequence = state.injection_sequence;
        let due_at = Instant::now() + jitter_delay(&state.config);
        let mut queued = 0usize;
        for (node_id, logical_ts) in targets {
            let frame = MediaDownlinkFrame {
                session_id: session_id.to_string(),
                source_node_id: "media-switch:injection".to_string(),
                sequence,
                logical_ts,
                codec: MediaCodec::TetraAcelp0,
                payload: input.payload.clone(),
            };
            if enqueue_locked(
                &mut state,
                StreamKey {
                    session_id: session_id.to_string(),
                    target_node_id: node_id,
                    target_logical_ts: logical_ts,
                },
                BufferedFrame { due_at, frame },
            ) {
                queued += 1;
            }
        }
        state.frames_injected = state.frames_injected.wrapping_add(1);
        state.frames_routed = state.frames_routed.wrapping_add(queued as u64);
        push_tap_locked(
            &mut state,
            session_id,
            "media-switch:injection",
            0,
            sequence,
            queued,
            input.payload.len(),
            true,
        );
        push_event_locked(
            &mut state,
            "media_injected",
            Some(session_id.to_string()),
            None,
            json!({"sequence": sequence, "targets": queued}),
        );
        Ok(queued)
    }

    pub fn metrics(&self) -> String {
        let status = self.status();
        format!(
            concat!(
                "# HELP netcore_media_switch_up Service liveness.\n",
                "# TYPE netcore_media_switch_up gauge\n",
                "netcore_media_switch_up 1\n",
                "# TYPE netcore_media_switch_sessions gauge\n",
                "netcore_media_switch_sessions {}\n",
                "# TYPE netcore_media_switch_streams gauge\n",
                "netcore_media_switch_streams {}\n",
                "# TYPE netcore_media_switch_pending_frames gauge\n",
                "netcore_media_switch_pending_frames {}\n",
                "# TYPE netcore_media_switch_frames_received counter\n",
                "netcore_media_switch_frames_received {}\n",
                "# TYPE netcore_media_switch_frames_routed counter\n",
                "netcore_media_switch_frames_routed {}\n",
                "# TYPE netcore_media_switch_frames_sent counter\n",
                "netcore_media_switch_frames_sent {}\n",
                "# TYPE netcore_media_switch_frames_dropped counter\n",
                "netcore_media_switch_frames_dropped {}\n",
                "# TYPE netcore_media_switch_duplicate_frames counter\n",
                "netcore_media_switch_duplicate_frames {}\n"
            ),
            status.sessions_active,
            status.streams_active,
            status.pending_frames,
            status.frames_received,
            status.frames_routed,
            status.frames_sent,
            status.frames_dropped,
            status.duplicate_frames,
        )
    }

    fn apply_gateway_snapshot(&self, snapshot: GatewaySnapshot) {
        let mut state = self.0.lock().expect("media state poisoned");
        state.gateway_connected = true;
        state.gateway_last_error = None;
        let previous_sessions = state
            .nodes
            .iter()
            .map(|(node_id, node)| (node_id.clone(), node.gateway_session_id.clone()))
            .collect::<HashMap<_, _>>();
        let nodes = snapshot
            .nodes
            .into_iter()
            .map(|node| {
                let record = NodeRecord {
                    node_id: node.node_id.clone(),
                    station_name: node.identity.station_name,
                    gateway_session_id: node.session_id,
                    site: node.identity.site,
                    connected: node.connected,
                    stale: node.stale,
                    last_seen: node.last_seen,
                    media_bridge: node.capabilities.media_bridge,
                    media_frame_count: node.media_frame_count,
                    mcc: node.identity.mcc,
                    mnc: node.identity.mnc,
                    location_area: node.identity.location_area,
                    colour_code: node.identity.colour_code,
                };
                (node.node_id, record)
            })
            .collect::<BTreeMap<_, _>>();
        let restarted_nodes = nodes
            .iter()
            .filter_map(|(node_id, node)| {
                previous_sessions
                    .get(node_id)
                    .filter(|previous| *previous != &node.gateway_session_id)
                    .map(|_| node_id.clone())
            })
            .collect::<Vec<_>>();
        state.nodes = nodes;
        for node_id in restarted_nodes {
            for session in state.sessions.values_mut() {
                for leg in session.legs.values_mut().filter(|leg| leg.node_id == node_id) {
                    leg.last_sequence = None;
                }
            }
            push_event_locked(
                &mut state,
                "node_media_sequence_reset",
                None,
                Some(node_id),
                json!({"reason":"new Node Gateway session"}),
            );
        }
    }

    fn route_uplink(&self, frame: MediaUplinkFrame) {
        let mut state = self.0.lock().expect("media state poisoned");
        state.frames_received = state.frames_received.wrapping_add(1);

        if frame.codec != MediaCodec::TetraAcelp0
            || frame.payload.len() != TETRA_ACELP_FRAME_BYTES
            || !(1..=7).contains(&frame.logical_ts)
        {
            state.frames_dropped = state.frames_dropped.wrapping_add(1);
            push_event_locked(
                &mut state,
                "invalid_uplink_media",
                None,
                Some(frame.node_id),
                json!({
                    "logical_ts": frame.logical_ts,
                    "payload_bytes": frame.payload.len()
                }),
            );
            return;
        }

        let Some(session_id) = state
            .route_index
            .get(&(frame.node_id.clone(), frame.logical_ts))
            .cloned()
        else {
            state.unknown_stream_frames = state.unknown_stream_frames.wrapping_add(1);
            state.frames_dropped = state.frames_dropped.wrapping_add(1);
            return;
        };

        let source_stream_id = stream_id(&frame.node_id, frame.logical_ts);
        let duplicate = state
            .sessions
            .get(&session_id)
            .and_then(|session| session.legs.get(&source_stream_id))
            .and_then(|leg| leg.last_sequence)
            .is_some_and(|last| frame.sequence <= last);
        if duplicate {
            state.duplicate_frames = state.duplicate_frames.wrapping_add(1);
            state.frames_dropped = state.frames_dropped.wrapping_add(1);
            if let Some(session) = state.sessions.get_mut(&session_id) {
                session.frames_dropped = session.frames_dropped.wrapping_add(1);
                if let Some(leg) = session.legs.get_mut(&source_stream_id) {
                    leg.dropped_frames = leg.dropped_frames.wrapping_add(1);
                }
            }
            return;
        }

        let targets = state
            .sessions
            .get(&session_id)
            .map(|session| {
                session
                    .legs
                    .values()
                    .filter(|leg| is_routable_leg(&leg.phase))
                    .filter(|leg| {
                        state.config.media.allow_same_leg_loopback
                            || leg.node_id != frame.node_id
                            || leg.logical_ts != frame.logical_ts
                    })
                    .map(|leg| {
                        (
                            leg.node_id.clone(),
                            leg.logical_ts,
                            leg.muted,
                            node_can_receive(&state, &leg.node_id),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        if let Some(session) = state.sessions.get_mut(&session_id) {
            session.frames_received = session.frames_received.wrapping_add(1);
            session.updated_at = now_iso();
            if let Some(leg) = session.legs.get_mut(&source_stream_id) {
                leg.rx_frames = leg.rx_frames.wrapping_add(1);
                leg.last_sequence = Some(frame.sequence);
                leg.last_frame_at = Some(now_iso());
            }
        }

        let due_at = Instant::now() + jitter_delay(&state.config);
        let mut routed = 0usize;
        for (node_id, logical_ts, muted, online) in targets {
            if muted {
                state.muted_frames = state.muted_frames.wrapping_add(1);
                continue;
            }
            if !online {
                state.frames_dropped = state.frames_dropped.wrapping_add(1);
                if let Some(session) = state.sessions.get_mut(&session_id) {
                    session.frames_dropped = session.frames_dropped.wrapping_add(1);
                    if let Some(leg) = session.legs.get_mut(&stream_id(&node_id, logical_ts)) {
                        leg.dropped_frames = leg.dropped_frames.wrapping_add(1);
                    }
                }
                continue;
            }
            let downlink = MediaDownlinkFrame {
                session_id: session_id.clone(),
                source_node_id: frame.node_id.clone(),
                sequence: frame.sequence,
                logical_ts,
                codec: frame.codec,
                payload: frame.payload.clone(),
            };
            if enqueue_locked(
                &mut state,
                StreamKey {
                    session_id: session_id.clone(),
                    target_node_id: node_id,
                    target_logical_ts: logical_ts,
                },
                BufferedFrame {
                    due_at,
                    frame: downlink,
                },
            ) {
                routed += 1;
            }
        }

        state.frames_routed = state.frames_routed.wrapping_add(routed as u64);
        if let Some(session) = state.sessions.get_mut(&session_id) {
            session.frames_routed = session.frames_routed.wrapping_add(routed as u64);
        }
        push_tap_locked(
            &mut state,
            &session_id,
            &frame.node_id,
            frame.logical_ts,
            frame.sequence,
            routed,
            frame.payload.len(),
            false,
        );
    }
}

fn status_locked(state: &MediaState) -> MediaSwitchStatus {
    MediaSwitchStatus {
        service: "netcore-media-switch",
        started_at: state.started_at.clone(),
        security_mode: "open_lab",
        warning: "OPEN LAB: no authentication, no tokens and no TLS; isolated test network only",
        node_gateway_connected: state.gateway_connected,
        node_gateway_last_error: state.gateway_last_error.clone(),
        call_control_connected: state.call_control_connected,
        call_control_last_error: state.call_control_last_error.clone(),
        nodes_connected: state
            .nodes
            .values()
            .filter(|node| node.connected && !node.stale)
            .count(),
        nodes_media_capable: state
            .nodes
            .values()
            .filter(|node| node.connected && !node.stale && node.media_bridge)
            .count(),
        sessions_total: state.sessions.len(),
        sessions_active: state.sessions.values().filter(|session| is_routable_call_phase(&session.phase)).count(),
        streams_active: state.sessions.values().map(|session| session.legs.len()).sum(),
        pending_frames: state.pending_frames,
        frames_received: state.frames_received,
        frames_routed: state.frames_routed,
        frames_sent: state.frames_sent,
        frames_injected: state.frames_injected,
        frames_dropped: state.frames_dropped,
        duplicate_frames: state.duplicate_frames,
        unknown_stream_frames: state.unknown_stream_frames,
        muted_frames: state.muted_frames,
        buffer_overflows: state.buffer_overflows,
        send_failures: state.send_failures,
    }
}

fn streams_locked(state: &MediaState) -> Vec<StreamSnapshot> {
    state
        .sessions
        .values()
        .flat_map(|session| {
            session.legs.iter().map(|(stream_id, leg)| StreamSnapshot {
                session_id: session.logical_call_id.clone(),
                stream_id: stream_id.clone(),
                node_id: leg.node_id.clone(),
                local_call_id: leg.local_call_id,
                logical_ts: leg.logical_ts,
                carrier_num: leg.carrier_num,
                phase: leg.phase.clone(),
                muted: leg.muted,
                floor_holder: leg.floor_holder,
                rx_frames: leg.rx_frames,
                tx_frames: leg.tx_frames,
                dropped_frames: leg.dropped_frames,
                last_sequence: leg.last_sequence,
                last_frame_at: leg.last_frame_at.clone(),
            })
        })
        .collect()
}

fn rebuild_route_index_locked(state: &mut MediaState) {
    state.route_index.clear();
    let mut collisions = Vec::new();
    for session in state.sessions.values() {
        for leg in session.legs.values() {
            let key = (leg.node_id.clone(), leg.logical_ts);
            if let Some(previous) = state
                .route_index
                .insert(key.clone(), session.logical_call_id.clone())
            {
                collisions.push((key, previous, session.logical_call_id.clone()));
            }
        }
    }
    for ((node_id, logical_ts), previous, replacement) in collisions {
        push_event_locked(
            state,
            "route_collision",
            Some(replacement.clone()),
            Some(node_id),
            json!({
                "logical_ts": logical_ts,
                "previous_session": previous,
                "selected_session": replacement
            }),
        );
    }
}

fn enqueue_locked(state: &mut MediaState, key: StreamKey, frame: BufferedFrame) -> bool {
    if state.pending_frames >= state.config.limits.max_pending_frames {
        state.buffer_overflows = state.buffer_overflows.wrapping_add(1);
        state.frames_dropped = state.frames_dropped.wrapping_add(1);
        return false;
    }

    let max_per_stream = state.config.media.max_jitter_buffer_frames;
    let overflowed = {
        let queue = state.buffers.entry(key.clone()).or_default();
        let overflowed = if queue.len() >= max_per_stream {
            queue.pop_front().is_some()
        } else {
            false
        };
        queue.push_back(frame);
        overflowed
    };

    if overflowed {
        state.pending_frames = state.pending_frames.saturating_sub(1);
        state.buffer_overflows = state.buffer_overflows.wrapping_add(1);
        state.frames_dropped = state.frames_dropped.wrapping_add(1);
        if let Some(session) = state.sessions.get_mut(&key.session_id) {
            session.frames_dropped = session.frames_dropped.wrapping_add(1);
            if let Some(leg) = session
                .legs
                .get_mut(&stream_id(&key.target_node_id, key.target_logical_ts))
            {
                leg.dropped_frames = leg.dropped_frames.wrapping_add(1);
            }
        }
    }
    state.pending_frames += 1;
    true
}

fn prune_buffers_locked(state: &mut MediaState) {
    let valid = state
        .sessions
        .values()
        .flat_map(|session| {
            session.legs.values().map(|leg| StreamKey {
                session_id: session.logical_call_id.clone(),
                target_node_id: leg.node_id.clone(),
                target_logical_ts: leg.logical_ts,
            })
        })
        .collect::<std::collections::HashSet<_>>();
    let stale = state
        .buffers
        .keys()
        .filter(|key| !valid.contains(*key))
        .cloned()
        .collect::<Vec<_>>();
    for key in stale {
        if let Some(queue) = state.buffers.remove(&key) {
            state.pending_frames = state.pending_frames.saturating_sub(queue.len());
        }
    }
}

fn flush_session_locked(state: &mut MediaState, session_id: &str) -> usize {
    let keys = state
        .buffers
        .keys()
        .filter(|key| key.session_id == session_id)
        .cloned()
        .collect::<Vec<_>>();
    let mut removed = 0usize;
    for key in keys {
        if let Some(queue) = state.buffers.remove(&key) {
            removed += queue.len();
        }
    }
    state.pending_frames = state.pending_frames.saturating_sub(removed);
    removed
}

fn push_tap_locked(
    state: &mut MediaState,
    session_id: &str,
    source_node_id: &str,
    source_logical_ts: u8,
    source_sequence: u64,
    target_count: usize,
    payload_bytes: usize,
    injected: bool,
) {
    let record = TapRecord {
        seq: state.next_tap_seq,
        timestamp: now_iso(),
        session_id: session_id.to_string(),
        source_node_id: source_node_id.to_string(),
        source_logical_ts,
        source_sequence,
        target_count,
        payload_bytes,
        injected,
    };
    state.next_tap_seq = state.next_tap_seq.wrapping_add(1);
    state.taps.push_back(record);
    while state.taps.len() > state.config.media.tap_history_frames {
        state.taps.pop_front();
    }
}

fn push_event_locked(
    state: &mut MediaState,
    kind: &str,
    session_id: Option<String>,
    node_id: Option<String>,
    detail: Value,
) {
    let event = EventRecord {
        seq: state.next_event_seq,
        timestamp: now_iso(),
        kind: kind.to_string(),
        session_id,
        node_id,
        detail,
    };
    state.next_event_seq = state.next_event_seq.wrapping_add(1);
    state.events.push_back(event);
    while state.events.len() > state.config.server.history_limit {
        state.events.pop_front();
    }
}

fn jitter_delay(config: &MediaSwitchConfig) -> Duration {
    Duration::from_millis(
        config
            .media
            .frame_duration_ms
            .saturating_mul(config.media.jitter_buffer_frames as u64),
    )
}

fn node_can_receive(state: &MediaState, node_id: &str) -> bool {
    state.nodes.get(node_id).is_some_and(|node| {
        node.connected && !node.stale && node.media_bridge
    })
}

fn stream_id(node_id: &str, logical_ts: u8) -> String {
    format!("{node_id}:ts{logical_ts}")
}

fn is_routable_call(call: &CallControlCall) -> bool {
    is_routable_call_phase(&call.phase)
}

fn is_routable_call_phase(phase: &str) -> bool {
    matches!(phase, "starting" | "partial" | "active" | "releasing")
}

fn is_routable_leg(phase: &str) -> bool {
    matches!(phase, "starting" | "active" | "releasing")
}

pub fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::MediaSwitchConfig;
    use crate::protocol::{CallControlCall, CallControlLeg};

    fn sample_call() -> CallControlCall {
        CallControlCall {
            logical_call_id: "call-1".to_string(),
            kind: "group".to_string(),
            phase: "active".to_string(),
            floor_holder: Some(1001),
            priority: 5,
            emergency: false,
            legs: BTreeMap::from([
                (
                    "a".to_string(),
                    CallControlLeg {
                        node_id: "tbs-a".to_string(),
                        local_call_id: Some(1),
                        phase: "active".to_string(),
                        timeslot: Some(2),
                        carrier_num: Some(720),
                        floor_holder: Some(1001),
                        restored: false,
                    },
                ),
                (
                    "b".to_string(),
                    CallControlLeg {
                        node_id: "tbs-b".to_string(),
                        local_call_id: Some(2),
                        phase: "active".to_string(),
                        timeslot: Some(3),
                        carrier_num: Some(721),
                        floor_holder: Some(1001),
                        restored: false,
                    },
                ),
            ]),
        }
    }

    #[test]
    fn reconciliation_creates_two_stream_session() {
        let media = SharedMedia::new(MediaSwitchConfig::default());
        media.reconcile_calls(vec![sample_call()]);
        assert_eq!(media.sessions().len(), 1);
        assert_eq!(media.streams().len(), 2);
    }

    #[test]
    fn mute_and_flush_are_operator_actions() {
        let media = SharedMedia::new(MediaSwitchConfig::default());
        media.reconcile_calls(vec![sample_call()]);
        media
            .mute_stream(
                "call-1",
                MuteInput {
                    node_id: "tbs-b".to_string(),
                    logical_ts: 3,
                    muted: true,
                },
            )
            .expect("mute succeeds");
        assert!(
            media
                .streams()
                .into_iter()
                .any(|stream| stream.node_id == "tbs-b" && stream.muted)
        );
        assert_eq!(media.flush_session("call-1").expect("flush succeeds"), 0);
    }

    #[test]
    fn uplink_routes_to_other_active_leg_and_rejects_duplicate() {
        let mut config = MediaSwitchConfig::default();
        config.media.jitter_buffer_frames = 0;
        let media = SharedMedia::new(config);
        {
            let mut state = media.0.lock().expect("media state");
            for (node_id, session_id) in [("tbs-a", "session-a"), ("tbs-b", "session-b")] {
                state.nodes.insert(
                    node_id.to_string(),
                    NodeRecord {
                        node_id: node_id.to_string(),
                        station_name: node_id.to_string(),
                        gateway_session_id: session_id.to_string(),
                        site: None,
                        connected: true,
                        stale: false,
                        last_seen: now_iso(),
                        media_bridge: true,
                        media_frame_count: 0,
                        mcc: 262,
                        mnc: 42,
                        location_area: 1,
                        colour_code: 1,
                    },
                );
            }
        }
        media.reconcile_calls(vec![sample_call()]);
        let frame = MediaUplinkFrame {
            node_id: "tbs-a".to_string(),
            sequence: 1,
            timestamp: now_iso(),
            carrier_num: 720,
            logical_ts: 2,
            codec: MediaCodec::TetraAcelp0,
            payload: vec![0x55; TETRA_ACELP_FRAME_BYTES],
        };
        media.route_uplink(frame.clone());
        let routed = media.drain_due_frames();
        assert_eq!(routed.len(), 1);
        assert!(matches!(
            &routed[0],
            BackendRequest::MediaFrame { node_id, frame }
                if node_id == "tbs-b" && frame.logical_ts == 3
        ));

        media.route_uplink(frame);
        assert_eq!(media.status().duplicate_frames, 1);
    }

    #[test]
    fn injection_requires_exact_packed_frame_size() {
        let media = SharedMedia::new(MediaSwitchConfig::default());
        media.reconcile_calls(vec![sample_call()]);
        let result = media.inject(
            "call-1",
            InjectionInput {
                payload: vec![0; TETRA_ACELP_FRAME_BYTES - 1],
                target_node: None,
                target_logical_ts: None,
            },
        );
        assert!(result.is_err());
    }
}

#!/usr/bin/env python3
from pathlib import Path
import re
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]
required = [
    'system-backend/media-switch/Cargo.toml',
    'system-backend/media-switch/src/main.rs',
    'system-backend/media-switch/src/config.rs',
    'system-backend/media-switch/src/protocol.rs',
    'system-backend/media-switch/src/state.rs',
    'system-backend/media-switch/src/gateway.rs',
    'system-backend/media-switch/src/call_control.rs',
    'system-backend/media-switch/src/http.rs',
    'system-backend/media-switch/config/media-switch.example.toml',
    'system-backend/media-switch/systemd/netcore-media-switch.service',
    'system-backend/media-switch/install/install.sh',
    'system-backend/media-switch/install/update.sh',
    'system-backend/media-switch/install/uninstall.sh',
    'system-backend/media-switch/README.md',
    'system-backend/media-switch/docs/open-lab-mode.md',
    'system-backend/media-switch/docs/media-routing.md',
    'system-backend/media-switch/docs/jitter-buffer.md',
    'system-backend/media-switch/docs/recorder-player-interfaces.md',
    'system-backend/media-switch/docs/lxc-deployment.md',
    'system-backend/media-switch/tests/open_lab_api_examples.md',
    'crates/tetra-entities/src/net_media/mod.rs',
    'crates/tetra-entities/tests/test_media_bridge.rs',
    'Docs/SWMI_CORE_1_PACKAGE_D_MEDIA_SWITCH.md',
    'Docs/SWMI_CORE_1_PACKAGE_D_APPLY.md',
    '.github/workflows/swmi-core-media-switch.yml',
]
missing = [item for item in required if not (ROOT / item).is_file()]
if missing:
    print('Missing files:', *missing, sep='\n  ')
    sys.exit(1)

workspace = (ROOT / 'Cargo.toml').read_text()
lock = (ROOT / 'Cargo.lock').read_text()
media_protocol = (ROOT / 'crates/tetra-entities/src/net_media/mod.rs').read_text()
room_protocol = (ROOT / 'crates/tetra-entities/src/net_control_room/protocol.rs').read_text()
room_worker = (ROOT / 'crates/tetra-entities/src/net_control_room/worker.rs').read_text()
umac = (ROOT / 'crates/tetra-entities/src/umac/umac_bs.rs').read_text()
bs_main = (ROOT / 'bins/bluestation-bs/src/main.rs').read_text()
gateway_state = (ROOT / 'system-backend/node-gateway/src/state.rs').read_text()
gateway_ws = (ROOT / 'system-backend/node-gateway/src/ws.rs').read_text()
state = (ROOT / 'system-backend/media-switch/src/state.rs').read_text()
gateway = (ROOT / 'system-backend/media-switch/src/gateway.rs').read_text()
call_control = (ROOT / 'system-backend/media-switch/src/call_control.rs').read_text()
http = (ROOT / 'system-backend/media-switch/src/http.rs').read_text()
service_files = [path for path in (ROOT / 'system-backend/media-switch').rglob('*') if path.is_file()]
service = '\n'.join(path.read_text(errors='ignore') for path in service_files)

with (ROOT / 'system-backend/services.toml').open('rb') as handle:
    manifest = tomllib.load(handle)
media_service = next(item for item in manifest['services'] if item['name'] == 'media-switch')

checks = {
    'workspace member': '"system-backend/media-switch"' in workspace,
    'workspace lock entry': 'name = "netcore-media-switch"' in lock,
    'packed media protocol': all(term in media_protocol for term in (
        'TETRA_ACELP_FRAME_BYTES', 'MediaUplinkFrame', 'MediaDownlinkFrame',
        'MediaCodec', 'media_bridge_channel', 'bounded(capacity)',
    )),
    'node capability': 'pub media_bridge: bool' in room_protocol and 'media_bridge:' in room_protocol,
    'node protocol messages': 'MediaFrame { frame: MediaUplinkFrame }' in room_protocol and 'MediaFrame { frame: MediaDownlinkFrame }' in room_protocol,
    'TBS worker media forwarding': all(term in room_worker for term in (
        'drain_media_uplink', 'media_uplink_source', 'media_downlink_sink',
        'NodeToControlRoomMessage::MediaFrame', 'ControlRoomToNodeMessage::MediaFrame',
    )),
    'UMAC media bridge': all(term in umac for term in (
        'set_media_bridge', 'forward_media_uplink', 'drain_media_downlink',
        'pack_ul_acelp_bits', 'dl_schedule_tmd', 'circuit_is_active',
    )),
    'base station channel wiring': 'media_bridge_channel(1_024)' in bs_main and 'control_room_media' in bs_main,
    'gateway media broadcast': all(term in gateway_state for term in (
        'media_frame_count', 'total_media_frames', 'send_media_frame',
        'MediaFrame {', 'set_backend_topics', 'media_frames: bool',
    )),
    'gateway high-rate request': all(term in gateway_ws for term in ('MediaFrame {', 'send_media_frame', 'BackendRequest::Subscribe')), 
    'logical session routing': all(term in state for term in (
        'struct MediaSession', 'struct MediaLeg', 'route_index', 'route_uplink',
        'reconcile_calls', 'node_can_receive',
    )),
    'bounded jitter buffer': all(term in state for term in (
        'struct BufferedFrame', 'jitter_delay', 'max_jitter_buffer_frames',
        'max_pending_frames', 'drain_due_frames', 'buffer_overflows',
    )),
    'duplicate and mute protection': all(term in state for term in (
        'duplicate_frames', 'unknown_stream_frames', 'muted_frames', 'mute_stream',
    )),
    'recorder and player preparation': 'push_tap_locked' in state and 'pub fn inject' in state and '/api/v1/taps' in http and '/inject' in http,
    'call-control reconciliation': 'GET {} HTTP/1.1' in call_control and 'reconcile_calls' in call_control,
    'gateway media loop': 'drain_due_frames' in gateway and 'Duration::from_millis(10)' in gateway and 'media_frames' in gateway,
    'WebUI and REST API': 'const INDEX_HTML' in http and '/api/v1/sessions' in http and '/api/v1/buffers' in http,
    'own management port': media_service.get('management_port') == 8130,
    'open lab only': media_service.get('security_mode') == 'open_lab' and media_service.get('token_auth') is False and media_service.get('tls') is False,
    'no credential fields': all(term not in service.lower() for term in (
        'api_token', 'auth_token', 'bearer_token', 'client_secret', 'password ='
    )),
    'no https/wss config': 'https://' not in service.lower() and 'wss://' not in service.lower(),
}

match = re.search(r'const INDEX_HTML: &str = r#"(.*)"#;\s*$', http, re.S)
checks['embedded WebUI raw string'] = bool(match and '<script>' in match.group(1) and '</script>' in match.group(1))

failed = [name for name, ok in checks.items() if not ok]
if failed:
    print('Media Switch checks failed:', *failed, sep='\n  ')
    sys.exit(1)

print('SWMI Core 1 Package D Media Switch checks passed.')
print('  deployable LXC service and own WebUI: present')
print('  TBS-to-TBS packed speech-frame bridge: present')
print('  Call Control session/leg reconciliation: present')
print('  bounded jitter buffers, mute and injection: present')
print('  recorder/player integration points: present')
print('  token/password/TLS fields: absent')

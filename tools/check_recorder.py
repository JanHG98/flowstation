#!/usr/bin/env python3
from pathlib import Path
import re
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]
required = [
    'system-backend/recorder/Cargo.toml',
    'system-backend/recorder/src/main.rs',
    'system-backend/recorder/src/config.rs',
    'system-backend/recorder/src/protocol.rs',
    'system-backend/recorder/src/media_switch.rs',
    'system-backend/recorder/src/state.rs',
    'system-backend/recorder/src/tar.rs',
    'system-backend/recorder/src/http.rs',
    'system-backend/recorder/config/recorder.example.toml',
    'system-backend/recorder/systemd/netcore-recorder.service',
    'system-backend/recorder/install/install.sh',
    'system-backend/recorder/install/update.sh',
    'system-backend/recorder/install/uninstall.sh',
    'system-backend/recorder/README.md',
    'system-backend/recorder/docs/open-lab-mode.md',
    'system-backend/recorder/docs/storage-format.md',
    'system-backend/recorder/docs/media-tap.md',
    'system-backend/recorder/docs/retention-integrity.md',
    'system-backend/recorder/docs/lxc-deployment.md',
    'system-backend/recorder/tests/open_lab_api_examples.md',
    'Docs/SWMI_CORE_1_PACKAGE_E_RECORDER.md',
    'Docs/SWMI_CORE_1_PACKAGE_E_APPLY.md',
    '.github/workflows/swmi-core-recorder.yml',
]
missing = [item for item in required if not (ROOT / item).is_file()]
if missing:
    print('Missing Recorder files:', *missing, sep='\n  ')
    sys.exit(1)

workspace = (ROOT / 'Cargo.toml').read_text()
lock = (ROOT / 'Cargo.lock').read_text()
config_rs = (ROOT / 'system-backend/recorder/src/config.rs').read_text()
protocol = (ROOT / 'system-backend/recorder/src/protocol.rs').read_text()
state = (ROOT / 'system-backend/recorder/src/state.rs').read_text()
http = (ROOT / 'system-backend/recorder/src/http.rs').read_text()
media_worker = (ROOT / 'system-backend/recorder/src/media_switch.rs').read_text()
tar = (ROOT / 'system-backend/recorder/src/tar.rs').read_text()
media_state = (ROOT / 'system-backend/media-switch/src/state.rs').read_text()
media_http = (ROOT / 'system-backend/media-switch/src/http.rs').read_text()
service_files = [
    path for path in (ROOT / 'system-backend/recorder').rglob('*')
    if path.is_file() and path.suffix in {'.rs', '.toml', '.service', '.sh'}
]
service_text = '\n'.join(path.read_text(errors='ignore') for path in service_files).lower()

with (ROOT / 'system-backend/services.toml').open('rb') as handle:
    manifest = tomllib.load(handle)
recorder_service = next(item for item in manifest['services'] if item['name'] == 'recorder')
with (ROOT / 'system-backend/recorder/config/recorder.example.toml').open('rb') as handle:
    example = tomllib.load(handle)

checks = {
    'workspace member': '"system-backend/recorder"' in workspace,
    'workspace lock entry': 'name = "netcore-recorder"' in lock,
    'own management port': recorder_service.get('management_port') == 8140 and example['server']['bind'].endswith(':8140'),
    'open lab only': recorder_service.get('security_mode') == 'open_lab' and recorder_service.get('token_auth') is False and recorder_service.get('tls') is False,
    'media switch dependency': recorder_service.get('depends_on') == ['media-switch'],
    'no credential fields': all(term not in service_text for term in ('api_token', 'auth_token', 'bearer_token', 'client_secret', 'password =')),
    'no encrypted listener': 'https://' not in service_text and 'wss://' not in service_text,
    'full-frame tap protocol': all(term in protocol for term in ('RecorderTapBatch', 'RecorderTapRecord', 'payload: Vec<u8>', 'speaker_issi')),
    'cursor polling': all(term in media_worker for term in ('after={cursor}', 'batch_limit', 'media_sequence_reset', 'ingest_batch')),
    'media switch full tap endpoint': '/api/v1/recorder/taps' in media_http and all(term in media_state for term in ('RecorderTapRecord', 'payload: payload.to_vec()', 'dropped_before', 'recorder_tap_history_frames')),
    'lossless packed archive': all(term in state for term in ('EXPECTED_TETRA_FRAME_BYTES: usize = 35', 'audio.tacelp.part', 'audio.tacelp', 'write_all(&tap.payload)', 'frames.jsonl')),
    'speaker and call metadata': all(term in state for term in ('SpeakerSegment', 'speaker_issi', 'source_issi', 'gssi', 'calling_issi', 'called_issi', 'emergency')),
    'integrity': all(term in state for term in ('Sha256', 'integrity.json', 'verify_recording', 'audio_sha256', 'index_sha256')),
    'crash recovery': all(term in state for term in ('metadata.active.json', 'recover_active_manifests', 'unclean_shutdown_recovery')),
    'retention and legal hold': all(term in state for term in ('run_retention_locked', 'retention_until', 'legal_hold', 'set_retention', 'set_hold')),
    'export archive': 'create_tar' in tar and 'export_recording' in state and '/export' in http,
    'WebUI and REST API': all(term in http for term in ('const INDEX_HTML', '/api/v1/active', '/api/v1/recordings', '/health/ready', '/metrics', '/openapi.json')),
    'storage guard': all(term in config_rs for term in ('minimum_free_space_mb', 'max_active_recordings', 'max_recordings')) and 'ensure_storage_space_locked' in state,
}

match = re.search(r'const INDEX_HTML: &str = r#"(.*)"#;\s*$', http, re.S)
checks['embedded WebUI raw string'] = bool(match and '<script>' in match.group(1) and '</script>' in match.group(1) and 'OPEN LAB' in match.group(1))

failed = [name for name, ok in checks.items() if not ok]
if failed:
    print('Recorder checks failed:', *failed, sep='\n  ')
    sys.exit(1)

print('SWMI Core 1 Package E Recorder checks passed.')
print('  deployable LXC service and own WebUI: present')
print('  replayable full-frame Media Switch tap: present')
print('  lossless TETRA ACELP archive and frame index: present')
print('  integrity, recovery, retention, hold and export: present')
print('  token/password/TLS fields: absent')

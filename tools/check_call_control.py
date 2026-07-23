#!/usr/bin/env python3
from pathlib import Path
import re
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]
required = [
    'system-backend/call-control/Cargo.toml',
    'system-backend/call-control/src/main.rs',
    'system-backend/call-control/src/config.rs',
    'system-backend/call-control/src/protocol.rs',
    'system-backend/call-control/src/state.rs',
    'system-backend/call-control/src/gateway.rs',
    'system-backend/call-control/src/http.rs',
    'system-backend/call-control/config/call-control.example.toml',
    'system-backend/call-control/systemd/netcore-call-control.service',
    'system-backend/call-control/install/install.sh',
    'system-backend/call-control/install/update.sh',
    'system-backend/call-control/install/uninstall.sh',
    'system-backend/call-control/README.md',
    'system-backend/call-control/docs/open-lab-mode.md',
    'system-backend/call-control/docs/call-model.md',
    'system-backend/call-control/docs/floor-control.md',
    'system-backend/call-control/docs/call-restore.md',
    'system-backend/call-control/docs/lxc-deployment.md',
    'crates/tetra-entities/src/cmce/subentities/cc_bs/control_plane.rs',
    'crates/tetra-entities/tests/test_managed_call_control_protocol.rs',
    'Docs/SWMI_CORE_1_PACKAGE_C_CALL_CONTROL.md',
    'Docs/SWMI_CORE_1_PACKAGE_C_APPLY.md',
    '.github/workflows/swmi-core-call-control.yml',
]
missing = [item for item in required if not (ROOT / item).is_file()]
if missing:
    print('Missing files:', *missing, sep='\n  ')
    sys.exit(1)

commands = (ROOT / 'crates/tetra-entities/src/net_control/commands.rs').read_text()
worker = (ROOT / 'crates/tetra-entities/src/net_control/worker.rs').read_text()
room_worker = (ROOT / 'crates/tetra-entities/src/net_control_room/worker.rs').read_text()
capabilities = (ROOT / 'crates/tetra-entities/src/net_control_room/protocol.rs').read_text()
cmce = (ROOT / 'crates/tetra-entities/src/cmce/cmce_bs.rs').read_text()
adapter = (ROOT / 'crates/tetra-entities/src/cmce/subentities/cc_bs/control_plane.rs').read_text()
restore = (ROOT / 'crates/tetra-entities/src/cmce/call_restore_runtime.rs').read_text()
service_files = [
    path for path in (ROOT / 'system-backend/call-control').rglob('*') if path.is_file()
]
service = '\n'.join(path.read_text(errors='ignore') for path in service_files)
state = (ROOT / 'system-backend/call-control/src/state.rs').read_text()
http = (ROOT / 'system-backend/call-control/src/http.rs').read_text()
lock = (ROOT / 'Cargo.lock').read_text()
workspace = (ROOT / 'Cargo.toml').read_text()

with (ROOT / 'system-backend/services.toml').open('rb') as handle:
    manifest = tomllib.load(handle)
call_service = next(item for item in manifest['services'] if item['name'] == 'call-control')

command_names = [
    'CallControlGroupStart',
    'CallControlIndividualStart',
    'CallControlRelease',
    'CallControlFloorRequest',
    'CallControlFloorRelease',
    'CallControlExportRestoreContext',
    'CallControlImportRestoreContext',
    'CallControlRemoveRestoreContext',
]
response_names = [
    'CallControlLegStarted',
    'CallControlLegReleased',
    'CallControlFloorChanged',
    'CallControlRestoreContextExported',
    'CallControlRestoreContextImported',
    'CallControlRestoreContextRemoved',
]
checks = {
    'workspace member': '"system-backend/call-control"' in workspace,
    'workspace lock entry': 'name = "netcore-call-control"' in lock,
    'all call-control commands': all(name in commands for name in command_names),
    'all call-control responses': all(name in commands for name in response_names),
    'CMCE worker routing': all(name in worker for name in command_names),
    'Node worker routing and response correlation': all(name in room_worker for name in command_names + response_names) and 'CallControl(u32)' in room_worker,
    'CMCE command handling': all(name in cmce for name in command_names),
    'local call-leg adapter': all(term in adapter for term in (
        'control_start_group_call', 'control_start_individual_call',
        'control_release_call', 'control_request_floor', 'control_release_floor',
    )),
    'local FSM reuse': all(term in adapter for term in (
        'fsm_on_network_call_start', 'fsm_on_network_circuit_setup_request',
        'fsm_on_u_tx_demand', 'fsm_on_u_tx_ceased',
    )),
    'restore payload conversion': 'to_managed_payload' in restore and 'from_managed_payload' in restore,
    'call capabilities': 'pub call_control: bool' in capabilities and 'pub call_restore_context: bool' in capabilities,
    'logical call and leg state': 'struct LogicalCall' in state and 'struct CallLeg' in state,
    'floor queue': 'floor_queue' in state and 'queued_issi' in state,
    'restore state machine': 'enum RestorePhase' in state and 'complete_matching_restore' in state,
    'target restore placeholder': 'restore context pending; awaiting target radio leg' in state,
    'restore context cleanup': all(term in state for term in (
        'CallControlRemoveRestoreContext', 'PendingAction::RemoveRestore',
        'restore_context_cleanup_failed', 'remove_pending_for_restore',
    )),
    'request command handle correlation': all(term in state for term in ('request_id', 'command_id', 'handle')),
    'telemetry discovery': 'observe_group_start' in state and 'observe_individual_start' in state,
    'persistent database': 'calls.json' in service and 'backup_path' in service,
    'WebUI and REST API': 'const INDEX_HTML' in http and '/api/v1/calls/group' in http and '/api/v1/restores' in http,
    'open lab only': call_service.get('security_mode') == 'open_lab' and call_service.get('token_auth') is False and call_service.get('tls') is False,
    'own management port': call_service.get('management_port') == 8120,
    'no credential fields': all(term not in service.lower() for term in (
        'api_token', 'auth_token', 'bearer_token', 'client_secret', 'password ='
    )),
    'no https/wss listener config': 'https://' not in service.lower() and 'wss://' not in service.lower(),
}

# Ensure the embedded UI contains one script block and no accidental raw-string terminator.
match = re.search(r'const INDEX_HTML: &str = r#"(.*)"#;\s*$', http, re.S)
checks['embedded WebUI raw string'] = bool(match and '<script>' in match.group(1) and '</script>' in match.group(1))

failed = [name for name, ok in checks.items() if not ok]
if failed:
    print('Call Control checks failed:', *failed, sep='\n  ')
    sys.exit(1)

print('SWMI Core 1 Package C Call Control checks passed.')
print('  deployable LXC service and own WebUI: present')
print('  logical calls and TBS call legs: present')
print('  local CMCE call/floor integration: present')
print('  correlated multi-cell call restore: present')
print('  token/password/TLS fields: absent')

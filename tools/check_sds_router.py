#!/usr/bin/env python3
from pathlib import Path
import re
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]
required = [
    'system-backend/sds-router/Cargo.toml',
    'system-backend/sds-router/src/main.rs',
    'system-backend/sds-router/src/config.rs',
    'system-backend/sds-router/src/protocol.rs',
    'system-backend/sds-router/src/gateway.rs',
    'system-backend/sds-router/src/state.rs',
    'system-backend/sds-router/src/http.rs',
    'system-backend/sds-router/config/sds-router.example.toml',
    'system-backend/sds-router/systemd/netcore-sds-router.service',
    'system-backend/sds-router/install/install.sh',
    'system-backend/sds-router/install/update.sh',
    'system-backend/sds-router/install/uninstall.sh',
    'system-backend/sds-router/README.md',
    'system-backend/sds-router/docs/architecture.md',
    'system-backend/sds-router/docs/queues-and-reports.md',
    'system-backend/sds-router/docs/application-routing.md',
    'system-backend/sds-router/docs/lxc-deployment.md',
    'system-backend/sds-router/docs/open-lab-mode.md',
    'system-backend/sds-router/tests/open_lab_api_examples.md',
    'Docs/SWMI_CORE_1_PACKAGE_F_SDS_ROUTER.md',
    'Docs/SWMI_CORE_1_PACKAGE_F_APPLY.md',
    '.github/workflows/swmi-core-sds-router.yml',
]
missing = [item for item in required if not (ROOT / item).is_file()]
if missing:
    print('Missing SDS Router files:', *missing, sep='\n  ')
    sys.exit(1)

workspace = (ROOT / 'Cargo.toml').read_text()
lock = (ROOT / 'Cargo.lock').read_text()
config_rs = (ROOT / 'system-backend/sds-router/src/config.rs').read_text()
state = (ROOT / 'system-backend/sds-router/src/state.rs').read_text()
http = (ROOT / 'system-backend/sds-router/src/http.rs').read_text()
gateway = (ROOT / 'system-backend/sds-router/src/gateway.rs').read_text()
commands = (ROOT / 'crates/tetra-entities/src/net_control/commands.rs').read_text()
events = (ROOT / 'crates/tetra-entities/src/net_telemetry/events.rs').read_text()
sds_edge = (ROOT / 'crates/tetra-entities/src/cmce/subentities/sds_bs.rs').read_text()
control_cfg = (ROOT / 'crates/tetra-config/src/bluestation/sec_control_room.rs').read_text()

service_files = [
    path for path in (ROOT / 'system-backend/sds-router').rglob('*')
    if path.is_file() and path.suffix in {'.rs', '.toml', '.service', '.sh'}
]
service_text = '\n'.join(path.read_text(errors='ignore') for path in service_files).lower()

with (ROOT / 'system-backend/services.toml').open('rb') as handle:
    manifest = tomllib.load(handle)
service = next(item for item in manifest['services'] if item['name'] == 'sds-router')
with (ROOT / 'system-backend/sds-router/config/sds-router.example.toml').open('rb') as handle:
    example = tomllib.load(handle)

checks = {
    'workspace member': '"system-backend/sds-router"' in workspace,
    'workspace lock entry': 'name = "netcore-sds-router"' in lock,
    'own management port': service.get('management_port') == 8150 and example['server']['bind'].endswith(':8150'),
    'open lab only': service.get('security_mode') == 'open_lab' and service.get('token_auth') is False and service.get('tls') is False,
    'node gateway dependency': service.get('depends_on') == ['node-gateway'],
    'no credential fields': all(term not in service_text for term in ('api_token', 'auth_token', 'bearer_token', 'client_secret', 'password =')),
    'no encrypted listener': 'https://' not in service_text and 'wss://' not in service_text,
    'lossless edge event': all(term in events for term in ('SdsEdgeIngress', 'sds_type: u8', 'protocol_id: u8', 'len_bits: u16', 'payload: Vec<u8>')),
    'central downlink commands': all(term in commands for term in ('DeliverSds {', 'SendStatus {', 'SdsDeliveryResponse {')),
    'opt-in TBS setting': 'central_sds_routing: bool' in control_cfg and 'central_sds_routing_enabled' in sds_edge,
    'TBS uplink handoff': all(term in sds_edge for term in ('emit_sds_edge_data', 'emit_sds_edge_status', 'SdsEdgeIngress')),
    'TBS downlink reconstruction': all(term in sds_edge for term in ('rx_sds_from_control', 'ControlCommand::DeliverSds', 'send_status_from_control')),
    'store and forward state': all(term in state for term in ('MessageState::Offline', 'MessageState::DeadLetter', 'refresh_offline_locked', 'expire_locked')),
    'retry and TTL': all(term in state for term in ('retry_delay_secs', 'max_attempts', 'expires_at', 'next_attempt_at')),
    'individual and group routing': all(term in state for term in ('RouteKind::Individual', 'RouteKind::Group', 'group_nodes', 'subscribers')),
    'protocol application routes': all(term in state for term in ('RouteKind::Protocol', 'ApplicationLeg', 'application_outbox', 'acknowledge_application')),
    'duplicate detection': all(term in state for term in ('fingerprint_message', 'dedupe_window_secs', 'message_duplicate')),
    'persistent database': all(term in state for term in ('SdsDatabase', 'persist_locked', 'backup_path', 'schema_version')),
    'gateway loop': all(term in gateway for term in ('BACKEND_PROTOCOL_VERSION', 'router.tick()', 'handle_backend_event')),
    'WebUI and REST API': all(term in http for term in ('const INDEX_HTML', '/api/v1/messages', '/api/v1/routes', '/api/v1/application-outbox', '/health/ready', '/metrics', '/openapi.json')),
    'config safety limits': all(term in config_rs for term in ('max_payload_bytes', 'max_messages', 'max_routes', 'max_body_bytes')),
}

match = re.search(r'const INDEX_HTML: &str = r#"(.*)"#;\s*$', http, re.S)
checks['embedded WebUI raw string'] = bool(
    match and '<script>' in match.group(1) and '</script>' in match.group(1) and 'OPEN LAB' in match.group(1)
)

failed = [name for name, ok in checks.items() if not ok]
if failed:
    print('SDS Router checks failed:', *failed, sep='\n  ')
    sys.exit(1)

print('SWMI Core 1 Package F SDS Router checks passed.')
print('  deployable LXC service and own WebUI: present')
print('  lossless SDS edge/core handoff: present')
print('  store-and-forward, TTL, retry and dead letter: present')
print('  individual, group and application routing: present')
print('  token/password/TLS fields: absent')

#!/usr/bin/env python3
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
required = [
    'system-backend/packet-core/Cargo.toml',
    'system-backend/packet-core/src/main.rs',
    'system-backend/packet-core/src/config.rs',
    'system-backend/packet-core/src/protocol.rs',
    'system-backend/packet-core/src/state.rs',
    'system-backend/packet-core/src/gateway.rs',
    'system-backend/packet-core/src/http.rs',
    'system-backend/packet-core/config/packet-core.example.toml',
    'system-backend/packet-core/systemd/netcore-packet-core.service',
    'system-backend/packet-core/install/install.sh',
    'Docs/SWMI_CORE_1_PACKAGE_G_PACKET_CORE.md',
]
markers = {
    'Cargo.toml': 'system-backend/packet-core',
    'system-backend/services.toml': 'management_port = 8160',
    'crates/tetra-entities/src/net_control/commands.rs': 'PacketDataActionResult',
    'crates/tetra-entities/src/net_control/worker.rs': 'TetraEntity::Sndcp',
    'crates/tetra-entities/src/net_control_room/worker.rs': 'PacketDataContextDeactivate',
    'crates/tetra-entities/src/sndcp/sndcp_bs.rs': 'process_control_commands',
    'bins/bluestation-bs/src/main.rs': 'sndcp.set_control(endpoint)',
    'system-backend/packet-core/src/protocol.rs': 'netcore-packet-edge-v1',
    'system-backend/packet-core/src/state.rs': 'ContextState::Ready',
    'system-backend/packet-core/src/http.rs': '/api/v1/edge/events',
}
errors=[]
for rel in required:
    if not (ROOT/rel).is_file(): errors.append(f'missing {rel}')
for rel, marker in markers.items():
    p=ROOT/rel
    if not p.is_file() or marker not in p.read_text(errors='replace'):
        errors.append(f'missing marker {marker!r} in {rel}')
if errors:
    print('\n'.join(errors), file=sys.stderr); sys.exit(1)
print('Packet Core static package check: OK')

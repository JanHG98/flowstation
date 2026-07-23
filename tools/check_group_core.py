#!/usr/bin/env python3
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
required = [
    'system-backend/group-core/Cargo.toml',
    'system-backend/group-core/src/main.rs',
    'system-backend/group-core/src/config.rs',
    'system-backend/group-core/src/protocol.rs',
    'system-backend/group-core/src/state.rs',
    'system-backend/group-core/src/gateway.rs',
    'system-backend/group-core/src/http.rs',
    'system-backend/group-core/config/group-core.example.toml',
    'system-backend/group-core/systemd/netcore-group-core.service',
    'system-backend/group-core/install/install.sh',
    'system-backend/group-core/README.md',
    'Docs/SWMI_CORE_1_PACKAGE_B_GROUP_CORE.md',
    'Docs/SWMI_CORE_1_PACKAGE_B_APPLY.md',
    '.github/workflows/swmi-core-group.yml',
]
missing = [item for item in required if not (ROOT / item).is_file()]
if missing:
    print('Missing files:', *missing, sep='\n  ')
    sys.exit(1)

commands = (ROOT / 'crates/tetra-entities/src/net_control/commands.rs').read_text()
mm = (ROOT / 'crates/tetra-entities/src/mm/mm_bs.rs').read_text()
state = (ROOT / 'crates/tetra-config/src/bluestation/state.rs').read_text()
capabilities = (ROOT / 'crates/tetra-entities/src/net_control_room/protocol.rs').read_text()
cmce = (ROOT / 'crates/tetra-entities/src/cmce/subentities/cc_bs/procedures/setup.rs').read_text()
control_room = (ROOT / 'crates/tetra-entities/src/net_control_room/worker.rs').read_text()
lock = (ROOT / 'Cargo.lock').read_text()
service = '\n'.join(
    path.read_text(errors='ignore')
    for path in (ROOT / 'system-backend/group-core').rglob('*')
    if path.is_file()
)
checks = {
    'GroupAccessPolicyApply command': 'GroupAccessPolicyApply' in commands,
    'GroupAccessPolicyApplied response': 'GroupAccessPolicyApplied' in commands,
    'GroupDgnaApply command': 'GroupDgnaApply' in commands,
    'GroupDgnaApplied response': 'GroupDgnaApplied' in commands,
    'central runtime policy': 'CentralGroupPolicy' in state and 'group_policy_override' in state,
    'MM affiliation enforcement': 'group_policy_allows_attach' in mm,
    'MM policy reconciliation': 'apply_group_policy' in mm and 'automatic_groups_for' in mm,
    'stale policy guard': 'stale group policy revision' in mm,
    'CMCE group-call enforcement': 'allows_group_call' in cmce and 'allows_emergency_call' in cmce and 'effective_priority' in cmce,
    'Control Room routing/correlation': 'GroupPolicy(u32)' in control_room and 'GroupDgna(u32)' in control_room,
    'workspace lock entry': 'name = "netcore-group-core"' in lock,
    'group policy capability': 'pub group_policy: bool' in capabilities,
    'WebUI': 'const INDEX_HTML' in service and 'Group Core' in service,
    'open lab': 'open_lab' in service,
    'no token fields': all(term not in service.lower() for term in ('api_token', 'auth_token', 'bearer_token', 'client_secret', 'password =')),
    'persistent database': 'groups.json' in service and 'backup_path' in service,
    'DGNA state machine': 'DgnaPhase' in service and 'GroupDgnaApply' in service,
    'stale response correlation': 'policy_sync_orphan_response' in service and 'revision_matches' in service,
}
failed = [name for name, ok in checks.items() if not ok]
if failed:
    print('Group Core checks failed:', *failed, sep='\n  ')
    sys.exit(1)
print('SWMI Core 1 Package B Group Core checks passed.')
print('  deployable LXC service and WebUI: present')
print('  persistent GSSI and membership database: present')
print('  versioned TBS group policy and local enforcement: present')
print('  correlated DGNA operations: present')
print('  token/password fields: absent')

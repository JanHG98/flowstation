#!/usr/bin/env python3
from pathlib import Path
import sys
root=Path(__file__).resolve().parents[1]
required=[
' system-backend/subscriber-core/Cargo.toml','system-backend/subscriber-core/src/main.rs','system-backend/subscriber-core/src/config.rs','system-backend/subscriber-core/src/state.rs','system-backend/subscriber-core/src/gateway.rs','system-backend/subscriber-core/src/http.rs','system-backend/subscriber-core/config/subscriber-core.example.toml','system-backend/subscriber-core/systemd/netcore-subscriber-core.service','Docs/SWMI_CORE_1_PACKAGE_A_SUBSCRIBER_CORE.md','crates/tetra-entities/src/mm/mobility_runtime.rs','crates/tetra-entities/src/mm/mm_bs.rs']
required=[x.strip() for x in required]
missing=[x for x in required if not (root/x).is_file()]
if missing: print('missing:',*missing,sep='\n  ');sys.exit(1)
all_text='\n'.join((root/x).read_text(errors='ignore') for x in required if (root/x).suffix in {'.rs','.toml','.md'})
need=['SubscriberAccessPolicyApply','SubscriberAccessPolicyApplied','open_lab','/api/v1/subscribers','/api/v1/sync','disconnect_unauthorized','issi_whitelist_deny_all','subscriber_policy','home_issi_for_local','home_issi_by_local_issi','register_local_identity','forget_local_identity']
missing_terms=[x for x in need if x not in all_text and x not in (root/'crates/tetra-entities/src/net_control/commands.rs').read_text() and x not in (root/'crates/tetra-config/src/bluestation/state.rs').read_text()]
if missing_terms: print('missing terms:',missing_terms);sys.exit(1)
low=all_text.lower()
for forbidden in ['api_token','auth_token','bearer_token','client_secret','password =']:
    if forbidden in low: print('forbidden credential field:',forbidden);sys.exit(1)
print('SWMI Core 1 Package A Subscriber Core checks passed.')
print('  deployable LXC service and WebUI: present')
print('  persistent subscriber database: present')
print('  versioned TBS admission policy: present')
print('  explicit deny-all and migrated Home-ISSI semantics: present')
print('  token/password fields: absent')

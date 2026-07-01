# Native Control Room UI

The native UI lives in:

```text
system-backend/control-room/ui
```

It is a standalone Cargo project and is built with `--manifest-path`. This avoids changing the main FlowStation workspace and lets operator workstations build the UI independently.

## Capabilities in v1

- Overview dashboard
- Node status including RF, carriers, health, subscriber/call counts
- Subscribers
- Groups
- Calls
- SDS
- Locations
- Command/audit log
- Kick
- DGNA attach/detach
- Emergency clear
- Admin token list/create/enable/disable/delete
- Raw JSON view for diagnostics

## Auth

The UI sends `Authorization: Bearer <token>` to the Control Room Core. Recommended setup is `token_file`, not inline `token`.

## Config paths

The UI looks for configs in this order unless `--config` is used:

- `$XDG_CONFIG_HOME/netcore/control-room/operator.toml` or platform config dir
- `/etc/netcore-control-room/operator.toml`

# NetCore Control Room

This directory contains the software and deployment assets that belong to the external Control Room / Leitstelle side of NetCore-Tetra.

The radio base station remains responsible for RF/TETRA runtime. The Control Room side runs outside of the TBS, typically in an LXC/VM, and native operator clients connect to it.

## Layout

```text
system-backend/control-room/
  config/       Example TOML config for the Control-Room-Core service
  docs/         LXC/deployment notes
  operator/     Native operator client / Leitstellenkonsole
  schema/       SQLite schema reference
  systemd/      Example systemd unit for the LXC service
```

## Runtime split

```text
TBS:
- bluestation-bs
- SDR / RF / Asterisk / codec runtime
- connects as node to the Control-Room-Core

Control-Room LXC:
- netcore-control-room
- State / API / telemetry aggregation / command audit / SQLite persistence
- no SDR hardware and no Soapy/GSM/TETRA codec dependencies needed

Operator workstation:
- netcore-control-room-operator
- native Leitstellenkonsole
- talks to the Control-Room-Core API
```

## Recommended LXC start

```bash
./target/release/netcore-control-room \
  --config /etc/netcore-control-room/control-room.toml
```

See [`docs/lxc-deployment.md`](docs/lxc-deployment.md) for the full LXC deployment flow.

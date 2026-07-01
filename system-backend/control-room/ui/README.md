# NetCore Control Room UI

Native desktop UI for NetCore Control Room.

This is intentionally not a web app. It is an egui/eframe desktop client that talks to the Control Room Core HTTP API.

## Build

```bash
cargo build --release --manifest-path system-backend/control-room/ui/Cargo.toml
```

## Run

```bash
./system-backend/control-room/ui/target/release/netcore-control-room-ui
```

## Config

The UI uses the same profile format as the operator CLI:

```toml
[profiles.default]
api = "http://10.0.1.25:9010"
default_node = "SRV-M_TBS-01"
operator_id = "jan"
token_file = "/home/jan/.config/netcore/control-room/operator.token"
```

Resolution order:

1. CLI args
2. environment variables
3. profile config
4. defaults

CLI args:

```bash
netcore-control-room-ui --api http://10.0.1.25:9010 --token-file ~/.config/netcore/control-room/operator.token
```

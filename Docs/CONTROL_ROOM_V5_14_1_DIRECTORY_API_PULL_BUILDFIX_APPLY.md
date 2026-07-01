# NetCore Control Room v5.14.1 – Directory-API Pull Buildfix

Dieses Paket behebt den Buildfehler:

```text
error[E0599]: no method named `len` found for enum `serde_json::Value`
```

## Ursache

`directory_resolved_from_value(...)` liefert ein `serde_json::Value`.
Darauf ist `.len()` nicht gültig.

## Fix

Alle betroffenen Zählungen nutzen jetzt:

```rust
resolved.as_object().map(|object| object.len()).unwrap_or(0)
```

## LXC Build

```bash
cd /opt/netcore/flowstation

systemctl stop netcore-control-room || true

cargo clean -p netcore-control-room
cargo build --release \
  -p netcore-control-room \
  -p netcore-control-room-operator

systemctl daemon-reload
systemctl start netcore-control-room
journalctl -u netcore-control-room -f
```

## Test

```bash
curl -u admin:DEIN_PASSWORT http://127.0.0.1:9010/api/directory/upstream | jq
curl -u admin:DEIN_PASSWORT http://127.0.0.1:9010/api/directory/resolved | jq
```

## Windows UI

```cmd
cargo build --release --manifest-path system-backend\control-room\ui\Cargo.toml
target\release\netcore-control-room-ui.exe --config "%APPDATA%\netcore\control-room\operator.toml" --profile default
```

Oben muss stehen:

```text
Native UI v5.14.1 · Directory-API Pull Buildfix
```

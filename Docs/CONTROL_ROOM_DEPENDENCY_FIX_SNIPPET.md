# NetCore Control Room dependency split

This patch makes `netcore-control-room` depend on `tetra-entities` in protocol-only mode:

```toml
tetra-entities = { workspace = true, default-features = false }
```

`tetra-entities` now has:

- `runtime` feature: full base-station entity stack, SDR, Brew, dashboard, EchoLink, network transports.
- default = `["runtime"]`: existing base-station builds keep current behavior.
- protocol-only mode: `default-features = false`, used by Control Room Core.
- no-op `asterisk` feature on `netcore-control-room`, so `cargo build -p netcore-control-room --features asterisk` does not pull native voice-codec dependencies.

Build Control Room in the LXC with:

```bash
cargo clean -p tetra-entities -p netcore-control-room
cargo build --release -p netcore-control-room
```

Optional accepted no-op:

```bash
cargo build --release -p netcore-control-room --features asterisk
```

Do not use plain workspace-wide `cargo build --release --features asterisk` on the LXC unless you also want to build the base-station binary and therefore install SDR/voice-codec libraries.

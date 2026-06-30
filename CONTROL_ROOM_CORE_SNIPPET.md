# Control Room Core - Startsnippet

```bash
cargo run --release -p netcore-control-room -- --bind 127.0.0.1:9010
```

Base-Station-Config:

```toml
[control_room]
enabled = true
host = "127.0.0.1"
port = 9010
use_tls = false
endpoint_path = "/node"

node_id = "tbs-04010001"
station_name = "NetCore TBS 04010001"
site = "Lab / Rack"
```

State prüfen:

```bash
curl http://127.0.0.1:9010/api/state | jq
```

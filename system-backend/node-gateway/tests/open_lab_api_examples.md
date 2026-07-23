# API-Beispiele im offenen Testmodus

Node-Liste:

```bash
curl http://127.0.0.1:8080/api/v1/nodes
```

Node anpingen:

```bash
curl -X POST http://127.0.0.1:8080/api/v1/nodes/tbs-test/ping
```

Node trennen:

```bash
curl -X POST http://127.0.0.1:8080/api/v1/nodes/tbs-test/disconnect
```

Beispielkommando:

```bash
curl -X POST http://127.0.0.1:8080/api/v1/nodes/tbs-test/commands \
  -H 'Content-Type: application/json' \
  -d '{"operator_id":"jan-test","command":{"KickMs":{"issi":1234567}}}'
```

Die genaue JSON-Darstellung der Rust-Enums entspricht der bestehenden Serde-Darstellung des `ControlCommand`-Typs.

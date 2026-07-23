# Open-Lab-API-Beispiele

```bash
curl http://127.0.0.1:8120/api/v1/status
```

Gruppenruf:

```bash
curl -X POST http://127.0.0.1:8120/api/v1/calls/group \
  -H 'Content-Type: application/json' \
  -d '{"gssi":15502,"source_issi":9999,"priority":3,"target_nodes":[]}'
```

Individualruf:

```bash
curl -X POST http://127.0.0.1:8120/api/v1/calls/individual \
  -H 'Content-Type: application/json' \
  -d '{"calling_issi":9999,"called_issi":1234,"simplex":true,"priority":3,"target_node":null}'
```

Alle Beispiele sind absichtlich ohne Authorization-Header. Das gilt ausschließlich für die isolierte Testumgebung.

# Open-Lab-API-Beispiele

```bash
curl http://127.0.0.1:8130/api/v1/status
curl http://127.0.0.1:8130/api/v1/sessions
curl http://127.0.0.1:8130/api/v1/streams
curl http://127.0.0.1:8130/api/v1/buffers
curl 'http://127.0.0.1:8130/api/v1/recorder/taps?after=0&limit=10'
```

Stream stummschalten:

```bash
curl -X POST http://127.0.0.1:8130/api/v1/sessions/CALL-ID/mute \
  -H 'Content-Type: application/json' \
  -d '{"node_id":"tbs-b","logical_ts":3,"muted":true}'
```

Testframe einspeisen:

```bash
curl -X POST http://127.0.0.1:8130/api/v1/sessions/CALL-ID/inject \
  -H 'Content-Type: application/json' \
  -d '{"payload":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]}'
```

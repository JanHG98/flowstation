# Open-Lab API examples

```bash
curl http://127.0.0.1:8100/api/v1/status
curl -X POST http://127.0.0.1:8100/api/v1/subscribers \
  -H 'Content-Type: application/json' \
  -d '{"issi":1234,"display_name":"Test HRT","enabled":true,"registration_allowed":true,"default_groups":[1001]}'
curl -X POST http://127.0.0.1:8100/api/v1/sync
```

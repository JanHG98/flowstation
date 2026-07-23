# Open-Lab API Beispiele

```bash
curl http://GROUP-CORE:8110/api/v1/status
curl -X POST http://GROUP-CORE:8110/api/v1/groups -H 'Content-Type: application/json' -d '{"gssi":15501,"name":"Status","enabled":true,"attach_allowed":true,"dgna_allowed":true,"call_allowed":false,"sds_allowed":true,"emergency_allowed":false,"call_priority":0,"class_of_usage":4,"area_nodes":[],"notes":"keine Sprache"}'
curl -X POST http://GROUP-CORE:8110/api/v1/memberships -H 'Content-Type: application/json' -d '{"issi":1234,"gssi":15501,"allowed":true,"auto_attach":true,"locked":false,"notes":""}'
```

# API-Beispiele

Transfer starten:

```bash
curl -X POST http://MOBILITY-CORE:8090/api/v1/transfers \
  -H 'Content-Type: application/json' \
  -d '{
    "issi": 1234567,
    "source_node": "tbs-a",
    "target_node": "tbs-b",
    "target_local_issi": 1234567
  }'
```

Transfer abbrechen:

```bash
curl -X POST http://MOBILITY-CORE:8090/api/v1/transfers/TRANSFER-ID/cancel
```

Es werden bewusst keine Authorization-Header oder Tokens verwendet.

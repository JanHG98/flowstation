# Open-Lab API-Beispiele

## SDS-TL-Text an Einzelteilnehmer

```bash
curl -X POST http://127.0.0.1:8150/api/v1/messages \
  -H 'Content-Type: application/json' \
  -d '{
    "source_issi": 9999,
    "dest_issi": 4010001,
    "is_group": false,
    "sds_type": 4,
    "protocol_id": 130,
    "text": "NetCore SDS Router Test",
    "priority": 3,
    "ttl_secs": 300,
    "ingress": "curl"
  }'
```

## Pre-coded Status

```bash
curl -X POST http://127.0.0.1:8150/api/v1/messages \
  -H 'Content-Type: application/json' \
  -d '{
    "source_issi": 9999,
    "dest_issi": 4010001,
    "sds_type": 0,
    "status_code": 32770,
    "ttl_secs": 60
  }'
```

## Protocol-ID an Anwendung routen

```bash
curl -X POST http://127.0.0.1:8150/api/v1/routes \
  -H 'Content-Type: application/json' \
  -d '{
    "name": "LIP Collector",
    "enabled": true,
    "kind": "protocol",
    "match_value": 10,
    "target_kind": "application",
    "target": "lip-service",
    "mode": "tap",
    "notes": "Open-Lab LIP route"
  }'
```

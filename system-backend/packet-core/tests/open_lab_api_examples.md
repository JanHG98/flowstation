# Open-Lab API Beispiele

## Edge Hello

```bash
curl -sS http://127.0.0.1:8160/api/v1/edge/events \
  -H 'content-type: application/json' \
  -d '{"kind":"hello","protocol_version":"netcore-packet-edge-v1","node_id":"tbs-04010001","station_name":"Lab TBS","mcc":1,"mnc":333,"location_area":1}'
```

## Kontextaktivierung simulieren

```bash
curl -sS http://127.0.0.1:8160/api/v1/edge/events \
  -H 'content-type: application/json' \
  -d '{"kind":"activate_demand","node_id":"tbs-04010001","issi":4010001,"nsapi":1,"requested_ipv4":null,"primary_nsapi":null,"snei":null,"mtu":1500,"priority":3}'
```

## Kontext pagen

```bash
curl -sS -X POST http://127.0.0.1:8160/api/v1/contexts/4010001:1/wake \
  -H 'content-type: application/json' -d '{}'
```

## N-PDU einspeisen

```bash
curl -sS -X POST http://127.0.0.1:8160/api/v1/downlink \
  -H 'content-type: application/json' \
  -d '{"issi":4010001,"nsapi":1,"payload_hex":"4500001400000000400100000a2c00010a2c0002"}'
```

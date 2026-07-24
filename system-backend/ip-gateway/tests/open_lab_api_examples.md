# API-Beispiele

## Route

```bash
curl -X POST http://127.0.0.1:8170/api/v1/routes \
  -H 'content-type: application/json' \
  -d '{"name":"Lab","destination":"192.168.50.0/24","gateway":"10.0.1.1","interface":"eth0","enabled":true}'
```

## Firewall

```bash
curl -X POST http://127.0.0.1:8170/api/v1/firewall \
  -H 'content-type: application/json' \
  -d '{"name":"HTTP outbound","chain":"forward","action":"accept","protocol":"tcp","source_cidr":"10.0.0.0/24","destination_port":80,"priority":50,"enabled":true}'
```

## Capture

```bash
curl -X POST http://127.0.0.1:8170/api/v1/captures \
  -H 'content-type: application/json' \
  -d '{"name":"ISSI test","direction":"both","host":"10.0.0.2","protocol":"udp","port":53}'
```

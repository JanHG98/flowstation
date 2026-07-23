# Open-Lab-API-Beispiele

Status und Suche:

```bash
curl http://127.0.0.1:8140/api/v1/status
curl http://127.0.0.1:8140/api/v1/active
curl 'http://127.0.0.1:8140/api/v1/recordings?gssi=2000&limit=100'
curl 'http://127.0.0.1:8140/api/v1/recordings?issi=1001&emergency=true'
```

Integrität prüfen:

```bash
curl -X POST http://127.0.0.1:8140/api/v1/recordings/RECORDING-ID/verify
```

Retention und Legal Hold:

```bash
curl -X POST http://127.0.0.1:8140/api/v1/recordings/RECORDING-ID/retention \
  -H 'Content-Type: application/json' \
  -d '{"days":90}'

curl -X POST http://127.0.0.1:8140/api/v1/recordings/RECORDING-ID/hold \
  -H 'Content-Type: application/json' \
  -d '{"legal_hold":true}'
```

Export:

```bash
curl -OJ http://127.0.0.1:8140/api/v1/recordings/RECORDING-ID/export
```

Achtung: Diese API ist im aktuellen Stand vollständig offen.

# Open-Lab API-Beispiele

## CCK erzeugen

```bash
curl -sS http://127.0.0.1:8190/api/v1/keys \
  -H 'Content-Type: application/json' \
  -d '{
    "kind":"CCK",
    "scope":"network",
    "scope_value":null,
    "label":"Test CCK",
    "key_bytes":16,
    "crypto_period_start":null,
    "crypto_period_end":null,
    "notes":"lab"
  }'
```

Die Antwort enthält Fingerprint und Referenz, aber kein Schlüsselmaterial.

## Node-Profil erzeugen

```bash
curl -sS http://127.0.0.1:8190/api/v1/nodes \
  -H 'Content-Type: application/json' \
  -d '{"node_id":"tbs-04010001","display_name":"TBS 1","notes":"lab"}'
```

Die Antwort enthält den serverseitigen Bootstrap-Pfad. Das Secret selbst bleibt in der Datei.

## OTAR-Job

```bash
curl -sS http://127.0.0.1:8190/api/v1/otar/jobs \
  -H 'Content-Type: application/json' \
  -d '{
    "key_id":"KEY_UUID",
    "target_nodes":["tbs-04010001"],
    "target_issis":[],
    "target_gssis":[15501],
    "not_before":null,
    "expires_at":null,
    "notes":"GCK rollout"
  }'
```

Danach mit zwei unterschiedlichen Actors freigeben und queueen.

## Edge-Claim

```bash
curl -sS http://127.0.0.1:8190/api/v1/edge/actions/claim \
  -H 'Content-Type: application/json' \
  -d '{"node_id":"tbs-04010001","max_actions":10}'
```

Die Antwort enthält ausschließlich nodegebundene Envelopes, keine Rohschlüssel.

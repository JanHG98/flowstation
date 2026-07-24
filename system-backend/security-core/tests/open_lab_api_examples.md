# Open-Lab API-Beispiele

```bash
curl -X PUT http://security-core:8180/api/v1/profiles/4010001 \
  -H 'content-type: application/json' \
  -d '{"issi":4010001,"display_name":"Jan HRT","preferred_security_class":3,"minimum_security_class":1}'

curl -X POST http://security-core:8180/api/v1/auth/start \
  -H 'content-type: application/json' \
  -d '{"node_id":"tbs-04010001","issi":4010001,"requested_security_class":3,"supported_security_classes":[1,3]}'

curl -X POST http://security-core:8180/api/v1/edge/actions/claim \
  -H 'content-type: application/json' \
  -d '{"node_id":"tbs-04010001","limit":10}'
```

Die Claim-Antwort kann geheimes Lab-Material enthalten und gehört nicht in Tickets, Screenshots oder normale Logs.

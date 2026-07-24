# Connector-Vertrag

## Generisches Outbound Envelope

Generische HTTP-Connectoren erhalten:

```json
{
  "schema": "netcore-application-event-v1",
  "delivery_id": "uuid",
  "event_id": "uuid",
  "event_type": "alarm.created",
  "destination": "optional",
  "text": "optional",
  "payload": {},
  "correlation_id": "uuid-or-external-id",
  "priority": 3,
  "connector_kind": "generic_webhook"
}
```

HTTP 2xx gilt als Erfolg. Andere Statuscodes werden mit begrenztem Response-Ausschnitt protokolliert und nach Backoff erneut versucht.

## Inbound Webhook

```text
POST /api/v1/webhooks/{connector_id}
```

Body:

```json
{
  "event_type": "external.message",
  "destination": "2000",
  "text": "Hallo",
  "payload": {},
  "idempotency_key": "foreign-message-id",
  "correlation_id": "foreign-trace-id",
  "priority": 3
}
```

Der Connector muss `inbound` oder `bidirectional` sein. Routingregeln entscheiden anschließend über die Ziele.

## Authentisierung externer Systeme

Unterstützte Secret-Namen:

- `bearer_token` → `Authorization: Bearer ...`
- `api_key` → `X-API-Key`
- `auth_key` → `X-Auth-Key`
- `basic_password` zusammen mit `settings.basic_username`
- benannte Platzhalter in URLs, zum Beispiel `{bot_token}`

Secret-Werte werden nie in Connector-GETs, Export oder normalen Backups ausgegeben.

## Zustellgarantie

Der Gateway bietet persistentes **at-least-once** Retry. Fremdsysteme sollen `delivery_id` oder `idempotency_key` zur Deduplizierung verwenden. Eine systemübergreifende Exactly-once-Garantie wird ausdrücklich nicht behauptet.

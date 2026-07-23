# Anwendungsgateways und Protocol-ID-Routing

Protocol-ID-Regeln können SDS-Nachrichten an eine benannte Anwendung weiterreichen. Die Anwendung liest ihre Queue über:

```text
GET /api/v1/application-outbox?application=<name>
```

Nach Verarbeitung bestätigt sie das Leg:

```text
POST /api/v1/application-outbox/<name>/<message-id>/ack
Content-Type: application/json

{"success":true,"message":"accepted"}
```

Die Modi sind:

- `tap`: Anwendung erhält eine Kopie; Funkrouting bleibt bestehen.
- `route`: Anwendung wird als reguläres Ziel ergänzt.
- `intercept`: Anwendung übernimmt die Nachricht; automatische Funkweiterleitung wird unterdrückt.

In der aktuellen Open-Lab-Phase ist die Outbox nicht authentifiziert. Die spätere Application-Gateway-/Security-Phase muss dafür Dienstidentitäten, Signaturen, RBAC und gegebenenfalls Payload-Masking ergänzen.

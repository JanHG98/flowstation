# Backend-WebSocket

Der offene Backend-WebSocket unter `/ws/backend` ist die erste Transportfläche für spätere zentrale Dienste wie `mobility-core`, `call-control` und `sds-router`.

Optionaler WebSocket-Subprotocol-Name:

```text
netcore-node-gateway-backend-v1
```

## Serverereignisse

Direkt nach der Verbindung:

```json
{
  "kind": "snapshot",
  "snapshot": {
    "status": {},
    "nodes": []
  }
}
```

Danach unter anderem:

```json
{
  "kind": "event",
  "event": {
    "seq": 12,
    "timestamp": "2026-07-23T20:00:00.000Z",
    "kind": "node_connected",
    "node_id": "tbs-test",
    "detail": {}
  }
}
```

Vollständige TBS-Nachrichten werden weitergereicht:

```json
{
  "kind": "node_message",
  "node_id": "tbs-test",
  "message": {
    "kind": "heartbeat",
    "heartbeat": {}
  }
}
```

## Clientanfragen

Gateway-Ping:

```json
{ "kind": "ping" }
```

TBS anpingen:

```json
{ "kind": "ping_node", "node_id": "tbs-test" }
```

TBS trennen:

```json
{ "kind": "disconnect_node", "node_id": "tbs-test" }
```

Kommando senden:

```json
{
  "kind": "command",
  "node_id": "tbs-test",
  "operator_id": "mobility-core-test",
  "command": {
    "KickMs": {
      "issi": 1234567
    }
  }
}
```

Jede Anfrage erhält ein `action_result`.

## Sicherheit

In diesem Paket ist der Backend-WebSocket bewusst offen. Es gibt keine Tokens und keine Herkunftsprüfung. Er darf nur aus dem isolierten Backend-/Managementnetz erreichbar sein.

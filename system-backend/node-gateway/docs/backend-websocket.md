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
{ "kind": "ping", "request_id": "health-1" }
```

TBS anpingen:

```json
{ "kind": "ping_node", "request_id": "ping-1", "node_id": "tbs-test" }
```

TBS trennen:

```json
{ "kind": "disconnect_node", "request_id": "disconnect-1", "node_id": "tbs-test" }
```

Kommando senden:

```json
{
  "kind": "command",
  "request_id": "mobility-export-1",
  "node_id": "tbs-test",
  "operator_id": "mobility-core-test",
  "command": {
    "KickMs": {
      "issi": 1234567
    }
  }
}
```

Jede Anfrage erhält ein `action_result`. Bei Kommandos enthält die Antwort zusätzlich die vom Gateway vergebene `command_id`:

```json
{
  "kind": "action_result",
  "request_id": "mobility-export-1",
  "command_id": "de305d54-75b4-431b-adb2-eb6b9e546014",
  "ok": true,
  "message": "command queued"
}
```

`request_id` wird vom Backend-Dienst vergeben und unverändert zurückgesendet. Dadurch können mehrere zentrale Dienste ihre asynchronen Anfragen zuverlässig korrelieren.

## Sicherheit

In diesem Paket ist der Backend-WebSocket bewusst offen. Es gibt keine Tokens und keine Herkunftsprüfung. Er darf nur aus dem isolierten Backend-/Managementnetz erreichbar sein.

## Hochratiger Media-Topic

Sprachframes werden nicht an jeden Backend-Client verteilt. Ein Media Switch meldet das Topic nach der Verbindung explizit an:

```json
{
  "kind": "subscribe",
  "request_id": "media-switch-subscribe",
  "topics": ["media_frames"]
}
```

Nur diese Backend-Session erhält danach `node_message`-Ereignisse mit `message.kind = "media_frame"`. Mobility Core, Subscriber Core, Group Core und Call Control werden dadurch nicht mit Sprachtraffic belastet.

Downlink-Media wird ohne Erfolgsbestätigung pro Frame gesendet:

```json
{
  "kind": "media_frame",
  "node_id": "tbs-ziel",
  "frame": {
    "session_id": "logical-call-id",
    "source_node_id": "tbs-quelle",
    "sequence": 42,
    "logical_ts": 3,
    "codec": "tetra_acelp0",
    "payload": [0, 1, 2]
  }
}
```

Fehler – beispielsweise ein offline befindlicher oder nicht mediafähiger Node – werden weiterhin als `action_result` gemeldet. Erfolgreiche Einzel-Frames erhalten bewusst kein ACK, damit der Gateway-Datenpfad nicht die doppelte Nachrichtenrate erzeugt.

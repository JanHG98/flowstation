# Control-Room-Verbindungsfix und Warning-Cleanup

Dieser Stand behebt zwei getrennte Punkte:

1. Ältere Control-Room-Builds konnten einen WebSocket zunächst mit HTTP 101 annehmen und ihn unmittelbar danach schließen. Die Basisstation meldete dadurch beim ersten Hello nur `Broken pipe`.
2. Fünf echte Dead-Code-Warnungen im Binary `netcore-control-room` wurden an ihrer Ursache entfernt.

## Belastbare Verbindungsprüfung

Der Control Room kennzeichnet jeden erfolgreich angenommenen WebSocket-Upgrade jetzt mit:

```
X-NetCore-Control-Room: 1
```

Bei einem `netcore-control-room-*`-Subprotokoll verlangt die Basisstation diesen Marker. Anschließend führt sie noch vor dem Rückgabewert von `connect()` einen echten WebSocket-Ping/Pong-Test durch. Die Meldung `ControlRoom transport connected` erscheint damit erst, wenn der serverseitige WebSocket-Handler nachweislich lebt.

Erwartetes Erfolgslog:

```
WebSocketTransport: connected to ws://HOST:PORT/node (subprotocol=netcore-control-room-node-v1)
ControlRoom transport connected
ControlRoom hello accepted: NetCore Control Room accepted node
```

Ein alter oder nicht neu gestarteter Control Room wird nun eindeutig erkannt:

```
Control Room endpoint did not advertise x-netcore-control-room=1 ... deploy/restart the matching netcore-control-room binary
```

Ein Authentifizierungsfehler wird bereits beim HTTP-Handshake mit 401 abgewiesen. Der Token in `[control_room]` auf der Basisstation muss mit dem Node-Token des Control Rooms übereinstimmen.

## Serverstand kontrollieren

Nach Installation und Neustart des Control Rooms:

```bash
curl -s http://127.0.0.1:9010/health | jq
```

Der neue Stand liefert unter anderem:

```json
{
  "build_fix": "v5.14.2-no-resolved-len",
  "control_room_ws": "marker-ping-v1"
}
```

## Warning-Cleanup

Nicht unterdrückt, sondern entfernt beziehungsweise sinnvoll verwendet:

- unbenutztes `AuthState::disabled()` entfernt
- interne Passwortprüfung lädt nur noch tatsächlich benötigte Benutzerfelder aus SQLite
- `V5_14_2_NO_RESOLVED_LEN_MARKER` wird über `/health` ausgegeben
- unbenutzte Wrapper- und interne State-Methoden entfernt

## Wichtig beim Deployment

Für diesen Fix müssen **beide** neuen Binaries installiert und neu gestartet werden:

- `netcore-control-room` auf dem Control-Room-Host/LXC
- `bluestation-bs` auf der Basisstation

Nur die Basisstation auszutauschen reicht absichtlich nicht mehr: Sie lehnt einen alten Control-Room-Handshake mit einer eindeutigen Versionsmeldung ab.

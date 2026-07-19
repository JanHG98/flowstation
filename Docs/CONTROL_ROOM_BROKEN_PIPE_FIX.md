# NetCore Control Room WebSocket Broken-Pipe Fix

## Betroffener Fehler

Auf der Basisstation erschien direkt nach jedem scheinbar erfolgreichen WebSocket-Handshake:

```text
ControlRoom transport connected
ControlRoom transport send failed: ... Broken pipe (os error 32)
```

Die Verbindung wurde anschließend in sehr kurzer Folge neu aufgebaut.

## Ursache

Es kamen zwei Probleme zusammen:

1. Bei aktivierter Control-Room-Authentifizierung nahm der Server einen nicht autorisierten
   WebSocket zunächst mit HTTP `101 Switching Protocols` an und schloss ihn erst danach.
   Die Basisstation wertete den Handshake daher als erfolgreich und traf beim ersten Node-Hello
   auf eine bereits geschlossene TCP-Verbindung.
2. Der Telemetriepfad rief bei jedem neuen Ereignis sofort erneut `connect()` auf und umging
   damit den vorgesehenen Reconnect-Backoff von zehn Sekunden.

Die frühere Meldung über eine fehlende `X-Brew-Version` war für die Control-Room-Verbindung
außerdem irreführend. Diese Headerauswertung gehört ausschließlich zum Brew/TetraPack-Protokoll.

## Änderungen

- Der Control Room weist nicht autorisierte WebSockets bereits während des HTTP-Handshakes mit
  `401 Unauthorized` zurück.
- Unbekannte WebSocket-Pfade werden während des Handshakes mit `404 Not Found` abgewiesen.
- Die Basisstation zeigt HTTP-Status und Antworttext des abgewiesenen Handshakes an.
- Der Zehn-Sekunden-Reconnect-Backoff gilt jetzt auch für Verbindungsversuche aus dem
  Telemetriepfad.
- Fehlerhafte Verbindungen werden explizit geschlossen, bevor ein neuer Versuch erfolgt.
- Der Control-Room-Subprotocol-Handshake wird geprüft.
- `X-Brew-Version` wird nur noch bei einer echten Brew-Verbindung ausgewertet und geloggt.
- Die Basisstation warnt beim Start, wenn für den Control Room keine Node-Credentials gesetzt sind.

## Wichtig: Authentifizierung passend konfigurieren

Der Code-Fix verhindert Broken-Pipe-Flapping und liefert eine klare Fehlermeldung. Wenn der
Control Room mit `[auth] enabled = true` läuft, benötigt die Basisstation trotzdem denselben
Node-Token.

Auf dem Control-Room-Host steht dieser üblicherweise in:

```text
/etc/netcore-control-room/control-room.env
```

Beispiel:

```bash
sudo grep '^NETCORE_CONTROL_ROOM_NODE_TOKEN=' /etc/netcore-control-room/control-room.env
```

Denselben Wert auf der Basisstation in der aktiven `config.toml` setzen:

```toml
[control_room]
enabled = true
host = "10.0.1.25"
port = 9010
use_tls = false
endpoint_path = "/node"

node_id = "SRV-M_TBS-01"
station_name = "SRV-M_TBS-01"
site = "Main"

token = "EXAKT-DERSELBE-NODE-TOKEN"
```

Der Token wird vom vorhandenen Config-Parser intern als HTTP Basic Auth mit Benutzer `node`
verwendet. Er muss nicht zusätzlich als `username` und `password` angegeben werden.

## Build

Vom Repo-Root aus alte Artefakte vollständig entfernen:

```bash
rm -rf target
cargo clean
```

Danach alle betroffenen Programme neu bauen:

```bash
cargo build --release \
  -p bluestation-bs \
  -p netcore-control-room \
  -p netcore-control-room-operator
```

`bluestation-bs` aktiviert Asterisk und Recording in diesem Stand standardmäßig. Alternativ
explizit:

```bash
cargo build --release \
  -p bluestation-bs \
  -p netcore-control-room \
  -p netcore-control-room-operator \
  --features "bluestation-bs/asterisk,bluestation-bs/recording"
```

## Installation

Zuerst beide Dienste stoppen:

```bash
sudo systemctl stop bluestation-bs
sudo systemctl stop netcore-control-room
```

Die tatsächlichen Binary-Pfade prüfen:

```bash
systemctl cat bluestation-bs | grep '^ExecStart='
systemctl cat netcore-control-room | grep '^ExecStart='
```

Beispiel für `/usr/local/bin`:

```bash
sudo rm -f /usr/local/bin/bluestation-bs
sudo rm -f /usr/local/bin/netcore-control-room

sudo install -m 0755 target/release/bluestation-bs /usr/local/bin/bluestation-bs
sudo install -m 0755 target/release/netcore-control-room /usr/local/bin/netcore-control-room
```

Falls der Control Room auf einem anderen Host/LXC läuft, dort nur das passende neue
`netcore-control-room`-Binary installieren.

Danach zuerst den Control Room und anschließend die Basisstation starten:

```bash
sudo systemctl start netcore-control-room
sudo systemctl start bluestation-bs
```

## Erwartete Logs

### Erfolgreich

```text
WebSocketTransport: connected to ws://10.0.1.25:9010/node (subprotocol=netcore-control-room-node-v1)
ControlRoom transport connected
ControlRoom hello accepted: NetCore Control Room accepted node
```

### Token fehlt oder ist falsch

```text
ControlRoom transport connection failed: Connection failed: WebSocket handshake rejected with HTTP 401 Unauthorized: unauthorized websocket request, will retry in 10s
```

Die Verbindung wird dann höchstens alle zehn Sekunden erneut versucht; es entsteht keine
Broken-Pipe-Schleife mehr.

### Falscher Pfad

```text
ControlRoom transport connection failed: Connection failed: WebSocket handshake rejected with HTTP 404 Not Found: unknown websocket endpoint, will retry in 10s
```

## Kontrolle

Beide Logs parallel prüfen:

```bash
sudo journalctl -u netcore-control-room -f
sudo journalctl -u bluestation-bs -f
```

API-Status des Control Rooms:

```bash
curl -s http://10.0.1.25:9010/health
```

Bei aktivierter Benutzer-/API-Authentifizierung benötigt der Operator weiterhin seine eigenen
Credentials. Der Node-Token der Basisstation ist davon getrennt.

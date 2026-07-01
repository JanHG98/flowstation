# Native Control Room UI

Die native UI ist der grafische Operator-Client für Windows/Linux.
Der Control-Room-Core läuft separat auf dem LXC und bleibt headless.

## Architektur

- TBS: `bluestation-bs` mit `[control_room] token = "..."`
- LXC: `netcore-control-room` Core-Service, SQLite, Auth/RBAC, API
- Windows/Operator-PC: `netcore-control-room-ui.exe`

## Profile

Die UI liest dieselbe `operator.toml` wie die CLI.
Unter Windows liegt diese typischerweise hier:

```text
%APPDATA%\netcore\control-room\operator.toml
```

Beispiel:

```toml
[profiles.default]
api = "http://10.0.1.25:9010"
default_node = "SRV-M_TBS-01"
operator_id = "jan"
token_file = 'C:\Users\Jan\AppData\Roaming\netcore\control-room\operator.token'
```

## Multi-Window-Modus

Jedes Modul kann über den `↗`-Button in der Seitenleiste als eigenes Modulfenster geöffnet werden.
Die Fenster sind verschiebbar und skalierbar. Das ist ideal für mehrere Monitore oder einen POS-/Touch-Bedienplatz.

## Karte

Der Tab `Karte` visualisiert Standortmeldungen aus `/api/locations`.
Die Karte ist eine Offline-/Schemadarstellung ohne externe Tile-Server.
Das ist bewusst so, damit die Leitstellen-UI auch ohne Internet funktioniert.

## Start

```cmd
system-backend\control-room\ui\target\release\netcore-control-room-ui.exe --config "%APPDATA%\netcore\control-room\operator.toml" --profile default
```

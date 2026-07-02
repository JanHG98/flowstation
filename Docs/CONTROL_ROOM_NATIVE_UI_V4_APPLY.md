# NetCore Control Room Native UI v4 – echte Live-Karte

Dieser Stand ersetzt die bisherige Native UI v3 vollständig.

## Zielarchitektur

- Basisstation/TBS: bleibt Funkknoten und behält ihren Node-Token in `config.toml`.
- Control-Room-LXC: bleibt headless, nur Core-Service, SQLite und CLI.
- Windows-Operator-PC: bekommt die grafische Native UI.

## Neu in v4

- sichtbarer Versionshinweis `Native UI v4 · echte OS-Fenster · Live-Karte`
- Kartenmodul nutzt echte Kartenkacheln statt weißer Pseudo-Fläche
- Standortpunkte kommen live aus `/api/locations`
- Kartenkacheln werden lokal gecacht
- Online-Kacheln können in der UI deaktiviert werden
- Zoom +/− im Kartenmodul
- Fallback-Zentrum Hannover, solange noch keine LIP-/Standortdaten vorliegen
- echte OS-Fenster aus v3 bleiben erhalten

## Windows Update von alter UI-Version

Voraussetzung: Du bist in CMD im Repo-Root.

### 1. Alte UI schließen

```cmd
taskkill /IM netcore-control-room-ui.exe /F
```

Wenn kein Prozess gefunden wird, ist das okay.

### 2. Alte UI-Builds und alte EXEs löschen

```cmd
cargo clean --manifest-path system-backend\control-room\ui\Cargo.toml
rmdir /S /Q system-backend\control-room\ui\target
del /F /Q target\release\netcore-control-room-ui.exe
del /F /Q system-backend\control-room\ui\target\release\netcore-control-room-ui.exe
```

Wenn ein Ordner oder eine Datei nicht existiert, ist das okay.

Zur Sicherheit alle alten UI-EXEs suchen und löschen:

```cmd
powershell -NoProfile -Command "Get-ChildItem -Recurse -Filter netcore-control-room-ui.exe | Remove-Item -Force"
```

### 3. ZIP entpacken

Wenn die ZIP im Downloads-Ordner liegt:

```cmd
powershell -NoProfile -Command "Expand-Archive -Force '%USERPROFILE%\Downloads\netcore-control-room-native-ui-v4-live-map-files.zip' -DestinationPath '%CD%'"
```

### 4. Prüfen, ob v4 wirklich im Quellcode liegt

```cmd
findstr /S /N /I "Native UI v4 Live-Karte tile.openstreetmap.org" system-backend\control-room\ui\src\main.rs
```

Du solltest Treffer sehen.

### 5. UI neu bauen

```cmd
cargo build --release --manifest-path system-backend\control-room\ui\Cargo.toml
```

### 6. Neuste EXE finden

```cmd
powershell -NoProfile -Command "Get-ChildItem -Recurse -Filter netcore-control-room-ui.exe | Sort-Object LastWriteTime -Descending | Select-Object LastWriteTime,FullName"
```

Die neuste EXE starten.

Meistens:

```cmd
target\release\netcore-control-room-ui.exe --config "%APPDATA%\netcore\control-room\operator.toml" --profile default
```

oder:

```cmd
system-backend\control-room\ui\target\release\netcore-control-room-ui.exe --config "%APPDATA%\netcore\control-room\operator.toml" --profile default
```

## Optional: operator.toml um Kartenkonfiguration ergänzen

Öffnen:

```cmd
notepad "%APPDATA%\netcore\control-room\operator.toml"
```

Anhängen:

```toml
[ui.map]
online_tiles = true
tile_url = "https://tile.openstreetmap.org/{z}/{x}/{y}.png"
tile_attribution = "© OpenStreetMap contributors"
default_lat = 52.3759
default_lon = 9.7320
default_zoom = 13
min_zoom = 3
max_zoom = 18
```

Wenn keine `ui.map`-Sektion vorhanden ist, nutzt die UI diese Defaults automatisch.

## Erwartung

In der Kopfzeile muss stehen:

```text
Native UI v4 · echte OS-Fenster · Live-Karte
```

Im Kartenmodul muss sichtbar sein:

- echte Karte/Kacheln statt weißer Fläche
- Checkbox `Online-Kartenkacheln laden`
- Buttons `− Zoom`, `+ Zoom`, `Zoom reset`
- Marker, sobald `/api/locations` Positionen liefert

Wenn keine Standortdaten vorhanden sind, zeigt die Karte trotzdem echte Kacheln um Hannover als Fallback-Zentrum.

## LXC und TBS

Auf dem LXC ist für dieses Update nichts an der UI zu tun.

Kontrolle:

```bash
systemctl status netcore-control-room --no-pager -l
curl -i http://127.0.0.1:9010/health
curl -i http://127.0.0.1:9010/api/overview
```

Erwartung:

- `/health` ohne Token: `200 OK`
- `/api/overview` ohne Token: `401 Unauthorized`

Die TBS behält den Token in ihrer `config.toml`:

```toml
[control_room]
token = "..."
```


## v4.1 Buildfix

Dieser ZIP-Stand enthält zusätzlich den Rust-Borrow-Checker-Fix für E0502 in `render_locations` und `render_map`.

# NetCore Control Room UI

Native Desktop UI für den NetCore Control Room.

## v4.3 Highlights

- echte OS-Fenster pro Modul
- Multi-Monitor-tauglich
- echte Live-Karte mit Kartenkacheln
- interaktive Karte: ziehen, Mausrad-Zoom, Doppelklick-Zentrierung
- flüssigere Karte durch nicht-blockierendes Tile-Laden im Hintergrund
- Klick auf GPS-/LIP-Punkt zeigt Geräteinfos direkt in der Karte
- lokale Tile-Cache-Ablage
- Standortpunkte aus `/api/locations`
- kein Browser, keine Web-App
- Token/Profile über `operator.toml`

## Windows Update/Build

Im Repo-Root:

```cmd
taskkill /IM netcore-control-room-ui.exe /F
cargo clean --manifest-path system-backend\control-room\ui\Cargo.toml
rmdir /S /Q system-backend\control-room\ui\target
del /F /Q target\release\netcore-control-room-ui.exe
del /F /Q system-backend\control-room\ui\target\release\netcore-control-room-ui.exe
powershell -NoProfile -Command "Get-ChildItem -Recurse -Filter netcore-control-room-ui.exe | Remove-Item -Force"
cargo build --release --manifest-path system-backend\control-room\ui\Cargo.toml
```

Start:

```cmd
target\release\netcore-control-room-ui.exe --config "%APPDATA%\netcore\control-room\operator.toml" --profile default
```

Wenn die EXE an anderer Stelle liegt:

```cmd
powershell -NoProfile -Command "Get-ChildItem -Recurse -Filter netcore-control-room-ui.exe | Sort-Object LastWriteTime -Descending | Select-Object LastWriteTime,FullName"
```

## Kartenkonfiguration

In `%APPDATA%\netcore\control-room\operator.toml` kann zusätzlich stehen:

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

Die UI cached Kacheln lokal. Wenn `online_tiles = false` gesetzt ist, nutzt sie nur den lokalen Cache/Fallback.

## Bedienung

Links:

- `OS-Fenster-Modus`
- `↗` Modul als echtes OS-Fenster öffnen
- `▣` Modulfenster offen
- `Alle Module als OS-Fenster öffnen`
- `Alle OS-Fenster schließen`

Im Kartenmodul:

- Ziehen mit linker Maustaste: Karte verschieben
- Mausrad: rein-/rauszoomen, mit Mausposition als Anker
- Doppelklick: Karte auf Mausposition zentrieren
- `Positionen folgen`: automatisch auf vorhandene LIP-Punkte zoomen
- `Ansicht reset`: wieder auf Live-/Folgemodus zurücksetzen
- Online-Kartenkacheln ein/aus
- Standortpunkte live aus `/api/locations`
- Klick auf Standortpunkt: Geräte-/ISSI-Details anzeigen


## v4.5 Highlights

- Standorte und Karte zeigen pro ISSI nur noch den aktuellsten Standort.
- Alte historische Positionsmeldungen werden in der UI als Zombie-Positionen ausgeblendet.

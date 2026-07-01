# NetCore Control Room Native UI

Die Native UI ist der grafische Operator-Client für Windows/Linux-Operator-PCs.

Der Control-Room-LXC bleibt headless. Die Basisstation bleibt Funkknoten. Die UI spricht nur mit dem Control-Room-Core per HTTP API.

## Version v4.2

v4.2 ergänzt eine echte interaktive Live-Karte:

- jedes Modul kann als eigenes Betriebssystem-Fenster geöffnet werden
- Fenster können über mehrere Monitore verteilt werden
- Karte nutzt echte Kartenkacheln und `/api/locations`
- Karte kann per Maus gezogen werden
- Mausrad zoomt auf die Mausposition
- Doppelklick zentriert auf die Mausposition
- sichtbarer Versionshinweis in der Kopfzeile: `Native UI v4.2 · echte OS-Fenster · interaktive Live-Karte`

## Windows Build

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

Wenn die EXE dort nicht liegt, die neuste EXE suchen:

```cmd
powershell -NoProfile -Command "Get-ChildItem -Recurse -Filter netcore-control-room-ui.exe | Sort-Object LastWriteTime -Descending | Select-Object LastWriteTime,FullName"
```

## Profile

Beispiel `operator.toml`:

```toml
[profiles.default]
api = "http://10.0.1.25:9010"
default_node = "SRV-M_TBS-01"
operator_id = "jan"
token_file = 'C:\Users\Jan\AppData\Roaming\netcore\control-room\operator.token'
```

Optionale Karte:

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

## Multi-Window

In der linken Leiste:

- `OS-Fenster-Modus` aktivieren
- pro Modul `↗` drücken
- oder `Alle Module als OS-Fenster öffnen`

Die Modulfenster sind echte Betriebssystem-Fenster und können auf andere Monitore gezogen werden.

## Kartenbedienung

- Linke Maustaste halten und ziehen: Karte verschieben
- Mausrad: zoomen, die Mausposition bleibt dabei der Zoom-Anker
- Doppelklick: auf Mausposition zentrieren
- `Positionen folgen`: zurück zur automatischen Ansicht auf vorhandene LIP-Punkte
- `Ansicht reset`: Zoom/Pan zurücksetzen und Follow-Modus aktivieren


## v4.5 Highlights

- Standorte und Karte zeigen pro ISSI nur noch den aktuellsten Standort.
- Alte historische Positionsmeldungen werden in der UI als Zombie-Positionen ausgeblendet.

## v4.6 Directory / Aufräumen

Die UI kann ein lokales Directory aus `operator.toml` nutzen. Das Backend und die TBS bleiben unverändert.

Wichtige Effekte:

- Pro ISSI wird bei Standorten und Karte nur der aktuellste Standort angezeigt.
- Teilnehmer zeigt nur echte Endgeräte; Infrastruktur/Basisstation/Gateway werden standardmäßig ausgeblendet.
- Namen, Gerätetypen, statische Gruppen, Gruppennamen, Statuslabels und Statusgruppen können lokal gepflegt werden.

Beispiel:

```toml
[directory]
hide_infrastructure = true

[directory.subscribers."2010002"]
name = "Jan HRT"
device_class = "HRT"
status = "Einsatzbereit"
status_group = "crew"
groups = [15201, 15205]

[directory.groups."15205"]
name = "Tactical"
kind = "Sprechgruppe"

[directory.status_groups."crew"]
name = "Crew-Status"

[directory.statuses."1"]
label = "Frei / bereit"
group = "crew"
```

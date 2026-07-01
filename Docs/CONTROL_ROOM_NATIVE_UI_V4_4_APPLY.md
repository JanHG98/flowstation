# NetCore Control Room Native UI v4.4 – fein dosierte Karte

Dieser Stand baut auf v4.3 auf.

## Neu

- Mausrad-Zoom der Karte ist deutlich weniger sensibel.
- Ein physisches Mausrad-Raster soll nur noch eine Zoomstufe auslösen.
- Die UI nutzt dafür `raw_scroll_delta` statt `smooth_scroll_delta`, damit egui die Scrollbewegung nicht über mehrere Frames erneut anwendet.
- Der Tab `Standorte` zeigt keine Karte mehr, sondern nur noch Metriken und Tabelle.
- Die Live-Karte liegt nur noch im Tab `Karte`.

## Architektur

- TBS/Basisstation bleibt unverändert und behält ihren Node-Token in `config.toml`.
- LXC bleibt headless mit Core-Service, SQLite und CLI.
- Windows-Operator-PC bleibt das einzige System mit grafischer UI.

## Windows Update

1. UI schließen:

```cmd
taskkill /IM netcore-control-room-ui.exe /F
```

2. Alte Builds und alte EXEs löschen:

```cmd
cargo clean --manifest-path system-backend\control-room\ui\Cargo.toml
rmdir /S /Q system-backend\control-room\ui\target
del /F /Q target\release\netcore-control-room-ui.exe
del /F /Q system-backend\control-room\ui\target\release\netcore-control-room-ui.exe
powershell -NoProfile -Command "Get-ChildItem -Recurse -Filter netcore-control-room-ui.exe | Remove-Item -Force"
```

3. ZIP entpacken:

```cmd
powershell -NoProfile -Command "Expand-Archive -Force '%USERPROFILE%\Downloads\netcore-control-room-native-ui-v4-4-map-wheel-table-files.zip' -DestinationPath '%CD%'"
```

4. Prüfen:

```cmd
findstr /S /N /I "Native UI v4.4 raw_scroll_delta Live-Karte liegt jetzt" system-backend\control-room\ui\src\main.rs
```

5. Bauen:

```cmd
cargo build --release --manifest-path system-backend\control-room\ui\Cargo.toml
```

6. Neuste EXE finden:

```cmd
powershell -NoProfile -Command "Get-ChildItem -Recurse -Filter netcore-control-room-ui.exe | Sort-Object LastWriteTime -Descending | Select-Object LastWriteTime,FullName"
```

7. Starten:

```cmd
target\release\netcore-control-room-ui.exe --config "%APPDATA%\netcore\control-room\operator.toml" --profile default
```

Falls die EXE dort nicht liegt:

```cmd
system-backend\control-room\ui\target\release\netcore-control-room-ui.exe --config "%APPDATA%\netcore\control-room\operator.toml" --profile default
```

## Erwartung

Oben steht:

```text
Native UI v4.4 · echte OS-Fenster · Live-Karte fein dosiert
```

Im Tab `Standorte` ist nur noch die Tabelle sichtbar.

Im Tab `Karte` gilt:

- Ziehen = Karte verschieben
- Mausrad = eine Zoomstufe pro Rad-Raster
- Doppelklick = zentrieren
- Klick auf Marker = Gerätedetails

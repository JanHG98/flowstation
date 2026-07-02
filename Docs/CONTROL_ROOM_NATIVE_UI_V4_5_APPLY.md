# NetCore Control Room Native UI v4.5 – aktuelle Standorte ohne Zombies

Dieser Stand baut auf v4.4 auf.

## Änderung

- `Standorte` zeigt pro ISSI nur noch den aktuellsten bekannten Standort.
- `Karte` zeichnet pro ISSI nur noch den aktuellsten GPS-/LIP-Punkt.
- Alte historische Positionsmeldungen bleiben im Backend/Raw JSON erhalten, werden in der Bedienoberfläche aber nicht mehr als Zombie-Punkte dargestellt.
- Im Tab `Standorte` wird angezeigt, wie viele alte Zombie-Positionen ausgeblendet wurden.
- Live-Karte, echte OS-Fenster, Marker-Klick mit Geräteinfos und fein dosierter Mausrad-Zoom bleiben erhalten.

## Windows-Update

Im Repo-Root in CMD:

```cmd
taskkill /IM netcore-control-room-ui.exe /F
cargo clean --manifest-path system-backend\control-room\ui\Cargo.toml
rmdir /S /Q system-backend\control-room\ui\target
del /F /Q target\release\netcore-control-room-ui.exe
del /F /Q system-backend\control-room\ui\target\release\netcore-control-room-ui.exe
powershell -NoProfile -Command "Get-ChildItem -Recurse -Filter netcore-control-room-ui.exe | Remove-Item -Force"
```

Dann ZIP entpacken:

```cmd
powershell -NoProfile -Command "Expand-Archive -Force '%USERPROFILE%\Downloads\netcore-control-room-native-ui-v4-5-latest-locations-files.zip' -DestinationPath '%CD%'"
```

Prüfen:

```cmd
findstr /S /N /I "Native UI v4.5 latest_location_rows Zombie" system-backend\control-room\ui\src\main.rs
```

Bauen:

```cmd
cargo build --release --manifest-path system-backend\control-room\ui\Cargo.toml
```

Neuste EXE finden:

```cmd
powershell -NoProfile -Command "Get-ChildItem -Recurse -Filter netcore-control-room-ui.exe | Sort-Object LastWriteTime -Descending | Select-Object LastWriteTime,FullName"
```

Starten, meistens:

```cmd
target\release\netcore-control-room-ui.exe --config "%APPDATA%\netcore\control-room\operator.toml" --profile default
```

Alternativ:

```cmd
system-backend\control-room\ui\target\release\netcore-control-room-ui.exe --config "%APPDATA%\netcore\control-room\operator.toml" --profile default
```

## Erwartung

Oben steht:

```text
Native UI v4.5 · aktuelle Standorte ohne Zombies
```

Im Tab `Standorte`:

- `Aktuelle Geräte` zählt nur eindeutige ISSIs mit jeweils aktuellstem Standort.
- `alte Zombie-Positionen ausgeblendet` erscheint, wenn historische Mehrfacheinträge vorhanden sind.
- Die Tabelle enthält keine alten Mehrfacheinträge pro ISSI mehr.

Im Tab `Karte`:

- Jeder ISSI hat maximal einen Marker.
- Der Marker zeigt den neuesten bekannten Standort.
- Klick auf Marker zeigt weiterhin Geräteinfos.

LXC und TBS bleiben unverändert.

# NetCore Control Room Native UI v4.2 – interaktive Live-Karte

Dieser Stand ersetzt v4.1 vollständig.

## Neu in v4.2

- sichtbarer Versionshinweis `Native UI v4.2 · echte OS-Fenster · interaktive Live-Karte`
- Karte mit linker Maustaste verschiebbar
- Mausrad-Zoom direkt auf die Mausposition
- Doppelklick zentriert auf die Mausposition
- `Positionen folgen` schaltet zurück in automatische Ansicht auf vorhandene LIP-Punkte
- `Ansicht reset` setzt Zoom/Pan zurück
- echte OS-Fenster aus v3 bleiben erhalten
- echte Live-Kartenkacheln aus v4 bleiben erhalten

## Windows Update

Im Repo-Root:

```cmd
taskkill /IM netcore-control-room-ui.exe /F
cargo clean --manifest-path system-backend\control-room\ui\Cargo.toml
rmdir /S /Q system-backend\control-room\ui\target
del /F /Q target\release\netcore-control-room-ui.exe
del /F /Q system-backend\control-room\ui\target\release\netcore-control-room-ui.exe
powershell -NoProfile -Command "Get-ChildItem -Recurse -Filter netcore-control-room-ui.exe | Remove-Item -Force"
powershell -NoProfile -Command "Expand-Archive -Force '%USERPROFILE%\Downloads\netcore-control-room-native-ui-v4-2-interactive-map-files.zip' -DestinationPath '%CD%'"
findstr /S /N /I "Native UI v4.2" system-backend\control-room\ui\src\main.rs
cargo build --release --manifest-path system-backend\control-room\ui\Cargo.toml
powershell -NoProfile -Command "Get-ChildItem -Recurse -Filter netcore-control-room-ui.exe | Sort-Object LastWriteTime -Descending | Select-Object LastWriteTime,FullName"
```

Start meistens:

```cmd
target\release\netcore-control-room-ui.exe --config "%APPDATA%\netcore\control-room\operator.toml" --profile default
```

## Test

- Kopfzeile muss `Native UI v4.2` zeigen
- Karte öffnen
- mit linker Maustaste ziehen
- mit Mausrad zoomen
- Doppelklick auf Karte
- `Ansicht reset` drücken
- OS-Fenster-Modus testen

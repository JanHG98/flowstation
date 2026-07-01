# NetCore Control Room Native UI v4.3 – flüssige Live-Karte + Gerätedetails

Dieser Stand ersetzt v4.2 vollständig.

## Neu in v4.3

- sichtbarer Versionshinweis `Native UI v4.3 · echte OS-Fenster · flüssige Live-Karte`
- Kartenkacheln werden im Hintergrund geladen, nicht mehr blockierend im UI-Thread
- deutlich flüssigeres Verschieben und Zoomen der Karte
- Klick auf GPS-/LIP-Marker öffnet eine Gerätekarte direkt in der Karte
- ausgewählter Marker wird gelb hervorgehoben
- Gerätekarte zeigt ISSI, Position, Quelle, Update und soweit vorhanden Teilnehmerdaten aus `/api/subscribers`
- Ziehen, Mausrad-Zoom, Doppelklick-Zentrierung und echte OS-Fenster bleiben erhalten

## Windows Update/Build

Im Repo-Root ausführen:

```cmd
taskkill /IM netcore-control-room-ui.exe /F
cargo clean --manifest-path system-backend\control-room\ui\Cargo.toml
rmdir /S /Q system-backend\control-room\ui\target
del /F /Q target\release\netcore-control-room-ui.exe
del /F /Q system-backend\control-room\ui\target\release\netcore-control-room-ui.exe
powershell -NoProfile -Command "Get-ChildItem -Recurse -Filter netcore-control-room-ui.exe | Remove-Item -Force"
powershell -NoProfile -Command "Expand-Archive -Force '%USERPROFILE%\Downloads\netcore-control-room-native-ui-v4-3-smooth-map-device-info-files.zip' -DestinationPath '%CD%'"
findstr /S /N /I "Native UI v4.3" system-backend\control-room\ui\src\main.rs
cargo build --release --manifest-path system-backend\control-room\ui\Cargo.toml
```

Neuste EXE finden:

```cmd
powershell -NoProfile -Command "Get-ChildItem -Recurse -Filter netcore-control-room-ui.exe | Sort-Object LastWriteTime -Descending | Select-Object LastWriteTime,FullName"
```

Start meistens:

```cmd
target\release\netcore-control-room-ui.exe --config "%APPDATA%\netcore\control-room\operator.toml" --profile default
```

Falls die EXE dort nicht liegt:

```cmd
system-backend\control-room\ui\target\release\netcore-control-room-ui.exe --config "%APPDATA%\netcore\control-room\operator.toml" --profile default
```

## Erwartung

- Kopfzeile zeigt `Native UI v4.3`
- Karte lädt Kacheln weiter, während die UI bedienbar bleibt
- Ziehen wirkt deutlich weniger hakelig
- Mausrad zoomt weiterhin auf Mausposition
- Klick auf grünen GPS-Punkt zeigt Geräteinfos
- ausgewählter Punkt wird gelb hervorgehoben

LXC und TBS bleiben unverändert.

# NetCore Control Room Native UI v4.1 – Live-Karte Buildfix

Dieser Stand ersetzt den v4-Live-Karten-Stand vollständig.

## Inhalt

- Native UI v4.1
- echte OS-Fenster aus v3 bleiben erhalten
- Live-Karte mit Kartenkacheln und lokalem Cache bleibt erhalten
- Buildfix für Rust-Borrow-Checker-Fehler E0502 in `render_locations` und `render_map`
- keine Patch-Dateien
- vollständige Dateien

## Wichtig

Der Fehler aus v4 war:

```text
cannot borrow `*self` as mutable because it is also borrowed as immutable
```

Ursache war, dass `self.locations` noch immutably geborgt war, während die Kartenroutine `self` mutable braucht. In v4.1 wird der aktuelle Locations-Snapshot lokal geklont, bevor die Karte gerendert wird.

## Windows-Update kurz

Im Repo-Root:

```cmd
taskkill /IM netcore-control-room-ui.exe /F
cargo clean --manifest-path system-backend\control-room\ui\Cargo.toml
rmdir /S /Q system-backend\control-room\ui\target
del /F /Q target\release\netcore-control-room-ui.exe
powershell -NoProfile -Command "Get-ChildItem -Recurse -Filter netcore-control-room-ui.exe | Remove-Item -Force"
powershell -NoProfile -Command "Expand-Archive -Force '%USERPROFILE%\Downloads\netcore-control-room-native-ui-v4-1-live-map-buildfix-files.zip' -DestinationPath '%CD%'"
cargo build --release --manifest-path system-backend\control-room\ui\Cargo.toml
target\release\netcore-control-room-ui.exe --config "%APPDATA%\netcore\control-room\operator.toml" --profile default
```

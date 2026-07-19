# NetCore Control Room UI v5.8 – Kompakter Leitstellenkopf

Dieses Paket behebt den kaputten, zu hohen Ribbon-/Kopfbereich aus v5.6/v5.7.

## Änderungen

- kein riesiger hellblauer Leerbereich mehr
- Speichern/Maske/Vorschlag werden nicht mehr abgeschnitten
- kompakte Toolbar direkt unter der blauen Kopfzeile
- Statuszeile bleibt kompakt und responsive
- Karte/Hauptbereich bekommt wieder mehr Platz
- RBAC/User+Passwort bleibt unverändert

## Windows Update

1. UI schließen.
2. Alte UI-EXEs löschen.
3. ZIP entpacken.
4. UI neu bauen.

```cmd
taskkill /IM netcore-control-room-ui.exe /F

cargo clean --manifest-path system-backend\control-room\ui\Cargo.toml
rmdir /S /Q system-backend\control-room\ui\target
del /F /Q target\release\netcore-control-room-ui.exe
del /F /Q system-backend\control-room\ui\target\release\netcore-control-room-ui.exe

powershell -NoProfile -Command "Get-ChildItem -Recurse -Filter netcore-control-room-ui.exe | Remove-Item -Force"

powershell -NoProfile -Command "Expand-Archive -Force '%USERPROFILE%\Downloads\netcore-control-room-v5-8-compact-header-responsive-files.zip' -DestinationPath '%CD%'"

cargo build --release --manifest-path system-backend\control-room\ui\Cargo.toml

target\release\netcore-control-room-ui.exe --config "%APPDATA%\netcore\control-room\operator.toml" --profile default
```

# NetCore Control Room UI v5.11.9 – Buildfix

Dieses Paket behebt die Buildfehler aus v5.11.8.

## Fixes

- `id_key_variants(...)` wiederhergestellt
  - wurde beim Entfernen von `nearest_marker(...)` versehentlich mit entfernt
- verbliebene Initializer-Zeilen entfernt:
  - `config_path,`
  - `username_source,`
- v5.11.8-Fixes bleiben enthalten:
  - Directory-first Namen
  - eigene Karten für gruppenlose Einzelgeräte
  - kein `2020001 2020001`
  - Statusfarbe/-nummer aus Textstatus
  - Kartenlabels Directory-first
  - alte Warning-Funktionen entfernt

## Windows Update

```cmd
taskkill /IM netcore-control-room-ui.exe /F

cargo clean --manifest-path system-backend\control-room\ui\Cargo.toml
rmdir /S /Q system-backend\control-room\ui\target
del /F /Q target\release\netcore-control-room-ui.exe
del /F /Q system-backend\control-room\ui\target\release\netcore-control-room-ui.exe

powershell -NoProfile -Command "Get-ChildItem -Recurse -Filter netcore-control-room-ui.exe | Remove-Item -Force"

powershell -NoProfile -Command "Expand-Archive -Force '%USERPROFILE%\Downloads\netcore-control-room-v5-11-9-status-tableau-buildfix-files.zip' -DestinationPath '%CD%'"

cargo build --release --manifest-path system-backend\control-room\ui\Cargo.toml
```

## Start

```cmd
target\release\netcore-control-room-ui.exe --config "%APPDATA%\netcore\control-room\operator.toml" --profile default
```

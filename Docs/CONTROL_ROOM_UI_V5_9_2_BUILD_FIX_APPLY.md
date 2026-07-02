# NetCore Control Room UI v5.9.2 – Buildfix

Dieses Paket behebt den verbliebenen Feldnamenfehler aus v5.9.1.

## Fix

Falsch:
- `emergency_clear_issi`

Richtig:
- `clear_issi`

Der Button `Maske leer` nutzt jetzt den echten Feldnamen aus `ControlRoomApp`.

## Windows Update

```cmd
taskkill /IM netcore-control-room-ui.exe /F

cargo clean --manifest-path system-backend\control-room\ui\Cargo.toml
rmdir /S /Q system-backend\control-room\ui\target
del /F /Q target\release\netcore-control-room-ui.exe
del /F /Q system-backend\control-room\ui\target\release\netcore-control-room-ui.exe

powershell -NoProfile -Command "Get-ChildItem -Recurse -Filter netcore-control-room-ui.exe | Remove-Item -Force"

powershell -NoProfile -Command "Expand-Archive -Force '%USERPROFILE%\Downloads\netcore-control-room-v5-9-2-clean-workplace-buildfix-files.zip' -DestinationPath '%CD%'"

cargo build --release --manifest-path system-backend\control-room\ui\Cargo.toml

target\release\netcore-control-room-ui.exe --config "%APPDATA%\netcore\control-room\operator.toml" --profile default
```

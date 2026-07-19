# NetCore Control Room UI v5.11.6 – Status-Tableau SDS exakt

Dieses Paket behebt, dass die Status-SDS im SDS-Tab sichtbar ist, aber im Status-Tableau nicht greift.

## Wichtige Änderung

Die SDS-Tabelle verwendet tatsächlich diese Felder:

- `source_issi`
- `dest_issi`
- `protocol_id`
- `text`

Der bisherige Fallback suchte primär nach:

- `source`
- `proto`

Deshalb wurde die sichtbare SDS-Zeile vom Tableau nicht gefunden.

## Fix

- SDS-Fallback liest jetzt exakt `source_issi` und `protocol_id`
- zusätzlich weiterhin Fallbacks auf `source/src/from/issi`
- Statusgeräte, die nur per SDS auftauchen, werden ins Tableau aufgenommen
- Im Status-Tableau steht nun eine kleine Diagnosezeile:
  - `SDS-Fallback: x/y SDS-Zeilen als Statuskandidaten erkannt`
- SDS-Tabelle nutzt denselben Parser wie das Tableau

## Windows Update

```cmd
taskkill /IM netcore-control-room-ui.exe /F

cargo clean --manifest-path system-backend\control-room\ui\Cargo.toml
rmdir /S /Q system-backend\control-room\ui\target
del /F /Q target\release\netcore-control-room-ui.exe
del /F /Q system-backend\control-room\ui\target\release\netcore-control-room-ui.exe

powershell -NoProfile -Command "Get-ChildItem -Recurse -Filter netcore-control-room-ui.exe | Remove-Item -Force"

powershell -NoProfile -Command "Expand-Archive -Force '%USERPROFILE%\Downloads\netcore-control-room-v5-11-6-status-tableau-sds-exakt-files.zip' -DestinationPath '%CD%'"

cargo build --release --manifest-path system-backend\control-room\ui\Cargo.toml
```

## Start

```cmd
target\release\netcore-control-room-ui.exe --config "%APPDATA%\netcore\control-room\operator.toml" --profile default
```

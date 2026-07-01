# NetCore Control Room UI v5.11.1 – Status-Tableau Buildfix

Dieses Paket behebt den v5.11-Build und stellt klar: Das Status-Tableau nutzt das zentrale NetCore Directory vom LXC (`/api/directory`).

## Fixes gegenüber v5.11

- `StatusTableauCard` und `StatusTableauDevice` sind korrekt definiert.
- `Tab::ALL` ist auf 11 Module angepasst.
- Kein falscher `can_read()`-Aufruf mehr.
- Status-Tableau nutzt vorhandene UI-Methoden:
  - `self.settings.directory`
  - `self.clean_subscriber_rows(...)`
  - `self.subscriber_is_hidden(...)`
  - `self.subscriber_display_name(...)`
  - `self.subscriber_type_label(...)`
  - `self.format_group(...)`
- Directory bedeutet hier ausdrücklich der zentrale Directory-Server über `/api/directory`.
- Lokale `operator.toml`-Directoryeinträge bleiben nur optionale Overrides.

## Directory-Beispiel auf dem LXC

```toml
[directory.subscribers."2010002"]
name = "Jan HRT"
device_class = "HRT"
status = "2"
status_group = "arbeitsplaetze"
groups = [15201]

[directory.status_groups."arbeitsplaetze"]
name = "Arbeitsplätze"

[directory.statuses."2"]
label = "Frei"
```

## Windows Update

```cmd
taskkill /IM netcore-control-room-ui.exe /F

cargo clean --manifest-path system-backend\control-room\ui\Cargo.toml
rmdir /S /Q system-backend\control-room\ui\target
del /F /Q target\release\netcore-control-room-ui.exe
del /F /Q system-backend\control-room\ui\target\release\netcore-control-room-ui.exe

powershell -NoProfile -Command "Get-ChildItem -Recurse -Filter netcore-control-room-ui.exe | Remove-Item -Force"

powershell -NoProfile -Command "Expand-Archive -Force '%USERPROFILE%\Downloads\netcore-control-room-v5-11-1-status-tableau-buildfix-files.zip' -DestinationPath '%CD%'"

cargo build --release --manifest-path system-backend\control-room\ui\Cargo.toml

target\release\netcore-control-room-ui.exe --config "%APPDATA%\netcore\control-room\operator.toml" --profile default
```

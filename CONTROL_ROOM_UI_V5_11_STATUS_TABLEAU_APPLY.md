# NetCore Control Room UI v5.11 – Status-Tableau

Dieses Paket ergänzt ein Status-Tableau nach Leitstellen-/Selectric-Vorbild.

## Funktionen

- neues Modul `Status`
- Statusgruppen kommen aus dem zentralen NetCore Directory
- Geräte mit `status_group` werden in Statusgruppen-Karten gesammelt
- Geräte ohne `status_group` erscheinen unter `Einzelgeräte`
- Namen, Gerätetypen, Gruppen und Statuslabels kommen aus dem Directory
- Live-Statusdaten werden einbezogen, wenn `/api/subscribers` sie liefert
- farbige Statusfelder ähnlich Status-Tableau:
  - 1/2/5 grün
  - 3 blau
  - 4 orange
  - 6 rot
  - 7/8 magenta/violett
- Hover zeigt Details inklusive ISSI, Typ, Gruppen, Online und letzte Meldung
- RBAC bleibt erhalten: Viewer darf Status sehen, Operator/Admin auch

## Directory-Beispiel

```toml
[directory.subscribers."2010002"]
name = "Jan HRT"
device_class = "HRT"
status = "2"
status_group = "arbeitsplaetze"
groups = [15201]

[directory.subscribers."2020004"]
name = "Event Operator"
device_class = "HRT"
status = "3"
status_group = "sprechwuensch"
groups = [15201]

[directory.status_groups."arbeitsplaetze"]
name = "Arbeitsplätze"

[directory.status_groups."sprechwuensch"]
name = "Sprechwunsch"

[directory.statuses."2"]
label = "Frei"

[directory.statuses."3"]
label = "Sprechwunsch"
```

## Windows Update

```cmd
taskkill /IM netcore-control-room-ui.exe /F

cargo clean --manifest-path system-backend\control-room\ui\Cargo.toml
rmdir /S /Q system-backend\control-room\ui\target
del /F /Q target\release\netcore-control-room-ui.exe
del /F /Q system-backend\control-room\ui\target\release\netcore-control-room-ui.exe

powershell -NoProfile -Command "Get-ChildItem -Recurse -Filter netcore-control-room-ui.exe | Remove-Item -Force"

powershell -NoProfile -Command "Expand-Archive -Force '%USERPROFILE%\Downloads\netcore-control-room-v5-11-status-tableau-files.zip' -DestinationPath '%CD%'"

cargo build --release --manifest-path system-backend\control-room\ui\Cargo.toml

target\release\netcore-control-room-ui.exe --config "%APPDATA%\netcore\control-room\operator.toml" --profile default
```

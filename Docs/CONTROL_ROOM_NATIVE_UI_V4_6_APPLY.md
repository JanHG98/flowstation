# NetCore Control Room Native UI v4.6 – Aufräumen + Directory

Dieser Stand baut auf v4.5 auf.

## Änderung

- `Standorte` und `Karte` zeigen weiterhin pro ISSI nur den aktuellsten Standort. Alte Zombie-Positionen werden in der UI ausgeblendet.
- `Teilnehmer` wird bereinigt:
  - Basisstation/Infrastruktur/Gateway werden standardmäßig ausgeblendet.
  - doppelte/veraltete Einträge pro ISSI werden ausgeblendet.
  - Einträge ohne echte ISSI oder mit ISSI 0 werden nicht als Funkgerät dargestellt.
- Neues lokales UI-Directory in `operator.toml`:
  - Teilnehmernamen
  - Gerätetypen/Klassen
  - statische Gruppen
  - Gruppennamen
  - Statuslabels
  - Statusgruppen
- Teilnehmer, Gruppen, Rufe, Standorte und Marker-Details nutzen das Directory zur Anzeige.
- Zahlenstatus wird nicht mehr roh als nackte Nummer dargestellt. Wenn kein Directory-Label existiert, steht z. B. `Statuscode 1` oder `ESM aktiv` statt nur `1`.

## Beispiel Directory

```toml
[directory]
hide_infrastructure = true

[directory.subscribers."2010002"]
name = "Jan HRT"
device_class = "HRT"
status = "Einsatzbereit"
status_group = "crew"
groups = [15201, 15205]

[directory.groups."15205"]
name = "Tactical"
kind = "Sprechgruppe"

[directory.status_groups."crew"]
name = "Crew-Status"

[directory.statuses."1"]
label = "Frei / bereit"
group = "crew"
```

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

ZIP entpacken:

```cmd
powershell -NoProfile -Command "Expand-Archive -Force '%USERPROFILE%\Downloads\netcore-control-room-native-ui-v4-6-directory-cleanup-files.zip' -DestinationPath '%CD%'"
```

Prüfen:

```cmd
findstr /S /N /I "Native UI v4.6 DirectoryConfig clean_subscriber_rows" system-backend\control-room\ui\src\main.rs
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
Native UI v4.6 · aufgeräumte Teilnehmer + Directory
```

- Teilnehmer enthält keine Basisstation mehr.
- Teilnehmer zeigt Name/Typ/Status/Statusgruppe/Gruppen statt rohe Zahlenwüste.
- Standorte und Karte zeigen je ISSI nur den aktuellen Punkt.
- Marker-Klick zeigt Name/Typ/Status/Gruppen, wenn Directory/Subscribers Daten liefern.

LXC und TBS bleiben unverändert.

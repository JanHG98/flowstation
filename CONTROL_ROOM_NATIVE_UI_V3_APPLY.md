# NetCore Control Room Native UI v3 – echte OS-Fenster + Karte

Dieser Stand ersetzt die bisherige Native UI v1/v2 vollständig.

## Zielarchitektur

- Basisstation/TBS: bleibt Funkknoten, behält ihren Node-Token in `config.toml`.
- Control-Room-LXC: bleibt headless, nur Core-Service, SQLite und CLI.
- Windows-Operator-PC: bekommt die grafische Native UI.

## Neu in v3

- sichtbarer Versionshinweis `Native UI v3 · echte OS-Fenster`
- Multi-Window nicht mehr nur als egui-In-App-Fenster
- Module werden als echte Betriebssystem-Fenster geöffnet
- Fenster können über mehrere Monitore verteilt werden
- Kartenmodul bleibt enthalten
- Buttons heißen jetzt bewusst `OS-Fenster`
- Update-Anleitung enthält das Löschen alter Builds/EXEs

## Windows Update von alter UI-Version

Voraussetzung: Du bist in CMD im Repo-Root.

### 1. Alte UI schließen

```cmd
taskkill /IM netcore-control-room-ui.exe /F
```

Wenn kein Prozess gefunden wird, ist das okay.

### 2. Alte UI-Builds löschen

```cmd
cargo clean --manifest-path system-backend\control-room\ui\Cargo.toml
rmdir /S /Q system-backend\control-room\ui\target
rmdir /S /Q target\release\deps
```

Wenn ein Ordner nicht existiert, ist das okay.

Optional gezielt alte UI-EXEs löschen:

```cmd
del /F /Q target\release\netcore-control-room-ui.exe
del /F /Q system-backend\control-room\ui\target\release\netcore-control-room-ui.exe
```

### 3. ZIP entpacken

Wenn die ZIP im Downloads-Ordner liegt:

```cmd
powershell -NoProfile -Command "Expand-Archive -Force '%USERPROFILE%\Downloads\netcore-control-room-native-ui-v3-real-os-windows-map-files.zip' -DestinationPath '%CD%'"
```

### 4. Prüfen, ob v3 wirklich im Quellcode liegt

```cmd
findstr /S /N /I "Native UI v3 OS-Fenster show_viewport_immediate" system-backend\control-room\ui\src\main.rs
```

Du solltest Treffer sehen.

### 5. UI neu bauen

```cmd
cargo build --release --manifest-path system-backend\control-room\ui\Cargo.toml
```

### 6. Neuste EXE finden

```cmd
powershell -NoProfile -Command "Get-ChildItem -Recurse -Filter netcore-control-room-ui.exe | Sort-Object LastWriteTime -Descending | Select-Object LastWriteTime,FullName"
```

Die neuste EXE starten.

Meistens:

```cmd
target\release\netcore-control-room-ui.exe --config "%APPDATA%\netcore\control-room\operator.toml" --profile default
```

oder:

```cmd
system-backend\control-room\ui\target\release\netcore-control-room-ui.exe --config "%APPDATA%\netcore\control-room\operator.toml" --profile default
```

## Bedienung v3

Links im Modulbereich:

- `OS-Fenster-Modus` aktivieren
- `↗` neben einem Modul öffnet dieses Modul als echtes OS-Fenster
- `▣` zeigt ein bereits geöffnetes OS-Fenster
- `Alle Module als OS-Fenster öffnen` verteilt alle Module in eigene Fenster
- `Alle OS-Fenster schließen` schließt die Modulfenster

Die Fenster können frei über mehrere Monitore gezogen werden.

## Erwartung

In der Kopfzeile muss stehen:

```text
Native UI v3 · echte OS-Fenster
```

Wenn das nicht sichtbar ist, läuft noch eine alte EXE.

## LXC und TBS

Auf dem LXC ist für dieses Update nichts an der UI zu tun.

Kontrolle:

```bash
systemctl status netcore-control-room --no-pager -l
curl -i http://127.0.0.1:9010/health
curl -i http://127.0.0.1:9010/api/overview
```

Erwartung:

- `/health` ohne Token: `200 OK`
- `/api/overview` ohne Token: `401 Unauthorized`

Die TBS behält den Token in ihrer `config.toml`:

```toml
[control_room]
token = "..."
```

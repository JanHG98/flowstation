# NetCore Control Room v5.4 – responsive UI cleanup

Dieser Stand baut auf v5.3 auf.

## Enthalten

- Login-Maske als zentrierte responsive Karte statt über die ganze Breite gezogener Mini-Felder
- größere Textfelder und Buttons
- responsive Kopfzeile mit sauberem Wrapping
- resizable Seitenleiste
- Seitenleiste mit ScrollArea, damit kleine Fenster nicht kaputt aussehen
- Hauptbereich mit ScrollArea
- Tabellen mit beidseitigem Scrollen und besserer Mindestspaltenbreite
- Befehlsfelder passen sich der Seitenleistenbreite an
- Admin-User-Felder größer und bedienbarer
- Versionslabel: `Native UI v5.4 · responsives Layout · Login/RBAC`

## Architektur

- TBS bleibt unverändert und nutzt weiterhin den Node-Token in `config.toml`.
- LXC bleibt headless und läuft weiter als Core/SQLite/Auth/RBAC.
- Windows ist der Operator-Client mit klassischem User+Passwort-Login.

## Update kurz

Windows:

```cmd
taskkill /IM netcore-control-room-ui.exe /F
cargo clean --manifest-path system-backend\control-room\ui\Cargo.toml
rmdir /S /Q system-backend\control-room\ui\target
del /F /Q target\release\netcore-control-room-ui.exe
del /F /Q system-backend\control-room\ui\target\release\netcore-control-room-ui.exe
powershell -NoProfile -Command "Get-ChildItem -Recurse -Filter netcore-control-room-ui.exe | Remove-Item -Force"
powershell -NoProfile -Command "Expand-Archive -Force '%USERPROFILE%\Downloads\netcore-control-room-v5-4-responsive-ui-files.zip' -DestinationPath '%CD%'"
findstr /S /N /I "Native UI v5.4" system-backend\control-room\ui\src\main.rs
cargo build --release --manifest-path system-backend\control-room\ui\Cargo.toml
```

Start:

```cmd
target\release\netcore-control-room-ui.exe --config "%APPDATA%\netcore\control-room\operator.toml" --profile default
```

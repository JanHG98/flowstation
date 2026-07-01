# NetCore Control Room UI

Native Desktop-UI für den NetCore Control Room.

## Build unter Windows

```cmd
cargo build --release --manifest-path system-backend\control-room\ui\Cargo.toml
```

## Start unter Windows

```cmd
system-backend\control-room\ui\target\release\netcore-control-room-ui.exe --config "%APPDATA%\netcore\control-room\operator.toml" --profile default
```

## Features

- Übersicht über Basisstationen
- Teilnehmer, Gruppen, Rufe, SDS, Commands/Audit
- Admin-/Tokenverwaltung
- Befehle: Kick, DGNA Attach/Detach, Emergency Clear
- Multi-Window-Modus: jedes Modul als eigenes Fenster
- Offline-Karte für `/api/locations`

Die UI ist ein Client. Der Control-Room-Core bleibt auf dem LXC.

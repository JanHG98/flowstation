# NetCore Control Room UI v5.7 – Responsive Layout Fix

Dieses Paket behebt die Responsive-Regressions aus v5.6.

## Enthalten

- kompakter, wieder dynamischer Header statt starrem Top-Bereich
- Ribbon/Toolbar bricht sauber um
- kleinere und besser passende Ribbon-Buttons
- Seitenleiste mit sinnvollerer Breite je Fenstergröße
- Statusleiste und Directory-Hinweis bleiben sichtbar, ohne den Inhalt zu zerdrücken
- Kartenansicht bekommt wieder mehr nutzbare Fläche
- RBAC/User+Passwort bleibt unverändert erhalten

## Windows – Schnellupdate

1. UI schließen.
2. Alte EXE löschen.
3. ZIP in dein Repo entpacken und vorhandene Dateien überschreiben.
4. Neu bauen:

```cmd
cargo clean --manifest-path system-backend\control-room\ui\Cargo.toml
cargo build --release --manifest-path system-backend\control-room\ui\Cargo.toml
```

5. Starten:

```cmd
target\release\netcore-control-room-ui.exe --config "%APPDATA%\netcore\control-room\operator.toml" --profile default
```

Wenn du eine ältere UI-EXE an einem anderen Ort startest, zuerst löschen, sonst landest du wieder im alten Stand.

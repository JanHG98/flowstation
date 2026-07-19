# NetCore Control Room UI v5.10 – Kartencluster + Spiderfy

Dieses Paket räumt die Kartenmarker auf, wenn mehrere Geräte sehr nah beieinander liegen.

## Änderungen

- Marker-Clustering in der Live-Karte
- mehrere Geräte auf einem Fleck werden als Cluster-Kreis mit Anzahl dargestellt
- Klick auf Cluster fächert die Geräte radial auf (Spiderfy)
- Klick auf aufgefächerten Einzelmarker wählt genau dieses Gerät
- Detailkarte zeigt bei Cluster eine Geräteliste
- Hover über Cluster zeigt die enthaltenen Geräte
- Einzelmarker nutzen weiterhin Directory-Namen/ISSI
- Zombie-Standorte bleiben weiterhin draußen, weil nur aktuelle Positionen pro ISSI genutzt werden
- Clean-Workspace-UI, Login/RBAC und OS-Fenster bleiben erhalten

## Verhalten

- 1 Gerät: normaler grüner Marker
- mehrere Geräte nah beieinander: blauer Cluster mit Zahl
- Cluster anklicken: Geräte werden sternförmig aufgefächert
- aufgefächerten Punkt anklicken: Gerätedetails
- ausgewähltes Gerät: gelber Marker

## Windows Update

```cmd
taskkill /IM netcore-control-room-ui.exe /F

cargo clean --manifest-path system-backend\control-room\ui\Cargo.toml
rmdir /S /Q system-backend\control-room\ui\target
del /F /Q target\release\netcore-control-room-ui.exe
del /F /Q system-backend\control-room\ui\target\release\netcore-control-room-ui.exe

powershell -NoProfile -Command "Get-ChildItem -Recurse -Filter netcore-control-room-ui.exe | Remove-Item -Force"

powershell -NoProfile -Command "Expand-Archive -Force '%USERPROFILE%\Downloads\netcore-control-room-v5-10-map-cluster-spiderfy-files.zip' -DestinationPath '%CD%'"

cargo build --release --manifest-path system-backend\control-room\ui\Cargo.toml

target\release\netcore-control-room-ui.exe --config "%APPDATA%\netcore\control-room\operator.toml" --profile default
```

LXC/TBS müssen dafür nicht geändert werden, solange v5.x dort läuft.

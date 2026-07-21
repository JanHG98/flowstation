# Dashboard: Deutsch fest + versteckte Labor-Integrationen

## Enthaltene Änderungen

- Deutsch ist fest als einzige auswählbare Dashboard-Sprache gesetzt.
- Die Sprachauswahl wurde aus der Topbar entfernt.
- Die Login-Seite ist auf Deutsch umgestellt.
- DAPNET, EchoLink, MeshCom und GeoAlarm sind im normalen Dashboard vollständig ausgeblendet.
- Die vier Integrationen erscheinen im normalen Modus auch nicht mehr in der Health-Übersicht und werden dort nicht abgefragt.
- Backend, Konfiguration und API-Routen bleiben unverändert erhalten.

## Versteckten Integrationsmodus öffnen

Die vier Integrationen werden nur für den aktuellen Browser-Tab eingeblendet.

Alle versteckten Menüpunkte aktivieren und direkt DAPNET öffnen:

```text
http://<FLOWSTATION-IP>:<PORT>/?intern=netcore&modul=dapnet
```

Direkt verfügbare Module:

```text
?intern=netcore&modul=dapnet
?intern=netcore&modul=echolink
?intern=netcore&modul=meshcom
?intern=netcore&modul=geoalarm
```

Nach dem Aufruf wird der geheime Parameter aus der Adresszeile entfernt. Die Menüpunkte bleiben für den aktuellen Tab sichtbar. Ein neuer Tab startet wieder im normalen, versteckten Zustand.

Versteckten Modus im aktuellen Tab sofort abschalten:

```text
?intern=off
```

> Hinweis: Das ist bewusst nur eine versteckte Bedienoberfläche und keine Zugriffskontrolle. Die vorhandenen API-Endpunkte werden dadurch nicht gesperrt.

## Installation als vollständige Ersatzdatei

### 1. FlowStation stoppen

```bash
sudo systemctl stop tetra.service
```

### 2. In das Repository wechseln

```bash
cd ~/flowstation
```

### 3. Vorhandene Datei sichern und entfernen

```bash
cp crates/tetra-entities/src/net_dashboard/html.rs \
   crates/tetra-entities/src/net_dashboard/html.rs.backup-$(date +%Y%m%d-%H%M%S)

rm -f crates/tetra-entities/src/net_dashboard/html.rs
```

### 4. Neue vollständige Datei kopieren

Aus dem Austauschpaket:

```bash
cp /PFAD/ZUM/PAKET/crates/tetra-entities/src/net_dashboard/html.rs \
   ~/flowstation/crates/tetra-entities/src/net_dashboard/html.rs
```

### 5. Alte Build-Artefakte vollständig entfernen

```bash
cd ~/flowstation
cargo clean
rm -rf target
```

### 6. Neu bauen

```bash
cargo build --release --features asterisk
```

### 7. Dienst starten

```bash
sudo systemctl start tetra.service
```

### 8. Status und Log prüfen

```bash
sudo systemctl status tetra.service --no-pager
sudo journalctl -u tetra.service -n 150 --no-pager
```

### 9. Browser-Cache umgehen

Dashboard mit `Strg` + `F5` neu laden oder den Cache für die FlowStation-Adresse löschen.

## Rückbau

```bash
sudo systemctl stop tetra.service
cd ~/flowstation
rm -f crates/tetra-entities/src/net_dashboard/html.rs
cp crates/tetra-entities/src/net_dashboard/html.rs.backup-YYYYMMDD-HHMMSS \
   crates/tetra-entities/src/net_dashboard/html.rs
cargo clean
rm -rf target
cargo build --release --features asterisk
sudo systemctl start tetra.service
```

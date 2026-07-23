# SWMI Core 1 – Paket E einspielen

## 1. Dienste stoppen

```bash
sudo systemctl stop netcore-recorder.service 2>/dev/null || true
sudo systemctl stop netcore-media-switch.service 2>/dev/null || true
```

Ein laufender Funkruf sollte für das Upgrade vermieden werden. Der Media Switch kann zwar unabhängig neu starten, sein In-Memory-Tap-Ring geht dabei jedoch verloren.

## 2. Konfigurationen und Recorder-Daten sichern

```bash
sudo mkdir -p /root/netcore-backup
sudo cp -a /etc/netcore /root/netcore-backup/etc-netcore-$(date +%F-%H%M%S) 2>/dev/null || true
sudo cp -a /var/lib/netcore-recorder /root/netcore-backup/recorder-$(date +%F-%H%M%S) 2>/dev/null || true
```

## 3. Repository vollständig ersetzen

Das ZIP in ein neues Verzeichnis entpacken und nicht dateiweise über eine alte Arbeitskopie kopieren.

```bash
cd /opt
sudo mv netcore-tetra netcore-tetra.old-$(date +%F-%H%M%S) 2>/dev/null || true
sudo unzip /pfad/netcore-tetra-swmi-recorder-lxc-open-lab.zip
sudo mv netcore-tetra-swmi netcore-tetra
sudo chown -R "$USER":"$USER" /opt/netcore-tetra
cd /opt/netcore-tetra
```

## 4. Statische Prüfungen

```bash
python3 tools/check_recorder.py
python3 tools/check_media_switch.py
python3 tools/check_call_control.py
python3 tools/check_node_gateway.py
python3 tools/protocol_inventory.py --check
```

## 5. Rust-Prüfungen

```bash
cargo fmt --all -- --check
cargo test -p netcore-media-switch
cargo test -p netcore-recorder
cargo clippy -p netcore-media-switch -p netcore-recorder --all-targets -- -D warnings
```

## 6. Media Switch aktualisieren

Die neue Version muss zuerst laufen, da nur sie den Vollframe-Tap bereitstellt.

```bash
sudo system-backend/media-switch/install/update.sh
curl 'http://127.0.0.1:8130/api/v1/recorder/taps?after=0&limit=1'
```

In `/etc/netcore/media-switch.toml` kann die Ringgröße festgelegt werden:

```toml
[media]
recorder_tap_history_frames = 20000
```

## 7. Recorder konfigurieren

Im neuen Recorder-LXC:

```bash
sudo install -d /etc/netcore
sudo cp system-backend/recorder/config/recorder.example.toml /etc/netcore/recorder.toml
sudo nano /etc/netcore/recorder.toml
```

Mindestens anpassen:

```toml
[media_switch]
tap_url = "http://10.0.1.25:8130/api/v1/recorder/taps"
sessions_url = "http://10.0.1.25:8130/api/v1/sessions"

[storage]
root = "/var/lib/netcore-recorder/recordings"
export_root = "/var/lib/netcore-recorder/exports"
```

## 8. Recorder installieren

```bash
sudo system-backend/recorder/install/install.sh
```

## 9. Kontrolle

```bash
systemctl status netcore-recorder --no-pager
journalctl -u netcore-recorder -n 150 --no-pager
curl http://127.0.0.1:8140/health/live
curl http://127.0.0.1:8140/health/ready
curl http://127.0.0.1:8140/api/v1/status
curl http://127.0.0.1:8140/api/v1/active
```

WebUI:

```text
http://<RECORDER-LXC-IP>:8140/
```

`/health/ready` wird erst grün, wenn der Media Switch erreichbar und genügend freier Speicher vorhanden ist.

## 10. Funktionstest

1. Gruppenruf aufbauen.
2. Mindestens zwei Sprecherwechsel durchführen.
3. Ruf beenden.
4. Aufnahme in der WebUI öffnen.
5. GSSI, Sprecher, Framezahl und Quell-TBS prüfen.
6. Integritätsprüfung auslösen.
7. TAR exportieren und Inhalt kontrollieren.

## 11. Startreihenfolge

```text
1. Node Gateway
2. Subscriber / Group / Mobility Core
3. Call Control
4. Media Switch
5. Recorder
6. TBS
```

Der Recorder reconnectet automatisch. Der Media Switch darf niemals vom Recorder-Status abhängig gemacht werden.

# SWMI Core 1 – Paket D einspielen

## 1. Dienste stoppen

Auf der TBS beziehungsweise den bisherigen Backend-LXC:

```bash
sudo systemctl stop tetra.service 2>/dev/null || true
sudo systemctl stop netcore-node-gateway.service 2>/dev/null || true
sudo systemctl stop netcore-call-control.service 2>/dev/null || true
sudo systemctl stop netcore-media-switch.service 2>/dev/null || true
```

## 2. Konfigurationen sichern

```bash
sudo mkdir -p /root/netcore-backup
sudo cp -a /etc/netcore /root/netcore-backup/etc-netcore-$(date +%F-%H%M%S) 2>/dev/null || true
sudo cp -a /etc/flowstation /root/netcore-backup/etc-flowstation-$(date +%F-%H%M%S) 2>/dev/null || true
```

## 3. Altes Repository vollständig ersetzen

Nicht einzelne Dateien zusammenkopieren. Das ZIP in ein neues Verzeichnis entpacken und das bisherige Repository vollständig ersetzen. Konfigurationen außerhalb des Repositories bleiben erhalten.

```bash
cd /opt
sudo mv netcore-tetra netcore-tetra.old-$(date +%F-%H%M%S) 2>/dev/null || true
sudo unzip /pfad/netcore-tetra-swmi-core1-package-d-media-switch-open-lab.zip
sudo mv netcore-tetra-main netcore-tetra
sudo chown -R "$USER":"$USER" /opt/netcore-tetra
cd /opt/netcore-tetra
```

## 4. Alte Build-Artefakte und Binaries entfernen

```bash
rm -rf target
cargo clean
sudo rm -f /usr/local/bin/netcore-media-switch
```

## 5. Statische Prüfungen

```bash
python3 tools/check_media_switch.py
python3 tools/check_call_control.py
python3 tools/check_group_core.py
python3 tools/check_node_gateway.py
python3 tools/protocol_inventory.py --check
```

## 6. Rust-Tests

```bash
cargo fmt --all -- --check
cargo test -p tetra-entities --test test_media_bridge
cargo test -p netcore-media-switch
```

## 7. TBS neu bauen

Der TBS-Build enthält den lokalen UMAC-Media-Bridge-Pfad:

```bash
cargo build --release --features asterisk -p bluestation-bs
```

Vor dem Start sicherstellen, dass `[control_room]` auf den Node Gateway zeigt und im offenen Labormodus keine Credentials benötigt:

```toml
[control_room]
enabled = true
host = "10.0.1.20"
port = 8080
use_tls = false
endpoint_path = "/ws/node"
```

## 8. Media-Switch-LXC konfigurieren

```bash
sudo install -d /etc/netcore
sudo cp system-backend/media-switch/config/media-switch.example.toml /etc/netcore/media-switch.toml
sudo nano /etc/netcore/media-switch.toml
```

Mindestens anpassen:

```toml
[node_gateway]
url = "ws://10.0.1.20:8080/ws/backend"

[call_control]
url = "http://10.0.1.24:8120/api/v1/calls"
```

## 9. Installieren

```bash
sudo system-backend/media-switch/install/install.sh
```

Das Skript stoppt den alten Dienst, löscht alte Binaries und Workspace-Artefakte, führt `cargo clean` aus, baut neu und installiert die systemd-Unit.

## 10. Kontrolle

```bash
systemctl status netcore-media-switch --no-pager
journalctl -u netcore-media-switch -n 150 --no-pager
curl http://127.0.0.1:8130/health/live
curl http://127.0.0.1:8130/health/ready
curl http://127.0.0.1:8130/api/v1/status
curl http://127.0.0.1:8130/api/v1/sessions
```

WebUI:

```text
http://<MEDIA-SWITCH-LXC-IP>:8130/
```

## 11. Startreihenfolge

```text
1. Node Gateway
2. Subscriber Core / Group Core / Mobility Core
3. Call Control
4. Media Switch
5. TBS
```

Der Media Switch verbindet sich bei anderer Reihenfolge automatisch erneut, aber diese Reihenfolge macht die Diagnose übersichtlicher.

## 12. Rollback

```bash
sudo systemctl stop netcore-media-switch
sudo rm -f /usr/local/bin/netcore-media-switch
cd /opt
sudo mv netcore-tetra netcore-tetra.failed-$(date +%F-%H%M%S)
sudo mv netcore-tetra.old-<ZEITSTEMPEL> netcore-tetra
```

Danach die vorherigen Binaries aus dem alten Repository neu bauen oder aus einer gesicherten Installation zurückkopieren.

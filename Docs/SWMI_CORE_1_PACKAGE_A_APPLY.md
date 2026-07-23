# Einspielen – Subscriber Core

## 1. Bestehende Konfiguration und Daten sichern

```bash
sudo systemctl stop netcore-subscriber-core.service 2>/dev/null || true
sudo cp -a /etc/netcore/subscriber-core.toml /root/subscriber-core.toml.bak 2>/dev/null || true
sudo cp -a /var/lib/netcore-subscriber-core /root/subscriber-core-data.bak 2>/dev/null || true
```

## 2. Altes Repository und alte Build-Artefakte entfernen

```bash
cd /opt
sudo rm -rf netcore-tetra-old
sudo mv netcore-tetra netcore-tetra-old 2>/dev/null || true
sudo mkdir -p netcore-tetra
# ZIP nach /opt/netcore-tetra entpacken bzw. Repo-Inhalt dorthin kopieren
cd /opt/netcore-tetra
rm -rf target
cargo clean
```

## 3. Konfiguration anpassen

In `system-backend/subscriber-core/config/subscriber-core.example.toml` die Node-Gateway-IP setzen und nach `/etc/netcore/subscriber-core.toml` übernehmen.

## 4. Installieren

```bash
sudo system-backend/subscriber-core/install/install.sh
```

## 5. Prüfen

```bash
systemctl status netcore-subscriber-core --no-pager
journalctl -u netcore-subscriber-core -n 100 --no-pager
curl http://127.0.0.1:8100/health/live
curl http://127.0.0.1:8100/api/v1/status
```

WebUI: `http://<LXC-IP>:8100/`

## 6. Clean-Build des Gesamtprojekts

```bash
rm -rf target
cargo clean
cargo test --workspace
cargo build --release --workspace
```

## Rollback

Dienst stoppen, neues Repo entfernen, `/opt/netcore-tetra-old` zurückbenennen und gesicherte Konfiguration/Daten wiederherstellen.

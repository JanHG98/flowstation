# Einspielen von SWMI Core 1 – Paket B

## 1. Sicherung

```bash
cd /opt
sudo tar -czf netcore-tetra-backup-$(date +%F-%H%M%S).tar.gz netcore-tetra
sudo cp -a /etc/netcore /etc/netcore.backup-$(date +%F-%H%M%S) 2>/dev/null || true
sudo cp -a /var/lib/netcore-group-core /var/lib/netcore-group-core.backup-$(date +%F-%H%M%S) 2>/dev/null || true
```

## 2. Laufende Dienste stoppen

```bash
sudo systemctl stop netcore-group-core 2>/dev/null || true
sudo systemctl stop netcore-subscriber-core 2>/dev/null || true
sudo systemctl stop netcore-mobility-core 2>/dev/null || true
sudo systemctl stop netcore-node-gateway 2>/dev/null || true
sudo systemctl stop tetra 2>/dev/null || true
```

## 3. Altes Repository und Build-Artefakte entfernen

Das vollständige ZIP ersetzt das bisherige Repository. Konfigurations- und Datenverzeichnisse außerhalb des Repos bleiben erhalten.

```bash
cd /opt
sudo rm -rf netcore-tetra.old
sudo mv netcore-tetra netcore-tetra.old
sudo unzip netcore-tetra-swmi-core1-package-b-group-core-open-lab.zip
sudo mv netcore-tetra-main netcore-tetra
sudo chown -R "$USER":"$USER" /opt/netcore-tetra
cd /opt/netcore-tetra
rm -rf target
cargo clean
```

## 4. Tests und vollständiger Build

```bash
cargo fmt --all -- --check
cargo test -p tetra-config
cargo test -p tetra-entities
cargo test -p netcore-node-gateway
cargo test -p netcore-mobility-core
cargo test -p netcore-subscriber-core
cargo test -p netcore-group-core
cargo build --release -p bluestation-bs -p netcore-node-gateway -p netcore-mobility-core -p netcore-subscriber-core -p netcore-group-core
```

## 5. Group-Core-LXC installieren

Im neuen Group-Core-LXC:

```bash
cd /opt/netcore-tetra
sudo cp system-backend/group-core/config/group-core.example.toml /etc/netcore/group-core.toml
sudo nano /etc/netcore/group-core.toml
sudo system-backend/group-core/install/install.sh
```

Im Konfigurationsfile die Adresse des Node Gateway eintragen:

```toml
[node_gateway]
url = "ws://10.0.1.XX:8080/ws/backend"
```

## 6. Startreihenfolge

```bash
sudo systemctl restart netcore-node-gateway
sudo systemctl restart netcore-mobility-core
sudo systemctl restart netcore-subscriber-core
sudo systemctl restart netcore-group-core
sudo systemctl restart tetra
```

## 7. Kontrolle

```bash
systemctl status netcore-group-core --no-pager
journalctl -u netcore-group-core -n 150 --no-pager
curl http://127.0.0.1:8110/health/live
curl http://127.0.0.1:8110/api/v1/status
```

WebUI:

```text
http://<GROUP-CORE-LXC-IP>:8110/
```

## 8. Rollback

```bash
sudo systemctl stop netcore-group-core netcore-subscriber-core netcore-mobility-core netcore-node-gateway tetra
cd /opt
sudo rm -rf netcore-tetra
sudo mv netcore-tetra.old netcore-tetra
cd /opt/netcore-tetra
rm -rf target
cargo clean
cargo build --release --features asterisk
sudo systemctl restart tetra
```

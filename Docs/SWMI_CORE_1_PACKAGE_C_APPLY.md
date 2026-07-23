# Einspielen von SWMI Core 1 – Paket C

## 1. Sicherung

```bash
cd /opt
sudo tar -czf netcore-tetra-backup-$(date +%F-%H%M%S).tar.gz netcore-tetra
sudo cp -a /etc/netcore /etc/netcore.backup-$(date +%F-%H%M%S) 2>/dev/null || true
sudo cp -a /var/lib/netcore-call-control /var/lib/netcore-call-control.backup-$(date +%F-%H%M%S) 2>/dev/null || true
```

## 2. Laufende Dienste stoppen

```bash
sudo systemctl stop netcore-call-control 2>/dev/null || true
sudo systemctl stop netcore-group-core 2>/dev/null || true
sudo systemctl stop netcore-subscriber-core 2>/dev/null || true
sudo systemctl stop netcore-mobility-core 2>/dev/null || true
sudo systemctl stop netcore-node-gateway 2>/dev/null || true
sudo systemctl stop tetra 2>/dev/null || true
```

## 3. Altes Repository und Build-Artefakte entfernen

Das vollständige ZIP ersetzt das bisherige Repository. Konfigurationen und Daten außerhalb des Repos bleiben erhalten.

```bash
cd /opt
sudo rm -rf netcore-tetra.old
sudo mv netcore-tetra netcore-tetra.old
sudo unzip netcore-tetra-swmi-core1-package-c-call-control-open-lab.zip
sudo mv netcore-tetra-main netcore-tetra
sudo chown -R "$USER":"$USER" /opt/netcore-tetra
cd /opt/netcore-tetra
rm -rf target
cargo clean
```

## 4. Statische Paketprüfungen

```bash
python3 tools/check_swmi_foundation_types.py
python3 tools/check_tlmc_runtime.py
python3 tools/check_ltpd_runtime.py
python3 tools/check_foundation_acceptance.py
python3 tools/check_mle_cell_change.py
python3 tools/check_cmce_call_restore.py
python3 tools/check_mm_mobility.py
python3 tools/check_node_gateway.py
python3 tools/check_mobility_core.py
python3 tools/check_subscriber_core.py
python3 tools/check_group_core.py
python3 tools/check_call_control.py
python3 tools/protocol_inventory.py --check
```

## 5. Tests und vollständiger Build

```bash
cargo fmt --all -- --check
cargo test -p tetra-config
cargo test -p tetra-entities
cargo test -p netcore-node-gateway
cargo test -p netcore-mobility-core
cargo test -p netcore-subscriber-core
cargo test -p netcore-group-core
cargo test -p netcore-call-control
cargo build --release \
  -p bluestation-bs \
  -p netcore-node-gateway \
  -p netcore-mobility-core \
  -p netcore-subscriber-core \
  -p netcore-group-core \
  -p netcore-call-control
```

## 6. Call-Control-LXC installieren

Im neuen LXC:

```bash
cd /opt/netcore-tetra
sudo install -d -m 0755 /etc/netcore
sudo cp system-backend/call-control/config/call-control.example.toml /etc/netcore/call-control.toml
sudo nano /etc/netcore/call-control.toml
sudo system-backend/call-control/install/install.sh
```

Node Gateway eintragen:

```toml
[node_gateway]
url = "ws://10.0.1.XX:8080/ws/backend"
```

## 7. Startreihenfolge

```bash
sudo systemctl restart netcore-node-gateway
sudo systemctl restart netcore-mobility-core
sudo systemctl restart netcore-subscriber-core
sudo systemctl restart netcore-group-core
sudo systemctl restart netcore-call-control
sudo systemctl restart tetra
```

## 8. Kontrolle

```bash
systemctl status netcore-call-control --no-pager
journalctl -u netcore-call-control -n 150 --no-pager
curl http://127.0.0.1:8120/health/live
curl http://127.0.0.1:8120/api/v1/status
curl http://127.0.0.1:8120/api/v1/nodes
```

WebUI:

```text
http://<CALL-CONTROL-LXC-IP>:8120/
```

## 9. Rollback

```bash
sudo systemctl stop netcore-call-control netcore-group-core netcore-subscriber-core netcore-mobility-core netcore-node-gateway tetra
cd /opt
sudo rm -rf netcore-tetra
sudo mv netcore-tetra.old netcore-tetra
cd /opt/netcore-tetra
rm -rf target
cargo clean
cargo build --release --features asterisk
sudo systemctl restart tetra
```

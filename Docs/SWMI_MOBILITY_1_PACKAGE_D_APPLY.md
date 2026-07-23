# SWMI Mobility 1 – Paket D einspielen

## 1. Vorhandene Dienste stoppen

Auf dem bisherigen TBS-System:

```bash
sudo systemctl stop tetra.service
sudo systemctl stop netcore-control-room.service 2>/dev/null || true
```

Auf einem eventuell schon vorbereiteten Gateway-LXC:

```bash
sudo systemctl stop netcore-node-gateway.service 2>/dev/null || true
```

## 2. Konfiguration sichern

```bash
mkdir -p ~/netcore-backup
cp -a ~/netcore-tetra/config ~/netcore-backup/ 2>/dev/null || true
sudo cp -a /etc/netcore ~/netcore-backup/etc-netcore 2>/dev/null || true
```

## 3. Altes Repository vollständig ersetzen

Das neue ZIP entpacken und nicht nur einzelne Dateien über einen älteren Arbeitsbaum kopieren.

```bash
cd ~
rm -rf netcore-tetra-old
mv netcore-tetra netcore-tetra-old
mv netcore-tetra-main netcore-tetra
cd netcore-tetra
```

## 4. Alte Build-Artefakte entfernen

```bash
rm -rf target
cargo clean
```

## 5. Tests und Build

```bash
cargo fmt --all -- --check
cargo test -p netcore-node-gateway
cargo test --workspace
cargo build --release -p netcore-node-gateway
```

## 6. Gateway-LXC installieren

Im Gateway-LXC:

```bash
cd /opt/netcore-tetra
sudo system-backend/node-gateway/install/install.sh
```

Danach:

```bash
systemctl status netcore-node-gateway
journalctl -u netcore-node-gateway -f
curl http://127.0.0.1:8080/health/live
```

## 7. TBS auf den Gateway umstellen

In der TBS-Konfiguration den vorhandenen `[control_room]`-Block auf den neuen LXC setzen:

```toml
[control_room]
host = "10.0.1.XX"
port = 8080
use_tls = false
endpoint_path = "/ws/node"
```

Im offenen Testmodus werden keine Credentials und kein Token eingetragen.

## 8. TBS sauber neu bauen

```bash
sudo systemctl stop tetra.service
rm -f target/release/bluestation-bs
rm -rf target/release/deps/bluestation_bs-*
cargo build --release -p bluestation-bs --features asterisk
sudo systemctl start tetra.service
```

## 9. Kontrolle

WebUI:

```text
http://<Gateway-LXC-IP>:8080/
```

Logs:

```bash
journalctl -u netcore-node-gateway -f
journalctl -u tetra.service -f
```

Erwartet werden ein erfolgreicher WebSocket-Handshake, `HelloAck` und regelmäßige Heartbeats.

## 10. Sicherheitswarnung

Port 8080 darf nur im isolierten Test-/Managementnetz erreichbar sein. Jeder erreichbare Client kann die WebUI und API ohne Anmeldung verwenden und bei aktivem `allow_remote_management` Nodes trennen oder Kommandos senden.

## 11. Rollback

```bash
sudo systemctl stop tetra.service
cd ~
rm -rf netcore-tetra
mv netcore-tetra-old netcore-tetra
cd netcore-tetra
rm -rf target
cargo build --release -p bluestation-bs --features asterisk
sudo systemctl start tetra.service
```

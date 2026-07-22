# SWMI Mobility 1 – Paket A einspielen

## 1. Sicherung

```bash
cd ~
tar -czf netcore-tetra-backup-before-mobility1-a-$(date +%F-%H%M%S).tar.gz netcore-tetra
```

Produktive Konfigurationen und lokale Audio-/TTS-Dateien außerhalb des Repositories separat sichern.

## 2. Dienste stoppen

```bash
sudo systemctl stop tetra.service 2>/dev/null || true
sudo systemctl stop netcore-control-room.service 2>/dev/null || true
```

## 3. Gesamtarchiv entpacken

```bash
cd ~
unzip netcore-tetra-swmi-mobility1-package-a.zip
```

Das Archiv enthält das vollständige Repository unter:

```text
netcore-tetra-main/
```

## 4. Alten Stand vollständig ersetzen

Keine Einzeldateien über den alten Stand kopieren.

```bash
cd ~
rm -rf netcore-tetra.old
mv netcore-tetra netcore-tetra.old
mv netcore-tetra-main netcore-tetra
cd netcore-tetra
```

Danach die eigene produktive Konfiguration aus der Sicherung zurückkopieren.

## 5. Alte Build-Artefakte löschen

```bash
cd ~/netcore-tetra
rm -rf target
cargo clean
```

## 6. Statische Prüfungen

```bash
python3 tools/check_swmi_foundation_types.py
python3 tools/check_tlmc_runtime.py
python3 tools/check_ltpd_runtime.py
python3 tools/check_foundation_acceptance.py
python3 tools/check_mle_cell_change.py
python3 tools/protocol_inventory.py --check
bash tools/check_protocol_inventory.sh
```

## 7. Formatierung und gezielte Tests

```bash
cargo fmt --all -- --check
cargo test -p tetra-saps
cargo test -p tetra-pdus --test test_mle_cell_change_pdus
cargo test -p tetra-entities --test test_mle_cell_change_runtime
cargo test -p tetra-entities --test test_two_cell_foundation
cargo test -p tetra-entities --test test_two_cell_mobility
```

## 8. Workspace-Abnahme

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## 9. Vollständiger Clean Release Build

```bash
cargo build --release --features asterisk \
  -p bluestation-bs \
  -p netcore-control-room \
  -p netcore-control-room-operator
```

Alternativ der gesamte Workspace:

```bash
cargo build --release --features asterisk
```

## 10. Dienste starten

Prüfen, dass systemd auf die neu gebauten Binaries zeigt.

```bash
sudo systemctl daemon-reload
sudo systemctl start tetra.service
sudo systemctl status tetra.service --no-pager
```

Control Room bei Bedarf:

```bash
sudo systemctl start netcore-control-room.service
```

## 11. Laufzeitkontrolle

```bash
sudo journalctl -u tetra.service -f \
  | grep --line-buffered -iE 'U-PREPARE|D-NEW-CELL|PREPARE-FAIL|U-RESTORE|RESTORE-ACK|RESTORE-FAIL|CHANNEL-REQUEST|CHANNEL-RESPONSE|cell-change|timeout'
```

Im normalen Ein-Zellen-Betrieb dürfen keine Zellwechsel-PDUs ohne entsprechenden Uplink oder Steuerbefehl entstehen.

## Rückfall

```bash
sudo systemctl stop tetra.service 2>/dev/null || true
cd ~
rm -rf netcore-tetra
mv netcore-tetra.old netcore-tetra
cd netcore-tetra
rm -rf target
cargo clean
cargo build --release --features asterisk
sudo systemctl start tetra.service
```

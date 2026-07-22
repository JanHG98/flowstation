# SWMI Foundation 1 – Paket E einspielen

## 1. Sicherung

```bash
cd ~
tar -czf netcore-tetra-backup-before-package-e-$(date +%F-%H%M%S).tar.gz netcore-tetra
```

Produktive Konfigurationen außerhalb des Repositories zusätzlich sichern.

## 2. Dienste stoppen

```bash
sudo systemctl stop tetra.service 2>/dev/null || true
sudo systemctl stop netcore-control-room.service 2>/dev/null || true
```

## 3. Neues Gesamtarchiv entpacken

```bash
cd ~
unzip netcore-tetra-swmi-foundation1-package-e.zip
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

## 5. Alte Build-Artefakte entfernen

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
python3 tools/protocol_inventory.py --check
bash tools/check_protocol_inventory.sh
```

## 7. Formatierung und Pakettests

```bash
cargo fmt --all -- --check
cargo test -p tetra-saps
cargo test -p tetra-entities --lib mle::ltpd_runtime::tests
cargo test -p tetra-entities --test test_tlmc_runtime
cargo test -p tetra-entities --test test_ltpd_runtime
cargo test -p tetra-entities --test test_two_cell_foundation
```

## 8. Workspace-Abnahme

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## 9. Clean Release Build

Für den aktuell bevorzugten Stand:

```bash
cargo build --release --features asterisk \
  -p bluestation-bs \
  -p netcore-control-room \
  -p netcore-control-room-operator
```

Alternativ vollständig:

```bash
cargo build --release --features asterisk
```

## 10. Dienste starten

Vor dem Start prüfen, dass systemd nicht auf ein altes Binary außerhalb des neuen `target/release/` verweist.

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
  | grep --line-buffered -iE 'TLPD|TLMC|SNDCP|TxReporter|timeout|duplicate|cancel|reconnect|break|resume'
```

Erwartet werden:

- ausgehende Transfers bleiben bis zum realen TxReporter-Ergebnis pending;
- bestätigte Transfers erhalten `SuccessBufferEmpty`;
- verworfene, verlorene oder abgelaufene Transfers erhalten `FailedRemovedFromBuffer`;
- Duplicate Handles erzeugen keine zweite Aussendung;
- Break, Disable, Close und Release hinterlassen keine Pending-Transfers;
- keine Panic-Schleife bei wiederholten Cancel-/Reconnect-Anforderungen.

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

# SWMI Foundation 1 – Paket D einspielen

## 1. Sicherung

Auf der TBS beziehungsweise dem Build-System:

```bash
cd ~
tar -czf netcore-tetra-backup-before-package-d-$(date +%F-%H%M%S).tar.gz netcore-tetra
```

Zusätzliche produktive Konfigurationsdateien außerhalb des Repositories separat sichern.

## 2. Dienste stoppen

```bash
sudo systemctl stop tetra.service 2>/dev/null || true
sudo systemctl stop netcore-control-room.service 2>/dev/null || true
```

## 3. Neues Gesamtarchiv entpacken

Das ZIP enthält das vollständige Repository unter `netcore-tetra-main/`.

```bash
cd ~
unzip netcore-tetra-swmi-foundation1-package-d.zip
```

## 4. Altes Repository vollständig ersetzen

Nicht einzelne Dateien über den alten Stand kopieren. Dadurch könnten veraltete Rust-Module oder generierte Inventuren erhalten bleiben.

```bash
cd ~
rm -rf netcore-tetra.old
mv netcore-tetra netcore-tetra.old
mv netcore-tetra-main netcore-tetra
cd netcore-tetra
```

Die eigene produktive Konfiguration anschließend aus der Sicherung zurückkopieren.

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
python3 tools/protocol_inventory.py --check
```

## 7. Format und Tests

```bash
cargo fmt --all -- --check
cargo test -p tetra-saps
cargo test -p tetra-entities --test test_tlmc_runtime
cargo test -p tetra-entities --test test_ltpd_runtime
```

Danach der vollständige Workspace-Test:

```bash
cargo test --workspace
```

## 8. Clean Release Build

Für den bevorzugten aktuellen Stand:

```bash
cargo build --release --features asterisk \
  -p bluestation-bs \
  -p netcore-control-room \
  -p netcore-control-room-operator
```

Alternativ der vollständige Workspace-Build:

```bash
cargo build --release --features asterisk
```

## 9. Installation beziehungsweise Start

Die vorhandenen systemd-Installationspfade des Projekts verwenden. Vor dem Start kontrollieren, dass nicht versehentlich ein altes Binary außerhalb von `target/release/` gestartet wird.

```bash
sudo systemctl daemon-reload
sudo systemctl start tetra.service
sudo systemctl status tetra.service --no-pager
```

Control Room bei Bedarf:

```bash
sudo systemctl start netcore-control-room.service
```

Operator-Beispiel:

```bash
./target/release/netcore-control-room-operator \
  --api http://10.0.1.25:9010 dashboard
```

## 10. Runtime-Kontrolle

Logs beobachten:

```bash
sudo journalctl -u tetra.service -f \
  | grep --line-buffered -iE 'TLPD|SNDCP|MLE|packet|PDCH|break|resume|reconnect'
```

Erwartet werden:

- `LtpdMleOpenInd` beim Start;
- eingehende SNDCP-PDUs über MLE;
- ausgehende Antworten über `LtpdMleUnitdataReq` und MLE;
- Transfer Reports mit Handle;
- Break/Resume bei echten TLMC-Ressourcenkanten;
- keine direkte SNDCP→LLC-Abkürzung mehr.

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

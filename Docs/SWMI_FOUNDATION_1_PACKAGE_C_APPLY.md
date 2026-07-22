# Einspielen und Prüfen – SWMI Foundation 1 Paket C

## 1. Laufende Dienste stoppen

```bash
sudo systemctl stop tetra.service 2>/dev/null || true
sudo systemctl stop netcore-control-room.service 2>/dev/null || true
```

## 2. Lokale Konfiguration und Daten sichern

```bash
cp -a ~/netcore-tetra/config.toml ~/netcore-tetra-config.toml.backup 2>/dev/null || true
cp -a ~/netcore-tetra/system-backend ~/netcore-system-backend.backup 2>/dev/null || true
```

Audio-, Recording-, TTS-, NFS- und Datenbankdaten außerhalb des Repositorys zusätzlich entsprechend der lokalen Installation sichern.

## 3. Altes Repository vollständig ersetzen

Das ZIP enthält das vollständige Repository. Keine einzelnen Dateien aus verschiedenen Paketständen mischen.

```bash
cd ~
mv netcore-tetra netcore-tetra.before-foundation1-c
unzip netcore-tetra-swmi-foundation1-package-c.zip
mv netcore-tetra-main netcore-tetra
```

Anschließend die lokale Konfiguration zurückkopieren:

```bash
cp -a ~/netcore-tetra-config.toml.backup ~/netcore-tetra/config.toml 2>/dev/null || true
```

Lokale, nach Paket B vorgenommene Änderungen unter `system-backend/` nur gezielt übernehmen und nicht blind den neuen Stand überschreiben.

## 4. Alte Build-Artefakte vollständig löschen

```bash
cd ~/netcore-tetra
rm -rf target
cargo clean
```

Dieser Schritt ist verbindlich, damit keine alte Binary mit dem neuen Quellstand verwechselt wird.

## 5. Statische Paketprüfungen

```bash
python3 tools/check_swmi_foundation_types.py
python3 tools/check_tlmc_runtime.py
python3 tools/protocol_inventory.py --check
bash tools/check_protocol_inventory.sh
```

## 6. Rust-Tests

```bash
cargo test -p tetra-saps
cargo test -p tetra-entities --test test_tlmc_runtime
```

Danach den vollständigen Workspace prüfen:

```bash
cargo test --workspace
cargo clippy --workspace --all-targets
```

## 7. Vollständiger Release-Build

Bevorzugter Build des aktuellen Workspace:

```bash
cargo build --release --features asterisk
```

Explizite Binaries:

```bash
cargo build --release \
  --bin bluestation-bs \
  --bin netcore-control-room \
  --bin netcore-control-room-operator \
  --features bluestation-bs/asterisk
```

## 8. Dienst starten

```bash
sudo systemctl start tetra.service
sudo journalctl -u tetra.service -f
```

Control Room abhängig von der lokalen Installation anschließend separat starten.

## 9. Erwartetes Verhalten

Im normalen BS-Standalone-Betrieb sollen die bisherigen Ruf-, SDS-, Dual-Carrier-, Audio- und Control-Room-Funktionen unverändert bleiben.

Neu sind vor allem interne beziehungsweise testbare TLMC-Funktionen:

- Configure Confirm oder sauberer Reject;
- Measurement-/Monitor-Indications im MS-/Teststack;
- kantengetriggerter Ressourcenverlust und Recovery;
- korrelierte Scan-, Cell-Read- und Select-Zustände;
- negative Confirmations bei Timeout;
- kein unimplementierter TLMC-Routingpfad in UMAC oder MLE;
- Diagnose-Snapshot für spätere TBS-WebUI-Anbindung.

## 10. Logfilter

```bash
sudo journalctl -u tetra.service -f | grep --line-buffered -iE \
  'TLMC|resource|measurement|monitor|scan|cell read|select|reject|timeout'
```

## 11. Rückkehr zu Paket B

```bash
sudo systemctl stop tetra.service
cd ~
mv netcore-tetra netcore-tetra.failed-foundation1-c
mv netcore-tetra.before-foundation1-c netcore-tetra
cd ~/netcore-tetra
rm -rf target
cargo clean
cargo build --release --features asterisk
sudo systemctl start tetra.service
```

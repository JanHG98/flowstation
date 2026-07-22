# SWMI Mobility 1 – Paket C einspielen

## 1. Dienste stoppen und Sicherung erstellen

```bash
sudo systemctl stop tetra.service 2>/dev/null || true
sudo systemctl stop netcore-control-room.service 2>/dev/null || true
sudo systemctl stop netcore-control-room-operator.service 2>/dev/null || true

cd ~
tar -czf netcore-tetra-backup-before-mobility1-c-$(date +%F-%H%M%S).tar.gz netcore-tetra
```

Produktive Konfigurationen, Schlüssel, lokale Audio-/TTS-Dateien und nicht versionierte Daten separat sichern.

## 2. Archiv entpacken und alten Quellbaum vollständig ersetzen

```bash
cd ~
unzip netcore-tetra-swmi-mobility1-package-c-mm-mobility.zip

rm -rf netcore-tetra.old
mv netcore-tetra netcore-tetra.old
mv netcore-tetra-main netcore-tetra
cd netcore-tetra
```

Eigene produktive Konfiguration anschließend kontrolliert aus der Sicherung zurückkopieren. Keine einzelnen neuen Dateien über einen alten Quellbaum verteilen.

## 3. Alte Artefakte und Binaries entfernen

```bash
cd ~/netcore-tetra
rm -rf target
rm -f \
  target/release/bluestation-bs \
  target/release/netcore-control-room \
  target/release/netcore-control-room-operator
cargo clean
```

Damit kann kein veraltetes Binary versehentlich weiterlaufen.

## 4. Statische Prüfungen

```bash
python3 tools/check_swmi_foundation_types.py
python3 tools/check_tlmc_runtime.py
python3 tools/check_ltpd_runtime.py
python3 tools/check_foundation_acceptance.py
python3 tools/check_mle_cell_change.py
python3 tools/check_cmce_call_restore.py
python3 tools/check_mm_mobility.py
python3 tools/protocol_inventory.py --check
bash tools/check_protocol_inventory.sh
```

## 5. Formatierung und gezielte Tests

```bash
cargo fmt --all -- --check
cargo test -p tetra-saps
cargo test -p tetra-pdus --test test_mle_cell_change_pdus
cargo test -p tetra-pdus --test test_mm_mobility_pdus
cargo test -p tetra-entities --test test_mm_mobility_runtime
cargo test -p tetra-entities --test test_two_cell_mm_mobility
cargo test -p tetra-entities --test test_call_restore_runtime
cargo test -p tetra-entities --test test_two_cell_call_restore
```

## 6. Workspace-Abnahme

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Bei einem Fehler die Dienste nicht starten. Zuerst den vollständigen Compiler- oder Testfehler beheben.

## 7. Clean Release Build

```bash
cargo build --release --features asterisk \
  -p bluestation-bs \
  -p netcore-control-room \
  -p netcore-control-room-operator
```

## 8. Binaries und systemd-Ziele prüfen

```bash
ls -lh \
  target/release/bluestation-bs \
  target/release/netcore-control-room \
  target/release/netcore-control-room-operator

systemctl cat tetra.service
systemctl cat netcore-control-room.service 2>/dev/null || true
```

## 9. Dienste starten

```bash
sudo systemctl daemon-reload
sudo systemctl start tetra.service
sudo systemctl status tetra.service --no-pager
```

Control Room bei Bedarf:

```bash
sudo systemctl start netcore-control-room.service
sudo systemctl status netcore-control-room.service --no-pager
```

## 10. Logkontrolle

```bash
sudo journalctl -u tetra.service -f \
  | grep --line-buffered -iE \
  'migration|VASSI|D-LOCATION-UPDATE-PROCEEDING|forward registration|U-PREPARE|D-NEW-CELL|context transfer|LocationUpdateReject|mobility.*timeout'
```

Erwartet:

- eine erste Migrating-Anfrage erzeugt genau ein Proceeding mit VASSI;
- Wiederholungen liefern dieselbe aktive VASSI statt einen zweiten Context anzulegen;
- die zweite Demand-Anfrage wird nur mit passender Home-Identität akzeptiert;
- Gruppen und Energy-Economy-Daten bleiben nach Context Import erhalten;
- Forward Registration erzeugt eine eingebettete MM-Antwort für D-NEW-CELL;
- fehlende beziehungsweise inkonsistente Kontexte werden kontrolliert abgewiesen;
- offene Transaktionen laufen nach 432 Timeslots definiert aus.

## 11. Zwei-TBS-Test

Vor einem realen Funkversuch beide TBS auf denselben Stand bringen und zunächst nur Testgeräte verwenden.

Prüfen:

1. Quell-TBS kann den Teilnehmerkontext exportieren;
2. Ziel-TBS erhält den Kontext unter der zugewiesenen VASSI;
3. Gruppenaffiliationen stimmen auf der Ziel-TBS;
4. kein Teilnehmer bleibt gleichzeitig als aktiver Eigentümer in zwei Zellen zurück;
5. Call Restore aus Paket B verwendet anschließend den übertragenen Kontext;
6. bei Core-/Backhaul-Ausfall greift der dokumentierte lokale Fallback.

Der eigentliche Netzwerktransport wird erst mit `system-backend/node-gateway` und `system-backend/mobility-core` aktiviert.

## Rückfall

```bash
sudo systemctl stop tetra.service 2>/dev/null || true
sudo systemctl stop netcore-control-room.service 2>/dev/null || true
cd ~
rm -rf netcore-tetra
mv netcore-tetra.old netcore-tetra
cd netcore-tetra
rm -rf target
cargo clean
cargo build --release --features asterisk \
  -p bluestation-bs \
  -p netcore-control-room \
  -p netcore-control-room-operator
sudo systemctl start tetra.service
```

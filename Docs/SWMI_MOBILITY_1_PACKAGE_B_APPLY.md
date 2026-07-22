# SWMI Mobility 1 – Paket B einspielen

## 1. Sicherung erstellen

```bash
cd ~
tar -czf netcore-tetra-backup-before-mobility1-b-$(date +%F-%H%M%S).tar.gz netcore-tetra
```

Produktive Konfigurationen, lokale Audio-/TTS-Dateien und nicht im Repository liegende Schlüssel separat sichern.

## 2. Dienste stoppen

```bash
sudo systemctl stop tetra.service 2>/dev/null || true
sudo systemctl stop netcore-control-room.service 2>/dev/null || true
sudo systemctl stop netcore-control-room-operator.service 2>/dev/null || true
```

Prüfen, dass keine alten Prozesse weiterlaufen:

```bash
pgrep -af 'bluestation-bs|netcore-control-room|netcore-control-room-operator' || true
```

## 3. Gesamtarchiv entpacken

```bash
cd ~
unzip netcore-tetra-swmi-mobility1-package-b-call-restore.zip
```

Das Archiv enthält das vollständige Repository unter:

```text
netcore-tetra-main/
```

## 4. Alten Stand vollständig ersetzen

Keine Einzeldateien über den alten Quellbaum kopieren.

```bash
cd ~
rm -rf netcore-tetra.old
mv netcore-tetra netcore-tetra.old
mv netcore-tetra-main netcore-tetra
cd netcore-tetra
```

Danach die eigene produktive Konfiguration aus der Sicherung zurückkopieren.

## 5. Alte Binaries und Build-Artefakte löschen

Damit kein altes Binary versehentlich gestartet wird:

```bash
cd ~/netcore-tetra
rm -rf target
rm -f \
  target/release/bluestation-bs \
  target/release/netcore-control-room \
  target/release/netcore-control-room-operator
cargo clean
```

Bei separat kopierten Produktivbinaries deren bisherigen Zielpfad ebenfalls vor dem Neuaufbau entfernen oder eindeutig sichern.

## 6. Statische Prüfungen

```bash
python3 tools/check_swmi_foundation_types.py
python3 tools/check_tlmc_runtime.py
python3 tools/check_ltpd_runtime.py
python3 tools/check_foundation_acceptance.py
python3 tools/check_mle_cell_change.py
python3 tools/check_cmce_call_restore.py
python3 tools/protocol_inventory.py --check
bash tools/check_protocol_inventory.sh
```

## 7. Formatierung und gezielte Tests

```bash
cargo fmt --all -- --check
cargo test -p tetra-saps
cargo test -p tetra-pdus --test test_mle_cell_change_pdus
cargo test -p tetra-entities --test test_mle_cell_change_runtime
cargo test -p tetra-entities --test test_call_restore_runtime
cargo test -p tetra-entities --test test_two_cell_foundation
cargo test -p tetra-entities --test test_two_cell_mobility
cargo test -p tetra-entities --test test_two_cell_call_restore
```

## 8. Workspace-Abnahme

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Bei einem Fehler nicht starten. Zuerst den Compiler- oder Testfehler vollständig beheben.

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

## 10. Neu gebaute Binaries prüfen

```bash
ls -lh \
  target/release/bluestation-bs \
  target/release/netcore-control-room \
  target/release/netcore-control-room-operator
```

Optional den eingebetteten Commit-/Versionsstand kontrollieren:

```bash
target/release/bluestation-bs --version 2>/dev/null || true
```

## 11. Dienste starten

Prüfen, dass systemd wirklich auf die soeben gebauten Binaries zeigt:

```bash
systemctl cat tetra.service
systemctl cat netcore-control-room.service 2>/dev/null || true
```

Danach:

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

Operator Dashboard alternativ interaktiv:

```bash
target/release/netcore-control-room-operator \
  --api http://10.0.1.25:9010 \
  dashboard
```

## 12. Laufzeitkontrolle

```bash
sudo journalctl -u tetra.service -f \
  | grep --line-buffered -iE \
  'U-RESTORE|D-RESTORE|U-CALL RESTORE|D-CALL RESTORE|U-TX DEMAND|U-TX CEASED|call.restore|Callqueued|RequestQueued|GrantedToOtherUser|D-TX GRANTED|restore.*queued|restore.*timeout|new call identifier|channel allocation'
```

Erwartet:

- ohne eingehendes `U-RESTORE` entstehen keine spontanen Restore-Transaktionen;
- unbekannte Calls werden kontrolliert abgewiesen;
- bei freiem Traffic Channel enthält `D-RESTORE-ACK` die Allocation;
- bei belegten Traffic Channels wird `Callqueued` ohne Allocation gesendet;
- `U-TX DEMAND` während `Callqueued` wird mit `RequestQueued` bestätigt;
- `U-TX CEASED` während `Callqueued` storniert die Sendeanforderung;
- nach frei werdendem Bearer folgt `D-TX GRANTED` mit Allocation;
- Gruppenruf-Listener erhalten bei fremdem Sprecher `GrantedToOtherUser`;
- Duplex-Individualrufe werden unabhängig vom Request-Bit mit `Granted` wiederhergestellt;
- keine doppelten Circuits nach Replay;
- kein Neustart von T310 durch normale Restore-Antworten.

## 13. Zwei-TBS-Test

Für einen realen Test werden beide TBS auf denselben Softwarestand gebracht. Zunächst nur mit Testgeräten und getrennten beziehungsweise ausreichend entkoppelten RF-Zellen arbeiten.

Vor dem Funkversuch prüfen:

1. beide TBS melden identische Protokollversion;
2. Restore Context wird auf der Ziel-TBS bereitgestellt;
3. Zielzelle besitzt freie Traffic-Ressource oder zeigt bewusst Queue-Verhalten;
4. Logs enthalten alten und gegebenenfalls neuen Call Identifier;
5. der Call wird nach dem Restore nicht doppelt angelegt.

Der netzweite Audio-Transport zwischen zwei physischen TBS ist noch nicht Bestandteil dieses Pakets und folgt mit Call Core und Media Switch.

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

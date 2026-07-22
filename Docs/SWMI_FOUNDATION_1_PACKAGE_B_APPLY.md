# Einspielen und Prüfen – SWMI Foundation 1 Paket B

## 1. Laufende Dienste stoppen

Vor dem Austausch der Dateien:

```bash
sudo systemctl stop tetra.service 2>/dev/null || true
sudo systemctl stop netcore-control-room.service 2>/dev/null || true
```

## 2. Konfiguration und lokale Daten sichern

Mindestens sichern:

```bash
cp -a ~/netcore-tetra/config.toml ~/netcore-tetra-config.toml.backup 2>/dev/null || true
cp -a ~/netcore-tetra/system-backend ~/netcore-system-backend.backup 2>/dev/null || true
```

Zusätzliche lokale Audio-, Recording-, TTS- oder Datenbankpfade bleiben außerhalb des Repository-Austauschs zu sichern.

## 3. Altes Arbeitsverzeichnis ersetzen

Das ZIP enthält das vollständige Repository. Nicht einzelne Dateien wahllos mischen.

Beispiel:

```bash
cd ~
mv netcore-tetra netcore-tetra.before-foundation1-b
unzip netcore-tetra-swmi-foundation1-package-b.zip
mv netcore-tetra-main netcore-tetra
```

Danach die lokale `config.toml` beziehungsweise notwendige lokale Konfiguration zurückkopieren.

## 4. Alte Build-Artefakte vollständig entfernen

```bash
cd ~/netcore-tetra
rm -rf target
cargo clean
```

Damit kann keine alte Binary versehentlich weiterverwendet werden.

## 5. Paketprüfungen

```bash
python3 tools/check_swmi_foundation_types.py
python3 tools/protocol_inventory.py --check
cargo test -p tetra-saps
```

## 6. Vollständiger Build

Für den aktuellen NetCore-Tetra-Stand:

```bash
cargo build --release --features asterisk
```

Falls die Binaries explizit angegeben werden sollen:

```bash
cargo build --release \
  --bin bluestation-bs \
  --bin netcore-control-room \
  --bin netcore-control-room-operator \
  --features bluestation-bs/asterisk
```

## 7. Dienst wieder starten

```bash
sudo systemctl start tetra.service
sudo journalctl -u tetra.service -f
```

Control Room abhängig von der lokalen Installation anschließend separat starten.

## 8. Erwartetes Verhalten

Paket B verändert noch keine TLMC-Runtime und löst daher noch keine Zellscans oder Zellwechsel aus. Bestehende Single-Site-Funktionen sollen unverändert bleiben.

Neu prüfbar sind vor allem:

- Kompilation der typisierten SAPs;
- nicht panikender `SapMsgInner`-Displaypfad;
- korrektes LTPD-Routing auf der MS-Seite;
- vollständige statische Paketprüfung;
- aktualisierte Protokollinventur.

## 9. Rückkehr zum vorherigen Stand

```bash
sudo systemctl stop tetra.service
cd ~
mv netcore-tetra netcore-tetra.failed-foundation1-b
mv netcore-tetra.before-foundation1-b netcore-tetra
cd ~/netcore-tetra
rm -rf target
cargo clean
cargo build --release --features asterisk
sudo systemctl start tetra.service
```

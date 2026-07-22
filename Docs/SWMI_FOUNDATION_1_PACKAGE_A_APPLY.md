# SWMI Foundation 1 – Paket A anwenden

## Inhalt

Diese Lieferung setzt **Paket A – Inventur** um und ergänzt die verbindliche Management-Architektur für alle späteren LXC-/VM-Dienste. Es werden noch keine TLMC-/TLPD-Runtimepfade verändert und keine neuen LXC-Dienste gestartet. Jeder zukünftige Container ist jedoch bereits verbindlich mit einer eigenen WebUI eingeplant.

## Neue Dateien

```text
.github/workflows/protocol-inventory.yml
Docs/BACKEND_WEBUI_SERVICE_MATRIX.md
Docs/BACKEND_WEBUI_STANDARD.md
Docs/ETSI_CONFORMANCE_MATRIX.md
Docs/ETSI_SOURCE_REGISTER.md
Docs/IMPLEMENTATION_GAPS.md
Docs/SAP_PRIMITIVE_MATRIX.md
Docs/STATE_MACHINE_INVENTORY.md
Docs/SWMI_FOUNDATION_1_INVENTORY.md
Docs/SWMI_FOUNDATION_1_PACKAGE_A_APPLY.md
Docs/TLMC_TLPD_WORKLIST.md
Docs/generated/gap_inventory.csv
Docs/generated/pdu_inventory.csv
Docs/generated/protocol_inventory.json
Docs/generated/sap_inventory.csv
Docs/generated/state_inventory.csv
tools/README.md
tools/check_protocol_inventory.sh
tools/protocol_inventory.py
system-backend/services.toml
system-backend/shared/web-ui/README.md
```

Geändert wurden außerdem:

```text
system-backend/README.md
system-backend/roadmap.md
system-backend/<dienst>/README.md
Docs/TLMC_TLPD_WORKLIST.md
```

## Sauberes Einspielen des vollständigen ZIP

### 1. Laufende Dienste stoppen

Falls die Basisstation aus diesem Arbeitsverzeichnis läuft:

```bash
sudo systemctl stop tetra.service
```

### 2. Alten Stand sichern

```bash
cd ~
mv netcore-tetra netcore-tetra-backup-$(date +%Y%m%d-%H%M%S)
```

Den tatsächlichen Verzeichnisnamen bei Bedarf anpassen.

### 3. ZIP entpacken

```bash
unzip netcore-tetra-swmi-foundation1-package-a.zip
mv netcore-tetra-main netcore-tetra
cd netcore-tetra
```

### 4. Alte Build-Artefakte entfernen

Paket A verändert zwar keinen Rust-Runtimecode, ein sauberer Stand bleibt dennoch verbindlich:

```bash
rm -rf target
```

### 5. Inventur prüfen

```bash
python3 tools/protocol_inventory.py --check
```

Erwartete Ausgabe:

```text
Protocol inventory is up to date.
```

### 6. Inventur nach späteren Codeänderungen aktualisieren

```bash
python3 tools/protocol_inventory.py
python3 tools/protocol_inventory.py --check
```

### 7. Bestehenden Stack bauen und testen

```bash
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets
cargo build --release --features asterisk
```

### 8. Dienst wieder starten

```bash
sudo systemctl start tetra.service
sudo journalctl -u tetra.service -f
```

## WebUI-Grundregel prüfen

Die verbindlichen Vorgaben befinden sich in:

```text
Docs/BACKEND_WEBUI_STANDARD.md
Docs/BACKEND_WEBUI_SERVICE_MATRIX.md
system-backend/services.toml
```

Für jeden späteren Container ist eine eigene Verwaltungsoberfläche vorgesehen. Paket A implementiert noch keine Service-Runtime, legt aber die Architektur und Definition of Done verbindlich fest.

## Rückbau

Da Paket A keine Runtimefunktion verändert, genügt bei Bedarf die Rückkehr zum gesicherten Verzeichnis:

```bash
sudo systemctl stop tetra.service
cd ~
rm -rf netcore-tetra
mv netcore-tetra-backup-YYYYMMDD-HHMMSS netcore-tetra
sudo systemctl start tetra.service
```

## Nächster Schritt

Paket B ersetzt die generischen `Todo`-Felder in TLMC/TLPD durch konkrete Typen und erweitert `SapMsgInner` um die benötigten Primitive. Die verbindliche Reihenfolge steht in `Docs/TLMC_TLPD_WORKLIST.md`.

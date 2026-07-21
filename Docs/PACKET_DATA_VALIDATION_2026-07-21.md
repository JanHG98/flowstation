# NetCore-Tetra Paketdaten-Gateway – Validierungsprotokoll

Stand: 21.07.2026

## Durchgeführt

- 453 Rust-Quelldateien mit Tree-sitter-Rust vollständig geparst;
- keine Rust-Syntaxfehler gefunden;
- alle 19 TOML-Dateien mit Python `tomllib` geparst;
- keine TOML-Fehler gefunden;
- alle vier Paketdaten-Shellhelfer mit `sh -n` geprüft;
- alle vier Helfer besitzen Modus `0755`;
- systemd-Drop-in mit einer Test-Basisunit durch `systemd-analyze verify` geprüft;
- `git diff --check` ohne Whitespacefehler;
- IPv4/UDP-Referenzvektor unabhängig berechnet;
- IPv4-Headerprüfsumme unabhängig bestätigt;
- Fragmentierung eines 1220-Byte-Datagramms bei MTU 300 unabhängig geprüft;
- fünf Fragmente mit Längen 300/300/300/300/100 und korrekten Prüfsummen;
- Out-of-order-Reassembly unabhängig bestätigt;
- IPCP-DNS-Configure-NAK-Vektor unabhängig bestätigt;
- Subnetz `10.0.0.0/24`, Gateway `10.0.0.1` und Pool `10.0.0.2–10.0.0.254` unabhängig geprüft;
- Patch-Roundtrip auf dem exakten vorherigen SNDCP-ZIP;
- gepatchter Baum inhaltlich bytegleich mit dem neuen vollständigen ZIP;
- die vier neu hinzugefügten Betriebshelfer besitzen im Patch und ZIP den ausführbaren Git-Modus `100755`;
- normale Dateien werden im neuen Git-Archiv kanonisch als `0644` ausgeliefert; das ältere SNDCP-ZIP hatte durch den damaligen Packvorgang teilweise `0664`, was den Dateiinhalt nicht verändert;
- ZIP-Neuextraktion und Integritätsvergleich;
- SHA-256-Prüfsummen für alle Lieferartefakte.

## Referenzvektoren

### IPv4/UDP

```text
4500001f00070000201116c4c0000201c0000202c00023f0000b0000776170
```

### IPCP DNS Configure-NAK

Anfrage:

```text
01 07 00 0a 81 06 00 00 00 00
```

Antwort für DNS `1.1.1.1` und `9.9.9.9`:

```text
03 07 00 10 81 06 01 01 01 01 83 06 09 09 09 09
```

## Nicht durchgeführt

Im Arbeitscontainer war keine echte Rust-Toolchain installiert und die
Toolchain-Paketserver waren aus dem Container nicht erreichbar. Deshalb wurden
hier **kein `cargo test`, kein `cargo check`, kein Release-Build und kein echter
On-Air-Test mit einem TETRA-Endgerät** durchgeführt.

Der erste Build auf dem Zielsystem ist daher weiterhin zwingend. Die statischen,
konfigurationsbezogenen, protokollvektorbezogenen und Artefakt-Roundtrip-Prüfungen
ersetzen keinen Rust-Typcheck und keinen Funkfeldtest.

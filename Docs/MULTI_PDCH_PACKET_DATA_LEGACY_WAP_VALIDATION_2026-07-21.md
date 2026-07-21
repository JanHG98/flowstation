# Validierungsprotokoll – Multi-PDCH / Paketdaten-Dashboard / Legacy-WAP

Basis: vom Nutzer bereitgestelltes `netcore-tetra-wap(1).zip`.

## Durchgeführt

- ZIP erfolgreich entpackt und als unveränderter Git-Baseline-Commit eingefroren.
- Sämtliche geänderten Dateien mit `git diff --check` auf Whitespace-/Patchfehler geprüft.
- 454 Rust-Dateien mit Tree-sitter-Rust syntaktisch geparst; keine Syntaxfehler.
- 19 TOML-Dateien mit Python `tomllib` geparst; keine Parsefehler.
- Zwei JavaScript-Blöcke des eingebetteten Basisstations-Dashboards extrahiert und mit Node.js `--check` geprüft; beide fehlerfrei.
- Vollständiger ZIP-Inhalt ohne `.git`, `target` und Cache-Artefakte erzeugt.
- Patch-Roundtrip auf dem unveränderten Nutzer-ZIP durchgeführt.
- Gepatchter Baum byteweise mit dem vollständigen Auslieferungsbaum verglichen.
- SHA-256-Prüfsummen erzeugt.

## Abgedeckte statische Prüfbereiche

- Multi-PDCH-Konfiguration und Grenzen.
- Gemeinsamer Timeslot-Allocator für CMCE/Brew/SNDCP.
- Mehrere parallele SNDCP-Bearer.
- Bearer-Lifecycle bei READY-Ablauf, END-OF-DATA, PDP-Deaktivierung und Deregistrierung.
- Paketdaten-Telemetrie und JSON-Serialisierung.
- Lokales Dashboard und WebSocket-Nachrichten.
- Control-Room-State, REST-API und Operator-CLI.
- Native Control-Room-UI-Quelltext.
- Legacy-WAP-Payloadgrenze und PID-Header.

## Nicht im Arbeitscontainer möglich

Im Arbeitscontainer standen weder `cargo` noch `rustc` zur Verfügung. Ein echter Rust-Typcheck, Linklauf und On-Air-Test konnten deshalb hier nicht ausgeführt werden. DNS-Zugriff auf die Rust-Toolchain-Server war ebenfalls nicht verfügbar.

Der exakte vom Nutzer gelieferte Ausgangsstand wurde zuvor auf dessen System erfolgreich kompiliert. Nach dem Einspielen sind dennoch zwingend die in der Integrationsanleitung genannten `cargo check`, `cargo test` und Release-Builds auszuführen.

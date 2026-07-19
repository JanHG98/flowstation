# Phase 2 – gemeinsame Audio-Zentrale

## Änderung

Die bisherigen Dashboard-Seiten `AUFZEICHNUNGEN` und `AUSSENDEN` wurden zu einer gemeinsamen Seite zusammengeführt.

Neue Navigation:

```text
INTEGRATIONEN
└── AUDIO-ZENTRALE
```

Die gemeinsame Seite enthält in dieser Reihenfolge:

1. Status und Bedienung der laufenden Aussendung
2. lokalen WAV-/MP3-Dateibrowser
3. Dialog `Senden an`
4. Status und Bedienung der Gesprächsaufzeichnung
5. Aufzeichnungsliste, Browser-Wiedergabe, Download, erneutes Aussenden und Löschen

## Technische Änderungen

- `nav-recordings` wurde entfernt.
- `page-recordings` wurde entfernt.
- Die Recording-Oberfläche liegt nun innerhalb von `page-audio`.
- `showPage('audio')` lädt Player und Recorder gemeinsam.
- Der 5-Sekunden-Refresh der Aufzeichnungen beobachtet nun `page-audio`.
- Die Audio-Zentrale wurde aus `SYSTEM` nach `INTEGRATIONEN` verschoben.
- Die vorhandenen APIs und Backend-Dienste wurden nicht verändert.

## Vollständige geänderte Pfade

```text
Docs/PHASE2_AUDIO_CENTER_UI.md
crates/tetra-entities/src/net_dashboard/html.rs
```

## Build

Da das Dashboard in `tetra-entities` eingebettet ist, muss `bluestation-bs` neu gebaut und ausgetauscht werden.

```bash
cd ~/flowstation
rm -rf target
cargo clean

cargo build --release \
  -p bluestation-bs \
  --features "asterisk,recording,audio-player"
```

Anschließend den tatsächlichen Binary-Pfad prüfen:

```bash
systemctl cat bluestation-bs | grep '^ExecStart='
```

Beispiel:

```bash
sudo systemctl stop bluestation-bs
sudo rm -f /usr/local/bin/bluestation-bs
sudo install -m 0755 target/release/bluestation-bs /usr/local/bin/bluestation-bs
sudo systemctl start bluestation-bs
sudo systemctl status bluestation-bs --no-pager
```

Im Browser anschließend einmal hart neu laden (`Strg+F5`), damit kein altes eingebettetes Dashboard aus dem Browsercache angezeigt wird.

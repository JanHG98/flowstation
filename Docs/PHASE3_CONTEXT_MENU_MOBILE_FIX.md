# Phase 3.3 – Kontextmenü-Position und mobiles Aktionsmenü

## Änderung

Das Kontextmenü der WAV-/MP3-Medienbibliothek wird beim Öffnen aus der animierten Audio-Seite in `document.body` verschoben. Dadurch beziehen sich die `position: fixed`-Koordinaten wieder direkt auf den Browser-Viewport.

### Desktop

- Rechtsklick öffnet das Menü unmittelbar am Mauszeiger.
- Das Menü wird an allen vier Viewport-Rändern begrenzt.
- Vorschau und `Senden an …` bleiben als sichtbare Aktionsbuttons vorhanden.

### Mobil und Touch

- Bei höchstens 760 px Breite oder einem Gerät ohne Hover-Maus werden die breiten Aktionsbuttons ausgeblendet.
- Rechts in jeder WAV-/MP3-Dateizeile erscheint ein `⋮`-Button.
- Das Menü wird am Button rechts ausgerichtet geöffnet.
- Reicht der Platz nach unten nicht aus, öffnet es sich automatisch oberhalb des Buttons.
- Ein Klick außerhalb, Scrollen, Größenänderung, Fokusverlust oder `Escape` schließt das Menü.

## Geänderte Datei

```text
crates/tetra-entities/src/net_dashboard/html.rs
```

## Build

Da das Dashboard in `bluestation-bs` eingebettet ist, muss das Binary neu gebaut und ausgetauscht werden.

```bash
cd ~/flowstation
rm -rf target
cargo clean
cargo build --release \
  -p bluestation-bs \
  --features "asterisk,recording,audio-player"
```

Danach den tatsächlich verwendeten Binary-Pfad über `systemctl cat bluestation-bs` prüfen, den Dienst stoppen, das alte Binary entfernen, das neue Binary installieren und den Dienst neu starten.

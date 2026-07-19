# Phase 3.2 – Vorschau für Audiodateien

Die Audio-Zentrale bietet jetzt eine authentifizierte Browser-Vorschau für lokale sowie auf Server-/NFS-Quellen liegende WAV- und MP3-Dateien.

## Bedienung

Jede Audiodatei besitzt in der Medienbibliothek einen Button **Vorschau**. Die ausgewählte Datei wird in einem eigenen HTML-Audioplayer geöffnet und kann vor der TETRA-Aussendung vollständig kontrolliert werden. Das Rechtsklick-Menü enthält ebenfalls den Eintrag **Vorschau**. Gesprächsaufzeichnungen verwenden weiterhin ihren bereits vorhandenen Wiedergabeplayer.

## API

```text
GET  /api/audio/preview?source=<quellen-id>&path=<relativer-pfad>
HEAD /api/audio/preview?source=<quellen-id>&path=<relativer-pfad>
```

Der Endpunkt:

- verwendet dieselben kanonischen Root-/Pfadprüfungen wie die Aussendung,
- akzeptiert ausschließlich WAV und MP3,
- berücksichtigt `max_file_size_mb`,
- unterstützt einen einzelnen HTTP-Byte-Range für Wiedergabe und Spulen,
- sendet `Cache-Control: private, no-store`,
- akzeptiert niemals einen beliebigen absoluten Dateisystempfad.

Eine Vorschau vom NFS-Server wird direkt aus dem gemounteten, read-only verwendeten Share gelesen. Die eigentliche Funkaussendung kopiert Netzwerkdateien weiterhin vor dem Rufaufbau in den lokalen Cache.

# Storage-Format

```text
/var/lib/netcore-media-library/
├── state.json
├── backups/
└── assets/
    └── <asset-id>/
        ├── original.wav|mp3|tacelp
        ├── preview.wav
        ├── audio.tacelp
        └── metadata.json
```

`original.*` bleibt unverändert. `preview.wav` ist das hörbare kanonische 8-kHz-PCM-Format. `audio.tacelp` enthält ohne Header exakt 35 Byte pro TETRA-Sprachblock.

Die globale `state.json` ist die Runtime-Datenbank. `metadata.json` ist ein lesbarer Sidecar pro Asset und kann für Recovery oder Archivprüfung verwendet werden.

## Archiv

Bei jeder Archivierung wird eine neue unveränderliche Version angelegt:

```text
/mnt/nfs-share/Media-Library/<asset-id>/<UTC-version>/
├── original.*
├── preview.wav          # falls vorhanden
├── audio.tacelp         # falls vorhanden
└── manifest.json        # Asset-Metadaten, Größe und SHA-256 jeder Kopie
```

Die Archivkopie wird nie als laufende Playout-Quelle verwendet. Das aktuelle Asset verweist nur auf die zuletzt erfolgreich erzeugte Archivversion.

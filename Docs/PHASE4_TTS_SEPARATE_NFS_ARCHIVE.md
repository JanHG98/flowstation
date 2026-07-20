# Phase 4: Getrenntes NFS-Archiv für TTS-WAVs

## Ziel

Normale Funkmitschnitte und erzeugte TTS-WAVs bleiben gemeinsam in der lokalen Aufzeichnungsbibliothek sichtbar und werden von dort identisch abgespielt. Die Serverkopie wird jedoch getrennt abgelegt:

```text
Normale Recordings -> /mnt/nfs-share/Recordings
TTS-WAVs           -> /mnt/nfs-share/TTS-Dateien
```

Damit landen neue TTS-Dateien nicht mehr im Recording-Archiv.

## Erkennung

Ein Bibliothekseintrag gilt als TTS-Datei, wenn seine JSON-Metadaten enthalten:

```json
"origin": "tts"
```

Alle anderen Einträge verwenden unverändert das normale Recording-Archiv.

## Ablage

Normale Mitschnitte behalten ihre bisherige Datumsstruktur:

```text
/mnt/nfs-share/Recordings/YYYY/MM/DD/<Datei>.wav
/mnt/nfs-share/Recordings/YYYY/MM/DD/<Datei>.json
```

TTS-Dateien werden wegen ihrer eindeutigen Namen flach im eigenen Ordner abgelegt:

```text
/mnt/nfs-share/TTS-Dateien/TTS-<Name>_<Zeitstempel>_<UUID>.wav
/mnt/nfs-share/TTS-Dateien/TTS-<Name>_<Zeitstempel>_<UUID>.json
```

Die lokale Bibliothek unter `/var/lib/netcore/recordings` bleibt unverändert. Sie ist weiterhin die Quelle für Vorschau, Auswahl, Versand und Löschung.

## Konfiguration

```toml
[recording]
archive_enabled = true
archive_directory = "/mnt/nfs-share/Recordings"

tts_archive_enabled = true
tts_archive_directory = "/mnt/nfs-share/TTS-Dateien"
archive_retry_seconds = 60
```

Beide Zielordner müssen bereits existieren und für den Benutzer des `tetra.service` beschreibbar sein. FlowStation legt die Wurzelordner absichtlich nicht selbst an, damit ein fehlender NFS-Mount nicht unbemerkt durch einen lokalen Ordner ersetzt wird.

## Wiederholungs- und Retentionsverhalten

- Die Archivierung läuft asynchron und blockiert weder TTS-Erzeugung noch Funkbetrieb.
- Ist eines der Ziele kurzzeitig nicht erreichbar, wird alle `archive_retry_seconds` erneut versucht.
- Erst nach verifizierter WAV- und JSON-Kopie wird lokal ein `.archived`-Marker geschrieben.
- Die Retention löscht einen archivierungspflichtigen lokalen Eintrag nicht, solange der passende Marker für sein tatsächliches Ziel fehlt.
- Alte TTS-Marker, die noch auf `/mnt/nfs-share/Recordings` zeigen, werden nicht akzeptiert. Dadurch werden bestehende lokale TTS-Dateien automatisch in den neuen Ordner nacharchiviert.

## Logmeldungen

Normale Aufnahme:

```text
Recorder archive: copied recording id=... to /mnt/nfs-share/Recordings
```

TTS-Datei:

```text
Recorder archive: copied TTS WAV id=... to /mnt/nfs-share/TTS-Dateien
```

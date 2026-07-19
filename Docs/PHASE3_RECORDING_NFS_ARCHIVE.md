# Phase 3.4 – Automatisches NFS-Archiv für Gesprächsaufzeichnungen

## Ziel

Jede lokal vollständig finalisierte Gesprächsaufzeichnung wird automatisch als WAV-Datei und JSON-Metadatendatei in das bereits gemountete Serververzeichnis kopiert:

```text
/mnt/nfs-share/Recordings
```

Die lokale Aufnahme bleibt erhalten. Der Recorder schreibt niemals direkt während eines Funkgesprächs auf das NFS-Share.

## Ablauf

```text
TETRA-Gespräch
  -> lokale .wav.part / .json.part
  -> lokale WAV und JSON sauber finalisieren
  -> Auftrag an Hintergrund-Archiver
  -> Kopie als versteckte .part-Datei auf den Server
  -> fsync und atomare Umbenennung
  -> WAV und JSON per Dateigröße verifizieren
  -> lokale .archived-Bestätigung schreiben
```

Die JSON-Datei wird zuletzt kopiert und dient im Serverarchiv als Fertigmarker. Lokal wird anschließend eine kleine Datei mit der Endung `.archived` angelegt. Sie enthält Archivpfad, Zeitpunkt und die bestätigten Dateigrößen und verhindert, dass die lokale Aufbewahrungsbereinigung eine noch nicht bestätigte Aufnahme löscht.

## Verzeichnisstruktur

Die lokale Datumsstruktur wird unter dem Archivwurzelverzeichnis erhalten:

```text
/mnt/nfs-share/Recordings/
└── 2026/
    └── 07/
        └── 19/
            ├── 2026-07-19_17-18-22_CALL-4_TS-2_GSSI-15201_<uuid>.wav
            └── 2026-07-19_17-18-22_CALL-4_TS-2_GSSI-15201_<uuid>.json
```

## Ausfallverhalten

- Ist NFS offline oder nicht beschreibbar, läuft die lokale Aufnahme unverändert weiter.
- Nicht archivierte Aufnahmen werden in jedem Retry-Zyklus erneut geprüft.
- Beim Start werden auch ältere lokale Aufnahmen nacharchiviert.
- Die lokale Aufbewahrungsbereinigung löscht keine alte Aufnahme, solange keine passende lokale `.archived`-Bestätigung existiert.
- Bereits vollständig vorhandene Dateien werden anhand ihrer Dateigröße erkannt und nicht erneut kopiert.
- Unvollständige Serverdateien besitzen versteckte `.part`-Namen und werden beim nächsten Versuch ersetzt.

## Konfiguration

```toml
[recording]
enabled = true
active = true
directory = "/var/lib/netcore/recordings"
mode = "all"
selected_groups = []
minimum_free_space_mb = 2048
retention_days = 30
max_recording_minutes = 120
idle_finalize_secs = 15
max_list_entries = 2000

archive_enabled = true
archive_directory = "/mnt/nfs-share/Recordings"
archive_retry_seconds = 60
```

`archive_directory` muss ein absoluter Pfad sein, bereits existieren und für den Benutzer des `bluestation-bs`-Dienstes beschreibbar sein. FlowStation legt den Archivwurzelordner absichtlich nicht selbst an. Dadurch wird bei einem fehlenden NFS-Mount nicht versehentlich lokal unter `/mnt` archiviert.

## Berechtigungen prüfen

Dienstbenutzer anzeigen:

```bash
systemctl show bluestation-bs -p User -p Group
```

Mount und Zielordner prüfen:

```bash
findmnt /mnt/nfs-share
mountpoint /mnt/nfs-share
ls -ld /mnt/nfs-share/Recordings
```

Beispiel für den Dienstbenutzer `bluestation`:

```bash
sudo -u bluestation test -r /mnt/nfs-share/Recordings
sudo -u bluestation test -w /mnt/nfs-share/Recordings
sudo -u bluestation sh -c 'touch /mnt/nfs-share/Recordings/.permission-test && rm /mnt/nfs-share/Recordings/.permission-test'
```

Bei NFS mit Root-Squash müssen UID/GID und Exportrechte auf dem NFS-Server passend gesetzt werden. Ein lokales `chown` auf dem Client kann bei NFS wirkungslos sein.

## Dashboard

Unter `INTEGRATIONEN -> AUDIO-ZENTRALE -> AUFZEICHNUNGEN` erscheint eine zusätzliche Karte `Server-Archiv`:

- `ONLINE`: Share erreichbar, keine ausstehenden Kopien
- `ÜBERTRÄGT`: Archivlauf aktiv
- `WARTESCHLANGE`: Share erreichbar, einzelne Kopien noch ausstehend
- `OFFLINE`: Zielpfad nicht erreichbar oder nicht beschreibbar
- `DEAKTIVIERT`: `archive_enabled = false`

Angezeigt werden außerdem:

- Anzahl bereits bestätigter Archivkopien
- Anzahl ausstehender lokaler Aufnahmen
- Archivpfad oder letzter Archivfehler
- Zeitpunkt der letzten erfolgreichen Kopie als Tooltip

## Erwartete Logs

Beim Start:

```text
-> Local WAV recording enabled (/var/lib/netcore/recordings)
   Recording archive: /mnt/nfs-share/Recordings (retry every 60s)
```

Nach erfolgreicher Kopie:

```text
Recorder archive: copied recording id=<uuid> to /mnt/nfs-share/Recordings
```

Bei einem ausgefallenen Share:

```text
Recorder archive: archive directory is unavailable or not a directory: /mnt/nfs-share/Recordings
```

## Sicherheits- und Konsistenzregeln

- relative Aufnahmepfade werden validiert
- kein Ausbruch aus dem lokalen oder entfernten Archivbaum
- Symlink-Inhalte werden vom lokalen Recorder-Scanner nicht verfolgt
- lokale Originaldateien werden durch die Archivierung nicht verändert
- keine automatische Löschung auf dem NFS-Server
- manuelles Löschen im Dashboard entfernt nur die lokale Kopie; das Archiv bleibt erhalten

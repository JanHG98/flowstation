# Phase 3 – NFS-Server-Browser in der Audio-Zentrale

## Ziel

Die vorhandene Audio-Zentrale kann neben der lokalen Medienbibliothek jetzt zusätzliche, vom Betriebssystem eingehängte Verzeichnisse durchsuchen. Der erste konfigurierte Server ist der bereits gemountete NFS-Pfad:

```text
/mnt/nfs-share
```

FlowStation implementiert bewusst keinen eigenen NFS-Client. Mount, Wiederverbindung und Zugangsdaten bleiben Aufgabe von Linux/systemd. Die Anwendung behandelt das Share als read-only Medienquelle.

## Bedienung

Unter **INTEGRATIONEN → AUDIO-ZENTRALE** befindet sich über dem Dateibrowser eine Quellenauswahl:

- Lokale Dateien
- NFS-Server

Ordner können wie bisher geöffnet werden. WAV- und MP3-Dateien lassen sich per Button oder Rechtsklick an eine Gruppe oder ein Einzelgerät senden.

Ist das NFS-Share nicht verfügbar, bleibt die Basisstation vollständig betriebsbereit. Nur die Quelle wird im Browser als `OFFLINE` angezeigt; lokale Aufzeichnungen und lokale Aussendungen funktionieren weiter.

## Sicherer Aussendepfad

Serverdateien werden niemals während des aktiven TETRA-Rufs direkt vom NFS-Share gelesen.

```text
NFS-Datei
  → Größen- und Pfadprüfung
  → begrenzte Kopie nach /var/cache/netcore/audio
  → WAV/MP3 dekodieren
  → vollständig als TETRA-ACELP vorbereiten
  → erst danach Call aufbauen
  → Cachedatei löschen
```

Damit beendet ein kurzer NFS-Ausfall keinen bereits laufenden Funkruf und blockiert nicht den TDMA-Kern.

## Konfiguration

```toml
[audio_player]
enabled = true
directory = "/var/lib/netcore/audio"
cache_directory = "/var/cache/netcore/audio"
source_issi = 4010099
default_priority = 5
max_file_size_mb = 100
max_duration_seconds = 1800
tail_silence_blocks = 3
individual_answer_timeout_seconds = 30
ffmpeg_path = "ffmpeg"

[[audio_player.shares]]
id = "server"
name = "NFS-Server"
path = "/mnt/nfs-share"
```

Weitere bereits gemountete Quellen können später ergänzt werden:

```toml
[[audio_player.shares]]
id = "archiv"
name = "Audio-Archiv"
path = "/mnt/audio-archiv"
```

Regeln für `id`:

- eindeutig
- maximal 48 Zeichen
- Buchstaben, Zahlen, Punkt, Bindestrich oder Unterstrich
- `local` ist für die lokale Bibliothek reserviert

## API

### Quellen abrufen

```http
GET /api/audio/sources
```

Beispiel:

```json
{
  "sources": [
    {
      "id": "local",
      "name": "Lokale Dateien",
      "path": "/var/lib/netcore/audio",
      "source_type": "local",
      "available": true,
      "error": null
    },
    {
      "id": "server",
      "name": "NFS-Server",
      "path": "/mnt/nfs-share",
      "source_type": "server",
      "available": true,
      "error": null
    }
  ]
}
```

### Quelle durchsuchen

```http
GET /api/audio/browse?source=server&path=Durchsagen
```

### Serverdatei senden

```json
POST /api/audio/play
{
  "source_type": "media",
  "source_id": "server",
  "path": "Durchsagen/Evakuierung.mp3",
  "target_type": "group",
  "target_id": 1001,
  "priority": 5
}
```

Bestehende Clients ohne `source_id` verwenden weiterhin automatisch `local`.

## Berechtigungen

Der Dienstbenutzer von `bluestation-bs` benötigt:

- Leserechte auf `/mnt/nfs-share`
- Schreibrechte auf `/var/cache/netcore/audio`
- weiterhin Schreibrechte auf `/var/lib/netcore/audio` für die lokale Bibliothek

Prüfen:

```bash
systemctl show bluestation-bs -p User -p Group
sudo -u <DIENSTBENUTZER> find /mnt/nfs-share -maxdepth 1 -type f -o -type d | head
sudo -u <DIENSTBENUTZER> test -w /var/cache/netcore/audio && echo CACHE_OK
```

## Empfohlener NFS-Mount

Die genaue Serveradresse muss an das eigene NAS angepasst werden. Ein systemd-Automount verhindert, dass ein nicht erreichbares NAS den Bootvorgang der Basisstation blockiert.

Beispiel `/etc/fstab`:

```fstab
10.0.1.20:/volume/audio /mnt/nfs-share nfs4 ro,nofail,_netdev,x-systemd.automount,x-systemd.idle-timeout=600,timeo=10,retrans=2 0 0
```

Danach:

```bash
sudo mkdir -p /mnt/nfs-share
sudo systemctl daemon-reload
sudo mount -a
findmnt /mnt/nfs-share
```

FlowStation verändert oder löscht keine Dateien auf dem Server.

## Abnahmetest

1. Audio-Zentrale öffnen.
2. Quelle `NFS-Server` wählen.
3. Unterordner öffnen und WAV-/MP3-Datei anzeigen lassen.
4. Datei an eine Testgruppe senden.
5. Im Log die lokale Cachemeldung kontrollieren.
6. Während einer laufenden Aussendung das NFS-Share testweise trennen: Der bereits gestartete Ruf muss vollständig weiterlaufen.
7. Nach Abschluss prüfen, dass `/var/cache/netcore/audio` keine Jobdatei mehr enthält.
8. NFS trennen und Browser aktualisieren: Quelle muss `OFFLINE` melden; lokale Medien und Aufzeichnungen müssen weiter funktionieren.

## Noch nicht Bestandteil dieses Schritts

- Aufzeichnungen automatisch auf das NAS verschieben
- Upload, Umbenennen oder Löschen von Serverdateien
- Schreiben direkt auf das NFS-Share
- Synchronisation oder Offline-Spiegelung kompletter Verzeichnisse

Die Archivierung fertiger Aufzeichnungen kann als nächster, separater Phase-3-Schritt mit Retry-Queue ergänzt werden.

# Audio-Zentrale

Die Audio-Zentrale bündelt Aufzeichnung, Medienverwaltung, Vorschau, TTS und Funkaussendung. Die Funktionen werden standardmäßig mit den Cargo-Features `recording` und `audio-player` gebaut.

## Aufzeichnungen

Empfangene TETRA-Sprache wird lokal als **8 kHz, mono, 16 Bit PCM WAV** gespeichert. Zu jeder WAV-Datei wird eine JSON-Metadatendatei angelegt.

```toml
[recording]
enabled = true
active = true
directory = "/var/lib/netcore/recordings"
mode = "all"                  # all | selected_groups
selected_groups = []
minimum_free_space_mb = 2048
retention_days = 30
max_recording_minutes = 480
idle_finalize_secs = 15
max_list_entries = 2000
```

`mode = "selected_groups"` zeichnet nur die in `selected_groups` eingetragenen GSSIs auf.

## NFS-Archiv

Der NFS-Mount muss bereits durch das Betriebssystem verfügbar und für den Dienstbenutzer beschreibbar sein.

```toml
archive_enabled = true
archive_directory = "/mnt/nfs-share/Recordings"

tts_archive_enabled = true
tts_archive_directory = "/mnt/nfs-share/TTS-Dateien"
archive_retry_seconds = 60
```

Normale Aufzeichnungen landen im Ordner `Recordings`. Erzeugte TTS-WAVs werden separat nach `TTS-Dateien` kopiert. Bei kurzfristigem NFS-Ausfall bleiben lokale Dateien erhalten und die Archivierung wird erneut versucht.

## Medienbibliothek und Aussendung

```toml
[audio_player]
enabled = true
directory = "/var/lib/netcore/audio"
cache_directory = "/var/cache/netcore/audio"
source_issi = <QUELL-ISSI>
default_priority = 5
max_file_size_mb = 100
max_duration_seconds = 1800
lead_in_silence_blocks = 12
tail_silence_blocks = 3
group_release_guard_seconds = 6
individual_answer_timeout_seconds = 30
ffmpeg_path = "ffmpeg"
```

WAV- und MP3-Dateien werden vor dem Funkruf vollständig vorbereitet und in das benötigte Sprachformat umgesetzt. Erst danach startet der neue Ruf. Das verhindert, dass eine langsame Dateikonvertierung die erste Aussendung zerstört.

Zusätzliche, bereits gemountete Verzeichnisse können als Shares eingebunden werden:

```toml
[[audio_player.shares]]
name = "Server"
path = "/mnt/nfs-share"
read_only = true
```

## TTS

NetCore Piper stellt eine lokale HTTP-Schnittstelle bereit. Die Basisstation erzeugt daraus eine WAV-Datei, speichert sie wie ein Medium und sendet sie erst nach Auswahl aus. Das ist robuster als eine direkte Synthese während eines laufenden Funkrufs.

```toml
[tts]
enabled = true
endpoint = "http://127.0.0.1:5005"
cache_directory = "/var/cache/netcore/tts"
template_directory = "/var/lib/netcore/tts/templates"
auto_save_generated_templates = true
default_voice = "de-thorsten"
default_speed = 0.95
default_priority = 5
max_text_characters = 2000
synthesis_timeout_seconds = 90
max_output_file_mb = 25
cache_retention_minutes = 1440
keep_generated_audio = false
```

TTS-Dateien erhalten den Namen der Vorlage oder bei Freitext einen vom Bediener vergebenen Namen. So lassen sie sich später erneut auswählen und aussenden.

## Dateirechte

```bash
sudo install -d -o netcore -g netcore /var/lib/netcore/recordings
sudo install -d -o netcore -g netcore /var/lib/netcore/audio
sudo install -d -o netcore -g netcore /var/lib/netcore/tts/templates
sudo install -d -o netcore -g netcore /var/cache/netcore/audio
sudo install -d -o netcore -g netcore /var/cache/netcore/tts
```

Benutzer und Gruppe müssen zur Systemd-Unit passen.

## Fehlersuche

### Datei ist sichtbar, Ruf startet aber nicht

- `ffmpeg` vorhanden und ausführbar?
- native Sprachcodec-Bibliothek korrekt gelinkt?
- Ziel-GSSI/ISSI gültig und erreichbar?
- bereits ein Ruf oder eine Audio-Aussendung aktiv?
- Cache-Verzeichnis beschreibbar?

### TTS klappt nur einmal oder Endgerät hört Folgerufe nicht

- TTS nicht direkt synthetisieren und gleichzeitig senden.
- Datei zuerst vollständig erzeugen, speichern und anschließend aus der Medienliste senden.
- Ruf-Release und Hangtime im Log kontrollieren.
- bei gerätespezifischem Verhalten Re-Registrierung und Call-State prüfen.

### NFS-Kopie fehlt

```bash
mount | grep nfs
sudo -u netcore test -w /mnt/nfs-share/Recordings && echo schreibbar
sudo journalctl -u tetra.service -n 300 --no-pager | grep -iE 'archive|nfs|record|tts'
```

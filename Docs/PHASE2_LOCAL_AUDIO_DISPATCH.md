# NetCore FlowStation – Phase 2: lokale WAV-/MP3-Aussendung

## Ziel

Phase 2 erweitert den funktionierenden lokalen Recorder um eine lokale Audioaussendung über TETRA.
Audiodateien werden vollständig vorbereitet, bevor ein Verkehrskanal angefordert wird. Der RF-/TDMA-Kern wartet daher während einer laufenden Aussendung weder auf Dateisystemzugriffe noch auf `ffmpeg`.

Enthalten sind:

- lokaler WAV-/MP3-Dateibrowser im bestehenden FlowStation-Dashboard
- Windows-artiges Kontextmenü per Rechtsklick
- zusätzlicher sichtbarer „Senden an …“-Button für Touchgeräte
- Zielwahl aus dem vorhandenen Gruppen- und Geräteverzeichnis
- manuelle Eingabe einer GSSI oder ISSI
- Gruppenaussendung
- Einzelruf mit Annahme-Timeout
- Fortschritts-, Call- und Timeslotanzeige
- manuelles Stoppen und sauberer Rufabbau
- erneutes Aussenden vorhandener lokaler Aufzeichnungen
- genau eine aktive Aussendung gleichzeitig
- Unterstützung der logischen Timeslots 1–8 und beider Carrier

NAS/SMB/NFS ist bewusst noch nicht Teil dieser Phase. Das folgt in Phase 3 auf demselben Storage- und Browserkonzept.

## Architektur

```text
Dashboard
  ├─ GET  /api/audio/status
  ├─ GET  /api/audio/browse
  ├─ POST /api/audio/play
  └─ POST /api/audio/stop
          │
          ▼
AudioPlayerHandle
          │
          ▼
AudioPlayerEntity
  ├─ Vorbereitungs-Worker
  │    WAV/MP3 → Mono-PCM → 8 kHz → ACELP/TMD
  │
  ├─ Gruppenruf: NetworkCallStart / Ready / End
  ├─ Einzelruf: NetworkCircuitSetup / MediaReady / Release
  └─ TDMA-synchron je 60 ms ein TMD-Sprachblock
          │
          ▼
UMAC → LMAC → TETRA-Funk
```

## Unterstützte Audioquellen

### WAV

Nativ verarbeitet werden:

- PCM 8, 16, 24 oder 32 Bit
- IEEE Float 32 Bit
- Mono und Mehrkanal
- beliebige Abtastrate mit linearer Konvertierung auf 8 kHz

Nicht nativ unterstützte WAV-Layouts werden über `ffmpeg` konvertiert, sofern es installiert ist.

### MP3

MP3 wird über `ffmpeg` in 8-kHz-Mono-PCM konvertiert.

### Aufzeichnungen

Fertige WAV-Aufzeichnungen aus Phase 1 können direkt aus dem Tab **AUFZEICHNUNGEN** über **Senden** oder Rechtsklick erneut ausgesendet werden.

## Konfiguration

```toml
[audio_player]
enabled = true
directory = "/var/lib/netcore/audio"
source_issi = 4010099
default_priority = 5
max_file_size_mb = 100
max_duration_seconds = 1800
tail_silence_blocks = 3
individual_answer_timeout_seconds = 30
ffmpeg_path = "ffmpeg"
```

### Hinweise

- `source_issi` ist die im TETRA-Ruf angezeigte NetCore-Audio-Identität.
- `default_priority` muss zwischen 0 und 15 liegen.
- `tail_silence_blocks` verhindert ein abgeschnittenes Dateiende.
- `individual_answer_timeout_seconds` gilt für Rufaufbau beziehungsweise Annahme eines Einzelrufs.
- Die Datei wird vor dem Ruf vollständig decodiert und ACELP-codiert.

## Lokales Medienverzeichnis vorbereiten

Dienstbenutzer prüfen:

```bash
systemctl cat bluestation-bs | grep -E '^(User|Group|ExecStart)='
```

Verzeichnis anlegen:

```bash
sudo install -d -m 0750 /var/lib/netcore/audio
```

Beispiel mit eigenem Dienstbenutzer:

```bash
sudo chown bluestation:bluestation /var/lib/netcore/audio
```

`bluestation:bluestation` durch den tatsächlich verwendeten Benutzer und die Gruppe ersetzen.

Beispielstruktur:

```text
/var/lib/netcore/audio/
├── Alarmierungen/
├── Durchsagen/
├── Gong/
└── Test/
```

Dateien kopieren:

```bash
sudo cp Probealarm.wav /var/lib/netcore/audio/Alarmierungen/
sudo cp Evakuierung.mp3 /var/lib/netcore/audio/Durchsagen/
```

## ffmpeg installieren

Für MP3 und nicht nativ unterstützte WAV-Dateien:

```bash
sudo apt-get update
sudo apt-get install -y ffmpeg
ffmpeg -version
```

Normale 8-kHz-/16-Bit-PCM-WAV-Dateien aus dem Recorder funktionieren auch ohne `ffmpeg`.

## Sauber bauen

Alte Artefakte vollständig entfernen:

```bash
cd ~/flowstation
rm -rf target
cargo clean
```

Alle verwendeten Komponenten bauen:

```bash
cargo build --release \
  -p bluestation-bs \
  -p netcore-control-room \
  -p netcore-control-room-operator \
  --features "bluestation-bs/asterisk,bluestation-bs/recording,bluestation-bs/audio-player"
```

Nur die Basisstation:

```bash
cargo build --release \
  -p bluestation-bs \
  --features "asterisk,recording,audio-player"
```

## Binary installieren

Tatsächlichen Dienstpfad prüfen:

```bash
systemctl cat bluestation-bs | grep '^ExecStart='
```

Beispiel `/usr/local/bin/bluestation-bs`:

```bash
sudo systemctl stop bluestation-bs
sudo rm -f /usr/local/bin/bluestation-bs
sudo install -m 0755 target/release/bluestation-bs /usr/local/bin/bluestation-bs
sudo systemctl daemon-reload
sudo systemctl start bluestation-bs
sudo systemctl status bluestation-bs --no-pager
```

Logs:

```bash
sudo journalctl -u bluestation-bs -f
```

Erwartete Startmeldung:

```text
Local WAV/MP3 audio dispatch enabled (/var/lib/netcore/audio)
```

## Bedienung

### Lokale Datei senden

1. Dashboard öffnen.
2. **SYSTEM → AUSSENDEN** öffnen.
3. Ordner und WAV-/MP3-Datei auswählen.
4. Rechtsklick oder **Senden an …**.
5. **Gruppe** oder **Einzelgerät** wählen.
6. Ziel aus dem Verzeichnis auswählen oder ISSI/GSSI manuell eingeben.
7. Priorität prüfen.
8. **Jetzt senden**.

### Aufzeichnung erneut senden

1. **SYSTEM → AUFZEICHNUNGEN** öffnen.
2. Bei der gewünschten Aufnahme **Senden** wählen oder Rechtsklick verwenden.
3. Ziel und Priorität auswählen.
4. Aussendung starten.

## API

### Status

```http
GET /api/audio/status
```

### Medienverzeichnis

```http
GET /api/audio/browse?path=Durchsagen
```

### Lokale Datei an Gruppe senden

```http
POST /api/audio/play
Content-Type: application/json

{
  "source_type": "media",
  "path": "Durchsagen/Evakuierung.mp3",
  "target_type": "group",
  "target_id": 1001,
  "priority": 5
}
```

### Aufzeichnung an Einzelgerät senden

```http
POST /api/audio/play
Content-Type: application/json

{
  "source_type": "recording",
  "recording_id": "RECORDING-ID",
  "target_type": "individual",
  "target_id": 2010001,
  "priority": 5
}
```

### Stoppen

```http
POST /api/audio/stop
```

## Technische Schutzmaßnahmen

- nur reguläre `.wav`- und `.mp3`-Dateien
- keine Symlinks im Medienbrowser
- canonicalisierte Pfade müssen unterhalb des Medienwurzelverzeichnisses bleiben
- `..` und andere ausbrechende Pfadkomponenten werden abgewiesen
- maximale Dateigröße und maximale Audiodauer
- vollständige Vorbereitung vor dem Rufaufbau
- nur eine aktive Aussendung
- Rufabbau bei Stop, Fehler und Dateiende
- konfigurierbares Silence-Tail
- veraltete Ergebnisse abgebrochener Vorbereitungsjobs werden ignoriert

## Einschränkungen von Phase 2

- TETRA ACELP ist ein Sprachcodec. Musik klingt entsprechend schmalbandig; Durchsagen, Gong und Warntexte sind der ideale Einsatz.
- MP3 benötigt `ffmpeg`.
- Einzelrufe beginnen erst nach Annahme beziehungsweise `MediaReady`.
- Ein Gruppenruf wird abgewiesen, wenn aktuell kein Teilnehmer an der Zielgruppe angemeldet ist.
- Es läuft bewusst nur eine Aussendung gleichzeitig.
- Netzwerkshares und NAS-Archivierung folgen in Phase 3.

## Abnahmetests

1. Recorder-WAV an eine lokal angemeldete Gruppe senden.
2. MP3 an dieselbe Gruppe senden.
3. Aussendung während der Wiedergabe stoppen.
4. Dateiende auf abgeschnittene letzte Silben prüfen.
5. Zielgruppe ohne Listener testen; UI muss einen Fehler melden.
6. Einzelgerät anrufen, annehmen und Audio prüfen.
7. Einzelruf nicht annehmen; Timeout und sauberer Rufabbau prüfen.
8. Carrier 1 und Carrier 2 beziehungsweise logische TS 1–8 testen.
9. Dateiname mit Leerzeichen, Umlauten und Sonderzeichen testen.
10. Basisstation nach einem abgebrochenen Vorbereitungsjob weiter betreiben; keine blockierte Audio-Session darf verbleiben.

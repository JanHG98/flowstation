# NetCore-TETRA – Phase 1: lokale Gesprächsaufzeichnung

## Umfang

Phase 1 ergänzt die Basisstation um eine passive lokale Sprachaufzeichnung mit Bedienung im vorhandenen Dashboard.

Enthalten sind:

- automatische Aufzeichnung lokal empfangener TETRA-Sprachframes,
- Gruppenrufe einschließlich Sprecherwechseln,
- Simplex-Einzelrufe,
- Duplex-Einzelrufe als getrennte Datei je lokalem Sprachpfad/Timeslot,
- Speicherung als WAV, 8 kHz, Mono, 16 Bit PCM,
- JSON-Metadaten je Aufnahme,
- temporäre Dateien und Wiederherstellung nach unsauberem Prozessende,
- Mindestfreiplatz, Aufnahmedauer-Limit und Aufbewahrungsfrist,
- Dashboard-Seite mit Status, Pause/Aktivierung, Suche, Wiedergabe, Download und Löschen.

Nicht Bestandteil von Phase 1 sind MP3/WAV-Aussendung und NAS/SMB/NFS. Diese bauen in Phase 2 und Phase 3 auf demselben Audio- und Storage-Unterbau auf.

## Audiofluss

```text
CMCE FloorGranted/FloorReleased/CallEnded
                  │
                  ├── Call-ID, Sprecher-ISSI, Ziel-SSI, Zieltyp, Timeslot
                  ▼
            RecorderEntity
                  ▲
                  │ gültige lokale UL-TMD-Sprachblöcke
                  │
                UMAC
                  │
                  ▼
      TETRA ACELP → 8-kHz PCM → WAV
```

Der Recorder verändert den Funkpfad nicht. UMAC sendet lediglich eine Kopie bereits akzeptierter Sprachblöcke an die Recorder-Entity.

## Dateistruktur

Standardpfad:

```text
/var/lib/netcore/recordings/
└── YYYY/
    └── MM/
        └── DD/
            ├── YYYY-MM-DD_HH-MM-SS_CALL-<id>_TS-<ts>_GSSI-<ziel>_<uuid>.wav
            └── YYYY-MM-DD_HH-MM-SS_CALL-<id>_TS-<ts>_GSSI-<ziel>_<uuid>.json
```

Während einer laufenden Aufnahme werden `.wav.part` und `.json.part` verwendet. Erst nach sauberem Abschluss wird die WAV-Datei finalisiert und veröffentlicht.

## Konfiguration

```toml
[recording]
enabled = true
active = true
directory = "/var/lib/netcore/recordings"
mode = "all"                    # all | selected_groups
selected_groups = []             # nur bei selected_groups
minimum_free_space_mb = 2048
retention_days = 30              # 0 deaktiviert automatische Bereinigung
max_recording_minutes = 120
idle_finalize_secs = 15
max_list_entries = 2000
```

`enabled` legt fest, ob die Recorder-Entity beim Start geladen wird. `active` ist der initiale Laufzeitstatus. Die Schaltfläche im Dashboard ändert den Laufzeitstatus bis zum nächsten Prozessstart; sie schreibt die TOML-Datei bewusst nicht automatisch um.

`mode = "selected_groups"` zeichnet ausschließlich Gruppenrufe auf, deren GSSI in `selected_groups` enthalten ist. Einzelrufe werden in diesem Modus nicht aufgenommen.

## Dashboard

Die neue Seite befindet sich unter:

```text
SYSTEM → AUFZEICHNUNGEN
```

Funktionen:

- Aufzeichnung aktivieren oder pausieren,
- aktive Sessions und Call-IDs anzeigen,
- freien und belegten Speicher anzeigen,
- Aufnahmen durchsuchen,
- WAV im Browser wiedergeben,
- WAV herunterladen,
- Aufnahme und JSON-Metadaten löschen.

Gerätebezeichnungen werden aus `/api/devices`, Gruppenbezeichnungen aus `/api/groups` ergänzt. Ohne Directory-Dienst werden ISSI und GSSI numerisch angezeigt.

## HTTP-API

```text
GET    /api/recordings/status
GET    /api/recordings
POST   /api/recordings/state
GET    /api/recordings/{uuid}
GET    /api/recordings/{uuid}/metadata
GET    /api/recordings/{uuid}/audio
DELETE /api/recordings/{uuid}
```

Beispiel zum Pausieren:

```json
{"active": false}
```

Die vorhandene Dashboard-Authentifizierung schützt auch diese Endpunkte.

## Installation und Build

### 1. Alte Build-Artefakte entfernen

```bash
cd /pfad/zu/flowstation
rm -rf target
cargo clean
```

### 2. Aufnahmeverzeichnis anlegen

Der Benutzer des `bluestation-bs`-Dienstes benötigt Schreibrechte. Beispiel für den Dienstbenutzer `jan`:

```bash
sudo install -d -o jan -g jan -m 0750 /var/lib/netcore/recordings
```

Den Benutzer und die Gruppe an die tatsächliche systemd-Unit anpassen.

### 3. Native Codec-Bibliothek prüfen

Phase 1 verwendet dieselbe `libtetra-codec`-Installation wie die vorhandene Asterisk-Integration:

```bash
pkg-config --libs tetra-codec
ldconfig -p | grep tetra-codec
```

### 4. Basisstation bauen

```bash
cargo build --release -p bluestation-bs --features "asterisk recording"
```

Die Features sind beim gelieferten Stand bereits Standard. Die explizite Angabe macht den Build jedoch eindeutig.

### 5. Alten Dienst stoppen und Binärdatei ersetzen

```bash
sudo systemctl stop bluestation-bs
sudo install -m 0755 target/release/bluestation-bs /usr/local/bin/bluestation-bs
sudo systemctl start bluestation-bs
```

Falls die Unit einen anderen Binärpfad oder Namen verwendet, diesen entsprechend ersetzen.

### 6. Prüfung

```bash
sudo systemctl status bluestation-bs --no-pager
sudo journalctl -u bluestation-bs -n 200 --no-pager
sudo journalctl -u bluestation-bs -f
```

Erwartete Startmeldung:

```text
Local WAV recording enabled (/var/lib/netcore/recordings)
```

Anschließend einen kurzen Gruppenruf durchführen und im Dashboard unter `AUFZEICHNUNGEN` prüfen.

## Verhalten bei Fehlern

- Ist das Aufnahmeverzeichnis nicht beschreibbar, läuft der Funkstack weiter; der Recorder wird als nicht verfügbar angezeigt.
- Unterhalb des konfigurierten Mindestfreiplatzes wird keine neue Aufnahme begonnen.
- Nach einem unsauberen Ende werden `.wav.part`-Dateien beim nächsten Start repariert und als wiederhergestellt markiert.
- Nach `max_recording_minutes` wird eine verwaiste Session beendet.
- Nach `idle_finalize_secs` ohne aktiven Floor oder Call-Ende wird eine inaktive Session abgeschlossen.

## Grenzen von Phase 1

- Aufgezeichnet wird der lokal über RF empfangene Uplink. Reine Remote-/Brew-/Asterisk-Downlink-Audiosignale ohne lokalen RF-Uplink sind noch nicht Bestandteil dieser Phase.
- Gruppenrufe werden als sprachaktive Aufnahme gespeichert; längere Pausen zwischen Sprechern werden nicht künstlich als Stille aufgefüllt. Sprechersegmente stehen im JSON-Sidecar.
- HTTP-Range/komfortables Springen innerhalb sehr großer WAV-Dateien ist noch nicht implementiert.
- Der UI-Schalter ist ein Laufzeitschalter und wird beim Neustart aus `active` in der Konfiguration neu initialisiert.
- Die rechtliche Zulässigkeit, Kennzeichnung und Aufbewahrung von Gesprächsaufzeichnungen muss für den konkreten Betrieb eigenverantwortlich geklärt werden.

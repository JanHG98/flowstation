# FlowStation TTS — Schritt 1: lokale Vorlagen

## Funktionsumfang

Dieser Stand ergänzt die bestehende TTS-Aussendung um einen lokalen, persistenten Vorlagenspeicher.
Die Vorlagen liegen nicht in einer Datenbank, sondern als einzeln bearbeitbare TOML-Dateien unter:

```text
/var/lib/netcore/tts/templates
```

Unterstützt werden:

- Vorlagen im Dashboard auswählen und in den TTS-Editor laden
- neue Vorlagen speichern
- vorhandene Vorlagen überschreiben oder umbenennen
- Vorlagen löschen
- Text, Stimme, Tempo und Priorität speichern
- optional Zielart und ISSI/GSSI speichern
- jede erfolgreich erzeugte TTS-Datei automatisch als Vorlage sichern
- identische Inhalte deduplizieren
- atomare Dateischreibvorgänge
- sichere Dateinamen und feste lokale Ablage

Der NFS-Ordner `TTS-Dateien` wird in Schritt 2 als zusätzliche Serverquelle eingebunden.

## Dateiformat

Beispiel:

```toml
schema_version = 1
id = "evakuierung-haupthalle-a1b2c3d4"
name = "Evakuierung Haupthalle"
text = """
Achtung.

Das Gebäude muss geräumt werden.
"""
voice_id = "de-thorsten"
speed = 0.95
priority = 10
target_type = "group"
target_id = 15201
auto_saved = false
created_at = "2026-07-20T10:30:00Z"
updated_at = "2026-07-20T10:32:00Z"
```

Dateien heißen immer:

```text
<id>.tts.toml
```

Die ID darf ausschließlich Buchstaben, Zahlen, Punkt, Bindestrich und Unterstrich enthalten.

## Automatisches Speichern

Mit:

```toml
auto_save_generated_templates = true
```

wird nach erfolgreicher Piper-Synthese automatisch eine Vorlage gespeichert. Fehlgeschlagene
Synthesen erzeugen keine Vorlage. Existiert bereits eine Vorlage mit exakt demselben Text,
derselben Stimme, demselben Tempo, derselben Priorität und demselben Ziel, wird kein Duplikat
angelegt.

Automatisch erzeugte Einträge werden im Dashboard mit `AUTO` markiert. Sobald ein solcher Eintrag
manuell unter einem Namen gespeichert wird, wird er zu einer normalen Vorlage.

## API

```http
GET  /api/audio/tts/templates
POST /api/audio/tts/templates/save
POST /api/audio/tts/templates/delete
```

Speichern:

```json
{
  "id": "optional-existing-id",
  "name": "Evakuierung Haupthalle",
  "text": "Achtung. Das Gebäude muss geräumt werden.",
  "voice_id": "de-thorsten",
  "speed": 0.95,
  "priority": 10,
  "target_type": "group",
  "target_id": 15201
}
```

Löschen:

```json
{
  "id": "evakuierung-haupthalle-a1b2c3d4"
}
```

## Stimmen

Konfiguriert sind:

```text
Deutsch – Thorsten (mittel)
Deutsch – Thorsten (hoch)
Deutsch – Karlsson
Deutsch – Pavoque
Deutsch – Thorsten emotional (neutral)
```

Die Piper-Modelle müssen tatsächlich unter `/var/lib/netcore/piper` vorhanden sein. FlowStation
prüft `/voices` und deaktiviert fehlende Stimmen im Auswahlfeld. Dadurch kann Piper nicht mehr
unbemerkt auf die Standardstimme zurückfallen.

## Konfiguration

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
```

## Sicherheit und Robustheit

- Vorlagenpfade werden ausschließlich aus geprüften IDs gebildet.
- Beliebige absolute oder relative Dateipfade werden nicht akzeptiert.
- Pro Vorlagendatei gelten maximal 256 KiB.
- Es werden nur reguläre Dateien mit Endung `.tts.toml` eingelesen.
- Speichern erfolgt zunächst in eine temporäre Datei, danach atomar per Rename.
- Temporäre Dateien werden bei Fehlern entfernt.
- Vorlagenfehler deaktivieren nicht die gesamte TTS-Funktion.
- Ungültige manuell bearbeitete Dateien werden übersprungen und im Log gemeldet.

## Vorbereitung des lokalen Verzeichnisses

```bash
SERVICE_USER="$(systemctl show tetra.service -p User --value)"
SERVICE_GROUP="$(systemctl show tetra.service -p Group --value)"

[ -n "$SERVICE_USER" ] || SERVICE_USER=root
[ -n "$SERVICE_GROUP" ] || SERVICE_GROUP="$(id -gn "$SERVICE_USER")"

sudo install -d \
  -o "$SERVICE_USER" \
  -g "$SERVICE_GROUP" \
  -m 0750 \
  /var/lib/netcore/tts/templates
```

Prüfung:

```bash
sudo -u "$SERVICE_USER" sh -c '
  touch /var/lib/netcore/tts/templates/.write-test &&
  rm /var/lib/netcore/tts/templates/.write-test
'
```

# Phase 4: TTS als WAV in der Aufzeichnungsbibliothek

## Ziel

TTS startet keinen Funkruf mehr. Piper erzeugt zuerst eine vollständige WAV-Datei. Diese wird anschließend als regulärer Eintrag in der lokalen Aufzeichnungsbibliothek gespeichert. Eine spätere Aussendung erfolgt ausschließlich über den vorhandenen Aufzeichnungs-Workflow.

## Bedienablauf

1. Vorlage auswählen oder Freitext eingeben.
2. Bei einer Vorlage wird deren Name automatisch als WAV-Name verwendet.
3. Bei Freitext muss ein eigener Name mit 1 bis 120 Zeichen eingegeben werden.
4. `Als WAV in Aufzeichnungen speichern` ausführen.
5. Die neue Datei im Abschnitt `Aufzeichnungen & TTS-WAVs` auswählen.
6. Dort `Senden` verwenden und Ziel sowie Priorität festlegen.

## Speicherung

Die erzeugte Datei wird vor dem Import auf das gleiche Format wie Gesprächsaufzeichnungen gebracht:

- PCM signed 16-bit little-endian
- Mono
- 8.000 Hz
- WAV mit JSON-Metadaten

Beispiel für einen Dateinamen:

```text
TTS-Einlass_verzögert_2026-07-20_18-30-00_<UUID>.wav
```

Der sichtbare Titel in der Oberfläche bleibt exakt der Vorlagen- beziehungsweise Freitextname. Zeitstempel und UUID verhindern, dass eine erneute Erzeugung eine vorhandene Datei überschreibt.

## Gemeinsamer Recording-Pfad

TTS-WAVs werden über `RecorderHandle::import_named_wav()` importiert. Dadurch verwenden sie danach dieselben Funktionen wie normale Aufzeichnungen:

- Auflistung über `/api/recordings`
- Vorschau über `/api/recordings/<id>/audio`
- Aussendung über `source_type=recording`
- Löschen
- Aufbewahrungslogik
- NFS-Archivierung und Archivmarker

## Entfernt

- TTS-Schaltfläche zum direkten Senden
- `POST /api/audio/tts/send`
- direkte TTS-Übergabe an den AudioPlayer
- Ziel- und Prioritätsauswahl im TTS-Formular

## Logmeldungen

Nach erfolgreicher Erzeugung erscheinen unter anderem:

```text
TTS: materialized complete recording-format WAV ...
Recorder: imported tts WAV title=... id=... duration_ms=... path=...
TTS: saved as recording job=... recording_id=... name=...
```

Beim späteren Senden erscheint der Eintrag wie jede andere Aufzeichnung im AudioPlayer und CMCE.

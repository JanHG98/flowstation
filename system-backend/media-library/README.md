# NetCore-Tetra Media Library

## Zweck

Die Media Library ist die zentrale, kontrollierte Ablage für Durchsagen, TTS-Dateien, Alarmtöne, importierte Recorder-Ausschnitte und vorbereitete TETRA-Sprachframes. Sie trennt Dateisystem, Vorschau, Freigabe und Playout vom zeitkritischen Media Switch.

Der Dienst läuft auf **Port 8230** und besitzt eine eigene WebUI.

## Enthalten

- Upload von WAV, MP3 und gepacktem `.tacelp`
- Import per URL mit optionaler Größe und SHA-256
- nativer Importvertrag `netcore-media-import-v1` für den Application Gateway
- gezielter Recorder-Import über dessen unveränderte `audio.tacelp`-Kopie
- persistente Asset-Metadaten, Tags, Quelle, TTS-Stimme/Text und Broadcast-Hinweise
- RIFF/WAVE-Parser und strikte TACELP-Frameausrichtung
- kanonische Vorschau als 8 kHz, mono, PCM16 WAV
- Waveform-Peaks für die WebUI
- Freigabezustände `draft`, `approved`, `rejected`
- optionale externe TETRA-Encoder-/Decoder-Helfer ohne Shell-Ausführung
- verlustfreier TETRA-Cache mit exakt 35 Byte pro 60-ms-Frame
- kontrollierte Einspeisung in **bereits bestehende** Media-Switch-Sessions
- Shadow- und Authoritative-Modus
- Jobfortschritt, Abbruch und bewusster manueller Retry ab Frame 0
- versionierte NFS-/Archivkopie mit Manifest und Dateihashes, ohne das Archiv als Live-Playout-Quelle zu missbrauchen
- WebUI, REST-API, OpenAPI, Prometheus-Metriken, Audit, Backup und Export
- systemd- und LXC-Installationsskripte

## Architekturgrenze

```text
Application Gateway / Upload / Recorder
                 │
                 ▼
          Media Library
  Original → Preview → TACELP-Cache
                 │
          Freigabe + Job
                 ▼
      bestehende Media Session
                 │
                 ▼
           Media Switch
```

Die Media Library:

- erstellt **keinen CMCE-Ruf**,
- besitzt **keinen Floor-Control-State**,
- ändert **keine Recorder-Retention und keinen Legal Hold**,
- transkodiert nicht im Media-Switch-Prozess,
- injiziert nur validierte 35-Byte-TETRA-Frames.

## Open Lab

Die aktuelle Teststufe ist absichtlich offen:

- keine Benutzeranmeldung,
- keine Tokens,
- kein TLS.

Jeder erreichbare Client kann Assets hochladen, freigeben, archivieren, löschen und aussenden. Der LXC gehört deshalb in ein isoliertes Managementnetz.

## WebUI

```text
http://<media-library-lxc>:8230/
```

Die Oberfläche enthält Übersicht, Bibliothek, Upload/URL-Import, Recorder-Import, Vorschau, Aussendung, Jobs, Storage/Audit und API.

## Audio- und Codec-Verhalten

WAV und MP3 werden zu einem kanonischen Vorschauformat normalisiert:

```text
8 kHz · mono · signed 16-bit PCM · RIFF/WAVE
```

Ohne konfigurierten TETRA-Encoder bleiben WAV/MP3 **previewfähig, aber nicht funkbereit**. Das ist Absicht. Eine Dateiendung wird nicht als Codec-Ersatz behandelt.

Gepackte `.tacelp`-Dateien sind sofort funkbereit, wenn ihre Größe ein positives Vielfaches von 35 Byte ist. Für deren hörbare Vorschau ist ein konfigurierter Decoder-Helfer erforderlich.

## Playout

Für einen Job wird eine vorhandene `session_id` benötigt. Vor dem Start prüft der Worker den gespeicherten SHA-256 des TETRA-Caches. Anschließend liest er `audio.tacelp` frameweise und ruft im festen 60-ms-Takt auf:

```text
POST /api/v1/sessions/{session_id}/inject
```

Ein Neustart während eines laufenden Jobs markiert ihn als fehlgeschlagen. Er wird nicht automatisch neu gestartet, weil eine teilweise doppelte Durchsage schlimmer ist als ein sichtbarer Fehler.

## Wichtige Endpunkte

```text
GET  /health/live
GET  /health/ready
GET  /metrics
GET  /openapi.json
GET  /api/v1/assets
POST /api/v1/assets/upload-json
POST /api/v1/assets/import-url
POST /api/v1/recorder/import
POST /api/v1/assets/{id}/approve
GET  /api/v1/assets/{id}/preview
GET  /api/v1/assets/{id}/audio.tacelp
POST /api/v1/dispatch
GET  /api/v1/jobs
```

## Bewusste Grenzen

Noch nicht enthalten sind:

- produktives RBAC, TLS oder mTLS,
- eingebettete proprietäre TETRA-Sprachcodec-Algorithmen,
- Musik- oder Lautheits-Mastering jenseits der technischen Normalisierung,
- CMCE-Call-Erzeugung und Floor-Control,
- framegenaue verteilte Playout-Synchronisierung über mehrere Regionen,
- S3-/Object-Storage oder Medien-CDN,
- rechtssichere WORM-Archivierung.

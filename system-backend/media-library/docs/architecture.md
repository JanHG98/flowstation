# Architektur

## Eigentümerschaft

| Zustand | Eigentümer |
|---|---|
| Originaldatei, Preview, TACELP-Cache, Freigabe | Media Library |
| laufender Ruf, Legs, Floor, Priorität | Call Control |
| gepufferte und geroutete Sprachframes | Media Switch |
| unverändertes Beweis-/Recorderarchiv | Recorder |
| TTS-Synthese und Fremdsystem-Connector | Application Gateway |

## Pipeline

1. Import schreibt immer zuerst eine partielle Datei und veröffentlicht sie atomar.
2. SHA-256 und Größenlimit werden vor der Verarbeitung geprüft.
3. WAV/MP3 wird zu 8-kHz-Mono-PCM16 normalisiert.
4. Ein TACELP-Cache wird nur nach erfolgreicher Encoder-Ausgabe oder bei gültigem Rohimport markiert.
5. Eine Aussendung erfordert `ready`, `approved` und `broadcast_ready`.
6. Der Media Switch erhält ausschließlich einzelne 35-Byte-Frames.

## Ausfallverhalten

- Import-/Codec-Fehler stoppen keinen anderen Dienst.
- Ein Media-Library-Ausfall erzeugt keine Backpressure im Media Switch.
- Ein Playout-Neustart wird nicht automatisch wiederholt.
- NFS ist nur Archivziel; ein NFS-Ausfall blockiert keine lokale Vorschau oder lokale Aussendung.

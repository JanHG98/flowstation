# SWMI Core 1 – Package O: Media Library

## Ergebnis

Dieses Paket implementiert den eigenständigen Media-Library-LXC auf Port 8230.

## Enthalten

- Upload und URL-Import für WAV, MP3 und gepacktes TETRA-ACELP,
- `netcore-media-import-v1` als Application-Gateway-Vertrag,
- Recorder-Rohimport ohne Veränderung des Recorder-Archivs,
- SHA-256 für Original, Preview und TETRA-Cache, Größenlimits, Deduplizierungshinweise und atomare Ablage,
- 8-kHz-Mono-PCM16-Vorschau und WebUI-Waveform,
- optionaler externer Encoder-/Decoder-Vertrag,
- Freigabe vor Playout,
- versionierte NFS-Archivkopie mit Manifest und Hashprüfung,
- Shadow-/Authoritative-Modus,
- frameweises Playout in bestehende Media-Switch-Sessions,
- WebUI, REST-API, OpenAPI, Metrics, Audit, Backup und Export,
- systemd-, LXC-, Checker- und CI-Dateien.

## Sicherheitsstatus

Die Management-Ebene bleibt projektgemäß `open_lab`: keine Anmeldung, keine Tokens und kein TLS. Codec-Befehle sind ausschließlich statische Konfiguration und nicht über die API änderbar.

## Ehrliche Codec-Grenze

Das Paket enthält keinen vorgetäuschten TETRA-Sprachcodec. WAV/MP3 sind ohne externen Encoder nur previewfähig. Gültige `.tacelp`-Assets sind direkt funkbereit. Ein Decoder ist nur für deren hörbare Vorschau nötig.

## Betriebsgrenze

Die Media Library erzeugt keinen Ruf und übernimmt keinen Floor. Sie benötigt eine existierende Media-Switch-Session und verwendet deren vorbereitete Injection-Schnittstelle.

## Nächster Baustein

`shared`: gemeinsame Verträge, UI-Bausteine und eine abschließende LXC-übergreifende Integrations-/Deployment-Schicht. Fachlich deploybare Kerndienste sind mit der Media Library vollständig angelegt.

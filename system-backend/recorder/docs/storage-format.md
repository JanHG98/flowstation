# Speicher- und Archivformat

## Audio

`audio.tacelp` ist kein WAV-Container. Die Datei enthält die vom Media Switch gelieferten gepackten TETRA Speech Service 0 Frames unverändert hintereinander:

- 35 Bytes je Frame
- standardmäßig 60 ms je Frame
- keine Transkodierung
- keine Lautstärkeänderung
- kein Resampling

Frame `n` beginnt grundsätzlich bei `byte_offset = n × 35`. Der JSONL-Index enthält den tatsächlichen Offset zusätzlich explizit.

## Index

Jede Zeile in `frames.jsonl` ist ein unabhängiges JSON-Objekt mit:

- Tap-Sequenz
- RFC-3339-Zeitstempel
- Sequenz der Quell-TBS
- Node-ID und logischem Timeslot
- Sprecher-ISSI, soweit Call Control einen Floor Holder meldet
- Injection-Kennzeichen
- Byte-Offset und Payload-Länge

## Metadaten

`metadata.json` beschreibt den vollständigen Ruf und enthält unter anderem GSSI beziehungsweise Teilnehmer-ISSIs, Priorität, Notrufkennzeichen, Quell-TBS, Sprecher, Segmente, Retention und erkannte Tap-Lücken.

## Integrität

`integrity.json` enthält getrennte SHA-256-Werte für `audio.tacelp` und `frames.jsonl`. Eine Prüfung verändert nur `integrity_status` und `last_verified_at` in den Metadaten; die Audio- und Indexdatei bleiben unverändert.

## Crash-Recovery

Während einer aktiven Aufnahme werden verwendet:

```text
audio.tacelp.part
frames.jsonl.part
metadata.active.json
```

Audio und Index werden periodisch geflusht und synchronisiert, danach wird das aktive Manifest aktualisiert. Beim Neustart finalisiert der Dienst gefundene aktive Manifeste als `unclean_shutdown_recovery`, berechnet neue Hashes und veröffentlicht normale Metadaten.

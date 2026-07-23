# Recorder- und Player-Schnittstellen

Der Media Switch stellt zwei bewusst getrennte Tap-Arten bereit.

## Diagnose-Tap

```text
GET /api/v1/taps?limit=<n>
```

Dieser Endpunkt liefert nur kompakte Metadaten für WebUI und Fehlersuche. Er enthält keine Sprachpayloads und ist nicht für Aufzeichnung oder Wiedergabe geeignet.

## Replay-fähiger Recorder-Tap

```text
GET /api/v1/recorder/taps?after=<sequenz>&limit=<n>
```

Jeder Datensatz enthält:

- monotone Tap-Sequenz
- Zeitstempel
- logische Call-ID, Rufart und Rufphase
- Source-ISSI, GSSI beziehungsweise Calling-/Called-ISSI
- Priorität und Notrufkennzeichen
- aktuellen Floor Holder als Sprecher-ISSI
- Quell-TBS, logischen Timeslot und Quellsequenz
- Zielanzahl
- Codec-Bezeichnung
- vollständige gepackte Payload
- Kennzeichen für künstlich eingespeiste Frames

Die Antwort nennt außerdem älteste und neueste Sequenz im Ring sowie `dropped_before`, wenn der angefragte Cursor nicht mehr vollständig verfügbar ist.

Der Ring wird über `media.recorder_tap_history_frames` begrenzt. Er dient ausschließlich zur kurzen Entkopplung und ist kein persistentes Archiv.

## Audio Player / Media Library

Die bestehende Injection-API bleibt der Anschluss für spätere Wiedergabe:

```text
POST /api/v1/sessions/{call-id}/inject
```

Sie akzeptiert exakt einen gepackten 35-Byte-TETRA-ACELP-Frame. Ein Audio Player kann daher `audio.tacelp` frameweise lesen und im ursprünglichen 60-ms-Takt einspeisen, ohne den Media Switch um einen Codec oder Dateisystemzugriff zu erweitern.

## Entkopplungsregel

Recorder und Player bleiben externe Dienste. Der Media Switch führt keine synchronen Aufrufe zu ihnen aus und wartet nicht auf deren Verarbeitung. Ein Recorder-Ausfall darf einen Ruf nicht beeinflussen; ein Player-Ausfall darf höchstens die jeweilige Injection beenden.

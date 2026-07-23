# Media-Switch-Tap

Der Recorder liest den ausschließlich für Recorder vorgesehenen Endpunkt:

```text
GET /api/v1/recorder/taps?after=<letzte-sequenz>&limit=<batch>
```

Die Antwort enthält:

- `requested_after`
- älteste und neueste im Ring verfügbare Sequenz
- `dropped_before`, falls der Cursor bereits aus dem Ring gefallen ist
- vollständige Tap Records einschließlich des 35-Byte-Payloads

## Cursor-Verhalten

Der Recorder bestätigt nicht aktiv. Er hält lokal die höchste vollständig verarbeitete Tap-Sequenz und fragt ab dort weiter. Duplikate werden verworfen. Sprünge werden als Verlust gezählt und als Ereignis protokolliert.

Startet der Media Switch neu und seine Sequenz liegt wieder unter dem Recorder-Cursor, setzt der Recorder seinen Cursor kontrolliert zurück. Aktive Aufnahmen bleiben bestehen; eine mögliche Lücke ist im Ereignisverlauf sichtbar.

## Entkopplung

Der Tap ist ein begrenzter In-Memory-Ring. Das Einfügen eines Frames ist Teil der lokalen Media-Switch-Zustandsaktualisierung, aber es gibt weder einen Netzwerkaufruf zum Recorder noch Backpressure vom Recorder in den Rufpfad.

Die Ringgröße wird im Media Switch über `media.recorder_tap_history_frames` festgelegt. Bei 20.000 Frames überbrückt sie für einen einzelnen Sprecher ungefähr 20 Minuten.

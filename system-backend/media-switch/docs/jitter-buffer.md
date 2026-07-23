# Jitter-Puffer

Jeder Zielstream besitzt einen begrenzten FIFO-Puffer. Die Startverzögerung ergibt sich aus:

```text
frame_duration_ms * jitter_buffer_frames
```

Mit den Standardwerten sind dies `60 ms * 3 = 180 ms`. Pro Stream sind höchstens `max_jitter_buffer_frames` Frames und global höchstens `max_pending_frames` Frames erlaubt.

Bei Überlauf wird der älteste Frame des betroffenen Zielstreams entfernt. Dadurch wächst die Latenz nicht unbegrenzt. Duplikate oder rückwärts laufende Sequenznummern werden bereits vor dem Puffer verworfen.

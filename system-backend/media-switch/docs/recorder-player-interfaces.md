# Recorder- und Audio-Player-Schnittstellen

## Recorder

`GET /api/v1/taps` liefert pro empfangenem oder eingespeistem Frame Metadaten wie Session, Quelle, Sequenz, Zielanzahl und Payload-Größe. Das ist die vorbereitete Kontrollschnittstelle. Der nächste Recorder-Baustein erhält zusätzlich einen eigenen verlustarmen Frame-Tap, ohne den Routingpfad zu blockieren.

## Audio-Player / Media Library

`POST /api/v1/sessions/{session_id}/inject` akzeptiert exakt 35 bereits gepackte TETRA-ACELP-Bytes. Optional kann die Einspeisung auf Node und Timeslot begrenzt werden. Die spätere Media Library beziehungsweise der Audio-Player codiert WAV/MP3 zunächst in dieses Format und nutzt dann denselben Sessionpfad wie Funk-Audio.

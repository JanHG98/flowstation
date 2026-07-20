# Phase 4 – TTS als vollständig materialisierte Recording-Datei

## Anlass

Manuell ausgewählte Gesprächsaufzeichnungen wurden zuverlässig ausgesendet, während TTS-
Durchsagen sporadisch erst nach einem `RoamingLocationUpdating` am Funkgerät ankamen. Gewünscht
war deshalb ein harter Gleichlauf: TTS erst vollständig erzeugen und anschließend exakt wie eine
Recording-WAV behandeln.

## Was der Log zeigt

Die Synthese war bereits nicht-streamend: Piper schrieb zuerst eine vollständige WAV, danach
meldete TTS `preview ready`, und erst beim späteren Senden übernahm der AudioPlayer die Datei.
Der Funkruf startete erst nach `AudioPlayer: prepared`, also nachdem die komplette WAV dekodiert
und vollständig in TETRA-ACELP-Blöcke umgewandelt war.

Der auffällige Unterschied lag im Rufzeitpunkt. Beim problematischen TTS-Ruf trafen unmittelbar
nach dem initialen D-SETUP zusätzlich Backup- und Energy-Economy-Ankündigung im selben
Schedulerfenster ein. Der Log zeigte daraufhin:

```text
dl_enqueue_tma: ... deferring chan_alloc PDU to next frame (slot capacity)
-> Fragged MacResource ... chan_alloc_element: Some(...)
```

Das ist kein Piper-/WAV-Problem, sondern eine Kollision mehrerer D-SETUP-PDUs mit
Kanalzuweisung. Ein kurz darauf erfolgender Re-Register bringt das Funkgerät erneut auf den
Steuerkanal und erklärt, warum es danach in den laufenden Ruf einsteigt.

## Änderungen

### 1. TTS-WAV explizit vollständig schließen

Die Piper-Ausgabe wird weiterhin in eine `.part.wav` geschrieben und mit `sync_all()` gespült.
Neu wird das Dateihandle anschließend ausdrücklich geschlossen, bevor WAV-Prüfung und atomare
Umbenennung zur finalen `.wav` erfolgen. Erst die finalisierte Datei darf weitergereicht werden.

### 2. Exakt derselbe AudioPlayer-Einstieg wie bei Recordings

`play_generated_audio()` validiert die finale WAV und delegiert danach direkt an
`play_recording()`. Damit existiert nach der TTS-Erzeugung kein eigener TTS-Abspielzweig mehr.
Quelle, Dekodierung, vollständige ACELP-Vorbereitung, Rufaufbau und Playout entsprechen einer
manuell ausgewählten Recording-Datei.

### 3. Keine Energy-Economy-Doppelankündigung während des initialen Bursts

Für netzgestartete Gruppenrufe wird die separate EE-Ankündigung in den ersten 112 TETRA-
Timeslots (ca. 1,6 Sekunden) unterdrückt. In diesem Zeitraum laufen bereits initiales D-SETUP,
Backup und vier dichte Zusatzankündigungen. Dadurch werden nicht mehr mehrere
Kanalzuweisungen im selben TS1-Turn gestapelt und fragmentiert. Danach bleibt die
Energy-Economy-Wake-Window-Ankündigung unverändert aktiv.

## Erwartete Logs

Vor der Aussendung:

```text
TTS: finalized complete WAV before AudioPlayer handoff ...
AudioPlayer: finalized TTS WAV handed to recording playback path ...
AudioPlayer: prepared ...
CMCE: starting NEW network call ...
```

Nicht mehr direkt beim Rufstart erwartet:

```text
EE: group ... announce re-sent ...
dl_enqueue_tma: ... deferring chan_alloc PDU to next frame ...
-> Fragged MacResource ... chan_alloc_element: Some(...)
```

Eine EE-Ankündigung darf später nach Abschluss des initialen 1,6-Sekunden-Bursts erscheinen.

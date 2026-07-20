# Phase 4 – TTS Frame-18 Queue Hygiene Fix

## Beobachtung

WAV-/Recording-Dateien werden zuverlässig ausgesendet, während spätere TTS-Rufe teilweise
nicht am Funkgerät ankommen. Beide Quellen benutzen nach der Dateivorbereitung denselben
`AudioPlayer`-, CMCE-, UMAC- und TCH/S-Pfad. Die Ursache liegt daher nicht in Piper oder im
TTS-WAV-Format, sondern im Zustand der zusätzlich eingeführten Frame-18-Common-SCCH-Warteschlange.

## Logdiagnose

Der erste AudioPlayer-Ruf nach dem Neustart begann mit leerer Frame-18-Warteschlange:

```text
14:53:13 call_id=4
14:53:13 queue depth_before=0
14:53:13 transmitting ... remaining=0
```

Weitere Paging-Seiten dieses bereits laufenden Rufs blieben jedoch liegen:

```text
14:53:15 queue depth_before=0
14:53:17 queue depth_before=1
```

Der folgende TTS-Ruf `call_id=5` erbte diese zwei alten Seiten und erhöhte die Tiefe weiter:

```text
14:53:33 queue depth_before=2
14:53:35 queue depth_before=3
14:53:37 queue depth_before=4
```

Ein weiterer Ruf erhöhte die Tiefe bis auf acht Einträge. Bei der nächsten nutzbaren
Frame-18-Gelegenheit entfernte der Scheduler alle acht Einträge im selben Slot:

```text
14:54:14 transmitting ... remaining=7
14:54:14 transmitting ... remaining=6
...
14:54:14 transmitting ... remaining=0
```

Damit wurden D-SETUP-Seiten mehrerer bereits beendeter und aktueller Rufe mit verschiedenen
Call-IDs und Usage-Markern vermischt. Nur der erste Eintrag konnte sinnvoll in den SCH/F-Block
passen; die weiteren Einträge erzeugten Fragmentierungsreste für Folgeslots. Das erklärt die
scheinbare Quellenabhängigkeit: Der erste manuelle Recording-Ruf traf häufig auf eine leere
Queue, spätere TTS-Rufe auf den angesammelten Altbestand.

## Änderungen

### Call-ID im internen Delivery-Handle

Der BS-interne Request-Handle für Frame-18-Paging trägt nun zusätzlich die CMCE-Call-ID.
MLE und LLC reichen ihn unverändert bis UMAC weiter; er wird weiterhin nicht über die
Luftschnittstelle übertragen.

### Getaggte Warteschlangeneinträge

Jeder Frame-18-Eintrag enthält:

- Call-ID
- GSSI
- MAC-Ressource

Dadurch kann UMAC alte Seiten gezielt erkennen und entfernen.

### Deduplizierung und Ablösung

- Pro Call-ID und GSSI bleibt höchstens eine noch nicht gesendete Paging-Seite in der Queue.
- Eine neuere Call-ID zur gleichen GSSI entfernt noch wartende Seiten des älteren Rufs.
- `CallEnded` entfernt alle noch wartenden Seiten der beendeten Call-ID.

### Maximal eine Seite pro Frame-18-Gelegenheit

Eine Frame-18-/TS1-Gelegenheit beginnt jetzt höchstens eine gepinnte D-SETUP-Ressource.
Der Scheduler leert nicht mehr die gesamte Warteschlange in einen einzelnen SCH/F-Block.

Muss diese eine Ressource fragmentiert werden, wird ausschließlich ihr Fortsetzungsfragment
an den Anfang der normalen TS1-Warteschlange des unmittelbar folgenden Frames gesetzt. Es
werden keine weiteren D-SETUP-Ressourcen im selben Frame-18-Slot begonnen.

## Erwartete Logs

Beim laufenden Ruf dürfen wiederholte CMCE-Seiten höchstens zu einer Deduplizierung führen:

```text
BsChannelScheduler: queued dedicated frame-18 common SCCH resource call_id=5 gssi=15201 ...
BsChannelScheduler: deduplicating pending frame-18 common SCCH page call_id=5 gssi=15201 ...
```

Beim Rufende:

```text
BsChannelScheduler: retired 1 frame-18 common SCCH page(s) for ended call_id=5
```

Beginnt ein neuer Ruf, obwohl vom Vorgänger noch ein Eintrag vorhanden ist:

```text
BsChannelScheduler: purged 1 stale frame-18 common SCCH page(s) for gssi=15201 before call_id=6
```

Pro Frame-18-Gelegenheit darf die Meldung `transmitting dedicated ...` nur einmal erscheinen.

# Gruppen- und Einzelrufe

## Gruppenruf

Ein Gruppenruf wird an eine GSSI aufgebaut. Ein Teilnehmer erhält die Sprechberechtigung, weitere affiliierte Geräte hören den Traffic Channel. Nach Ende der Senderphase bleibt der Ruf optional in Hangtime offen.

Relevante Parameter:

```toml
[cell_info]
hangtime_secs = 5
call_timeout_secs = 120
ul_inactivity_secs = 3
```

`release_group_on_same_speaker_retake` ist ein optionaler Workaround für ältere Funkgeräte, die einen schnellen erneuten Floor Grant bestätigen, danach aber keine Sprache senden.

## Einzelruf

Ein Einzelruf adressiert eine ISSI. Audio-Zentrale und Asterisk können ebenfalls Einzelrufe aufbauen. Für abgespielte Dateien gilt ein eigener Antwort-Timeout.

## Priorität

Rufe können Prioritäten tragen. Priorität `15` wird in der Notfalllogik besonders behandelt. Priorität allein garantiert nicht, dass ein bereits belegter oder technisch nicht verfügbarer Timeslot sofort nutzbar wird.

## Release

Ein sauberer Rufabschluss ist entscheidend, besonders bei Dual Carrier. Im Fehlerfall prüfen:

- Floor-State
- Hangtime
- U-DISCONNECT/D-RELEASE
- Carrier und Timeslot
- AudioPlayer-Guard-Zeit
- Endgerät, das auf einem Slot „hängen“ bleibt

## Lasttest

Nicht nur den Rufaufbau testen, sondern auch:

- vierte und weitere parallele Gespräche
- Wechsel zwischen Carriern
- erneuter PTT während Hangtime
- abrupt verschwindender Sender
- Einzelruf ohne Antwort
- vollständige Freigabe aller Timeslots danach

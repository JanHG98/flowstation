# Phase 4 – TTS Common-SCCH / Frame-18 Paging Fix

## Beobachtung

Beim Test wurde ein netzgestarteter TTS-Gruppenruf auf GSSI 15201 zunächst nicht am
Funkgerät geöffnet. Die Basisstation sendete das initiale D-SETUP, das Backup und alle vier
Zusatzankündigungen. Erst während eines `RoamingLocationUpdating` wurde die aktive
Gruppenverbindung erneut angekündigt; unmittelbar danach empfing das Funkgerät die laufende
Durchsage.

Damit sind Piper, WAV/ACELP, TCH/S und der Re-Register-Reannounce-Pfad nachweislich
funktionsfähig. Der Fehler liegt vor dem Traffic Channel: Ein im Leerlauf befindliches
AIv2/common-SCCH-Terminal sieht die anfängliche Gruppenrufankündigung nicht zuverlässig.

## Gefundener Widerspruch

MM übermittelt bei `clch_needed=true` oder `common_scch=true` die Information, dass das
Funkgerät in Frame 18 auf TS1 einen gemeinsamen sekundären Steuerkanal überwachen soll.
Der bisherige UMAC-Scheduler verwarf jedoch ausnahmslos alle adressierten Ressourcen in
Frame 18 und erzeugte dort ausschließlich BSCH/BNCH.

Das erklärt das Testbild:

1. Das Funkgerät ist im Idle-/common-SCCH-Zustand und verpasst D-SETUP auf dem normalen MCCH.
2. Beim Re-Register arbeitet es aktiv auf dem Steuerkanal.
3. Der bestehende Affiliation-Refresh-Fix sendet D-SETUP erneut.
4. Das Funkgerät wechselt auf TCH/S und hört die laufende Durchsage.

## Änderungen

### UMAC

- Frame 18 bleibt grundsätzlich für Broadcast und Associated Control reserviert.
- Auf dem Primärträger darf TS1 jetzt als adressierter gemeinsamer SCCH verwendet werden,
  sofern TS1 in diesem Multiframe nicht durch die rotierende zwingende BSCH-/BNCH-Belegung
  belegt ist.
- Andere Frame-18-Slots sowie sekundäre Träger bleiben unverändert gesperrt.
- Eine Frame-18-SCCH-Nachricht wird als adressierter SCH/F-MAC-Block ausgesendet.

### CMCE

- Für junge netzgestartete Gruppenrufe wird unmittelbar vor einer nutzbaren
  Frame-18/TS1-Gelegenheit ein weiteres D-SETUP eingeplant.
- Einplanung erfolgt auf Frame 17/TS3, damit MLE, LLC und UMAC ausreichend Vorlauf haben.
- Maximal drei Frame-18-SCCH-Seiten innerhalb der ersten sechs Sekunden.
- Lokale, vom Funkgerät gestartete Gruppenrufe bleiben unberührt.

## Empfohlener Audio-Vorlauf

Für TTS/AudioPlayer sollte der Vorlauf auf 40 Blöcke gesetzt werden:

```toml
[audio_player]
lead_in_silence_blocks = 40
tail_silence_blocks = 5
group_release_guard_seconds = 7
```

40 × 60 ms ergeben 2,4 Sekunden. Damit liegt auch bei ungünstiger Multiframe-Position eine
nutzbare Frame-18/TS1-SCCH-Gelegenheit vor der ersten gesprochenen Silbe.

## Erwartete Logs

```text
CMCE: starting NEW network call ... priority=5 (D-SETUP only)
CMCE: network D-SETUP burst ... stage=1/4
...
CMCE: network D-SETUP queued for frame-18 common SCCH ... page=1/3 ...
```

Ein Re-Register darf weiterhin zu einer zusätzlichen sofortigen Neuankündigung führen, soll
für die erste Rufannahme aber nicht mehr erforderlich sein.

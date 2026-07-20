# Phase 4 – TTS Network Group Call Reliability Fix

## Anlass

TTS/Piper und AudioPlayer meldeten erfolgreiche Jobs, während einzelne Funkgeräte den
Gruppenruf auf GSSI 15201 sporadisch nicht öffneten. Der Log zeigte dabei:

- `NetworkCallStart ... priority=5`, aber `DSetup ... call_priority: 0`
- unmittelbar nach D-SETUP zusätzlich ein gruppenadressiertes D-CONNECT
- nach initialem D-SETUP/Backup erst wieder nach etwa fünf Sekunden ein Late-Entry-D-SETUP
- gelegentliche `RoamingLocationUpdating`-/Affiliation-Refreshes des Funkgeräts

GSSI 15501 ist projektspezifisch eine reine Hintergrundgruppe für SDS/Status und wird in
diesem Fix nicht als Sprachziel verwendet.

## Änderungen

### 1. Kein gruppenadressiertes D-CONNECT

Ein durch AudioPlayer/TTS oder Brew gestarteter Gruppenruf besitzt kein lokales anrufendes
Funkgerät. Die Gruppenmitglieder werden deshalb ausschließlich mit D-SETUP plus
Kanalzuweisung gerufen. D-CONNECT bleibt den Verfahren mit einer realen Calling-Party-Leg
vorbehalten.

### 2. Richtige Rufpriorität

Die über `NetworkCallStart` gelieferte Priorität wird auf 0 bis 15 begrenzt und direkt in
`DSetup.call_priority` übernommen. Dashboard-Priorität 5 erscheint damit auch als
`call_priority: 5` auf der Luftschnittstelle.

### 3. Dichte D-SETUP-Ankündigung während des Silence-Lead-ins

Zusätzlich zum unmittelbaren D-SETUP und dem normalen Backup werden vier weitere
D-SETUPs gesendet:

- ca. 0,4 Sekunden
- ca. 0,8 Sekunden
- ca. 1,2 Sekunden
- ca. 1,6 Sekunden

Damit liegen alle Zusatzankündigungen innerhalb des aktuell konfigurierten 1,8-Sekunden-
Vorlaufs. Die Sprache beginnt erst, nachdem das Funkgerät mehrere Gelegenheiten zum Wechsel
auf den TCH/S hatte. Das bestehende Late-Entry-Intervall von fünf Sekunden bleibt erhalten.

### 4. Sofortige Neuankündigung nach Re-Register/Affiliation-Refresh

Meldet MM bei einem aktiven netzgestarteten Gruppenruf die Gruppenzugehörigkeit erneut,
verschickt CMCE sofort ein frisches D-SETUP mit Kanalzuweisung. Das Funkgerät muss nach
einem `RoamingLocationUpdating` dadurch nicht bis zum nächsten Fünf-Sekunden-Late-Entry
warten.

## Erwartete Logs

Beim Rufstart:

```text
CMCE: starting NEW network call ... priority=5 (D-SETUP only)
DSetup { ... call_priority: 5 ... }
CMCE: network D-SETUP burst ... stage=1/4 ...
CMCE: network D-SETUP burst ... stage=2/4 ...
CMCE: network D-SETUP burst ... stage=3/4 ...
CMCE: network D-SETUP burst ... stage=4/4 ...
```

Nach einem Mobility-/Affiliation-Refresh während des Rufs:

```text
CMCE: re-announcing active network call after affiliation refresh ...
```

Nicht mehr erscheinen darf unmittelbar nach einem TTS-Gruppenrufstart ein an die GSSI
adressiertes `DConnect`.

## Betroffene Dateien

- `crates/tetra-entities/src/cmce/subentities/cc_bs/state/mod.rs`
- `crates/tetra-entities/src/cmce/subentities/cc_bs/procedures/isi.rs`
- `crates/tetra-entities/src/cmce/subentities/cc_bs/timers.rs`
- `crates/tetra-entities/src/cmce/subentities/cc_bs/pdu.rs`
- `crates/tetra-entities/tests/test_cmce_bs.rs`

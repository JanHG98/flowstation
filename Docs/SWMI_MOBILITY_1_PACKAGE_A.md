# SWMI Mobility 1 – Paket A: MLE-Zellwechselbasis

## Status

**Umgesetzt.** Dieses Paket baut auf `SWMI Foundation 1` auf und implementiert die konventionelle MLE-Zellwechselbasis auf der Infrastrukturseite.

Enthalten sind vollständige Codecs und lokale Transaktionszustände für:

- `U-PREPARE`;
- `D-NEW-CELL`;
- `D-PREPARE-FAIL`;
- `U-RESTORE`;
- `D-RESTORE-ACK`;
- `D-RESTORE-FAIL`;
- `U-CHANNEL-REQUEST`;
- `D-CHANNEL-RESPONSE`.

`U-PREPARE-DA`, Irregular Channel Advice und die vollständige Direct-Access-Ausprägung bleiben bewusst spätere Profile.

## Architektur

Die lokale MLE-Runtime bleibt in der TBS:

```text
LLC
 └─ MLE protocol discriminator
     └─ MLE cell-change runtime
         ├─ lokale Endpoint-/Link-Zuordnung
         ├─ Transaktion und Timeout
         ├─ MM-/CMCE-Indikation
         └─ Downlinkantwort an LLC
```

Ein späterer `mobility-core` darf die Entscheidung liefern, aber nicht die lokalen Endpoint-, Link- und Air-Interface-Timer übernehmen.

## PDU-Codecs

Die vorherigen Platzhalter wurden durch bitgenaue Parser und Encoder ersetzt. Eingebettete MM-, OTAR- und CMCE-PDUs werden als vollständige `BitBuffer` übernommen. Dadurch sind sie nicht mehr auf 64 Bit begrenzt.

Die implementierten Downlink-PDUs tragen:

| PDU | Inhalt |
|---|---|
| `D-NEW-CELL` | Channel Command Valid und optional eingebettete MM-/OTAR-PDU |
| `D-PREPARE-FAIL` | Fail Cause und optional eingebettete MM-PDU |
| `D-RESTORE-ACK` | eingebettete CMCE `D-CALL RESTORE`-PDU |
| `D-RESTORE-FAIL` | Fail Cause |
| `D-CHANNEL-RESPONSE` | Annahme/Ablehnung, Request Reason und Retry Delay |

Die implementierten Uplink-PDUs tragen:

| PDU | Inhalt |
|---|---|
| `U-PREPARE` | optionale Cell Identifier CA und eingebettete MM-/OTAR-PDU |
| `U-RESTORE` | optionale alte MCC/MNC/LA und eingebettete CMCE-PDU |
| `U-CHANNEL-REQUEST` | Grund, gewünschte Channel Classes und Channel IDs |

## Lokale Zustandsmaschine

Neu:

```text
crates/tetra-entities/src/mle/cell_change_runtime.rs
```

Eine Transaktion durchläuft abhängig vom Verfahren:

```text
PrepareReceived
 ├─ PrepareDeferred
 ├─ NewCellGranted
 ├─ Rejected
 └─ TimedOut

RestoreReceived
 ├─ Restored
 ├─ Rejected
 └─ TimedOut

ChannelRequestReceived
 ├─ ChannelResponseSent
 └─ TimedOut
```

Die Registry wird mit `TetraAddress` adressiert und behält:

- Teilnehmer;
- Endpoint-ID;
- Link-ID;
- Zeitpunkt und Alter;
- Cell Identifier CA;
- Zielzelle;
- alte MCC, MNC und Location Area;
- Channel Request Reason;
- gewünschte Channel Classes und Channel IDs;
- Länge der eingebetteten SDU.

## Steuerpfad

Neu:

```text
MleCellChangeControl
```

Unterstützte lokale Befehle:

- Prepare akzeptieren und `D-NEW-CELL` senden;
- Prepare ablehnen und `D-PREPARE-FAIL` senden;
- Restore bestätigen und `D-RESTORE-ACK` senden;
- Restore ablehnen und `D-RESTORE-FAIL` senden;
- Channel Request beantworten.

Der Typ ist eine interne Kontrollnachricht. Er ist ausdrücklich **nicht** das spätere Edge/Core-Netzwerkprotokoll.

## MM- und CMCE-Weitergabe

Eine in `U-PREPARE` enthaltene MM-PDU wird als `LMM-MLE-UNITDATA indication` an MM geleitet.

Eine in `U-RESTORE` enthaltene CMCE-PDU wird über die neue Primitive:

```text
LcmcMleRestoreInd
```

an CMCE geleitet. Sie enthält zusätzlich Teilnehmer, Endpoint, Link und die optional gemeldete alte Netzidentität.

Die zunächst in Paket A enthaltene konservative Ablehnung wurde mit `SWMI Mobility 1 – Paket B` ersetzt. CMCE verarbeitet die Restore-Indikation nun über die Gruppen- beziehungsweise Individualruf-State-Machine und liefert abhängig von Context, Ressourcen und Dienstprofil ein positives `D-RESTORE-ACK` oder einen definierten Restore-Fehler.

## Timeouts

Nicht beantwortete lokale Transaktionen laufen nach 432 Timeslots ab. Die TBS erzeugt dann deterministisch:

- `D-PREPARE-FAIL` mit temporär nicht möglicher Neighbour-Cell-Anfrage;
- `D-RESTORE-FAIL` mit nicht möglicher Wiederherstellung;
- abgelehnte `D-CHANNEL-RESPONSE` ohne erneute Übertragungserlaubnis.

So bleiben keine unbegrenzt wartenden Zellwechselzustände zurück.

## Diagnose und WebUI

`MleBs::cell_change_snapshot()` stellt read-only bereit:

- aktive und zuletzt abgeschlossene Transaktionen;
- aktuelle Phase;
- Endpoint und Link;
- Ziel- und alte Zelle;
- Alter in Timeslots;
- Prepare-, Restore- und Channel-Request-Zähler;
- Parsefehler;
- ungültige Steuerbefehle;
- Timeouts.

Die Werte werden später über TBS-WebUI und Node Gateway sichtbar. Es entsteht kein eigener MLE-Container.

Die verbindliche Regel bleibt unverändert: Jeder später eigenständig laufende Dienst unter `system-backend/` benötigt seine eigene WebUI.

## Zwei-Zellen-Abnahme

Der bestehende Harness wurde erweitert um:

- `submit_u_prepare`;
- `submit_u_restore`;
- `submit_u_channel_request`;
- `control_cell_change`;
- `cell_change_snapshot`.

Der Test `test_two_cell_mobility.rs` prüft:

1. `U-PREPARE` trifft in Zelle A ein;
2. MM erhält die eingebettete PDU;
3. Zelle A sendet `D-NEW-CELL`;
4. `U-RESTORE` trifft ausschließlich in Zelle B ein;
5. CMCE erhält die Restore-Indikation;
6. der Test-Control-Pfad lässt Zelle B `D-RESTORE-ACK` senden;
7. beide lokalen Transaktionszustände bleiben voneinander getrennt.

## Tests

Neu:

```text
crates/tetra-pdus/tests/test_mle_cell_change_pdus.rs
crates/tetra-entities/tests/test_mle_cell_change_runtime.rs
crates/tetra-entities/tests/test_two_cell_mobility.rs
tools/check_mle_cell_change.py
.github/workflows/swmi-mobility-cell-change.yml
```

Geprüft werden normative Feldreihenfolge, Roundtrips, lange eingebettete SDUs, positive und negative Zustandsübergänge, Timeouts und der Zwei-Zellen-Pfad.

## Bewusste Grenzen

Noch nicht Bestandteil dieses Pakets:

- reales SDR-Retuning beziehungsweise physischer Zellwechsel eines MS;
- `U-PREPARE-DA` und Direct Access;
- vollständige Forward Registration in MM;
- zentraler Mobility Core;
- Edge/Core-Context-Transfer;
- CMCE Call Restore State Machine;
- netzweite Call Legs;
- zwei reale TBS über RF.

## Folgeschritte

`SWMI Mobility 1 – Paket B` ergänzt die vollständige lokale CMCE-Call-Restore-State-Machine für Gruppen- und Individualrufe.

Danach folgt `SWMI Mobility 1 – Paket C`:

1. explizite MLE-Cell-State-Machine für Serving, Preparing, Changing und Restoring;
2. Verbindung zu TLMC Select/Configure;
3. MM Forward Registration und `D-LOCATION-UPDATE-PROCEEDING`;
4. unangekündigte und angekündigte Cell Reselection im Zwei-Zellen-Harness;
5. kontrollierter Context Transfer zwischen den beiden TBS-Runtimes.

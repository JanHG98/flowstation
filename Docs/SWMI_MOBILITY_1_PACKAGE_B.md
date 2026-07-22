# SWMI Mobility 1 – Paket B: CMCE Call Restore

## Status

**Umgesetzt für das derzeit unterstützte NetCore-Sprachprofil:** unverschlüsselte TCH/S-Gruppen- und Einzelrufe.

Dieses Paket ersetzt den bisherigen konservativen `D-RESTORE-FAIL`-Fallback durch eine vollständige lokale CMCE-/Call-Restore-State-Machine auf der Ziel-TBS.

Unterstützt werden:

- laufende Gruppenrufe;
- laufende Individualrufe;
- Simplex- und Duplexzustände des vorhandenen Call-Modells;
- Beibehaltung von Priorität, Floor-Zustand und Call Origin;
- neuer lokaler Call Identifier bei Kollision;
- Restore-Warteschlange bei belegten Traffic Channels;
- späteres Fortsetzen über `D-TX GRANTED`;
- Replay, Timeout, Reject und Cleanup;
- Channel Allocation durch `D-RESTORE-ACK` bis LLC/MAC.

## Architektur

Die Restore-Transaktion bleibt lokal in der TBS:

```text
U-RESTORE
  └─ MLE
      └─ LCMC-MLE-RESTORE indication
          └─ CMCE Call Restore
              ├─ Context prüfen
              ├─ Call-Leg rekonstruieren oder wieder anbinden
              ├─ Traffic Channel zuteilen oder Restore einreihen
              ├─ Floor/Teilnehmerzustand übernehmen
              └─ D-CALL RESTORE
                  └─ D-RESTORE-ACK + optionale Channel Allocation
```

Ein zukünftiger `mobility-core` beziehungsweise `call-control` transportiert den fachlichen Restore Context zwischen den TBS. Endpoint, Link, lokaler Call Identifier, lokale Timeslots und die Air-Interface-Transaktion bleiben Eigentum der Ziel-TBS.

## Restore Context

Neu beziehungsweise erweitert:

```text
crates/tetra-entities/src/cmce/call_restore_runtime.rs
```

### Gruppenruf

Gespeichert werden:

- alter Call Identifier;
- GSSI;
- ursprünglicher Sprecher;
- aktueller Floor Holder;
- Priorität;
- Call Timeout;
- ursprünglicher T310-Zeitbezug;
- aktiver oder ruhender Sprecherzustand;
- lokaler oder netzseitiger Ursprung;
- Communication Type;
- Circuit Mode;
- Speech Service;
- E2EE-Kennzeichnung.

### Individualruf

Gespeichert werden:

- alter Call Identifier;
- Calling und Called Address;
- Simplex-/Duplexzustand;
- Priorität;
- Call Timeout;
- ursprünglicher T310-Zeitbezug;
- Floor Holder;
- Brew-/Netzwerkzuordnung;
- Network Call Metadata;
- Communication Type;
- Circuit Mode;
- Speech Service;
- E2EE-Kennzeichnung.

## Zustandsmaschine

Eine Restore-Transaktion durchläuft:

```text
Requested
  └─ ContextMatched
      ├─ ResourceAllocated
      │   └─ Restored
      ├─ Queued
      │   ├─ ResourceAllocated
      │   │   └─ Restored
      │   ├─ Rejected
      │   └─ TimedOut
      ├─ Rejected
      └─ TimedOut
```

Parallel wechseln bestehende beziehungsweise neu erzeugte Call Legs formal:

```text
Active → Restore → Active
```

Fehler, Release und Timeout führen aus `Restore` in den Release-/Cleanup-Pfad.

## Gruppenruf-Wiederherstellung

Die Ziel-TBS:

1. validiert GSSI, Teilnehmer und Dienstprofil;
2. übernimmt Priorität, Call Origin und ursprünglichen T310-Zeitbezug;
3. verwendet ein bereits wiederhergestelltes Gruppenruf-Leg oder erzeugt ein neues;
4. öffnet den lokalen Circuit bei UMAC;
5. übernimmt aktuellen Sprecher und Floor-Zustand;
6. hält bei einem Listener-Restore den Receive-U-Plane aktiv, wenn ein anderer Teilnehmer spricht;
7. reiht eine erneute Sendeanforderung ein, wenn ein anderer Teilnehmer spricht;
8. gibt einen vom wiederkehrenden Sprecher nicht mehr angeforderten Floor kontrolliert frei;
9. erzeugt die lokale Channel Allocation;
10. antwortet mit `D-CALL RESTORE` in `D-RESTORE-ACK`;
11. legt ein Late-Entry-/Release-fähiges D-SETUP-Cacheobjekt an.

Mehrere Gruppenrufteilnehmer werden getrennt wiederhergestellt, verwenden auf der Zielzelle aber dasselbe logische Call-Leg und denselben gegebenenfalls neu vergebenen Call Identifier.

## Individualruf-Wiederherstellung

Die Ziel-TBS:

1. prüft Calling/Called Party und optional die gemeldete Gegenstelle;
2. übernimmt Simplex/Duplex, Priorität, Floor Holder und Netzmetadaten;
3. bindet Endpoint und Link der wiederkehrenden Seite neu;
4. behält den ursprünglichen T310-Zeitbezug;
5. erstellt oder verwendet das lokale Individual-Call-Leg;
6. teilt einen lokalen Circuit zu;
7. beantwortet die Restore-Anfrage mit aktuellem Transmission Grant.

Bei Simplex erhält ein Teilnehmer nicht automatisch den Floor, wenn die andere Partei ihn weiterhin hält. Ohne eigene Sendeanforderung wird trotzdem `GrantedToOtherUser` gemeldet, damit der Receive-U-Plane aktiv bleibt. Eine eigene Anforderung wird zusätzlich lokal eingereiht. Gibt der bisherige Floor Holder beim Restore keine neue Sendeanforderung ab, wird der Floor freigegeben.

Bei Duplex wird die wiederhergestellte Verbindung unabhängig vom Request-to-transmit-Bit mit `Granted` bestätigt; für Duplex wird kein künstlicher Simplex-Floor erzeugt.

## T310

Der Call Length Timer läuft während der Wiederherstellung weiter.

NetCore speichert deshalb:

- bei Gruppenrufen `created_at`;
- bei Individualrufen `active_timer_started`.

`D-CALL RESTORE` enthält im normalen Restore-Pfad **keinen neuen Call-Timeout-Wert** und setzt das Reset-Bit nicht. Damit wird der laufende T310 nicht versehentlich neu gestartet.

Die allgemeine PDU-Builder-Funktion setzt das Reset-Bit nur noch dann, wenn ausdrücklich ein neuer Timeoutwert mitgegeben wird.

## Channel Allocation

Die lokale Traffic-Channel-Zuteilung wird durchgängig transportiert:

```text
CMCE
  └─ MleCallRestoreDecision::Acknowledge
      ├─ eingebettete D-CALL RESTORE-PDU
      └─ CmceChanAllocReq
          └─ MleCellChangeControl::AcknowledgeRestore
              └─ D-RESTORE-ACK
                  └─ TLA-TL-DATA request
                      └─ LLC/MAC
```

Wenn der U-Plane-Zustand eingeschaltet werden soll, enthält die Antwort eine Channel Allocation.

## Call-Identifier-Kollisionen

Ist der alte Call Identifier auf der Zielzelle bereits anderweitig belegt, reserviert die Ziel-TBS einen neuen lokalen Call Identifier.

Die Registry speichert:

```text
old_call_id → local_call_id
```

Alle weiteren Teilnehmer desselben Gruppenrufs erhalten denselben neuen Identifier. Replays erhalten ebenfalls denselben Identifier und dieselbe Bearer Allocation.

## Congestion und Restore Queue

Ist kein Traffic Channel verfügbar:

- wird die Restore-Transaktion nicht sofort abgelehnt;
- `D-CALL RESTORE` meldet `Callqueued`;
- bei bestehender Sendeanforderung wird `RequestQueued` gemeldet;
- es wird noch keine Channel Allocation mitgegeben;
- ein gegebenenfalls neuer Call Identifier wird reserviert;
- die Transaktion wird bei jedem CMCE-Tick erneut geprüft.

Während die Transaktion gequeued ist, kann das Endgerät:

- mit `U-TX DEMAND` eine Sendeanforderung setzen oder erneuern;
- mit `U-TX CEASED` die Sendeanforderung wieder zurücknehmen.

NetCore bestätigt eine gequeuete Sendeanforderung mit `D-TX GRANTED / RequestQueued` ohne Channel Allocation.

Sobald ein Bearer verfügbar ist:

1. wird das Call-Leg erzeugt;
2. die Restore-Transaktion wird abgeschlossen;
3. ein individuell adressiertes `D-TX GRANTED` wird gesendet;
4. die neue Channel Allocation begleitet dieses PDU.

Wenn ein neues Call ID ohne Channel Allocation direkt über den LCMC-Pfad gesendet wird, verwendet NetCore dafür einen acknowledged Layer-2-Service.

## Replay, Reject und Timeout

Die Runtime unterscheidet:

- laufendes Duplikat;
- gequeuetes Duplikat;
- Replay einer abgeschlossenen Transaktion.

Terminale Ergebnisse bleiben kurz gespeichert. Dadurch werden wiederholte Restore-Anfragen idempotent beantwortet und erzeugen keine doppelten Circuits oder Call Legs.

Nicht beantwortete beziehungsweise dauerhaft gequeuete Restore-Transaktionen laufen nach 432 Timeslots ab. Danach wird der Teilnehmer mit `D-RELEASE` und Call-Restore-Fehlerursache aus dem lokalen Restore-Pfad entfernt.

Definierte Ablehnungsgründe sind unter anderem:

- unbekannter Call;
- Teilnehmer passt nicht zum Context;
- Dienstprofil nicht unterstützt;
- ungültiger Zustand;
- fehlerhafte PDU;
- Duplicate Request;
- Timeout.

## Unterstütztes Dienstprofil

Die lokale Runtime akzeptiert derzeit ausschließlich:

```text
Circuit Mode: TCH/S
Speech Service: 0 / TETRA encoded speech
E2EE: aus
```

Andere Circuit-Data-, Speech- oder E2EE-Kombinationen werden kontrolliert abgewiesen. Das ist eine bewusste Profilgrenze und kein stiller Fallback.

## Media-Pfad

Die Restore-State-Machine stellt das lokale Funk-Call-Leg vollständig wieder her und setzt für nicht lokale Medien:

```text
CircuitDlMediaSource::SwMI
```

Damit ist die Schnittstelle für den späteren zentralen `media-switch` vorbereitet.

Noch nicht in diesem Paket enthalten ist der eigentliche netzweite Audio-Transport zwischen zwei physischen TBS. Bis der Media Switch implementiert ist, kann eine Ziel-TBS nur Audio aus den bereits vorhandenen lokalen beziehungsweise Brew-/SwMI-Medienpfaden ausgeben.

## WebUI und Diagnose

`CmceBs::call_restore_snapshot()` liefert read-only:

- installierte Contexts;
- Teilnehmer;
- alten und neuen Call Identifier;
- Gruppen- oder Individualruf;
- aktuelle Restore Phase;
- Endpoint und Link;
- Timeslot und Usage;
- Transmission Grant;
- Reject Reason;
- Alter der Transaktion;
- Request-, Duplicate-, Queue-, Restore-, Reject- und Timeout-Zähler;
- Call-ID-Änderungen;
- Floor Grants;
- gequeue U-TX-Anforderungen und deren Stornierungen;
- Request-to-transmit-Zustand je Restore-Transaktion.

Die Werte sind für die spätere TBS-WebUI, den Node Gateway und den Control Room vorbereitet. CMCE erhält keinen eigenen LXC.

Die allgemeine Regel bleibt bestehen: Jeder später eigenständig laufende Dienst unter `system-backend/` besitzt eine eigene WebUI.

## Tests

Neu:

```text
crates/tetra-entities/tests/test_call_restore_runtime.rs
crates/tetra-entities/tests/test_two_cell_call_restore.rs
tools/check_cmce_call_restore.py
.github/workflows/swmi-mobility-call-restore.yml
```

Geprüft werden:

- Gruppenruf-Restore mit Floor und Priorität;
- Individualruf-Restore mit fremdem Floor Holder;
- Replay und laufende Duplikate;
- Timeout;
- Queue und spätere Bearer Allocation auf Runtime-Ebene;
- Call-ID-Kollision und einheitliche Aliasverwendung;
- Congestion mit `Callqueued` ohne Channel Allocation;
- Erhalt der Channel Allocation bei Replay;
- Weitergabe der Allocation durch MLE;
- Erhalt des T310-Zeitbezugs.

## Nächster Schritt

Als nächstes folgt `SWMI Mobility 1 – Paket C`:

1. explizite Serving-/Preparing-/Changing-/Restoring-Zellzustände;
2. Kopplung von MLE-Zellwechsel und TLMC Select/Configure;
3. MM Forward Registration;
4. `D-LOCATION-UPDATE-PROCEEDING`;
5. automatischer Context Transfer zwischen zwei TBS-Runtimes;
6. unangekündigte und angekündigte Cell Reselection im Zwei-Zellen-Harness.

# SWMI Foundation 1 – Paket E: Abnahme und Robustheit

## Status

**Umgesetzt.** Paket E schließt `SWMI Foundation 1` ab und macht die in Paket C und D aufgebauten TLMC-/TLPD-Runtimes belastbar genug für die folgenden MLE-Zellwechsel- und Multi-Site-Arbeiten.

TLMC und TLPD bleiben lokale, funknahe Komponenten der TBS. Es entsteht weiterhin kein eigener Container. Die zusätzlichen Zustände und Zähler stehen jedoch als read-only Snapshot für die spätere TBS-WebUI und den Node Gateway bereit.

## Ziele

Paket E ergänzt:

- echte Transferergebnisse über `TxReporter` statt sofortiger Erfolgsannahme;
- Duplicate- und Replay-Schutz für `RequestHandle`;
- definiertes Cancel-Verhalten;
- gebundene Transfer-Timeouts;
- sauberes Aufräumen bei Break, Disable, Close, Release und Call Release;
- negative Lifecycle-Transitionen;
- zusätzliche Diagnosewerte;
- einen wiederverwendbaren Zwei-Zellen-Testharness.

## TxReporter-gestützter Transferabschluss

Paket D meldete Erfolg bereits nach dem Einreihen in die lokale LLC-Queue. Paket E hält den Transfer nun so lange als `pending`, bis der zugehörige `TxReporter` einen belastbaren Zustand liefert:

```text
Pending
 ├─ Transmitted → bei unbestätigtem Dienst erfolgreich
 ├─ Transmitted → Acknowledged → bei bestätigtem Dienst erfolgreich
 ├─ Discarded → fehlgeschlagen
 ├─ Transmitted → Lost → fehlgeschlagen
 └─ Timeout → fehlgeschlagen
```

Damit unterscheidet die Runtime nun zwischen:

- lokal angenommen;
- tatsächlich ausgesendet;
- bestätigt;
- verworfen;
- verloren;
- zeitlich festgefahren.

Der `TxReporter` wird von MLE an LLC und UMAC weitergereicht. Die unteren Schichten markieren die tatsächliche Übertragung beziehungsweise Bestätigung.

## Request-Handle-Schutz

Jeder ausgehende LTPD-Transfer besitzt einen `RequestHandle`.

Paket E verwaltet zwei Tabellen:

```text
pending    = aktuell laufende Transfers
completed  = kürzlich abgeschlossene Handles als Replay-Schutz
```

Ein Handle wird abgewiesen, wenn es:

- bereits aktiv ist;
- innerhalb der Schutzfrist bereits abgeschlossen wurde;
- verspätet erneut durch SNDCP eingereicht wird.

Abgeschlossene Handles bleiben für eine Hyperframe-Dauer geschützt. Danach werden sie automatisch freigegeben.

## Cancel

`LTPD-MLE-CANCEL request` besitzt jetzt eindeutige Ergebnisse:

- aktiver Transfer: wird entfernt und als fehlgeschlagen gemeldet;
- bereits abgeschlossener Transfer: Cancel kommt zu spät;
- unbekannter Handle: unbekannter Request;
- wiederholtes Cancel: erzeugt keinen verwaisten Transfer und keine doppelte Aussendung.

## Timeouts

Ein Transfer darf maximal 432 Timeslots ohne endgültigen `TxReporter`-Zustand verbleiben. Danach wird er:

- aus `pending` entfernt;
- als fehlgeschlagen gemeldet;
- im Link-Zähler erfasst;
- in den temporären Replay-Schutz übernommen.

Die Timeoutgrenze schützt insbesondere vor:

- hängen gebliebenen LLC-Transaktionen;
- verlorenen MAC-Ressourcen;
- fehlenden ACK-Zuständen;
- späteren asynchronen Adaptern, die keine Rückmeldung liefern.

## Ressourcen- und Lifecycle-Abbruch

Noch offene Transfers werden kontrolliert beendet bei:

- `TLMC`-Ressourcenverlust;
- Disable;
- Network Close;
- Link Release;
- Call Release für den betreffenden Endpoint.

Dadurch bleiben keine Pending-Handles zurück, die nach einer Zustandsänderung versehentlich doch noch ausgesendet werden könnten.

## Negative Zustandsübergänge

Zusätzliche Prüfungen verhindern unter anderem:

- zweiten Connect auf einen bereits aktiven Link;
- zweiten Disconnect eines bereits geschlossenen Links;
- Reconnect eines bereits verbundenen Links;
- Reconnect bei geschlossenem Netz, fehlenden Ressourcen oder Disable;
- Transfers während Busy, Broken, Closed, Disabled oder Releasing.

Abgelehnte Transitionen werden im Snapshot gezählt.

## Diagnose für TBS-WebUI und Node Gateway

`LtpdRuntimeSnapshot` enthält jetzt zusätzlich:

- alle Pending-Transfers;
- Handle, Endpoint-ID und Link-ID;
- Alter des Transfers in Timeslots;
- aktuellen `TxState`;
- kürzlich abgeschlossene Transfers;
- Replay-Guard-Größe;
- Duplicate-Handle-Rejections;
- Cancel-Anforderungen;
- erfolgreich abgebrochene Transfers;
- Transfer-Timeouts;
- abgelehnte Lifecycle-Transitionen.

Diese Daten sind read-only und verändern den fachlichen Funkpfad nicht.

## Zwei-Zellen-Testharness

Neu:

```text
crates/tetra-entities/tests/common/two_cell.rs
crates/tetra-entities/tests/test_two_cell_foundation.rs
```

Der Harness erzeugt zwei voneinander getrennte Testzellen:

| Eigenschaft | Zelle A | Zelle B |
|---|---:|---:|
| Main Carrier | 1521 | 1522 |
| Location Area | 10 | 11 |
| Colour Code | 1 | 2 |

Jede Zelle besitzt:

- eigenen `MessageRouter`;
- eigene MLE-/TLPD-Runtime;
- eigene Link- und Teilnehmerkontexte;
- eigene Ressourcenverfügbarkeit;
- eigene Sinks für SNDCP und LLC.

Bereits getestet werden:

- getrennte Zell- und Broadcastparameter;
- Context nur in der zuständigen Zelle;
- simulierter Context-Transfer von A nach B;
- alter Link in A wird geschlossen;
- neuer Link in B wird verbunden;
- Ressourcenverlust einer Zelle beeinflusst die andere nicht.

Der Harness simuliert noch keine echte Luftschnittstellenprozedur `D-NEW-CELL`. Er ist die Testbasis, auf der diese im nächsten Paket aufgebaut wird.

## Tests

Neu beziehungsweise erweitert:

```text
crates/tetra-entities/src/mle/ltpd_runtime.rs
crates/tetra-entities/tests/test_ltpd_runtime.rs
crates/tetra-entities/tests/test_two_cell_foundation.rs
tools/check_foundation_acceptance.py
.github/workflows/swmi-foundation-acceptance.yml
```

Abgedeckt sind unter anderem:

- TxReporter bis Transmitted/Acknowledged;
- Duplicate während Pending;
- Replay nach Completion;
- Cancel eines Pending-Transfers;
- wiederholtes beziehungsweise verspätetes Cancel;
- Timeout ohne LLC-/MAC-Fortschritt;
- unbekannter Context;
- negative Reconnect-Transition;
- Zwei-Zellen-Isolation und Context-Transfer.

## WebUI- und Containerregel

Die vollständige `system-backend/`-Struktur und die WebUI-Pflicht für jeden später laufenden LXC-/VM-Dienst bleiben unverändert bestehen.

TLMC und TLPD sind keine eigenen LXC-Dienste. Ihre Diagnose wird später angezeigt über:

```text
TBS-WebUI
  └─ Node Gateway
      └─ Control Room
```

## Bewusste Grenzen

Paket E implementiert noch nicht:

- echte `D-NEW-CELL`-Signalisierung;
- `D-PREPARE-FAIL`;
- `U-RESTORE` und `D-RESTORE-ACK/FAIL`;
- `D-CHANNEL-RESPONSE`;
- zentralen Mobility Core;
- Context Transfer über eine echte Edge/Core-Schnittstelle;
- zwei reale RF-Zellen.

## Nächster Schritt

Mit Paket E ist `SWMI Foundation 1` abgeschlossen.

Als Nächstes folgt die MLE-Zellwechselbasis:

1. vollständige MLE-PDU-Codecs;
2. explizite MLE-Cell-State-Machine;
3. `D-NEW-CELL` und `D-PREPARE-FAIL`;
4. `U/D-RESTORE`;
5. `D-CHANNEL-RESPONSE`;
6. Nutzung des Zwei-Zellen-Harness für angekündigte und unangekündigte Zellwechsel.

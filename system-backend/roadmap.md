# NetCore-Tetra SwMI-Roadmap

## 1. Strategische Leitlinie

NetCore-Tetra wird nicht sofort in 15 einzelne Dienste zerlegt. Zuerst wird der vorhandene Air-Interface-Stack vollständig genug gemacht, um eine Basisstation später zuverlässig als **TBS Edge** an einen zentralen Core anzubinden.

Die Reihenfolge lautet deshalb:

```text
Protokollfundament
        ↓
vollständige lokale TBS
        ↓
definierte Edge/Core-Schnittstelle
        ↓
zentrale Teilnehmer- und Mobility-Dienste
        ↓
Multi-Site Call Control und Media
        ↓
SDS und Packet Data
        ↓
Sicherheit
        ↓
Transit und ISI
```

Grundsatz:

> Erst müssen alle notwendigen lokalen Protokollwege funktionieren. Danach werden Zuständigkeiten aus der TBS herausgelöst. Nicht umgekehrt.

## Verbindliche Management-Ebene

Jeder später eigenständig laufende LXC- oder VM-Dienst erhält eine eigene WebUI. Die WebUI wird mit dem jeweiligen Dienst ausgeliefert und bleibt unabhängig vom Control Room erreichbar. Gemeinsame Vorgaben stehen in `Docs/BACKEND_WEBUI_STANDARD.md`.

Die WebUI gehört ab der ersten Implementierung eines Dienstes zu dessen Definition of Done; sie wird nicht als spätere kosmetische Zusatzphase behandelt.

Die bisher umgesetzten LXC-Dienste starten im ausdrücklich markierten `open_lab`-Modus ohne Tokens, Benutzerkonten oder TLS. Diese Zwischenstufe dient nur dem isolierten Testnetz und wird vor einem Produktivbetrieb durch die spätere Security-Phase ersetzt beziehungsweise abgesichert.

---

## Aktueller LXC-Ausbaustand

- `node-gateway`: umgesetzt, offene TBS-/Backend-Vermittlung mit WebUI
- `mobility-core`: umgesetzt, Context Transfer und Teilnehmerlage mit WebUI
- `subscriber-core`: umgesetzt, persistente Teilnehmerprofile und TBS-Zugangsrichtlinie mit WebUI
- `group-core`: umgesetzt, GSSI, Mitgliedschaften, TBS-Gruppenpolicy und DGNA mit WebUI
- `call-control`: umgesetzt, logische Gruppen-/Einzelrufe, Floor Control und Call Restore mit WebUI
- `media-switch`: umgesetzt, netzweites Routing gepackter TETRA-Sprachframes mit WebUI
- `recorder`: umgesetzt, passiver Vollframe-Tap, Archiv, Integrität, Retention und WebUI
- `sds-router`: umgesetzt, zentrale SDS-/Statusvermittlung, Store-and-forward, Routing und WebUI
- `packet-core`: umgesetzt, PDP-/NSAPI-Kontexte, Fragmentierung, Mobility Anchor und WebUI
- `ip-gateway`: umgesetzt, TUN, Routing, Firewall/NAT, DNS/WAP-Test und WebUI
- `security-core`: umgesetzt, Security-Class-Policy, Challenge/Response, DCK-Kontexte, Sperren, Alarm/Audit und WebUI
- `kmf`: umgesetzt, CCK/GCK/SCK-Lifecycle, Key-Versionen, Crypto Periods, Rotation, versiegelte OTAR-Aktionen, Backup und WebUI
- `transit`: umgesetzt, NetCore-native Regionen-/Peervermittlung, Routing, Failover und WebUI
- `control-room`: umgesetzt, zentrale Bedien-/Lageebene mit Service-Federation, Incident-Journal, Schichtbuch und Browser-WebUI
- Observability/NMS als eigener LXC-Dienst umgesetzt; nächster Baustein: `application-gateway`
- nächster Media-Baustein: `media-library` / Audio Player auf Basis des stabilen Recorder-Formats

# 2. Normative Grundlage

Als feste Protokollbasis wird zunächst verwendet:

* ETSI TS 100 392-2 V3.10.1, veröffentlicht am 31. März 2023
* ETSI EN 300 392-2 V3.8.1 für die bisher im Repo verwendeten Klauselverweise
* relevante Teile der ETSI-ISI-Reihe
* ETSI-Sicherheits- und Supplementary-Service-Dokumente je Ausbaustufe

TS 100 392-2 V3.10.1 ist derzeit die jüngste veröffentlichte Air-Interface-Fassung. Parallel läuft bei ETSI ein neuer Überarbeitungsvorgang, der 2026 bereits WG Approval erreicht hat; dessen Veröffentlichung muss beobachtet und später als Delta eingearbeitet werden.

Zu Beginn wird im Repo eine Datei angelegt:

```text
Docs/ETSI_CONFORMANCE_MATRIX.md
```

Sie enthält für jede PDU und jede Primitive:

| Feld          | Bedeutung                                      |
| ------------- | ---------------------------------------------- |
| Schicht       | MAC, LLC, MLE, MM, CMCE, SNDCP                 |
| Richtung      | Uplink oder Downlink                           |
| PDU/Primitive | exakte Bezeichnung                             |
| Parser        | vorhanden, teilweise, fehlt                    |
| Encoder       | vorhanden, teilweise, fehlt                    |
| State Machine | vorhanden, teilweise, fehlt                    |
| MS-Seite      | vorhanden oder nicht relevant                  |
| BS-Seite      | vorhanden                                      |
| Unit Test     | vorhanden                                      |
| Golden Vector | vorhanden                                      |
| On-Air-Test   | Motorola, Sepura, Hytera                       |
| Relevanz      | Single-Site, Multi-Site, Packet Data, Security |
| Roadmap-Phase | Zuordnung                                      |

Ohne diese Matrix wird „komplett“ sonst schnell zu einem beweglichen Ziel.

---

# 3. Phase 0 – Stack stabilisieren und messbar machen

## Ziel

Bevor neue PDUs implementiert werden, bekommt jede Schicht klare Tests und Zustandsgrenzen.

## Aufgaben

### 3.1 Einheitliche PDU-Tests

Für jede PDU:

1. bekannte Bits dekodieren,
2. Struktur prüfen,
3. wieder kodieren,
4. Bitfolge muss exakt identisch sein,
5. abgeschnittene und ungültige PDUs dürfen niemals panicen.

Testkategorien:

```text
decode_golden_vector
encode_golden_vector
roundtrip
minimum_length
maximum_length
optional_fields
reserved_values
truncated_pdu
invalid_enum
unknown_extension
```

### 3.2 Primitive-Tests

Für alle SAP-Primitiven:

* korrekte Quelle,
* korrektes Ziel,
* vollständige Parameter,
* Handle-Erhalt,
* Link-ID-Erhalt,
* Endpoint-ID-Erhalt,
* Channel-Allocation-Erhalt,
* Fehlerpfad.

### 3.3 Zustandsmaschinen

Alle bisher implizit in Handlern verteilten Zustände werden explizit beschrieben:

* MLE State Machine,
* MM Registration State,
* CMCE Group Call,
* CMCE Individual Call,
* SNDCP Context State,
* LLC Link State,
* Channel Change State.

### 3.4 Noch keine neuen Runtime-Dienste

In dieser Phase werden noch keine LXC-Dienste gestartet. Die verbindliche WebUI-Architektur, Service-Matrix und gemeinsamen Management-Endpunkte werden jedoch bereits festgelegt, damit jeder spätere Dienst von Beginn an verwaltbar entwickelt wird.

## Abnahmekriterium

```text
cargo test --workspace
cargo clippy --workspace
```

laufen reproduzierbar durch, und jede bereits unterstützte PDU besitzt mindestens einen Golden-Vector-Test.

---

# 4. Phase 1 – TLMC vollständig implementieren

## Warum zuerst?

TLMC beziehungsweise der lokale TMC-SAP transportiert **lokale Layer-Management-Informationen** zwischen MLE, LLC und MAC. Er überträgt keine Nutzdaten über die Luftschnittstelle, ist aber für Konfiguration, Zellwahl, Channel Change, Ressourcenverlust, Scanning und spätere Mobilität entscheidend.

Im aktuellen Repo existieren bereits die TLMC-Strukturen, viele davon sind aber leer oder enthalten zahlreiche `Todo`-Felder. Dazu gehören unter anderem:

* Assessment

* Cell Read

* Configure

* Measurement

* Monitor

* Scan

* Select

* Report

## Umzusetzende Primitive

### 4.1 TLMC-CONFIGURE

Vollständig typisieren:

* Zellparameter,
* gültige MCC/MNC-Adressen,
* Endpoint-ID,
* Energy Economy,
* SCCH-Konfiguration,
* Frame-18-Verteilung,
* Datenprioritäten,
* Channel-Change-Handle,
* Channel-Change-Accept,
* Operating Mode,
* Call Release,
* Graceful Service Degradation.

Die generischen `Todo`-Typen werden durch konkrete Enums, Strukturen und Wertebereiche ersetzt.

### 4.2 TLMC-SELECT

Vollständiger Ablauf:

```text
MLE → LLC/MAC: SELECT request
LLC/MAC → MLE: SELECT indication/confirm
```

Dabei:

* Zielzelle,
* Träger,
* Frequenz,
* Colour Code,
* Timeslot-Konfiguration,
* Selection Cause,
* Erfolg oder Fehler,
* Channel-Change-Handle.

### 4.3 TLMC-SCAN

* Scan Request
* Scan Confirm
* Scan Report
* gefundene Zellen
* RSSI beziehungsweise RXLEV
* MCC/MNC
* Colour Code
* Träger
* Cell Service Level
* Abbruch und Timeout

### 4.4 TLMC-MONITOR und ASSESSMENT

* Nachbarzellen überwachen,
* Messlisten verwalten,
* Messberichte erzeugen,
* Zellkandidaten bewerten,
* MLE mit verwertbaren Ergebnissen versorgen.

### 4.5 TLMC-CELL-READ

* Systeminformationen einer Kandidatenzelle lesen,
* gültige beziehungsweise ungültige Zelle melden,
* Network Identity und Cell Identity liefern.

### 4.6 Ressourcenverlust

`TLMC-CONFIGURE indication` muss korrekt signalisieren:

* Ressource verloren,
* Ressource wieder verfügbar,
* Endpoint betroffen,
* Ursache,
* erforderliche Reaktion.

## Abnahmetests

### Simulation

* zwei virtuelle Zellen,
* eine aktuelle und eine Nachbarzelle,
* Scan,
* Auswahl,
* erfolgreicher Wechsel,
* abgewiesener Wechsel,
* Ressourcenausfall während eines Calls.

### On-Air

* Nachbarzellenaussendung wird von mindestens einem Endgerät erkannt,
* Endgerät bewertet beide Zellen,
* Wechselentscheidung ist in Logs vollständig nachvollziehbar.

## Abschlusszustand

TLMC ist danach kein Satz leerer Platzhalter mehr, sondern die vollständige lokale Steuerung zwischen MLE und Layer 2.

---

# 5. Phase 2 – TLPD vollständig implementieren

## Bedeutung

LTPD-SAP ist die definierte Schnittstelle zwischen **MLE und SNDCP**. ETSI sieht dort nicht nur `MLE-UNITDATA` vor, sondern einen umfangreichen Dienstzustand mit Open, Close, Break, Resume, Connect, Disconnect, Reconnect, Release, Activity, Cancel, Busy und Configure. Außerdem können auf Infrastrukturseite mehrere SNDCP-Instanzen beziehungsweise TSI-Familien bedient werden.

Im aktuellen Repo funktioniert der Uplink von MLE zu SNDCP grundsätzlich. Der Gegenweg von SNDCP über MLE nach LLC ist jedoch noch ausdrücklich unvollständig; `rx_tlpd_prim()` protokolliert aktuell nur „not implemented“.

## Aufgaben

### 5.1 Alle LTPD-Primitiven typisieren

Mindestens:

* `MLE-OPEN`
* `MLE-CLOSE`
* `MLE-UNITDATA`
* `MLE-REPORT`
* `MLE-BREAK`
* `MLE-RESUME`
* `MLE-BUSY`
* `MLE-CONFIGURE`
* `MLE-CONNECT`
* `MLE-DISCONNECT`
* `MLE-IDLE`
* `MLE-INFO`
* `MLE-RECONNECT`
* `MLE-RELEASE`
* `MLE-ACTIVITY`
* `MLE-CANCEL`

### 5.2 Downlinkpfad herstellen

```text
SNDCP
   │ LTPD Primitive
   ▼
MLE
   │ MLE discriminator + SDU
   ▼
LLC
   │
   ▼
UMAC
```

Dabei müssen erhalten bleiben:

* ISSI/ITSI,
* NSAPI-Kontext,
* Endpoint-ID,
* Link-ID,
* Handle,
* Priorität,
* Channel Allocation,
* acknowledged/unacknowledged mode,
* Packet-Data-Kennzeichnung,
* Fragmentierungsinformationen.

### 5.3 Mehrere SNDCP-Kontexte

Noch kein vollständiges Packet Data, aber die Schnittstelle muss bereits ermöglichen:

* mehrere Teilnehmer,
* mehrere NSAPI,
* mehrere TSI-Familien,
* mehrere gleichzeitige Verbindungen,
* unabhängige Zustände.

### 5.4 Break/Resume/Reconnect

Diese Pfade werden benötigt, damit Paketdaten einen Zellwechsel beziehungsweise zeitweiligen Ressourcenverlust überstehen können.

## Abnahmetests

* SNDCP kann eine beliebige Downlink-N-PDU über MLE und LLC ausgeben.
* Uplink und Downlink erhalten identische Teilnehmer- und Kontextzuordnung.
* Zwei Teilnehmer mit gleichem NSAPI werden nicht verwechselt.
* Ein Break/Resume verliert keine Context-ID.
* Ein Reconnect verwendet den bestehenden Context.
* Fehlerhafte Primitive bringen den Stack nicht zum Absturz.

---

# 6. Phase 3 – MLE-PDUs und Zellwechsel vollständig machen

Das ist der erste Bereich, in dem die Aussage „D-PDUs zuerst“ wirklich zutrifft.

Im aktuellen MLE werden mehrere zentrale Downlink-PDUs lediglich erkannt und anschließend als nicht implementiert protokolliert:

* D-NEW-CELL
* D-PREPARE-FAIL
* D-RESTORE-ACK
* D-RESTORE-FAIL
* D-CHANNEL-RESPONSE
* Teile von D-NWRK-BROADCAST und Erweiterungen

ETSI verwendet D-NEW-CELL, D-RESTORE-ACK und die zugehörigen TL-SELECT-/TL-CONFIGURE-Abläufe gemeinsam für angekündigte Zellwechsel, Forward Registration und Call Restoration.

## 6.1 MLE-PDU-Codecs

Vollständige Parser und Encoder:

* U-PREPARE
* U-PREPARE-DA
* D-NEW-CELL
* D-PREPARE-FAIL
* U-RESTORE
* D-RESTORE-ACK
* D-RESTORE-FAIL
* U-CHANNEL-REQUEST
* D-CHANNEL-RESPONSE
* U-MLE-UNITDATA
* D-MLE-UNITDATA
* D-NWRK-BROADCAST
* D-NWRK-BROADCAST-EXT
* D-NWRK-BROADCAST-DA, soweit im gewählten Profil relevant.

D-PREPARE-FAIL muss bei abgelehntem Zellwechsel auch ein eingebettetes MM-Reject transportieren können.

## 6.2 Zellwechselprofile

In dieser Reihenfolge:

### Profil A – Unannounced Reselection

* Teilnehmer verlässt alte Zelle,
* registriert sich neu,
* kein aktiver Call wird erhalten.

### Profil B – Announced Type 2

* alte Zelle bereitet Wechsel vor,
* neue Zelle wird bekanntgegeben,
* Registrierung auf neuer Zelle,
* Call Restore nach Wechsel.

### Profil C – Announced Type 1

* Forward Registration,
* Kontext ist vor Umschalten vorbereitet,
* möglichst geringe Unterbrechung.

### Profil D – Dual-Watch/DA

Erst nach stabilen Profilen A bis C.

## 6.3 MLE-Zustandsmaschine

Explizite Zustände:

```text
Serving
Scanning
CandidateSelected
Preparing
WaitingNewCell
ChangingChannel
Restoring
Resuming
Failed
```

## Abnahmetests

* zwei simulierte TBS,
* Endgerät startet in Zelle A,
* Zelle B wird Kandidat,
* erfolgreicher Wechsel,
* abgelehnte Zielzelle,
* Timeout bei fehlender Antwort,
* Wechsel während Gruppenruf,
* Wechsel während Individualruf,
* Wechsel während SNDCP Context,
* Rückkehr zur alten Zelle.

---

# 7. Phase 4 – MM vollständig für Multi-Site vorbereiten

## Aktueller Zustand

Registrierung, Gruppenattach, Recovery und lokale Teilnehmerzustände funktionieren bereits umfangreich. Migration wird aber bewusst abgewiesen, weil `D-LOCATION-UPDATE-PROCEEDING` und der notwendige Identitätsaustausch fehlen.

## Aufgaben

### 7.1 Vollständige Registration Procedures

* ITSI Attach
* Roaming Location Update
* Periodic Location Update
* Demand Location Update
* Service Restoration
* Migrating Location Update
* Forward Registration
* Deregistration
* ITSI Detach

### 7.2 Fehlende MM-PDUs

Insbesondere:

* D-LOCATION-UPDATE-PROCEEDING
* vollständige Varianten von D-LOCATION-UPDATE-ACCEPT
* vollständige Varianten von D-LOCATION-UPDATE-REJECT
* U/D-AUTHENTICATION-Hooks, zunächst ohne echte Kryptografie
* U/D-OTAR-Hooks als typisierte, zunächst deaktivierte Schnittstellen
* Enable/Disable-Verfahren
* Status- und Funktionsfehlerpfade
* vollständige Group Report Procedures.

### 7.3 Kontextmodell

Ein Teilnehmerkontext erhält:

```text
ITSI/ISSI
TEI
Serving TBS
Serving Cell
Location Area
Registration State
Last Update
Energy Saving Mode
Attached Groups
Active Calls
SDS Reachability
Packet Data Contexts
Security Context Reference
Fallback Version
```

### 7.4 Noch lokal, aber core-fähig

MM bleibt in dieser Phase weiterhin in der TBS-Binary. Seine Zustände werden jedoch so abstrahiert, dass sie später über ein Repository-/Service-Interface gespeichert werden können.

Nicht mehr direkt:

```rust
HashMap<ISSI, Client>
```

sondern logisch:

```rust
trait MobilityContextStore
trait SubscriberAuthorizer
trait GroupAffiliationStore
```

Lokale Implementierungen bleiben als Standalone-Backend bestehen.

## Abnahmetests

* vollständiger Attach/Detach-Zyklus,
* periodisches Update,
* Forward Registration,
* Migration akzeptiert und abgelehnt,
* Gruppen bleiben beim Zellwechsel erhalten,
* Neustart beider Zellen ohne Ghost-Teilnehmer,
* Standalone-Betrieb bleibt unverändert funktionsfähig.

---

# 8. Phase 5 – CMCE Call Restore und Multi-Cell-Fähigkeit

Jetzt werden MLE und MM mit CMCE verbunden.

## Aufgaben

### 8.1 Call Restore

* U-CALL-RESTORE
* D-CALL-RESTORE
* MLE-RESTORE request/confirm/indication/response
* Call-ID-Wechsel
* Timeslot-Neuzuteilung
* Wiederherstellung von Floor Owner
* Wiederherstellung von Call Priority
* Restore Reject
* Timer und Cleanup.

ETSI sieht vor, dass erfolgreiche Wiederherstellung über U-RESTORE und D-RESTORE-ACK den darin enthaltenen CMCE-Call-Restore-Inhalt zwischen MLE und CMCE transportiert.

### 8.2 Call Legs einführen

Ein logischer Call wird von seinen lokalen Funkbeinen getrennt:

```text
LogicalCall
├── CallLeg TBS-A
├── CallLeg TBS-B
├── DispatcherLeg
└── RecorderLeg
```

Zunächst befinden sich alle Legs noch im selben Prozess. Entscheidend ist das Datenmodell.

### 8.3 Globale und lokale IDs trennen

* globale NetCore Call UUID,
* TETRA Call Identifier pro Funkzelle,
* Media Session ID,
* Floor Session ID,
* External Gateway ID.

### 8.4 Call-State-Recovery

Aktive Calls erhalten serialisierbare Zustände:

* Call-Typ,
* GSSI oder ISSI,
* Priorität,
* aktueller Sprecher,
* beteiligte TBS,
* lokale Call IDs,
* lokale Timeslots,
* Timer,
* Restore-Status.

## Abnahmetests

* Gruppenruf wird beim Zellwechsel fortgesetzt,
* Individualruf wird wiederhergestellt,
* Restore einer Seite schlägt fehl,
* alte Ressourcen werden sicher freigegeben,
* keine doppelte Sprecherfreigabe,
* kein Zombie-Call,
* Priorität bleibt erhalten.

---

# 9. Gate 1 – Vollständige lokale TBS

Erst hier ist die Basis geschaffen, um Dienste auszulagern.

## Gate-1-Anforderungen

| Bereich    | Muss erfüllt sein                                         |
| ---------- | --------------------------------------------------------- |
| TLMC       | vollständig und getestet                                  |
| TLPD       | bidirektional und zustandsbehaftet                        |
| MLE        | Zellwechsel und Restore                                   |
| MM         | Migration und Forward Registration                        |
| CMCE       | Call Restore und Call Legs                                |
| SNDCP      | mindestens transportfähige Schnittstelle                  |
| Standalone | weiterhin funktionsfähig                                  |
| Tests      | Zwei-Zellen-Simulation                                    |
| Hardware   | mindestens zwei reale TBS oder eine TBS plus RF-Simulator |
| Endgeräte  | mindestens Motorola und Sepura, später Hytera             |
| Panics     | keine durch ungültige Funk-PDUs                           |

---

# 10. Phase 6 – Edge/Core-Protokoll definieren

Jetzt, nicht früher, wird die Netzwerkgrenze gebaut.

## Neues Crate

```text
crates/netcore-edge-protocol
```

## Protokollbereiche

### Node

* Hello
* Capabilities
* Heartbeat
* Time Sync
* Cell Status
* Carrier Status
* Protocol Version

### Mobility

* Registration Request
* Registration Decision
* Location Update
* Group Report
* Context Transfer
* Cell Change Prepare
* Cell Change Commit
* Cell Change Abort

### Call

* Call Setup Request
* Call Setup Decision
* Allocate Local Leg
* Local Leg Ready
* Floor Demand
* Floor Grant
* Floor Release
* Call Restore
* Call Release

### SDS

* Incoming SDS
* Route SDS
* Delivery Result
* Store-and-forward State

### Packet Data

* Context Request
* Context Decision
* N-PDU Uplink
* N-PDU Downlink
* Data Resource Request
* Context Release

### Media

* Media Session Open
* Media Session Close
* Stream Endpoint
* Codec
* Sequence
* Loss Information

## Transport

### Control Plane

* QUIC oder mTLS-WebSocket,
* zuverlässig,
* geordnet je Session,
* versioniert,
* reconnect-fähig.

### Media Plane

* separater QUIC-Datagram- oder UDP-Pfad,
* Sequenznummern,
* Jitterbuffer,
* keine Kopplung an normale Telemetrie.

### Event Plane

* später NATS,
* nicht für zeitkritische Request/Response-Prozeduren.

## Abnahmekriterium

Eine TBS kann wahlweise starten als:

```text
mode = "standalone"
```

oder:

```text
mode = "core-managed"
```

Bei Verlust des Core fällt sie kontrolliert in den definierten Fallback-Modus.

---

# 11. Phase 7 – Erste Core-Dienste

Jetzt werden zunächst nur vier zentrale Dienste aufgebaut.

## LXC 01 – Node Gateway

* TBS-Verbindungen,
* mTLS,
* Sessions,
* Heartbeats,
* Routing der Core-Protokolle,
* Versionsprüfung,
* Reconnect.

## LXC 02 – PostgreSQL

* getrennte Schemas,
* Migrationen,
* Backup,
* Audit.

## LXC 03 – Subscriber/Group Core

Zunächst gemeinsam:

* Teilnehmerprofile,
* Geräte,
* Gruppen,
* Berechtigungen,
* DGNA,
* Gruppenzuordnung,
* Prioritäten.

## LXC 04 – Mobility Core

* Serving TBS,
* Location Areas,
* Kontexttransfer,
* Migration,
* Zellwechselkoordination,
* Visitor State.

## Abnahmetest

Zwei TBS greifen auf dieselbe Teilnehmer- und Gruppendatenbasis zu. Ein Endgerät wechselt zwischen den Zellen, ohne dass zwei widersprüchliche Teilnehmerzustände entstehen.

---

# 12. Phase 8 – Zentrales Call Control

## LXC 05 – Call Control

Aus CMCE wird getrennt:

### TBS CMCE Edge

* Air-Interface-PDUs,
* lokale Timeslots,
* lokale Call IDs,
* lokale Signalisierung.

### Core Call Control

* logischer Call,
* Teilnehmer- und Gruppenrouting,
* TBS-Auswahl,
* Call Legs,
* Floor Arbitration,
* Priorität,
* Pre-emption,
* Emergency,
* Late Entry,
* Restore,
* Call Ownership.

## Übergangsstrategie

Zunächst Shadow Mode:

```text
TBS entscheidet weiterhin.
Core berechnet parallel.
Ergebnisse werden verglichen.
```

Danach:

```text
Core entscheidet.
TBS validiert und setzt lokal um.
```

Erst nach stabiler Laufzeit wird die lokale Entscheidung im Normalbetrieb deaktiviert.

---

# 13. Phase 9 – Zentraler Media-Switch

## LXC/VM 06 – Media Switch

* Sprachannahme von TBS,
* Verteilung an mehrere TBS,
* Leitstellen-Audio,
* Recorder-Taps,
* SIP/RTP,
* Asterisk,
* Audio-Player,
* TTS,
* EchoLink.

## Reihenfolge

1. passiver Audio-Tap,
2. eine TBS zum Recorder,
3. eine TBS zur Leitstelle,
4. zwei TBS in einem Gruppenruf,
5. externer Netzsprecher,
6. Asterisk,
7. Audio-Player und TTS.

## Abnahmekriterium

Ein Sprecher auf TBS-A ist gleichzeitig auf TBS-A, TBS-B, in der Leitstelle und im Recorder hörbar, ohne mehrfaches Echo oder unterschiedliche Call-Zustände.

---

# 14. Phase 10 – Zentraler SDS-Router

## LXC 07 – SDS Router

Erst wenn Mobility stabil ist, kann SDS zuverlässig zentral geroutet werden.

Funktionen:

* Individual-SDS,
* Gruppen-SDS,
* Status,
* Protokoll-ID-Routing,
* Store-and-forward,
* Offline Queue,
* TTL,
* Zustellberichte,
* Prioritäten,
* Duplikaterkennung,
* Ziel-TBS-Ermittlung,
* externe Anwendungen.

Der lokale SDS Edge behält:

* Air-PDU,
* MCCH/FACCH,
* Acknowledgement,
* EE-Wake-Window,
* kurze lokale Zustellqueue.

---

# 15. Phase 11 – Vollständiges SNDCP und Packet Data

Erst jetzt wird SNDCP vollständig aufgebaut.

Der aktuelle Code verarbeitet im Wesentlichen die PDP-Context-Aktivierung und sendet einen Accept; die übrigen erkannten SN-PDU-Typen besitzen noch keine vollständige Runtime-Verarbeitung.

## LXC 08 – Packet Core

* PDP Contexts,
* NSAPI,
* READY/STANDBY,
* Data Transmit Request/Response,
* Reconnect,
* Modify,
* End of Data,
* Deactivation,
* Fragmentierung,
* Reassembly,
* Mobility Anchoring,
* Packet Priority,
* Flow Control.

## LXC 09 – IP Gateway

* TUN/TAP,
* IP-Pool,
* Routing,
* NAT,
* Firewall,
* DNS,
* WAP,
* Testserver,
* Packet Capture.

## Reihenfolge innerhalb SNDCP

1. Context State Machine
2. Data Transmit
3. SN-UNITDATA
4. SN-DATA
5. PDCH-Zuteilung
6. Fragmentierung und Reassembly
7. End of Data
8. Reconnect
9. Modify
10. Mobility
11. TUN/TAP
12. Routing/NAT
13. WAP
14. allgemeiner IP-Verkehr.

---

# 16. Phase 12 – Sicherheit

Sicherheit wird nicht ganz am Anfang implementiert, aber ihre Hooks werden bereits in MM, MLE und UMAC vorgesehen.

## LXC beziehungsweise VM 10 – Security Core

* Authentication Centre,
* Security Policies,
* Challenge/Response,
* DCK-Verwaltung,
* Security Class,
* Disable/Enable,
* Security Audit.

## VM 11 – KMF

* CCK,
* GCK,
* SCK,
* OTAR,
* Key Versions,
* Rotation,
* sichere Backups,
* später HSM.

## Reihenfolge

1. Authentication ohne Air Interface Encryption
2. Class 2
3. DCK
4. Air Interface Encryption
5. Class 3
6. Group Keys
7. OTAR
8. Key Rotation
9. End-to-End-Key-Management, soweit vorgesehen.

---

# 17. Phase 13 – Supplementary Services

Erst jetzt werden Facility-basierte Dienste ergänzt.

Priorität:

1. Talking Party Identification
2. Call Authorized by Dispatcher
3. Area Selection
4. Include Call
5. Transfer of Control
6. Call Retention
7. Ambience Listening
8. Discreet Listening
9. Call Barring
10. telefonieartige Dienste wie Hold, Waiting und Forwarding.

Nicht jeder ETSI-Dienst muss im ersten NetCore-Profil aktiviert werden. Aber jeder empfangene Dienst muss wenigstens sauber:

* erkannt,
* validiert,
* unterstützt oder
* standardkonform abgewiesen

werden.

---

# 18. Phase 14 – Control Room und NMS trennen

## LXC 12 – Control Room

Der vorhandene Control Room bleibt Leitstellen- und Bedienebene.

Er bekommt Daten künftig von den autoritativen Core-Diensten und wird nicht selbst zur Teilnehmer- oder Mobility-Datenbank.

## LXC 13 – NMS/Observability

* Prometheus,
* Grafana,
* Loki,
* Alertmanager,
* RF-Metriken,
* Call-Metriken,
* Mobility-Metriken,
* Packet-Data-Metriken,
* Security-Alarme,
* Audit.

## LXC 14 – Application Gateway

* Telegram,
* DAPNET,
* MeshCom,
* Snom,
* Geoalarm,
* WX/METAR,
* TPG2200,
* Directory,
* Fremd-APIs.

---

# 19. Phase 15 – DXT-Region und DXTT-Transit

Erst wenn eine vollständige DXT-artige Region funktioniert, wird eine zweite Region aufgebaut.

## Region A

```text
TBS-A1
TBS-A2
Mobility-A
Call-Control-A
Media-A
```

## Region B

```text
TBS-B1
TBS-B2
Mobility-B
Call-Control-B
Media-B
```

## LXC 15 – NetCore Transit

> Implementierungsstand: Das NetCore-native Transit-Grundpaket unter `system-backend/transit/` enthält Regionen/Peers, Teilnehmer- und Gruppenauflösung, Routen, Path-Vector/Loop-Prevention, Sessions, Queues, Retry und Regional-Failover. Standardisiertes ETSI ISI bleibt der nachfolgende Interworking-Ausbau.

* Teilnehmerregion bestimmen,
* Gruppenruf zwischen Regionen,
* Individualruf zwischen Regionen,
* SDS-Transit,
* Media-Transit,
* redundante Pfade,
* Loop Prevention,
* Regional Failover.

Danach folgt standardisiertes ISI:

1. ISI General Design
2. ISI Mobility Management
3. ISI Individual Call
4. ISI Group Call
5. ISI SDS
6. ISI Supplementary Services
7. Interoperabilität mit fremder SwMI.

---

# 20. Zusammengefasste Reihenfolge

| Reihenfolge | Phase                        | Ergebnis                            |
| ----------: | ---------------------------- | ----------------------------------- |
|           0 | Test- und Konformitätsmatrix | „Komplett“ wird messbar             |
|           1 | TLMC                         | lokale Layer-Steuerung vollständig  |
|           2 | TLPD                         | MLE–SNDCP bidirektional vollständig |
|           3 | MLE-D-PDUs                   | Zellwechselprotokolle vollständig   |
|           4 | MM                           | Migration und Forward Registration  |
|           5 | CMCE Restore                 | Calls überstehen Zellwechsel        |
|      Gate 1 | lokale TBS vollständig       | bereit zur Core-Trennung            |
|           6 | Edge/Core-Protokoll          | stabile Netzwerkgrenze              |
|           7 | Subscriber, Groups, Mobility | erste DXT-Core-Funktionen           |
|           8 | Call Control                 | netzweite Rufsteuerung              |
|           9 | Media Switch                 | Multi-Site-Sprache                  |
|          10 | SDS Router                   | netzweite SDS                       |
|          11 | Packet Core                  | vollständiges SNDCP und IP          |
|          12 | Security                     | Authentication und AIE              |
|          13 | Supplementary Services       | ETSI-Zusatzdienste                  |
|          14 | NMS und Anwendungen          | Betriebsplattform                   |
|          15 | Transit und ISI              | DXTT- und Fremdnetzfähigkeit        |

---

# 21. Was ausdrücklich nicht zuerst gemacht wird

## Nicht alle D-PDUs blind implementieren

Diese Gruppen gehören bewusst später:

| D-PDU-Bereich                    | Grund                                         |
| -------------------------------- | --------------------------------------------- |
| D-OTAR und Key-PDUs              | benötigen Security Core und KMF               |
| D-FACILITY                       | benötigt Supplementary-Service-State-Machines |
| vollständige SNDCP-Downlink-PDUs | benötigen Packet Context und TLPD             |
| ISI-bezogene Signalisierung      | benötigt zwei funktionsfähige Core-Regionen   |
| Dispatcher-Sonderdienste         | benötigen zentralen Call Core und RBAC        |
| seltene optionale Profile        | erst nach Baseline-Konformität                |

## Keine Aufteilung von PHY bis LLC auf LXC

Diese Schichten bleiben gemeinsam auf der TBS:

```text
PHY
LMAC
UMAC
LLC
lokaler Scheduler
lokale TDMA-Zeit
```

## Keine sofortige Abschaffung des Standalone-Modus

Jede Auslagerung erfolgt mit:

```text
StandaloneBackend
CoreManagedBackend
```

Die heutige einzelne Basisstation muss während des gesamten Umbaus weiter nutzbar bleiben.

---

# 22. Erster konkreter Arbeitsblock

Der unmittelbar nächste Entwicklungsblock lautet:

## Milestone: `SWMI Foundation 1 – TLMC/TLPD`

### Paket A – Inventur ✅ abgeschlossen am 22.07.2026

Umgesetzt wurden:

* vollständige statische PDU-/Primitive-Matrix,
* Inventur aller `Todo`-Typen und TODO/FIXME-Hinweise,
* Inventur aller aktiven `unimplemented!`, `unimplemented_log!`, `panic!` und `unreachable!`-Pfade,
* statische Ermittlung nicht oder nur einseitig erreichbarer SAP-Pfade,
* Ermittlung fehlender PDU-/Primitive-Testverweise,
* Zustandsmaschinen-Inventur,
* maschinenlesbare JSON-/CSV-Exporte,
* reproduzierbarer Generator und CI-Konsistenzprüfung.

Ergebnisse:

* `Docs/SWMI_FOUNDATION_1_INVENTORY.md`
* `Docs/ETSI_CONFORMANCE_MATRIX.md`
* `Docs/SAP_PRIMITIVE_MATRIX.md`
* `Docs/IMPLEMENTATION_GAPS.md`
* `Docs/STATE_MACHINE_INVENTORY.md`
* `Docs/TLMC_TLPD_WORKLIST.md`
* `Docs/generated/`
* `tools/protocol_inventory.py`

Hinweis: Paket A ist eine statische und reproduzierbare Bestandsaufnahme. Es behauptet noch keine ETSI-Konformität und ersetzt weder Golden Vectors noch On-Air-Tests.

### Paket B – Typen ✅ abgeschlossen

* TLMC-Enums und Strukturen,
* TLPD-Primitiven,
* MLE-State-Enums,
* Channel-Change-Typen,
* Cell Candidate,
* Measurement Report,
* Restore Context.

### Paket C – TLMC Runtime ✅ abgeschlossen

* Configure,
* Scan,
* Monitor,
* Assessment,
* Cell Read,
* Select.

### Paket D – TLPD Runtime ✅ abgeschlossen

* vollständiger Uplink,
* vollständiger Downlink,
* Connect/Disconnect,
* Break/Resume,
* Reconnect,
* Context Routing.

### Paket E – Tests und Robustheit ✅ abgeschlossen

* Unit Tests,
* Golden Vectors,
* zwei virtuelle Zellen,
* malformed input,
* Timeout und Recovery.

### Definition of Done

Der Milestone ist erst abgeschlossen, wenn:

```text
kein TLMC-Typ mehr nur ein leerer Platzhalter ist
kein benötigtes TLMC-Feld mehr Todo verwendet
rx_tlmc_prim vollständig routet
rx_tlpd_prim vollständig routet
jede Primitive mindestens einen Test besitzt
zwei virtuelle Zellen gescannt und ausgewählt werden können
SNDCP bidirektional durch MLE und LLC transportiert werden kann
```

Erst danach beginnt Milestone 2 mit D-NEW-CELL, D-PREPARE-FAIL, D-RESTORE und D-CHANNEL-RESPONSE.

# Verbindliche WebUI-Abnahme ab der ersten LXC-Phase

Für jede Phase, in der ein neuer Containerdienst entsteht, gelten zusätzlich:

- eigene WebUI im jeweiligen `system-backend/<dienst>/`-Paket;
- Übersicht, Fachverwaltung, Health, Abhängigkeiten, Audit, Konfiguration und Wartung;
- versionierte Verwaltungs-API;
- RBAC für alle schreibenden Aktionen im späteren gesicherten Betrieb;
- bei einer ausdrücklich markierten `open_lab`-Stufe: deutliche Warnung und Netzisolation statt vorgetäuschter Authentisierung;
- unabhängige Erreichbarkeit bei ausgefallenem Control Room;
- UI- und API-Tests;
- dokumentierter HTTPS-Zugriff im Managementnetz beziehungsweise dokumentierter HTTP-Laborausnahme.


## Funkstack-Voraussetzung: SWMI Foundation 1 – Paket D

Die lokale TLPD-Runtime ist abgeschlossen. Sie bleibt auf der TBS und stellt Diagnose-Snapshots für die spätere TBS-WebUI und den Node Gateway bereit. Es entsteht kein eigener Backend-Container.

## Aktueller Stand: SWMI Mobility 1 und Core 1

Die lokale Funkstack- und Mobility-Grundlage ist abgeschlossen. Darauf aufbauend sind die zentralen Open-Lab-Dienste `node-gateway`, `mobility-core`, `subscriber-core`, `group-core`, `call-control`, `media-switch`, `recorder`, `sds-router`, `packet-core`, `ip-gateway`, `security-core` und `kmf` jeweils mit eigener WebUI umgesetzt.

Der Packet Core hält PDP-/NSAPI-Zustand, Reassembly und Downlink-Queue. Der IP Gateway koppelt dessen vollständige IPv4-N-PDUs über Linux-TUN an Routing, nftables, NAT, DNS sowie lokale WAP-/Testdienste und erzeugt direkt PCAP-Dateien. Security Core und KMF ergänzen Security-Class-Policy, Authentisierungs-/DCK-Orchestrierung sowie CCK/GCK/SCK-Lifecycle, Crypto Periods, Rotation, nodegebundene OTAR-Envelopes und sichere Vault-Backups. Transit und Control Room sind als zentrale LXC-Dienste umgesetzt. Observability/NMS ist als eigener LXC-Dienst mit Prometheus-, Grafana-, Loki- und Alertmanager-Konfiguration umgesetzt. Als nächster Baustein folgt `application-gateway`.

Bis zur späteren Security-Phase bleiben alle genannten LXC-Dienste ausdrücklich `open_lab`: keine Tokens, keine Benutzerkonten und kein TLS. Das ist nur für das isolierte Testnetz vorgesehen.



# SWMI Core 1 – Paket H: IP Gateway

## Ergebnis

Paket H ergänzt den eigenständigen LXC-Dienst `system-backend/ip-gateway/` mit WebUI auf Port 8170. Vollständige IPv4-N-PDUs aus dem Packet Core werden ohne künstliche Ethernet-Schicht über ein Linux-TUN-Interface in normale IP-Netze überführt.

## Funktionen

- TUN-Interface und bidirektionale Packet-Core-Kopplung
- IPv4-zu-ISSI/NSAPI-Zuordnung aus den aktiven PDP-Kontexten
- Routing, IPv4-Forwarding und Kernel-Reconcile
- nftables-Firewall, Flow-Block und Default-Policies
- Masquerading, SNAT und DNAT
- DNS-Forwarder mit statischen A-Records
- WAP/WML-, HTTP- und UDP-Testdienste
- Flow-Zähler und rohe IPv4-PCAPs (`DLT_RAW`)
- Shadow- und Authoritative-Modus
- persistente Regeln, API, OpenAPI, Metrics und eigene WebUI

## Architekturgrenze

Der Packet Core bleibt Eigentümer der SNDCP-State-Machine, Fragmentierung, Reassembly, Mobility Anchors und Downlink-Queue. Der IP Gateway kennt keine Air-PDUs und weist keine NSAPI zu. Er transportiert ausschließlich vollständige IP-N-PDUs.

## Sicherheitsstatus

Der Dienst läuft in der aktuellen Teststufe als `open_lab`: keine Anmeldung, keine Token und kein TLS. Im Authoritative-Modus besitzt er bewusst `CAP_NET_ADMIN`, `CAP_NET_RAW` und `CAP_NET_BIND_SERVICE`; deshalb ausschließlich im isolierten Labor betreiben.


---

## Package I – Security Core

Der Security Core ist als eigenständiger LXC-Dienst auf Port 8180 umgesetzt. Er verwaltet Sicherheitsprofile, Security-Class-Aushandlung, Challenge/Response-Kontexte, kurzlebige DCK-Installationsaufträge, Teilnehmer-/Gerätesperren, Alarme und Audit. Das Management bleibt in der aktuellen Testphase `open_lab`; Rohgeheimnisse sind aus normalen Managementpfaden ausgeschlossen.

Der mitgelieferte HMAC-Lab-Provider ist nur für Integrationstests vorgesehen. Langzeitschlüssel, normative Authentisierungsprovider, CCK/GCK/SCK und OTAR folgen getrennt im nächsten Baustein `kmf`.

---

## Package J – KMF

Die Key Management Facility ist als eigenständiger LXC-Dienst auf Port 8190 umgesetzt. Sie verwaltet CCK, GCK und SCK mit versionierten Crypto Periods, Rotation, Vorgänger-/Nachfolgerketten, Vier-Augen-Workflow, nodegebundener OTAR-Zustellung, Retry/Timeout, hashverkettetem Audit sowie verschlüsseltem Lab-Vault und verifizierten Backups.

Die normale WebUI und Management-API geben kein Rohschlüsselmaterial aus. Edge-Zustellungen enthalten ausschließlich einen an das Ziel-Node gebundenen versiegelten Envelope. Der aktuelle `lab_file_vault` und das Lab-Envelope sind klar als Integrationsmechanismen markiert; HSM/PKCS#11, TETRA-TA-Algorithmen und D-OTAR-Air-Interface-PDUs bleiben spätere Ausbaustufen.

Der Dienst bleibt in dieser Phase `open_lab`: keine Anmeldung, keine Tokens und kein TLS. Standard ist `shadow`; erst `authoritative` stellt vollständig freigegebene OTAR-Aktionen für die TBS Edge bereit.



---

## Package K – Transit

Der NetCore-native Transit-Dienst ist als eigenständiger LXC auf Port 8200 umgesetzt. Er verwaltet Regionen, Peers, Teilnehmer-/Gruppenregionen, Routen, Path Vector, Loop Prevention, Sessions, Queues, Retry und Failover. Standardisiertes ETSI ISI bleibt ein späterer Interworking-Ausbau.

---

## Package L – Control Room

Der bestehende Control Room ist als eigenständiger LXC-Dienst mit Browser-WebUI auf Port 9010 ausgebaut. Er pollt Live-/Ready- und Statusdaten aller bisherigen Core-Dienste, zeigt die TBS-/Ruf-/Notfalllage, erzeugt automatische Service-Incidents, führt ein manuelles Incident-Journal und ein persistentes Schichtbuch und verlinkt die eigenständigen Fach-WebUIs.

Der Control Room bleibt Presentation und Operator Plane. Er ist keine zweite Teilnehmer-, Gruppen-, Mobility-, Call-, SDS-, Packet- oder Schlüssel-Datenbank und enthält keinen beliebigen Schreibproxy zu Fachsystemen.

Die aktuelle Stufe bleibt `open_lab`: keine Anmeldung, keine Tokens, kein Node-Token und kein TLS. Observability/NMS ist umgesetzt. Nächster Baustein ist `application-gateway`.

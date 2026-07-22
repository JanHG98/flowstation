# SWMI Foundation 1 – TLMC/TLPD-Arbeitsliste

## Status nach Paket E

Paket B und Paket C sind umgesetzt:

- **18 normative TLMC-Primitive** besitzen konkrete Strukturen und `SapMsgInner`-Varianten;
- **27 LTPD-Primitive** besitzen konkrete Strukturen und `SapMsgInner`-Varianten;
- die neuen TLMC-/LTPD-Module verwenden keine aktiven `Todo`-Typen mehr;
- gemeinsame Mobilitäts-, RF-, QoS-, Channel-Change- und SNDCP-Typen liegen in `tetra_saps::common`;
- Scan-, Selection-, MLE-Cell-, Channel-Change- und LTPD-Link-Zustände sind explizit modelliert;
- Unit- und Integrationstests sowie ein dependency-freier statischer Checker sind vorhanden;
- `rx_tlmc_prim()` besitzt nun eine vollständige Package-C-Routinglogik in UMAC-MS, defensives BS-Verhalten und einen nicht panikenden MLE-Consumer;
- Configure, Ressourcenstatus, Measurement, Monitor, Assessment, Scan, Cell Read und Select besitzen eine explizite Runtime;
- Scan, Cell Read und Select besitzen korrelierte Zustände sowie negative Timeouts;
- `rx_tlpd_prim()` besitzt eine vollständige lokale Paket-D-Runtime;
- SNDCP sendet Paketdaten nicht mehr direkt an LLC, sondern über LTPD und MLE;
- Context Routing, Route Recovery, Transfer Reports und Link-Lifecycle sind aktiv;
- TLMC-Ressourcenkanten erzeugen LTPD Break/Resume;
- MLE und SNDCP stellen read-only TLPD-Diagnose-Snapshots bereit.
- Transferergebnisse werden über `TxReporter` bis zur tatsächlichen Übertragung beziehungsweise Bestätigung verfolgt.
- Duplicate- und Replay-Handles, Cancel, Timeouts und negative Lifecycle-Transitionen sind abgesichert.
- ein wiederverwendbarer Zwei-Zellen-Testharness prüft getrennte Zellzustände, Context-Transfer und Fehlerisolation.

Die genauen Einzelzeilen stehen in `Docs/SAP_PRIMITIVE_MATRIX.md`, `Docs/IMPLEMENTATION_GAPS.md` und `Docs/SWMI_FOUNDATION_1_PACKAGE_B.md` und `Docs/SWMI_FOUNDATION_1_PACKAGE_C.md`.

## Paket B – Typen: abgeschlossen

### B1 – Gemeinsame Basistypen ✅

Zuerst werden gemeinsam verwendete Typen definiert, damit TLMC und TLPD nicht zwei inkompatible Modelle erhalten:

- `ChannelChangeHandle`
- `ChannelChangeDecision`
- `LowerLayerResourceAvailability`
- `LowerLayerResourceReason`
- `CellIdentity`
- `CellCandidate`
- `CellServiceLevel`
- `MeasurementValue`
- `MeasurementReport`
- `ScanRequestId`
- `SelectionCause`
- `SelectionResult`
- `OperatingMode`
- `CallReleaseInstruction`
- `DataPriority`
- `Layer2Qos`
- `ReservationInfo`
- `TransferResult`
- `SetupReport`
- `SndcpStatus`
- `RestoreContext`

Ablageziel:

```text
crates/tetra-saps/src/common/
```

oder – falls die Typen auch außerhalb der SAP-Schicht benötigt werden – in einem klar benannten Modul unter `tetra-core`.

### B2 – TLMC-Typen ✅

Die leeren Strukturen erhalten konkrete Felder:

- Assessment und Assessment List
- Cell Read
- Configure Request/Indication/Confirm
- Measurement
- Monitor und Monitor List
- Report
- Scan Request/Confirm/Report
- Select Request/Indication/Response/Confirm

Besonderes Augenmerk:

- Trennung zwischen BS- und MS-relevanten Parametern;
- keine erfundenen Defaultwerte für normative Pflichtfelder;
- `Option<T>` nur bei tatsächlich optionalen Parametern;
- eindeutige Einheiten für RXLEV, Frequenz, Zeit und Priorität;
- keine nackten `u8`/`u32`, wenn der Wertebereich normativ eingeschränkt ist.

### B3 – TLPD-Typen ✅

Alle `Todo`-Felder werden ersetzt. Priorität haben:

1. `LtpdMleUnitdataReq`
2. Configure Request/Indication
3. Connect Request/Indication/Response/Confirm
4. Disconnect Request/Indication
5. Reconnect Request/Indication/Confirm
6. Break/Resume
7. Report/Cancel/Release
8. Open/Close/Idle/Info/Activity/Busy/Enable/Disable

### B4 – `SapMsgInner`-Verdrahtung ✅

Jede benötigte Primitive erhält eine eigene Variante. Dabei gilt:

- keine Verwendung eines generischen untypisierten Payloads;
- Variantenname entspricht dem Struct-Namen;
- Display/Debug darf bei neuen Varianten nicht panicen;
- Source, Destination und SAP werden in Tests geprüft;
- große Payloads werden nicht unnötig kopiert.

### B5 – Explizite Kontexte ✅

Noch vor der Runtime-Implementierung werden folgende Zustände modelliert:

```text
MleCellState
ChannelChangeState
TlmcScanState
TlmcSelectionState
LtpdLinkState
LtpdContextKey
```

`LtpdContextKey` muss mindestens Teilnehmer, Endpoint, Link und SNDCP-Kontext eindeutig unterscheiden können.

## Paket C – TLMC Runtime: abgeschlossen ✅

1. Configure ✅
2. Ressourcenverlust/-wiederkehr ✅
3. Measurement und Monitor ✅
4. Scan ✅
5. Cell Read ✅
6. Assessment ✅
7. Select ✅
8. Channel-Change-Antworten ✅
9. bounded Operationstimeouts ✅
10. read-only Diagnose-Snapshot für die spätere TBS-WebUI ✅

Die Runtime liegt in `crates/tetra-entities/src/umac/tlmc_runtime.rs`. Der experimentelle MS-Lower-Layer erhält für Scan/Select bereits den Zielträger über `TmvConfigureReq`; ein hardwareabhängiges dynamisches SDR-Retuning bleibt ein gesonderter Adapterpunkt.

## Paket D – TLPD Runtime: abgeschlossen ✅

1. vollständiges `MLE-UNITDATA` in beide Richtungen ✅
2. Configure ✅
3. Connect/Disconnect ✅
4. Break/Resume ✅
5. Reconnect ✅
6. Context Routing und Route Recovery ✅
7. Report/Cancel/Release ✅
8. Open/Close/Idle/Info und Activity/Busy/Enable/Disable ✅
9. read-only Diagnose-Snapshots für TBS-WebUI und Node Gateway ✅

Die Runtime liegt in `crates/tetra-entities/src/mle/ltpd_runtime.rs`. SNDCP-Antworten laufen jetzt über `Sap::TlpdSap` zu MLE und erst dort weiter zu LLC.

## Paket E – Abnahme und Robustheit: abgeschlossen ✅

1. Konstruktion und Routing der TLMC-/TLPD-Primitive ✅
2. Erhalt von Handle, Link-ID und Endpoint-ID ✅
3. TxReporter-gestützte Transferergebnisse ✅
4. unbekannter Context und ungültiger Zustand ✅
5. Duplicate Request und Replay Guard ✅
6. Cancel und verspätetes Cancel ✅
7. gebundene Transfer-Timeouts ✅
8. Break/Disable/Close/Release räumen Pending-Transfers auf ✅
9. negative Connect/Disconnect/Reconnect-Transitionen ✅
10. Zwei-Zellen-Testharness mit Context-Isolation und Ressourcenfehlern ✅
11. read-only Robustheitszähler für TBS-WebUI und Node Gateway ✅

Die Foundation-Tests liegen in:

```text
crates/tetra-entities/tests/test_tlmc_runtime.rs
crates/tetra-entities/tests/test_ltpd_runtime.rs
crates/tetra-entities/tests/test_two_cell_foundation.rs
```

Der Zwei-Zellen-Harness liegt unter:

```text
crates/tetra-entities/tests/common/two_cell.rs
```

## Management- und WebUI-Auswirkung

TLMC und TLPD bleiben funknahe In-Process-Komponenten der TBS und erhalten keinen eigenen LXC. Deshalb entsteht in Paket B bis E keine separate TLMC- oder TLPD-WebUI beziehungsweise kein eigener Container.

Diagnosewerte aus diesen Schichten müssen jedoch später über die TBS-WebUI und den Node Gateway sichtbar werden, insbesondere:

- Scan-, Monitor- und Measurement-Zustände;
- Cell Candidates und Selection-Ergebnisse;
- Channel-Change- und Restore-Kontexte;
- TLPD-Links, Context Keys und Fehler;
- Ressourcenverlust und Recovery.

Die zukünftigen Backend-Dienste folgen dem Standard in `Docs/BACKEND_WEBUI_STANDARD.md`.

## Nicht Bestandteil von Paket C

- noch keine Multi-TBS-Netzwerkverbindung;
- noch kein LXC-Dienst;
- noch kein zentraler Mobility Core;
- noch keine vollständige D-NEW-CELL-/RESTORE-Runtime;
- noch kein vollständiger Packet Core.

Paket B hat die typsichere Grundlage geschaffen, Paket C die TLMC-Runtime, Paket D den lokalen TLPD-Lifecycle und Paket E die Robustheits- und Zwei-Zellen-Abnahme umgesetzt. SWMI Foundation 1 ist damit abgeschlossen. Als Nächstes folgen die vollständigen MLE-Zellwechsel- und Restore-PDUs.

## SWMI Mobility 1 – Paket A: abgeschlossen ✅

Auf der Foundation wurden nun die konventionellen MLE-Zellwechsel-PDUs und die lokale Infrastruktur-Runtime ergänzt:

- `U-PREPARE` ✅
- `D-NEW-CELL` ✅
- `D-PREPARE-FAIL` ✅
- `U-RESTORE` ✅
- `D-RESTORE-ACK` ✅
- `D-RESTORE-FAIL` ✅
- `U-CHANNEL-REQUEST` ✅
- `D-CHANNEL-RESPONSE` ✅
- lokale Endpoint-/Link-gebundene Transaktionsregistry ✅
- MM- und CMCE-Indikationen ✅
- negative Timeouts ✅
- read-only Snapshot für TBS-WebUI/Node Gateway ✅
- Zwei-Zellen-Prepare/Restore-Abnahmepfad ✅

Details stehen in `Docs/SWMI_MOBILITY_1_PACKAGE_A.md`.

# SWMI Foundation 1 – TLMC/TLPD-Arbeitsliste

## Status nach Paket A

Die statische Inventur zeigt:

- **19 TLMC-nahe Primitive** sind als Typnamen vorhanden;
- davon ist aktuell nur `TlmcConfigureReq` in `SapMsgInner` verdrahtet;
- **25 LTPD-nahe Primitive** sind als Typnamen vorhanden;
- davon ist aktuell nur `LtpdMleUnitdataInd` in `SapMsgInner` verdrahtet;
- zahlreiche Felder verwenden noch den generischen Platzhaltertyp `Todo`;
- `rx_tlmc_prim()` und `rx_tlpd_prim()` besitzen noch keine vollständige Runtime-Routinglogik;
- für TLMC und LTPD ist statisch keine belastbare eigene Testabdeckung nachgewiesen.

Die genauen Einzelzeilen stehen in `Docs/SAP_PRIMITIVE_MATRIX.md` und `Docs/IMPLEMENTATION_GAPS.md`.

## Paket B – Typen: verbindliche Reihenfolge

### B1 – Gemeinsame Basistypen

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

### B2 – TLMC-Typen

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

### B3 – TLPD-Typen

Alle `Todo`-Felder werden ersetzt. Priorität haben:

1. `LtpdMleUnitdataReq`
2. Configure Request/Indication
3. Connect Request/Indication/Response/Confirm
4. Disconnect Request/Indication
5. Reconnect Request/Indication/Confirm
6. Break/Resume
7. Report/Cancel/Release
8. Open/Close/Idle/Info/Activity/Busy/Enable/Disable

### B4 – `SapMsgInner`-Verdrahtung

Jede benötigte Primitive erhält eine eigene Variante. Dabei gilt:

- keine Verwendung eines generischen untypisierten Payloads;
- Variantenname entspricht dem Struct-Namen;
- Display/Debug darf bei neuen Varianten nicht panicen;
- Source, Destination und SAP werden in Tests geprüft;
- große Payloads werden nicht unnötig kopiert.

### B5 – Explizite Kontexte

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

## Paket C – TLMC Runtime: vorgesehene Reihenfolge

1. Configure
2. Ressourcenverlust/-wiederkehr
3. Measurement und Monitor
4. Scan
5. Cell Read
6. Assessment
7. Select
8. Channel-Change-Antworten

## Paket D – TLPD Runtime: vorgesehene Reihenfolge

1. vollständiges `MLE-UNITDATA` in beide Richtungen
2. Configure
3. Connect/Disconnect
4. Break/Resume
5. Reconnect
6. Context Routing
7. Report/Cancel/Release
8. Open/Close/Idle/Info und Activity/Busy

## Paket E – Mindesttests

Für jede Primitive:

- Konstruktion und Routing über `SapMsgInner`;
- korrekte Quelle, Ziel und SAP;
- Erhalt von Handle, Link-ID und Endpoint-ID;
- ungültiger Zustand;
- unbekannter Kontext;
- doppeltes Request;
- Timeout;
- Recovery;
- mindestens ein Roundtrip- oder Golden-Vector-Test, wo eine Air-PDU beteiligt ist.

## Management- und WebUI-Auswirkung

TLMC und TLPD bleiben funknahe In-Process-Komponenten der TBS und erhalten keinen eigenen LXC. Deshalb entsteht in Paket B bis E keine separate TLMC- oder TLPD-WebUI.

Diagnosewerte aus diesen Schichten müssen jedoch später über die TBS-WebUI und den Node Gateway sichtbar werden, insbesondere:

- Scan-, Monitor- und Measurement-Zustände;
- Cell Candidates und Selection-Ergebnisse;
- Channel-Change- und Restore-Kontexte;
- TLPD-Links, Context Keys und Fehler;
- Ressourcenverlust und Recovery.

Die zukünftigen Backend-Dienste folgen dem Standard in `Docs/BACKEND_WEBUI_STANDARD.md`.

## Nicht Bestandteil von Paket B

- noch keine Multi-TBS-Netzwerkverbindung;
- noch kein LXC-Dienst;
- noch kein zentraler Mobility Core;
- noch keine vollständige D-NEW-CELL-/RESTORE-Runtime;
- noch kein vollständiger Packet Core.

Paket B schafft ausschließlich die typsichere Grundlage, auf der Paket C und D ohne erneute Datenmodelländerung aufgebaut werden können.

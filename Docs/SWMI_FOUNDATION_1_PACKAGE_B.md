# SWMI Foundation 1 – Paket B: TLMC-/TLPD-Typfundament

## Status

**Umgesetzt.** Paket B ersetzt die bisherigen leeren beziehungsweise generischen TLMC-/TLPD-Platzhalter durch ein gemeinsames, typisiertes Datenmodell. Die eigentlichen TLMC- und TLPD-Zustandsübergänge folgen in Paket C und Paket D.

## Ziel des Pakets

Vor der Runtime-Implementierung mussten die lokalen Service Access Points zwischen MLE, LLC, MAC und SNDCP vollständig modelliert werden. Ohne diese Typbasis würden Scan, Zellwahl, Channel Change, Advanced Links und SNDCP-Kontexte später mehrfach mit unterschiedlichen Datenmodellen implementiert.

Paket B umfasst deshalb:

- gemeinsame normative Basistypen;
- vollständige TLMC-Primitive;
- vollständige LTPD-Primitive;
- vollständige Varianten in `SapMsgInner`;
- explizite Zustände für Scan, Auswahl, MLE-Zelle, Channel Change und LTPD-Link;
- typisierte Context Keys für mehrere Teilnehmer und NSAPI;
- erste Unit- und Integrationstests;
- statische Paketprüfung und GitHub-Actions-Workflow.

## Normative Einordnung

Die Typen orientieren sich an ETSI EN 300 392-2, insbesondere an:

- LTPD-SAP und dessen Zustands-/Primitive-Modell;
- TLC-/TMC-SAP für lokales Layer Management;
- TL-/TMC-CONFIGURE;
- Assessment, Measurement, Monitor, Scan, Cell Read und Select;
- Advanced-Link-Setup, Disconnect und Reconnect;
- MLE-UNITDATA und zugehörigen Reports.

Die Typen bilden lokale, prozessinterne SAP-Grenzen ab. Sie sind **kein** Netzwerkprotokoll zwischen TBS und künftigem System-Backend.

## Neue gemeinsame Typen

Ablage:

```text
crates/tetra-saps/src/common/mod.rs
```

Wichtige Gruppen:

### Mobilität und Zellverwaltung

- `CellIdentity`
- `CellCandidate`
- `CellServiceLevel`
- `MleCellState`
- `SelectionCause`
- `SelectionResult`
- `ChannelChangeHandle`
- `ChannelChangeDecision`
- `ChannelChangeState`
- `RestoreContext`

### Messung und RF-Kandidaten

- `MeasurementValue`
- `MeasurementReport`
- `QualityIndication`
- `RfChannelNumber`
- `RfChannelCharacteristics`
- `ChannelClassAssessmentRequest`
- `ChannelClassMeasurement`
- `ScanningMeasurementMethod`

### Layer-2- und Packet-Data-Kontext

- `Layer2Qos`
- `DataPriority`
- `PduPriority`
- `Nsapi`
- `ReservationInfo`
- `SndcpStatus`
- `TransferResult`
- `SetupReport`
- `LtpdContextKey`
- `LtpdLinkState`

Eingeschränkte normative Wertebereiche werden soweit sinnvoll durch Konstruktoren und `validate()` geprüft, statt beliebige nackte Integer zu akzeptieren.

## TLMC

Ablage:

```text
crates/tetra-saps/src/tlmc/mod.rs
```

Implementiert wurden die Typen für:

- Assessment Indication
- Assessment List Request
- Cell Read Request/Confirm
- Configure Request/Indication/Confirm
- Measurement Indication
- Monitor Indication
- Monitor List Request
- Report Indication
- Scan Request/Confirm/Report Indication
- Select Request/Indication/Response/Confirm

Der bisherige Platzhalter `TlmcCellReadInd` bleibt vorübergehend als deprecated Alias auf `TlmcCellReadReq` erhalten, damit externe Entwicklungsstände nicht unnötig hart brechen.

## LTPD

Ablage:

```text
crates/tetra-saps/src/ltpd/mod.rs
```

Typisiert wurden:

- Activity
- Break
- Busy
- Cancel
- Close
- Configure Request/Indication
- Connect Request/Indication/Response/Confirm
- Disable/Enable
- Disconnect Request/Indication
- Idle/Info/Open/Receive
- Reconnect Request/Indication/Confirm
- Release
- Report
- Resume
- Unitdata Request/Indication

`LtpdMleUnitdataInd` enthält jetzt zusätzlich einen expliziten `ReceivedAddressType`. Die bestehenden MLE-BS- und MLE-MS-Konstruktionsstellen wurden darauf angepasst. Auf der MS-Seite wurden die SNDCP-Indications außerdem auf `TlpdSap` und `TetraEntity::Sndcp` korrigiert, statt sie irrtümlich über LCMC an CMCE zu routen.

## `SapMsgInner`

Alle benötigten TLMC- und LTPD-Primitiven besitzen nun eine eigene Variante. Der `Display`-Fallback panikt nicht mehr bei einer neuen Variante, sondern verwendet eine Debug-Darstellung.

Damit kann Paket C die Runtimepfade schrittweise aktivieren, ohne bei jeder Primitive erneut den gemeinsamen Message-Bus umbauen zu müssen.

## Tests und Prüfhilfen

Neu:

```text
crates/tetra-saps/tests/swmi_foundation_types.rs
tools/check_swmi_foundation_types.py
.github/workflows/swmi-foundation-types.yml
```

Der statische Checker prüft unter anderem:

- Vorhandensein aller erwarteten Typen;
- vollständige `SapMsgInner`-Varianten;
- keine aktiven `Todo`-/`unimplemented!`-Platzhalter in den neuen Typmodulen;
- nicht panikenden `SapMsgInner`-Displaypfad;
- vollständige `received_address_type`-Felder;
- korrektes MS-Routing zum LTPD-SAP;
- benötigte Equality-/Hash-Traits für TETRA-Adressen;
- ausgeglichene Klammerstrukturen.

Die CI kompiliert und testet das typisierte SAP-Crate zusätzlich mit:

```bash
cargo test -p tetra-saps
```

## WebUI- und Container-Auswirkung

Paket B erzeugt keinen neuen Container. TLMC und TLPD bleiben Teil der zeitkritischen TBS-Runtime.

Die neuen Typen sind aber bewusst so strukturiert, dass Paket C/D später folgende Diagnosewerte an TBS-WebUI und Node Gateway liefern können:

- aktiver Scan und dessen Fortschritt;
- überwachte RF-Kanäle;
- Cell Candidates;
- Selection- und Channel-Change-Ergebnisse;
- Ressourcenverlust und Wiederkehr;
- aktive LTPD-Links;
- Endpoint-/Link-/NSAPI-Zuordnung;
- Reconnect- und Transferfehler.

Die allgemeine WebUI-Pflicht für spätere Backend-Container bleibt unverändert bestehen.

## Bewusste Grenzen

Noch nicht enthalten:

- keine TLMC-Runtime-State-Machine;
- keine echte Scan-/Monitor-/Select-Ausführung;
- keine TLPD-Downlink-Runtime;
- kein Connect-/Reconnect-Lifecycle;
- keine Multi-TBS-Verbindung;
- kein neuer LXC-Dienst;
- keine D-NEW-CELL-/RESTORE-Runtime;
- kein vollständiger Packet Core.

## Nächster Schritt

**Paket C – TLMC Runtime** beginnt in dieser Reihenfolge:

1. Configure Request/Indication/Confirm;
2. Ressourcenverlust und Ressourcenwiederkehr;
3. Measurement;
4. Monitor/Monitor List;
5. Scan;
6. Cell Read;
7. Assessment;
8. Select und Channel-Change-Antworten.

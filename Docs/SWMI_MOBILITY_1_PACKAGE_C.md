# SWMI Mobility 1 – Paket C: MM Migration und Forward Registration

## Status

Dieses Paket ergänzt die lokale MM-Seite um die bislang fehlenden, zustandsbehafteten Verfahren für:

- zweistufige Migration mit `D-LOCATION-UPDATE-PROCEEDING` und VASSI;
- Abschluss der Migration durch eine zweite `U-LOCATION-UPDATE-DEMAND`;
- optionalen Import eines auf einer anderen TBS exportierten Teilnehmerkontexts;
- Forward Registration über `U-PREPARE` und eingebettete MM-PDUs;
- kontrollierte Accept-, Reject- und Timeout-Pfade;
- Übernahme von Gruppenaffiliationen, Energy-Economy-Daten, TEI und lokalem Handle;
- read-only Diagnosewerte für TBS-WebUI, Node Gateway und späteren Mobility Core.

Die Runtime bleibt bewusst in der TBS. Der spätere `system-backend/mobility-core` transportiert und autorisiert Kontexte zwischen Nodes, besitzt aber keine Air-Interface-Timer oder lokalen Linkzustände.

## Zweistufige Migration

```text
U-LOCATION-UPDATE-DEMAND (Migrating, USSI + Home MNI)
  └─ MM legt MigrationTransaction an
      ├─ lokale VASSI aus reserviertem Pool
      └─ D-LOCATION-UPDATE-PROCEEDING (VASSI + Home MNI)

U-LOCATION-UPDATE-DEMAND (Demand, adressiert mit VASSI)
  └─ Home ISSI/MNI validieren
      ├─ optional übertragenen Teilnehmerkontext importieren
      ├─ Teilnehmer unter lokaler VASSI registrieren
      └─ D-LOCATION-UPDATE-ACCEPT
```

Der lokale Standalone-Pool verwendet standardmäßig `0xE00000..0xEFFFFE`. Die Vergabe prüft Kollisionen mit bekannten Teilnehmern und aktiven Migrationen. Die spätere zentrale Vergabe wird über den Mobility Core konfigurierbar und netzweit autoritativ.

## Context Transfer

Der transportierbare MM-Kontext enthält:

- Home-ISSI;
- Teilnehmerzustand;
- affiliierte GSSIs;
- Energy-Saving-Mode;
- Monitoring Frame und Multiframe;
- Class of MS;
- letztes lokales Handle;
- TEI.

Die öffentliche TBS-seitige Übergabe erfolgt über:

```rust
MmBs::export_mobility_context(issi)
MmBs::provide_migration_context(vassi, context)
MmBs::import_mobility_context(local_issi, &context)
```

Diese Methoden sind lokale Adapterpunkte. Sie sind **nicht** das spätere Edge/Core-Wire-Protokoll.

## Forward Registration

MLE übergibt eine in `U-PREPARE` enthaltene `U-LOCATION-UPDATE-DEMAND` als typisierte `LmmMlePrepareInd` an MM. Dadurch bleiben erhalten:

- Teilnehmeradresse;
- Endpoint-ID;
- Link-ID;
- Cell-Identifier;
- vollständige eingebettete MM-SDU.

MM validiert den bestehenden Teilnehmer, exportiert dessen Kontext und antwortet intern an MLE:

```text
MleCellChangeControl::GrantPrepare
  ├─ ChangeChannelImmediately
  └─ eingebettete D-LOCATION-UPDATE-ACCEPT
```

Fehler werden als `RejectPrepare` mit eingebetteter `D-LOCATION-UPDATE-REJECT` und einer passenden MLE-Ursache zurückgegeben.

Der exportierte Kontext kann anschließend über:

```rust
MmBs::take_forward_context(issi)
```

vom späteren Mobility Core oder einem Testharness übernommen werden.

## Zustandsmaschine

Migration:

```text
ProceedingSent
  ├─ MigrationAccepted
  ├─ MigrationRejected
  └─ TimedOut
```

Forward Registration:

```text
ForwardRegistrationRequested
  ├─ ForwardRegistrationAccepted
  │   └─ ContextTransferred
  ├─ ForwardRegistrationRejected
  └─ TimedOut
```

Alle offenen Vorgänge besitzen einen konservativen Grenzwert von 432 Timeslots. Ein Timeout erzeugt einen definierten Reject-Pfad statt eines hängenden Teilnehmerzustands.

## PDU-Arbeiten

Zusätzlich wurden vervollständigt:

- Parser und Encoder für `D-LOCATION-UPDATE-PROCEEDING`;
- Parser für `D-LOCATION-UPDATE-REJECT` einschließlich bedingter Ciphering Parameters;
- Clone-Fähigkeit der generischen Type-3-/Type-4-Felder für transportierbare PDU-Strukturen.

## Standalone und Core-managed

Im Standalone-Modus kann eine einzelne TBS:

- VASSI lokal vergeben;
- Migration vollständig auf der Air-Interface-Seite beantworten;
- einen zuvor bereitgestellten Kontext übernehmen;
- Forward Registration lokal vorbereiten.

Im späteren Core-managed-Modus übernimmt der Mobility Core:

- Auswahl der zuständigen Quell- und Ziel-TBS;
- netzweite Identitäts- und Kontextprüfung;
- Context Transfer über das Edge Protocol;
- Konflikt- und Split-Brain-Auflösung;
- Audit und Operatorfreigaben.

Die TBS behält:

- MM-PDU-Verarbeitung;
- lokale Timer;
- VASSI-/Link-Zuordnung der laufenden Air-Transaktion;
- D-PDU-Erzeugung;
- lokalen Fallback.

## WebUI und Diagnose

`MmBs::mobility_snapshot()` liefert:

- aktive Migrationen und Forward Registrations;
- VASSI, Home-ISSI und Home-MNI;
- Phase und Alter in Timeslots;
- Ziel-Location-Area und Cell-Identifier;
- Anzahl übertragener Gruppen;
- Accept-, Reject-, Duplicate-, Mismatch- und Timeout-Zähler;
- Vorhandensein eines importierten Kontexts.

Die spätere TBS-WebUI zeigt die lokale Air-Transaktion. Die WebUI des Mobility Core zeigt den netzweiten Context Transfer und die Zuordnung zu Quell-/Ziel-TBS.

## Tests

Neu enthalten sind:

- PDU-Roundtriptests für Proceeding und Reject;
- Runtime-Tests für VASSI, Identitätsprüfung, Context Transfer und Timeout;
- Zwei-Zellen-MM-Test für Migration und Forward Registration;
- statische Architekturprüfung;
- GitHub-Actions-Workflow mit Format-, Test- und Clippy-Abnahme.

## Bewusste Grenze

Dieses Paket baut noch **keinen laufenden Mobility-Core-Container** und kein Netzwerkprotokoll zwischen TBS. Es stellt die lokale, testbare und exportierbare MM-Funktion bereit, auf der der spätere Core sicher aufsetzen kann.

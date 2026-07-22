# SWMI Foundation 1 – Paket C: TLMC Runtime

## Status

**Umgesetzt.** Paket C aktiviert die in Paket B typisierten TLC-/TMC-Primitiven als lokale Runtime zwischen MLE und Layer 2.

TLMC bleibt eine zeitkritische, prozessinterne TBS-/MS-Komponente. Es wird **kein eigener LXC** erzeugt. Die Runtime stellt jedoch einen read-only Diagnose-Snapshot bereit, der später durch die TBS-WebUI und den Node Gateway veröffentlicht werden kann.

## Ziel

Paket B hatte Strukturen und Message-Bus-Varianten geschaffen. Paket C ergänzt nun:

- eine explizite TLMC-Zustandsmaschine;
- Konfigurationsprüfung und partielle Konfigurationsupdates;
- kantengetriggerte Meldung von Ressourcenverlust und Ressourcenwiederkehr;
- Measurement- und Monitor-Verarbeitung;
- korrelierte Scan-, Cell-Read- und Select-Abläufe;
- Assessment-Listen und validierte Assessment-Ergebnisse;
- definierte Fehler- und Timeoutpfade;
- defensives Verhalten in der BS-Runtime;
- Diagnosezustände für die spätere WebUI.

## Normative Einordnung

Die Umsetzung orientiert sich an ETSI EN 300 392-2, insbesondere an den lokalen TLC-/TMC-Diensten in Abschnitt 20.3.5.4 und dem TMC-SAP in Abschnitt 20.4.3. Die dort beschriebenen Primitive übertragen lokale Managementinformationen zwischen MLE, LLC und MAC; sie werden nicht über die Luftschnittstelle übertragen.

Wesentliche Primitive:

- TL-ASSESSMENT / TL-ASSESSMENT-LIST;
- TL-CELL-READ;
- TL-CONFIGURE;
- TL-MEASUREMENT;
- TL-MONITOR / TL-MONITOR-LIST;
- TL-REPORT;
- TL-SCAN / TL-SCAN-REPORT;
- TL-SELECT.

## Neue Runtime

Ablage:

```text
crates/tetra-entities/src/umac/tlmc_runtime.rs
```

Die Runtime besitzt getrennte Zustände für:

- aktuelle TLMC-Konfiguration;
- Ressourcenstatus je Endpoint;
- überwachte RF-Kanäle;
- angeforderte Channel Classes;
- laufenden Scan;
- laufendes Cell Read;
- laufende Auswahl;
- BS-gesteuerte Select-Indication;
- aktuelle Zelle;
- letzte Measurement- und Monitorwerte.

### Fehlerzustände

Explizit unterschieden werden:

- ungültige Konfiguration;
- bereits laufende Operation;
- unbekannte Operation;
- falsche Request-Korrelation;
- nicht überwachter Kanal;
- nicht angeforderte Channel Class;
- fehlende Select-Indication.

Fehler werden nicht durch Panic behandelt, sondern als TLMC-Report beziehungsweise negative Confirmation an MLE gemeldet.

## Configure

`TlmcConfigureReq` wird validiert und anschließend als partielles Update in die aktive Konfiguration übernommen.

Geprüft werden derzeit insbesondere:

- Frame-18-Timeslot im Bereich 1 bis 4;
- Energy-Economy-Startpunkte;
- Dual-Watch-Startpunkte;
- Schedule-Repetition-Parameter.

Die Confirmation spiegelt ausschließlich die von Layer 2 übernommenen Werte zurück.

## Ressourcenverlust und Recovery

Die Runtime speichert den letzten Zustand je Endpoint und erzeugt eine `TlmcConfigureInd` nur bei einer tatsächlichen Zustandskante:

```text
unknown/unavailable -> available
available           -> unavailable
unavailable         -> available
```

Wiederholte identische Beobachtungen erzeugen keine Meldungsflut.

UMAC-MS meldet einen Ressourcenverlust, wenn länger als 432 Timeslots keine gültige Serving-Channel-Beobachtung eingegangen ist. Eine neue gültige BSCH-Beobachtung erzeugt die Recovery-Indication.

## Measurement und Monitor

Gültige Serving-Channel-Beobachtungen erzeugen:

- periodisch eine `TlmcMeasurementInd`;
- bei explizit überwachtem Kanal zusätzlich eine `TlmcMonitorInd`.

Messwerte besitzen weiterhin eine explizite Einheit. Der aktuelle PHY-Pfad liefert RSSI in dBFS; dieser Wert wird deshalb als `MeasurementValue::Raw` weitergereicht und nicht fälschlich als normativer C1/C2-dB-Wert ausgegeben.

Channel-Class-spezifische Ergebnisse werden nur akzeptiert, wenn die jeweilige Klasse vorher über `TlmcAssessmentListReq` angefordert wurde.

## Scan, Cell Read und Select

Alle Operationen sind korreliert und erlauben jeweils nur eine laufende lokale Operation ihres Typs.

### Scan

1. MLE sendet `TlmcScanReq`.
2. UMAC legt Request-ID und Zielträger ab.
3. UMAC fordert den Zielträger beim unteren MAC an.
4. Eine passende gültige Kanalbeobachtung beendet den Scan.
5. UMAC sendet `TlmcScanConf`.

### Cell Read

1. MLE sendet `TlmcCellReadReq`.
2. UMAC fordert den Zielträger an.
3. Eine passende SYSINFO-Beobachtung beendet die Operation.
4. UMAC sendet `TlmcCellReadConf`.

### Select

1. MLE sendet `TlmcSelectReq`.
2. UMAC speichert Kandidat und Selection Cause.
3. Eine passende gültige Beobachtung beendet die Auswahl.
4. UMAC sendet `TlmcSelectConf` und aktualisiert den lokalen Zellzustand.

BS-gesteuerte `TlmcSelectInd`/`TlmcSelectResp` besitzen zusätzlich eine Handle-Prüfung.

### Timeout

Scan, Cell Read und Select werden nach 432 Timeslots negativ abgeschlossen, wenn keine passende Lower-Layer-Beobachtung erfolgt. MLE bleibt damit nicht unbegrenzt auf einer lokalen Operation hängen.

## UMAC-MS

`UmacMs` enthält nun:

- die TLMC-Runtime;
- den vollständigen TLMC-Dispatcher;
- Configure-Verarbeitung;
- Monitor- und Assessment-Listen;
- Scan-, Cell-Read- und Select-Start;
- Measurement-/Monitor-Erzeugung;
- Ressourcenverlust und Recovery;
- Operationstimeouts;
- einen read-only `tlmc_snapshot()`.

## UMAC-BS

Scan, Monitor, Assessment und Select sind in der normativen Grundlogik MS-seitige Funktionen. `UmacBs` führt sie deshalb nicht irrtümlich aus.

Die BS-Runtime:

- akzeptiert eine lokale Configure-Anforderung;
- bestätigt gültige Konfigurationen;
- meldet ungültige Konfigurationen mit Reject;
- beantwortet MS-seitige TLMC-Anforderungen sauber mit `ServiceNotSupported`;
- besitzt ebenfalls einen Diagnose-Snapshot.

Damit kann eine falsch geroutete Primitive den BS-Worker nicht mehr durch einen unimplementierten Pfad beenden.

## MLE-Anbindung

`MleBs` konsumiert jetzt die erzeugten TLMC-Indications und Confirmations ohne `unimplemented_log!` im TLMC-Dispatcher.

Die eigentlichen Mobility-Entscheidungen auf Basis dieser Ergebnisse folgen in den späteren MLE-/MM-Paketen. Paket C stellt hierfür die verlässlichen Lower-Layer-Ergebnisse bereit.

## Diagnose und WebUI

`TlmcRuntimeSnapshot` stellt read-only bereit:

- Scan State;
- Selection State;
- konfigurierten Endpoint;
- überwachte Kanäle;
- angeforderte Channel Classes;
- laufendes Cell Read;
- Anzahl bekannter und nicht verfügbarer Ressourcen;
- letzte Measurement.

Paket C erzeugt keine neue WebUI. Die Daten sind für die bestehende beziehungsweise zukünftige TBS-WebUI vorgesehen. Die WebUI-Pflicht für alle späteren Backend-Container unter `system-backend/` bleibt unverändert.

## Tests

Neu beziehungsweise erweitert:

```text
crates/tetra-entities/tests/test_tlmc_runtime.rs
tools/check_tlmc_runtime.py
.github/workflows/swmi-foundation-tlmc-runtime.yml
```

Geprüft werden unter anderem:

- Configure Request/Confirm;
- partielle Konfiguration und Validierung;
- kantengetriggerter Ressourcenverlust und Recovery;
- erfolgreicher korrelierter Scan;
- nicht überwachter Monitor-Kanal;
- Scan-/Select-Lifecycle;
- Operationstimeout;
- Diagnose-Snapshot;
- defensives BS-Routing;
- vollständige SapMsgInner-Verdrahtung.

## Bewusste Grenze

Der TLMC-State und die UMAC-Adapterlogik sind implementiert. Ein dynamisches physisches SDR-Retuning des experimentellen MS-Stacks ist noch nicht allgemein abgeschlossen: UMAC erzeugt dafür bereits einen `TmvConfigureReq` mit Zielträger, der konkrete LMAC-/PHY-Adapter muss diesen abhängig vom eingesetzten Empfangsmodell ausführen.

Für die produktive NetCore-Basisstation ist dies kein normaler BS-Sendepfad; TLMC liefert dort vor allem das Fundament für spätere Mobilitäts-, Test- und Simulationsverfahren.

## Nächster Schritt

**Paket D – TLPD Runtime**:

1. bidirektionales MLE-UNITDATA;
2. Configure;
3. Context Registry;
4. Connect/Disconnect;
5. Break/Resume;
6. Reconnect;
7. Report/Cancel/Release;
8. Open/Close/Idle/Info und Activity/Busy.

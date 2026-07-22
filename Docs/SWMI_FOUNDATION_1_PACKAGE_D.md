# SWMI Foundation 1 – Paket D: TLPD Runtime

## Status

**Umgesetzt.** Paket D aktiviert den in Paket B typisierten LTPD-SAP als lokale Runtime zwischen SNDCP, MLE und LLC.

TLPD bleibt eine funknahe In-Process-Komponente der Basisstation. Es entsteht kein eigener Container. Die Runtime besitzt jedoch read-only Diagnose-Snapshots für die spätere TBS-WebUI und den Node Gateway.

## Ziel

Vor Paket D verlief der Uplink bereits über MLE zu SNDCP. SNDCP-Antworten umgingen MLE jedoch und wurden direkt an LLC gesendet. Außerdem bestand `rx_tlpd_prim()` nur aus einem nicht implementierten Platzhalter.

Paket D ergänzt:

- bidirektionales `MLE-UNITDATA`;
- lokale Context Registry für Endpoint, Link und Teilnehmerroute;
- Route Recovery nach lokalem MLE-Neustart;
- Configure;
- Connect/Disconnect;
- Break/Resume;
- Reconnect;
- Release und Cancel;
- Transfer Reports;
- Open/Close/Idle/Info;
- Activity, Busy, Enable und Disable;
- Diagnose-Snapshots auf MLE- und SNDCP-Seite.

## Runtime

Ablage:

```text
crates/tetra-entities/src/mle/ltpd_runtime.rs
```

Die Runtime verwaltet pro lokalem Link:

- TETRA-Adresse;
- Endpoint-ID;
- Link-ID;
- `LtpdLinkState`;
- Layer-2-QoS;
- Verschlüsselungskennzeichen;
- SNDCP-Status;
- letzte Aktivität;
- erfolgreiche und fehlgeschlagene Transfers.

Die Runtime ist nicht die spätere zentrale Packet-Core-Datenbank. Sie ist der lokale, zeitkritische Adapter zwischen SNDCP und den unteren Schichten.

## Bidirektionales MLE-UNITDATA

### Uplink

```text
LLC
 └─ TLA-TL-DATA/UNITDATA indication
     └─ MLE liest den Protokolldiskriminator
         └─ LTPD-MLE-UNITDATA indication
             └─ SNDCP
```

Beim Empfang registriert MLE die lokale Route aus:

- Teilnehmeradresse;
- Endpoint-ID;
- Link-ID.

### Downlink

```text
SNDCP
 └─ LTPD-MLE-UNITDATA request
     └─ MLE ergänzt den SNDCP-Protokolldiskriminator
         └─ TLA-TL-DATA oder TLA-TL-UNITDATA request
             └─ LLC
```

SNDCP sendet damit keine Paketdatenantworten mehr direkt an LLC.

Die Auswahl des unteren Dienstes erfolgt anhand von `Layer2Service`:

- `Acknowledged` und der alte Kompatibilitätswert `Todo` → `TlaTlDataReqBl`;
- `Unacknowledged` → `TlaTlUnitdataReqBl`.

Dynamische PDCH-Zuteilungen bleiben erhalten und werden als optionales `chan_alloc` über LTPD bis LLC transportiert.

## Route Recovery

`LtpdMleUnitdataReq` besitzt einen optionalen Adresshinweis. Dadurch kann MLE nach einem lokalen Neustart eine fehlende Route aus dem weiterhin vorhandenen SNDCP-Zustand rekonstruieren.

Ohne bekannte Route und ohne Adresshinweis wird die Anforderung abgewiesen und nicht mit einer erfundenen Adresse ausgesendet.

## Transfer Reports

Jede lokale Unitdata-Anforderung erhält einen eindeutigen `RequestHandle`.

Nach erfolgreicher Übernahme in die LLC-Queue meldet MLE:

```text
TransferResult::SuccessBufferEmpty
```

Bei unbekanntem Context, unterbrochener Ressource oder deaktiviertem Dienst:

```text
TransferResult::FailedRemovedFromBuffer
```

Die aktuelle Meldung bestätigt die lokale Übergabe an LLC. Eine spätere Erweiterung kann echte LLC-Confirmations und TxReporter bis zum gleichen Handle zurückführen, ohne die SNDCP-Schnittstelle erneut zu ändern.

## Connect, Disconnect und Reconnect

Die Advanced-Link-Lifecycle-Primitive besitzen jetzt eine explizite lokale Zustandsbehandlung:

```text
Null
Open
Connecting
Connected
Busy
Broken
Reconnecting
Releasing
Closed
Disabled
```

### Connect

- QoS wird validiert;
- Context wird angelegt oder zurückgesetzt;
- Erfolg oder Parameterfehler wird mit `LtpdMleConnectConfirm` gemeldet.

### Disconnect

- Context wird in `Closed` gesetzt;
- `LtpdMleDisconnectInd` enthält ein eindeutiges Layer-2-Ergebnis.

### Reconnect

- bestehende Contexts können nach Ressourcen- oder Zellunterbrechung reaktiviert werden;
- unbekannte Contexts werden mit `ReconnectionResult::Reject` abgewiesen.

Die vollständigen Air-Interface-Advanced-Link-PDUs bleiben ein eigener LLC/MLE-Ausbauschritt. Paket D stellt dafür den lokalen Lifecycle und die stabile SAP-Grenze bereit.

## Break/Resume und TLMC-Kopplung

`TlmcConfigureInd` steuert nun den Paketdaten-Ressourcenstatus:

```text
Unavailable → LTPD-MLE-BREAK indication
Available   → LTPD-MLE-RESUME indication
```

Die Meldungen sind kantengetriggert. Wiederholte identische Zustände erzeugen keine Meldungsflut.

Während `Broken`, `Disabled` oder `Closed` werden neue Transfers sauber abgewiesen.

## Open/Close/Info und Betriebszustände

Beim ersten MLE-Tick werden SNDCP bereitgestellt:

- MCC und MNC über `LtpdMleOpenInd`;
- lokale Broadcast-/Cell-Informationen über `LtpdMleInfoInd`.

Weiterhin unterstützt die Runtime:

- `Close`;
- `Busy`/`Idle`;
- `Disable`/`Enable`;
- `Activity` und Sleep Permission.

## SNDCP-Clientzustand

SNDCP verarbeitet nun die LTPD-Lifecycle-Indications und führt einen eigenen read-only Snapshot:

```text
SndcpLtpdSnapshot
```

Enthalten sind:

- geöffnetes Netz mit MCC/MNC;
- aktueller Linkzustand;
- Busy- und Disable-Zustand;
- letztes Transferergebnis;
- Erfolgs- und Fehlerzähler.

## WebUI

Paket D erzeugt keinen LXC und keine neue eigenständige WebUI. TLPD ist Teil der TBS.

Für die spätere TBS-WebUI stehen bereit:

- `MleBs::ltpd_snapshot()`;
- `Sndcp::ltpd_snapshot()`.

Anzuzeigen sind später mindestens:

- Netz offen/geschlossen;
- Ressourcen verfügbar/unterbrochen;
- aktive Links;
- Endpoint und Link-ID;
- Linkzustand;
- Transferzähler;
- letzter Fehler;
- Busy/Disable;
- QoS und Verschlüsselungskennzeichen.

Die allgemeine Vorgabe, dass jeder spätere Container unter `system-backend/` eine eigene WebUI erhält, bleibt unverändert bestehen.

## Tests

Neu:

```text
crates/tetra-entities/tests/test_ltpd_runtime.rs
tools/check_ltpd_runtime.py
.github/workflows/swmi-foundation-ltpd-runtime.yml
```

Geprüft werden:

- Uplink-Routing von LLC über MLE zu SNDCP;
- Aufbau der Context Registry;
- Downlink-Routing von SNDCP über MLE zu LLC;
- SNDCP-Protokolldiskriminator;
- Transfer Report und Handle-Erhalt;
- Context Recovery über Adresshinweis;
- Ablehnung unbekannter Contexts;
- Connect/Disconnect/Reconnect;
- TLMC-gesteuertes Break/Resume;
- Diagnose-Snapshot.

## Bewusste Grenzen

Paket D enthält noch nicht:

- keine zentrale Packet-Core-Auslagerung;
- keine Multi-TBS-Context-Synchronisation;
- keine vollständigen Advanced-Link-Air-PDUs;
- keine echte LLC-Transmission-Confirmation für jedes LTPD-Handle;
- keine D-NEW-CELL-/RESTORE-Runtime;
- keinen eigenen LTPD-Container.

## Nächster Schritt

**Paket E – Foundation-Abnahme und Robustheit**:

1. fehlende negative Primitive- und State-Tests;
2. Duplicate-Handle- und Cancel-Tests;
3. Timeout- und Recovery-Tests;
4. vollständige TLMC-/TLPD-Routingmatrix;
5. Cargo-/Clippy-/Format-Abnahme;
6. Zwei-Zellen-Testharness als Vorbereitung für MLE D-NEW-CELL und RESTORE.

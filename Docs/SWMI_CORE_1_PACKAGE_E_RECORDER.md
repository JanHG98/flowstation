# SWMI Core 1 – Paket E: Recorder

## Ziel

Paket E ergänzt einen eigenständigen Recorder-LXC, der netzweite TETRA-Sprachrufe passiv archiviert. Der Recorder ist kein Bestandteil des Rufpfads: Call Control bleibt Eigentümer der Rufzustände, der Media Switch routet die Frames, und der Recorder konsumiert ausschließlich einen begrenzten replay-fähigen Tap.

## Architektur

```text
TBS A/B/…
   ↓ gepackte TETRA-Sprachframes
Node Gateway
   ↓
Media Switch
   ├─ Routing zu den Ziel-TBS
   └─ Recorder-Tap-Ring
            ↓ HTTP Cursor-Polling
       Recorder LXC
            ├─ audio.tacelp
            ├─ frames.jsonl
            ├─ metadata.json
            └─ integrity.json
```

Ein Recorder-Ausfall erzeugt keine Backpressure. Kann der Tap-Ring einen Ausfall nicht vollständig überbrücken, meldet der Recorder die fehlenden Sequenzen sichtbar.

## Media-Switch-Erweiterung

Der Media Switch erhält den Endpunkt:

```text
GET /api/v1/recorder/taps?after=<seq>&limit=<n>
```

Anders als der bisherige Diagnose-Tap enthält er die vollständige 35-Byte-Payload sowie Call-, Sprecher-, Node- und Timeslot-Metadaten. `oldest_available_seq`, `newest_available_seq` und `dropped_before` ermöglichen kontrolliertes Cursor-Verhalten.

## Archivformat

Die erste Recorder-Stufe transkodiert nicht. `audio.tacelp` enthält die gepackten TETRA Speech Service 0 Frames unverändert und lückenlos hintereinander. `frames.jsonl` hält pro Frame Zeit, Herkunft, Sprecher, Sequenz und Byte-Offset fest.

Beim Finalisieren werden getrennte SHA-256-Werte für Audio und Index berechnet. Ein aktives Recovery-Manifest ermöglicht die Wiederherstellung nach einem unsauberen Neustart.

## Rufmetadaten

Gespeichert werden unter anderem:

- logische Call-ID und Rufart
- Gruppenadresse oder Teilnehmeradressen
- Priorität und Notrufkennzeichen
- Quell-TBS und logische Timeslots
- erkannte Sprecher-ISSIs
- Sprechersegmente
- Anzahl der Frames und erkannte Tap-Lücken
- Aufbewahrungsfrist und Legal Hold

## Verwaltung

Die eigene WebUI auf Port 8140 bietet:

- laufende und abgeschlossene Aufnahmen
- Suche nach Call, ISSI, GSSI und TBS
- Storage- und Verbindungszustand
- Sprecher- und Notrufanzeige
- Hashprüfung
- Retention und Legal Hold
- TAR-Export
- manuelle Finalisierung und – sofern freigegeben – endgültige Löschung

## Sicherheitsmodus

Wie die bisherige Testumgebung implementiert das Paket ausschließlich `open_lab`:

- keine Tokens
- keine Benutzerkonten
- keine Passwörter
- kein TLS
- kein RBAC

Port 8140 darf daher nur im isolierten Managementnetz erreichbar sein.

## Definition of Done

- Recorder ist Workspace-Crate und eigenständig baubar
- eigener LXC-Installationssatz und systemd-Unit vorhanden
- eigene WebUI, REST, Health, Metrics und OpenAPI vorhanden
- Vollframe-Tap des Media Switch implementiert
- 35-Byte-Frames werden unverändert archiviert
- Recovery, Hashprüfung, Retention, Hold und Export implementiert
- statische Paketprüfung und CI-Workflow vorhanden

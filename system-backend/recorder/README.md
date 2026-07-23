# NetCore Recorder

Der Recorder ist ein eigenständiger LXC-Dienst für die passive, verlustfreie Ablage bereits codierter TETRA-Sprachframes. Er hängt ausschließlich am replay-fähigen Recorder-Tap des Media Switch und liegt damit außerhalb des zeitkritischen Rufpfads.

> Ein Ausfall des Recorders darf weder Call Control noch Media Switch noch einen laufenden Ruf blockieren.

## Umgesetzter Umfang

- Polling des Vollframe-Taps `GET /api/v1/recorder/taps?after=<seq>&limit=<n>`
- exakte Ablage jedes 35-Byte-TETRA-ACELP-Frames in `audio.tacelp`
- JSONL-Index mit Tap-Sequenz, Zeit, TBS, Timeslot, Sprecher und Byte-Offset
- Rufmetadaten für Gruppen-, Einzel- und Notrufe
- Sprechersegmente nach Floor Holder, TBS und logischem Timeslot
- Erkennung von Tap-Lücken, Duplikaten und ungültigen Frames
- SHA-256-Integritätsdatei für Audio und Index
- Wiederherstellung nicht sauber finalisierter `.part`-Aufnahmen beim Neustart
- Aufbewahrungsfrist, Legal Hold, manuelle Löschung und TAR-Export
- Health, Readiness, Prometheus-Metriken und OpenAPI
- eigene eingebettete WebUI auf Port `8140`

## Sicherheitszustand dieser Phase

Der Dienst implementiert absichtlich nur `open_lab`:

- keine Tokens
- keine Benutzerkonten
- keine Passwörter
- kein TLS
- kein RBAC

Jeder Client, der Port 8140 erreicht, kann die Management-API benutzen. Das ist ausschließlich für ein isoliertes Testnetz gedacht.

## Datenfluss

```text
TBS → Node Gateway → Media Switch
                       ├─ zeitkritisches TBS-zu-TBS-Routing
                       └─ begrenzter Replay-Ring mit Vollframes
                                      ↓ HTTP Polling
                                  Recorder LXC
                                      ↓
                     audio.tacelp + frames.jsonl + metadata + SHA-256
```

Der Media Switch wartet nicht auf den Recorder. Wenn der Recorder länger ausfällt als der Replay-Ring überbrücken kann, wird die Lücke in globalen Zählern und – soweit zuordenbar – in den Aufnahmemetadaten sichtbar.

## Schnellstart

```bash
sudo cp system-backend/recorder/config/recorder.example.toml /etc/netcore/recorder.toml
sudo nano /etc/netcore/recorder.toml
sudo system-backend/recorder/install/install.sh
```

WebUI:

```text
http://<RECORDER-LXC-IP>:8140/
```

Kontrolle:

```bash
curl http://127.0.0.1:8140/health/live
curl http://127.0.0.1:8140/health/ready
curl http://127.0.0.1:8140/api/v1/status
```

## Archivlayout

```text
/var/lib/netcore-recorder/recordings/YYYY/MM/DD/<recording-id>/
├── audio.tacelp
├── frames.jsonl
├── metadata.json
└── integrity.json
```

Während der Aufnahme heißen Audio und Index `*.part`; `metadata.active.json` ist das Recovery-Manifest. Erst beim Finalisieren werden die endgültigen Dateinamen atomar veröffentlicht.

## REST-Endpunkte

| Methode | Pfad | Zweck |
|---|---|---|
| GET | `/api/v1/status` | Dienst-, Storage- und Tap-Zustand |
| GET | `/api/v1/active` | laufende Aufnahmen |
| GET | `/api/v1/recordings` | Suche und Liste |
| GET | `/api/v1/recordings/{id}` | Metadaten |
| POST | `/api/v1/recordings/{id}/verify` | Hashprüfung |
| POST | `/api/v1/recordings/{id}/retention` | Frist ändern |
| POST | `/api/v1/recordings/{id}/hold` | Legal Hold setzen/lösen |
| POST | `/api/v1/recordings/{id}/finalize` | aktive Aufnahme manuell schließen |
| POST | `/api/v1/recordings/{id}/delete` | endgültig löschen, sofern erlaubt |
| GET | `/api/v1/recordings/{id}/export` | unkomprimiertes TAR erzeugen/laden |
| GET | `/api/v1/events` | Ereignisverlauf |
| GET | `/metrics` | Prometheus-Textformat |

Weitere Details stehen unter `docs/`.

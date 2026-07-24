# NetCore-Tetra Entwicklungswerkzeuge

## Protocol Inventory

`protocol_inventory.py` erzeugt die statische PDU-, SAP-, Gap- und State-Machine-Inventur.

### Aktualisieren

```bash
python3 tools/protocol_inventory.py
```

### Prüfen

```bash
./tools/check_protocol_inventory.sh
```

Der Check beendet sich mit Fehlercode 1, sobald Quellcode und eingecheckte Inventurdateien nicht mehr zusammenpassen.

## Grenzen

Das Werkzeug arbeitet bewusst ohne externe Python-Pakete und verwendet konservative Quelltextanalyse. Es kann:

- vorhandene Parser-/Encoder-Einstiegspunkte erkennen,
- offensichtliche Platzhalter und Panic-Pfade finden,
- SAP-Primitive und `SapMsgInner`-Verdrahtung erfassen,
- Testverweise und State-Enums zählen.

Es kann nicht:

- ETSI-Konformität zertifizieren,
- semantische Korrektheit eines Codecs beweisen,
- On-Air-Kompatibilität bestätigen,
- implizite Zustände vollständig rekonstruieren.

## Packet Core

```bash
python3 tools/check_packet_core.py
```

Der Check prüft Paketstruktur, Workspace-Einbindung, TBS-Control-Routing, SNDCP-Endpunkt, WebUI/API und Open-Lab-Konfiguration des Packet-Core-LXC.

- `check_security_core.py`: prüft Security-Core-Paket, Open-Lab-Konfiguration, Secret-Redaction, WebUI-JavaScript und Installationsskripte.

- `check_kmf.py`: prüft KMF-Paket, Vault-/Secret-Grenzen, nodegebundene OTAR-Envelopes, Audit-Hashkette, WebUI-JavaScript und Installationsskripte.

- `check_transit.py`: prüft Transit-Paket, Regionen-/Peer- und Routing-Grundlagen, Path-Vector/Loop-Prevention, Failover, Open-Lab-Konfiguration, WebUI-JavaScript und Installationsskripte.

- `check_control_room.py`: prüft Control-Room-Federation, Open-Lab-Konfiguration, Architekturgrenze, Incident-/Schichtbuch-Paket, Browser-WebUI und LXC-Skripte.

- `check_observability.py`: prüft Observability/NMS, Targets, Alarmregeln, Stack-Konfigurationen, WebUI und LXC-Skripte.

- `check_application_gateway.py`: prüft Application-Gateway-Paket, Connectorinventar, Routing/Vorlagen, Secret-Redaction, TTS-WAV-Grenze, WebUI und LXC-Skripte.

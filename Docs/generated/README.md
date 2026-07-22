# Generierte Inventurdaten

Die Dateien in diesem Ordner werden durch folgendes Werkzeug erzeugt:

```bash
python3 tools/protocol_inventory.py
```

Sie dürfen nicht von Hand gepflegt werden.

## Dateien

- `protocol_inventory.json` – vollständige maschinenlesbare Inventur
- `pdu_inventory.csv` – PDU-Codec-Matrix
- `sap_inventory.csv` – SAP-Primitive und Routingstatus
- `gap_inventory.csv` – Runtime-Platzhalter, Panic-Pfade und technische Schuld
- `state_inventory.csv` – gefundene State-Typen

## Konsistenzprüfung

```bash
python3 tools/protocol_inventory.py --check
```

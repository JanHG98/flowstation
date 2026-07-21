# NetCore-Tetra: Multi-PDCH, Paketdaten-Dashboard und Legacy-WAP/SDS

Stand: 2026-07-21

Dieses Paket baut direkt auf dem vom Betreiber bereitgestellten und zuvor erfolgreich kompilierten Stand `netcore-tetra-wap(1).zip` auf.

## 1. Funktionsumfang

### 1.1 Dynamischer Multi-PDCH-Scheduler

Der bisher feste Paketdatenkanal wurde durch einen dynamischen Bearer-Pool ersetzt.

- Jeder aktive Teilnehmer kann einen eigenen ein-Slot-PDCH erhalten.
- Mehrere NSAPIs derselben ISSI teilen denselben Funk-Bearer.
- Mehrere ISSIs können gleichzeitig Paketdaten verwenden.
- Sprache und SNDCP greifen auf denselben zentralen Timeslot-Allocator zu.
- Paketdaten können dadurch keinen bereits von CMCE/Brew belegten Slot verwenden.
- Bei Dual Carrier werden standardmäßig zuerst Carrier 2 TS2 bis TS4 gewählt.
- Carrier 1 bleibt dadurch bevorzugt für Sprache und Rufaufbau frei.
- Ein konfigurierbarer Voice-Headroom verhindert, dass Paketdaten alle Traffic-Slots belegen.
- READY-, END-OF-DATA-, Deaktivierungs- und Deregistrierungsabläufe geben exakt den zugewiesenen Slot frei.
- Telemetrie zeigt pro Bearer ISSI, Carrier, Air-Timeslot, logischen Timeslot und NSAPIs.

Logische Timeslot-Zuordnung:

| Logischer Slot | Funk-Bearer |
|---:|---|
| 2 | Hauptcarrier TS2 |
| 3 | Hauptcarrier TS3 |
| 4 | Hauptcarrier TS4 |
| 5 | Sekundärcarrier TS2 |
| 6 | Sekundärcarrier TS3 |
| 7 | Sekundärcarrier TS4 |

TS1 bleibt auf beiden Carriern außerhalb des dynamischen Paketdatenpools.

### 1.2 Konfiguration

Im Abschnitt `[cell_info.packet_data_gateway]` stehen drei neue Parameter:

```toml
# 0 = automatisch alle verfügbaren Paketdaten-Bearer verwenden.
# Ein expliziter Wert begrenzt die Anzahl auf 1 bis 6.
max_pdch_bearers = 0

# So viele freie Traffic-Slots werden für Sprache/Notruf freigehalten.
reserved_voice_slots = 1

# Bei Dual Carrier neue PDCHs zunächst auf Carrier 2 legen.
prefer_secondary_carrier = true
```

Praktische Kapazität mit den Standardwerten:

- Single Carrier: maximal 2 gleichzeitige PDCH-Bearer.
- Dual Carrier: maximal 5 gleichzeitige PDCH-Bearer.

Das ist eine Schutzgrenze. Bereits laufende Paketdaten werden nicht hart für einen neuen Sprachruf präemptiert; stattdessen verhindert der reservierte Headroom, dass der letzte Sprachslot überhaupt an SNDCP vergeben wird.

### 1.3 Lokales Paketdaten-Dashboard

Das Basisstations-Dashboard enthält eine neue Seite **Paketdaten**.

Angezeigt werden:

- TUN-/Gateway-Status;
- Gateway-Adresse und Interface;
- Uplink-/Downlink-Pakete und Bytes;
- Drops und I/O-Fehler;
- Downlink-Warteschlange;
- aktive PDP-Kontexte;
- PDP-Zustand, IPv4, SNEI, NSAPI, MTU und Priorität;
- aktive PDCH-Bearer;
- Carrier, Air-TS und logischer Slot;
- freie Traffic-Slots und reservierter Sprach-Headroom.

Lokale Schnittstellen:

```text
GET /api/packet-data
WebSocket event: type=packet_data
```

Die Seite enthält außerdem ein Formular zum Versand einer kompakten WML-Karte über SDS Type 4.

### 1.4 Control-Room-Integration

Der Control-Room-Core speichert die Paketdaten-Telemetrie pro Node und stellt sie aggregiert bereit:

```text
GET /api/packet-data
GET /api/nodes/{node_id}/packet-data
```

Die Node-Übersicht enthält zusätzlich:

- Paketdaten-Gateway aktiv;
- aktive PDP-Kontexte;
- aktive PDCH-Bearer;
- PDCH-Kapazität.

Der native Control-Room-Client enthält eine neue Seite **Paketdaten** mit Gateway-, Bearer- und Kontexttabellen sowie dem Legacy-WAP-Sendeformular.

Operator-CLI:

```bash
netcore-control-room-operator \
  --api http://10.0.1.25:9010 \
  packet-data

netcore-control-room-operator \
  --api http://10.0.1.25:9010 \
  packet-data --node SRV-M_TBS-01
```

### 1.5 Legacy-WAP über SDS Type 4

Unterstützt werden zwei Transportvarianten:

- PID `0x04`: WAP/WDP-Nutzlast direkt in SDS Type 4;
- PID `0x84`: WAP über einen minimalen SDS-TL-TRANSFER-Header.

Der Generator:

- erzeugt eine kompakte WML-1.1-Karte;
- XML-escaped Titel, Text und URL;
- begrenzt den vollständigen Type-4-Payload auf 255 Byte;
- kürzt ausschließlich den Nachrichtentext, niemals XML-Tags oder die URL;
- verwendet den bereits vorhandenen Raw-SDS-Type4-Pfad, ohne den PID doppelt einzufügen.

Control-Room-API:

```text
POST /api/nodes/{node_id}/commands/legacy-wap
```

Beispiel:

```bash
curl -X POST \
  http://127.0.0.1:9010/api/nodes/SRV-M_TBS-01/commands/legacy-wap \
  -H 'Content-Type: application/json' \
  -d '{
    "operator_id":"jan",
    "dest_issi":4010001,
    "source_issi":4010001,
    "title":"NetCore",
    "message":"Statusseite öffnen",
    "url":"http://10.0.0.1:9200/",
    "transport":"wdp"
  }'
```

Operator-CLI:

```bash
netcore-control-room-operator \
  --api http://10.0.1.25:9010 \
  legacy-wap \
  --node SRV-M_TBS-01 \
  --dest-issi 4010002 \
  --title NetCore \
  --message 'Statusseite öffnen' \
  --url http://10.0.0.1:9200/ \
  --transport wdp
```

Für PID `0x84`:

```bash
--transport sds_tl --message-reference 1
```

Hinweis: Das ist ein Legacy-/Kompatibilitätspfad für entsprechend konfigurierte Endgeräte. Ob ein bestimmtes Terminal rohe WML-Nutzlast, SDS-TL oder einen herstellerspezifischen WAP-Push-Envelope erwartet, hängt von Modell, Firmware und Codeplug ab. Der normale browserbasierte Paketdatenpfad über SNDCP/WTP/WSP bleibt davon unabhängig.

## 2. Saubere Installation als vollständiger Ersatzstand

### 2.1 Dienst stoppen und Sicherung erstellen

```bash
cd ~
sudo systemctl stop tetra.service

cp -a netcore-tetra \
  "netcore-tetra.backup-$(date +%F-%H%M%S)"
```

Konfiguration zusätzlich separat sichern:

```bash
cp -a ~/netcore-tetra/config.toml \
  "~/config.toml.pre-multi-pdch-$(date +%F-%H%M%S)"

cp -a ~/netcore-tetra/config.toml.fallback \
  "~/config.toml.fallback.pre-multi-pdch-$(date +%F-%H%M%S)"
```

### 2.2 Alten Quellbaum entfernen und ZIP entpacken

```bash
cd ~
rm -rf netcore-tetra
unzip ~/Downloads/netcore-tetra-multi-pdch-dashboard-legacy-wap-2026-07-21.zip
mv netcore-tetra-wap netcore-tetra
cd ~/netcore-tetra
```

Eigene produktive Frequenz-, SDR- und Netzwerkparameter anschließend aus der Sicherung kontrolliert zurückübernehmen. Nicht blind eine alte komplette TOML über die neue Datei kopieren, weil sonst die neuen Multi-PDCH-Felder fehlen können.

### 2.3 Alte Buildartefakte vollständig entfernen

```bash
cd ~/netcore-tetra
rm -rf target
cargo clean
```

### 2.4 Prüfungen

```bash
cargo check -p tetra-core
cargo check -p tetra-config
cargo check -p tetra-entities --features runtime
cargo check -p netcore-control-room
cargo check -p netcore-control-room-operator

cargo test -p tetra-core timeslot_alloc
cargo test -p tetra-entities legacy_wap --features runtime
cargo test -p tetra-entities sndcp --features runtime
```

### 2.5 Vollständiger Server-Build

```bash
cargo build --release \
  -p bluestation-bs \
  -p netcore-control-room \
  -p netcore-control-room-operator \
  --features bluestation-bs/asterisk
```

### 2.6 Native Control-Room-UI separat bauen

Die native UI ist absichtlich kein Mitglied des Haupt-Workspaces:

```bash
rm -rf system-backend/control-room/ui/target

cargo build --release \
  --manifest-path system-backend/control-room/ui/Cargo.toml
```

### 2.7 Dienst starten

```bash
sudo systemctl start tetra.service

sudo journalctl \
  -u tetra.service \
  -n 500 \
  --no-pager \
  | grep -iE 'SNDCP|PDP|PDCH|packet gateway|legacy WAP|SDS|error|panic'
```

## 3. On-Air-Testplan

1. Single Carrier starten und prüfen, dass mindestens ein Sprachslot frei bleibt.
2. Ein Endgerät PDP aktivieren lassen und zugewiesenen PDCH/Carrier im Dashboard prüfen.
3. Zweites Endgerät parallel aktivieren; beide ISSIs müssen unterschiedliche logische Slots erhalten.
4. Mehrere NSAPIs derselben ISSI aktivieren; diese müssen denselben PDCH teilen.
5. Einen Sprachruf bei zwei aktiven PDCHs aufbauen; keine Timeslot-Kollision darf auftreten.
6. Dual Carrier aktivieren; neue Paketdaten-Bearer sollen bevorzugt auf Carrier 2 erscheinen.
7. READY-Timer, SN-END-OF-DATA und Deregistrierung prüfen; jeweiliger Slot muss freigegeben werden.
8. Downlink an einen STANDBY-Kontext schicken; Queue und Paging im Dashboard beobachten.
9. Legacy-WAP mit PID 0x04 senden.
10. Bei geeignetem Endgerät PID 0x84 testen.
11. Control-Room-API und native UI mit mehreren Nodes prüfen.
12. Lasttest bis zur konfigurierten Bearergrenze durchführen; weitere Anforderungen müssen sauber mit Ressourcenmangel abgelehnt werden.

## 4. Bekannte Grenzen

- Jeder dynamische PDCH verwendet weiterhin genau einen Timeslot.
- Keine harte Paketdaten-Präemption eines bereits laufenden Bearers durch CMCE; Schutz erfolgt über reservierten Voice-Headroom.
- Enhanced-PDCH/TEDS, QAM und echte Multi-Slot-Bündelung pro Endgerät sind nicht enthalten.
- Legacy-WAP-Endgeräte unterscheiden sich stark; PID und WML-Payload sind standardnah, aber nicht jede Firmware öffnet eine empfangene Karte automatisch.
- Die neue Telemetrie ist eine Live-Momentaufnahme und kein dauerhaftes Accounting-System.

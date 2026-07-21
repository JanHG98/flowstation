# NetCore-Tetra – WAP über SNDCP

Stand: 21.07.2026

## Ziel

Diese Erweiterung stellt einen lokalen, terminaltauglichen WAP-Statusdienst über TETRA-Paketdaten bereit. Sie ist als Clean-room-Implementierung aus dem vorhandenen Port-Vertrag, den eingefrorenen Funk-/Protokollvektoren und den öffentlich spezifizierten Protokollformaten entstanden. Es wurde kein Nexus-Quelltext übernommen.

Der Dienst ist bewusst **kein allgemeiner Internetzugang**. Das Funkgerät erhält eine IPv4-Adresse und kann den lokalen NetCore-Endpunkt aufrufen:

- Gateway/Server: `10.0.0.1`
- UDP-Port: `9200`
- Startseite: `/`
- XHTML: `/status.xhtml`
- WML: `/status.wml`

## Eingebaute Funktionen

### SNDCP/PDP

- SN-ACTIVATE PDP CONTEXT DEMAND/ACCEPT
- statische und dynamische IPv4-Zuweisung
- dynamischer Pool, standardmäßig `10.0.0.2` bis `10.0.0.254`
- Motorola-/Dimetra-CHAP-Success bleibt erhalten
- saubere ACTIVATE-REJECT-Antworten für ungültige, doppelte oder nicht erlaubte Adressen sowie einen leeren Pool
- feste MTU 576 des interoperablen WAP-Profils
- PDP-Zustände `STANDBY` und `READY`
- SN-DATA TRANSMIT REQUEST/RESPONSE
- SN-DEACTIVATE PDP CONTEXT DEMAND/ACCEPT
- SN-END OF DATA mit Rückkehr zum Common Control Channel

### PDCH

- ein einzelner Paketdatenkanal auf Hauptcarrier TS2
- Channel Allocation `Replace`, UL/DL `Both`
- TS2 wird im gemeinsamen NetCore-Timeslot-Allocator als `Sndcp` reserviert
- ein parallel startender Sprachruf kann diesen Zeitschlitz nicht doppelt belegen
- ist TS2 bereits belegt, wird SN-DATA TRANSMIT mit Ursache `System resources not available` abgelehnt
- bei SN-END OF DATA oder PDP-Deaktivierung wird TS2 wieder freigegeben

### IP/WAP

- IPv4 ohne Optionen
- UDP mit IPv4-konform optionaler Prüfsumme `0`
- keine Fragmentierung
- SN-UNITDATA und SN-DATA als Uplink
- MLE-Routing für unacknowledged TL-UNITDATA bis zum SNDCP-Entity
- SN-UNITDATA als Downlink
- WTP Invoke, Result, ACK und ABORT
- WSP Connect/Connect-Reply
- WSP Resume/Resume-OK
- WSP GET mit ein- oder mehrbyteigem UIntVar
- absolute und relative WAP-URIs
- einfache Klartext-Anfrage `GET /…`
- Openwave-taugliche XHTML-/WML-Seiten mit harten Größenlimits

### Live-Statusseite

Die Seite enthält in kompakter Form:

- Betriebszustand
- NetCore-Version
- registrierte Funkgeräte
- aktuell affiliierte Gruppen
- aktive Rufkanäle
- wartende Live-SDS
- Uptime
- letzte WAP-/PDP-Aktivität

## Konfiguration

```toml
[cell_info]
sndcp_service = true
advanced_link = true

[cell_info.wap_ip]
enabled = true
address = "10.0.0.1"
port = 9200
response_ttl = 32
dynamic_pool_prefix = "10.0.0"
dynamic_pool_first_host = 2
dynamic_pool_last_host = 254
allow_static_ipv4 = true
accept_empty_probe = true
accept_root_path = true
accept_status_path = true
accept_status_wml_path = true
max_request_payload_bytes = 1024
assume_pdch_ready_after_data_transmit = false
```

Die Konfigurationsprüfung verhindert unter anderem:

- WAP ohne SNDCP-Ankündigung
- SNDCP-Ankündigung ohne aktivierten WAP-Endpunkt
- WAP ohne Advanced Link
- Port oder TTL `0`
- ungültige Poolgrenzen
- eine Gateway-Adresse innerhalb des dynamischen Client-Pools
- Tippfehler durch unbekannte Felder im WAP-Block

## Erwarteter Ablauf im Log

```text
SYSINFO advertising: sndcp_service=true advanced_link=true ...
SNDCP: PDP context accepted ISSI=... NSAPI=... IPv4=... CHAP=true
SNDCP: <- type=6 ...
SNDCP: <- type=4 ...
```

Bei einem belegten TS2:

```text
SNDCP: TS2 unavailable for packet data ISSI=... NSAPI=...
```

## Tests

```bash
cargo test -p tetra-core timeslot_alloc
cargo test -p tetra-entities sndcp --features runtime
cargo test -p tetra-config
```

Danach vollständiger Release-Build:

```bash
rm -rf target
cargo clean
cargo build --release \
  -p bluestation-bs \
  -p netcore-control-room \
  -p netcore-control-room-operator \
  --features bluestation-bs/asterisk
```

## Bewusste Grenzen dieser Version

- nur lokaler WAP-Statusdienst; kein NAT, Routing oder Internet-Gateway
- nur IPv4/UDP; kein TCP/HTTP-Kompatibilitätsendpunkt
- keine IP-Fragmentierung
- keine Header-/Datenkompression
- keine AIE-Verschlüsselung für Paketdaten
- CHAP dient hier nur der Motorola-/Dimetra-Kompatibilität; der Hash wird nicht als eigene Zugangskontrolle validiert
- keine zusätzliche WAP-Berechtigungsebene jenseits der bestehenden Netzzulassung/Whitelist
- genau ein aktiver PDCH auf Hauptcarrier TS2
- noch kein PDCH auf dem Secondary Carrier
- keine vollständige Nexus-SNDCP-Suite mit RECONNECT, PAGE, MODIFY und DATA PRIORITY
- kein eigener Inaktivitäts-Timer für abgebrochene PDCH-Sitzungen; ohne END/Deaktivierung bleibt TS2 bis zur Neuregistrierung oder zum Neustart reserviert
- noch kein realer On-Air-Nachweis mit dem Zielgerät in dieser Arbeitsumgebung

## Korrigierte Port-Vertragsstellen

Im mitgelieferten `Docs/wap-port-spec.md` waren zwei interne Widersprüche:

1. SN-UNITDATA besitzt `type + NSAPI + PCOMP + DCOMP`, also **16 Bit** Header. Der korrekte Bytebeginn für NSAPI 2 ohne Kompression ist `42 00 45 …`, nicht `42 45 …`.
2. Der angegebene 70-Bit-PDP-ACCEPT-Bitstring war korrekt, aber seine Byteumrechnung nicht. Der korrekte gepaddete Vektor lautet `02 90 8E 82 80 00 38 80 10`.

Beide Stellen und die zugehörigen Tests wurden korrigiert.

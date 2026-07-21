# NetCore-TETRA WAP/IP über SNDCP

Diese Implementierung stellt eine kleine WAP-Statusseite direkt aus der TETRA-Basisstation bereit. Sie benötigt keinen externen Webserver: Das Funkgerät baut einen SNDCP-PDP-Kontext auf, erhält eine private IPv4-Adresse und spricht anschließend WTP/WSP über UDP.

## Unterstützter Ablauf

1. `SN-ACTIVATE PDP CONTEXT DEMAND`
2. `SN-ACTIVATE PDP CONTEXT ACCEPT`, bei Motorola-PCO inklusive CHAP-Success
3. `SN-DATA TRANSMIT REQUEST`
4. Zuweisung des Paketdatenkanals auf Hauptträger TS2
5. IPv4/UDP über `SN-UNITDATA` oder `SN-DATA`
6. WTP Invoke mit WSP Connect, Resume oder GET
7. Antwort als WTP Result in `SN-UNITDATA`
8. Rückkehr auf den Steuerkanal über `SN-END OF DATA` beziehungsweise Deaktivierung

WTP ACK und ABORT werden protokollgerecht nicht beantwortet. IPv4-Fragmente, IP-Kompression, SNDCP-Kompression, IPv6 und TCP werden nicht verarbeitet.

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
max_request_payload_bytes = 1024
title = "NetCore-TETRA"
```

`address` ist die virtuelle IPv4-Adresse der Basisstation. Der dynamische Pool wird aus dem dreiteiligen Präfix und dem Hostbereich gebildet. Eine ISSI erhält innerhalb einer Laufzeit möglichst wieder dieselbe Adresse.

## Bereitgestellte Pfade

- `/`
- `/status`
- `/status.xhtml`
- `/status.wml`

Der optionale Query-Parameter `s=0`, `s=1` oder `s=2` wechselt zwischen den kompakten Statussektoren. Die Ausgabe wird an die kleinen Openwave-Browserbudgets angepasst.

## Funkkanal und Parallelbetrieb

Der WAP-Bearer verwendet ausschließlich **TS2 des Hauptträgers**. Die Zuweisung erfolgt erst nach einer Datenübertragungsanforderung. Ein aktiver Sprachkanal auf TS2 hat Vorrang; in diesem Fall wird keine neue Paketdatenzuweisung aktiviert. Adresslose MAC-DATA-, MAC-FRAG-UL- und MAC-END-UL-PDUs werden über den gespeicherten PDCH-Besitzer der richtigen ISSI zugeordnet.

Eine Paketdatenzuweisung wird durch `QuitAndGo` entfernt. Zusätzlich existiert ein Fail-safe-Ablauf, damit ein verschwundenes Funkgerät TS2 nicht dauerhaft belegt.

## Aktuelle Grenzen

- ein aktiver PDCH-Besitzer gleichzeitig
- ein IPv4-PDP-Kontext pro NSAPI, maximal vier Kontexte pro ISSI
- ausgehandeltes MTU-Profil: 576 Byte
- WTP/WSP nur über UDP, Standardport 9200
- keine Weiterleitung in ein externes IP-Netz
- keine allgemeine HTTP/TCP-Kompatibilitätsschnittstelle
- keine persistente Wiederherstellung von PDP-Kontexten nach Neustart

## Prüfung nach Installation

```bash
cd ~/netcore-tetra
rm -rf target
cargo build --release --features asterisk
sudo systemctl restart tetra.service
sudo journalctl -u tetra.service -f | grep --line-buffered -iE 'SNDCP|WAP|PDCH|CHAP'
```

Erwartete Logfolge:

```text
SNDCP/WAP: PDP context accepted ...
SNDCP/WAP: ... entered READY on TS2
packet-data PDCH assigned ... on TS2
SNDCP/WAP: -> ... IPv4/UDP response ... octets
packet-data PDCH released ...
```

## Herkunft und Lizenzgrenze

Die Implementierung wurde neu anhand des öffentlichen Protokollverhaltens, der ETSI-/OMA-Wireformate und der Bytevektoren in `Docs/wap-port-spec.md` erstellt. Es wurde kein Quelltext aus dem unter PolyForm-Noncommercial stehenden WAP-Teil von `nexus-bs` übernommen.

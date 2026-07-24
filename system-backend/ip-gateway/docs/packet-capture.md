# Packet Capture

Captures werden direkt am Übergang zwischen Packet Core und TUN erzeugt. Dadurch enthalten sie die vollständigen IPv4-N-PDUs vor beziehungsweise nach Linux-Routing und ohne künstliche Ethernet-Header.

## Format

- PCAP Classic
- Link-Type `DLT_RAW` (101)
- Mikrosekunden-Zeitstempel
- konfigurierbare Snaplen
- Größenlimit pro Datei

## Filter

Ein Capture kann filtern nach:

- `uplink`, `downlink` oder `both`
- IPv4-Host
- `tcp`, `udp` oder `icmp`
- Quell- oder Zielport

Der Download erfolgt über die WebUI oder:

```text
GET /api/v1/captures/{id}/download
```

Bei Neustart werden aktive Captures sauber als gestoppt markiert. Dateien werden nicht automatisch gelöscht.

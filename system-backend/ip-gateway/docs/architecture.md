# Architektur

## Zuständigkeiten

Der Packet Core bleibt Eigentümer der PDP-/NSAPI-State-Machine, Fragmentierung, Reassembly, Priorität, Mobility Anchors und Downlink-Queue. Der IP Gateway kennt keine Air-PDU und weist keine NSAPI zu. Er verwendet die vom Packet Core bereitgestellte Zuordnung `IPv4 → ISSI/NSAPI`.

## Uplink

1. Der Packet Core stellt eine vollständig reassemblierte N-PDU in `/api/v1/npdu-outbox` bereit.
2. Der IP Gateway prüft IPv4-Header und Größenlimit.
3. Flow-Zähler und aktive Captures werden aktualisiert.
4. Das Paket wird ohne zusätzliche Ethernet-Kapselung in `ntc-tun0` geschrieben.
5. Erst nach erfolgreichem TUN-Write wird der Outbox-Eintrag gelöscht.
6. Der Linux-Kernel entscheidet anhand Routing, lokaler Sockets und nftables über den weiteren Weg.

## Downlink

1. Ein lokaler oder weitergeleiteter IPv4-Flow erzeugt ein Paket mit Zieladresse aus dem TETRA-Pool.
2. Die verbundene Route liefert das Paket an `ntc-tun0`.
3. Der IP Gateway liest die N-PDU, sucht den aktuellen PDP-Kontext und übergibt sie an `/api/v1/downlink`.
4. Bei kurzzeitigen HTTP-Fehlern wird bis zu fünfmal lokal erneut versucht.
5. Fragmentierung, Page/Wake und Zustellung bleiben Aufgabe des Packet Core.

## Kernelmodell

Der Dienst besitzt ausschließlich zwei nftables-Tabellen:

```text
inet netcore_ip_gateway
ip   netcore_ip_gateway_nat
```

Andere Tabellen werden nicht verändert. Benutzerdefinierte Routen werden mit `ip route replace` angewendet. Bei Löschung oder Änderung entfernt der Reconciler die zuvor bekannte Route.

## Persistenz

Die Betriebsdatenbank enthält Regeln, DNS-Einträge, Blocklisten, Capture-Metadaten, Flow-Historie und Events. PCAP-Dateien liegen separat im Capture-Verzeichnis. Pakettransport und Downlink-Retry-Queue bleiben bewusst flüchtig; langlebige Downlink-Zustellung gehört in den Packet Core.

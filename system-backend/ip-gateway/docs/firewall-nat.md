# Firewall und NAT

## Baseline

Die generierte Forward-Chain akzeptiert zuerst `established,related`. Operator-Blocklisten werden vor allgemeinen Freigaben ausgewertet. Danach folgen benutzerdefinierte Regeln nach aufsteigender Priorität.

Im Open-Lab-Beispiel ist allgemeiner ausgehender IPv4-Verkehr erlaubt:

```toml
[firewall]
allow_general_internet = true
```

Für einen restriktiven Testserverbetrieb auf `false` setzen und gezielte Forward-Regeln anlegen.

## Lokale Dienste

Vom TUN-Interface zum Gateway werden standardmäßig DNS, HTTP/WAP, UDP-Echo und optional ICMP zugelassen. Das Management-WebUI auf Port 8170 ist nicht automatisch aus dem TETRA-Paketdatennetz freigegeben.

## NAT-Typen

- `masquerade`: dynamische Quelladressübersetzung am Egress-Interface
- `snat`: feste Quelladresse, optional mit Port
- `dnat`: feste Zieladresse, optional mit Port

Regeln werden vollständig aus der Datenbank gerendert. Freitext-nftables wird absichtlich nicht akzeptiert; die API validiert CIDRs, Protokolle, Ports und Interface-Namen.

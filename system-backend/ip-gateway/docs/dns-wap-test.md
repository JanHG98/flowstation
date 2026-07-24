# DNS, WAP und Testdienste

## DNS

Der eingebaute UDP-DNS-Server beantwortet statische A-Records direkt und leitet unbekannte Anfragen an den konfigurierten Upstream weiter. Built-in-Einträge:

```text
netcore.test
wap.netcore.test
test.netcore.test
```

Alle zeigen auf die Gateway-Adresse des TUN-Netzes.

## WAP

Der Testserver stellt eine bewusst einfache WML-1.1-Seite bereit:

```text
http://wap.netcore.test:8088/wap/
http://wap.netcore.test:8088/wap/status.wml
```

Damit kann der bereits vorhandene Legacy-WAP-/SNDCP-Pfad ohne externen Webserver geprüft werden.

## Weitere Tests

```text
GET  http://test.netcore.test:8088/test/echo
GET  http://test.netcore.test:8088/test/info
UDP  test.netcore.test:7007
```

`/test/info` liefert die beobachtete Peer-Adresse und den aktuellen Gatewaystatus als JSON.

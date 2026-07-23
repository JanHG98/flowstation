# SWMI Mobility 1 – Paket D: Node Gateway

## Ziel

Dieses Paket führt den ersten tatsächlich als LXC laufenden System-Backend-Dienst ein:

```text
system-backend/node-gateway/
```

Der Gateway bildet den zentralen Einstiegspunkt zwischen TBS-Edge und den späteren Core-Diensten.

## Sicherheitsentscheidung für die Testumgebung

Auf ausdrücklichen Wunsch wird diese erste LXC-Version ohne Tokens betrieben. Der Modus heißt `open_lab` und beinhaltet weder Benutzeranmeldung noch TLS oder Client-Zertifikate.

Die Entscheidung ist in folgenden Stellen verbindlich sichtbar:

- Konfigurationsschema
- Beispielkonfiguration
- Startlogs
- HTTP-Header
- WebSocket-Handshake
- WebUI-Warnbanner
- REST-Status
- OpenAPI-Dokument
- systemd-Beschreibung
- Service-Matrix

Andere Security-Modi werden in dieser Version abgewiesen. Dadurch kann ein versehentlich gesetzter Wert wie `token` nicht den Eindruck erwecken, Tokenprüfung sei bereits implementiert.

## TBS-Kompatibilität

Der vorhandene TBS-Worker verwendet zunächst weiterhin das Node-Protokoll:

```text
netcore-control-room-node-v1
```

Der Node Gateway handelt dieses Protokoll aus und setzt den bestehenden Kompatibilitätsmarker `x-netcore-control-room: 1`. Dadurch kann die Basisstation ohne sofortigen Umbau des TBS-Workers vom bisherigen direkten Control-Room-Endpunkt auf den Gateway umgestellt werden.

Zusätzlich wird gesetzt:

```text
x-netcore-node-gateway: 1
x-netcore-security-mode: open-lab
```

## Datenpfade

```text
TBS
 └─ WS /ws/node
      └─ Node Gateway
           ├─ WebUI / REST API
           ├─ In-Memory Node Registry
           ├─ Ereignis-History
           └─ WS /ws/backend
                 └─ spätere mobility-core/call-control/sds-router Dienste
```

## Unterstützte Node-Nachrichten

- Hello
- Heartbeat
- Telemetry
- ControlAck
- ControlResponse
- Error

Der Gateway sendet:

- HelloAck
- Ping
- Command

## Management

Die integrierte WebUI zeigt Nodes, Zellen, Carrier, Versionen, Capabilities, Heartbeats, Telemetrie- und Response-Zähler sowie die Ereignis-History. Ping und Disconnect sind direkt möglich.

Die API akzeptiert außerdem den bestehenden typisierten `ControlCommand`-Typ.

## Noch nicht enthalten

- persistente Datenhaltung
- gesicherter Produktivmodus
- fachliche Mobility-Entscheidungen
- Teilnehmer- oder Gruppenautorität
- Media-Transport
- HA/Cluster

## Nächster Schritt

Als nächstes kann `mobility-core` als zweiter LXC-Dienst den offenen Backend-WebSocket abonnieren und die in Paket C vorbereiteten Mobility Context Transfers zentral koordinieren.

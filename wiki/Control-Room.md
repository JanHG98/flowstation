# NetCore Control Room

NetCore Control Room ist die optionale zentrale Leitstelle für eine oder mehrere Basisstationen. Sie besteht aus einem headless Core, einem Operator-Werkzeug und einer nativen UI.

## Komponenten

- `netcore-control-room` – Server/Core
- `netcore-control-room-operator` – CLI und Operator-Dashboard
- `system-backend/control-room/ui` – native Bedienoberfläche

## Basisstationskonfiguration

```toml
[control_room]
enabled = true
host = "<CONTROL-ROOM-IP>"
port = 9010
use_tls = false
endpoint_path = "/node"
node_id = "BTS-01"
station_name = "Basisstation 01"
site = "Standort A"
token = "<NODE-TOKEN>"
```

Alternativ kann explizites Basic Auth mit Benutzername und Passwort verwendet werden. Bei aktivierter Leitstellen-Authentifizierung muss das Node-Token mit der Serverkonfiguration übereinstimmen.

## Rollen

- **Viewer** – lesen
- **Operator** – lesen und Befehle ausführen
- **Admin** – Benutzer- und Dienstverwaltung

Die Basisstation selbst authentifiziert sich als Node und nicht als menschlicher Operator.

## Build

Auf einem Leitstellen-LXC nur die benötigten Pakete bauen. Dadurch werden unnötige SDR- und Audio-Abhängigkeiten vermieden:

```bash
cargo clean
rm -rf target
cargo build --release -p netcore-control-room
cargo build --release -p netcore-control-room-operator
```

## Startbeispiel

```bash
./target/release/netcore-control-room --bind 127.0.0.1:9010
```

Operator-Dashboard:

```bash
./target/release/netcore-control-room-operator \
  --api http://<CONTROL-ROOM-IP>:9010 dashboard
```

## Betrieb

- Leitstelle und Basisstation über ein Managementnetz verbinden.
- Node-Token und Operator-Anmeldungen getrennt halten.
- Reverse Proxy/TLS verwenden, sobald die Verbindung das vertrauenswürdige LAN verlässt.
- Reconnects sind normal; dauerhafte Auth- oder TLS-Fehler hingegen nicht.
- Nicht gleichzeitig alte und neue Command-Worker aktivieren, wenn dadurch doppelte Antworten entstehen könnten.

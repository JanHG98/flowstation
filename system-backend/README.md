# NetCore-Tetra System Backend

Dieser Ordner enthält alle Dienste, die später unabhängig von der TBS als LXC, VM oder zentraler Backend-Prozess betrieben werden.

## Grundregeln

- Jeder deploybare Dienst besitzt einen eigenen Unterordner.
- Funknahe Echtzeitkomponenten bleiben außerhalb von `system-backend/`.
- Gemeinsamer Backend-Code liegt unter `shared/`.
- ZIP-Lieferungen behalten den vollständigen Pfad `system-backend/<dienst>/...` bei.
- **Jeder eigenständig laufende Container oder jede VM besitzt eine eigene WebUI zur Verwaltung.**
- Die WebUI wird vom jeweiligen Dienst selbst ausgeliefert; dafür wird kein zusätzlicher Frontend-Container benötigt.
- Ein Ausfall der WebUI darf niemals die fachliche Runtime des Dienstes stoppen.
- Der Control Room verlinkt und aggregiert die Service-WebUIs, ersetzt sie aber nicht.

## Verbindlicher WebUI-Standard

Die gemeinsame Vorgabe steht in:

```text
Docs/BACKEND_WEBUI_STANDARD.md
```

Die dienstspezifischen Verwaltungsbereiche stehen in:

```text
Docs/BACKEND_WEBUI_SERVICE_MATRIX.md
```

Gemeinsame UI-Bausteine werden zukünftig unter folgendem Pfad entwickelt:

```text
system-backend/shared/web-ui/
```

## Standardzugriff

Langfristig verwenden neue Dienste mit eigener LXC-IP einheitlich:

```text
https://<LXC-IP>:8443/
```

Die bisher umgesetzten Dienste sind ausdrücklich dokumentierte Ausnahmen für die isolierte Testumgebung und verwenden je Dienst einen eigenen HTTP-Port im offenen Labormodus. Die verbindliche Zuordnung steht in `services.toml`; der Recorder verwendet Port 8140, der SDS Router Port 8150, der Packet Core Port 8160 und der IP Gateway Port 8170.

## Bereits deploybare Dienste

Bereits deploybar sind:

- `node-gateway/` – TBS- und Backend-Vermittlung, Port 8080
- `mobility-core/` – Teilnehmerlage und MM-Context-Transfer, Port 8090
- `subscriber-core/` – Teilnehmerprofile und Admission, Port 8100
- `group-core/` – Gruppen, Mitgliedschaften und DGNA, Port 8110
- `call-control/` – logische Calls, Floor Control und Restore, Port 8120
- `media-switch/` – Routing gepackter TETRA-Sprachframes, Port 8130
- `recorder/` – passive Aufnahme, Integrität, Retention und Export, Port 8140
- `sds-router/` – SDS-/Statusvermittlung, Store-and-forward und Anwendungsrouten, Port 8150
- `packet-core/` – PDP-/NSAPI-State-Machine, Mobility Anchoring, Fragmentierung und Flow Control, Port 8160
- `ip-gateway/` – TUN, Routing, NAT, Firewall, DNS, WAP/Testdienste und PCAP, Port 8170

Alle enthalten Rust-Runtime, REST-API, eigene WebUI, systemd-Unit und Installationsskripte. In der aktuellen Teststufe laufen sie bewusst im deutlich markierten `open_lab`-Modus ohne Tokens, Benutzeranmeldung oder TLS.

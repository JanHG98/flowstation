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

Die ersten drei Dienste sind ausdrücklich dokumentierte Ausnahmen für die isolierte Testumgebung: Node Gateway auf Port 8080, Mobility Core auf Port 8090 und Subscriber Core auf Port 8100, jeweils per HTTP im offenen Labormodus. Abweichungen werden im jeweiligen README und in `services.toml` festgehalten.

## Bereits deploybarer Dienst

Bereits deploybar sind:

- `node-gateway/` als zentrale TBS- und Backend-Verbindungsstelle auf Port 8080,
- `mobility-core/` als zentrale Teilnehmerlage und MM-Context-Transfer-Steuerung auf Port 8090.

Beide Dienste enthalten Rust-Runtime, REST-API, eigene WebUI, systemd-Unit und Installationsskripte. In der ersten Teststufe laufen sie bewusst im deutlich markierten `open_lab`-Modus ohne Tokens, Benutzeranmeldung oder TLS.

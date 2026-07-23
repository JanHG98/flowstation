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

Der erste Node Gateway ist eine ausdrücklich dokumentierte Ausnahme: Er läuft für die frühe isolierte Testumgebung unter `http://<LXC-IP>:8080/` im offenen Labormodus. Abweichungen werden im jeweiligen README und in `services.toml` festgehalten.

## Bereits deploybarer Dienst

`node-gateway/` ist der erste tatsächlich implementierte LXC-Dienst. Er enthält Rust-Runtime, TBS- und Backend-WebSockets, REST-API, WebUI, systemd-Unit und Installationsskripte. In der ersten Teststufe läuft er bewusst im deutlich markierten `open_lab`-Modus ohne Tokens oder Benutzeranmeldung.

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

## Standardzugriff im Testumfeld

Da jeder LXC eine eigene IP erhält, kann für neue Dienste einheitlich derselbe Management-Port verwendet werden:

```text
https://<LXC-IP>:8443/
```

Abweichungen für bereits bestehende Dienste sind zulässig und werden im jeweiligen README dokumentiert.

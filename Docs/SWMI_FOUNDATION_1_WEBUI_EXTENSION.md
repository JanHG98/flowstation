# SWMI Foundation 1 – WebUI-Erweiterung

## Entscheidung

Jeder später eigenständig laufende Backend-Container erhält eine eigene WebUI zur Verwaltung. Diese Entscheidung ist ab Paket A verbindlicher Bestandteil der Architektur.

## Auswirkungen auf Foundation 1

- Die Backend-Service-README-Dateien beschreiben nun jeweils ihre vorgesehenen Verwaltungsansichten und kritischen Aktionen.
- `Docs/BACKEND_WEBUI_STANDARD.md` definiert gemeinsame Endpunkte, RBAC, Audit, Sicherheit und Definition of Done.
- `Docs/BACKEND_WEBUI_SERVICE_MATRIX.md` ordnet jedem Dienst seine fachlichen UI-Bereiche zu.
- `system-backend/services.toml` hält die Pflicht maschinenlesbar fest.
- `system-backend/shared/web-ui/` ist für gemeinsame Komponenten reserviert.
- TLMC und TLPD bleiben Teil der TBS und erhalten keinen eigenen Container; ihre Diagnosezustände werden später über TBS und Node Gateway sichtbar gemacht.

## Kein zusätzlicher Container pro Oberfläche

Die WebUI wird vom jeweiligen Dienst selbst ausgeliefert. Ein Dienst benötigt daher nicht noch einen separaten Frontend-LXC.

## Zielbild

```text
Browser
  ├── Node Gateway WebUI
  ├── Subscriber Core WebUI
  ├── Mobility Core WebUI
  ├── Call Control WebUI
  └── weitere Service-WebUIs

Control Room
  └── aggregiert Status und verlinkt auf die einzelnen Oberflächen
```

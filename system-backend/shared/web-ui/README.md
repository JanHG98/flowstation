# NetCore Shared WebUI

Dieser Ordner ist für die gemeinsamen Verwaltungsoberflächen-Bausteine aller System-Backend-Dienste vorgesehen.

## Ziel

Jeder LXC beziehungsweise jede VM besitzt eine eigene WebUI, ohne dass 18 völlig unterschiedliche Oberflächen entstehen. Gemeinsame Komponenten sorgen für ein einheitliches Bedienkonzept, während jeder Dienst seine fachlichen Seiten selbst bereitstellt.

## Geplante Module

```text
web-ui/
├── shell/          # Layout, Navigation, Kopfzeile und Service-Menü
├── auth/           # Login, Rollen, Session und Break-Glass-Anmeldung
├── api-client/     # typisierter Zugriff auf /api/v1
├── components/     # Tabellen, Formulare, Dialoge und Statuskarten
├── health/         # Liveness, Readiness und Dependency-Ansichten
├── audit/          # Änderungsprotokoll und Bedienerinformationen
├── i18n/           # Deutsch und Englisch
└── assets/         # gemeinsame statische Ressourcen
```

## Produktionsprinzip

Die fertigen UI-Assets werden in den jeweiligen Dienst eingebettet oder gemeinsam mit ihm installiert. In Produktion ist kein Node.js-Entwicklungsserver und kein separater Frontend-Container erforderlich.

## Pflichtseiten jedes Dienstes

- Übersicht
- Fachliche Verwaltung
- Zustand und Abhängigkeiten
- Ereignisse beziehungsweise Audit
- Konfiguration
- Wartung
- API-Dokumentation
- Version und Buildinformationen

Weitere verbindliche Anforderungen stehen in `Docs/BACKEND_WEBUI_STANDARD.md`.

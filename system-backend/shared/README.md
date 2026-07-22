# Shared Backend Components

## Zweck

Dieser Ordner enthält gemeinsam genutzte Bibliotheken und Protokolle für mehrere Backend-Dienste.

## Geplante Inhalte

- `edge-protocol/` für die versionierte TBS-Backend-Schnittstelle
- `service-common/` für gemeinsame Service-Grundfunktionen
- `database-common/` für Datenbankhilfen
- `auth-common/` und `telemetry-common/`

## Architekturregel

Hier liegen keine eigenständig deploybaren Dienste und keine autoritativen Fachzustände.

## Gemeinsame WebUI-Bausteine

`shared/` ist kein eigenständig laufender Container und benötigt daher keine eigene Runtime-WebUI. Der Ordner stellt jedoch die gemeinsame Grundlage für alle Service-WebUIs bereit.

Geplanter Unterordner:

```text
shared/web-ui/
```

Dort werden unter anderem abgelegt:

- Layout, Navigation und Design-Tokens
- gemeinsame Login-, RBAC- und Audit-Komponenten
- API-Client und Fehlerdarstellung
- Health-, Dependency-, Log- und About-Seiten
- Tabellen-, Formular- und Bestätigungsdialoge
- gemeinsame deutsche und englische Texte

Jeder deploybare Dienst bindet diese Komponenten ein, bleibt aber unabhängig administrierbar.

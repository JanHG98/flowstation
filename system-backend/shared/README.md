# Shared Backend Components

## Zweck

Dieser Ordner enthält gemeinsam genutzte Bibliotheken, Wire-Verträge und build-freie WebUI-Bausteine für die unabhängig deploybaren Backend-Dienste. `shared/` ist selbst **kein Runtime-Dienst**, besitzt keinen autoritativen Fachzustand und benötigt deshalb keine eigene WebUI oder LXC-IP.

## Implementierte Module

```text
shared/
├── contracts/          # netcore.v1, IDs, Envelope, Health, Fehler, Audit, Schemas
├── service-common/     # Service-Identität, Open-Lab-Policy, Build-/Request-Metadaten
├── database-common/    # atomare JSON-Persistenz und Backup-Helfer
├── telemetry-common/   # Prometheus-Textformat und Label-Escaping
└── web-ui/             # CSS, ES-Module, i18n und statische Demo
```

## Architekturregeln

- Keine fachliche Datenhoheit in Shared-Crates.
- Keine direkte Abhängigkeit von TBS-Echtzeitpfaden.
- Major-Vertragsänderungen werden parallel versioniert und nicht still ausgerollt.
- Generische Envelopes transportieren keine Rohschlüssel oder unredigierten Secrets.
- Bestehende Dienste werden kontrolliert migriert; kein riskanter Big-Bang-Umbau.

## Deployment

Die LXC-übergreifende Integrationsschicht liegt unter `deploy/open-lab/`. Sie verwendet die gemeinsamen Verträge, bleibt aber bewusst außerhalb von `shared/`, weil Deployment kein Library-Code ist.

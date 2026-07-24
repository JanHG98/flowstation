# SWMI Core 1 – Package P: Shared Platform and LXC Integration

## Ergebnis

Dieses Paket schließt die erste Core-Dienstserie mit gemeinsam genutzten Verträgen, Basiskomponenten, WebUI-Bausteinen und einer inventory-gesteuerten LXC-Deployment-Schicht ab. `shared/` bleibt ausdrücklich eine Bibliothek und kein weiterer Runtime-Container.

## Gemeinsame Rust-Crates

- `netcore-contracts`: 24-Bit-ISSI/GSSI/SSI, `netcore.v1`-Envelope, Health, Problem Details, Events, Audit, Pagination, Service Descriptor und Capability-Kompatibilität.
- `netcore-service-common`: Open-Lab-Policy, Service-Identität, Buildinformationen und Request-IDs.
- `netcore-database-common`: atomare JSON-Dateischreibvorgänge, fsync und Backup-Helfer.
- `netcore-telemetry-common`: stabile Prometheus-Textausgabe und Label-Escaping.

Die Crates besitzen Unit-Tests und sind als Workspace-Member samt Lockfile-Einträgen registriert.

## Verträge und WebUI

JSON-Schemas und Beispiele dokumentieren die transportneutralen Wire-Formate. Das build-freie Shared-WebUI-Kit liefert CSS, JSON-API-Client, Statusanzeige, Bestätigungsdialog, Toasts sowie deutsche und englische Basistexte. Die bestehenden Dienst-WebUIs werden dadurch nicht zwangsweise ersetzt; die Migration kann ohne Funktionsverlust schrittweise erfolgen.

## LXC-Integration

`deploy/open-lab/netcore-deploy.py` validiert eine TOML-Inventardatei, löst Abhängigkeiten topologisch auf, rendert Service-URLs, erzeugt Servicekatalog, Portliste, Hosts-Datei und Abhängigkeitsgraph, baut ein PDF-freies Quellarchiv und kann die bestehenden Installer kontrolliert per SSH ausführen.

Der echte `apply`-Pfad ist explizit. `--dry-run` zeigt jeden SSH-/SCP-Befehl. Passwörter, Tokens, TLS-Schlüssel, KMF-Mastermaterial und Connector-Secrets werden weder abgefragt noch generiert.

## Sicherheitsstatus

Das Paket bleibt vollständig in der vereinbarten Zwischenstufe `open_lab`: keine Anmeldung, keine Management-Tokens und kein TLS. Die Deployment-Dokumentation fordert deshalb ein isoliertes Management-VLAN und verbietet öffentliche Portweiterleitungen.

## Grenze

Die gemeinsamen Crates definieren eine stabile Basis, refaktorieren aber nicht in einem riskanten Big Bang sämtliche bestehenden Dienste. Neue dienstübergreifende APIs sollen `netcore.v1` unmittelbar verwenden; bestehende private Payloads werden kontrolliert über Adapter migriert.

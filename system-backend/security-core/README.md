# NetCore-Tetra Security Core

Zentraler SwMI-Dienst für Authentisierung, Security-Class-Policy, kurzlebige DCK-Kontexte, Teilnehmer-/Gerätesperren, Alarme und Audit.

> **OPEN LAB:** Port 8180 besitzt aktuell keine Benutzerkonten, Tokens oder TLS. Nur im isolierten Testnetz betreiben.

## Funktionsumfang

- persistente Sicherheitsprofile je ISSI
- globale und teilnehmerspezifische Security-Class-Policy
- Aushandlung von Class 1, 2 und 3; Class 3 erzwingt immer Authentisierung und DCK-Workflow
- Challenge/Response-State-Machine mit TTL, Retry und Lockout
- DCK-Erzeugung und Edge-Installationsworkflow für Class 3
- Disable/Enable für Teilnehmer und Equipment, inklusive optionaler automatischer Sperre nach Fehlversuchen
- Kontext- und DCK-Widerruf
- Security-Alarme und append-orientiertes Audit
- Node-Gateway-Abhängigkeitsstatus
- eigene WebUI, REST API, OpenAPI, Metrics, Liveness und Readiness
- Crash-Recovery ohne Persistieren von Rohgeheimnissen

## Bewusste Sicherheitsgrenze

Der enthaltene Provider `lab_hmac_sha256` ist ein **Testprovider** für End-to-End-Integration. Er implementiert nicht die proprietären beziehungsweise normativen TETRA-Authentisierungsalgorithmen und ersetzt keine KMF. Das folgende KMF-Paket liefert die echten Provider-Hooks und langfristige Schlüsselverwaltung.

Normale Managementantworten enthalten niemals Seed, Challenge, erwartete Antwort oder DCK. Der getrennte Edge-Claim-Pfad darf dieses Material nur kurzlebig an den TBS-Adapter ausgeben.

## Start

```bash
cargo run -p netcore-security-core -- \
  --config system-backend/security-core/config/security-core.example.toml
```

WebUI: `http://127.0.0.1:8180/`

```bash
curl http://127.0.0.1:8180/health/live
curl http://127.0.0.1:8180/health/ready
curl http://127.0.0.1:8180/api/v1/status
```

## Betriebsmodi

- `shadow`: State Machines, Policy und Audit laufen, Entscheidungen sind beobachtend.
- `authoritative`: Edge-Aktionen für Challenge, DCK, Sperre und Widerruf werden verbindlich bereitgestellt.

## Dokumentation

- [Architektur](docs/architecture.md)
- [Authentisierungs-State-Machine](docs/auth-state-machine.md)
- [Edge-Protokoll](docs/edge-protocol.md)
- [Lab-Provider und Geheimnisse](docs/lab-provider-secret-handling.md)
- [Open Lab](docs/open-lab-mode.md)
- [LXC-Deployment](docs/lxc-deployment.md)

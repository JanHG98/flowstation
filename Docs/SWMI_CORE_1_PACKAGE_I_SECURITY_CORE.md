# SwMI Core 1 – Package I: Security Core

## Ziel

Package I führt den zentralen Security Core als eigenen LXC-Dienst ein. Der Dienst entscheidet über Security Classes, verwaltet Authentisierungszustände, DCK-Metadaten, Sperren, Alarme und Audit. Die KMF bleibt bewusst der nächste getrennte Baustein.

## Laufzeitkomponenten

```text
system-backend/security-core/
├── src/config.rs
├── src/crypto.rs
├── src/gateway.rs
├── src/http.rs
├── src/protocol.rs
├── src/state.rs
└── src/main.rs
```

## Management

- WebUI/API: TCP 8180
- Liveness: `/health/live`
- Readiness: `/health/ready`
- Metrics: `/metrics`
- OpenAPI: `/openapi.json`
- Modus: `open_lab`, ohne Token und TLS

## Sicherheitsmodell dieser Phase

Der Managementzugang bleibt entsprechend der aktuellen Projektbedingung offen. Rohgeheimnisse werden trotzdem aus WebUI, Export, Audit, normaler API und Persistenz herausgehalten. Nur der dedizierte Edge-Claim-Pfad erhält kurzlebiges Challenge-/DCK-Material.

Der HMAC-basierte Lab-Provider dient ausschließlich zur Integration der Zustandsmaschine. Normative TETRA-Algorithmen, Langzeitschlüssel, CCK/GCK/SCK und OTAR folgen in Package J – KMF.

## Abnahmekriterien

- Profile und Policy bleiben über Neustarts erhalten.
- Offene Challenges und DCK-Material bleiben **nicht** über Neustarts erhalten.
- Falsche Antworten erzeugen Retry, Lockout, Alarm und Audit; optional folgt eine verbindliche Disable-Aktion.
- Class 3 kann die Authentisierung nicht umgehen und erzeugt danach einen DCK-Installationsauftrag.
- WebUI und normale API zeigen keine Rohschlüssel oder Challenge-Werte.
- Edge-Aktionen sind nodegebunden, sequenziert, quittierbar und zeitlich begrenzt.
- `tools/check_security_core.py` läuft erfolgreich.

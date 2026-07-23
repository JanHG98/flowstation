# Offener Testmodus

Der Node Gateway wird in dieser Ausbaustufe absichtlich ohne Authentifizierung betrieben.

## Nicht vorhanden

- keine Node-Tokens
- keine Benutzerkonten
- keine Passwörter
- keine API-Schlüssel
- keine Client-Zertifikate
- kein TLS
- kein RBAC

Alle erreichbaren Clients können Statusdaten lesen. Wenn `allow_remote_management = true` gesetzt ist, können sie außerdem Nodes trennen und TBS-Kommandos auslösen.

## Verbindliche Schutzmaßnahme

Der LXC darf nur in einem isolierten Test- beziehungsweise Managementnetz erreichbar sein. Port `8080/tcp` darf nicht aus dem Internet oder aus unkontrollierten Clientnetzen erreichbar sein.

## Bewusste technische Sicherung

Die Konfiguration akzeptiert ausschließlich:

```toml
[security]
mode = "open_lab"
```

Andere Werte führen zu einem Startfehler. Dadurch wird kein noch nicht implementierter Token-Modus als vermeintlich sicherer Produktivbetrieb dargestellt.

Die spätere Sicherheitsphase ergänzt einen neuen, ausdrücklich implementierten Modus. Der offene Labormodus bleibt dann nur als bewusst aktivierbare Testoption erhalten.

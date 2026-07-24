# Operator-Profile

Operator-Profile speichern ausschließlich lokale Komfortwerte. Keine Tokens, Rohschlüssel oder Passwörter werden darin abgelegt.

```toml
[profiles.default]
api = "http://10.0.1.25:9010"
default_node = "SRV-M_TBS-01"
operator_id = "jan"
username = "jan"
```

## Open Lab

Der Server läuft aktuell mit `--no-auth`. `operator_id` dient daher nur der Nachvollziehbarkeit in Command-Audit, Incident-Journal und Schichtbuch; er ist keine verifizierte Identität.

Die in CLI und nativer UI bereits vorbereiteten Benutzer-/Passwortfelder gehören zum späteren gesicherten Profil und sind in dieser Phase nicht die Sicherheitsgrenze. Vor Produktivbetrieb müssen Server, Browser-WebUI, CLI und native UI gemeinsam auf TLS, Maschinenidentitäten und RBAC umgestellt werden.

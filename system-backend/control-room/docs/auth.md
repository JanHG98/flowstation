# Späterer gesicherter Betrieb

Die Rust-Codebasis enthält bereits Benutzer/Passwort und Rollen `viewer`, `operator`, `admin`. Diese Funktionen sind in der aktuellen Open-Lab-Phase absichtlich deaktiviert.

Aktuell verbindlich:

```toml
[auth]
enabled = false
node_token_env = ""
bootstrap_username_env = ""
bootstrap_password_env = ""
```

und im systemd-Service:

```text
--no-auth
```

Vor Produktivbetrieb müssen Authentisierung, TLS, Maschinenidentität, Audit-Schutz und Rollenmodell gemeinsam aktiviert und getestet werden. Einzelne Tokens halb einzuschalten wäre Scheinsicherheit.

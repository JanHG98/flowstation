# NetCore Control Room

Control Room Core + native Operator-UI.

Ab v5.0 nutzt der Operatorzugang klassischen Benutzername+Passwort-Login mit RBAC.

- TBS: Maschinen-Token für `/node` bleibt in der TBS `config.toml`.
- LXC: headless Core, SQLite, User/RBAC-Verwaltung.
- Windows: native UI mit Loginmaske.

Rollen:

```text
viewer    lesen
operator  lesen + Funkbefehle
admin     alles + Benutzerverwaltung
```

Siehe `CONTROL_ROOM_USER_LOGIN_RBAC_APPLY.md` und `docs/auth.md`.

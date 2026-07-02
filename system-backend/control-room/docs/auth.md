# Auth / RBAC ab v5.0

Der Control Room nutzt für Operatoren klassischen Benutzername+Passwort-Login mit RBAC.

Rollen:

```text
viewer    Lesen: Übersicht, Teilnehmer, Gruppen, Rufe, SDS, Karte, Directory
operator  viewer + Befehle: Kick, DGNA, Emergency Clear
admin     operator + Benutzerverwaltung + Service-Restart/Shutdown
```

Die TBS verwendet weiterhin einen Maschinen-Token für `/node`. Dieser Token gehört auf den LXC in `control-room.env` und in die TBS `config.toml`.

## Secrets

`/etc/netcore-control-room/control-room.env`:

```bash
NETCORE_CONTROL_ROOM_NODE_TOKEN=<node-token-fuer-TBS>
NETCORE_CONTROL_ROOM_BOOTSTRAP_USER=jan
NETCORE_CONTROL_ROOM_BOOTSTRAP_PASSWORD=<admin-passwort>
```

`/etc/netcore-control-room/control-room.toml`:

```toml
[auth]
enabled = true
allow_health_unauthenticated = true
node_token_env = "NETCORE_CONTROL_ROOM_NODE_TOKEN"
bootstrap_username_env = "NETCORE_CONTROL_ROOM_BOOTSTRAP_USER"
bootstrap_password_env = "NETCORE_CONTROL_ROOM_BOOTSTRAP_PASSWORD"
bootstrap_role = "admin"
```

## Endpoints

```text
POST /api/login
GET  /api/me
GET  /api/admin/users
POST /api/admin/users
PATCH /api/admin/users/{username}
POST /api/admin/users/{username}/password
DELETE /api/admin/users/{username}
```

Protected API requests use HTTP Basic Auth. The native UI handles this internally after login.

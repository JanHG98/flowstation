# NetCore Control Room v5.0 – User/Passwort Login + RBAC

Dieser Stand ersetzt Operator-/Admin-Tokens durch klassischen Login:

- TBS/Basisstation: bleibt bei `NETCORE_CONTROL_ROOM_NODE_TOKEN` als Maschinen-Token für `/node`.
- LXC/Core: `auth_users` in SQLite + Bootstrap-Admin aus `/etc/netcore-control-room/control-room.env`.
- Windows-UI: Loginmaske mit Benutzername + Passwort. Kein `operator.token` mehr.
- RBAC: `viewer`, `operator`, `admin`.

## Token-/Passwort-Zuordnung

### LXC `/etc/netcore-control-room/control-room.env`

```bash
NETCORE_CONTROL_ROOM_NODE_TOKEN=<node-token-fuer-TBS>
NETCORE_CONTROL_ROOM_BOOTSTRAP_USER=jan
NETCORE_CONTROL_ROOM_BOOTSTRAP_PASSWORD=<admin-passwort>
```

### LXC `/etc/netcore-control-room/control-room.toml`

```toml
[auth]
enabled = true
allow_health_unauthenticated = true
node_token_env = "NETCORE_CONTROL_ROOM_NODE_TOKEN"
bootstrap_username_env = "NETCORE_CONTROL_ROOM_BOOTSTRAP_USER"
bootstrap_password_env = "NETCORE_CONTROL_ROOM_BOOTSTRAP_PASSWORD"
bootstrap_role = "admin"
```

### TBS `config.toml`

```toml
[control_room]
token = "<Wert aus NETCORE_CONTROL_ROOM_NODE_TOKEN>"
```

### Windows `%APPDATA%\netcore\control-room\operator.toml`

```toml
[profiles.default]
api = "http://10.0.1.25:9010"
default_node = "SRV-M_TBS-01"
operator_id = "jan"
username = "jan"
```

Das Passwort wird in der UI eingegeben, nicht als Token-Datei gespeichert.

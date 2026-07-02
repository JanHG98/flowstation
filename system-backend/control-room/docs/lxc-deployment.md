# LXC Deployment – User/Passwort Login + RBAC

## Secrets

```bash
install -d -m 700 /etc/netcore-control-room
cat > /etc/netcore-control-room/control-room.env <<'EOF'
NETCORE_CONTROL_ROOM_NODE_TOKEN=<node-token-fuer-TBS>
NETCORE_CONTROL_ROOM_BOOTSTRAP_USER=jan
NETCORE_CONTROL_ROOM_BOOTSTRAP_PASSWORD=<admin-passwort>
EOF
chmod 600 /etc/netcore-control-room/control-room.env
```

Der Node-Token bleibt auch auf der TBS in `[control_room] token = "..."`.

## Config

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

## Tests

```bash
source /etc/netcore-control-room/control-room.env
curl -i http://127.0.0.1:9010/health
curl -i http://127.0.0.1:9010/api/overview
curl -u "$NETCORE_CONTROL_ROOM_BOOTSTRAP_USER:$NETCORE_CONTROL_ROOM_BOOTSTRAP_PASSWORD" \
  http://127.0.0.1:9010/api/me | jq
curl -u "$NETCORE_CONTROL_ROOM_BOOTSTRAP_USER:$NETCORE_CONTROL_ROOM_BOOTSTRAP_PASSWORD" \
  http://127.0.0.1:9010/api/admin/users | jq
```

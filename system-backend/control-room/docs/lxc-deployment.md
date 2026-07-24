# LXC Deployment

## Voraussetzungen

```bash
apt update
apt install -y build-essential pkg-config curl ca-certificates
useradd --system --home /var/lib/netcore-control-room --shell /usr/sbin/nologin netcore || true
```

Rust über die gewünschte Toolchain installieren, Repository auschecken und dann:

```bash
sudo system-backend/control-room/install/install.sh
```

Konfiguration:

```text
/etc/netcore-control-room/control-room.toml
```

State:

```text
/var/lib/netcore-control-room/control-room.sqlite3
/var/lib/netcore-control-room/operations.json
/var/lib/netcore-control-room/operations.json.bak
```

Nach Anpassung der LXC-Adressen:

```bash
systemctl restart netcore-control-room
curl http://127.0.0.1:9010/health/live
curl http://127.0.0.1:9010/health/ready
```

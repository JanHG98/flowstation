# LXC Deployment

Empfehlung für die Testumgebung:

- Debian 13 LXC
- 2 vCPU
- 1 bis 2 GiB RAM
- 8 GiB Storage
- feste Management-IP
- Erreichbarkeit zu Node Gateway TCP 8080
- WebUI TCP 8160 nur im isolierten Labornetz

Installation aus dem Repository:

```bash
sudo system-backend/packet-core/install/install.sh
```

Prüfung:

```bash
systemctl status netcore-packet-core
journalctl -u netcore-packet-core -f
curl http://127.0.0.1:8160/health/live
curl http://127.0.0.1:8160/health/ready
```

Der Container benötigt **kein** `/dev/net/tun` und kein `CAP_NET_ADMIN`. Diese Rechte gehören später ausschließlich zum IP-Gateway-LXC.

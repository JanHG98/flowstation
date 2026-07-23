# LXC-Betrieb

Empfohlen: Debian-LXC, 1 vCPU, 512 MiB RAM, statische IP. Der Dienst benötigt ausgehend Zugriff auf den Node Gateway und eingehend TCP 8100 für WebUI/API.

```bash
cd /opt/netcore-tetra
sudo system-backend/subscriber-core/install/install.sh
```

Datenbank: `/var/lib/netcore-subscriber-core/subscribers.json`  
Konfiguration: `/etc/netcore/subscriber-core.toml`

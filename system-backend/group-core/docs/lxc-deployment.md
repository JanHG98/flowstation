# LXC-Betrieb

Empfehlung für das Testsystem:

- Debian-LXC
- eigene IP-Adresse
- TCP 8110 für WebUI/API
- ausgehend TCP 8080 zum Node Gateway
- `/var/lib/netcore-group-core` persistent sichern

Installation:

```bash
cd /opt/netcore-tetra
sudo system-backend/group-core/install/install.sh
```

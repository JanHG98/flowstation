# LXC-Bereitstellung

Empfohlene Laborwerte:

- Debian 12 oder Ubuntu 24.04
- 2 vCPU
- 1–2 GiB RAM
- 8 GiB Datenträger
- feste IP im Managementnetz
- TCP 8120 von Bedienplätzen
- ausgehender WebSocket zum Node Gateway auf TCP 8080

Installation:

```bash
cd /opt/netcore-tetra
sudo cp system-backend/call-control/config/call-control.example.toml /etc/netcore/call-control.toml
sudo nano /etc/netcore/call-control.toml
sudo system-backend/call-control/install/install.sh
```

Prüfung:

```bash
systemctl status netcore-call-control --no-pager
journalctl -u netcore-call-control -n 150 --no-pager
curl http://127.0.0.1:8120/health/live
curl http://127.0.0.1:8120/api/v1/status
```

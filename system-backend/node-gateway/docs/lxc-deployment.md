# LXC-Bereitstellung

## Empfohlene Testressourcen

- Debian 12 LXC
- 2 vCPU
- 1 bis 2 GB RAM
- 8 GB Datenträger
- feste IP im isolierten Test-/Management-VLAN
- TCP-Port 8080 zwischen TBS, Backend-Testdiensten und Administrations-PC

## Pakete

```bash
apt update
apt install -y build-essential pkg-config libssl-dev git curl ca-certificates
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source /root/.cargo/env
```

## Installation

Repository in den LXC kopieren und anschließend:

```bash
cd /opt/netcore-tetra
sudo system-backend/node-gateway/install/install.sh
```

WebUI:

```text
http://<LXC-IP>:8080/
```

## TBS anbinden

Der Gateway spricht für die erste Ausbaustufe dasselbe Node-Protokoll wie der bisherige Control Room. In der TBS-Konfiguration wird daher der bisherige `[control_room]`-Endpunkt auf den Gateway gelegt:

```toml
[control_room]
host = "<LXC-IP>"
port = 8080
use_tls = false
endpoint_path = "/ws/node"
# Keine credentials und kein Token im offenen Testmodus.
```

Der Gateway setzt den vom bestehenden TBS-Transport erwarteten Kompatibilitätsmarker und handelt `netcore-control-room-node-v1` aus.

## Kontrolle

```bash
systemctl status netcore-node-gateway
journalctl -u netcore-node-gateway -f
curl http://127.0.0.1:8080/health/live
curl http://127.0.0.1:8080/api/v1/status
```

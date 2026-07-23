# SWMI Core 1 – Paket G anwenden

## 1. Packet-Core-LXC vorbereiten

Empfohlen: Debian 13, 2 vCPU, 1–2 GiB RAM, feste Management-IP.

## 2. Installieren

```bash
cd /opt/netcore-tetra
sudo system-backend/packet-core/install/install.sh
```

## 3. Konfigurieren

```bash
sudo editor /etc/netcore/packet-core.toml
sudo systemctl restart netcore-packet-core
```

Node Gateway muss unter der in `[node_gateway].url` eingetragenen Adresse erreichbar sein.

## 4. Prüfen

```bash
curl http://127.0.0.1:8160/health/live
curl http://127.0.0.1:8160/health/ready
curl http://127.0.0.1:8160/api/v1/status
journalctl -u netcore-packet-core -f
```

WebUI:

```text
http://<Packet-Core-IP>:8160/
```

## 5. Betriebsmodus

Zuerst immer:

```toml
[packet]
mode = "shadow"
```

Erst nach stabilen Vergleichsläufen in einem isolierten Testnetz auf `authoritative` umstellen.

## 6. Rollback

Der Dienst ist nicht im zeitkritischen RF-Pfad. Ein Stop des Packet Core lässt die lokale TBS-SNDCP-Implementierung weiterarbeiten. Für einen vollständigen Rückbau:

```bash
sudo system-backend/packet-core/install/uninstall.sh
```

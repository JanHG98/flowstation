# SWMI Core 1 – Paket H anwenden

## 1. LXC vorbereiten

Empfohlen: Debian 13, 2 vCPU, 1–2 GiB RAM, feste Management-IP.

Proxmox-LXC:

```text
lxc.cgroup2.devices.allow: c 10:200 rwm
lxc.mount.entry: /dev/net/tun dev/net/tun none bind,create=file
```

Pakete:

```bash
apt update
apt install -y iproute2 nftables ca-certificates
```

## 2. Installieren

```bash
cd /opt/netcore-tetra
sudo system-backend/ip-gateway/install/install.sh
```

## 3. Erst im Shadow-Modus prüfen

```toml
[interface]
mode = "shadow"
```

```bash
curl http://127.0.0.1:8170/api/v1/status
curl http://127.0.0.1:8170/api/v1/kernel/plan
```

## 4. Echten Datenweg aktivieren

`nat.egress_interface` an den LXC anpassen, danach:

```toml
[interface]
mode = "authoritative"
```

```bash
sudo systemctl restart netcore-ip-gateway
journalctl -u netcore-ip-gateway -f
```

## 5. Funktionstest

WebUI:

```text
http://<IP-Gateway-LXC>:8170/
```

Aus dem TETRA-Paketdatennetz:

```text
DNS:      10.0.0.1
WAP:      http://wap.netcore.test:8088/wap/
HTTP:     http://test.netcore.test:8088/test/echo
UDP Echo: test.netcore.test:7007
```

## 6. Rollback

```bash
sudo system-backend/ip-gateway/install/uninstall.sh
sudo nft delete table inet netcore_ip_gateway
sudo nft delete table ip netcore_ip_gateway_nat
```
